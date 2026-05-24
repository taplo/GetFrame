# Requirements: GetFrame

**Defined:** 2026-05-24
**Core Value:** In CPU-only environments, reliably process hundreds of concurrent video streams with minimal resources and deliver specified frames to Kafka.

## v1 Requirements

### Source Management — STREAM

- [ ] **STREAM-01**: User can add video sources via URL (RTSP/RTMP/HLS/file) with auto-detected stream type
- [ ] **STREAM-02**: User can edit/delete existing stream configurations
- [ ] **STREAM-03**: System validates stream URL reachability before saving configuration (test connection)
- [ ] **STREAM-04**: System displays per-stream health status (Online/Offline/Error) in real-time
- [ ] **STREAM-05**: User can add metadata (name, tags, description) to each stream
- [ ] **STREAM-06**: User can organize streams by tags for filtering and bulk operations
- [ ] **STREAM-07**: System automatically reconnects disconnected streams with exponential backoff
- [ ] **STREAM-08**: System gracefully handles stream timeout and resource cleanup on extended failure

### Extraction Rules — RULE

- [ ] **RULE-01**: User can configure fixed-interval extraction (extract every N seconds) per stream
- [ ] **RULE-02**: User can configure FPS-based extraction (extract at N frames per second) per stream
- [ ] **RULE-03**: System supports per-stream configurable extraction rate
- [ ] **RULE-04**: System supports scene-change detection extraction (via FFmpeg `scdet` filter) with configurable threshold
- [ ] **RULE-05**: System supports composite rules combining interval + scene change (any/all operators)
- [ ] **RULE-06**: System enforces rate limiting (max frames per minute/hour) per stream to prevent overload

### Frame Output — FRAME

- [ ] **FRAME-01**: Extracted frames are stored in object storage (MinIO / S3-compatible)
- [ ] **FRAME-02**: Frame images are encoded as JPEG with configurable quality
- [ ] **FRAME-03**: Object storage path/key is deterministic (stream_id / timestamp / frame_number format)
- [ ] **FRAME-04**: Frame storage supports configurable retention policy (auto-cleanup after N days)

### Kafka Integration — KAFKA

- [ ] **KAFKA-01**: System pushes frame metadata to configurable Kafka broker(s)
- [ ] **KAFKA-02**: Kafka message contains: stream_id, timestamp, frame_number, rule_trigger, and object storage URL/path
- [ ] **KAFKA-03**: Kafka message includes metadata headers (stream ID, timestamp, source type)
- [ ] **KAFKA-04**: System supports at-least-once delivery semantics
- [ ] **KAFKA-05**: System integrates with Schema Registry using Avro or Protobuf for structured metadata
- [ ] **KAFKA-06**: Kafka topic and partition key are configurable per stream or per task

### REST API — API

- [ ] **API-01**: Full CRUD API for stream sources
- [ ] **API-02**: Full CRUD API for extraction tasks
- [ ] **API-03**: Full CRUD API for extraction rules
- [ ] **API-04**: Status query endpoints (stream health, task state, system health)
- [ ] **API-05**: OpenAPI/Swagger documentation

### Web UI — UI

- [ ] **UI-01**: Stream list view with status indicators and metadata
- [ ] **UI-02**: Task list view with stream, rule, and status display
- [ ] **UI-03**: Create stream form (URL, type, metadata, tags)
- [ ] **UI-04**: Create task form (select stream + configure rules)
- [ ] **UI-05**: Task detail page with status, metrics, recent activity
- [ ] **UI-06**: Dashboard with health summary (total streams, tasks, health)
- [ ] **UI-07**: Last extracted frame preview per stream (thumbnail view)

### Observability — OBS

- [ ] **OBS-01**: Prometheus metrics endpoint (frames extracted, streams active, errors, Kafka lag)
- [ ] **OBS-02**: Liveness and readiness health check endpoints for K8s probes
- [ ] **OBS-03**: Structured JSON logging via tracing crate
- [ ] **OBS-04**: Per-stream detailed metrics (FPS, decode latency, Kafka send latency)
- [ ] **OBS-05**: Pre-built Grafana dashboards for operational monitoring

### Deployment — DEPLOY

- [ ] **DEPLOY-01**: Docker multi-stage build (Rust binary + static FFmpeg on distroless)
- [ ] **DEPLOY-02**: Helm chart for Kubernetes deployment (Deployments, Services, ConfigMaps)
- [ ] **DEPLOY-03**: Configurable resource limits/requests per component
- [ ] **DEPLOY-04**: KEDA ScaledObject for worker auto-scaling based on Kafka lag + CPU metrics

### Stream Worker — WORKER

- [ ] **WORKER-01**: Single-node supports 200+ concurrent 1080P H.264 video streams
- [ ] **WORKER-02**: Cluster supports 1000+ concurrent streams with horizontal scaling
- [ ] **WORKER-03**: Workers are stateless and can be scaled up/down without data loss
- [ ] **WORKER-04**: Graceful shutdown drains streams before pod termination

## v2 Requirements

### Enhanced Features

- **RULE-07**: Scheduled extraction with cron-like patterns (e.g., extract only during business hours)
- **RULE-08**: Conditional rules (if stream healthy, extract at rate X; if degraded, reduce rate)
- **RULE-09**: Rule templates for common use cases
- **RULE-10**: Dry-run mode to evaluate rule output without actually extracting
- **FRAME-05**: Configurable image format (WebP, PNG) as alternative to JPEG
- **FRAME-06**: Optional frame downscaling before storage (reduce size at cost of resolution)
- **KAFKA-07**: Dead letter queue for failed message delivery
- **KAFKA-08**: Batch delivery option (multiple frames in single message)
- **KAFKA-09**: Exactly-once delivery semantics option
- **UI-08**: Real-time WebSocket updates for stream/task status
- **UI-09**: Activity log viewer with timeline
- **UI-10**: Bulk task editor for batch rule updates across streams
- **API-06**: API authentication (JWT / API key)
- **OBS-06**: Stream health scoring composite metric
- **OBS-07**: OpenTelemetry distributed tracing
- **DEPLOY-05**: Stream-level consistent hashing for predictable resource allocation

### Multi-Tenancy (P2)

- **MT-01**: Teams/projects isolation
- **MT-02**: RBAC (admin, operator, viewer roles)
- **MT-03**: Per-tenant resource quotas
- **MT-04**: Per-tenant Kafka configuration

## Out of Scope

| Feature | Reason |
|---------|--------|
| GPU hardware acceleration | Explicitly CPU-only per project requirements |
| AI/ML visual analysis (object detection, face recognition) | Downstream Kafka consumer responsibility |
| Video transcoding / re-encoding | Not in scope; would cannibalize CPU for frame extraction |
| Video storage / archival | GetFrame is extraction middleware, not an NVR |
| Custom Kafka Connect sink development | Use standard Kafka Connect connectors |
| Frame post-processing (resizing, filtering, watermarking) | Except for basic JPEG quality config |
| Comprehensive data pipeline UI / analytics | Grafana + existing Kafka tooling covers this |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| STREAM-01 | Phase 1 | Pending |
| STREAM-02 | Phase 2 | Pending |
| STREAM-03 | Phase 2 | Pending |
| STREAM-04 | Phase 2 | Pending |
| STREAM-05 | Phase 2 | Pending |
| STREAM-06 | Phase 2 | Pending |
| STREAM-07 | Phase 2 | Pending |
| STREAM-08 | Phase 2 | Pending |
| RULE-01 | Phase 1 | Pending |
| RULE-02 | Phase 3 | Pending |
| RULE-03 | Phase 3 | Pending |
| RULE-04 | Phase 4 | Pending |
| RULE-05 | Phase 4 | Pending |
| RULE-06 | Phase 3 | Pending |
| FRAME-01 | Phase 1 | Pending |
| FRAME-02 | Phase 1 | Pending |
| FRAME-03 | Phase 1 | Pending |
| FRAME-04 | Phase 5 | Pending |
| KAFKA-01 | Phase 1 | Pending |
| KAFKA-02 | Phase 1 | Pending |
| KAFKA-03 | Phase 1 | Pending |
| KAFKA-04 | Phase 5 | Pending |
| KAFKA-05 | Phase 5 | Pending |
| KAFKA-06 | Phase 5 | Pending |
| API-01 | Phase 2 | Pending |
| API-02 | Phase 6 | Pending |
| API-03 | Phase 3 | Pending |
| API-04 | Phase 2 | Pending |
| API-05 | Phase 6 | Pending |
| UI-01 | Phase 7 | Pending |
| UI-02 | Phase 7 | Pending |
| UI-03 | Phase 7 | Pending |
| UI-04 | Phase 7 | Pending |
| UI-05 | Phase 8 | Pending |
| UI-06 | Phase 8 | Pending |
| UI-07 | Phase 8 | Pending |
| OBS-01 | Phase 2 | Pending |
| OBS-02 | Phase 1 | Pending |
| OBS-03 | Phase 1 | Pending |
| OBS-04 | Phase 2 | Pending |
| OBS-05 | Phase 10 | Pending |
| DEPLOY-01 | Phase 1 | Pending |
| DEPLOY-02 | Phase 10 | Pending |
| DEPLOY-03 | Phase 10 | Pending |
| DEPLOY-04 | Phase 10 | Pending |
| WORKER-01 | Phase 9 | Pending |
| WORKER-02 | Phase 9 | Pending |
| WORKER-03 | Phase 9 | Pending |
| WORKER-04 | Phase 9 | Pending |

**Coverage:**
- v1 requirements: 49 total
- Mapped to phases: 49
- Unmapped: 0 ✓

**Per-Phase Requirement Counts:**
- Phase 1 (Core Pipeline): 11 requirements
- Phase 2 (Multi-Stream Management): 11 requirements
- Phase 3 (Per-Stream Rule Configuration): 4 requirements
- Phase 4 (Scene Detection & Composite Rules): 2 requirements
- Phase 5 (Kafka Production Readiness): 4 requirements
- Phase 6 (Task Management API): 2 requirements
- Phase 7 (Web UI — Stream & Task Management): 4 requirements
- Phase 8 (Web UI — Dashboard & Monitoring): 3 requirements
- Phase 9 (Worker Scaling): 4 requirements
- Phase 10 (Production Deployment): 4 requirements

---
*Requirements defined: 2026-05-24*
*Last updated: 2026-05-24 after initial definition*
