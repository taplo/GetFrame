# Phase 3 Summary: Per-Stream Rule Configuration & Reconnection

**Completed:** 2026-05-25
**Plan:** 03-01 (6 tasks)

## Delivered

### Rule Engine Refactor (Task 1)
- `RuleConfig` enum with serde-tagged variants: `Interval`, `FPS`, `RateLimited`
- `RuleEvaluator` trait with `should_extract()` and `description()`
- `IntervalEvaluator` — same PTS-based logic as original `IntervalRule`, now behind trait
- `RateLimitedEvaluator` — token bucket wrapping any inner evaluator
- `RuleEngine` — composite evaluation (any-match), `rebuild()` for hot-reload
- Factory `create_evaluator()` maps `RuleConfig` → `Box<dyn RuleEvaluator>`

### Shared Rules + Health Helpers (Task 2)
- `StreamInfo.rules: Arc<RwLock<Vec<RuleConfig>>>` — shared mutable rules per stream
- `StreamRegistry` methods: `get_rules()`, `update_rules()`, `get_rules_shared()`
- Default rule created from `StreamConfig.extract_interval_seconds` on stream add
- `StreamHealth` helpers: `mark_online()`, `mark_error()`, `mark_connecting()`, `mark_reconnected()`
- `StreamHealth` implements `Default`

### Pipeline Integration (Task 3)
- `Pipeline::start()` accepts `health_handle: Arc<Mutex<StreamHealth>>` and `rules_shared: Arc<RwLock<Vec<RuleConfig>>>`
- Decode loop marks health `Online` at start
- `RuleEngine` rebuilt from shared rules every frame (uncontended lock, neglible overhead)
- Health counters `frames_decoded`/`frames_extracted`/`last_pts` updated every 30 frames

### Reconnection Scheduler (Task 4)
- `PipelineExitReason` enum: `UserInitiated`, `Error(String)`, `Eof`
- Per-stream `tokio::spawn` reconnection task via `watch::channel`
- Exponential backoff: 1s → 2s → 4s → 8s → 16s → 30s max
- Health lifecycle: `Error` → (sleep) → `Connecting` → reconnect → `Online`
- Stream removal (`UserInitiated`) cleanly terminates the reconnection task

### Rule CRUD API (Task 5)
- `GET/POST /api/v1/streams/{id}/rules` — list/add rules
- `GET/PUT/DELETE /api/v1/streams/{id}/rules/{index}` — get/update/delete by index
- `update_rules()` writes to `Arc<RwLock>` → pipeline picks up changes on next frame
- Proper 404/error handling for missing streams and invalid indices

### Wiring (Task 6)
- `StreamManager::new(storage, kafka)` — updated constructor
- `main.rs` unchanged — already uses correct signatures
- `config.example.yaml` — added rule API documentation comments

## Deviations from Plan

| Planned | Actual | Reason |
|---------|--------|--------|
| Backoff reset on successful reconnect | Backoff does NOT reset on success | Resetting on success causes incorrect 1s wait on rapid failure-reconnect cycles; exponential backoff now persists until sustained uptime |
| 5 tasks | 6 tasks | Wiring/config separated for clarity |

## Build Result
- `cargo build` — **0 errors**
- 10 warnings — all expected (pre-existing dead code annotations for forward-looking API)

## Files Changed/Created

| File | Change |
|------|--------|
| `src/pipeline/rule.rs` | Complete rewrite: `RuleConfig`, `RuleEvaluator` trait, `IntervalEvaluator`, `RateLimitedEvaluator`, `RuleEngine` |
| `src/pipeline/mod.rs` | `Pipeline::start()` accepts `health_handle` + `rules_shared` |
| `src/pipeline/decode.rs` | Health feedback, shared rule hot-reload, `RuleEngine` replacing `IntervalRule` |
| `src/stream/mod.rs` | Reconnection scheduler, `PipelineHandle` with exit channel, refactored consumer |
| `src/stream/registry.rs` | `rules` field in `StreamInfo`, rule accessor methods |
| `src/stream/health.rs` | Helper methods (`mark_*`, `record_*`), `Default` impl |
| `src/storage/mod.rs` | Added `bucket()` getter |
| `src/api/mod.rs` | Added rules routes |
| `src/api/rules.rs` | New file: rule CRUD endpoints |
| `config.example.yaml` | Added rule API documentation |

## Next Phase: Phase 4 — Scene Detection & Composite Rules
Requirements: RULE-04 (scdet filter), RULE-05 (composite interval+scene rules)
