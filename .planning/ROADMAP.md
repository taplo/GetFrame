# Roadmap: GetFrame

## Overview

Build a high-performance, CPU-only video frame extraction platform in Rust that ingests 200–1000+ concurrent 1080p H.264 streams (RTSP/RTMP/HLS/file), decodes via FFmpeg libavcodec as a library, evaluates configurable extraction rules, stores JPEG frames in MinIO/S3, and pushes structured metadata to Kafka — all Kubernetes-native with KEDA auto-scaling. The roadmap progresses from a single-stream pipeline proving the architecture, through multi-stream management, intelligent rules, production Kafka integration, management API, web UI, horizontal scaling, and finally production deployment with Helm, KEDA, and Grafana.

## Phases

- [ ] **Phase 1: Core Pipeline — Single Stream Foundation** - End-to-end decode → extract → MinIO/S3 → Kafka pipeline proving the architecture
- [ ] **Phase 2: Multi-Stream Management & Monitoring** - Stream CRUD, health status, auto-reconnection, Prometheus metrics
- [ ] **Phase 3: Per-Stream Rule Configuration** - FPS-based extraction, per-stream rates, rate limiting, rule CRUD API
- [ ] **Phase 4: Scene Detection & Composite Rules** - FFmpeg scdet filter integration, composite interval+scene rules
- [ ] **Phase 5: Kafka Production Readiness** - Schema Registry, at-least-once delivery, configurable topics, retention policy
- [ ] **Phase 6: Task Management API & Documentation** - Complete task lifecycle CRUD, OpenAPI/Swagger docs
- [ ] **Phase 7: Web UI — Stream & Task Management** - Stream list, task list, creation forms
- [ ] **Phase 8: Web UI — Dashboard & Monitoring** - Dashboard, task detail, frame preview
- [ ] **Phase 9: Worker Scaling — 200+/1000+ Streams** - Horizontal scaling, stateless workers, graceful shutdown
- [ ] **Phase 10: Production Deployment — K8s, Helm, KEDA, Grafana** - Helm chart, KEDA auto-scaling, Grafana dashboards

## Phase Details

### Phase 1: Core Pipeline — Single Stream Foundation
**Goal**: Single video stream can be ingested, decoded, frames extracted by simple interval rule, stored to MinIO, and metadata pushed to Kafka.
**Mode**: mvp
**Depends on**: Nothing (foundation)
**Requirements**: STREAM-01, RULE-01, FRAME-01, FRAME-02, FRAME-03, KAFKA-01, KAFKA-02, KAFKA-03, OBS-02, OBS-03, DEPLOY-01
**Success Criteria** (what must be TRUE):
  1. User can configure a video source (RTSP/RTMP/HLS/file) via config file → system starts decoding immediately
  2. System extracts JPEG frames at configured interval (e.g., every 5 seconds) and stores them in MinIO/S3 with deterministic `stream_id/timestamp/frame_number` path
  3. Frame metadata (stream_id, timestamp, frame_number, rule_trigger, storage URL) is published to Kafka with proper metadata headers
  4. System exposes HTTP liveness/readiness endpoints for K8s health probes
  5. Structured JSON logs capture all pipeline operations (decode, extract, store, publish)
**Plans**: TBD

### Phase 2: Multi-Stream Management & Monitoring
**Goal**: User can manage multiple streams with full CRUD, real-time health monitoring, auto-reconnection, and Prometheus metrics.
**Mode**: mvp
**Depends on**: Phase 1
**Requirements**: STREAM-02, STREAM-03, STREAM-04, STREAM-05, STREAM-06, STREAM-07, STREAM-08, API-01, API-04, OBS-01, OBS-04
**Success Criteria** (what must be TRUE):
  1. User can add/edit/delete stream configurations via REST API → streams are created/torn down dynamically
  2. Stream URL is validated for reachability before saving (test connection on create)
  3. System displays per-stream health status (Online/Offline/Error) via status API with metadata (name, tags, description)
  4. Disconnected streams auto-reconnect with exponential backoff; extended failures trigger graceful resource cleanup
  5. Prometheus `/metrics` endpoint exposes `streams_active`, `frames_processed_total`, `errors_total`, per-stream decode latency and FPS
**Plans**: TBD

### Phase 3: Per-Stream Rule Configuration
**Goal**: User can configure extraction rules per stream with FPS, per-stream rates, rate limiting, and rule management via API.
**Mode**: mvp
**Depends on**: Phase 2
**Requirements**: RULE-02, RULE-03, RULE-06, API-03
**Success Criteria** (what must be TRUE):
  1. User can configure FPS-based extraction (e.g., extract at 0.5 fps = 1 frame every 2 seconds) per stream
  2. Each stream has independent extraction rate configuration
  3. System enforces rate limiting (max frames per minute / per hour) per stream, dropping excess frames
  4. User can create, read, update, delete rules via REST API
**Plans**: TBD

### Phase 4: Scene Detection & Composite Rules
**Goal**: System can detect scene changes via FFmpeg `scdet` filter and support composite rules combining interval + scene change triggers.
**Mode**: mvp
**Depends on**: Phase 3
**Requirements**: RULE-04, RULE-05
**Success Criteria** (what must be TRUE):
  1. User can enable scene-change detection with configurable threshold (0.0–1.0) per stream
  2. System extracts frame when scene change exceeds configured threshold (scdet filter)
  3. User can create composite rules combining interval extraction AND/OR scene change detection
  4. Composite rules evaluate correctly — any operator triggers on first match, all operator requires both triggers
**Plans**: TBD

### Phase 5: Kafka Production Readiness
**Goal**: Kafka integration is production-ready with Schema Registry, at-least-once delivery guarantees, configurable topics, and frame retention policy.
**Mode**: mvp
**Depends on**: Phase 1
**Requirements**: FRAME-04, KAFKA-04, KAFKA-05, KAFKA-06
**Success Criteria** (what must be TRUE):
  1. System supports at-least-once delivery semantics — frames are not lost on transient Kafka broker failures
  2. Kafka messages use Schema Registry with Avro or Protobuf schema for structured metadata
  3. Kafka topic name and partition key are configurable per stream or per task
  4. Object storage supports configurable retention policy — frames older than N days are automatically cleaned up
**Plans**: TBD

### Phase 6: Task Management API & Documentation
**Goal**: Complete REST API for extraction task lifecycle (create, start, pause, stop, delete) with OpenAPI/Swagger documentation.
**Mode**: mvp
**Depends on**: Phase 2, Phase 3
**Requirements**: API-02, API-05
**Success Criteria** (what must be TRUE):
  1. User can create, start, pause, stop, and delete extraction tasks via REST API
  2. User can query task status (running/paused/stopped/error) at any time
  3. All API endpoints are documented via OpenAPI 3.0 with interactive Swagger UI
  4. API responses follow consistent JSON format with proper HTTP status codes and error messages
**Plans**: TBD

### Phase 7: Web UI — Stream & Task Management
**Goal**: User can manage streams, tasks, and rules through a complete web interface.
**Mode**: mvp
**Depends on**: Phase 6
**Requirements**: UI-01, UI-02, UI-03, UI-04
**Success Criteria** (what must be TRUE):
  1. User can view a list of all streams with real-time status indicators (Online/Offline/Error) and metadata tags
  2. User can view a list of all extraction tasks with associated stream name, rule description, and current status
  3. User can create a new stream via form (URL, type auto-detection, metadata, tags)
  4. User can create a new extraction task via form (select stream + configure rule parameters)
**Plans**: TBD
**UI hint**: yes

### Phase 8: Web UI — Dashboard & Monitoring
**Goal**: User can monitor system health, view task details, and preview extracted frames from a dashboard.
**Mode**: mvp
**Depends on**: Phase 7
**Requirements**: UI-05, UI-06, UI-07
**Success Criteria** (what must be TRUE):
  1. User can open a task detail page showing current status, extraction metrics, and recent activity timeline
  2. Dashboard shows health summary — total streams (online/offline/error), active tasks, system health
  3. User can see the last extracted frame thumbnail preview per stream
  4. Dashboard displays auto-refreshing key metrics (frames extracted, error rate, Kafka delivery rate)
**Plans**: TBD
**UI hint**: yes

### Phase 9: Worker Scaling — 200+/1000+ Streams
**Goal**: System scales to 200+ concurrent 1080p H.264 streams per node and 1000+ across cluster with stateless workers and graceful shutdown.
**Mode**: mvp
**Depends on**: Phase 2, Phase 5
**Requirements**: WORKER-01, WORKER-02, WORKER-03, WORKER-04
**Success Criteria** (what must be TRUE):
  1. Single worker node stably processes 200+ concurrent 1080p H.264 streams at 1fps extraction rate
  2. Cluster of N workers handles 1000+ concurrent streams with horizontal scaling
  3. Workers are stateless — can be scaled up/down without frame loss or data corruption
  4. Pod termination gracefully drains active streams before shutdown (SIGTERM → drain → exit)
**Plans**: TBD

### Phase 10: Production Deployment — K8s, Helm, KEDA, Grafana
**Goal**: System runs in Kubernetes with Helm chart, KEDA auto-scaling, configurable resources, and Grafana dashboards.
**Mode**: mvp
**Depends on**: Phase 9
**Requirements**: DEPLOY-02, DEPLOY-03, DEPLOY-04, OBS-05
**Success Criteria** (what must be TRUE):
  1. Helm chart installs all components (worker, stream-manager, REST API) with a single `helm install` command
  2. CPU/memory resource limits and requests are configurable per component via Helm values
  3. KEDA ScaledObject auto-scales worker replicas based on Kafka consumer lag and CPU utilization
  4. Pre-built Grafana dashboard shows operational metrics: active streams, frames extracted, error rates, Kafka producer lag
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Core Pipeline | 0/0 | Not started | - |
| 2. Multi-Stream Management | 0/0 | Not started | - |
| 3. Per-Stream Rule Configuration | 0/0 | Not started | - |
| 4. Scene Detection & Composite Rules | 0/0 | Not started | - |
| 5. Kafka Production Readiness | 0/0 | Not started | - |
| 6. Task Management API | 0/0 | Not started | - |
| 7. Web UI — Stream & Task Management | 0/0 | Not started | - |
| 8. Web UI — Dashboard & Monitoring | 0/0 | Not started | - |
| 9. Worker Scaling | 0/0 | Not started | - |
| 10. Production Deployment | 0/0 | Not started | - |
