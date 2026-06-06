# Activity Log Viewer (UI-09) — Design Spec

**Date**: 2026-06-06
**Status**: Approved
**Version**: 1.0

## Overview

Add a unified activity log system that records all significant operations across GetFrame (stream CRUD, task lifecycle, auth events, worker actions) and presents them in a dedicated frontend page with filtering, search, and CSV export.

## Data Model

### New Table: `activity_log`

```sql
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

### Event Types

| event_type | resource_type | Trigger Point |
|-----------|---------------|--------------|
| `stream.created` | `stream` | StreamManager.add_stream |
| `stream.updated` | `stream` | StreamManager.update_stream_config |
| `stream.deleted` | `stream` | StreamManager.remove_stream |
| `stream.status_changed` | `stream` | Reconnection task / health check |
| `task.created` | `task` | TaskManager.create_task |
| `task.started` | `task` | TaskManager.start_task |
| `task.paused` | `task` | TaskManager.pause_task |
| `task.resumed` | `task` | TaskManager.resume_task |
| `task.stopped` | `task` | TaskManager.stop_task |
| `task.deleted` | `task` | TaskManager.delete_task |
| `task.error` | `task` | Pipeline decode/encode error |
| `auth.login` | `user` | Auth login handler |
| `auth.user_created` | `user` | Auth create_user handler |
| `auth.user_deleted` | `user` | Auth delete_user handler |
| `auth.api_key_created` | `api_key` | Auth create_api_key handler |
| `auth.api_key_deleted` | `api_key` | Auth delete_api_key handler |
| `worker.claimed` | `system` | WorkerManager.claim_stream |
| `worker.released` | `system` | WorkerManager.release_all_claims |

The existing `task_events` table is preserved unchanged. Task events are written to both `task_events` (for backward-compatible TaskDetail EventTimeline) and `activity_log` (for the unified view).

## Backend Implementation

### New Files

- `src/db/activity_log.rs` — DB access (insert, query with filtering, export)
- Register module in `src/db/mod.rs`

### `src/db/activity_log.rs`

```rust
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

pub struct ActivityLogQuery {
    pub event_type: Option<String>,
    pub resource_type: Option<String>,
    pub actor: Option<String>,
    pub search: Option<String>,       // LIKE on description
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub page: i64,
    pub page_size: i64,
}

pub async fn insert(pool: &MySqlPool, event: &ActivityLogRow) -> Result<(), sqlx::Error>;
pub async fn query(pool: &MySqlPool, q: &ActivityLogQuery) -> Result<(Vec<ActivityLogRow>, i64), sqlx::Error>;
pub async fn query_export(pool: &MySqlPool, q: &ActivityLogQuery) -> Result<Vec<ActivityLogRow>, sqlx::Error>;
```

### New API File

- `src/api/activity.rs` — 2 endpoints
- Register in `src/api/mod.rs`

#### `GET /api/v1/activity`

Query params: `event_type`, `resource_type`, `actor`, `search`, `since`, `until`, `page` (default 1), `page_size` (default 50).

Response:
```json
{
  "items": [
    {
      "id": 1,
      "event_type": "stream.created",
      "resource_type": "stream",
      "resource_id": "uuid",
      "actor": "admin",
      "description": "创建流 \"Camera-01\"",
      "details": null,
      "recorded_at": "2026-06-06T10:32:15Z"
    }
  ],
  "total": 100,
  "page": 1,
  "page_size": 50
}
```

#### `GET /api/v1/activity/export`

Same filters, returns CSV (Content-Type: text/csv). Headers: `id,event_type,resource_type,resource_id,actor,description,recorded_at`

### Instrumentation Points

- `src/stream/mod.rs`: `add_stream`, `remove_stream`, `update_stream_config`, health status transitions
- `src/task/mod.rs`: alongside each existing `record_event()` call (same 5 points)
- `src/auth/handlers.rs`: login success, user created/deleted, API key created/deleted
- `src/worker/mod.rs`: claim/release streams

### Migration

- `migrations/20260606_000001_activity_log.sql` — CREATE TABLE activity_log

## Frontend Implementation

### New Files

- `web/src/pages/ActivityLog.tsx` — main page
- `web/src/api/activity.ts` — API client
- `web/src/types/activity.ts` — TypeScript types

### Route & Navigation

- `App.tsx`: add `/activity` → `ActivityLog`
- `Layout.tsx`: add nav link «活动日志» after «任务管理»

### Page Layout

```
┌──────────────────────────────────────────────────┐
│ FilterBar                                         │
│  [event_type ▼] [resource_type ▼] [search...]     │
│  [since date] [until date] [导出 CSV]              │
├──────────────────────────────────────────────────┤
│ ActivityTimeline                                   │
│  10:32:15 │ [Stream] 创建流 "Camera-01"      admin │
│  10:31:02 │ [Task]   启动任务 "走廊抽帧"    system │
│  10:30:00 │ [Auth]   admin 登录              admin │
│  10:29:15 │ [Stream] 删除流 "Camera-03"      admin │
│  ...                                              │
│  Pagination: ← 上一页  第 1/5 页  下一页 →        │
└──────────────────────────────────────────────────┘
```

### Components

**FilterBar** — event_type Select (all/stream.*/task.*/auth.*/worker.*), resource_type Select (all/stream/task/user/system), Input search (debounced 300ms), date range inputs, Export button.

**ActivityTimeline** — vertical timeline. Each item: `HH:mm:ss` timestamp, colored badge by resource_type, description text, actor label.

**ExportButton** — re-fetches with current filters but no pagination, triggers CSV download via Blob.

### TypeScript Types

```typescript
interface ActivityEvent {
  id: number;
  event_type: string;
  resource_type: string;
  resource_id: string | null;
  actor: string;
  description: string;
  details: Record<string, unknown> | null;
  recorded_at: string;
}

interface ActivityQuery {
  event_type?: string;
  resource_type?: string;
  actor?: string;
  search?: string;
  since?: string;
  until?: string;
  page?: number;
  page_size?: number;
}

interface ActivityListResponse {
  items: ActivityEvent[];
  total: number;
  page: number;
  page_size: number;
}
```

### State & Loading

- Initial load: fetch with default filters, show skeleton
- Filter change: reset to page 1, re-fetch
- Search: debounced (300ms), reset to page 1
- Export: show loading spinner, trigger download
- Empty: «暂无活动记录»
- Error: inline error banner with retry

## Test Plan

### Backend Unit Tests

| Test | What it verifies |
|------|-----------------|
| `test_activity_log_insert` | Insert + query returns correct record |
| `test_activity_log_filter_type` | event_type filter works |
| `test_activity_log_filter_time` | Time range filter works |
| `test_activity_log_search` | description LIKE search works |
| `test_activity_log_pagination` | Page/page_size works with total count |
| `test_activity_log_export_csv` | CSV format has correct headers and rows |
| `test_stream_operation_records_event` | StreamManager.add_stream records activity |
| `test_auth_operation_records_event` | Login handler records activity |

### Frontend Tests

| Test | What it verifies |
|------|-----------------|
| `ActivityLog renders` | Page mounts without error |
| `ActivityLog empty state` | Shows "暂无活动记录" when no data |
| `ActivityLog filter change` | Changing filter triggers re-fetch |
| `ActivityLog pagination` | Page navigation works |

## Files Changed/Added

| File | Action |
|------|--------|
| `migrations/20260606_000001_activity_log.sql` | NEW |
| `src/db/activity_log.rs` | NEW |
| `src/db/mod.rs` | EDIT (register module) |
| `src/api/activity.rs` | NEW |
| `src/api/mod.rs` | EDIT (register route + openapi) |
| `src/stream/mod.rs` | EDIT (instrument add/update/remove) |
| `src/task/mod.rs` | EDIT (instrument alongside record_event) |
| `src/auth/handlers.rs` | EDIT (instrument login/user/api-key ops) |
| `src/worker/mod.rs` | EDIT (instrument claim/release) |
| `web/src/pages/ActivityLog.tsx` | NEW |
| `web/src/api/activity.ts` | NEW |
| `web/src/types/activity.ts` | NEW |
| `web/src/App.tsx` | EDIT (add route) |
| `web/src/components/Layout.tsx` | EDIT (add nav link) |
