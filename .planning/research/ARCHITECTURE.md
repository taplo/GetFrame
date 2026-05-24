# Architecture: GetFrame — High-Performance CPU-Only Video Frame Extraction Platform

**Domain:** High-scale video frame extraction platform  
**Researched:** 2026-05-24  
**Mode:** Ecosystem research — Architecture dimension  
**Overall confidence:** MEDIUM (requires validation at Phase 1 with actual benchmarks)

---

## Executive Architecture Summary

GetFrame is a distributed video frame extraction platform that ingests 200-1000+ concurrent 1080P H.264 streams (RTSP/RTMP/HLS/file), decodes them entirely in software, evaluates frame extraction rules, and pushes extracted frames to Kafka. The architecture must maximize CPU throughput under the constraint of zero GPU/NPU acceleration.

**Core architectural decisions:**

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | **Rust** | Zero-cost abstractions, memory safety without GC, FFmpeg bindings, strong async ecosystem, predictable performance |
| Decoding model | **Per-stream dedicated OS thread + FFmpeg `libavcodec`** | H.264 software decoding is CPU-bound and blocking; async won't help. Dedicated threads prevent head-of-line blocking |
| Concurrency model | **Hybrid: OS threads for decode + tokio async for I/O** | Decode is CPU-bound (threads), network I/O and Kafka are async (tokio). Actor-like per-stream state management via tokio tasks |
| Inter-component comms | **Bounded channels (crossbeam + tokio mpsc)** | Backpressure via bounded buffers; prevents memory exhaustion under load |
| Language runtime | **Rust** | Chosen over Go (GC pauses hurt real-time decode) and C++ (safety guarantees, modern tooling) |
| Kafka producer | **Shared producer pool (rdkafka)** | librdkafka is battle-tested; shared pool amortizes connection overhead; per-stream partition key for ordering |
| Rule evaluation | **Compiled evaluation tree from YAML/JSON config** | Rules compiled at config-load time into decision tree; scene change via FFmpeg `scdet` filter metadata |
| K8s deployment | **Stateless workers + KEDA auto-scaling** | No per-stream persistent state; scaling based on CPU + Kafka producer lag custom metrics |

---

## Overall System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Kubernetes Cluster                           │
│                                                                     │
│  ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐   │
│  │   API Gateway   │   │   Web UI        │   │   Config Store  │   │
│  │   (REST API)    │   │   (React)       │   │   (PostgreSQL)  │   │
│  └────────┬────────┘   └─────────────────┘   └─────────────────┘   │
│           │                                                        │
│  ┌────────▼────────────────────────────────────────────────────┐   │
│  │              Stream Manager (control plane)                   │   │
│  │  - Stream lifecycle (create/update/delete/watch)             │   │
│  │  - Rule configuration push                                   │   │
│  │  - Health monitoring / auto-reconnect                        │   │
│  │  - Partition assignment (which worker owns which stream)     │   │
│  └──────────────────────────────────────────────────────────────┘   │
│           │                                                        │
│  ┌────────▼────────────────────────────────────────────────────┐   │
│  │            Worker Pool (data plane — horizontally scaled)     │   │
│  │                                                               │   │
│  │  ┌──────────────────────────────────────────────────────┐    │   │
│  │  │  Stream Instance (one per stream, runs in a thread)   │    │   │
│  │  │                                                        │    │   │
│  │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────┐  │    │   │
│  │  │  │ Ingestion│─►│ Decode   │─►│ Rule Eval│─►│Kafka │  │    │   │
│  │  │  │ (network)│  │ (CPU)    │  │ (CPU)    │  │Producer│  │    │   │
│  │  │  └──────────┘  └──────────┘  └──────────┘  └──────┘  │    │   │
│  │  │                                                        │    │   │
│  │  │        Channel ring buffers (bounded, backpressured)    │    │   │
│  │  └──────────────────────────────────────────────────────┘    │   │
│  │                                                               │   │
│  │  ┌── Stream 1 (thread/task) ──┐  ┌── Stream 2 ──────────┐   │   │
│  │  │  RTSP → decode → eval→Kafka│  │  HLS → decode→...   │   │   │
│  │  └────────────────────────────┘  └──────────────────────┘   │   │
│  │  ... up to N streams per worker                               │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │   Kafka Cluster  │
                    │  (per-stream     │
                    │   topics or      │
                    │   partitioned    │
                    │   topic)         │
                    └─────────────────┘
```

---

## Component Boundaries

### 1. Stream Manager (Control Plane)

**Responsibility:** Stream lifecycle management, configuration distribution, health monitoring, auto-reconnect logic.

**Runs as:** Single (or HA pair) Kubernetes deployment, 1-2 replicas.

**Communicates with:**
- **API Gateway** — receives stream CRUD commands, returns status
- **PostgreSQL** — persists stream configurations, rules, connection state
- **Workers** — pushes config updates via shared ConfigMap or internal gRPC channel

**Key interfaces:**

```rust
// Stream Manager internal interface
trait StreamManager {
    /// Register a new stream for ingestion
    async fn create_stream(config: StreamConfig) -> Result<StreamId, Error>;
    
    /// Update stream configuration (rules, source URL, etc.)
    async fn update_stream(id: StreamId, config: StreamConfig) -> Result<(), Error>;
    
    /// Remove a stream
    async fn delete_stream(id: StreamId) -> Result<(), Error>;
    
    /// Get stream status (connected, decoding, fps, errors)
    async fn get_stream_status(id: StreamId) -> Result<StreamStatus, Error>;
    
    /// List all streams with filter/pagination
    async fn list_streams(filter: StreamFilter) -> Result<Vec<StreamSummary>, Error>;
    
    /// Distribute config to workers (push or watch-based)
    async fn distribute_config(assignment: WorkerAssignment) -> Result<(), Error>;
}
```

**State:** None per-stream beyond metadata in PostgreSQL. Stateless for scaling.

### 2. Worker (Data Plane)

**Responsibility:** Hosts N stream instances. Each worker is a single process running multiple per-stream decode pipelines.

**Runs as:** Kubernetes Deployment, horizontally scaled. N workers each handling M streams.

**Key architectural rule:** Each worker runs `min($cpu_cores * 0.5, 200)` streams per node at steady state. This prevents CPU oversubscription.

**Internal architecture:**

```
┌──────────────────────────────────────────────────────────────────┐
│  Worker Process                                                    │
│                                                                    │
│  ┌─────────────┐  ┌────────────────┐  ┌────────────────────────┐ │
│  │  Supervisor  │  │  Stream        │  │  Kafka Producer Pool   │ │
│  │  Task        │──│  Registry      │  │  (rdkafka, N producers)│ │
│  │  (tokio)     │  │  (HashMap)     │  │                        │ │
│  └─────────────┘  └────────────────┘  └────────────────────────┘ │
│                                                                    │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │  Per-Stream Pipeline (one OS thread per stream)              │ │
│  │                                                              │ │
│  │  Thread: [Ingest]──chan──>[Decode]──chan──>[Rule]──chan──>   │ │
│  │                                      async send to producer  │ │
│  │                                                              │ │
│  │  Channels: crossbeam::channel::bounded<T>                    │ │
│  │  - ingest→decode: 64 frames (raw AVPacket)                   │ │
│  │  - decode→rule:    8 frames  (AVFrame raw data)              │ │
│  │  - rule→kafka:     256 frames (encoded JPEG bytes)           │ │
│  └──────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

### 3. Stream Instance (Internal Worker Component)

Each stream is managed by a **dedicated OS thread** (not an async task) running a synchronous pipeline. Rationale:

- **H.264 software decoding via FFmpeg is blocking CPU work** — async cooperative scheduling cannot preempt a decode call. Dedicated OS threads allow the OS scheduler to time-slice correctly.
- **Pipeline stages within a stream are sequential** — there's no benefit to interleaving decode and I/O within a single stream (decode produces frames → rule consumes frames → producer sends frames).
- **OS thread per stream** at 200 streams on 16-32 cores gives ~6-12 streams per core, which is manageable for the OS scheduler.

**Pipeline stages:**

| Stage | Thread | CPU/IO | Bounded Buffer | Notes |
|-------|--------|--------|---------------|-------|
| **Ingestion** | Network I/O thread per stream | I/O-bound (network read) | `ingest_channel`: 64 `AVPacket`s | Reads RTSP/RTMP/HLS, demuxes, pushes packets |
| **Decode** | Dedicated decode thread | CPU-bound (libavcodec) | `decode_channel`: 8 `AVFrame`s | `avcodec_decode_video2()` in loop. 8-frame buffer limits memory: 1920×1080×4×8 ≈ 66MB max per stream |
| **Rule Evaluation** | Same decode thread (sequential) | CPU-light (bounded) | `rule_channel`: 256 `ExtractedFrame` | Evaluates rules against frame metadata/time. Scene change uses FFmpeg `scdet` filter metadata (per-frame score). Only matching frames → JPEG encode → Kafka |
| **Kafka Producer** | Async tokio task pool (shared) | I/O-bound (network) | N/A (async send) | Uses `rdkafka::FutureProducer`. Shared across streams. Per-stream partition key |

**Key insight:** Staging decode and rule eval on the same thread avoids an extra cross-thread copy of frame data. The only cross-thread handoff is `AVPacket` → decoded `AVFrame` (large) and `ExtractedFrame` → encoded JPEG bytes (smaller). The 8-frame decode buffer is a deliberate pressure point — if the rule evaluator falls behind, backpressure propagates to the decoder, which naturally paces the decode rate.

**Interface definition:**

```rust
// Stream pipeline — instantiated once per stream in a dedicated OS thread
struct StreamPipeline {
    id: StreamId,
    config: StreamConfig,
    state: StreamState,
    
    // Bounded channels between pipeline stages
    ingest_tx: crossbeam::Sender<OwnedPacket>,
    decode_tx: crossbeam::Sender<DecodedFrame>,
    frames_for_kafka_tx: crossbeam::Sender<ExtractedFrame>,
}

struct DecodedFrame {
    pts: i64,
    time_base: Rational,
    data: Vec<u8>,           // Raw YUV420P or RGB data
    width: u32,
    height: u32,
    scene_change_score: f32, // Set by scdet filter metadata
    key_frame: bool,
}

struct ExtractedFrame {
    stream_id: StreamId,
    pts: i64,
    timestamp: Duration,
    jpeg_bytes: Vec<u8>,
    rule_trigger: RuleTrigger, // Which rule caused extraction
    scene_score: Option<f32>,
}
```

---

## Data Flow

### Normal frame extraction path (time-based rule):

```
1. Network → [demux] → AVPacket (compressed H.264 NAL unit)
2. AVPacket → [avcodec_send_packet/avcodec_receive_frame] → AVFrame (raw YUV420P, 1920×1080×1.5 ≈ 3.1MB)
3. AVFrame → [scdet filter] → scene_change_score metadata attached
4. AVFrame → [rule engine] → "PTS matches interval rule? Extract."
5. AVFrame → [libjpeg-turbo encode] → JPEG bytes (quality=85, ~200-500KB per frame)
6. JPEG bytes → [kafka producer] → Kafka topic (partitioned by stream_id)

Timeline per frame: decode ~8-15ms → rule eval ~0.01ms → JPEG encode ~20-50ms → kafka send ~1-5ms
                                                                      ↑ THIS IS THE BOTTLENECK
```

### Scene-change detection path:

```
1-3. Same as above
4. AVFrame metadata has lavfi.scd.score from scdet filter
5. Rule engine checks: scene_score > threshold (e.g., 0.4)
6. If above threshold → JPEG encode → Kafka
7. If below → discard frame, continue decoding
```

### Stream reconnect path:

```
1. Network read timeout / HTTP disconnect
2. Ingest thread detects error, sets StreamState::Reconnecting
3. Exponentially backs off: 1s, 2s, 4s, 8s... up to 60s max
4. Attempts reconnect with FFmpeg AVFormatContext::avformat_open_input
5. On success: resets decoder state (avcodec_flush_buffers), resumes pipeline
6. On persistent failure (N retries): reports to supervisor, stream marked as FAILED
7. Supervisor notifies Stream Manager via heartbeat with status update
```

### Backpressure chain:

```
Kafka broker slow → producer buffer fills → send() returns Backpressure
  → frames_for_kafka channel blocks → rule eval blocks (backpressure on encode)
    → decode thread blocks on decode_tx.send() → decoder pauses
      → ingest thread blocks on ingest_tx.send() → network read pauses
        → TCP receive buffer fills → RTSP source backs off
```

This is a **virtuous backpressure chain** — every stage naturally paces the upstream stage via bounded channel blocking. No separate flow control mechanism needed.

---

## Concurrency Model — Deep Dive

### Why not pure async?

Rust's tokio async is excellent for I/O-bound workloads but **actively harmful** for CPU-bound video decoding:

1. **`avcodec_send_packet` / `avcodec_receive_frame` are blocking calls** — they may take 5-30ms for a single H.264 frame decode. In an async context, this blocks the entire worker thread's event loop, starving all other streams on that thread.

2. **`spawn_blocking` doesn't scale** — tokio's `spawn_blocking` creates OS threads from a thread pool. For 200 streams, you'd need 200 blocking threads anyway, so you might as well own them directly.

3. **JPEG encoding with libjpeg-turbo is also blocking** — similar reasoning applies.

### The hybrid model

```
┌──────────────────────────────────────────────────────────────────┐
│  Worker Process                                                    │
│                                                                    │
│  Thread 0: Tokio runtime (main thread)                             │
│  ├── HTTP health endpoint (axum)                                   │
│  ├── Configuration watch loop                                      │
│  ├── Kafka producer background (rdkafka internal poll)            │
│  ├── Metrics export (prometheus)                                  │
│  └── Stream supervisor                                            │
│                                                                     │
│  Thread 1..N: OS threads, each pinned to a logical core            │
│  ├── Each thread runs (2-8) stream pipelines sequentially          │
│  ├── Round-robin: decode frame from stream A → rule eval →         │
│  │   → decode frame from stream B → ...                            │
│  ├── No async here. Pure synchronous FFmpeg calls                  │
│  └── Channels to tokio for Kafka sends (async bridge)              │
│                                                                     │
│  Thread N+1: OS threads for Kafka producer poll (rdkafka)          │
│  └── librdkafka requires periodic poll() for callbacks             │
└──────────────────────────────────────────────────────────────────┘
```

### Core-affinitized stream scheduling

```rust
// Pseudo-code for per-core scheduling
fn worker_thread_main(core_id: usize, assigned_streams: Vec<StreamPipeline>) {
    // Pin thread to specific CPU core
    pin_thread_to_core(core_id);
    
    let mut streams = assigned_streams;
    let mut idx = 0;
    
    loop {
        // Round-robin across assigned streams
        let stream = &mut streams[idx % streams.len()];
        idx += 1;
        
        // Try to decode one frame (non-blocking from our perspective)
        match stream.decode_one_frame() {
            Ok(Some(frame)) => {
                // Rule evaluation (same thread, no context switch)
                if stream.rule_engine.should_extract(&frame) {
                    let jpeg = encode_jpeg(&frame);
                    // Non-blocking send to tokio's Kafka producer
                    stream.kafka_tx.try_send(jpeg).ok();
                }
            }
            Ok(None) => {
                // No frame ready (e.g., HLS buffer underrun). Small yield.
                std::thread::yield_now();
            }
            Err(StreamError::Disconnected) => {
                stream.handle_reconnect();
            }
        }
    }
}
```

### Why not thread-per-stream at 200+ streams?

At 200 streams with 32 cores:
- **Thread-per-stream**: 200 threads. Context switching overhead becomes significant. Stack memory: 200 × 2MB (default) = 400MB just for stacks.
- **Round-robin with 32 threads**: Each thread handles ~6-7 streams. Very manageable. Stacks: 32 × 2MB = 64MB.

The decode threads are **time-sliced by cooperation** (one frame per stream per round), not by preemption. This gives fair CPU distribution across streams.

---

## Memory Architecture

### Frame buffer management

This is the critical memory concern. Each decoded 1080p YUV420P frame is ~3.1MB. At 200 streams:

| Component | Per-stream | 200 streams | Notes |
|-----------|-----------|-------------|-------|
| Decode buffer (8 frames) | ~25 MB | ~5 GB | Maximum resident memory |
| JPEG buffer (in-flight) | ~2 MB | ~400 MB | Varies with frame content |
| Kafka producer buffer | ~8 MB | ~1.6 GB | `buffer.memory` per producer × N producers |
| **Total worst case** | | **~7 GB** | Excluding OS cache, code |

**Strategies:**

1. **`bumpalo` arena allocator for frame data** — Decoded frames are allocated from a per-stream arena. After rule evaluation, the arena is reset (frames are either discarded or JPEG-encoded, which copies the necessary data). This eliminates per-frame `malloc/free` overhead.

2. **Object pool for `AVPacket`** — Pre-allocate a pool of `AVPacket` structs. FFmpeg's `av_packet_unref` returns the struct to the pool instead of freeing. Critical because each compressed packet is typically small (few KB) but there are many of them.

3. **JPEG encoding buffer reuse** — libjpeg-turbo's `tjInitCompress` context is recycled per-stream. Destination buffer is grown as needed but never shrunk (amortized allocation).

4. **Zero-copy Kafka path** — `rdkafka::FutureProducer::send` can take a `Vec<u8>` and hand ownership to librdkafka (zero-copy on the producer side). Avoid `to_bytes()` copies.

```rust
// Per-stream frame lifecycle
struct StreamMemoryPool {
    decode_arena: bumpalo::Bump,           // Arena for decoded frames
    avpacket_pool: ObjectPool<AVPacket>,   // Reusable AVPacket structs
    jpeg_encoder: RefCell<tj::Compressor>, // Recycled libjpeg-turbo handle
    jpeg_output_buf: RefCell<Vec<u8>>,     // Grown-as-needed output buffer
}

impl StreamMemoryPool {
    fn reset_decode_arena(&self) {
        self.decode_arena.reset();  // O(1) — no per-element Drop
    }
}
```

---

## Kafka Integration Architecture

### Producer Design

**Decision: Shared producer pool with `rdkafka::FutureProducer`.**

- **Not per-stream producer** — 1000 connections to Kafka is wasteful (unnecessary connections, memory for each producer's internal buffers).
- **Not single producer** — single producer can bottleneck; librdkafka's internal I/O thread handles sends for one producer, limiting throughput for 1000 streams.
- **Pool of N producers** — `N = min(4, num_cpu_cores)`. Each producer has its own I/O thread and connection to Kafka brokers.

**Partitioning strategy:**
- Single topic `getframe.extracted_frames` partitioned by `hash(stream_id) % P` (where P = partition count).
- This preserves per-stream order (all frames from one stream go to one partition).
- Downstream consumers can read a single partition for stream-aligned processing.

**Producer configuration for throughput:**

```rust
ClientConfig::new()
    .set("bootstrap.servers", &brokers)
    .set("acks", "1")                          // Leader ack only (durability vs throughput tradeoff)
    .set("compression.type", "zstd")            // Best compression ratio; frames compress well
    .set("batch.size", "131072")                // 128KB batches
    .set("linger.ms", "100")                    // 100ms max latency batching
    .set("buffer.memory", "134217728")          // 128MB per producer
    .set("max.in.flight.requests.per.connection", "5")
    .set("enable.idempotence", "false")         // Not needed for at-least-once
    .set("queue.buffering.max.kbytes", "1048576") // 1GB max queued (system memory permitting)
```

**Why `acks=1` and not `acks=all`:** We're targeting at-least-once delivery, not exactly-once. If a broker crashes after acknowledging but before replicating, the frame could be lost. Acceptable tradeoff for 3-5x throughput gain. Use `acks=all` if durability is paramount (add ~20% latency).

### Message format

```json
{
  "schema_version": 1,
  "stream_id": "uuid-of-stream",
  "source_type": "rtsp|rtmp|hls|file",
  "frame": {
    "pts": 123456789,
    "timestamp_seconds": 42.567,
    "reason": "interval|scene_change|composite",
    "rule_id": "rule-001",
    "scene_score": 0.87
  },
  "image": {
    "format": "jpeg",
    "quality": 85,
    "width": 1920,
    "height": 1080,
    "size_bytes": 342156
  },
  "producer": {
    "worker_id": "worker-3",
    "timestamp": "2026-05-24T10:30:00Z"
  }
}
// Image payload follows as separate Kafka record value (bytes)
```

**Key consideration:** Frame payloads (JPEG bytes) should be the Kafka record value. Metadata should be in record headers or a separate envelope topic. This keeps the topic compactible (log compaction on key works with metadata-only deletes).

### Delivery semantics

| Guarantee | Implementation | Cost |
|-----------|---------------|------|
| **At-least-once** (default) | `acks=1`, retries=5, synchronous error handling | Potential duplicate frames |
| **At-most-once** | `acks=0` | Frame loss on failure |
| **Exactly-once** | Idempotent producer (`enable.idempotence=true`) + `acks=all` | 20-40% throughput reduction |

**Recommendation:** At-least-once as default. Downstream consumers must handle duplicates (idempotent processing). If the platform evolves to need exactly-once, the infrastructure supports it.

---

## Rule Engine Architecture

### Design: Compiled rule evaluation tree

Rules are defined in YAML, compiled at config-load time into an efficient evaluation tree, evaluated per-decoded-frame with zero dynamic dispatch.

**Rule definition format:**

```yaml
streams:
  - id: camera-01
    rules:
      - id: rule-every-5s
        type: interval
        every_seconds: 5
        # Also extract if scene change happens between intervals
        composite: any
        
      - id: rule-scene-change
        type: scene_change
        threshold: 0.4  # scdet filter threshold (8.0-14.0 range)
        
      - id: rule-complex
        type: composite
        operator: any   # any | all
        rules:
          - type: interval
            every_seconds: 30
          - type: scene_change
            threshold: 0.6
```

**Compilation flow:**

```
YAML config → RuleParser → Vec<Box<dyn RuleNode>> evaluation tree
                              ↓
                          Compiled to:
                              ↓
                     IntervalNode { next_pts }
                     SceneChangeNode { threshold }
                     CompositeNode { operator, children }
```

**Evaluation performance:** Each node's `evaluate(&DecodedFrame) -> bool` is a pure function with no allocations. For a typical config with 1 interval + 1 scene change rule: ~0.01ms per frame (< 0.1% of a decode cycle).

### Scene change detection

**Decision: Use FFmpeg's built-in `scdet` filter (since FFmpeg 4.3).**

- The `scdet` filter calculates Mean Absolute Frame Difference (MAFD) between consecutive frames
- Outputs metadata keys: `lavfi.scd.mafd`, `lavfi.scd.score`, `lavfi.scd.time`
- Threshold range `8.0-14.0` is recommended; `10.0` is default
- **This is essentially free** — the computation happens during the already-necessary decode pass. No extra pass needed.

```rust
// In the decode loop — after avcodec_receive_frame
fn check_scene_change(frame: &AVFrame, threshold: f32) -> Option<f32> {
    // Read metadata set by scdet filter (set via AVFrame side data or filter graph)
    let score = frame.metadata.get("lavfi.scd.score")?;
    if score > threshold {
        Some(score)
    } else {
        None
    }
}
```

**Alternative approaches considered and rejected:**
| Approach | Reason Rejected |
|----------|----------------|
| Pixel-by-pixel comparison | Too slow per-frame; reinvents what scdet does |
| Histogram comparison (OpenCV) | Extra dependency; adds 1-3ms per frame |
| ML-based scene detection | Entirely out of scope (no GPU); 100x+ overhead |
| External process (PySceneDetect) | Python dependency; IPC overhead doesn't scale |

The `scdet` approach is **LOW complexity, HIGH efficiency** for this use case.

---

## Kubernetes Architecture

### Component deployment model

| Component | K8s Resource | Replicas | Stateful? | Scaling |
|-----------|-------------|----------|-----------|---------|
| **Stream Manager** | Deployment | 1-2 | No (stateless; PG-backed) | Manual |
| **Worker** | Deployment | 3-20+ | No | **Auto (KEDA)** |
| **Web UI** | Deployment | 1-2 | No | Manual |
| **PostgreSQL** | StatefulSet / external | 1-3 (HA) | Yes | Manual |
| **Redis** (optional, for stream registry) | StatefulSet | 1-3 (sentinel) | Yes | Manual |

### Auto-scaling policy (KEDA)

**Scaler: Custom metrics + CPU**

```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: getframe-worker-scaler
spec:
  scaleTargetRef:
    name: getframe-worker
  minReplicaCount: 3
  maxReplicaCount: 50
  triggers:
    - type: prometheus
      metadata:
        serverAddress: http://prometheus:9090
        metricName: kafka_producer_lag
        query: |
          sum(rate(kafka_producer_record_send_total[30s])) 
          / sum(kafka_producer_record_send_rate{})
        threshold: "0.8"  # 80% of max producer throughput
    - type: cpu
      metadata:
        type: Utilization
        value: "70"  # Scale when average CPU > 70%
```

**Scaling logic:**
- **Scale up**: CPU > 70% OR Kafka producer lag growing → add workers
- **Scale down**: CPU < 30% for 5 minutes → remove workers
- **Cooldown**: Minimum 60 seconds between scale events (prevents thrashing)

### Resource allocation per worker

```yaml
resources:
  requests:
    cpu: "8"        # 8 cores minimum
    memory: "12Gi"   # 12GB RAM minimum
  limits:
    cpu: "16"        # Can burst to 16 cores
    memory: "24Gi"   # 24GB max
```

**Rationale:** A worker with 8-16 cores can handle ~100-200 streams (depending on frame rate and rule complexity). Memory limit of 24GB accounts for worst-case frame buffer usage + OS cache.

### Stateless worker design

Workers are **stateless** — they don't own persistent data. Stream assignments are distributed via the Stream Manager at startup and adjusted via rolling updates:

1. Worker starts, registers with Stream Manager
2. Stream Manager assigns `N` streams to the worker
3. Worker begins processing
4. On scale-down, Stream Manager drains streams from removed workers before terminating
5. On worker crash, orphaned streams are detected by Stream Manager via heartbeat timeout and reassigned

**Required for worker statelessness:**
- **Stream source URLs are stored in PostgreSQL** (not on the worker)
- **Kafka offsets are managed by Kafka** (not by workers)
- **No per-stream state on filesystem** — everything is in-memory and rebuildable

---

## Observability Architecture

### Per-stream metrics (Prometheus)

```rust
// Prometheus metrics labels: stream_id, worker_id, source_type
lazy_static! {
    static ref STREAM_FRAMES_DECODED: IntCounterVec = register_int_counter_vec!(
        "getframe_frames_decoded_total",
        "Total frames decoded per stream",
        &["stream_id", "worker_id"]
    ).unwrap();
    
    static ref STREAM_FRAMES_EXTRACTED: IntCounterVec = register_int_counter_vec!(
        "getframe_frames_extracted_total",
        "Total frames extracted (sent to Kafka) per stream",
        &["stream_id", "rule_id"]
    ).unwrap();
    
    static ref STREAM_DECODE_DURATION: HistogramVec = register_histogram_vec!(
        "getframe_decode_duration_seconds",
        "Decode time per frame",
        &["stream_id"],
        vec![0.005, 0.010, 0.015, 0.025, 0.050, 0.100]
    ).unwrap();
    
    static ref STREAM_KAFKA_SEND_DURATION: HistogramVec = register_histogram_vec!(
        "getframe_kafka_send_duration_seconds",
        "Time to send frame to Kafka",
        &["stream_id"]
    ).unwrap();
    
    static ref STREAM_CONNECTION_STATE: GaugeVec = register_gauge_vec!(
        "getframe_stream_connection_state",
        "Stream connection state: 0=disconnected, 1=connecting, 2=connected, 3=error",
        &["stream_id"]
    ).unwrap();
    
    static ref STREAM_SCENE_CHANGE_COUNT: IntCounterVec = register_int_counter_vec!(
        "getframe_scene_changes_detected_total",
        "Number of scene changes detected",
        &["stream_id"]
    ).unwrap();
    
    static ref WORKER_STREAM_COUNT: Gauge = register_gauge!(
        "getframe_worker_active_streams",
        "Current number of active streams on this worker"
    ).unwrap();
    
    static ref KAFKA_PRODUCER_QUEUE_SIZE: GaugeVec = register_gauge_vec!(
        "getframe_kafka_producer_queue_messages",
        "Messages in Kafka producer queue",
        &["producer_id"]
    ).unwrap();
}
```

### Structured logging

**Format:** JSON (via `tracing` crate with `tracing-subscriber` JSON layer)

```json
{
  "timestamp": "2026-05-24T10:30:00.123456Z",
  "level": "INFO",
  "target": "getframe::stream",
  "fields": {
    "stream_id": "camera-01",
    "event": "frame_extracted",
    "pts": 123456789,
    "rule": "interval-5s",
    "decode_ms": 12.3,
    "encode_ms": 31.2,
    "kafka_ms": 2.1
  }
}
```

**Log levels by severity:**

| Level | When | What |
|-------|------|------|
| ERROR | Stream disconnect, Kafka send failure, config parse failure | Every occurrence |
| WARN | Reconnect attempt, slow decode (>50ms), backpressure warning | Every occurrence |
| INFO | Stream start/stop, config change, scaling event | Per lifecycle event |
| DEBUG | Frame extracted, rule evaluation details | Per frame (use sparingly in production) |
| TRACE | Raw packet/demuxer details | Debugging only |

### Health check endpoints

```rust
// Worker health check (exposed on :8080/health)
GET /health → {
    "status": "healthy" | "degraded" | "unhealthy",
    "active_streams": 156,
    "max_streams": 200,
    "cpu_usage_pct": 67.2,
    "memory_usage_mb": 8192,
    "kafka_connected": true,
    "stream_manager_connected": true,
    "uptime_seconds": 123456
}

// Worker readiness check (exposed on :8080/ready)
GET /ready → 200 OK (when worker has initialized and connected to dependencies)
GET /ready → 503 Service Unavailable (during startup or draining)

// K8s probes
livenessProbe:  HTTP GET /health, initialDelay: 30s, period: 30s
readinessProbe: HTTP GET /ready,  initialDelay: 10s, period: 10s
```

---

## Bottleneck Analysis

### Critical path: Decode → JPEG encode → Kafka send

```
1080p H.264 decode:     ~10ms per frame (single thread, modern x86_64 with AVX2)
JPEG encode (Q=85):     ~25ms per frame (libjpeg-turbo, single thread)
Kafka send:             ~2ms per frame (async, batching amortizes)

Total per extracted frame: ~37ms on the decode thread
```

**At 1fps extraction rate per stream:** 37ms / 1000ms = 3.7% CPU per stream. With 200 streams on 8 cores: each core handles 25 streams → 25 × 37ms = 925ms per round. **At the edge of 1-core saturation.**

**Mitigation:** CPU profiling at Phase 1 will reveal the true decode/encode ratio. Likely optimization path: decode is optimized by FFmpeg's hand-tuned assembly, but **JPEG encode is the squeeze point**. Options:
- Reduce JPEG quality (Q=70 is often visually acceptable, gives ~40% smaller files)
- Use `libjpeg-turbo` SIMD paths (already fast, but tune for target architecture)
- If decode takes 10ms and encode takes 25ms, the ratio is ~40:60. Adding another core per worker for JPEG encoding (dedicated encoder pool) could nearly double throughput.

### Known CPU bottlenecks (by order of severity):

| Bottleneck | Severity | Cause | Mitigation |
|-----------|----------|-------|------------|
| **JPEG encoding** | HIGH | Each extracted frame requires full JPEG encode (25ms) | Reduce quality, use progressive JPEG, consider JPEG-XL or WebP if encode time improves |
| **H.264 software decode** | HIGH | Pure CPU decode, 10ms per frame | `ffmpeg` with `-threads auto`, use `h264_decode` with all available SIMD; consider edge264 as alternative decoder if performance-critical |
| **Frame copy overhead** | MEDIUM | FFmpeg frame data must be in contiguous memory for JPEG encode | `av_frame_get_buffer` with proper alignment; reuse buffers |
| **Kafka producer I/O** | MEDIUM | Network round-trip for each batch | Increase batch size, use zstd compression, tune `linger.ms` |
| **Context switching** | LOW | 200+ threads on moderate core count | Pin threads to cores; use round-robin scheduling |

---

## Build Order (Component Dependencies)

### Phase recommendations for architecture build:

```
Phase 1: Core Pipeline (prove decode throughput)
  ├── Single-stream FFmpeg decode loop (Rust + ffmpeg-next)
  ├── JPEG encoding (libjpeg-turbo)
  ├── Bounded channel pipeline prototype
  ├── Frame buffer pool (bumpalo)
  └── BENCHMARK: decode throughput, memory usage per stream
  
Phase 2: Multi-Stream Scaling
  ├── Per-core thread scheduling with core pinning
  ├── Multi-stream management (supervisor task)
  ├── Stream lifecycle (create, destroy, reconnect)
  └── BENCHMARK: N streams on M cores — saturation point

Phase 3: Rule Engine
  ├── Rule config parser (YAML → evaluation tree)
  ├── Interval-based rules
  ├── Scene change detection (FFmpeg scdet)
  ├── Composite rules
  └── BENCHMARK: rule evaluation overhead

Phase 4: Kafka Integration
  ├── rdkafka FutureProducer integration
  ├── Producer pool (shared across streams)
  ├── Frame metadata schema + Avro/JSON serialization
  ├── Delivery confirmation + retry
  └── BENCHMARK: Kafka throughput at scale

Phase 5: Management Layer
  ├── REST API (axum)
  ├── PostgreSQL integration for stream config
  ├── Stream Manager service
  └── K8s health probes

Phase 6: Web UI
  ├── React frontend (or alternative)
  ├── Stream CRUD interface
  ├── Live metrics dashboard
  └── Rule management UI

Phase 7: K8s Deployment
  ├── Dockerfile + Helm chart
  ├── KEDA auto-scaling configuration
  ├── Prometheus metrics export
  ├── Grafana dashboard
  └── Chaos testing (network partitions, pod failures)
```

### Dependency graph:

```
Phase 1 ──► Phase 2 ──► Phase 3 ──► Phase 4 ──► Phase 5 ──► Phase 6
                  │                         │
                  └────── Phase 3 ──────────┘
                                    │
                                    └── Phase 7 (can parallelize post-Phase 4)
```

---

## Architecture Anti-Patterns to Avoid

| Anti-Pattern | Why Bad | Instead |
|-------------|---------|---------|
| **Single tokio task per stream doing everything** | Cooperative async can't yield during blocking decode; one slow stream starves others | Dedicated OS thread per decode pipeline |
| **Unbounded channels between pipeline stages** | Memory exhaustion under load; OOM kills the pod | All channels bounded with small capacities; backpressure is a feature, not a bug |
| **Per-stream Kafka producer** | 200+ connections to Kafka; excessive `buffer.memory` allocation | Shared producer pool with stream-partitioned keys |
| **Spawning FFmpeg as a subprocess per stream** | Process spawn overhead; IPC cost of pipe; harder to monitor | Link FFmpeg as a library (`libavcodec`, `libavformat`) |
| **Global mutex for frame pool** | Contention at high concurrency destroys cache locality | Per-thread or per-stream arena allocator |
| **Python runtime for rule engine** | GIL limits parallelism; CPython overhead adds 5-50ms per rule eval | Compiled Rust evaluation tree (microseconds) |
| **Store frame data in Kafka messages > 1MB** | Kafka has a default 1MB max message size; JPEG frames can exceed this | Split large frames; use Kafka headers for metadata and store payload separately (or ensure quality settings keep frames under 1MB) |

---

## Sources and Confidence

| Finding | Source | Confidence | Notes |
|---------|--------|-----------|-------|
| FFmpeg `libavcodec` H.264 decode: ~10ms per 1080p frame on modern CPU | General knowledge + WebSearch benchmarks | **MEDIUM** — requires validation with target hardware |
| `scdet` filter available since FFmpeg 4.3 | FFmpeg documentation | **HIGH** — confirmed in docs |
| Rust `rdkafka` crate wraps librdkafka 1.9.2+ | crates.io / docs.rs | **HIGH** |
| Tokio work-stealing scheduler handles async I/O efficiently | Tokio documentation | **HIGH** |
| `bumpalo` arena allocator for frame reuse | GitHub / crates.io | **HIGH** |
| KEDA custom metrics autoscaling with Prometheus | KEDA documentation | **HIGH** |
| Core-pinned threads reduce context switching overhead | Operating systems knowledge | **HIGH** — fundamental OS concept |
| JPEG encoding is significant bottleneck (25ms per 1080p frame with libjpeg-turbo) | General knowledge + WebSearch | **MEDIUM** — depends on CPU generation, quality setting |
| Crossbeam bounded channels for zero-cost backpressure | crossbeam documentation | **HIGH** |

---

## Open Questions (Need Phase Validation)

1. **Real-world decode throughput per core:** What is the actual 1080p H.264 decode speed on target cloud CPU (e.g., Intel Xeon 4th/5th Gen)? 10ms/frame is an estimate — needs benchmarking.

2. **JPEG encode vs memory bandwidth tradeoff:** Is it faster to encode JPEG on the decode thread or hand off to a dedicated encode thread pool? The latter adds a cross-thread copy but enables decode/encode parallelism.

3. **`h264` vs `h264_cuvid` for software decode:** Without GPU, only `h264` is available. However, Intel QSV (`h264_qsv`) uses integrated GPU — is this available on target hardware? The project says "no GPU/NPU" but integrated iGPU might be acceptable.

4. **Librdkafka thread model vs tokio:** rd_kafka's internal I/O thread and poll-based model. Does `rdkafka::FutureProducer` play well with tokio at 200+ concurrent senders? Need to test.

5. **PostgreSQL vs ConfigMap for stream config distribution:** For 1000+ streams with rapid config updates, can K8s ConfigMaps handle the update rate? Or use a dedicated config service backed by PG?

6. **edge264 as alternative decoder:** [edge264](https://github.com/tvlabs/edge264) claims competitive performance to FFmpeg's `libavcodec`. Worth evaluating at Phase 1 as a drop-in replacement for the decode stage.
