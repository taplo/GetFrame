# Activity Log Viewer (UI-09) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a unified activity log that records all stream/task/auth/worker operations and presents them in a dedicated frontend page with filtering, search, and CSV export.

**Architecture:** New `activity_log` MySQL table with a `ActivityLogger` DB access layer. Backend API at `/api/v1/activity` with list + export endpoints. Frontend `ActivityLog` page with filter bar, timeline view, pagination, and CSV download. Existing `task_events` table preserved unchanged.

**Tech Stack:** Rust + SQLx + Axum (backend), React 19 + TypeScript + Tailwind v4 + shadcn/ui (frontend)

---

### Task 1: Migration + DB Layer

**Files:**
- Create: `migrations/20260606_000001_activity_log.sql`
- Create: `src/db/activity_log.rs`
- Modify: `src/db/mod.rs`

- [ ] **Step 1: Write migration SQL**

```sql
-- migrations/20260606_000001_activity_log.sql
CREATE TABLE IF NOT EXISTS activity_log (
  id           BIGINT AUTO_INCREMENT PRIMARY KEY,
  event_type   VARCHAR(50)   NOT NULL,
  resource_type VARCHAR(30)  NOT NULL,
  resource_id  VARCHAR(36),
  actor        VARCHAR(64)   NOT NULL DEFAULT 'system',
  description  TEXT          NOT NULL,
  details      JSON,
  recorded_at  TIMESTAMP(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  INDEX idx_activity_type (event_type),
  INDEX idx_activity_resource (resource_type, resource_id),
  INDEX idx_activity_recorded (recorded_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

- [ ] **Step 2: Write failing unit test**

```rust
// In src/db/activity_log.rs, at the bottom
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_log_row_roundtrip() {
        // We'll test the struct exists and fields are accessible
        let row = ActivityLogRow {
            id: 1,
            event_type: "stream.created".into(),
            resource_type: "stream".into(),
            resource_id: Some("uuid".into()),
            actor: "admin".into(),
            description: "创建流 \"Camera-01\"".into(),
            details: None,
            recorded_at: chrono::Utc::now(),
        };
        assert_eq!(row.event_type, "stream.created");
        assert_eq!(row.actor, "admin");
    }

    #[test]
    fn test_activity_log_query_struct() {
        let q = ActivityLogQuery {
            event_type: Some("stream.created".into()),
            resource_type: None,
            actor: None,
            search: None,
            since: None,
            until: None,
            page: 1,
            page_size: 50,
        };
        assert_eq!(q.page, 1);
        assert_eq!(q.page_size, 50);
        assert_eq!(q.event_type.unwrap(), "stream.created");
    }
}
```

- [ ] **Step 3: Verify test fails** (structs not defined yet)

Run: `cargo test --package getframe-worker --lib db::activity_log::tests --no-fail-fast`
Expected: compilation error (no module `activity_log`)

- [ ] **Step 4: Write ActivityLogRow struct and ActivityLogQuery struct**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLogRow {
    pub id: i64,
    pub event_type: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub actor: String,
    pub description: String,
    pub details: Option<serde_json::Value>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ActivityLogQuery {
    pub event_type: Option<String>,
    pub resource_type: Option<String>,
    pub actor: Option<String>,
    pub search: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub page: i64,
    pub page_size: i64,
}

impl Default for ActivityLogQuery {
    fn default() -> Self {
        Self {
            event_type: None,
            resource_type: None,
            actor: None,
            search: None,
            since: None,
            until: None,
            page: 1,
            page_size: 50,
        }
    }
}
```

- [ ] **Step 5: Write insert, query, and query_export functions**

Use two parallel `QueryBuilder` instances for count + data queries:

```rust
use sqlx::{MySqlPool, QueryBuilder};

pub async fn insert(pool: &MySqlPool, row: &ActivityLogRow) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_log (event_type, resource_type, resource_id, actor, description, details, recorded_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&row.event_type)
    .bind(&row.resource_type)
    .bind(&row.resource_id)
    .bind(&row.actor)
    .bind(&row.description)
    .bind(&row.details as Option<serde_json::Value>)
    .bind(row.recorded_at)
    .execute(pool)
    .await?;
    Ok(())
}

fn build_filtered_query<'a>(q: &'a ActivityLogQuery, select: &str) -> QueryBuilder<'a, sqlx::MySql> {
    let mut builder = QueryBuilder::new(select);
    let mut sep = " WHERE ";
    if let Some(ref et) = q.event_type {
        builder.push(sep).push("event_type = ").push_bind(et);
        sep = " AND ";
    }
    if let Some(ref rt) = q.resource_type {
        builder.push(sep).push("resource_type = ").push_bind(rt);
        sep = " AND ";
    }
    if let Some(ref a) = q.actor {
        builder.push(sep).push("actor = ").push_bind(a);
        sep = " AND ";
    }
    if let Some(ref s) = q.search {
        builder.push(sep).push("description LIKE ").push_bind(format!("%{}%", s));
        sep = " AND ";
    }
    if let Some(ref since) = q.since {
        builder.push(sep).push("recorded_at >= ").push_bind(since);
        sep = " AND ";
    }
    if let Some(ref until) = q.until {
        builder.push(sep).push("recorded_at <= ").push_bind(until);
    }
    builder
}

pub async fn query(
    pool: &MySqlPool,
    q: &ActivityLogQuery,
) -> Result<(Vec<ActivityLogRow>, i64), sqlx::Error> {
    let offset = (q.page - 1) * q.page_size;

    // Count query (reuse conditions via clone + replace)
    let mut count_builder = build_filtered_query(q, "SELECT COUNT(*) FROM activity_log");
    let total: (i64,) = count_builder.build_query_as().fetch_one(pool).await?;

    // Data query
    let mut data_builder = build_filtered_query(q,
        "SELECT id, event_type, resource_type, resource_id, actor, description, details, recorded_at FROM activity_log"
    );
    data_builder.push(" ORDER BY recorded_at DESC");
    data_builder.push(" LIMIT ").push_bind(q.page_size).push(" OFFSET ").push_bind(offset);
    let rows: Vec<ActivityLogRow> = data_builder.build_query_as().fetch_all(pool).await?;

    Ok((rows, total.0))
}

pub async fn query_export(
    pool: &MySqlPool,
    q: &ActivityLogQuery,
) -> Result<Vec<ActivityLogRow>, sqlx::Error> {
    let mut q2 = q.clone();
    q2.page = 1;
    q2.page_size = 100000;
    let (rows, _) = query(pool, &q2).await?;
    Ok(rows)
}
```

- [ ] **Step 6: Register module in `src/db/mod.rs`**

```rust
pub mod activity_log;
```

- [ ] **Step 7: Run unit tests**

Run: `cargo test --package getframe-worker --lib db::activity_log::tests --no-fail-fast`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add migrations/20260606_000001_activity_log.sql src/db/activity_log.rs src/db/mod.rs
git commit -m "feat: add activity_log migration and DB access layer (UI-09)"
```

### Task 2: Activity Log API Endpoints

**Files:**
- Create: `src/api/activity.rs`
- Modify: `src/api/mod.rs`

- [ ] **Step 1: Write failing test**

```rust
// At the bottom of src/api/activity.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_list_response_serialization() {
        let resp = ActivityListResponse {
            items: vec![],
            total: 0,
            page: 1,
            page_size: 50,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"total\":0"));
    }
}
```

- [ ] **Step 2: Write response types and handler**

```rust
use axum::{extract::{Query, State}, Json, Router};
use serde::Serialize;
use crate::db::activity_log::{self, ActivityLogRow, ActivityLogQuery};

#[derive(Debug, Serialize)]
pub struct ActivityListResponse {
    pub items: Vec<ActivityLogRow>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

pub fn activity_routes(pool: Option<sqlx::MySqlPool>) -> Router {
    let mut router = Router::new();
    if let Some(p) = pool {
        router = router
            .route("/", axum::routing::get(list_handler))
            .route("/export", axum::routing::get(export_handler))
            .with_state(p);
    }
    router
}

#[derive(Debug, serde::Deserialize)]
pub struct ActivityQueryParams {
    pub event_type: Option<String>,
    pub resource_type: Option<String>,
    pub actor: Option<String>,
    pub search: Option<String>,
    pub since: Option<String>,       // ISO 8601
    pub until: Option<String>,       // ISO 8601
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub async fn list_handler(
    State(pool): State<sqlx::MySqlPool>,
    Query(params): Query<ActivityQueryParams>,
) -> Result<Json<ActivityListResponse>, (StatusCode, Json<serde_json::Value>)> {
    let query = build_query(params);
    match activity_log::query(&pool, &query).await {
        Ok((items, total)) => Ok(Json(ActivityListResponse {
            items,
            total,
            page: query.page,
            page_size: query.page_size,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Query failed: {}", e)})),
        )),
    }
}

pub async fn export_handler(
    State(pool): State<sqlx::MySqlPool>,
    Query(params): Query<ActivityQueryParams>,
) -> Result<(StatusCode, [(String, String); 1], String), (StatusCode, Json<serde_json::Value>)> {
    let query = build_query(params);
    match activity_log::query_export(&pool, &query, 100000).await {
        Ok(rows) => {
            let mut csv = String::from("id,event_type,resource_type,resource_id,actor,description,recorded_at\n");
            for row in rows {
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{}\n",
                    row.id,
                    row.event_type,
                    row.resource_type,
                    row.resource_id.unwrap_or_default(),
                    row.actor,
                    row.description.replace('"', "\"\""),
                    row.recorded_at.to_rfc3339(),
                ));
            }
            Ok((
                StatusCode::OK,
                [("Content-Type".into(), "text/csv; charset=utf-8".into())],
                csv,
            ))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Export failed: {}", e)})),
        )),
    }
}

fn build_query(params: ActivityQueryParams) -> ActivityLogQuery {
    ActivityLogQuery {
        event_type: params.event_type,
        resource_type: params.resource_type,
        actor: params.actor,
        search: params.search,
        since: params.since.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
        until: params.until.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&chrono::Utc))),
        page: params.page.unwrap_or(1),
        page_size: params.page_size.unwrap_or(50),
    }
}
```

- [ ] **Step 3: Register in `src/api/mod.rs`**

```rust
mod activity;

// In openapi macro:
// paths(..., activity::list_handler, activity::export_handler)
// components(schemas(..., activity::ActivityListResponse, db::activity_log::ActivityLogRow))

// In api_router function:
pub fn api_router(manager: StreamManager, task_manager: Arc<TaskManager>, db_pool: Option<MySqlPool>) -> Router {
    let mut router = Router::new()
        .nest("/api/v1/streams", streams::stream_routes(manager.clone()))
        .nest("/api/v1/streams/{id}/rules", rules::rules_routes(manager))
        .nest("/api/v1/tasks", tasks::task_routes(task_manager))
        .nest("/api/v1/metrics", metrics::metrics_routes(...))
        .nest("/api/v1/activity", activity::activity_routes(db_pool.clone()));
    // ...
}
```

- [ ] **Step 4: Add openapi annotations**

```rust
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ActivityListResponse {
    pub items: Vec<ActivityLogRow>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[utoipa::path(
    get,
    path = "/api/v1/activity",
    params(
        ("event_type" = Option<String>, Query, description = "Filter by event type"),
        ("resource_type" = Option<String>, Query, description = "Filter by resource type"),
        ("actor" = Option<String>, Query, description = "Filter by actor"),
        ("search" = Option<String>, Query, description = "Search description"),
        ("since" = Option<String>, Query, description = "Start time (ISO 8601)"),
        ("until" = Option<String>, Query, description = "End time (ISO 8601)"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("page_size" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Activity list", body = ActivityListResponse),
    ),
    tag = "Activity",
)]
pub async fn list_handler(...) { ... }
```

- [ ] **Step 5: Compile and fix**

Run: `cargo check --package getframe-worker`
Expected: Compilation passes

- [ ] **Step 6: Commit**

```bash
git add src/api/activity.rs src/api/mod.rs
git commit -m "feat: add activity log API endpoints (list + CSV export) (UI-09)"
```

### Task 3: Instrument Stream Operations

**Files:**
- Modify: `src/stream/mod.rs`

- [ ] **Step 1: Add activity_log import and helper**

```rust
// At top of src/stream/mod.rs
use crate::db::activity_log::{self, ActivityLogRow};

// Helper to record activity, called from within StreamManager
impl StreamManager {
    fn record_activity(
        &self,
        event_type: &str,
        resource_id: Option<&str>,
        actor: &str,
        description: String,
        details: Option<serde_json::Value>,
    ) {
        let pool = self.db_pool.clone();
        let et = event_type.to_owned();
        let rid = resource_id.map(|s| s.to_owned());
        let a = actor.to_owned();
        tokio::spawn(async move {
            if let Some(p) = pool {
                let row = ActivityLogRow {
                    id: 0,
                    event_type: et,
                    resource_type: "stream".into(),
                    resource_id: rid,
                    actor: a,
                    description,
                    details,
                    recorded_at: chrono::Utc::now(),
                };
                let _ = activity_log::insert(&p, &row).await;
            }
        });
    }
}
```

- [ ] **Step 2: Instrument add_stream**

```rust
// Inside StreamManager::add_stream, after successful pipeline start:
self.record_activity(
    "stream.created",
    Some(&config.id.to_string()),
    "system",
    format!("创建流 \"{}\"", config.name),
    Some(serde_json::json!({
        "source_url": config.source_url,
        "source_type": config.source_type,
    })),
);
```

- [ ] **Step 3: Instrument remove_stream**

```rust
// In StreamManager::remove_stream, before removing from registry:
self.record_activity(
    "stream.deleted",
    Some(&id.to_string()),
    "system",
    format!("删除流 \"{}\"", name),
    None,
);
```

- [ ] **Step 4: Instrument update_stream_config**

```rust
// In StreamManager::update_stream_config:
self.record_activity(
    "stream.updated",
    Some(&id.to_string()),
    "system",
    format!("更新流 \"{}\" 配置", name),
    None,
);
```

- [ ] **Step 5: Compile and verify**

Run: `cargo check --package getframe-worker`
Expected: Compilation passes

- [ ] **Step 6: Commit**

```bash
git add src/stream/mod.rs
git commit -m "feat: instrument stream CRUD operations for activity log (UI-09)"
```

### Task 4: Instrument Task Operations

**Files:**
- Modify: `src/task/mod.rs`

- [ ] **Step 1: Add record_activity helper alongside existing record_event**

```rust
// In src/task/mod.rs
use crate::db::activity_log::{self, ActivityLogRow};

impl TaskManager {
    fn record_activity(
        &self,
        event_type: &str,
        task_id: &str,
        description: String,
        details: Option<serde_json::Value>,
    ) {
        let pool = self.db_pool.clone();
        let et = event_type.to_owned();
        let tid = task_id.to_owned();
        let actor = "system".to_owned(); // tasks are system-triggered
        tokio::spawn(async move {
            if let Some(p) = pool {
                let row = ActivityLogRow {
                    id: 0,
                    event_type: et,
                    resource_type: "task".into(),
                    resource_id: Some(tid),
                    actor,
                    description,
                    details,
                    recorded_at: chrono::Utc::now(),
                };
                let _ = activity_log::insert(&p, &row).await;
            }
        });
    }
}
```

- [ ] **Step 2: Add activity_log calls alongside each existing record_event call**

For each of the 5 existing `self.record_event(...)` calls (start_task, pause_task, resume_task, stop_task, delete_task), add a corresponding `self.record_activity(...)` call with the correct event_type and a Chinese description.

Pattern:
```rust
// Existing:
self.record_event(&id, "Started", None);

// New:
self.record_activity("task.started", &id.to_string(),
    format!("启动任务 \"{}\"", name), None);
```

Also add activity recording in `create_task` (event_type `"task.created"`) which doesn't currently call record_event.

- [ ] **Step 3: Compile and verify**

Run: `cargo check --package getframe-worker`
Expected: Compilation passes

- [ ] **Step 4: Commit**

```bash
git add src/task/mod.rs
git commit -m "feat: instrument task lifecycle for activity log (UI-09)"
```

### Task 5: Instrument Auth Operations

**Files:**
- Modify: `src/auth/handlers.rs`

- [ ] **Step 1: Add record_activity helper**

```rust
// In src/auth/handlers.rs
use crate::db::activity_log::{self, ActivityLogRow};

async fn record_activity(
    pool: &MySqlPool,
    event_type: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    actor: &str,
    description: String,
) {
    let row = ActivityLogRow {
        id: 0,
        event_type: event_type.to_owned(),
        resource_type: resource_type.to_owned(),
        resource_id: resource_id.map(|s| s.to_owned()),
        actor: actor.to_owned(),
        description,
        details: None,
        recorded_at: chrono::Utc::now(),
    };
    let _ = activity_log::insert(pool, &row).await;
}
```

- [ ] **Step 2: Instrument login handler**

```rust
// After successful login verification:
record_activity(
    &pool,
    "auth.login",
    "user",
    Some(&user.id.to_string()),
    &input.username,
    format!("用户 {} 登录", input.username),
).await;
```

- [ ] **Step 3: Instrument create_user/deleted_user**

```rust
// In create_user handler, after successful insert:
record_activity(
    &pool,
    "auth.user_created",
    "user",
    Some(&user_id),
    &auth_user.username,
    format!("创建用户 \"{}\" (角色: {})", input.username, input.role),
).await;

// In delete_user handler, before delete:
record_activity(
    &pool,
    "auth.user_deleted",
    "user",
    Some(&id),
    &auth_user.username,
    format!("删除用户 \"{}\"", username),
).await;
```

- [ ] **Step 4: Instrument create_api_key / delete_api_key**

```rust
// In create_api_key handler, after insert:
record_activity(
    &pool,
    "auth.api_key_created",
    "api_key",
    Some(&key_id),
    &auth_user.username,
    format!("创建 API Key \"{}\"", input.name),
).await;

// In delete_api_key handler:
record_activity(
    &pool,
    "auth.api_key_deleted",
    "api_key",
    Some(&id),
    &auth_user.username,
    format!("删除 API Key (前缀: {})", prefix),
).await;
```

- [ ] **Step 5: Compile and verify**

Run: `cargo check --package getframe-worker`
Expected: Compilation passes

- [ ] **Step 6: Commit**

```bash
git add src/auth/handlers.rs
git commit -m "feat: instrument auth operations for activity log (UI-09)"
```

### Task 6: Instrument Worker Operations

**Files:**
- Modify: `src/worker/mod.rs`

- [ ] **Step 1: Add record_activity helper**

```rust
// In src/worker/mod.rs
use crate::db::activity_log::{self, ActivityLogRow};

fn record_activity(&self, event_type: &str, description: String) {
    let pool = self.db_pool.clone();
    let et = event_type.to_owned();
    let worker_id = self.worker_id.clone();
    tokio::spawn(async move {
        if let Some(p) = pool {
            let row = ActivityLogRow {
                id: 0,
                event_type: et,
                resource_type: "system".into(),
                resource_id: None,
                actor: format!("worker:{}", worker_id),
                description,
                details: None,
                recorded_at: chrono::Utc::now(),
            };
            let _ = activity_log::insert(&p, &row).await;
        }
    });
}
```

- [ ] **Step 2: Instrument claim_stream and release_all_claims**

```rust
// After successful claim:
self.record_activity(
    "worker.claimed",
    format!("Worker 认领流 {}", stream_id),
);

// After release:
self.record_activity(
    "worker.released",
    format!("Worker 释放所有认领 (共 {} 个流)", count),
);
```

- [ ] **Step 3: Compile and verify**

Run: `cargo check --package getframe-worker`
Expected: Compilation passes

- [ ] **Step 4: Commit**

```bash
git add src/worker/mod.rs
git commit -m "feat: instrument worker claim/release for activity log (UI-09)"
```

### Task 7: Frontend Types + API Client

**Files:**
- Create: `web/src/types/activity.ts`
- Create: `web/src/api/activity.ts`

- [ ] **Step 1: Write TypeScript types**

```typescript
// web/src/types/activity.ts
export interface ActivityEvent {
  id: number;
  event_type: string;
  resource_type: string;
  resource_id: string | null;
  actor: string;
  description: string;
  details: Record<string, unknown> | null;
  recorded_at: string;
}

export interface ActivityQuery {
  event_type?: string;
  resource_type?: string;
  actor?: string;
  search?: string;
  since?: string;
  until?: string;
  page?: number;
  page_size?: number;
}

export interface ActivityListResponse {
  items: ActivityEvent[];
  total: number;
  page: number;
  page_size: number;
}
```

- [ ] **Step 2: Write API client**

```typescript
// web/src/api/activity.ts
import { request } from "./client";
import type { ActivityEvent, ActivityQuery, ActivityListResponse } from "../types/activity";

function buildQuery(params: ActivityQuery): string {
  const search = new URLSearchParams();
  if (params.event_type) search.set("event_type", params.event_type);
  if (params.resource_type) search.set("resource_type", params.resource_type);
  if (params.actor) search.set("actor", params.actor);
  if (params.search) search.set("search", params.search);
  if (params.since) search.set("since", params.since);
  if (params.until) search.set("until", params.until);
  if (params.page) search.set("page", String(params.page));
  if (params.page_size) search.set("page_size", String(params.page_size));
  const qs = search.toString();
  return qs ? `?${qs}` : "";
}

export const activityApi = {
  list(params: ActivityQuery = {}): Promise<ActivityListResponse> {
    return request<ActivityListResponse>(`/activity${buildQuery(params)}`);
  },

  async exportCsv(params: ActivityQuery = {}): Promise<void> {
    const qs = buildQuery(params);
    const res = await fetch(`/api/v1/activity/export${qs}`);
    if (!res.ok) throw new Error("Export failed");
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `activity-log-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  },
};
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd web && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add web/src/types/activity.ts web/src/api/activity.ts
git commit -m "feat: add activity log types and API client (UI-09)"
```

### Task 8: Frontend Activity Page + Routing

**Files:**
- Create: `web/src/pages/ActivityLog.tsx`
- Modify: `web/src/App.tsx`
- Modify: `web/src/components/Layout.tsx`

- [ ] **Step 1: Write ActivityLog page component**

```typescript
// web/src/pages/ActivityLog.tsx
import { useState, useEffect, useCallback } from "react";
import { activityApi } from "../api/activity";
import type { ActivityEvent, ActivityQuery } from "../types/activity";

const EVENT_TYPE_OPTIONS = [
  { value: "", label: "全部类型" },
  { value: "stream.", label: "流操作" },
  { value: "task.", label: "任务操作" },
  { value: "auth.", label: "认证操作" },
  { value: "worker.", label: "Worker 操作" },
];

const RESOURCE_TYPE_OPTIONS = [
  { value: "", label: "全部资源" },
  { value: "stream", label: "流" },
  { value: "task", label: "任务" },
  { value: "user", label: "用户" },
  { value: "api_key", label: "API Key" },
  { value: "system", label: "系统" },
];

function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function resourceBadge(type: string): { label: string; color: string } {
  const map: Record<string, { label: string; color: string }> = {
    stream: { label: "流", color: "bg-blue-100 text-blue-800" },
    task: { label: "任务", color: "bg-green-100 text-green-800" },
    user: { label: "用户", color: "bg-purple-100 text-purple-800" },
    api_key: { label: "API Key", color: "bg-orange-100 text-orange-800" },
    system: { label: "系统", color: "bg-gray-100 text-gray-800" },
  };
  return map[type] || { label: type, color: "bg-gray-100 text-gray-800" };
}

export default function ActivityLog() {
  const [items, setItems] = useState<ActivityEvent[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize] = useState(50);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Filters
  const [eventTypeFilter, setEventTypeFilter] = useState("");
  const [resourceTypeFilter, setResourceTypeFilter] = useState("");
  const [searchText, setSearchText] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");

  // Debounce search
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(searchText), 300);
    return () => clearTimeout(timer);
  }, [searchText]);

  const fetchData = useCallback(async (p: number) => {
    setLoading(true);
    setError(null);
    try {
      const query: ActivityQuery = {
        page: p,
        page_size: pageSize,
        search: debouncedSearch || undefined,
      };
      if (eventTypeFilter) query.event_type = eventTypeFilter;
      if (resourceTypeFilter) query.resource_type = resourceTypeFilter;

      const data = await activityApi.list(query);
      setItems(data.items);
      setTotal(data.total);
      setPage(data.page);
    } catch (e) {
      setError("加载活动日志失败");
      setItems([]);
    } finally {
      setLoading(false);
    }
  }, [eventTypeFilter, resourceTypeFilter, debouncedSearch, pageSize]);

  useEffect(() => {
    fetchData(1);
  }, [fetchData]);

  const totalPages = Math.ceil(total / pageSize);

  const handleExport = async () => {
    try {
      await activityApi.exportCsv({
        event_type: eventTypeFilter || undefined,
        resource_type: resourceTypeFilter || undefined,
        search: debouncedSearch || undefined,
      });
    } catch {
      // silent
    }
  };

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-bold">活动日志</h1>

      {/* Filter Bar */}
      <div className="flex flex-wrap gap-3 items-end">
        <select
          value={eventTypeFilter}
          onChange={(e) => { setEventTypeFilter(e.target.value); setPage(1); }}
          className="border rounded px-3 py-2 text-sm"
        >
          {EVENT_TYPE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </select>

        <select
          value={resourceTypeFilter}
          onChange={(e) => { setResourceTypeFilter(e.target.value); setPage(1); }}
          className="border rounded px-3 py-2 text-sm"
        >
          {RESOURCE_TYPE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </select>

        <input
          type="text"
          placeholder="搜索描述..."
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          className="border rounded px-3 py-2 text-sm flex-1 min-w-[200px]"
        />

        <button
          onClick={handleExport}
          className="bg-brand text-white rounded px-4 py-2 text-sm hover:opacity-90"
        >
          导出 CSV
        </button>
      </div>

      {/* Error State */}
      {error && (
        <div className="bg-red-50 text-red-700 rounded p-3 text-sm">
          {error}
          <button onClick={() => fetchData(page)} className="ml-3 underline">重试</button>
        </div>
      )}

      {/* Loading State */}
      {loading && (
        <div className="space-y-2">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="h-12 bg-gray-100 rounded animate-pulse" />
          ))}
        </div>
      )}

      {/* Empty State */}
      {!loading && !error && items.length === 0 && (
        <div className="text-center py-12 text-gray-500">暂无活动记录</div>
      )}

      {/* Timeline */}
      {!loading && items.length > 0 && (
        <div className="bg-white rounded shadow-sm">
          {items.map((item) => {
            const badge = resourceBadge(item.resource_type);
            return (
              <div key={item.id} className="flex items-center gap-3 px-4 py-3 border-b last:border-0 hover:bg-gray-50">
                <span className="text-gray-400 text-sm w-16 shrink-0">{formatTime(item.recorded_at)}</span>
                <span className={`text-xs font-medium px-2 py-0.5 rounded ${badge.color}`}>{badge.label}</span>
                <span className="flex-1 text-sm">{item.description}</span>
                <span className="text-xs text-gray-400 shrink-0">{item.actor}</span>
              </div>
            );
          })}
        </div>
      )}

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="flex justify-center items-center gap-4 text-sm">
          <button
            disabled={page <= 1}
            onClick={() => fetchData(page - 1)}
            className="px-3 py-1 border rounded disabled:opacity-50 hover:bg-gray-50"
          >
            ← 上一页
          </button>
          <span className="text-gray-500">第 {page}/{totalPages} 页</span>
          <button
            disabled={page >= totalPages}
            onClick={() => fetchData(page + 1)}
            className="px-3 py-1 border rounded disabled:opacity-50 hover:bg-gray-50"
          >
            下一页 →
          </button>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Add route in App.tsx**

```typescript
// In web/src/App.tsx, add import:
import ActivityLog from "./pages/ActivityLog";

// In the router, add route after /tasks/:id:
<Route path="/activity" element={<ActivityLog />} />
```

- [ ] **Step 3: Add nav link in Layout.tsx**

```typescript
// In web/src/components/Layout.tsx, add nav link after "任务管理":
<NavLink to="/activity" className={({ isActive }) => cn(
  isActive ? "text-brand font-medium" : "text-gray-600 hover:text-gray-900"
)}>
  活动日志
</NavLink>
```

- [ ] **Step 4: Verify build**

Run: `cd web && npx tsc --noEmit`
Expected: No TypeScript errors

- [ ] **Step 5: Commit**

```bash
git add web/src/pages/ActivityLog.tsx web/src/App.tsx web/src/components/Layout.tsx
git commit -m "feat: add activity log page with filters and CSV export (UI-09)"
```

### Task 9: E2E Verification

**Files:**
- Modify: `tests/e2e/test_full_flow.py`

- [ ] **Step 1: Add activity log checks to E2E test**

```python
# After stream registration and verification:
# Check activity log has stream.created event
resp = requests.get(f"{WORKER_API}/api/v1/activity?resource_type=stream&page_size=5", headers=auth_header)
data = resp.json()
assert data["total"] >= 1
created_events = [e for e in data["items"] if e["event_type"] == "stream.created"]
assert len(created_events) >= 1

# Check CSV export works
resp = requests.get(f"{WORKER_API}/api/v1/activity/export", headers=auth_header)
assert resp.status_code == 200
assert resp.text.startswith("id,event_type,")
```

- [ ] **Step 2: Run full E2E test**

Run: `ssh taplo@192.168.3.123 "cd /home/taplo/getframe && python3 tests/e2e/test_full_flow.py"`
Expected: All tests pass (15/15 with the new activity checks)

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/test_full_flow.py
git commit -m "test: add activity log verification to E2E tests (UI-09)"
```
