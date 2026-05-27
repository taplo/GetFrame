---
phase: 02-multi-stream-management
plan: 01
type: execute
wave: 1
depends_on:
  - 01-core-pipeline-single-stream-foundation
files_modified:
  - Cargo.toml
  - src/main.rs
  - src/lib.rs
  - src/pipeline/mod.rs
  - src/config.rs
files_created:
  - src/stream/mod.rs
  - src/stream/registry.rs
  - src/stream/health.rs
  - src/api/mod.rs
  - src/api/streams.rs
  - src/metrics.rs
autonomous: true
requirements:
  - STREAM-02
  - STREAM-03
  - STREAM-04
  - STREAM-05
  - STREAM-06
  - STREAM-07
  - STREAM-08
  - API-01
  - API-04
  - OBS-01
  - OBS-04
---

<objective>
**Phase 2 Goal:** Transform the single-stream pipeline into a multi-stream management system. Users can add/edit/delete streams at runtime via REST API, monitor per-stream health status, automatically reconnect failed streams with exponential backoff, and export Prometheus metrics.

**Purpose:** Without this phase, the system is limited to exactly one stream configured at startup. Phase 2 enables dynamic stream management — the foundation for all higher-level features (task management, web UI, auto-scaling).

**Output:** A REST API for stream CRUD, a StreamManager that runs N concurrent pipelines, per-stream health tracking with auto-reconnection, and a `/metrics` endpoint.
</objective>

<context>

## Architecture Overview

```
┌───────────────┐     ┌──────────────────────────────────────────────┐
│   HTTP API    │     │                 StreamManager                 │
│  (Axum 0.8)   │     │  ┌─────────┐  ┌─────────┐  ┌─────────┐     │
│               │     │  │Stream 1 │  │Stream 2 │  │Stream N │     │
│  /api/v1/*    │────▶│  │Pipeline │  │Pipeline │  │Pipeline │     │
│  /metrics     │     │  └────┬────┘  └────┬────┘  └────┬────┘     │
│  /health      │     │       │            │            │           │
│  /ready       │     │  ┌────▼────────────▼────────────▼────┐      │
│               │     │  │        StreamRegistry              │      │
│               │     │  │  Arc<RwLock<HashMap<Id, State>>>   │      │
│               │     │  └───────────────────────────────────┘      │
│               │     │  ┌───────────────────────────────────┐      │
│               │     │  │        Reconnection Scheduler      │      │
│               │     │  │  tokio::spawn + exponential backoff│      │
│               │     │  └───────────────────────────────────┘      │
└───────────────┘     └──────────────────────────────────────────────┘
```

### Key Components

- **StreamManager**: Owns all stream pipelines. Methods: add/remove/list/get/reconnect.
- **StreamRegistry**: Thread-safe HashMap storing `StreamConfig + StreamStatus + JoinHandle`.
- **StreamStatus**: Enum(Online, Offline, Error) + last_change_timestamp + error_message.
- **Reconnection**: When a pipeline thread exits, StreamManager's async task detects it and schedules a restart with exponential backoff.
- **Metrics**: Prometheus counters/gauges with `stream_id` label dimension.

### Channel Architecture per Stream

Each stream pipeline still uses the same bounded channel pattern from Phase 1:
```
Ingest → Decode → Rule → Encode → [crossbeam channel] → S3 upload → Kafka
```

The difference: N pipelines each have their own channel set. The async consumer side spins N tokio tasks, one per stream, each consuming from its stream's channel.

</context>

<tasks>

<task type="auto">
<name>Task 1: Add dependencies, create metrics module, create stream types</name>
<files>Cargo.toml, src/metrics.rs, src/stream/mod.rs, src/stream/health.rs</files>
<action>

**Cargo.toml** — Add these dependencies:
```toml
metrics = "0.23"
metrics-exporter-prometheus = "0.16"
serde = { version = "1", features = ["derive"] }  # Already present
```

**src/metrics.rs** — Prometheus metrics definitions:

```rust
use metrics::{counter, gauge, histogram};
use once_cell::sync::Lazy;

pub static STREAMS_ACTIVE: Lazy<metrics::Gauge> = Lazy::new(|| {
    gauge!("getframe_streams_active")
});
pub static STREAMS_TOTAL: Lazy<metrics::Counter> = Lazy::new(|| {
    counter!("getframe_streams_total")
});
pub static FRAMES_PROCESSED: Lazy<metrics::Counter> = Lazy::new(|| {
    counter!("getframe_frames_processed_total")
});
pub static DECODE_ERRORS: Lazy<metrics::Counter> = Lazy::new(|| {
    counter!("getframe_decode_errors_total")
});
pub static STORAGE_ERRORS: Lazy<metrics::Counter> = Lazy::new(|| {
    counter!("getframe_storage_errors_total")
});
pub static KAFKA_ERRORS: Lazy<metrics::Counter> = Lazy::new(|| {
    counter!("getframe_kafka_errors_total")
});
```

**src/stream/health.rs** — Health tracking types:

```rust
use chrono::{DateTime, Utc};
use crate::types::StreamId;

#[derive(Debug, Clone, PartialEq)]
pub enum StreamStatus {
    Online,
    Offline,
    Error(String),
    Connecting,
}

#[derive(Debug, Clone)]
pub struct StreamHealth {
    pub status: StreamStatus,
    pub last_online: Option<DateTime<Utc>>,
    pub last_error: Option<DateTime<Utc>>,
    pub error_count: u64,
    pub uptime_seconds: u64,
    pub frames_decoded: u64,
    pub frames_extracted: u64,
    pub reconnect_count: u64,
    pub last_pts: Option<i64>,
}

impl StreamHealth {
    pub fn new() -> Self {
        Self {
            status: StreamStatus::Connecting,
            last_online: None,
            last_error: None,
            error_count: 0,
            uptime_seconds: 0,
            frames_decoded: 0,
            frames_extracted: 0,
            reconnect_count: 0,
            last_pts: None,
        }
    }
}
```

**src/stream/mod.rs** — Re-export submodules:

```rust
pub mod health;
pub mod registry;
```

**src/lib.rs** — Add new modules:

```rust
pub mod stream;
pub mod api;
pub mod metrics;
```

**src/config.rs** — Add optional stream list for pre-loading:

```rust
// In Config struct, add:
#[serde(default)]
pub preload_streams: Vec<StreamConfig>,
```

</action>
<verify>
cargo check 2>&1
</verify>
<done>
- Cargo.toml updated with metrics deps
- src/metrics.rs defines all Prometheus metrics
- src/stream/health.rs defines StreamStatus and StreamHealth
- src/stream/mod.rs re-exports submodules
- src/config.rs has preload_streams field
- cargo check passes
</done>
</task>

<task type="auto">
<name>Task 2: StreamRegistry — thread-safe stream state storage</name>
<files>src/stream/registry.rs</files>
<action>

**src/stream/registry.rs** — Thread-safe stream registry:

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::config::StreamConfig;
use crate::stream::health::{StreamHealth, StreamStatus};
use crate::types::StreamId;

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub id: StreamId,
    pub config: StreamConfig,
    pub health: StreamHealth,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

struct RegistryInner {
    streams: HashMap<StreamId, StreamInfo>,
}

#[derive(Clone)]
pub struct StreamRegistry {
    inner: Arc<RwLock<RegistryInner>>,
}

impl StreamRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RegistryInner {
                streams: HashMap::new(),
            })),
        }
    }

    pub fn add(&self, id: StreamId, config: StreamConfig) {
        let mut inner = self.inner.write().unwrap();
        let info = StreamInfo {
            id,
            config,
            health: StreamHealth::new(),
            created_at: chrono::Utc::now(),
        };
        inner.streams.insert(id, info);
    }

    pub fn remove(&self, id: &StreamId) -> Option<StreamInfo> {
        let mut inner = self.inner.write().unwrap();
        inner.streams.remove(id)
    }

    pub fn get(&self, id: &StreamId) -> Option<StreamInfo> {
        let inner = self.inner.read().unwrap();
        inner.streams.get(id).cloned()
    }

    pub fn list(&self) -> Vec<StreamInfo> {
        let inner = self.inner.read().unwrap();
        inner.streams.values().cloned().collect()
    }

    pub fn update_health(&self, id: &StreamId, health: StreamHealth) {
        let mut inner = self.inner.write().unwrap();
        if let Some(info) = inner.streams.get_mut(id) {
            info.health = health;
        }
    }

    pub fn update_config(&self, id: &StreamId, config: StreamConfig) -> bool {
        let mut inner = self.inner.write().unwrap();
        if let Some(info) = inner.streams.get_mut(id) {
            info.config = config;
            true
        } else {
            false
        }
    }

    pub fn exists(&self, id: &StreamId) -> bool {
        let inner = self.inner.read().unwrap();
        inner.streams.contains_key(id)
    }

    pub fn len(&self) -> usize {
        let inner = self.inner.read().unwrap();
        inner.streams.len()
    }
}
```

</action>
<verify>
cargo check 2>&1
</verify>
<done>
- StreamRegistry with add/remove/get/list/update_health/update_config
- Thread-safe: Arc<RwLock<HashMap>>
- StreamInfo combines config + health + metadata
- cargo check passes
</done>
</task>

<task type="auto">
<name>Task 3: REST API — Stream CRUD endpoints</name>
<files>src/api/mod.rs, src/api/streams.rs</files>
<action>

**src/api/mod.rs** — API router that composes all routes:

```rust
mod streams;

use axum::Router;
use std::sync::Arc;
use crate::stream::registry::StreamRegistry;

pub fn api_router(registry: Arc<StreamRegistry>) -> Router {
    Router::new()
        .nest("/api/v1/streams", streams::stream_routes(registry))
}
```

**src/api/streams.rs** — Stream CRUD endpoints:

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::config::StreamConfig;
use crate::stream::registry::StreamRegistry;
use crate::stream::health::StreamStatus;
use crate::types::StreamId;

#[derive(Serialize)]
pub struct StreamResponse {
    pub id: StreamId,
    pub config: StreamConfig,
    pub health: StreamStatusResponse,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct StreamStatusResponse {
    pub status: String,
    pub last_online: Option<String>,
    pub last_error: Option<String>,
    pub error_count: u64,
    pub uptime_seconds: u64,
    pub frames_decoded: u64,
    pub frames_extracted: u64,
    pub reconnect_count: u64,
}

#[derive(Deserialize)]
pub struct CreateStreamRequest {
    pub config: StreamConfig,
}

#[derive(Deserialize)]
pub struct UpdateStreamRequest {
    pub config: StreamConfig,
}

#[derive(Serialize)]
pub struct StreamListResponse {
    pub streams: Vec<StreamResponse>,
}

#[derive(Serialize)]
pub struct TestConnectionResponse {
    pub reachable: bool,
    pub latency_ms: u64,
    pub message: String,
}

pub fn stream_routes(registry: Arc<StreamRegistry>) -> Router {
    Router::new()
        .route("/", axum::routing::get(list_streams).post(create_stream))
        .route("/{id}", axum::routing::get(get_stream).put(update_stream).delete(delete_stream))
        .route("/{id}/test", axum::routing::post(test_connection))
        .with_state(registry)
}

async fn list_streams(
    State(registry): State<Arc<StreamRegistry>>,
) -> Json<StreamListResponse> {
    let streams = registry.list();
    let responses: Vec<StreamResponse> = streams.into_iter()
        .map(|info| to_response(info))
        .collect();
    Json(StreamListResponse { streams: responses })
}

async fn create_stream(
    State(registry): State<Arc<StreamRegistry>>,
    Json(req): Json<CreateStreamRequest>,
) -> (StatusCode, Json<StreamResponse>) {
    let id = StreamId::new_v4();
    registry.add(id, req.config);
    let info = registry.get(&id).unwrap();
    (StatusCode::CREATED, Json(to_response(info)))
}

async fn get_stream(
    State(registry): State<Arc<StreamRegistry>>,
    Path(id): Path<StreamId>,
) -> Result<Json<StreamResponse>, (StatusCode, Json<serde_json::Value>)> {
    match registry.get(&id) {
        Some(info) => Ok(Json(to_response(info))),
        None => Err(not_found(id)),
    }
}

async fn update_stream(
    State(registry): State<Arc<StreamRegistry>>,
    Path(id): Path<StreamId>,
    Json(req): Json<UpdateStreamRequest>,
) -> Result<Json<StreamResponse>, (StatusCode, Json<serde_json::Value>)> {
    if !registry.exists(&id) {
        return Err(not_found(id));
    }
    registry.update_config(&id, req.config);
    let info = registry.get(&id).unwrap();
    Ok(Json(to_response(info)))
}

async fn delete_stream(
    State(registry): State<Arc<StreamRegistry>>,
    Path(id): Path<StreamId>,
) -> StatusCode {
    if registry.remove(&id).is_some() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn test_connection(
    Path(_id): Path<StreamId>,
) -> Json<TestConnectionResponse> {
    // Phase 2 placeholder: actual URL validation requires FFmpeg probe
    // Will be implemented with avformat_open_input + avformat_close_input
    Json(TestConnectionResponse {
        reachable: true,
        latency_ms: 0,
        message: "Connection validation not yet implemented".to_string(),
    })
}

fn not_found(id: StreamId) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "Stream not found",
            "stream_id": id.to_string(),
        })),
    )
}

fn to_response(info: crate::stream::registry::StreamInfo) -> StreamResponse {
    StreamResponse {
        id: info.id,
        config: info.config,
        health: StreamStatusResponse {
            status: match &info.health.status {
                StreamStatus::Online => "online".to_string(),
                StreamStatus::Offline => "offline".to_string(),
                StreamStatus::Error(e) => format!("error: {}", e),
                StreamStatus::Connecting => "connecting".to_string(),
            },
            last_online: info.health.last_online.map(|t| t.to_rfc3339()),
            last_error: info.health.last_error.map(|t| t.to_rfc3339()),
            error_count: info.health.error_count,
            uptime_seconds: info.health.uptime_seconds,
            frames_decoded: info.health.frames_decoded,
            frames_extracted: info.health.frames_extracted,
            reconnect_count: info.health.reconnect_count,
        },
        created_at: info.created_at.to_rfc3339(),
    }
}
```

</action>
<verify>
cargo check 2>&1
</verify>
<done>
- REST API with full CRUD for streams
- Axum routes: GET/POST/DELETE /api/v1/streams, GET/PUT/DELETE /api/v1/streams/{id}
- Test connection endpoint placeholder
- Proper error handling with JSON error responses
- cargo check passes
</done>
</task>

<task type="auto">
<name>Task 4: StreamManager — multi-stream pipeline orchestration</name>
<files>src/stream/mod.rs (amend)</files>
<action>

Transform `src/stream/mod.rs` into a StreamManager that owns all running pipelines.

**Architecture:**

```
StreamManager
  ├── registry: StreamRegistry
  ├── pipelines: HashMap<StreamId, PipelineHandle>
  │     └── PipelineHandle { shutdown_token, join_handle }
  └── reconnection task (tokio::spawn)

On add_stream(id, config):
  1. registry.add(id, config)
  2. Create Pipeline (spawns OS thread)
  3. Store PipelineHandle
  4. Increment STREAMS_ACTIVE gauge

On remove_stream(id):
  1. Cancel shutdown_token
  2. Join pipeline thread (with timeout)
  3. registry.remove(id)
  4. Remove PipelineHandle
  5. Decrement STREAMS_ACTIVE gauge

Reconnection:
  - Tokio task periodically checks for dead pipelines
  - When a pipeline thread exits unexpectedly:
    1. Update health to Offline/Error
    2. Wait exponential backoff (1s→2s→4s→8s→16s→30s max)
    3. Re-spawn pipeline thread
    4. Update health to Connecting → Online
```

```rust
// src/stream/mod.rs
pub mod health;
pub mod registry;

use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use crate::config::StreamConfig;
use crate::pipeline;
use crate::stream::health::{StreamHealth, StreamStatus};
use crate::stream::registry::StreamRegistry;
use crate::types::StreamId;

struct PipelineHandle {
    shutdown_token: CancellationToken,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

pub struct StreamManager {
    registry: StreamRegistry,
    pipelines: Arc<std::sync::Mutex<HashMap<StreamId, PipelineHandle>>>,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            registry: StreamRegistry::new(),
            pipelines: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    pub fn registry(&self) -> &StreamRegistry {
        &self.registry
    }

    pub fn add_stream(&self, config: StreamConfig, storage_client: Arc<crate::storage::StorageClient>, kafka_producer: Arc<crate::kafka::KafkaProducer>) -> StreamId {
        let id = StreamId::new_v4();
        self.registry.add(id, config.clone());

        let shutdown_token = CancellationToken::new();
        let mut pipeline = pipeline::Pipeline::start(&config, id, shutdown_token.clone());

        // Spawn async consumer for this stream's extracted frames
        let extracted_rx = pipeline.extracted_rx.clone();
        let st = storage_client.clone();
        let kp = kafka_producer.clone();
        let bucket_name = config.storage().map(|s| s.bucket.clone()).unwrap_or_default();
        let sid = id;
        tokio::spawn(async move {
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
                                match st.upload_frame_simple(&frame).await {
                                    Ok((url, key)) => {
                                        if let Err(e) = kp.publish_metadata(
                                            &frame, &url, &bucket_name, &key
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
                                tracing::info!(stream_id = %sid, "Extracted frame channel closed");
                                break;
                            }
                        }
                    }
                }
            }
        });

        let handle = PipelineHandle {
            shutdown_token,
            join_handle: Some(pipeline.decode_handle.take().unwrap()),
        };

        self.pipelines.lock().unwrap().insert(id, handle);
        crate::metrics::STREAMS_ACTIVE.increment(1.0);
        crate::metrics::STREAMS_TOTAL.increment(1);

        id
    }

    pub fn remove_stream(&self, id: &StreamId) -> bool {
        let handle = self.pipelines.lock().unwrap().remove(id);
        if let Some(mut h) = handle {
            h.shutdown_token.cancel();
            if let Some(jh) = h.join_handle.take() {
                let _ = jh.join();
            }
            self.registry.remove(id);
            crate::metrics::STREAMS_ACTIVE.decrement(1.0);
            tracing::info!(stream_id = %id, "Stream removed");
            true
        } else {
            false
        }
    }

    pub fn shutdown_all(&self) {
        let mut pipelines = self.pipelines.lock().unwrap();
        for (id, mut handle) in pipelines.drain() {
            handle.shutdown_token.cancel();
            if let Some(jh) = handle.join_handle.take() {
                let _ = jh.join();
            }
            tracing::info!(stream_id = %id, "Stream shut down");
        }
        crate::metrics::STREAMS_ACTIVE.set(0.0);
    }
}
```

Note: The `StreamConfig` struct needs an optional `storage` and `kafka` field for per-stream overrides. For Phase 2, streams default to the global config.

For simplicity, modify `src/pipeline/mod.rs` to accept `StreamConfig` directly (not the full `Config`), and add a helper method to extract storage/kafka config.

**src/pipeline/mod.rs** — Update Pipeline::start to accept StreamConfig:

```rust
pub fn start(
    stream_config: &StreamConfig,
    stream_id: StreamId,
    shutdown_token: CancellationToken,
) -> Self {
    let (extract_tx, extract_rx) = bounded::<ExtractedFrame>(DECODE_TO_EXTRACT_CAPACITY);
    
    let source_url = stream_config.source_url.clone();
    let source_type = stream_config.source_type.clone();
    let interval = stream_config.extract_interval_seconds;
    let jpeg_quality = stream_config.jpeg_quality;
    let ffmpeg_threads = stream_config.ffmpeg_threads;
    let rtsp_transport = stream_config.rtsp_transport.clone();
    // ... rest same as before
}
```

</action>
<verify>
cargo check 2>&1
</verify>
<done>
- StreamManager with add/remove/shutdown_all
- Each stream gets its own Pipeline + async consumer
- Pipelines tracked with CancellationToken for graceful shutdown
- Metrics updated on add/remove
- cargo check passes
</done>
</task>

<task type="auto">
<name>Task 5: Wire everything — main.rs with StreamManager + API + metrics</name>
<files>src/main.rs, src/api/mod.rs (amend), src/metrics.rs (amend), Cargo.toml (verify)</files>
<action>

**src/main.rs** — Rewrite to use StreamManager:

```rust
mod config;
mod types;
mod logging;
mod pipeline;
mod storage;
mod kafka;
mod health;
mod stream;
mod api;
mod metrics;

use clap::Parser;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "getframe-worker", version = "0.1.0")]
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
    let kafka_producer = Arc::new(kafka::KafkaProducer::new(&config.kafka)?);

    // StreamManager — manages all stream pipelines
    let stream_manager = stream::StreamManager::new();

    // Pre-load streams from config file
    for stream_cfg in &config.preload_streams {
        let id = stream_manager.add_stream(
            stream_cfg.clone(),
            storage_client.clone(),
            kafka_producer.clone(),
        );
        tracing::info!(stream_id = %id, url = %stream_cfg.source_url, "Pre-loaded stream");
    }

    // Health state
    let health_state = health::HealthState::new();

    // Compose all routes
    let app = health::health_router(health_state.clone())
        .merge(api::api_router(Arc::new(stream_manager.registry().clone())))
        .route("/metrics", axum::routing::get(metrics::metrics_handler));

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
            let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
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

    stream_manager.shutdown_all();
    tracing::info!("getframe-worker shut down cleanly");

    Ok(())
}
```

**src/metrics.rs** — Add metrics handler:

```rust
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use once_cell::sync::Lazy;

static PROMETHEUS_HANDLE: Lazy<PrometheusHandle> = Lazy::new(|| {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder")
});

pub async fn metrics_handler() -> String {
    PROMETHEUS_HANDLE.render()
}
```

</action>
<verify>
cargo check 2>&1; cargo build 2>&1
</verify>
<done>
- main.rs uses StreamManager for multi-stream orchestration
- Pre-loads streams from config.preload_streams
- API routes composed: health + stream CRUD + metrics
- Graceful shutdown drains all stream pipelines
- cargo build succeeds
</done>
</task>

</tasks>

<dependency_graph>
graph TD
    T1["Task 1: Dependencies + metrics + stream types"]
    T2["Task 2: StreamRegistry"]
    T3["Task 3: REST API"]
    T4["Task 4: StreamManager + Pipeline update"]
    T5["Task 5: Wire main.rs + metrics handler"]

    T1 --> T2
    T1 --> T3
    T1 --> T4
    T2 --> T3
    T2 --> T4
    T3 --> T5
    T4 --> T5
</dependency_graph>

<success_criteria>
Phase 2 is complete when ALL of the following are true:

1. ✅ **Multi-stream runtime** — N streams run in parallel, each in its own OS thread with independent channels
2. ✅ **Stream CRUD API** — REST API at /api/v1/streams supports create/read/update/delete
3. ✅ **Per-stream status** — GET /api/v1/streams returns health status (online/offline/error/connecting) per stream
4. ✅ **Metrics endpoint** — GET /metrics returns Prometheus-formatted metrics (streams_active, frames_processed_total, errors)
5. ✅ **Graceful removal** — Deleting a stream via API stops the pipeline, cleans up resources, joins the thread
6. ✅ **Auto-reconnection** — Failed streams reconnect with exponential backoff (1s→2s→4s→8s→16s→30s max)
7. ✅ **Pre-loaded streams** — Streams defined in YAML config under `preload_streams` are auto-started at boot
8. ✅ **Graceful shutdown** — SIGTERM drains all stream pipelines before exit
</success_criteria>

<verification>
1. `cargo check` — passes with no warnings
2. `cargo build` — debug build succeeds
3. API test: `curl -X POST http://localhost:8080/api/v1/streams -H 'Content-Type: application/json' -d '{"config":{...}}'` returns 201
4. Metrics test: `curl http://localhost:8080/metrics` returns Prometheus text
5. Health test: `curl http://localhost:8080/health` returns 200
</verification>
