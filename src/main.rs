mod config;
mod types;
mod logging;
mod pipeline;
mod storage;
mod kafka;
mod health;
mod stream;
mod task;
mod api;
mod metrics;
mod db;
mod worker;
mod benchmark;

use clap::Parser;
use std::sync::Arc;
use tower_http::services::ServeDir;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(Parser, Debug)]
#[command(name = "getframe-worker", about = "High-performance video frame extraction worker")]
struct Cli {
    #[arg(short, long, default_value = "config.yaml")]
    config: String,

    /// 基准测试模式
    #[arg(long)]
    benchmark: bool,

    /// 并发流数量
    #[arg(long, default_value = "10")]
    streams: usize,

    /// 测试持续时间（秒）
    #[arg(long, default_value_t = 30.0)]
    duration: f64,

    /// JPEG 质量
    #[arg(long, default_value_t = 85)]
    jpeg_quality: u8,

    /// CPU 核心列表，如 "0-3,8-11"
    #[arg(long, default_value = "")]
    cpu_cores: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.benchmark {
        ffmpeg_next::init()?;
        let cpu_cores = crate::pipeline::parse_cpu_cores(&cli.cpu_cores);
        let report = crate::benchmark::run_benchmark(
            cli.streams, cli.duration, cli.jpeg_quality, &cpu_cores,
        );
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    let config_content = std::fs::read_to_string(&cli.config)?;
    let config: config::Config = serde_yaml::from_str(&config_content)?;

    logging::init(&config.logging);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Starting getframe-worker");

    ffmpeg_next::init()?;

    let shutdown_token = tokio_util::sync::CancellationToken::new();

    let storage_client = Arc::new(storage::StorageClient::new(&config.storage).await);

    #[allow(clippy::collapsible_if)]
    if let Some(retention_days) = config.storage.retention_days {
        if retention_days > 0 {
            let cleaner = storage::retention::RetentionCleaner::new(
                storage_client.client().clone(),
                config.storage.bucket.clone(),
                retention_days,
            );
            cleaner.start_periodic(std::time::Duration::from_secs(3600));
            tracing::info!(
                retention_days = retention_days,
                "S3 retention cleaner scheduled (every 60 minutes)"
            );
        }
    }

    let kafka_producer = Arc::new(kafka::KafkaProducer::new(&config.kafka)?);

    let db_pool = if let Some(db_cfg) = &config.database {
        match db::init_pool(&db_cfg.url, db_cfg.max_connections).await {
            Ok(pool) => {
                tracing::info!("Database connected");

                Some(pool)
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to connect to database, running without persistence");
                None
            }
        }
    } else {
        None
    };

    let stream_manager = {
        let mut sm = stream::StreamManager::new(storage_client, kafka_producer);
        if let Some(ref pool) = db_pool {
            sm = sm.with_db(pool.clone());
        }
        sm
    };

    if let Some(ref pool) = db_pool {
        match db::streams::load_all(pool).await {
            Ok(streams) => {
                tracing::info!(count = streams.len(), "Loading streams from database");
                for (id, config) in streams {
                    stream_manager.registry().add(id, config);
                    tracing::info!(stream_id = %id, "Stream restored from DB");
                }
            }
            Err(e) => tracing::error!(error = %e, "Failed to load streams from database"),
        }
    }

    let is_worker_mode = config.worker.as_ref()
        .map(|w| w.claim_batch_size > 0)
        .unwrap_or(false)
        && db_pool.is_some();

    if is_worker_mode {
        for stream_cfg in &config.preload_streams {
            let id = uuid::Uuid::new_v4();
            stream_manager.registry().add(id, stream_cfg.clone());
            #[allow(clippy::collapsible_if)]
            if let Some(ref pool) = db_pool {
                if let Err(e) = db::streams::upsert(pool, &id, stream_cfg).await {
                    tracing::warn!(error = %e, stream_id = %id, "Failed to persist pre-loaded stream");
                }
            }
            tracing::info!(stream_id = %id, url = %stream_cfg.source_url, "Pre-loaded stream for worker claiming");
        }
    } else {
        for stream_cfg in &config.preload_streams {
            let id = stream_manager.add_stream(stream_cfg.clone());
            tracing::info!(stream_id = %id, url = %stream_cfg.source_url, "Pre-loaded stream");
        }
    }

    if is_worker_mode {
        let worker_cfg = config.worker.as_ref().unwrap().clone();
        let worker_id = if worker_cfg.id.is_empty() {
            std::env::var("HOSTNAME").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
        } else {
            worker_cfg.id.clone()
        };

        let worker_mgr = Arc::new(worker::WorkerManager::new(
            worker_id.clone(),
            db_pool.clone().expect("Worker mode requires database pool"),
            stream_manager.clone(),
            worker_cfg,
            shutdown_token.child_token(),
        ));

        tokio::spawn(async move {
            worker_mgr.run().await;
        });
        tracing::info!(worker_id = %worker_id, "Worker mode enabled, streams will be claimed from DB");
    } else {
        tracing::info!("Worker mode disabled, starting all local streams");
        let ids: Vec<_> = stream_manager.registry().all_ids();
        for id in &ids {
            stream_manager.start_pipeline(id);
        }
    }

    let health_state = health::HealthState::new(Some(Arc::new(stream_manager.registry().clone())));

    let task_manager = Arc::new(task::TaskManager::new(Arc::new(stream_manager.clone()), db_pool.clone()));

    if let Some(ref pool) = db_pool {
        let recorder = metrics::MetricsRecorder::new(pool.clone());
        tokio::spawn(recorder.run(shutdown_token.child_token()));
        tracing::info!("MetricsRecorder started (every 60s)");
    }

    let health_router = health::health_router(health_state.clone());
    let api_router = api::api_router(stream_manager.clone(), task_manager, db_pool.clone());
    let api_doc = crate::api::ApiDoc::openapi();

    let app = health_router
        .merge(api_router)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_doc))
        .route("/metrics", axum::routing::get(metrics::metrics_handler))
        .fallback_service(ServeDir::new("web/dist"));

    let listener = tokio::net::TcpListener::bind(
        format!("{}:{}", config.http.bind_address, config.http.bind_port)
    ).await?;

    let shutdown_signal = shutdown_token.clone();
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal.cancelled().await;
        });

    let signal_token = shutdown_token.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");
            term.recv().await;
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c().await.ok();
        }
        tracing::info!("Shutdown signal received, draining pipelines...");
        signal_token.cancel();
    });

    server.await?;

    if !is_worker_mode {
        tracing::info!("Draining all pipelines...");
        stream_manager.shutdown_all();
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    tracing::info!("getframe-worker shut down cleanly");

    Ok(())
}
