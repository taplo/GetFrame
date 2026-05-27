---
phase: 03-task-engine-scheduler
plan: 01
type: execute
wave: 1
depends_on:
  - 02-multi-stream-management
files_modified:
  - src/pipeline/rule.rs
  - src/pipeline/decode.rs
  - src/pipeline/mod.rs
  - src/stream/mod.rs
  - src/stream/registry.rs
  - src/stream/health.rs
  - src/api/mod.rs
  - src/types.rs
  - src/main.rs
  - config.example.yaml
files_created:
  - src/api/rules.rs
autonomous: true
requirements:
  - RULE-02
  - RULE-03
  - RULE-06
  - API-03
  - STREAM-07
  - STREAM-08
---

<objective>
**Phase 3 Goal:** Transform the hardcoded single-rule pipeline into a dynamically configurable per-stream rule system with auto-reconnection. Users can configure interval, FPS, and rate-limited extraction rules per stream via REST API without restarting pipelines. Failed streams automatically reconnect with exponential backoff, and health status is actively maintained.

**Purpose:** Phase 3 delivers two critical production capabilities: (1) rules that can be changed at runtime without service restart — essential for operational flexibility; (2) automatic reconnection with backoff — without this, a single network glitch kills a pipeline permanently, requiring manual intervention. Together, these enable the system to run unattended for extended periods.

**Output:** Per-stream rule CRUD API, FPS-based and rate-limited extraction rules, runtime rule hot-reload into active pipelines, automatic reconnection scheduler with exponential backoff, and live pipeline health feedback.
</objective>

<context>

## Architecture Overview

```
┌───────────────┐     ┌──────────────────────────────────────────────────────┐
│   HTTP API    │     │                  StreamManager                        │
│  (Axum 0.8)   │     │  ┌─────────┐  ┌─────────┐  ┌─────────┐             │
│               │     │  │Stream 1 │  │Stream 2 │  │Stream N │             │
│  /api/v1/*    │────▶│  │Pipeline │  │Pipeline │  │Pipeline │             │
│  /metrics     │     │  └────┬────┘  └────┬────┘  └────┬────┘             │
│  /health      │     │       │            │            │                   │
│  /ready       │     │  ┌────▼────────────▼────────────▼────┐              │
│               │     │  │        StreamRegistry              │              │
│               │     │  │  Arc<RwLock<HashMap<Id, State>>>   │              │
│               │     │  │  + rules: Vec<RuleConfig>          │              │
│               │     │  └───────────────────────────────────┘              │
│               │     │  ┌───────────────────────────────────┐              │
│               │     │  │     Reconnection Scheduler         │              │
│               │     │  │  per-stream tokio tasks            │              │
│               │     │  │  exp backoff: 1→2→4→8→16→30s      │              │
│               │     │  └───────────────────────────────────┘              │
│               │     │  ┌───────────────────────────────────┐              │
│               │     │  │  Shared Rule State (per stream)    │              │
│               │     │  │  Arc<RwLock<Vec<RuleConfig>>>      │──────┐      │
│               │     │  └───────────────────────────────────┘      │      │
│               │     └──────────────────────────────────────────────│──────┘
│               │                                                    │
│               │     decode thread reads:                           │
│               │     rules_lock.read().unwrap().iter()...           │
└───────────────┘
```

### Key Components

- **RuleConfig enum**: Serializable rule types tagged by `type` field
- **RuleEvaluator trait**: `should_extract(frame) -> bool` implemented per rule type
- **RuleEngine**: Owns a list of `(RuleConfig, Box<dyn RuleEvaluator>)`, evaluates all rules
- **Shared rules**: `Arc<RwLock<Vec<RuleConfig>>>` stored in StreamInfo, passed to pipeline
- **Reconnection task**: Per-stream tokio task that monitors pipeline health watch channel
- **Health feedback**: `Arc<Mutex<StreamHealth>>` passed to pipeline decode loop

### Data Flow for Rule Hot-Reload

```
PUT /api/v1/streams/{id}/rules/{index}
  → API handler acquires registry lock
  → Updates rules Vec in StreamInfo
  → Writes new rules to shared Arc<RwLock<Vec<RuleConfig>>>
  → Releases lock
  → Next frame eval in decode loop:
    → Acquires rules_lock.read()
    → Rebuilds engine from latest configs
    → Evaluates frames with new rules
```

### Reconnection Flow

```
Pipeline thread exits (error / EOF)
  → watch channel sends ExitSignal { reason }
  → Reconnection task receives signal
  → Updates health: status=Error, last_error=now, error_count++
  → If max_reconnects_exceeded → mark dead, stop retrying
  → Wait backoff_delay seconds
  → Update health: status=Connecting
  → Re-create Pipeline + async consumer
  → If success: health=Online, backoff=1s (reset)
  → If failure: health=Error, backoff=min(backoff*2, 30s), retry
```

</context>

<tasks>

<task type="auto">
<name>Task 1: Rule engine refactor — RuleConfig enum, RuleEvaluator trait, RateLimiter</name>
<files>src/pipeline/rule.rs, src/types.rs</files>
<action>

**src/types.rs** — Add RuleConfig type alias for cross-module access:

```rust
// (nothing new needed if RuleConfig lives in pipeline::rule;
//  but we need it accessible from config, api, and stream modules.
//  Re-export from lib.rs or types.rs.)

// Add to types.rs:
pub use crate::pipeline::rule::RuleConfig;
```

**src/pipeline/rule.rs** — Complete rewrite:

```rust
use crate::types::DecodedFrame;
use serde::{Deserialize, Serialize};

/// Serializable extraction rule configuration.
/// Tagged by `type` field for JSON/YAML deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuleConfig {
    /// Extract every N seconds (fixed interval)
    #[serde(rename = "interval")]
    Interval {
        /// Seconds between extractions (e.g., 5.0 = one frame every 5 seconds)
        interval_seconds: f64,
    },
    /// Extract at N frames per second
    #[serde(rename = "fps")]
    Fps {
        /// Target frames per second (e.g., 0.5 = 1 frame every 2 seconds)
        fps: f64,
    },
    /// Rate-limited wrapper around another rule
    #[serde(rename = "rate_limited")]
    RateLimited {
        /// The inner rule to evaluate
        rule: Box<RuleConfig>,
        /// Maximum frames per minute
        max_per_minute: u64,
    },
}

impl RuleConfig {
    pub fn description(&self) -> String {
        match self {
            RuleConfig::Interval { interval_seconds } => {
                format!("interval/{:.1}s", interval_seconds)
            }
            RuleConfig::Fps { fps } => {
                format!("fps/{:.2}", fps)
            }
            RuleConfig::RateLimited { max_per_minute, .. } => {
                format!("rate-limited/{}mpm", max_per_minute)
            }
        }
    }
}

/// Runtime rule evaluator trait.
pub trait RuleEvaluator: Send {
    /// Returns true if this frame should be extracted.
    fn should_extract(&mut self, frame: &DecodedFrame) -> bool;

    /// Human-readable description for logging.
    fn description(&self) -> String;
}

/// Factory: create a RuleEvaluator from a RuleConfig
pub fn create_evaluator(config: &RuleConfig, time_base: (i32, i32)) -> Box<dyn RuleEvaluator> {
    match config {
        RuleConfig::Interval { interval_seconds } => {
            Box::new(IntervalEvaluator::new(*interval_seconds, time_base))
        }
        RuleConfig::Fps { fps } => {
            // FPS = 1/interval
            let interval_seconds = 1.0 / fps.max(0.001); // avoid div by zero
            Box::new(IntervalEvaluator::new(interval_seconds, time_base))
        }
        RuleConfig::RateLimited { rule, max_per_minute } => {
            let inner = create_evaluator(rule, time_base);
            Box::new(RateLimitedEvaluator::new(inner, *max_per_minute))
        }
    }
}

/// Fixed-interval rule evaluator (also handles FPS by converting to interval).
pub struct IntervalEvaluator {
    interval_seconds: f64,
    interval_pts: i64,
    last_extracted_pts: Option<i64>,
    frames_evaluated: u64,
    frames_extracted: u64,
}

impl IntervalEvaluator {
    pub fn new(interval_seconds: f64, time_base: (i32, i32)) -> Self {
        let tb = time_base.0 as f64 / time_base.1 as f64;
        let interval_pts = if tb > 0.0 {
            (interval_seconds / tb) as i64
        } else {
            0
        };
        Self {
            interval_seconds,
            interval_pts,
            last_extracted_pts: None,
            frames_evaluated: 0,
            frames_extracted: 0,
        }
    }
}

impl RuleEvaluator for IntervalEvaluator {
    fn should_extract(&mut self, frame: &DecodedFrame) -> bool {
        self.frames_evaluated += 1;
        let should = match self.last_extracted_pts {
            None => true,
            Some(last_pts) => {
                frame.pts.saturating_sub(last_pts) >= self.interval_pts
            }
        };
        if should {
            self.last_extracted_pts = Some(frame.pts);
            self.frames_extracted += 1;
        }
        should
    }

    fn description(&self) -> String {
        format!("interval/{:.1}s", self.interval_seconds)
    }
}

/// Token-bucket rate limiter wrapping an inner evaluator.
pub struct RateLimitedEvaluator {
    inner: Box<dyn RuleEvaluator>,
    max_per_minute: u64,
    tokens: f64,
    last_refill: std::time::Instant,
}

impl RateLimitedEvaluator {
    pub fn new(inner: Box<dyn RuleEvaluator>, max_per_minute: u64) -> Self {
        Self {
            inner,
            max_per_minute: max_per_minute.max(1),
            tokens: max_per_minute as f64,
            last_refill: std::time::Instant::now(),
        }
    }

    fn refill(&mut self) {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        let rate = self.max_per_minute as f64 / 60.0;
        self.tokens = (self.tokens + elapsed * rate).min(self.max_per_minute as f64);
        self.last_refill = std::time::Instant::now();
    }

    fn consume(&mut self) -> bool {
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

impl RuleEvaluator for RateLimitedEvaluator {
    fn should_extract(&mut self, frame: &DecodedFrame) -> bool {
        self.refill();
        if self.inner.should_extract(frame) {
            self.consume()
        } else {
            false
        }
    }

    fn description(&self) -> String {
        format!("rate-limited({}, max={}/min)", self.inner.description(), self.max_per_minute)
    }
}

/// Composite rule engine: evaluates all rules, extracts if ANY matches.
pub struct RuleEngine {
    evaluators: Vec<(RuleConfig, Box<dyn RuleEvaluator>)>,
}

impl RuleEngine {
    pub fn new(configs: &[RuleConfig], time_base: (i32, i32)) -> Self {
        let evaluators = configs.iter()
            .map(|c| (c.clone(), create_evaluator(c, time_base)))
            .collect();
        Self { evaluators }
    }

    /// Evaluate all rules. Returns true if ANY rule triggers extraction.
    pub fn evaluate(&mut self, frame: &DecodedFrame) -> bool {
        self.evaluators.iter_mut().any(|(_, eval)| eval.should_extract(frame))
    }

    /// Rebuild evaluators from new configs (for hot-reload).
    pub fn rebuild(&mut self, configs: &[RuleConfig], time_base: (i32, i32)) {
        self.evaluators = configs.iter()
            .map(|c| (c.clone(), create_evaluator(c, time_base)))
            .collect();
    }
}
```

</action>
<verify>
cargo check 2>&1
</verify>
<done>
- RuleConfig enum with Interval, FPS, RateLimited variants
- RuleEvaluator trait with should_extract and description
- create_evaluator factory function
- IntervalEvaluator (same logic as original, now behind trait)
- RateLimitedEvaluator with token bucket
- RuleEngine for composite evaluation (any-match)
- RuleEngine::rebuild for hot-reload
- cargo check passes
</done>
</task>

<task type="auto">
<name>Task 2: Shared rule state in registry + pipeline health feedback</name>
<files>src/stream/registry.rs, src/stream/health.rs, src/types.rs</files>
<action>

**src/pipeline/rule.rs** — Add re-export path (already done in Task 1).

**src/types.rs** — Add re-exports:

```rust
pub use crate::pipeline::rule::RuleConfig;
```

**src/stream/health.rs** — Add Clone constraint and helper:

```rust
// StreamHealth already has Clone, ensure it's Send + Sync for Arc<Mutex<>>
// Add:
impl Default for StreamHealth {
    fn default() -> Self {
        Self::new()
    }
}
```

**src/stream/registry.rs** — Add shared rule state to StreamInfo:

```rust
use std::sync::{Arc, RwLock};
use crate::pipeline::rule::RuleConfig;

// In StreamInfo, add:
pub rules: Arc<RwLock<Vec<RuleConfig>>>,

// Update StreamRegistry::add to create default rules:
pub fn add(&self, id: StreamId, config: StreamConfig) {
    let mut inner = self.inner.write().unwrap();
    let default_rule = RuleConfig::Interval {
        interval_seconds: config.extract_interval_seconds,
    };
    let info = StreamInfo {
        id,
        config,
        health: StreamHealth::new(),
        rules: Arc::new(RwLock::new(vec![default_rule])),
        created_at: chrono::Utc::now(),
    };
    inner.streams.insert(id, info);
}

// Add methods:
pub fn get_rules(&self, id: &StreamId) -> Option<Vec<RuleConfig>> {
    let inner = self.inner.read().unwrap();
    inner.streams.get(id).map(|info| {
        info.rules.read().unwrap().clone()
    })
}

pub fn update_rules(&self, id: &StreamId, rules: Vec<RuleConfig>) -> bool {
    let inner = self.inner.read().unwrap();
    if let Some(info) = inner.streams.get(id) {
        let mut dest = info.rules.write().unwrap();
        *dest = rules;
        true
    } else {
        false
    }
}

pub fn get_rules_shared(&self, id: &StreamId) -> Option<Arc<RwLock<Vec<RuleConfig>>>> {
    let inner = self.inner.read().unwrap();
    inner.streams.get(id).map(|info| info.rules.clone())
}
```

**src/stream/health.rs** — Add method to update from decode stats:

```rust
impl StreamHealth {
    /// Update counters from the decode pipeline.
    pub fn record_decode_frame(&mut self) {
        self.frames_decoded += 1;
    }

    pub fn record_extracted_frame(&mut self) {
        self.frames_extracted += 1;
    }

    pub fn record_pts(&mut self, pts: i64) {
        self.last_pts = Some(pts);
    }

    pub fn mark_online(&mut self) {
        self.status = StreamStatus::Online;
        self.last_online = Some(chrono::Utc::now());
    }

    pub fn mark_error(&mut self, error: &str) {
        self.status = StreamStatus::Error(error.to_string());
        self.last_error = Some(chrono::Utc::now());
        self.error_count += 1;
    }

    pub fn mark_connecting(&mut self) {
        self.status = StreamStatus::Connecting;
    }

    pub fn mark_reconnected(&mut self) {
        self.reconnect_count += 1;
    }
}
```

</action>
<verify>
cargo check 2>&1
</verify>
<done>
- StreamInfo.rules: Arc<RwLock<Vec<RuleConfig>>>
- StreamRegistry methods: get_rules, update_rules, get_rules_shared
- StreamHealth helper methods for status updates
- Default rules created from stream config on add
- cargo check passes
</done>
</task>

<task type="auto">
<name>Task 3: Pipeline integration — shared rules + health feedback in decode loop</name>
<files>src/pipeline/mod.rs, src/pipeline/decode.rs, src/pipeline/rule.rs</files>
<action>

**src/pipeline/mod.rs** — Update Pipeline::start signature to accept shared rules and health:

```rust
pub fn start(
    stream_config: &StreamConfig,
    stream_id: StreamId,
    shutdown_token: CancellationToken,
    health_handle: Arc<std::sync::Mutex<crate::stream::health::StreamHealth>>,
    rules_shared: Arc<RwLock<Vec<RuleConfig>>>,
) -> Self {
    let (extract_tx, extract_rx) = bounded::<ExtractedFrame>(DECODE_TO_EXTRACT_CAPACITY);
    // ... (rest same as before, but pass health_handle + rules_shared to decode)
}
```

Update spawn block to pass new args to decode:

```rust
let health_handle_clone = health_handle.clone();
let rules_clone = rules_shared.clone();
let handle = thread::Builder::new()
    .name(format!("stream-{}", stream_id))
    .spawn(move || {
        let result = decode::run_decode_pipeline(
            &source_url, &source_type, &rtsp_transport,
            ffmpeg_threads, stream_id_clone,
            interval, jpeg_quality,
            extract_tx, shutdown,
            health_handle_clone,
            rules_clone,
        );
        // ...
    })
    .expect("Failed to spawn pipeline thread");
```

**src/pipeline/decode.rs** — Integrate health feedback + shared rules:

```rust
use std::sync::{Arc, Mutex, RwLock};
use crate::pipeline::rule::{RuleEngine, RuleConfig};
use crate::stream::health::StreamHealth;

pub fn run_decode_pipeline(
    source_url: &str,
    source_type: &str,
    rtsp_transport: &str,
    ffmpeg_threads: i32,
    stream_id: StreamId,
    interval_seconds: f64,
    jpeg_quality: u8,
    frame_tx: Sender<ExtractedFrame>,
    shutdown: tokio_util::sync::CancellationToken,
    health_handle: Arc<Mutex<StreamHealth>>,
    rules_shared: Arc<RwLock<Vec<RuleConfig>>>,
) -> Result<(), anyhow::Error> {
    tracing::info!(stream_id = %stream_id, "Starting decode pipeline");

    let mut demuxed = ingest::open_video_source(
        source_url, source_type, rtsp_transport, ffmpeg_threads
    )?;
    let time_base = demuxed.time_base;
    let tb_f = time_base.0 as f64 / time_base.1 as f64;

    // Mark online
    {
        let mut h = health_handle.lock().unwrap();
        h.mark_online();
    }

    let mut packet = Packet::empty();
    let mut frame_number: u64 = 0;
    let mut pts_queue: BTreeMap<i64, DecodedFrame> = BTreeMap::new();
    let mut reorder_depth: usize = 0;
    let mut first_keyframe_seen = false;
    let mut total_frames_decoded: u64 = 0;

    // Build rule engine from shared rules
    let initial_rules = rules_shared.read().unwrap().clone();
    let mut engine = RuleEngine::new(&initial_rules, (time_base.0, time_base.1));

    // Periodic health update counter
    let mut health_counter: u64 = 0;

    for (stream_idx, mut recv_packet) in demuxed.ictx.packets() {
        if shutdown.is_cancelled() {
            break;
        }

        if stream_idx.index() != demuxed.video_stream_index {
            continue;
        }

        // Send packet to decoder
        demuxed.decoder.send_packet(&recv_packet)?;

        // Receive all available frames
        loop {
            // Decode frame using ffmpeg-next API
            // ...

            // After receiving a frame:
            total_frames_decoded += 1;

            // PTS reordering logic (same as Phase 2)
            pts_queue.insert(pts, decoded_frame);

            // Emit reordered frames
            while let Some((_min_pts, frame)) = pop_reordered(&mut pts_queue, &reorder_depth) {
                if !first_keyframe_seen {
                    if frame.is_keyframe {
                        first_keyframe_seen = true;
                    } else {
                        continue; // Still waiting for keyframe
                    }
                }

                // Re-read rules periodically for hot-reload (check every frame is fine,
                // the RwLock is uncontended)
                // Optimize: only rebuild if Vec pointer changed? For now, simple:
                {
                    let current_rules = rules_shared.read().unwrap();
                    // Quick check: if engine doesn't match, rebuild
                    // Simple approach: rebuild every N frames with check
                }

                if engine.evaluate(&frame) {
                    // Encode JPEG
                    match crate::pipeline::encode::encode_jpeg(&frame, jpeg_quality) {
                        Ok(jpeg_bytes) => {
                            let timestamp_seconds = frame.pts as f64 * tb_f;
                            let extracted = ExtractedFrame {
                                stream_id,
                                frame_number,
                                pts: frame.pts,
                                timestamp_seconds,
                                jpeg_bytes,
                                rule_trigger: "rule".to_string(),
                                jpeg_quality,
                                width: frame.width,
                                height: frame.height,
                            };

                            if frame_tx.send(extracted).is_err() {
                                break;
                            }
                            frame_number += 1;
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "JPEG encoding failed");
                        }
                    }
                }
            }

            // Periodic health update (every 30 frames)
            health_counter += 1;
            if health_counter % 30 == 0 {
                let mut h = health_handle.lock().unwrap();
                h.frames_decoded = total_frames_decoded;
                h.frames_extracted = frame_number;
                // h.last_pts remains set from decode
            }
        }
    }

    // Flush decoder
    demuxed.decoder.send_eof()?;
    // Drain remaining frames...

    Ok(())
}
```

Note: The actual FFmpeg decode loop using raw `send_packet`/`receive_frame` is preserved from Phase 2. Only add the health_handle updates and rules_shared integration around the existing decode/extract logic.

**Critical implementation detail for PTS reordering:** The `pop_reordered` function should remain the same as Phase 2. The key change is that instead of calling `rule.should_extract()`, we call `engine.evaluate()`. The `RuleEngine` internally handles all rule types.

**Hot-reload rules:** The decode loop checks rules_shared on each frame evaluation. Since the lock is a read lock with no contention (only written by API calls), this has negligible overhead. For bulk operations, add a generation counter:

```rust
// To avoid rebuilding engine every frame on no-change,
// add a generation counter to the shared state:
pub struct SharedRules {
    pub configs: Vec<RuleConfig>,
    pub generation: u64,
}

// In registry (pseudo):
// rules_shared: Arc<RwLock<SharedRules>>
// Pipeline stores last_generation, only rebuilds when generation changes
```

Keep it simple for Phase 3: rebuild the `RuleEngine` from current configs on every frame. If profiling shows a concern, optimize in Phase 9.

</action>
<verify>
cargo check 2>&1
</verify>
<done>
- Pipeline::start accepts health_handle + rules_shared
- Decode loop updates health counters every 30 frames
- RuleEngine created from shared rules at start
- Hot-reload: rules checked on each frame evaluation
- RuleEngine rebuilt from current configs
- cargo check passes
</done>
</task>

<task type="auto">
<name>Task 4: Reconnection scheduler in StreamManager</name>
<files>src/stream/mod.rs</files>
<action>

**src/stream/mod.rs** — Major update: add reconnection scheduler, health feedback, watch channel.

Add a `PipelineExit` signal type and restructure `PipelineHandle`:

```rust
pub mod health;
pub mod registry;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use crate::config::StreamConfig;
use crate::pipeline;
use crate::pipeline::rule::RuleConfig;
use crate::stream::health::{StreamHealth, StreamStatus};
use crate::stream::registry::StreamRegistry;
use crate::types::StreamId;

/// Signal sent when a pipeline thread exits.
#[derive(Debug, Clone)]
pub enum PipelineExitReason {
    UserInitiated,
    Error(String),
    Eof,
}

struct PipelineHandle {
    shutdown_token: CancellationToken,
    join_handle: Option<std::thread::JoinHandle<()>>,
    health_handle: Arc<Mutex<StreamHealth>>,
    rules_shared: Arc<RwLock<Vec<RuleConfig>>>,
    exit_tx: tokio::sync::watch::Sender<Option<PipelineExitReason>>,
}

pub struct StreamManager {
    registry: StreamRegistry,
    pipelines: Arc<Mutex<HashMap<StreamId, PipelineHandle>>>,
    storage_client: Arc<crate::storage::StorageClient>,
    kafka_producer: Arc<crate::kafka::KafkaProducer>,
    max_backoff_seconds: u64,
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
        }
    }

    pub fn registry(&self) -> &StreamRegistry {
        &self.registry
    }

    pub fn add_stream(&self, config: StreamConfig) -> StreamId {
        let id = StreamId::new_v4();
        self.registry.add(id, config.clone());

        let shutdown_token = CancellationToken::new();
        let health_handle = Arc::new(Mutex::new(StreamHealth::new()));
        let rules_shared = self.registry.get_rules_shared(&id)
            .expect("Stream must exist after add");

        // Exit signal channel
        let (exit_tx, exit_rx) = tokio::sync::watch::channel(None::<PipelineExitReason>);

        // Start pipeline
        let mut pipeline = pipeline::Pipeline::start(
            &config, id, shutdown_token.clone(),
            health_handle.clone(), rules_shared.clone(),
        );

        // Spawn async consumer for this stream's extracted frames
        self.spawn_frame_consumer(id, pipeline.extracted_rx.clone(),
            shutdown_token.clone(), health_handle.clone());

        let handle = PipelineHandle {
            shutdown_token,
            join_handle: Some(pipeline.decode_handle.take().unwrap()),
            health_handle: health_handle.clone(),
            rules_shared: rules_shared.clone(),
            exit_tx,
        };
        self.pipelines.lock().unwrap().insert(id, handle);

        // Spawn reconnection scheduler
        self.spawn_reconnection_task(id, config, exit_rx, health_handle, rules_shared);

        crate::metrics::STREAMS_ACTIVE.increment(1.0);
        crate::metrics::STREAMS_TOTAL.increment(1);

        id
    }

    fn spawn_frame_consumer(
        &self,
        stream_id: StreamId,
        extracted_rx: crossbeam::channel::Receiver<ExtractedFrame>,
        shutdown_token: CancellationToken,
        health_handle: Arc<Mutex<StreamHealth>>,
    ) {
        let st = self.storage_client.clone();
        let kp = self.kafka_producer.clone();
        let sid = stream_id;

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
                                match st.upload_frame(&frame).await {
                                    Ok((url, key)) => {
                                        let bucket = &st.bucket;
                                        if let Err(e) = kp.publish_metadata(
                                            &frame, &url, bucket, &key
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
                // Wait for pipeline exit signal
                exit_rx.changed().await.ok();

                let reason = exit_rx.borrow().clone();
                match &reason {
                    Some(PipelineExitReason::UserInitiated) => {
                        tracing::info!(stream_id = %stream_id, "Pipeline removed by user");
                        break;
                    }
                    Some(PipelineExitReason::Eof) => {
                        tracing::info!(stream_id = %stream_id, "Pipeline reached end of stream");
                        // EOF: maybe try reconnect? For RTSP, EOF means connection closed.
                        // Treat as error and reconnect.
                    }
                    Some(PipelineExitReason::Error(e)) => {
                        tracing::warn!(stream_id = %stream_id, error = %e, "Pipeline terminated with error");
                    }
                    None => {
                        // Initial state, no signal yet
                        continue;
                    }
                }

                // Update health: error
                {
                    let mut h = health_handle.lock().unwrap();
                    match &reason {
                        Some(PipelineExitReason::Error(e)) => {
                            h.mark_error(e);
                        }
                        _ => {
                            h.mark_error("stream disconnected");
                        }
                    }
                }

                // If stream was removed during backoff, stop
                // Exponential backoff
                tracing::info!(
                    stream_id = %stream_id,
                    backoff_seconds = backoff_seconds,
                    "Reconnecting in {} seconds...", backoff_seconds
                );
                tokio::time::sleep(std::time::Duration::from_secs(backoff_seconds)).await;

                // Double-check stream still exists
                if !registry.exists(&stream_id) {
                    tracing::info!(stream_id = %stream_id, "Stream removed during backoff");
                    break;
                }

                // Mark connecting
                {
                    let mut h = health_handle.lock().unwrap();
                    h.mark_connecting();
                }

                // Create new shutdown token
                let new_shutdown = CancellationToken::new();
                let (new_exit_tx, new_exit_rx) = tokio::sync::watch::channel(None::<PipelineExitReason>);

                // Start new pipeline
                let config = registry.get(&stream_id)
                    .map(|info| info.config.clone())
                    .unwrap_or_else(|| config.clone());

                let mut pipeline = pipeline::Pipeline::start(
                    &config, stream_id, new_shutdown.clone(),
                    health_handle.clone(), rules_shared.clone(),
                );

                // Spawn new consumer
                let st = storage_client.clone();
                let kp = kafka_producer.clone();
                let sid = stream_id;
                let sh = new_shutdown.clone();
                let hh = health_handle.clone();
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
                                                let bucket = &st.bucket;
                                                if let Err(e) = kp.publish_metadata(
                                                    &frame, &url, bucket, &key
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

                // Replace pipeline handle
                {
                    let new_handle = PipelineHandle {
                        shutdown_token: new_shutdown,
                        join_handle: Some(pipeline.decode_handle.take().unwrap()),
                        health_handle: health_handle.clone(),
                        rules_shared: rules_shared.clone(),
                        exit_tx: new_exit_tx,
                    };
                    pipelines.lock().unwrap().insert(stream_id, new_handle);
                }

                // Update health: online + reconnected
                {
                    let mut h = health_handle.lock().unwrap();
                    h.mark_online();
                    h.mark_reconnected();
                }
                backoff_seconds = (backoff_seconds * 2).min(max_backoff);
                crate::metrics::STREAMS_ACTIVE.increment(1.0);
                tracing::info!(stream_id = %stream_id, "Reconnected successfully");

                // Switch to new exit channel
                exit_rx = new_exit_rx;
                backoff_seconds = 1; // Reset on success
            }
        });
    }

    pub fn remove_stream(&self, id: &StreamId) -> bool {
        let handle = self.pipelines.lock().unwrap().remove(id);
        if let Some(mut h) = handle {
            // Signal reconnection task to stop
            h.exit_tx.send(Some(PipelineExitReason::UserInitiated)).ok();
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
```

</action>
<verify>
cargo check 2>&1
</verify>
<done>
- Per-stream reconnection task with watch channel
- Exponential backoff: 1s → 2s → 4s → ... → 30s max, reset on success
- Health lifecycle: Error → (backoff) → Connecting → (reconnect) → Online
- PipelineExit signal: UserInitiated, Error, Eof
- Stream removal signals reconnection task to stop
- consumer_frames extracted to reusable method
- cargo check passes
</done>
</task>

<task type="auto">
<name>Task 5: Rule CRUD API endpoints</name>
<files>src/api/mod.rs, src/api/rules.rs</files>
<action>

**src/api/rules.rs** — Rule CRUD endpoints:

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::pipeline::rule::RuleConfig;
use crate::stream::registry::StreamRegistry;
use crate::types::StreamId;

#[derive(Serialize)]
pub struct RulesResponse {
    pub stream_id: StreamId,
    pub rules: Vec<RuleConfig>,
}

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub rule: RuleConfig,
}

#[derive(Deserialize)]
pub struct UpdateRuleRequest {
    pub rule: RuleConfig,
}

#[derive(Serialize)]
pub struct RuleOperationResponse {
    pub stream_id: StreamId,
    pub rule: RuleConfig,
    pub index: usize,
}

pub fn rules_routes(registry: Arc<StreamRegistry>) -> axum::Router {
    axum::Router::new()
        .route("/", axum::routing::get(list_rules).post(create_rule))
        .route("/{index}", axum::routing::get(get_rule).put(update_rule).delete(delete_rule))
        .with_state(registry)
}

async fn list_rules(
    State(registry): State<Arc<StreamRegistry>>,
    Path(stream_id): Path<StreamId>,
) -> Result<Json<RulesResponse>, (StatusCode, Json<serde_json::Value>)> {
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let rules = registry.get_rules(&stream_id).unwrap_or_default();
    Ok(Json(RulesResponse {
        stream_id,
        rules,
    }))
}

async fn create_rule(
    State(registry): State<Arc<StreamRegistry>>,
    Path(stream_id): Path<StreamId>,
    Json(req): Json<CreateRuleRequest>,
) -> Result<(StatusCode, Json<RuleOperationResponse>), (StatusCode, Json<serde_json::Value>)> {
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let mut rules = registry.get_rules(&stream_id).unwrap_or_default();
    rules.push(req.rule.clone());
    registry.update_rules(&stream_id, rules);
    let rules = registry.get_rules(&stream_id).unwrap_or_default();
    let index = rules.len() - 1;
    Ok((
        StatusCode::CREATED,
        Json(RuleOperationResponse {
            stream_id,
            rule: req.rule,
            index,
        }),
    ))
}

async fn get_rule(
    State(registry): State<Arc<StreamRegistry>>,
    Path((stream_id, index)): Path<(StreamId, usize)>,
) -> Result<Json<RuleOperationResponse>, (StatusCode, Json<serde_json::Value>)> {
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let rules = registry.get_rules(&stream_id).unwrap_or_default();
    rules.get(index).map(|rule| Json(RuleOperationResponse {
        stream_id,
        rule: rule.clone(),
        index,
    })).ok_or_else(|| index_error(stream_id, index))
}

async fn update_rule(
    State(registry): State<Arc<StreamRegistry>>,
    Path((stream_id, index)): Path<(StreamId, usize)>,
    Json(req): Json<UpdateRuleRequest>,
) -> Result<Json<RuleOperationResponse>, (StatusCode, Json<serde_json::Value>)> {
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let mut rules = registry.get_rules(&stream_id).unwrap_or_default();
    if index >= rules.len() {
        return Err(index_error(stream_id, index));
    }
    rules[index] = req.rule.clone();
    registry.update_rules(&stream_id, rules);
    Ok(Json(RuleOperationResponse {
        stream_id,
        rule: req.rule,
        index,
    }))
}

async fn delete_rule(
    State(registry): State<Arc<StreamRegistry>>,
    Path((stream_id, index)): Path<(StreamId, usize)>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let mut rules = registry.get_rules(&stream_id).unwrap_or_default();
    if index >= rules.len() {
        return Err(index_error(stream_id, index));
    }
    rules.remove(index);
    registry.update_rules(&stream_id, rules);
    Ok(StatusCode::NO_CONTENT)
}

fn not_found(stream_id: StreamId) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "Stream not found",
            "stream_id": stream_id.to_string(),
        })),
    )
}

fn index_error(stream_id: StreamId, index: usize) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": format!("Rule index {} not found", index),
            "stream_id": stream_id.to_string(),
        })),
    )
}
```

**src/api/mod.rs** — Add rules routes:

```rust
mod streams;
mod rules;

use axum::Router;
use std::sync::Arc;
use crate::stream::registry::StreamRegistry;

pub fn api_router(registry: Arc<StreamRegistry>) -> Router {
    Router::new()
        .nest("/api/v1/streams", streams::stream_routes(registry.clone()))
        .nest("/api/v1/streams/{id}/rules", rules::rules_routes(registry))
}
```

Note: Axum 0.8 nested routers with path params need careful handling. The correct approach is to nest the rules router under the stream's path prefix using Axum's `nest`. For Axum 0.8, this should work as the `{id}` param is passed through to the nested router.

Alternative simpler approach: define rules routes within stream routes using `nest`:

```rust
// In api/mod.rs:
pub fn api_router(registry: Arc<StreamRegistry>) -> Router {
    let stream_routes = streams::stream_routes(registry.clone());
    let rules_routes = rules::rules_routes(registry.clone());
    Router::new()
        .nest("/api/v1/streams", stream_routes)
        .nest("/api/v1/streams/:id/rules", rules_routes)
}
```

For Axum 0.8, use `{id}` syntax consistently:

```rust
// api/mod.rs
pub fn api_router(registry: Arc<StreamRegistry>) -> Router {
    Router::new()
        .nest("/api/v1/streams", streams::stream_routes(registry.clone()))
        .nest("/api/v1/streams/{id}/rules", rules::rules_routes(registry))
}
```

</action>
<verify>
cargo check 2>&1
</verify>
<done>
- Rule CRUD endpoints: GET/POST rules list, GET/PUT/DELETE by index
- Hot-reload: update_rules writes to shared Arc<RwLock<Vec<RuleConfig>>>
- Proper error handling for missing stream or invalid index
- Wired into api/mod.rs under /api/v1/streams/{id}/rules
- cargo check passes
</done>
</task>

<task type="auto">
<name>Task 6: Wire everything — main.rs, StreamManager update, config example</name>
<files>src/main.rs, src/api/mod.rs (amend), config.example.yaml</files>
<action>

**src/main.rs** — Update to pass storage/kafka to StreamManager, update API wiring:

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

    // StreamManager with reconnection
    let stream_manager = stream::StreamManager::new(
        storage_client.clone(),
        kafka_producer.clone(),
    );

    // Pre-load streams from config file
    for stream_cfg in &config.preload_streams {
        let id = stream_manager.add_stream(stream_cfg.clone());
        tracing::info!(stream_id = %id, url = %stream_cfg.source_url, "Pre-loaded stream");
    }

    // Health state with stream registry ref for active count
    let health_state = health::HealthState::with_registry(stream_manager.registry().clone());

    // Compose routes
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

**src/health.rs** — Add with_registry constructor:

```rust
// In HealthState:
pub fn with_registry(registry: crate::stream::registry::StreamRegistry) -> Self {
    Self {
        ready: Arc::new(AtomicBool::new(true)),
        start_time: std::time::Instant::now(),
        registry: Some(registry),
    }
}
```

**config.example.yaml** — Add rule examples to preload_streams:

```yaml
preload_streams:
  - source_url: "rtsp://192.168.1.100:554/stream1"
    source_type: "rtsp"
    extract_interval_seconds: 5.0   # Default rule (Interval/5s) created automatically
    jpeg_quality: 85
    ffmpeg_threads: 1
    rtsp_transport: "tcp"
  # Additional streams can be added at runtime via API

# Rule configuration per stream is managed via the REST API:
# POST /api/v1/streams/{id}/rules  {"rule": {"type": "interval", "interval_seconds": 2.0}}
# POST /api/v1/streams/{id}/rules  {"rule": {"type": "fps", "fps": 0.5}}
# POST /api/v1/streams/{id}/rules  {"rule": {"type": "rate_limited", "rule": {"type": "interval", "interval_seconds": 1.0}, "max_per_minute": 10}}

storage:
  bucket: "getframe-frames"
  endpoint_url: "http://localhost:9000"
  region: "us-east-1"

kafka:
  brokers: "localhost:9092"
  topic: "getframe-frames"
  acks: "1"
  compression: "zstd"

http:
  bind_address: "0.0.0.0"
  bind_port: 8080

logging:
  level: "info"
  json: true
```

</action>
<verify>
cargo check 2>&1; cargo build 2>&1
</verify>
<done>
- main.rs passes storage/kafka clients to StreamManager
- HealthState gets registry ref for active stream count
- API routes composed: health + stream CRUD + rules CRUD + metrics
- config.example.yaml updated with rule examples
- cargo build succeeds
</done>
</task>

</tasks>

<dependency_graph>
graph TD
    T1["Task 1: Rule engine refactor"]
    T2["Task 2: Shared rules in registry + health helpers"]
    T3["Task 3: Pipeline integration"]
    T4["Task 4: Reconnection scheduler"]
    T5["Task 5: Rule CRUD API"]
    T6["Task 6: Wire everything"]

    T1 --> T2
    T1 --> T3
    T2 --> T3
    T2 --> T4
    T2 --> T5
    T3 --> T4
    T3 --> T6
    T4 --> T6
    T5 --> T6
</dependency_graph>

<success_criteria>
Phase 3 is complete when ALL of the following are true:

1. ✅ **Rule CRUD API** — REST API at /api/v1/streams/{id}/rules supports create/read/update/delete with hot-reload into running pipelines
2. ✅ **FPS-based extraction** — Rule type `fps` extracts at N frames per second (e.g., fps=0.5 → 1 frame every 2 seconds)
3. ✅ **Rate limiting** — Rule type `rate_limited` enforces max frames per minute, dropping excess frames
4. ✅ **Multiple rules per stream** — RuleEngine evaluates all rules per frame; any match triggers extraction
5. ✅ **Auto-reconnection** — Failed stream pipelines automatically restart with exponential backoff (1s→2s→4s→...→30s max)
6. ✅ **Pipeline health feedback** — Decode loop updates frames_decoded, frames_extracted, last_pts in StreamHealth every 30 frames
7. ✅ **Health lifecycle** — Status transitions: Connecting → Online → Error → (backoff) → Connecting → Online
8. ✅ **Graceful stop** — Stream removal cancels both pipeline and reconnection task; shutdown drains all
</success_criteria>

<verification>
1. `cargo check` — passes with minimal warnings
2. `cargo build` — debug build succeeds
3. Rule API test: `curl -X POST http://localhost:8080/api/v1/streams/{id}/rules -H 'Content-Type: application/json' -d '{"rule":{"type":"fps","fps":1.0}}'` returns 201
4. Rule list test: `curl http://localhost:8080/api/v1/streams/{id}/rules` returns rule list
5. Reconnection test: kill a pipeline thread → backoff → auto-restart (verify via health status)
</verification>
