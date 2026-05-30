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
| STREAM-01 | Phase 1 | Done (auto-detect via URL prefix) |
| STREAM-02 | Phase 2 | Done (PUT/DELETE /api/v1/streams/{id}) |
| STREAM-03 | Phase 2 | Done (pre-save probe + POST /api/v1/streams/test-url) |
| STREAM-04 | Phase 2 | Done (per-stream health in API + UI indicators) |
| STREAM-05 | Phase 2 | Done (name, description, tags in config) |
| STREAM-06 | Phase 2 | Partial (tags stored/displayed, no server-side filter) |
| STREAM-07 | Phase 2 | Done (exponential backoff reconnection) |
| STREAM-08 | Phase 2 | Done (CancellationToken, resource cleanup) |
| RULE-01 | Phase 1 | Done (IntervalEvaluator) |
| RULE-02 | Phase 3 | Done (Fps -> Interval conversion) |
| RULE-03 | Phase 3 | Done (per-stream extract_interval_seconds + rules) |
| RULE-04 | Phase 4 | Done (scdet filter, configurable threshold) |
| RULE-05 | Phase 4 | Done (CompositeEvaluator, Any/All operators) |
| RULE-06 | Phase 3 | Done (RateLimitedEvaluator, token bucket) |
| FRAME-01 | Phase 1 | Done (StorageClient, aws-sdk-s3, MinIO) |
| FRAME-02 | Phase 1 | Done (JpegEncoder with quality) |
| FRAME-03 | Phase 1 | Done (stream_id/date/timestamp_key path) |
| FRAME-04 | Phase 5 | Done (RetentionCleaner, configurable days) |
| KAFKA-01 | Phase 1 | Done (KafkaProducer, rdkafka) |
| KAFKA-02 | Phase 1 | Done (FrameMetadata with all fields) |
| KAFKA-03 | Phase 1 | Done (OwnedHeaders) |
| KAFKA-04 | Phase 5 | Done (acks=all, idempotence, retries) |
| KAFKA-05 | Phase 5 | Done (SchemaRegistryClient, Avro) |
| KAFKA-06 | Phase 5 | Done (per-stream topic/partition config) |
| API-01 | Phase 2 | Done (5 endpoints for stream CRUD) |
| API-02 | Phase 6 | Done (9 endpoints, lifecycle state machine) |
| API-03 | Phase 3 | Done (5 endpoints for rule CRUD) |
| API-04 | Phase 2 | Done (/health, /ready, per-stream status) |
| API-05 | Phase 6 | Done (utoipa annotations, Swagger UI) |
| UI-01 | Phase 7 | Done (StreamTable with status indicators) |
| UI-02 | Phase 7 | Done (TaskTable with status badges) |
| UI-03 | Phase 7 | Done (StreamForm with create/edit) |
| UI-04 | Phase 7 | Done (TaskForm with stream/rules) |
| UI-05 | Phase 8 | Done (TaskDetail page) |
| UI-06 | Phase 8 | Done (Dashboard with StatCards) |
| UI-07 | Phase 8 | Done (FramePreview component) |
| OBS-01 | Phase 2 | Partial (Prometheus /metrics, no Kafka lag) |
| OBS-02 | Phase 1 | Done (/health + /ready endpoints) |
| OBS-03 | Phase 1 | Done (tracing-subscriber JSON logger) |
| OBS-04 | Phase 2 | Partial (per-stream health stats, no per-stream Prometheus labels) |
| OBS-05 | Phase 10 | Done (deploy/grafana/getframe-dashboard.json) |
| DEPLOY-01 | Phase 1 | Done (Docker multi-stage, docker-compose) |
| DEPLOY-02 | Phase 10 | Done (deploy/helm/getframe/) |
| DEPLOY-03 | Phase 10 | Done (Helm values: resources.requests/limits) |
| DEPLOY-04 | Phase 10 | Done (KEDA ScaledObject in Helm templates) |
| WORKER-01 | Phase 9 | Partial (architecture supports, not benchmarked) |
| WORKER-02 | Phase 9 | Partial (WorkerManager design, not validated) |
| WORKER-03 | Phase 9 | Done (stateless DB-claim based) |
| WORKER-04 | Phase 9 | Done (CancellationToken + SIGTERM) |

**Coverage:**
- v1 requirements: 49 total
- Fully implemented: 39
- Partially implemented: 5
- Not implemented: 0 ✓ (all mapped to at least partial)

**Per-Phase Requirement Counts:**
- Phase 1 (Core Pipeline): 11 requirements — All done
- Phase 2 (Multi-Stream Management): 11 requirements — 9 done, 2 partial (STREAM-06, OBS-04)
- Phase 3 (Per-Stream Rule Configuration): 4 requirements — All done
- Phase 4 (Scene Detection & Composite Rules): 2 requirements — All done
- Phase 5 (Kafka Production Readiness): 4 requirements — All done
- Phase 6 (Task Management API): 2 requirements — All done
- Phase 7 (Web UI — Stream & Task Management): 4 requirements — All done
- Phase 8 (Web UI — Dashboard & Monitoring): 3 requirements — All done
- Phase 9 (Worker Scaling): 4 requirements — 2 done, 2 partial (WORKER-01, WORKER-02)
- Phase 10 (Production Deployment): 4 requirements — All done

---
*Requirements defined: 2026-05-24*
*Last updated: 2026-05-24 after initial definition*
