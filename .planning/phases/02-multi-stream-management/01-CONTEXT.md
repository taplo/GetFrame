# Phase 2: Multi-Stream Management & Monitoring — Context

## Current Architecture (Phase 1)

Phase 1 implements a single-stream pipeline:
- Single OS thread runs the entire decode→extract→encode pipeline
- One `Pipeline` struct wraps one decode thread
- `main.rs` creates exactly one pipeline and one async consumer
- Stream config is read from YAML file at startup, immutable

## Phase 2 Goals

1. **Multi-stream runtime**: N streams running in parallel, each in its own OS thread
2. **Stream CRUD API**: REST endpoints to add/edit/delete streams at runtime without restart
3. **Health status**: Per-stream Online/Offline/Error tracking with timestamps
4. **Auto-reconnection**: Exponential backoff on stream failure
5. **Prometheus metrics**: Expose stream and system metrics via `/metrics`

## Requirements

| ID | Description | Priority |
|----|-------------|----------|
| STREAM-02 | User can edit/delete existing stream configurations | High |
| STREAM-03 | System validates stream URL reachability before saving | High |
| STREAM-04 | System displays per-stream health status in real-time | High |
| STREAM-05 | User can add metadata (name, tags, description) to each stream | Medium |
| STREAM-06 | User can organize streams by tags for filtering | Low |
| STREAM-07 | System auto-reconnects with exponential backoff | High |
| STREAM-08 | Graceful resource cleanup on extended failure | High |
| API-01 | Full CRUD API for stream sources | High |
| API-04 | Status query endpoints | High |
| OBS-01 | Prometheus metrics endpoint | High |
| OBS-04 | Per-stream metrics (FPS, decode latency) | Medium |

## Key Architectural Decisions

### AD-01: StreamManager
Introduce a `StreamManager` component that owns all stream pipelines. It provides:
- `add_stream(config) -> StreamId` — spawn new pipeline thread
- `remove_stream(id)` — cancel pipeline, join thread, clean up
- `get_status(id) -> StreamStatus` — query health
- `list_streams() -> Vec<StreamInfo>` — list all with status

### AD-02: Persistence
Phase 2 stores stream configs **in memory only** (HashMap). Phase 6+ will add PostgreSQL persistence. Configs can be pre-loaded from the YAML file at startup.

### AD-03: API Router
REST API lives under `/api/v1/streams`. Axum router is composed in `main.rs` alongside the health routes.

### AD-04: Metrics
Use `metrics` + `metrics-exporter-prometheus` crates for Prometheus output. Track counters and gauges per stream with `stream_id` labels.

### AD-05: Graceful Reconnection
When a pipeline thread exits (error or EOF), the StreamManager detects the exit and spawns a replacement after backoff delay. The backoff sequence: 1s → 2s → 4s → 8s → 16s → 30s cap.

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Pipeline thread panics bring down process | High | Use `catch_unwind` or JoinHandle error handling |
| Channel resource exhaustion with many streams | Medium | Bounded channels per stream (already in place) |
| Race condition on stream add/remove | Medium | Arc<RwLock<HashMap>> for stream registry |
