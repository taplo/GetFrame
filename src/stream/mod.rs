pub mod health;
pub mod registry;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use crate::config::StreamConfig;
use crate::pipeline;
use crate::pipeline::rule::RuleConfig;
use crate::stream::health::StreamHealth;
use crate::stream::registry::StreamRegistry;
use crate::types::StreamId;

#[derive(Debug, Clone)]
pub enum PipelineExitReason {
    UserInitiated,
    #[allow(dead_code)]
    Error(String),
    #[allow(dead_code)]
    Eof,
}

#[allow(dead_code)]
struct PipelineHandle {
    shutdown_token: CancellationToken,
    join_handle: Option<std::thread::JoinHandle<()>>,
    health_handle: Arc<Mutex<StreamHealth>>,
    rules_shared: Arc<RwLock<Vec<RuleConfig>>>,
    exit_tx: tokio::sync::watch::Sender<Option<PipelineExitReason>>,
}

#[derive(Clone)]
pub struct StreamManager {
    registry: StreamRegistry,
    pipelines: Arc<Mutex<HashMap<StreamId, PipelineHandle>>>,
    storage_client: Arc<crate::storage::StorageClient>,
    kafka_producer: Arc<crate::kafka::KafkaProducer>,
    max_backoff_seconds: u64,
    db_pool: Option<sqlx::MySqlPool>,
    stream_counter: Arc<AtomicUsize>,
}

impl StreamManager {
    pub fn new(
        storage_client: Arc<crate::storage::StorageClient>,
        kafka_producer: Arc<crate::kafka::KafkaProducer>,
    ) -> Self {
        Self {
            registry: StreamRegistry::new(),
            pipelines: Arc::new(Mutex::new(HashMap::new())),
            storage_client,
            kafka_producer,
            max_backoff_seconds: 30,
            db_pool: None,
            stream_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn with_db(mut self, pool: sqlx::MySqlPool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    pub fn registry(&self) -> &StreamRegistry {
        &self.registry
    }

    pub fn add_stream(
        &self,
        config: StreamConfig,
    ) -> StreamId {
        let id = StreamId::new_v4();
        self.registry.add(id, config.clone());

        let shutdown_token = CancellationToken::new();
        let health_handle = Arc::new(Mutex::new(StreamHealth::new()));
        let rules_shared = self.registry.get_rules_shared(&id)
            .expect("Stream must exist after add");

        let (exit_tx, exit_rx) = tokio::sync::watch::channel(None::<PipelineExitReason>);

        let mut pipeline = pipeline::Pipeline::start(
            &config, id, shutdown_token.clone(),
            health_handle.clone(), rules_shared.clone(), None,
        );

        self.spawn_frame_consumer(id, &config, pipeline.extracted_rx.clone(),
            shutdown_token.clone(), health_handle.clone());

        let handle = PipelineHandle {
            shutdown_token,
            join_handle: pipeline.decode_handle.take(),
            health_handle: health_handle.clone(),
            rules_shared: rules_shared.clone(),
            exit_tx,
        };
        self.pipelines.lock().unwrap().insert(id, handle);

        self.spawn_reconnection_task(id, config, exit_rx, health_handle, rules_shared);

        {
            let pool = self.db_pool.clone();
            let sid = id;
            let cfg = self.registry.get(&id).map(|i| i.config.clone());
            tokio::spawn(async move {
                if let (Some(p), Some(c)) = (pool, cfg) {
                    let _ = crate::db::streams::upsert(&p, &sid, &c).await;
                }
            });
        }

        crate::metrics::STREAMS_ACTIVE.increment(1.0);
        crate::metrics::STREAMS_TOTAL.increment(1);

        id
    }

    fn spawn_frame_consumer(
        &self,
        stream_id: StreamId,
        config: &StreamConfig,
        extracted_rx: crossbeam::channel::Receiver<crate::types::ExtractedFrame>,
        shutdown_token: CancellationToken,
        health_handle: Arc<Mutex<StreamHealth>>,
    ) {
        let st = self.storage_client.clone();
        let kp = self.kafka_producer.clone();
        let sid = stream_id;

        let kafka_topic_override: Option<String> = config.kafka.as_ref().map(|k| k.topic.clone());
        let partition_key_field: String = config.kafka.as_ref()
            .and_then(|k| k.partition_key_field.clone())
            .unwrap_or_else(|| "frame_number".into());

        tokio::spawn(async move {
            let kafka_topic_override_clone = kafka_topic_override.clone();
            let pk_field = partition_key_field.clone();
            loop {
                tokio::select! {
                    _ = shutdown_token.cancelled() => {
                        tracing::info!(stream_id = %sid, "Frame consumer shut down");
                        break;
                    }
                    result = tokio::task::spawn_blocking({
                        let rx = extracted_rx.clone();
                        move || rx.recv()
                    }) => {
                        match result {
                            Ok(Ok(frame)) => {
                                match st.upload_frame(&frame).await {
                                    Ok((url, key)) => {
                                        {
                                            let mut h = health_handle.lock().unwrap();
                                            h.record_frame_stored(key.clone());
                                        }
                                        let bucket = st.bucket().to_string();
                                        let partition_key = match pk_field.as_str() {
                                            "stream_id" => sid.to_string(),
                                            "timestamp" => format!("{}", frame.timestamp_seconds),
                                            _ => format!("{}", frame.frame_number),
                                        };
                                        if let Err(e) = kp.publish_metadata(
                                            &frame, &url, &bucket, &key,
                                            kafka_topic_override_clone.as_deref(),
                                            &partition_key,
                                        ).await {
                                            tracing::error!(error = %e, stream_id = %sid, "Metadata publish failed");
                                            crate::metrics::KAFKA_ERRORS.increment(1);
                                        }
                                        crate::metrics::FRAMES_PROCESSED.increment(1);
                                    }
                                    Err(e) => {
                                        tracing::error!(error = %e, stream_id = %sid, "Upload failed");
                                        crate::metrics::STORAGE_ERRORS.increment(1);
                                    }
                                }
                            }
                            _ => {
                                tracing::info!(stream_id = %sid, "Frame channel closed");
                                break;
                            }
                        }
                    }
                }
            }
        });
    }

    fn spawn_reconnection_task(
        &self,
        stream_id: StreamId,
        config: StreamConfig,
        mut exit_rx: tokio::sync::watch::Receiver<Option<PipelineExitReason>>,
        health_handle: Arc<Mutex<StreamHealth>>,
        rules_shared: Arc<RwLock<Vec<RuleConfig>>>,
    ) {
        let pipelines = self.pipelines.clone();
        let registry = self.registry.clone();
        let storage_client = self.storage_client.clone();
        let kafka_producer = self.kafka_producer.clone();
        let max_backoff = self.max_backoff_seconds;
        tokio::spawn(async move {
            let mut backoff_seconds: u64 = 1;

            loop {
                if exit_rx.changed().await.is_err() {
                    break;
                }

                let reason = exit_rx.borrow().clone();
                match &reason {
                    Some(PipelineExitReason::UserInitiated) => {
                        tracing::info!(stream_id = %stream_id, "Pipeline removed by user");
                        break;
                    }
                    Some(PipelineExitReason::Eof) => {
                        if config.source_type == "file" || config.source_type == "lavfi" {
                            tracing::info!(stream_id = %stream_id, "File stream finished, pipeline will not restart");
                            break;
                        }
                        tracing::info!(stream_id = %stream_id, "Pipeline reached end of stream, reconnecting");
                    }
                    Some(PipelineExitReason::Error(e)) => {
                        tracing::warn!(stream_id = %stream_id, error = %e, "Pipeline terminated with error, reconnecting");
                    }
                    None => continue,
                }

                {
                    let mut h = health_handle.lock().unwrap();
                    match &reason {
                        Some(PipelineExitReason::Error(e)) => h.mark_error(e),
                        _ => h.mark_error("stream disconnected"),
                    }
                }

                tracing::info!(
                    stream_id = %stream_id,
                    backoff_seconds = backoff_seconds,
                    "Reconnecting in {} seconds...", backoff_seconds
                );
                tokio::time::sleep(std::time::Duration::from_secs(backoff_seconds)).await;
                // Exponential backoff: double for next failure, cap at max
                backoff_seconds = (backoff_seconds * 2).min(max_backoff);

                if !registry.exists(&stream_id) {
                    tracing::info!(stream_id = %stream_id, "Stream removed during backoff");
                    break;
                }

                {
                    let mut h = health_handle.lock().unwrap();
                    h.mark_connecting();
                }

                let new_shutdown = CancellationToken::new();
                let (new_exit_tx, new_exit_rx) = tokio::sync::watch::channel(None::<PipelineExitReason>);

                let cfg = registry.get(&stream_id)
                    .map(|info| info.config.clone())
                    .unwrap_or_else(|| config.clone());

                let mut pipeline = pipeline::Pipeline::start(
                    &cfg, stream_id, new_shutdown.clone(),
                    health_handle.clone(), rules_shared.clone(), None,
                );

                let st = storage_client.clone();
                let kp = kafka_producer.clone();
                let sid = stream_id;
                let sh = new_shutdown.clone();
                let hh = health_handle.clone();
                let kafka_topic_override_re: Option<String> = cfg.kafka.as_ref().map(|k| k.topic.clone());
                let pk_field_re: String = cfg.kafka.as_ref()
                    .and_then(|k| k.partition_key_field.clone())
                    .unwrap_or_else(|| "frame_number".into());
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = sh.cancelled() => break,
                            result = tokio::task::spawn_blocking({
                                let rx = pipeline.extracted_rx.clone();
                                move || rx.recv()
                            }) => {
                                match result {
                                    Ok(Ok(frame)) => {
                                        match st.upload_frame(&frame).await {
                                            Ok((url, key)) => {
                                                {
                                                    let mut h = hh.lock().unwrap();
                                                    h.record_frame_stored(key.clone());
                                                }
                                                let partition_key = match pk_field_re.as_str() {
                                                    "stream_id" => sid.to_string(),
                                                    "timestamp" => format!("{}", frame.timestamp_seconds),
                                                    _ => format!("{}", frame.frame_number),
                                                };
                                                if let Err(e) = kp.publish_metadata(
                                                    &frame, &url, st.bucket(), &key,
                                                    kafka_topic_override_re.as_deref(),
                                                    &partition_key,
                                                ).await {
                                                    tracing::error!(error = %e, stream_id = %sid, "Kafka failed");
                                                    crate::metrics::KAFKA_ERRORS.increment(1);
                                                }
                                                crate::metrics::FRAMES_PROCESSED.increment(1);
                                            }
                                            Err(e) => {
                                                tracing::error!(error = %e, stream_id = %sid, "Upload failed");
                                                crate::metrics::STORAGE_ERRORS.increment(1);
                                            }
                                        }
                                    }
                                    _ => break,
                                }
                            }
                        }
                    }
                });

                {
                    let new_handle = PipelineHandle {
                        shutdown_token: new_shutdown,
                        join_handle: pipeline.decode_handle.take(),
                        health_handle: health_handle.clone(),
                        rules_shared: rules_shared.clone(),
                        exit_tx: new_exit_tx,
                    };
                    pipelines.lock().unwrap().insert(stream_id, new_handle);
                }

                {
                    let mut h = health_handle.lock().unwrap();
                    h.mark_online();
                    h.mark_reconnected();
                }
                crate::metrics::STREAMS_ACTIVE.increment(1.0);
                tracing::info!(stream_id = %stream_id, "Reconnected successfully");

                exit_rx = new_exit_rx;
            }
        });
    }

    pub fn remove_stream(&self, id: &StreamId) -> bool {
        let handle = self.pipelines.lock().unwrap().remove(id);
        if let Some(mut h) = handle {
            h.exit_tx.send(Some(PipelineExitReason::UserInitiated)).ok();
            h.shutdown_token.cancel();
            if let Some(jh) = h.join_handle.take() {
                let _ = jh.join();
            }
            self.registry.remove(id);

            let pool = self.db_pool.clone();
            let sid = *id;
            tokio::spawn(async move {
                if let Some(p) = pool {
                    let _ = crate::db::streams::delete(&p, &sid).await;
                }
            });

            crate::metrics::STREAMS_ACTIVE.decrement(1.0);
            tracing::info!(stream_id = %id, "Stream removed");
            true
        } else {
            false
        }
    }

    pub fn stop_pipeline(&self, id: &StreamId) -> bool {
        let handle = self.pipelines.lock().unwrap().remove(id);
        if let Some(mut h) = handle {
            h.exit_tx.send(Some(PipelineExitReason::UserInitiated)).ok();
            h.shutdown_token.cancel();
            if let Some(jh) = h.join_handle.take() {
                let _ = jh.join();
            }
            crate::metrics::STREAMS_ACTIVE.decrement(1.0);
            tracing::info!(stream_id = %id, "Pipeline stopped");
            true
        } else {
            false
        }
    }

    pub fn start_pipeline(&self, id: &StreamId) -> bool {
        if self.pipelines.lock().unwrap().contains_key(id) {
            return true;
        }

        let info = match self.registry.get(id) {
            Some(info) => info,
            None => {
                tracing::warn!(stream_id = %id, "Cannot start pipeline: stream not found in registry");
                return false;
            }
        };

        let shutdown_token = CancellationToken::new();
        let health_handle = Arc::new(Mutex::new(StreamHealth::new()));
        let rules_shared = match self.registry.get_rules_shared(id) {
            Some(rules) => rules,
            None => return false,
        };

        let (exit_tx, exit_rx) = tokio::sync::watch::channel(None::<PipelineExitReason>);

        let core_to_pin = std::env::var("GETFRAME_CPU_CORES").ok().map(|s| {
            let cores = crate::pipeline::parse_cpu_cores(&s);
            if cores.is_empty() {
                None
            } else {
                let idx = self.stream_counter.fetch_add(1, Ordering::Relaxed);
                Some(cores[idx % cores.len()])
            }
        }).flatten();

        let mut pipeline = pipeline::Pipeline::start(
            &info.config, *id, shutdown_token.clone(),
            health_handle.clone(), rules_shared.clone(), core_to_pin,
        );

        self.spawn_frame_consumer(
            *id, &info.config, pipeline.extracted_rx.clone(),
            shutdown_token.clone(), health_handle.clone(),
        );

        let handle = PipelineHandle {
            shutdown_token,
            join_handle: pipeline.decode_handle.take(),
            health_handle: health_handle.clone(),
            rules_shared: rules_shared.clone(),
            exit_tx,
        };
        self.pipelines.lock().unwrap().insert(*id, handle);

        self.spawn_reconnection_task(
            *id, info.config.clone(), exit_rx,
            health_handle, rules_shared,
        );

        crate::metrics::STREAMS_ACTIVE.increment(1.0);
        tracing::info!(stream_id = %id, "Pipeline started");
        true
    }

    pub fn update_stream_config(&self, id: &StreamId, config: crate::config::StreamConfig) {
        self.registry.update_config(id, config.clone());
        let pool = self.db_pool.clone();
        let sid = *id;
        tokio::spawn(async move {
            if let Some(p) = pool {
                let _ = crate::db::streams::upsert(&p, &sid, &config).await;
            }
        });
    }

    pub fn storage_client(&self) -> Arc<crate::storage::StorageClient> {
        self.storage_client.clone()
    }

    pub fn shutdown_all(&self) {
        let mut pipelines = self.pipelines.lock().unwrap();
        for (id, mut handle) in pipelines.drain() {
            handle.exit_tx.send(Some(PipelineExitReason::UserInitiated)).ok();
            handle.shutdown_token.cancel();
            if let Some(jh) = handle.join_handle.take() {
                let _ = jh.join();
            }
            tracing::info!(stream_id = %id, "Stream shut down");
        }
        crate::metrics::STREAMS_ACTIVE.set(0.0);
    }
}
