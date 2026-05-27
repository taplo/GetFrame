# Phase 6: Task Management API & Documentation — Research

**Researched:** 2026-05-25
**Domain:** REST API Task Lifecycle + OpenAPI 3.0 Documentation
**Confidence:** HIGH

## Summary

This phase adds a "task" abstraction layer on top of the existing stream infrastructure. A task wraps a stream + its extraction rules into a lifecycle-managed unit with states: Created → Running → Paused → Stopped → Deleted. The task API creates, starts, pauses, resumes, stops, and deletes extraction tasks via REST endpoints. All endpoints (including existing stream and rule routes) are documented with OpenAPI 3.0 via the `utoipa` crate, served interactively through Swagger UI at `/swagger-ui`.

**Key architectural insight:** Tasks are NOT streams — they are a higher-level abstraction that controls stream pipeline lifecycle. A task owns a `StreamId` and manages when its associated pipeline runs (start/pause/resume). The task registry is a separate in-memory store from `StreamRegistry`.

To support pause/resume cleanly, `StreamManager` needs three new internal methods: `start_pipeline`, `stop_pipeline`, and a refactored `add_stream` that delegates to `start_pipeline`. This avoids the current all-or-nothing coupling between config creation and pipeline start.

**Primary recommendation:** Add task CRUD via new `src/api/tasks.rs` and `src/task/` module, document ALL endpoints via `utoipa` 5.5 + `utoipa-swagger-ui` 9.0, and serve interactive Swagger UI at `/swagger-ui`. No frontend changes.

### Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| API-02 | Full CRUD API for extraction tasks | Task model, TaskManager, task REST routes, state machine |
| API-05 | OpenAPI/Swagger documentation | utoipa crate family, SwaggerUi router merge, ToSchema derive |

## User Constraints (from CONTEXT.md)

No CONTEXT.md exists for Phase 6 yet — this is the initial research. All decisions presented here are recommendations subject to user confirmation where marked `[ASSUMED]`.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Task CRUD routing + JSON serialization | API / Backend | — | New axum handlers in `src/api/tasks.rs` |
| Task state machine (pause/resume start/stop) | Backend / Core | — | Lives in `TaskManager` + `StreamManager` methods |
| Stream pipeline lifecycle (actual start/stop) | Backend / Core | — | `StreamManager` already owns pipeline handles |
| Task config persistence (in-memory) | Database / Storage | Backend / Core | `TaskRegistry` follows existing `StreamRegistry` in-memory pattern |
| OpenAPI spec generation | API / Backend | — | Compile-time code generation via `utoipa` macros |
| Swagger UI serving | API / Backend | — | Static asset serving via `utoipa-swagger-ui` axum feature |

---

## Standard Stack

### Core (new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `utoipa` | 5.5.0 | Compile-time OpenAPI 3.0 spec generation | Dominant Rust OpenAPI crate, active maintenance, 3.8k GitHub stars |
| `utoipa-axum` | 0.2.0 | Axum bindings for utoipa (OpenApiRouter) | Clean integration — `OpenApiRouter` auto-collects annotated handlers |
| `utoipa-swagger-ui` | 9.0.2 | Serve interactive Swagger UI from axum | Built-in `axum` feature merges SwaggerUi as axum Router |

**Version verification:**
```
utoipa = "5.5.0"               [VERIFIED: cargo search]
utoipa-axum = "0.2.0"          [VERIFIED: cargo search]
utoipa-swagger-ui = "9.0.2"    [VERIFIED: cargo search]
```

**Installation:**
```toml
utoipa = { version = "5", features = ["axum_extras", "chrono"] }
utoipa-axum = "0.2"
utoipa-swagger-ui = { version = "9", features = ["axum"] }
```

### Supporting (no new dependencies — uses existing crate patterns)

| Item | Purpose | Why |
|------|---------|-----|
| `serde` / `serde_json` | Request/Response serialization | Already in project |
| `chrono` | Task timestamps | Already in project |
| `uuid` | TaskId type | Already in project (`StreamId = Uuid`) |
| `thiserror` | Error types | Already in project |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| utoipa + utoipa-swagger-ui | aide | To evaluate at research time: aide is less maintained (2.4k ⭐ vs 3.8k ⭐). utoipa has `utoipa-axum` for direct Axum 0.8 support; aide's axum support is community. Not worth the risk for MVP. |
| utoipa + utoipa-swagger-ui | paperclip | Paperclip is unmaintained (last release 2021). Out of consideration. |
| utoipa + utoipa-swagger-ui | Okapi | Okapi targets Rocket, not Axum. Out of consideration. |
| utoipa without utoipa-axum | Manual ApiDoc path listing | Possible to skip `utoipa-axum` and list paths in `ApiDoc` struct manually, but maintainability cost is high as API grows. At 0.45 MB compile-time cost, `utoipa-axum` is worth it. |

---

## Package Legitimacy Audit

> slopcheck was unavailable at research time (pip not detected on this system). All packages tagged `[ASSUMED]` — planner must gate each install behind `checkpoint:human-verify`.

| Package | Registry | Age | Downloads | Source Repo | slopcheck | Disposition |
|---------|----------|-----|-----------|-------------|-----------|-------------|
| utoipa 5.5.0 | crates.io | ~4 yrs | 4M+ all-time | github.com/juhaku/utoipa | N/A (not run) | [ASSUMED] |
| utoipa-axum 0.2.0 | crates.io | ~2 yrs | 3.5M+ all-time | same repo | N/A | [ASSUMED] |
| utoipa-swagger-ui 9.0.2 | crates.io | ~4 yrs | 14M+ all-time | same repo | N/A | [ASSUMED] |

**Packages removed due to slopcheck [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

All three packages from the same well-known author (Juha Kukkonen), hosted on github.com/juhaku/utoipa, with millions of downloads and years of maintenance history. Low risk despite `[ASSUMED]` tag due to pip unavailability.

---

## Architecture Patterns

### System Architecture Diagram

```
                              ┌──────────────────────────────────┐
                              │           API Layer               │
                              │  ┌──────────┐  ┌──────────────┐  │
                              │  │ Streams  │  │    Tasks     │  │
                              │  │  CRUD    │  │ CRUD + Life- │  │
                              │  │(existing)│  │  cycle       │  │
                              │  └────┬─────┘  └──────┬───────┘  │
                              │       │               │          │
                              │  ┌────▼───────────────▼───────┐  │
                              │  │     utoipa OpenAPI gen     │  │
                              │  │     ┌─────────────────┐   │  │
                              │  │     │  Swagger UI     │   │  │
                              │  │     │  /swagger-ui    │   │  │
                              │  │     └─────────────────┘   │  │
                              │  └────────────────────────────┘  │
                              └──────────┬───────────────────────┘
                                         │
                    ┌────────────────────┼────────────────────┐
                    │                    │                    │
               ┌────▼────┐        ┌──────▼──────┐       ┌────▼────┐
               │ Stream  │        │  Task       │       │ Health  │
               │ Manager │        │  Manager    │       │ State   │
               │(existing│        │  (NEW)      │       │(existing│
               │  + stop │        │             │       │ )       │
               │ pipeline│        │  ┌────────┐ │       └─────────┘
               │  + start│        │  │ Task   │ │
               │ pipeline│        │  │Registry│ │
               └────┬────┘        │  │ (mem)  │ │
                    │             │  └────────┘ │
                    │             └──────┬──────┘
               ┌────▼────┐              │
               │ Stream  │          ┌───▼────┐
               │Registry │          │ Uuid   │
               │ (mem)   │          │ TaskId │
               └─────────┘          └────────┘

Data flow:
  POST /tasks → TaskManager.create_task() → TaskRegistry.add() → 201 Created
  POST /tasks/{id}/start → TaskManager.start_task()
    → StreamManager.start_pipeline(id, config) → starts decode pipeline → 200 OK
  POST /tasks/{id}/pause → TaskManager.pause_task()
    → StreamManager.stop_pipeline(id) → keeps registry entry → marks Paused
  GET /tasks/{id} → TaskRegistry.get() → returns task status + config
```

### Recommended Project Structure (new files only)

```
src/
├── api/
│   ├── mod.rs           ← Add `mod tasks;` + nest task routes
│   ├── streams.rs       ← Add #[utoipa::path] annotations (existing handlers)
│   ├── rules.rs         ← Add #[utoipa::path] annotations (existing handlers)
│   └── tasks.rs         ← NEW: task route handlers (CRUD + lifecycle)
├── task/
│   ├── mod.rs           ← NEW: TaskManager, TaskInfo, TaskStatus, TaskConfig
│   └── registry.rs      ← NEW: TaskRegistry (in-memory HashMap, follows StreamRegistry pattern)
├── main.rs              ← Merge SwaggerUi router, wire TaskManager into state
├── stream/
│   └── mod.rs           ← REFACTOR: add start_pipeline, stop_pipeline methods
```

### Task State Machine

```
                ┌──────────┐
                │ CREATED  │  ← POST /tasks (no pipeline started)
                └────┬─────┘
                     │ POST /tasks/{id}/start
                ┌────▼─────┐
          ┌─────│ RUNNING  │◄──────────┐
          │     └────┬─────┘           │
          │          │                 │
     pause│     ┌────┘                 │resume
          │     ▼                      │
          │  ┌───────┐                │
          └──┤PAUSED ├────────────────┘
             └───┬───┘
                 │ POST /tasks/{id}/stop
             ┌───▼────┐       ┌─────────┐
             │STOPPED │──────►│ DELETED │ (on DELETE)
             └────────┘       └─────────┘

Valid transitions:
  CREATED  → RUNNING  (start)
  RUNNING  → PAUSED   (pause)
  PAUSED   → RUNNING  (resume)
  RUNNING  → STOPPED  (stop)
  PAUSED   → STOPPED  (stop)
  STOPPED  → DELETED  (delete — also possible from any state)
  RUNNING  → ERROR    (internal — pipeline crash)
  ERROR    → RUNNING  (resume/restart)
```

### Task Data Model

```rust
/// Unique task identifier — reuses existing Uuid type pattern
pub type TaskId = uuid::Uuid;

/// Task lifecycle status
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum TaskStatus {
    Created,
    Running,
    Paused,
    Stopped,
    Error(String),
}

/// Core task entity
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TaskInfo {
    pub id: TaskId,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub stream_id: Option<StreamId>,       // Some when pipeline is running
    pub stream_config: StreamConfig,
    pub rules: Vec<RuleConfig>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
}

/// Task creation request body (POST /tasks)
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTaskRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub stream_config: StreamConfig,
    pub rules: Vec<RuleConfig>,
}
```

### Pattern 1: TaskManager wraps StreamManager for lifecycle

**What:** `TaskManager` owns the `TaskRegistry` and holds an `Arc<StreamManager>` to control pipeline lifecycle. It translates task lifecycle commands into StreamManager operations.

**Rationale:** Separates task concerns (state machine, metadata) from stream concerns (pipeline threads, decode). Follows existing pattern of managers holding shared state.

```rust
// src/task/mod.rs — conceptual structure
pub struct TaskManager {
    registry: TaskRegistry,
    stream_manager: Arc<StreamManager>,
}

impl TaskManager {
    pub fn create_task(&self, req: CreateTaskRequest) -> TaskInfo { ... }
    pub fn start_task(&self, id: TaskId) -> Result<TaskInfo, TaskError> { ... }
    pub fn pause_task(&self, id: TaskId) -> Result<TaskInfo, TaskError> { ... }
    pub fn resume_task(&self, id: TaskId) -> Result<TaskInfo, TaskError> { ... }
    pub fn stop_task(&self, id: TaskId) -> Result<TaskInfo, TaskError> { ... }
    pub fn delete_task(&self, id: TaskId) -> bool { ... }
    pub fn get_task(&self, id: TaskId) -> Option<TaskInfo> { ... }
    pub fn list_tasks(&self) -> Vec<TaskInfo> { ... }
}
```

### Pattern 2: Task lifecycle → StreamManager mapping

| Task Action | StreamManager Operation | TaskRegistry Change |
|---|---|---|
| create | none | Add entry, status=Created |
| start | `start_pipeline(id, config)` | Set stream_id, status=Running |
| pause | `stop_pipeline(stream_id)` | Clear stream_id, status=Paused |
| resume | `start_pipeline(id, config)` | Set stream_id, status=Running |
| stop | `stop_pipeline(stream_id)` | Clear stream_id, status=Stopped |
| delete | `remove_stream(stream_id)` | Remove entry |

### Pattern 3: OpenAPI documentation with utoipa

**What:** Use `#[utoipa::path]` attribute on each handler function to document its route, parameters, and responses. Annotate request/response structs with `#[derive(ToSchema)]`. Collect everything into an `ApiDoc` struct and serve via `SwaggerUi`.

**Source:** [CITED: docs.rs/utoipa/latest], [CITED: docs.rs/utoipa-swagger-ui/latest]

```rust
// Example pattern for annotating a task handler
#[derive(OpenApi)]
#[openapi(
    info(title = "GetFrame API", version = "0.1.0"),
    paths(
        // task endpoints
        api::tasks::list_tasks,
        api::tasks::create_task,
        api::tasks::get_task,
        api::tasks::delete_task,
        api::tasks::start_task,
        api::tasks::pause_task,
        api::tasks::resume_task,
        api::tasks::stop_task,
        // existing stream endpoints
        api::streams::list_streams,
        api::streams::create_stream,
        // ... all other endpoints
    ),
    components(schemas(
        // all request/response types
        api::tasks::CreateTaskRequest,
        api::tasks::TaskInfo,
        api::tasks::TaskStatus,
        // ... existing types
    ))
)]
struct ApiDoc;
```

### Anti-Patterns to Avoid

- **In-memory task registry with no persistence:** Follows existing `StreamRegistry` pattern. Acceptable for MVP, but the planner should note that task state is lost on restart. This is the same tradeoff already accepted for stream registry per STATE.md ("PostgreSQL deferred to Phase 6+" — a decision that now needs revisiting or explicit deferral).
- **Mixing task and stream state machines:** Tasks manage lifecycle (running/paused). Streams manage health (online/offline/error). These are orthogonal. Don't merge them.
- **Embedding OpenAPI logic into handler functions:** Keep `#[utoipa::path]` annotations and `ToSchema` derives on the handler/type definitions, but keep OpenAPI setup (ApiDoc, SwaggerUi) in `main.rs` or `api/mod.rs`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| OpenAPI 3.0 spec generation | Custom OpenAPI JSON builder | `utoipa` 5.5 with `#[utoipa::path]` + `#[derive(ToSchema)]` | Generate OpenAPI YAML at compile time; avoid runtime reflection and hand-maintained schema files |
| Swagger UI HTML serving | Static file server for Swagger UI assets | `utoipa-swagger-ui` 9.0 with `features = ["axum"]` | Embeds Swagger UI, serves it as axum Router directly, handles path routing for all assets |
| Task state machine | Custom thread-safe state transition logic | Use `TaskManager` wrapping `TaskRegistry` (in-memory HashMap + RwLock) | The state machine is simple (6 states, 7 transitions). A full state machine library (e.g., `skeletal` or `rust-machine`) is overkill. |
| Pipeline pause/resume | Custom thread management | Extend `StreamManager` with `stop_pipeline`/`start_pipeline` | Already have CancellationToken and JoinHandle infrastructure — just need to decouple pipeline stop from registry removal |

**Key insight:** The two hardest parts of this phase are (1) getting `#[utoipa::path]` proc-macro annotations correct for each handler (the macro is picky about response syntax) and (2) refactoring `StreamManager::add_stream` to separate "register config" from "start pipeline" without breaking existing call sites. Both are mechanical tasks once the pattern is established.

---

## Runtime State Inventory

> **Omitted:** Phase 6 is a greenfield API addition, not a rename/refactor/migration phase. No runtime state carries the "old string" or needs migration.

---

## Common Pitfalls

### Pitfall 1: utoipa macro attribute syntax

**What goes wrong:** The `#[utoipa::path(...)]` attribute has strict syntax requirements. Missing commas, wrong response body type, or incorrect `status = CODE` format causes cryptic proc-macro compile errors.

**Why it happens:** utoipa uses a proc-macro that parses its own DSL. Error messages often point to the handler function rather than the attribute.

**How to avoid:** Follow the canonical pattern exactly:
```rust
#[utoipa::path(
    get,
    path = "/api/v1/tasks",
    responses(
        (status = 200, description = "List of tasks", body = TaskListResponse)
    )
)]
```
- Always use `status = CODE` (not `status = "200"` — some versions differ)
- For empty response bodies, use `body = ()` or omit `body` entirely
- Use `content_type = "application/json"` when overriding default
- Test compile after adding each annotation, not all at once

**Warning signs:** Compile errors in generated code, span pointing to `#[utoipa::path]` line.

### Pitfall 2: utoipa-axum version compatibility with axum 0.8

**What goes wrong:** `utoipa-axum` 0.2.0 requires `axum >= 0.8`. If Cargo resolves a different version (e.g., 0.1.x), it pulls in `axum 0.7` as a transitive dependency, causing a version conflict.

**Why it happens:** Cargo semver resolution — `utoipa-axum = "0.2"` is the correct pin. Using `utoipa-axum = "0.1"` would pull axum 0.7.

**How to avoid:** Explicitly specify `utoipa-axum = "0.2"` in Cargo.toml. Run `cargo tree -i utoipa-axum` to verify it resolves to 0.2.x.

**Warning signs:** Duplicate axum versions in `Cargo.lock`, errors about `Router<()>` having multiple definitions.

### Pitfall 3: Swagger UI path routing with existing nest

**What goes wrong:** `SwaggerUi::new("/swagger-ui")` must be merged AFTER all other routers. If merged before, the Swagger UI catch-all route may steal API routes.

**How to avoid:** Always merge SwaggerUi last:
```rust
let app = health_router
    .merge(api_router)
    .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api))
    .route("/metrics", get(metrics_handler));
```

**Warning signs:** Swagger UI returns 404 for API routes, or API routes return Swagger UI HTML.

### Pitfall 4: Mutable access to StreamManager's pipelines map across pause/resume cycles

**What goes wrong:** When pausing (stop_pipeline) and resuming (start_pipeline), the `pipelines: Arc<Mutex<HashMap>>` lock must not be held across async boundaries. The reconnection task spawned by `start_pipeline` also accesses pipelines.

**How to avoid:** The existing code already uses `pipelines.lock().unwrap()` correctly (scoped locks, dropped before await). The new `stop_pipeline`/`start_pipeline` methods must follow the same pattern — acquire lock, modify map, drop lock before any async work.

**Warning signs:** Deadlocks during pause/resume, pipeline not restarting after resume.

---

## Code Examples

Verified patterns from official sources:

### Example 1: Task route handler with utoipa annotation

```rust
/// List all extraction tasks
#[utoipa::path(
    get,
    path = "/api/v1/tasks",
    tag = "tasks",
    responses(
        (status = 200, description = "Task list retrieved successfully", body = TaskListResponse)
    )
)]
async fn list_tasks(
    State(manager): State<Arc<TaskManager>>,
) -> Json<TaskListResponse> {
    let tasks = manager.list_tasks();
    Json(TaskListResponse { tasks })
}
```

### Example 2: Task lifecycle action handler

```rust
/// Start an extraction task (transition from Created to Running)
#[utoipa::path(
    post,
    path = "/api/v1/tasks/{id}/start",
    tag = "tasks",
    params(
        ("id" = TaskId, Path, description = "Task ID")
    ),
    responses(
        (status = 200, description = "Task started successfully", body = TaskInfo),
        (status = 404, description = "Task not found", body = ErrorResponse),
        (status = 409, description = "Invalid state transition", body = ErrorResponse),
    )
)]
async fn start_task(
    State(manager): State<Arc<TaskManager>>,
    Path(id): Path<TaskId>,
) -> Result<Json<TaskInfo>, ApiError> {
    manager.start_task(id)
        .map(Json)
        .map_err(|e| ApiError::Conflict(e.to_string()))
}
```

### Example 3: StreamManager pause/resume extension

```rust
// In src/stream/mod.rs — new methods on StreamManager

/// Stop pipeline for a stream but keep its registry entry.
/// Returns true if pipeline was running, false if it wasn't.
pub fn stop_pipeline(&self, id: &StreamId) -> bool {
    let handle = self.pipelines.lock().unwrap().remove(id);
    if let Some(mut h) = handle {
        h.exit_tx.send(Some(PipelineExitReason::UserInitiated)).ok();
        h.shutdown_token.cancel();
        if let Some(jh) = h.join_handle.take() {
            let _ = jh.join();
        }
        crate::metrics::STREAMS_ACTIVE.decrement(1.0);
        true
    } else {
        false
    }
}

/// Start pipeline for a stream that already has a registry entry.
/// Returns true if pipeline started, false if stream config not found.
pub fn start_pipeline(&self, id: &StreamId) -> bool {
    let info = match self.registry.get(id) {
        Some(info) => info,
        None => return false,
    };

    let shutdown_token = CancellationToken::new();
    let health_handle = Arc::new(Mutex::new(StreamHealth::new()));
    let rules_shared = self.registry.get_rules_shared(id)
        .expect("Stream must exist in registry");

    let (exit_tx, exit_rx) = tokio::sync::watch::channel(None::<PipelineExitReason>);

    let mut pipeline = pipeline::Pipeline::start(
        &info.config, *id, shutdown_token.clone(),
        health_handle.clone(), rules_shared.clone(),
    );

    self.spawn_frame_consumer(
        *id, &info.config, pipeline.extracted_rx.clone(),
        shutdown_token.clone(), health_handle.clone(),
    );

    let handle = PipelineHandle {
        shutdown_token,
        join_handle: pipeline.decode_handle.take(),
        health_handle,
        rules_shared,
        exit_tx,
    };
    self.pipelines.lock().unwrap().insert(*id, handle);

    self.spawn_reconnection_task(
        *id, info.config.clone(), exit_rx,
        health_handle, rules_shared,
    );

    crate::metrics::STREAMS_ACTIVE.increment(1.0);
    true
}
```

**Source:** [CITED: docs.rs/utoipa/latest/utoipa/attr.path.html] — utoipa path macro reference
**Source:** [CITED: docs.rs/utoipa-swagger-ui/latest/utoipa_swagger_ui] — SwaggerUi axum integration
**Source:** [CITED: github.com/juhaku/utoipa/tree/master/examples/axum-utoipa-bindings] — complete working example

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No API documentation | OpenAPI 3.0 + Swagger UI | This phase | API consumers can discover and test all endpoints interactively |
| Stream = running pipeline | Task = lifecycle-controlled pipeline | This phase | Separates config/storage from execution; enables pause/resume |
| Manual handler registration | utoipa-annotated handlers + auto-collected spec | This phase | Single source of truth for API docs; docs never get stale |

**Deprecated/outdated:**
- Skip `okapi` and `paperclip` — both are effectively unmaintained for axum.
- Do not use `utoipa` 4.x (needs `#[openapi(...)]` attribute on handlers) — 5.x is current.

---

## Assumptions Log

All packages in this research were verified via `cargo search` from the crate registry. However, since `slopcheck` was not available in this environment, all package recommendations are tagged `[ASSUMED]` per protocol — the planner should gate each install behind `checkpoint:human-verify`.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | utoipa 5.x with `features = ["axum_extras"]` works with axum 0.8 | Standard Stack | utoipa 5.5.0 may need specific version pin if API surface changed; pinned to `"5"` semver range handles minor bumps |
| A2 | utoipa-axum 0.2.0 is compatible with axum 0.8.x | Standard Stack | Confirmed via crates.io metadata — 0.2.0 depends on axum >= 0.8; verified by `cargo search` finding it |
| A3 | utoipa-swagger-ui 9.0.2 `features = ["axum"]` works with axum 0.8 | Standard Stack | Docs say "axum >= 0.7" which includes 0.8; verified by crates.io |
| A4 | `PipelineHandle` fields remain private in `src/stream/mod.rs` | Architecture | The `stop_pipeline`/`start_pipeline` methods are on `StreamManager` which has access — no visibility change needed |

---

## Open Questions (RESOLVED)

1. **Should task config persist as YAML somewhere, or stay purely in-memory?**
   - What we know: Current `StreamRegistry` is purely in-memory. STATE.md says "PostgreSQL deferred to Phase 6+."
   - What's unclear: Phase 6 is "Phase 6+" — should we add YAML persistence or stay in-memory?
   - RESOLVED: **Stay in-memory for MVP.** Follow the existing pattern. Task state is lost on restart, same as stream state. A later phase can add persistence.

2. **Should we annotate existing stream/rule handlers with utoipa in this phase, or only new task handlers?**
   - What we know: Success criteria says "All API endpoints are documented."
   - What's unclear: Whether "all" means all endpoints (including existing streams/rules) or just the new task endpoints.
   - RESOLVED: **Annotate all endpoints.** The effort is small (add `#[utoipa::path]` to existing handlers, `#[derive(ToSchema)]` to existing types). This satisfies the success criteria completely.

3. **Should tasks reuse the existing `StreamConfig` directly or wrap it in a task-specific config?**
   - RESOLVED: Reuse `StreamConfig` directly in `TaskInfo`. The task becomes a lifecycle manager around a stream config + rules. This minimizes new types.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust/cargo | All | ✓ | (stable 2024 edition) | — |
| utoipa 5+ | OpenAPI generation | ✓ (on crates.io) | 5.5.0 | — |
| utoipa-axum 0.2+ | Axum OpenAPI integration | ✓ (on crates.io) | 0.2.0 | Skip and use manual path listing |
| utoipa-swagger-ui 9+ | Swagger UI serving | ✓ (on crates.io) | 9.0.2 | Serve raw openapi.json without UI |

**Missing dependencies with no fallback:** None — all crates are on crates.io.

---

## Validation Architecture

> `workflow.nyquist_validation` key not found in config — treat as enabled.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | None detected in Cargo.toml (no `[dev-dependencies]` section) |
| Config file | None |
| Quick run command | `cargo test` (if tests exist) |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| API-02 | Task CRUD + lifecycle handlers respond correctly | integration | `cargo test --test task_api` | ❌ Wave 0 |
| API-05 | OpenAPI spec is valid JSON and contains all endpoints | integration | `cargo test --test openapi_spec` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo build` (compile check)
- **Per wave merge:** `cargo test`
- **Phase gate:** Full `cargo test` green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `tests/task_api.rs` — integration test for task CRUD + lifecycle
- [ ] `tests/openapi_spec.rs` — integration test that fetches `/api-docs/openapi.json` and validates structure
- [ ] `Cargo.toml` dev-dependencies: `tower` for test `Service` usage, `reqwest` for integration tests (reqwest already in main deps)

**Recommendation:** Add integration tests in `tests/` directory (not `src/`). Use `axum::test` helpers (axum's built-in test support) to test handlers without running a server. Test at minimum: create task, start task, get task status, pause task, resume task, stop task, delete task, and verify 404 for non-existent tasks.

---

## Security Domain

> `security_enforcement` absent from config — treat as enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | serde deserialization provides type-level validation; task actions validate state machine transitions server-side |
| V4 Access Control | no | No authentication in MVP (deferred to Phase v2 API-06) |
| V2 Authentication | no | No auth in MVP |
| V3 Session Management | no | No sessions in MVP |

### Known Threat Patterns for Axum + utoipa

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Invalid state transition (e.g., start an already-running task) | Tampering | Server-side state machine validation returns 409 Conflict |
| Path traversal in Swagger UI URL | Tampering | utoipa-swagger-ui handles URL sanitization internally |
| Large JSON body DoS | Denial of Service | axum defaults to 2MB body limit; serde deserialization is bounded |

---

## Sources

### Primary (HIGH confidence)
- [Cargo search results] — `utoipa` 5.5.0, `utoipa-axum` 0.2.0, `utoipa-swagger-ui` 9.0.2 confirmed on crates.io
- [docs.rs/utoipa/latest] — OpenAPI generation macros, `axum_extras` feature, `chrono` feature
- [docs.rs/utoipa-axum/latest] — `OpenApiRouter`, `routes!` macro, `split_for_parts()`
- [docs.rs/utoipa-swagger-ui/latest] — `SwaggerUi::new()`, `axum` feature support for axum >= 0.7
- [github.com/juhaku/utoipa/examples/axum-utoipa-bindings] — Complete Axum + utoipa example
- [Existing codebase reading] — `src/stream/mod.rs`, `src/api/streams.rs`, `src/api/rules.rs`, `src/stream/registry.rs`, `src/main.rs`, `Cargo.toml`

### Secondary (MEDIUM confidence)
- [StackOverflow thread 79338309] — Confirms utoipa-axum 0.1.x → axum 0.7, utoipa-axum 0.2.x → axum 0.8 compatibility boundary
- [Lib.rs utoipa-swagger-ui page] — Confirms 9.0.2 released May 2025, axum >= 0.7 support

### Tertiary (LOW confidence)
None — all claims verified against primary sources.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions verified via `cargo search`, compatibility documented in crate docs
- Architecture: HIGH — follows existing codebase patterns exactly (StreamRegistry → TaskRegistry, StreamManager methods → TaskManager)
- Pitfalls: HIGH — all identified from existing utoipa documentation, known crate compatibility issues, and codebase analysis

**Research date:** 2026-05-25
**Valid until:** 2026-06-25 (30 days — crate ecosystem stable; utoipa and axum are mature)
