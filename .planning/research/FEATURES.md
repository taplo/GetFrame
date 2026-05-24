# Feature Landscape

**Domain:** High-performance video frame extraction platform (CPU-only, Kubernetes-native)
**Researched:** 2026-05-24
**Mode:** Ecosystem research

## Competitive Landscape Summary

No existing open-source platform directly solves the "extract frames via rule engine → push to Kafka" use case at scale. The closest categories:

| Category | Examples | Gap for GetFrame |
|----------|---------|-----------------|
| **Video Surveillance (NVR/VMS)** | ZoneMinder, Kerberos.io, iSpy, Shinobi, Blue Iris | Built for recording/playback, not programmable frame extraction. No Kafka output. No rule engine for extraction. Motion detection but not configurable per-stream frame sampling. |
| **CV Frameworks** | Pipeless, VideoPipe, DeepStream (NVIDIA) | Pipeless is Python-only, not designed for 200+ streams. DeepStream requires GPU. VideoPipe is C++ but opinionated about CV pipelines. |
| **Cloud Services** | AWS Kinesis Video Streams + Fargate, Cloudinary | Vendor lock-in, expensive at scale, not CPU-optimized. |
| **FFmpeg Scripts** | Custom Python/Go wrappers | No management UI, no rule engine, no Kafka integration built-in, no health monitoring. |
| **Agent Skills** | video-frames-skill, PySceneDetect | Single-video focused. Not designed for persistent stream ingestion. |

**Key insight:** GetFrame occupies a unique niche — *persistent stream ingestion + programmable frame extraction + Kafka output* on a CPU-only Kubernetes stack. The closest comparison is a purpose-built "frame extraction middleware" that doesn't exist yet.

---

## Table Stakes

Features users expect as a minimum. Without these, the product feels incomplete and untrustworthy.

### STREAM-01: Video Source Management

| Feature | Why Expected | Complexity | Priority |
|---------|--------------|------------|----------|
| **Add/edit/delete stream sources** | Basic CRUD for stream lifecycle | S | P0 |
| **Stream type identification** (RTSP/RTMP/HLS/file) | User needs to know what type they configured | S | P0 |
| **Stream URL validation** (test connection) | Users won't trust a black box — need to verify reachability before creating task | M | P0 |
| **Stream health status indicator** (Online/Offline/Error) | Must know immediately if a stream is down | S | P0 |
| **Stream metadata** (name, description, tags, location) | Basic organization for 200+ streams | S | P0 |
| **Batch import streams** (CSV/JSON bulk add) | At 1000+ streams, individual add is painful | M | P1 |

**Dependencies:** None (foundational feature)

### STREAM-02: Stream Connection Reliability

| Feature | Why Expected | Complexity | Priority |
|---------|--------------|------------|----------|
| **Automatic reconnection with exponential backoff** | RTSP streams drop constantly in production. Users expect self-healing. | M | P0 |
| **Configurable reconnection policy** (max retries, backoff params, timeout) | Different streams have different reliability needs | M | P1 |
| **Stream timeout handling** (idle timeout, no-frame timeout) | Prevents zombie connections consuming resources | S | P0 |
| **Ice/restart for long-lived connections** | Some RTSP cameras need periodic session refresh | M | P2 |
| **Graceful degradation on stream failure** (log + alert, don't crash) | One failed stream should not affect others | M | P0 |

**Dependencies:** STREAM-01

### TASK-01: Task Lifecycle Management

| Feature | Why Expected | Complexity | Priority |
|---------|--------------|------------|----------|
| **Create extraction task** (assign stream + rule) | Core action | S | P0 |
| **Start/Pause/Resume/Stop task** | Users must control extraction without deleting config | M | P0 |
| **Task status** (Running/Paused/Error/Completed/Idle) | Transparency into what's happening | S | P0 |
| **Task deletion** (with confirmation) | Clean up obsolete tasks | S | P1 |
| **Bulk operations** (start/stop/pause multiple tasks) | 1000+ tasks need batch control | M | P1 |
| **Task restart** (with configurable reset behavior) | Recover from stuck state | M | P1 |

**Dependencies:** STREAM-01, RULE-01

### RULE-01: Time-Interval Extraction Rules

| Feature | Why Expected | Complexity | Priority |
|---------|--------------|------------|----------|
| **Fixed interval** (e.g., every 1s, 5s, 30s) | Most common use case for frame extraction | S | P0 |
| **FPS-based** (e.g., 1fps, 5fps, 0.5fps) | Familiar unit for video professionals | S | P0 |
| **Per-stream configurable rate** | Different streams need different rates | S | P0 |
| **Min/max duration between frames** (guard rails) | Prevent accidental over-extraction at high rates | S | P1 |

**Dependencies:** None (rule system core)

### KAFKA-01: Kafka Integration - Core

| Feature | Why Expected | Complexity | Priority |
|---------|--------------|------------|----------|
| **Configurable Kafka brokers** (bootstrap servers) | Must connect to user's existing Kafka | S | P0 |
| **Configurable topic per task or per stream** | Routing flexibility for downstream consumers | S | P0 |
| **Frame image as message payload** (JPEG bytes) | Core delivery mechanism | M | P0 |
| **Metadata in message headers or key** (stream ID, timestamp, frame number) | Downstream needs context to use frames | M | P0 |
| **Configurable message format** (binary + headers vs structured envelope) | Different consumers have different expectations | M | P1 |
| **At-least-once delivery semantics** | No dropped frames (per PROJECT.md requirement) | M | P0 |

**Dependencies:** TASK-01

### OPS-01: Monitoring & Observability - Essentials

| Feature | Why Expected | Complexity | Priority |
|---------|--------------|------------|----------|
| **Prometheus metrics endpoint** | Standard Kubernetes observability | M | P0 |
| **Health check endpoint** (liveness + readiness) | Kubernetes pod health management | S | P0 |
| **Structured JSON logging** | Log aggregation (Loki, ELK) | S | P0 |
| **Stream-level metrics** (frames extracted, errors, lag) | Per-stream operational visibility | M | P0 |
| **Resource usage per task** (CPU, memory) | Capacity planning and troubleshooting | M | P1 |

**Dependencies:** None (infrastructure concern)

### UI-01: Management Dashboard - Essentials

| Feature | Why Expected | Complexity | Priority |
|---------|--------------|------------|----------|
| **Stream list** (with status, name, type, task count) | Primary view for operators | M | P0 |
| **Task list** (with stream, rule, status, metrics) | Operational control center | M | P0 |
| **Create stream form** | Basic data entry | M | P0 |
| **Create task form** (select stream + configure rule) | Basic workflow | M | P0 |
| **Task detail page** (status, metrics, recent activity) | Drill-down troubleshooting | M | P0 |
| **Basic dashboard** (total streams, tasks, health summary) | At-a-glance system status | M | P1 |

**Dependencies:** All API endpoints

### API-01: RESTful API - Essentials

| Feature | Why Expected | Complexity | Priority |
|---------|--------------|------------|----------|
| **Stream CRUD** | Programmatic stream management | S | P0 |
| **Task CRUD** | Programmatic task management | S | P0 |
| **Rule CRUD** | Programmatic rule management | S | P0 |
| **Status queries** (stream health, task state) | Integration use cases | S | P0 |
| **API authentication** (API key or JWT) | Security baseline | M | P0 |
| **OpenAPI/Swagger documentation** | Developer experience | M | P1 |

**Dependencies:** Core domain logic

### PLAT-01: Kubernetes Deployment

| Feature | Why Expected | Complexity | Priority |
|---------|--------------|------------|----------|
| **Helm chart** | Standard Kubernetes deployment mechanism | M | P0 |
| **Docker images** (multi-arch if possible) | Container distribution | S | P0 |
| **Configurable resource limits/requests** | Predictable resource allocation | S | P0 |
| **ConfigMap for stream/task configuration** | Kubernetes-native config management | M | P1 |
| **Horizontal Pod Autoscaler support** | Scale-out for more streams | M | P1 |

**Dependencies:** None (deployment concern)

---

## Differentiators

Features that set GetFrame apart. These create competitive advantage in the niche.

### DIFF-01: CPU-Optimized Scene Change Detection

| Feature | Why Differentiating | Complexity | Priority |
|---------|--------------------|------------|----------|
| **FFmpeg `scdet` filter integration** (threshold 8-14 range) | Leverage FFmpeg's built-in scene detection rather than custom CPU-hungry implementations | M | P0 |
| **Configurable scene-change threshold** (per-stream sensitivity) | Different content needs different thresholds (surveillance vs. broadcast) | S | P1 |
| **Adaptive content detection** (rolling average approach like PySceneDetect) | Reduces false positives on camera motion/fast pans | M | P2 |
| **Composite rules** (time interval + scene change) | "Extract every 5s OR on scene change, whichever comes first" | M | P1 |
| **Scene-change only mode** | Only emit frames when visual content actually changes (saves bandwidth) | M | P1 |

**Why it's differentiating:** No existing platform combines configurable scene detection with Kafka output at scale. Most scene detectors are single-video tools. The CPU constraint makes lightweight histogram-based detection (not deep learning) the right choice — and GetFrame can own this niche.

**Dependencies:** RULE-01, TASK-01

### DIFF-02: Rich Rule Engine

| Feature | Why Differentiating | Complexity | Priority |
|---------|--------------------|------------|----------|
| **Scheduled extraction** (cron-like patterns) | "Extract every 5 minutes only between 9AM-5PM" — powerful for surveillance | M | P1 |
| **Day/time windows** (extract only during business hours, or only at night) | Reduce storage/bandwidth when nothing happens | M | P1 |
| **Rate limiting** (max frames per minute/hour) | Protection against accidental overload | S | P1 |
| **Conditional rules** (if stream is healthy, extract at rate X; if degraded, reduce rate) | Adaptive extraction based on stream quality | L | P2 |
| **Rule templates** (pre-built configurations for common use cases) | Speed up onboarding | S | P2 |
| **Dry-run mode** (evaluate what a rule would extract without actually sending to Kafka) | Rule validation before production | M | P2 |

**Why it's differentiating:** This is the "programmable extraction" layer. Competitors (surveillance NVRs) just record everything. GetFrame lets users specify *exactly* which frames to keep, radically reducing downstream Kafka storage and processing costs.

**Dependencies:** RULE-01

### DIFF-03: Kafka-Native Architecture

| Feature | Why Differentiating | Complexity | Priority |
|---------|--------------------|------------|----------|
| **Kafka Schema Registry integration** (Avro or Protobuf) | Enables schema evolution for downstream consumers | M | P1 |
| **Configurable partition key** (by stream ID, task ID, custom) | Downstream consumer partitioning flexibility | S | P1 |
| **Batch delivery** (batch frames into single message for efficiency) | Higher throughput at the cost of slight latency increase | M | P2 |
| **Delivery guarantees** (configurable: at-least-once, exactly-once) | Different use cases need different guarantees | L | P2 |
| **Kafka producer metrics** (delivery latency, batch size, error rate) | Deep visibility into Kafka pipeline health | M | P1 |
| **Dead letter queue** (failed messages routing) | Don't lose frames on Kafka write failures | M | P2 |

**Why it's differentiating:** GetFrame is built *for* Kafka users. Most video platforms are designed for storage or display, not event streaming. The Kafka-native design means GetFrame fits naturally into event-driven architectures.

**Dependencies:** KAFKA-01

### DIFF-04: Horizontal Scalability on Kubernetes

| Feature | Why Differentiating | Complexity | Priority |
|---------|--------------------|------------|----------|
| **Task distribution across nodes** (automatic stream-to-worker assignment) | Linear scaling with node count | L | P0 |
| **Stream-level sharding** (consistent hashing by stream ID) | Predictable resource allocation | M | P0 |
| **Graceful pod shutdown** (drain streams before terminating) | Zero-downtime operations during rolling updates | M | P0 |
| **Resource-aware scheduling** (don't overload a node with too many CPU-heavy streams) | Predictable performance at scale | L | P1 |
| **Task migration on node failure** (auto-restart tasks on healthy nodes) | Resilience to infrastructure failures | L | P1 |

**Why it's differentiating:** 200+ streams per node, 1000+ per cluster — these numbers require deliberate horizontal scaling design. Surveillance NVRs typically scale vertically (bigger server). GetFrame's Kubernetes-native approach enables elastic scaling.

**Dependencies:** PLAT-01, TASK-01

### DIFF-05: Stream Health Intelligence

| Feature | Why Differentiating | Complexity | Priority |
|---------|--------------------|------------|----------|
| **Stream health scoring** (composite score: frames received vs expected, reconnect rate, latency) | Proactive detection of degrading streams | M | P1 |
| **Frame rate tracking** (actual vs expected FPS for each stream) | Detect when streams are dropping frames | M | P1 |
| **Connection latency tracking** (time to establish stream) | Network path health indicator | S | P1 |
| **Stream quality metrics** (resolution drops, codec changes) | Detect upstream quality degradation | M | P2 |
| **Anomaly detection** (sudden change in frame rate, scene complexity) | Early warning for infrastructure issues | L | P2 |
| **Predictive reconnect** (re-establish connection proactively on latency degradation) | Prevent failures before they happen | L | P3 |

**Why it's differentiating:** At 1000+ streams, manual health monitoring is impossible. GetFrame's intelligence layer automates detection of problematic streams. This is a force-multiplier for operations teams.

**Dependencies:** OPS-01

### DIFF-06: Rich Management Web UI

| Feature | Why Differentiating | Complexity | Priority |
|---------|--------------------|------------|----------|
| **Real-time stream status updates** (WebSocket push) | Live view without page refresh | M | P1 |
| **Per-stream frame preview** (last extracted frame in browser) | Instant visual verification of extraction quality | L | P2 |
| **Metrics dashboards** (frames extracted per minute, per stream, error rates) | Visual operations | L | P1 |
| **Task configuration comparison** (side-by-side rule comparison) | Manage similar tasks efficiently | M | P2 |
| **Bulk task editor** (edit rules for multiple streams at once) | 1000+ streams need batch operations | L | P2 |
| **Activity log viewer** (timestamped stream events: connected, disconnected, extracted, errored) | Debugging and audit trail | M | P1 |

**Why it's differentiating:** The UI is the face of the product. Most frame extraction tools are CLI scripts. A first-class management UI that handles 1000+ streams without slowdown is a significant differentiator for operations teams.

**Dependencies:** UI-01, API-01

### DIFF-07: Multi-Tenancy

| Feature | Why Differentiating | Complexity | Priority |
|---------|--------------------|------------|----------|
| **Teams/projects isolation** | Multiple teams sharing one cluster | L | P2 |
| **RBAC** (admin, operator, viewer roles) | Access control for different operational responsibilities | L | P2 |
| **Per-tenant resource quotas** (max streams, tasks, extraction rate) | Fair resource sharing | L | P3 |
| **Per-tenant Kafka configuration** (different Kafka clusters per tenant) | Tenant-level data isolation | L | P3 |

**Why it's differentiating:** Multi-tenancy expands the addressable market from "one team's infrastructure" to "platform team serves multiple internal customers." Early-stage this is P2 — premature optimization. But the architecture should not preclude it.

**Dependencies:** All of the above

---

## Anti-Features

Things to explicitly NOT build (distractions, over-engineering, or scope violations).

### ANTI-01: Video Transcoding / Re-encoding

| Aspect | Detail |
|--------|--------|
| **What it is** | Converting video from one codec/format to another (e.g., H.264 → H.265, 1080p → 720p) |
| **Why avoid** | Explicitly out of scope per PROJECT.md. Transcoding is CPU-intensive and would cannibalize resources needed for frame extraction. Kills the "200+ streams per node" target. |
| **What to do instead** | Frame extraction decodes only keyframes or selected frames — far cheaper than full transcoding. If users need transcoded streams, that's a separate service. |

### ANTI-02: AI/ML Visual Analysis

| Aspect | Detail |
|--------|--------|
| **What it is** | Object detection, face recognition, license plate reading, text extraction from frames |
| **Why avoid** | Explicitly out of scope per PROJECT.md. This is a downstream consumer responsibility. Building AI analysis into GetFrame would dramatically increase complexity, GPU dependency (contradicting CPU-only constraint), and scope drift. |
| **What to do instead** | GetFrame delivers frames to Kafka. Downstream consumers (Python/Go services, Spark, Flink) do the AI analysis. GetFrame stays focused on reliable frame delivery. |

### ANTI-03: Video Storage / Archival

| Aspect | Detail |
|--------|--------|
| **What it is** | Long-term storage of video streams, recording to disk/S3, playback of historical video |
| **Why avoid** | Explicitly out of scope per PROJECT.md. This is what NVRs like ZoneMinder/Kerberos.io do. It's a different product. |
| **What to do instead** | GetFrame stores only extracted frames (transiently in Kafka). Video data is ephemeral — decoded and discarded after frame extraction. Users needing archival should use a separate storage solution. |

### ANTI-04: GPU Hardware Acceleration

| Aspect | Detail |
|--------|--------|
| **What it is** | Using NVIDIA CUDA, Intel QuickSync, VAAPI, etc. for decoding or processing |
| **Why avoid** | Explicit CPU-only constraint per PROJECT.md. GPU acceleration introduces vendor lock-in, complicates Kubernetes scheduling (GPU node pools), and increases cost. |
| **What to do instead** | Optimize CPU decoding: SIMD-optimized FFmpeg builds, memory pool reuse, zero-copy frame extraction, thread-per-stream vs thread-pool model analysis. These optimizations benefit everyone, not just GPU-equipped clusters. |

### ANTI-05: Comprehensive Data Pipeline UI

| Aspect | Detail |
|--------|--------|
| **What it is** | Building a full Kafka consumer UI, frame viewer/annotator, or analytics dashboard within GetFrame |
| **Why avoid** | Scope creep. GetFrame's UI should manage extraction configuration and health monitoring — not replace Kafka tooling (AKHQ, Kafka UI, Redpanda Console) or analytics platforms (Grafana). |
| **What to do instead** | GetFrame exposes Prometheus metrics (Grafana dashboards) and clear Kafka message schemas. Let existing tools handle the "what happens after extraction" part. |

### ANTI-06: Frame Post-Processing (Resizing, Filtering, Watermarking)

| Aspect | Detail |
|--------|--------|
| **What it is** | Applying image transformations to extracted frames before Kafka delivery |
| **Why avoid** | Every transformation adds CPU cost per frame. At 200+ streams × potentially 30+ fps, this multiplies compute requirements. It also adds latency to the extraction-to-delivery pipeline. If users need different frame formats, they can process from Kafka. |
| **What to do instead** | Deliver raw JPEG frames at native resolution. Downstream consumers resize/filter as needed. If demand emerges, consider optional lightweight post-processing in a future phase, but keep it OFF by default. |

### ANTI-07: Custom Kafka Connect / Sink Development

| Aspect | Detail |
|--------|--------|
| **What it is** | Building GetFrame-specific Kafka Connect connectors for popular storage destinations (S3, HDFS, Elasticsearch) |
| **Why avoid** | Kafka Connect already has mature connectors for these destinations. Building custom sinks duplicates ecosystem effort and increases maintenance burden. |
| **What to do instead** | Use standard Kafka Connect sinks. GetFrame produces to Kafka topics with clear schema — any Kafka Connect JDBC/S3/Elasticsearch connector can consume from those topics. |

---

## Feature Dependencies

```
STREAM-01 (Source Mgmt)
  ├── STREAM-02 (Connection reliability)
  ├── TASK-01 (Task lifecycle)
  │     ├── RULE-01 (Time-interval rules)
  │     │     └── DIFF-02 (Rich rule engine)
  │     │           └── DIFF-01 (Scene detection)
  │     └── KAFKA-01 (Kafka core)
  │           └── DIFF-03 (Kafka-native)
  ├── OPS-01 (Monitoring essentials)
  │     └── DIFF-05 (Stream health)
  └── UI-01 (Dashboard essentials)
        └── DIFF-06 (Rich UI)
              └── DIFF-07 (Multi-tenancy)

API-01 (REST API) ← feeds → UI-01
PLAT-01 (K8s deploy) ← enables → DIFF-04 (Horizontal scaling)
```

---

## Complexity Reference

| Label | Meaning | Example |
|-------|---------|---------|
| **S** (Small) | 1-3 days, well-understood problem | CRUD endpoints, form UI, Prometheus counter |
| **M** (Medium) | 1-2 weeks, known patterns but integration work | Scene detection via FFmpeg filter, Kafka producer with retries |
| **L** (Large) | 2-4 weeks, significant design + implementation | Horizontal scaling with stream migration, multi-tenancy RBAC |
| **XL** (Extra large) | 1-3 months, cross-cutting or novel | Custom rule engine DSL, predictive reconnect ML |

---

## MVP Recommendation

### Phase 1 (Core Extraction Engine) — P0 features
1. **STREAM-01**: Source management (CRUD + health status)
2. **STREAM-02**: Reconnection + graceful failure
3. **TASK-01**: Task lifecycle (create, start, stop, status)
4. **RULE-01**: Time-interval extraction (fixed rate per stream)
5. **KAFKA-01**: Basic Kafka producer (JPEG + metadata headers)
6. **OPS-01**: Prometheus metrics + health check + structured logging
7. **API-01**: REST API for streams, tasks, rules, status
8. **PLAT-01**: Helm chart + Docker images
9. **UI-01**: Stream list, task list, create forms, dashboard

### Phase 2 (Intelligence & Scale) — P1 features
10. **DIFF-01**: Scene change detection with FFmpeg scdet
11. **DIFF-02**: Scheduled/cron rules, rate limiting, composite rules
12. **DIFF-04**: Task distribution + graceful shutdown
13. **DIFF-03**: Schema Registry integration, enhanced Kafka metrics
14. **DIFF-05**: Stream health scoring, frame rate tracking
15. **DIFF-06**: Real-time WebSocket updates, activity logs, metrics dashboards

### Phase 3 (Enterprise) — P2 features
16. **DIFF-07**: Multi-tenancy, RBAC, project isolation
17. Rich rule engine (templates, conditional rules, dry-run)
18. Kafka batching, dead letter queues, exactly-once delivery
19. Advanced stream health (anomaly detection)
20. Batch import, bulk operations

### Never Build
- Video transcoding
- AI visual analysis
- Video storage/archival
- GPU acceleration
- Frame post-processing
- Custom Kafka Connect sinks

---

## Sources

- FFmpeg `scdet` filter documentation: https://ayosec.github.io/ffmpeg-filters-docs/8.0/Filters/Video/scdet.html (MEDIUM confidence)
- PySceneDetect detection algorithms: https://www.scenedetect.com/api (MEDIUM confidence)
- ZoneMinder open-source VMS: https://zoneminder.com/ (HIGH confidence)
- Kerberos.io architecture: https://github.com/kerberos-io (HIGH confidence)
- AWS Kinesis Video Streams + Fargate pattern: https://docs.aws.amazon.com/prescriptive-guidance/latest/patterns/build-a-video-processing-pipeline-by-using-amazon-kinesis-video-streams-and-aws-fargate.html (HIGH confidence)
- Pipeless CV framework + Kafka: https://python.plainenglish.io/handling-computer-vision-events-in-real-time-with-kafka-and-pipeless-61c1b45c2791 (MEDIUM confidence)
- RTSP reconnection strategies: https://github.com/NikhilBudaniya/rtsp-stream (LOW confidence — small project, but pattern is standard)
- Kafka serialization comparison (Avro vs Protobuf): https://conduktor.io/glossary/avro-vs-protobuf-vs-json-schema (HIGH confidence)
- Microsoft RulesEngine: https://microsoft.github.io/RulesEngine/ (MEDIUM confidence — reference for rule engine patterns)
- Free VMS Software Directory (IPVM): https://ipvm.com/reports/free-vms-software-directory (MEDIUM confidence — 2021 data, but landscape hasn't changed significantly)
