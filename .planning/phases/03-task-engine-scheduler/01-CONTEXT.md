# Phase 3: Per-Stream Rule Configuration & Reconnection — Context

## Current State (Phase 2)

Phase 2 implemented multi-stream management with REST API and metrics:

- **StreamManager** owns N concurrent pipelines, each in its own OS thread
- **StreamRegistry** stores config + health per stream (in-memory HashMap)
- **REST API** at `/api/v1/streams` for CRUD + test connection
- **Metrics** at `/metrics` (Prometheus format)
- **Pipeline** uses a single hardcoded `IntervalRule` created at startup

### Known Gaps from Phase 2

1. **No auto-reconnection** — when a pipeline thread exits (error/EOF), nothing restarts it
2. **No pipeline health feedback** — `StreamHealth` fields (`frames_decoded`, `frames_extracted`, `last_pts`, etc.) are never updated from the decode loop; they remain at defaults
3. **Single rule type** — only fixed-interval, hardcoded at pipeline start; no runtime changes
4. **No rate limiting** — a fast stream could produce excessive frames

## Phase 3 Goals

1. **Reconnection Scheduler**: Automatically restart failed pipelines with exponential backoff; update health status throughout the lifecycle
2. **Per-Stream Rule Configuration**: Support interval, FPS-based, and rate-limited rules
3. **Rule CRUD API**: Create/read/update/delete rules at runtime via REST API
4. **Rate Limiting**: Enforce max frames per minute/hour per stream

## Requirements

| ID | Description | Priority |
|----|-------------|----------|
| RULE-02 | FPS-based extraction (extract at N frames per second) | High |
| RULE-03 | Per-stream configurable extraction rate | High |
| RULE-06 | Rate limiting (max frames per minute/hour) per stream | High |
| API-03 | Full CRUD API for extraction rules | High |
| STREAM-07 | Auto-reconnect with exponential backoff | High (deferred) |
| STREAM-08 | Graceful resource cleanup on extended failure | Medium |

## Key Architectural Decisions

### AD-01: Reconnection Scheduler

The `StreamManager` spawns a per-stream tokio background task that monitors the pipeline
thread via a watch channel. When the thread exits unexpectedly:
1. Update health to `Error` with error message
2. Wait exponential backoff (1s → 2s → 4s → 8s → 16s → 30s max)
3. Update health to `Connecting`
4. Re-spawn the pipeline thread and consumer
5. Update health to `Online`

A manual `POST /api/v1/streams/{id}/reconnect` endpoint triggers an immediate reconnect.

### AD-02: RuleConfig Enum

Extraction rules are defined as a serializable enum:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RuleConfig {
    Interval { interval_seconds: f64 },
    Fps { fps: f64 },
    RateLimited {
        rule: Box<RuleConfig>,
        max_per_minute: u64,
    },
}
```

Each variant maps to a `RuleEvaluator` trait implementation that computes
`should_extract(frame) -> bool`.

### AD-03: Shared Rule State for Hot-Reload

Rules are stored per-stream in `StreamRegistry` and shared with the pipeline via
`Arc<RwLock<Vec<RuleConfig>>>`. The decode loop re-reads rules on each frame evaluation
(the mutex is uncontended in practice). This allows rule changes via API to take effect
without restarting the pipeline.

### AD-04: Rule CRUD API

REST endpoints at `/api/v1/streams/{id}/rules`:

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/streams/{id}/rules` | List rules for a stream |
| POST | `/api/v1/streams/{id}/rules` | Add a rule to a stream |
| PUT | `/api/v1/streams/{id}/rules/{index}` | Replace a rule |
| DELETE | `/api/v1/streams/{id}/rules/{index}` | Remove a rule |

When rules are modified, the shared `Arc<RwLock<Vec<RuleConfig>>>` is updated in-place,
and the pipeline picks up changes on its next frame evaluation.

### AD-05: Rate Limiter — Token Bucket

The rate limiter uses a simple token bucket algorithm:
- `max_per_minute` tokens refilled at `max_per_minute / 60.0` per second
- A frame is extracted only if a token is available (and the inner rule matches)
- Token bucket state is per-stream, stored in the rule evaluator pipeline

### AD-06: Pipeline Health Feedback

The decode loop periodically (every 30 frames) updates a shared `StreamHealth` via
an `Arc<Mutex<StreamHealth>>` passed at pipeline creation. Fields updated:
- `frames_decoded` (incremented per decoded frame)
- `frames_extracted` (incremented per extracted frame)
- `last_pts` (set to current PTS)
- `status` → `Online` (set once decode loop starts successfully)

The reconnection scheduler also updates `status`, `last_error`, `error_count`, `reconnect_count`.

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Rule hot-reload race (decode reads half-updated rules) | Low | `Arc<RwLock<Vec<RuleConfig>>>` — writer locks, reader reads atomically |
| Backoff timer survives stream removal | Medium | Reconnection task checks if stream still exists before each backoff attempt |
| Duplicate frame on reconnect | Low | PTS-based interval rule naturally handles restarts (last_extracted_pts is lost on thread exit) |
