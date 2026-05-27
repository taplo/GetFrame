# Plan 09-02: 水平扩展 + Docker 生产化

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Worker 支持水平扩展（PostgreSQL 抢占 + 心跳），Docker 多阶段构建（Alpine + musl），KEDA 指标。

**Architecture:** 单一二进制（API + 管道同进程），WorkerManager 通过 DB UPDATE ... WHERE claimed_by IS NULL 抢占 stream，15s 心跳保活，30s 超时释放。Docker 在 Alpine 上静态链接 FFmpeg + 编译前端。

**Tech Stack:** Rust (tokio), sqlx (pure Rust PG, no libpq), Alpine Linux, musl, ffmpeg-dev (Alpine pkg)

---

### Task 1: DB Migration — workers 表 + claimed_by 字段

**Files:**
- Create: `migrations/20260527_000001_horizontal_scaling.sql`

- [ ] **Step 1: 创建 migration 文件**

写入 `migrations/20260527_000001_horizontal_scaling.sql`:

```sql
CREATE TABLE IF NOT EXISTS workers (
    id TEXT PRIMARY KEY,
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE streams ADD COLUMN IF NOT EXISTS claimed_by TEXT REFERENCES workers(id);
ALTER TABLE streams ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_streams_claimable ON streams(claimed_by);
```

- [ ] **Step 2: 编译验证**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo build 2>&1 | Select-Object -Last 5`
Expected: migrations embedded via `sqlx::migrate!("./migrations")`, build succeeds

---

### Task 2: WorkerConfig — config.rs 增强

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: 添加 WorkerConfig 结构体**

在 `DatabaseConfig` 之后添加：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
    #[serde(default = "default_claim_batch_size")]
    pub claim_batch_size: u32,
    #[serde(default = "default_claim_timeout")]
    pub claim_timeout_secs: u64,
}

fn default_heartbeat_interval() -> u64 { 15 }
fn default_claim_batch_size() -> u32 { 5 }
fn default_claim_timeout() -> u64 { 30 }
```

在 `Config` 结构体中加入 `worker` 字段：

```rust
pub struct Config {
    #[serde(default)]
    pub preload_streams: Vec<StreamConfig>,
    pub storage: StorageConfig,
    pub kafka: KafkaConfig,
    pub http: HttpConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub database: Option<DatabaseConfig>,
    #[serde(default)]
    pub worker: Option<WorkerConfig>,   // <-- 新增
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo build 2>&1 | Select-Object -Last 5`
Expected: build succeeds

---

### Task 3: WorkerManager 模块

**Files:**
- Create: `src/worker/mod.rs`
- Modify: `src/lib.rs` (添加 `pub mod worker;`)

- [ ] **Step 1: 创建 WorkerManager**

写入 `src/worker/mod.rs`:

```rust
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::RwLock;

use crate::config::{StreamConfig, WorkerConfig};
use crate::stream::StreamManager;
use crate::types::StreamId;

pub struct WorkerManager {
    pub worker_id: String,
    db_pool: PgPool,
    stream_manager: StreamManager,
    config: WorkerConfig,
    shutdown_token: CancellationToken,
}

impl WorkerManager {
    pub fn new(
        worker_id: String,
        db_pool: PgPool,
        stream_manager: StreamManager,
        config: WorkerConfig,
        shutdown_token: CancellationToken,
    ) -> Self {
        Self { worker_id, db_pool, stream_manager, config, shutdown_token }
    }

    pub async fn run(self: Arc<Self>) {
        tracing::info!(worker_id = %self.worker_id, "WorkerManager started");

        // Register + initial heartbeat
        sqlx::query(
            "INSERT INTO workers (id, heartbeat_at) VALUES ($1, NOW()) \
             ON CONFLICT (id) DO UPDATE SET heartbeat_at = NOW()"
        )
        .bind(&self.worker_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| tracing::error!(error = %e, "Failed to register worker"))
        .ok();

        // Initial claim
        self.claim_loop_iteration().await;

        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.config.heartbeat_interval_secs)
        );
        interval.tick().await; // skip immediate tick

        loop {
            tokio::select! {
                _ = self.shutdown_token.cancelled() => {
                    tracing::info!(worker_id = %self.worker_id, "WorkerManager shutting down");
                    self.release_all_claims().await;
                    break;
                }
                _ = interval.tick() => {
                    self.claim_loop_iteration().await;
                }
            }
        }
    }

    async fn claim_loop_iteration(&self) {
        self.heartbeat().await;
        self.claim_streams().await;
        self.release_stale_streams().await;
    }

    async fn heartbeat(&self) {
        let result = sqlx::query(
            "UPDATE workers SET heartbeat_at = NOW() WHERE id = $1"
        )
        .bind(&self.worker_id)
        .execute(&self.db_pool)
        .await;

        match result {
            Ok(_) => tracing::trace!(worker_id = %self.worker_id, "Heartbeat"),
            Err(e) => tracing::warn!(error = %e, worker_id = %self.worker_id, "Heartbeat failed"),
        }
    }

    async fn claim_streams(&self) {
        let timeout_interval = self.config.claim_timeout_secs as i64;
        let batch_size = self.config.claim_batch_size as i64;

        let rows = sqlx::query_as::<_, ClaimRow>(
            r#"UPDATE streams SET claimed_by = $1, claimed_at = NOW()
               WHERE id IN (
                   SELECT id FROM streams
                   WHERE claimed_by IS NULL
                      OR claimed_at < NOW() - make_interval(secs => $2)
                   ORDER BY created_at ASC
                   LIMIT $3
               )
               RETURNING id, name, description, tags, source_url, source_type, stream_type,
                         extract_interval_seconds, jpeg_quality, ffmpeg_threads, rtsp_transport,
                         storage_config, kafka_config"#
        )
        .bind(&self.worker_id)
        .bind(timeout_interval)
        .bind(batch_size)
        .fetch_all(&self.db_pool)
        .await
        .unwrap_or_default();

        for row in &rows {
            let id: StreamId = row.id;
            if self.stream_manager.registry().exists(&id) {
                self.stream_manager.start_pipeline(&id);
                tracing::info!(worker_id = %self.worker_id, stream_id = %id, "Claimed existing stream");
            } else {
                let config = row_to_config(row);
                self.stream_manager.registry().add(id, config.clone());
                self.stream_manager.start_pipeline(&id);
                tracing::info!(worker_id = %self.worker_id, stream_id = %id, "Claimed new stream");
            }
            crate::metrics::CLAIMED_STREAMS.increment(1.0);
        }
    }

    async fn release_stale_streams(&self) {
        let stale_ids: Vec<StreamId> = {
            let mut local: Vec<StreamId> = Vec::new();
            // We can't easily enumerate which streams we have running.
            // Instead, query the DB for streams claimed_by us:
            let rows = sqlx::query_as::<_, (uuid::Uuid,)>(
                "SELECT id FROM streams WHERE claimed_by = $1"
            )
            .bind(&self.worker_id)
            .fetch_all(&self.db_pool)
            .await
            .unwrap_or_default();

            let active_ids: HashSet<StreamId> = rows.into_iter().map(|r| r.0).collect();

            for id in self.stream_manager.registry().all_ids() {
                if !active_ids.contains(&id) {
                    local.push(id);
                }
            }
            local
        };

        for id in stale_ids {
            self.stream_manager.stop_pipeline(&id);
            tracing::info!(worker_id = %self.worker_id, stream_id = %id, "Released stream (no longer claimed)");
        }
    }

    async fn release_all_claims(&self) {
        // Stop all local pipelines
        for id in self.stream_manager.registry().all_ids() {
            self.stream_manager.stop_pipeline(&id);
        }

        // Release claims in DB
        let _ = sqlx::query(
            "UPDATE streams SET claimed_by = NULL, claimed_at = NULL WHERE claimed_by = $1"
        )
        .bind(&self.worker_id)
        .execute(&self.db_pool)
        .await;

        let _ = sqlx::query("DELETE FROM workers WHERE id = $1")
            .bind(&self.worker_id)
            .execute(&self.db_pool)
            .await;

        tracing::info!(worker_id = %self.worker_id, "All claims released");
    }
}

#[derive(sqlx::FromRow)]
struct ClaimRow {
    id: uuid::Uuid,
    name: String,
    description: String,
    tags: serde_json::Value,
    source_url: String,
    source_type: String,
    stream_type: Option<String>,
    extract_interval_seconds: f64,
    jpeg_quality: i32,
    ffmpeg_threads: i32,
    rtsp_transport: String,
    storage_config: Option<serde_json::Value>,
    kafka_config: Option<serde_json::Value>,
}

fn row_to_config(row: &ClaimRow) -> StreamConfig {
    let tags: std::collections::HashMap<String, String> =
        serde_json::from_value(row.tags.clone()).unwrap_or_default();
    let storage = row.storage_config.as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let kafka = row.kafka_config.as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    StreamConfig {
        name: row.name.clone(),
        description: row.description.clone(),
        tags,
        source_url: row.source_url.clone(),
        source_type: row.source_type.clone(),
        stream_type: row.stream_type.clone(),
        extract_interval_seconds: row.extract_interval_seconds,
        jpeg_quality: row.jpeg_quality as u8,
        ffmpeg_threads: row.ffmpeg_threads,
        rtsp_transport: row.rtsp_transport.clone(),
        storage,
        kafka,
    }
}
```

- [ ] **Step 2: 在 lib.rs 中注册模块**

修改 `src/lib.rs`:

```rust
pub mod config;
pub mod types;
pub mod logging;
pub mod pipeline;
pub mod storage;
pub mod kafka;
pub mod health;
pub mod stream;
pub mod task;
pub mod api;
pub mod metrics;
pub mod db;
pub mod worker;   // <-- 新增
```

- [ ] **Step 3: 编译验证**

Run: `cargo build 2>&1 | Select-Object -Last 5`
Expected: build succeeds

---

### Task 4: 集成 WorkerManager 到 main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: 重构 main.rs 启动逻辑**

将主函数中加载 DB stream 后全部启动管道的逻辑改为条件启动：

```rust
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let config_content = std::fs::read_to_string(&cli.config)?;
    let config: config::Config = serde_yaml::from_str(&config_content)?;

    logging::init(&config.logging);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Starting getframe-worker");

    ffmpeg_next::init()?;

    let shutdown_token = tokio_util::sync::CancellationToken::new();
    let storage_client = Arc::new(storage::StorageClient::new(&config.storage).await);

    if let Some(retention_days) = config.storage.retention_days {
        if retention_days > 0 {
            let cleaner = storage::retention::RetentionCleaner::new(
                storage_client.client().clone(),
                config.storage.bucket.clone(),
                retention_days,
            );
            cleaner.start_periodic(std::time::Duration::from_secs(3600));
            tracing::info!(retention_days = retention_days, "S3 retention cleaner scheduled");
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
                    // Pipeline will be started below or by WorkerManager
                }
            }
            Err(e) => tracing::error!(error = %e, "Failed to load streams from database"),
        }
    }

    let is_worker_mode = config.worker.is_some()
        && config.worker.as_ref().map(|w| w.claim_batch_size > 0).unwrap_or(false)
        && db_pool.is_some();

    if is_worker_mode {
        // Worker mode: add to registry + persist to DB, but don't start pipelines
        for stream_cfg in &config.preload_streams {
            let id = uuid::Uuid::new_v4();
            stream_manager.registry().add(id, stream_cfg.clone());
            if let Some(ref pool) = db_pool {
                let _ = db::streams::upsert(pool, &id, stream_cfg).await;
            }
            tracing::info!(stream_id = %id, url = %stream_cfg.source_url, "Pre-loaded stream for worker claiming");
        }
    } else {
        // Non-worker mode: start pipelines immediately (old behavior)
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
            db_pool.clone().unwrap(),
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

    let task_manager = Arc::new(task::TaskManager::new(
        Arc::new(stream_manager.clone()),
        db_pool.clone(),
    ));

    let health_router = health::health_router(health_state.clone());
    let api_router = api::api_router(stream_manager.clone(), task_manager);
    let api_doc = crate::api::ApiDoc::openapi();

    let app = health_router
        .merge(api_router)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_doc))
        .route("/metrics", axum::routing::get(metrics::metrics_handler))
        .nest_service("/", ServeDir::new("web/dist"));

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

    tracing::info!("Draining all pipelines...");
    stream_manager.shutdown_all();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    tracing::info!("getframe-worker shut down cleanly");

    Ok(())
}
```

- [ ] **Step 2: 给 StreamRegistry 添加 all_ids 方法**

修改 `src/stream/registry.rs`：

在 `is_empty()` 方法之后添加：

```rust
pub fn all_ids(&self) -> Vec<StreamId> {
    let inner = self.inner.read().unwrap();
    inner.streams.keys().copied().collect()
}
```

- [ ] **Step 3: 编译验证**

Run: `cargo build 2>&1 | Select-Object -Last 5`
Expected: build succeeds

---

### Task 5: 新增 metrics

**Files:**
- Modify: `src/metrics.rs`

- [ ] **Step 1: 添加 claim 指标**

在文件末尾添加：

```rust
pub static CLAIMED_STREAMS: LazyLock<metrics::Gauge> = LazyLock::new(|| {
    gauge!("getframe_streams_claimed")
});
pub static CLAIM_ERRORS: LazyLock<metrics::Counter> = LazyLock::new(|| {
    counter!("getframe_claim_errors_total")
});
```

- [ ] **Step 2: 编译验证**

Run: `cargo build 2>&1 | Select-Object -Last 5`
Expected: build succeeds

---

### Task 6: 配置示例更新

**Files:**
- Modify: `config.example.yaml`

- [ ] **Step 1: 添加 worker 配置段**

在 `database:` 之后添加：

```yaml
worker:
  id: ""                           # 默认取 K8s pod hostname
  heartbeat_interval_secs: 15      # 心跳间隔
  claim_batch_size: 5              # 每次抢占 stream 数
  claim_timeout_secs: 30           # claim 过期秒数
```

完整 `config.example.yaml` 末尾新增部分。

- [ ] **Step 2: 验证 yaml 解析**

无需单独步骤 — Rust 类型系统会在编译时验证字段名。

---

### Task 7: Dockerfile 重写

**Files:**
- Modify: `Dockerfile`
- Create: `.dockerignore`

- [ ] **Step 1: 写入 .dockerignore**

```dockerignore
target/
.git/
*.md
.planning/
docs/
```

- [ ] **Step 2: 重写 Dockerfile**

```dockerfile
# Stage 1: Web UI build
FROM node:20-alpine AS web-builder
WORKDIR /web
COPY web/package.json web/ ./
RUN npm ci && npm run build

# Stage 2: Rust binary (musl, Alpine FFmpeg)
FROM rust:1.85-alpine3.20 AS rust-builder
RUN apk add --no-cache \
    ffmpeg-dev ffmpeg-libs \
    musl-dev pkgconfig cmake make g++ \
    openssl-dev
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ .cargo/
COPY src/ src/
COPY migrations/ migrations/
ENV PKG_CONFIG_ALLOW_CROSS=1
RUN cargo build --release --bin getframe-worker

# Stage 3: Minimal runtime image
FROM alpine:3.20
RUN apk add --no-cache ca-certificates ffmpeg-libs
COPY --from=rust-builder /app/target/release/getframe-worker /getframe-worker
COPY --from=web-builder /web/dist /web/dist
COPY config.example.yaml /etc/getframe/config.yaml
EXPOSE 8080
ENTRYPOINT ["/getframe-worker"]
CMD ["--config", "/etc/getframe/config.yaml"]
```

> 注意: `ffmpeg-libs` 在运行时需要以提供 FFmpeg 共享库（.so），Rust 二进制在 Alpine 上动态链接这些库。`ffmpeg-dev`（编译时头文件+静态库）仅在 builder 中需要。

- [ ] **Step 3: Docker build 验证**

Run: `docker build -t getframe-worker:test . 2>&1 | tail -10`
Expected: 构建成功，exit code 0

---

### Task 8: 编译 & 测试验证

- [ ] **Step 1: 全量编译**

Run: `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"; cargo build 2>&1 | Select-Object -Last 5`
Expected: `Finished dev profile` — 无错误

- [ ] **Step 2: 代码审查检查**

确认以下边界情况：
1. `worker:` 配置块不存在时（dev 模式），旧行为完全不变 — 所有 stream 立即启动 pipeline
2. `worker:` 存在但 `claim_batch_size: 0` 时等同于 worker 模式关闭
3. 没有 DB 时 worker 模式报错退出（或 fallback 到非 worker 模式）
