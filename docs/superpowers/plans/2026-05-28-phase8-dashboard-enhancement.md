# Phase 8 Dashboard Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add metrics history dashboard (recharts) and task event timeline to the web UI.

**Architecture:** Backend writes periodic metrics snapshots and task state transitions to MySQL tables; REST API exposes both; frontend uses recharts for Dashboard charts and a pure-CSS timeline for TaskDetail.

**Tech Stack:** Rust (axum, sqlx, metrics), React (recharts), MySQL 8.0

---

### Task 1: SQL Migration

**Files:**
- Create: `migrations/20260528_000001_metrics_and_events.sql`

- [ ] **Step 1: Create migration file**

```sql
CREATE TABLE metrics_history (
  id              BIGINT AUTO_INCREMENT PRIMARY KEY,
  recorded_at     TIMESTAMP(6) NOT NULL,
  streams_active  INT NOT NULL,
  frames_delta    INT NOT NULL,
  errors_decode   INT NOT NULL,
  errors_storage  INT NOT NULL,
  errors_kafka    INT NOT NULL,
  streams_claimed INT NOT NULL,
  INDEX idx_metrics_recorded (recorded_at)
);

CREATE TABLE task_events (
  id           BIGINT AUTO_INCREMENT PRIMARY KEY,
  task_id      CHAR(36) NOT NULL,
  event_type   VARCHAR(30) NOT NULL,
  event_data   JSON,
  recorded_at  TIMESTAMP(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  INDEX idx_task_events_task (task_id),
  INDEX idx_task_events_recorded (recorded_at)
);
```

- [ ] **Step 2: Register migration** (sqlx autodiscovers `./migrations`, no code change needed)

- [ ] **Step 3: Commit**

```bash
git add migrations/20260528_000001_metrics_and_events.sql
git commit -m "feat: add metrics_history and task_events tables"
```

---

### Task 2: DB repository for metrics_history

**Files:**
- Create: `src/db/metrics_history.rs`
- Modify: `src/db/mod.rs`

- [ ] **Step 1: Create `src/db/metrics_history.rs`**

```rust
use chrono::{DateTime, Utc};
use sqlx::{FromRow, MySqlPool};

#[derive(Debug, Clone, FromRow)]
pub struct MetricsPoint {
    pub recorded_at: DateTime<Utc>,
    pub streams_active: i32,
    pub frames_delta: i32,
    pub errors_decode: i32,
    pub errors_storage: i32,
    pub errors_kafka: i32,
    pub streams_claimed: i32,
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub recorded_at: DateTime<Utc>,
    pub streams_active: i32,
    pub frames_delta: i32,
    pub frames_ps: f64,
    pub errors_decode: i32,
    pub errors_storage: i32,
    pub errors_kafka: i32,
    pub streams_claimed: i32,
}

pub async fn insert(pool: &MySqlPool, point: &MetricsPoint) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO metrics_history (recorded_at, streams_active, frames_delta,
              errors_decode, errors_storage, errors_kafka, streams_claimed)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#
    )
    .bind(point.recorded_at)
    .bind(point.streams_active)
    .bind(point.frames_delta)
    .bind(point.errors_decode)
    .bind(point.errors_storage)
    .bind(point.errors_kafka)
    .bind(point.streams_claimed)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn query_recent(pool: &MySqlPool, minutes: i64) -> Result<Vec<MetricsPoint>, sqlx::Error> {
    let rows = sqlx::query_as::<_, MetricsPoint>(
        r#"SELECT recorded_at, streams_active, frames_delta,
                  errors_decode, errors_storage, errors_kafka, streams_claimed
           FROM metrics_history
           WHERE recorded_at >= NOW() - INTERVAL ? MINUTE
           ORDER BY recorded_at ASC"#
    )
    .bind(minutes)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn cleanup_old(pool: &MySqlPool, days: i32) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM metrics_history WHERE recorded_at < NOW() - INTERVAL ? DAY"
    )
    .bind(days)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
```

- [ ] **Step 2: Add module to `src/db/mod.rs`**

```rust
pub mod metrics_history;
```

Insert after the existing `pub mod tasks;` line.

- [ ] **Step 3: Commit**

```bash
git add src/db/metrics_history.rs src/db/mod.rs
git commit -m "feat: add metrics_history DB repository"
```

---

### Task 3: MetricsRecorder background task

**Files:**
- Modify: `src/metrics.rs` (add recorder module or inline function)

- [ ] **Step 1: Add recorder function to `src/metrics.rs`**

Append after the existing code:

```rust
use std::sync::Arc;
use tokio::time::interval;

pub struct MetricsRecorder {
    pool: sqlx::MySqlPool,
    handle: PrometheusHandle,
    last_frames: i64,
    last_decode: i64,
    last_storage: i64,
    last_kafka: i64,
}

impl MetricsRecorder {
    pub fn new(pool: sqlx::MySqlPool) -> Self {
        let handle = PROMETHEUS_HANDLE.clone();
        let raw = handle.render();
        let last_frames = Self::extract_counter(&raw, "getframe_frames_processed_total");
        let last_decode = Self::extract_counter(&raw, "getframe_decode_errors_total");
        let last_storage = Self::extract_counter(&raw, "getframe_storage_errors_total");
        let last_kafka = Self::extract_counter(&raw, "getframe_kafka_errors_total");
        Self { pool, handle, last_frames, last_decode, last_storage, last_kafka }
    }

    pub fn sample(&mut self) -> crate::db::metrics_history::MetricsPoint {
        use metrics_exporter_prometheus::PrometheusHandle;

        let raw = self.handle.render();
        let now = chrono::Utc::now();

        let frames = Self::extract_counter(&raw, "getframe_frames_processed_total");
        let dec = Self::extract_counter(&raw, "getframe_decode_errors_total");
        let st = Self::extract_counter(&raw, "getframe_storage_errors_total");
        let kaf = Self::extract_counter(&raw, "getframe_kafka_errors_total");
        let active = Self::extract_gauge(&raw, "getframe_streams_active") as i32;
        let claimed = Self::extract_gauge(&raw, "getframe_streams_claimed") as i32;

        let frames_delta = (frames - self.last_frames).max(0) as i32;
        let errors_decode = (dec - self.last_decode).max(0) as i32;
        let errors_storage = (st - self.last_storage).max(0) as i32;
        let errors_kafka = (kaf - self.last_kafka).max(0) as i32;

        self.last_frames = frames;
        self.last_decode = dec;
        self.last_storage = st;
        self.last_kafka = kaf;

        crate::db::metrics_history::MetricsPoint {
            recorded_at: now,
            streams_active: active,
            frames_delta,
            errors_decode,
            errors_storage,
            errors_kafka,
            streams_claimed: claimed,
        }
    }

    pub async fn run(mut self, shutdown: tokio_util::sync::CancellationToken) {
        let mut tick = interval(std::time::Duration::from_secs(60));
        let mut cleanup_tick = interval(std::time::Duration::from_secs(3600));

        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("MetricsRecorder shutting down");
                    break;
                }
                _ = tick.tick() => {
                    let point = self.sample();
                    if let Err(e) = crate::db::metrics_history::insert(&self.pool, &point).await {
                        tracing::error!(error = %e, "Failed to record metrics snapshot");
                    }
                }
                _ = cleanup_tick.tick() => {
                    match crate::db::metrics_history::cleanup_old(&self.pool, 7).await {
                        Ok(n) => tracing::debug!(deleted = n, "Cleaned old metrics"),
                        Err(e) => tracing::error!(error = %e, "Metrics cleanup failed"),
                    }
                }
            }
        }
    }

    fn extract_counter(raw: &str, name: &str) -> i64 {
        for line in raw.lines() {
            if line.starts_with(name) {
                if let Some(val) = line.split_whitespace().last() {
                    if let Ok(v) = val.parse::<f64>() {
                        return v as i64;
                    }
                }
            }
        }
        0
    }

    fn extract_gauge(raw: &str, name: &str) -> f64 {
        for line in raw.lines() {
            if line.starts_with(name) {
                if let Some(val) = line.split_whitespace().last() {
                    if let Ok(v) = val.parse::<f64>() {
                        return v;
                    }
                }
            }
        }
        0.0
    }
}
```

- [ ] **Step 2: Spawn recorder in `src/main.rs`**

After `task_manager` creation and before the router setup, add:

```rust
if let Some(ref pool) = db_pool {
    let recorder = metrics::MetricsRecorder::new(pool.clone());
    tokio::spawn(recorder.run(shutdown_token.child_token()));
    tracing::info!("MetricsRecorder started (every 60s)");
}
```

- [ ] **Step 3: Commit**

```bash
git add src/metrics.rs src/main.rs
git commit -m "feat: add MetricsRecorder background task"
```

---

### Task 4: Task events recording

**Files:**
- Modify: `src/task/mod.rs`

- [ ] **Step 1: Insert task_events in state transition methods**

In `start_task`, after `self.registry.update_status(&id, TaskStatus::Running);`, add:

```rust
self.record_event(id, "Started", None);
```

In `pause_task`, after `self.registry.update_status(&id, TaskStatus::Paused);`, add:

```rust
self.record_event(id, "Paused", None);
```

In `resume_task`, after `self.registry.update_status(&id, TaskStatus::Running);`, add:

```rust
self.record_event(id, "Resumed", None);
```

In `stop_task`, after `self.registry.update_status(&id, TaskStatus::Stopped);`, add:

```rust
self.record_event(id, "Stopped", None);
```

- [ ] **Step 2: Add `record_event` method to `TaskManager`**

```rust
fn record_event(&self, task_id: TaskId, event_type: &str, event_data: Option<serde_json::Value>) {
    let pool = self.db_pool.clone();
    let et = event_type.to_string();
    let ed = event_data;
    tokio::spawn(async move {
        if let Some(p) = pool {
            let _ = crate::db::task_events::insert(&p, &et, &task_id, ed).await;
        }
    });
}
```

- [ ] **Step 3: Commit**

```bash
git add src/task/mod.rs
git commit -m "feat: record task events on state transitions"
```

---

### Task 5: DB repository for task_events

**Files:**
- Create: `src/db/task_events.rs`
- Modify: `src/db/mod.rs`

- [ ] **Step 1: Create `src/db/task_events.rs`**

```rust
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, MySqlPool};
use crate::task::registry::TaskId;

#[derive(Debug, Clone, FromRow)]
pub struct TaskEventRow {
    pub id: i64,
    pub task_id: String,
    pub event_type: String,
    pub event_data: Option<JsonValue>,
    pub recorded_at: DateTime<Utc>,
}

pub async fn insert(
    pool: &MySqlPool,
    event_type: &str,
    task_id: &TaskId,
    event_data: Option<JsonValue>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO task_events (task_id, event_type, event_data) VALUES (?, ?, ?)"
    )
    .bind(task_id.to_string())
    .bind(event_type)
    .bind(event_data)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn query_by_task(pool: &MySqlPool, task_id: &TaskId) -> Result<Vec<TaskEventRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, TaskEventRow>(
        r#"SELECT id, task_id, event_type, event_data, recorded_at
           FROM task_events
           WHERE task_id = ?
           ORDER BY recorded_at DESC"#
    )
    .bind(task_id.to_string())
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 2: Add module to `src/db/mod.rs`**

```rust
pub mod task_events;
```

Insert after the `pub mod metrics_history;` line.

- [ ] **Step 3: Commit**

```bash
git add src/db/task_events.rs src/db/mod.rs
git commit -m "feat: add task_events DB repository"
```

---

### Task 6: API handlers for metrics and task events

**Files:**
- Create: `src/api/metrics.rs`
- Modify: `src/api/mod.rs`
- Modify: `src/api/tasks.rs`

- [ ] **Step 1: Create `src/api/metrics.rs`**

```rust
use std::sync::Arc;
use axum::{extract::Query, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_minutes")]
    minutes: i64,
}

fn default_minutes() -> i64 { 30 }

#[derive(Serialize)]
pub struct MetricsHistoryResponse {
    pub points: Vec<MetricsPointResponse>,
}

#[derive(Serialize)]
pub struct MetricsPointResponse {
    pub recorded_at: String,
    pub streams_active: i32,
    pub frames_ps: f64,
    pub errors_decode: i32,
    pub errors_storage: i32,
    pub errors_kafka: i32,
    pub streams_claimed: i32,
}

pub fn metrics_routes(pool: Arc<MySqlPool>) -> Router {
    Router::new()
        .route("/api/v1/metrics/history", axum::routing::get(history_handler))
        .with_state(pool)
}

pub async fn history_handler(
    State(pool): State<Arc<MySqlPool>>,
    Query(q): Query<HistoryQuery>,
) -> Json<MetricsHistoryResponse> {
    let rows = crate::db::metrics_history::query_recent(&pool, q.minutes)
        .await
        .unwrap_or_default();

    let points = rows.into_iter().map(|r| {
        MetricsPointResponse {
            recorded_at: r.recorded_at.to_rfc3339(),
            streams_active: r.streams_active,
            frames_ps: r.frames_delta as f64 / 60.0,
            errors_decode: r.errors_decode,
            errors_storage: r.errors_storage,
            errors_kafka: r.errors_kafka,
            streams_claimed: r.streams_claimed,
        }
    }).collect();

    Json(MetricsHistoryResponse { points })
}
```

- [ ] **Step 2: Add events handler to `src/api/tasks.rs`**

Append before the helper functions:

```rust
#[derive(Serialize, ToSchema)]
pub struct TaskEventsResponse {
    pub events: Vec<TaskEventItem>,
}

#[derive(Serialize, ToSchema)]
pub struct TaskEventItem {
    pub event_type: String,
    pub event_data: Option<serde_json::Value>,
    pub recorded_at: String,
}

pub async fn get_task_events(
    State(manager): State<Arc<TaskManager>>,
    Path(id): Path<TaskId>,
) -> Result<Json<TaskEventsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let pool = match &manager.db_pool {
        Some(p) => p,
        None => return Err((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "no database"})))),
    };

    let rows = crate::db::task_events::query_by_task(pool, &id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let events = rows.into_iter().map(|r| TaskEventItem {
        event_type: r.event_type,
        event_data: r.event_data,
        recorded_at: r.recorded_at.to_rfc3339(),
    }).collect();

    Ok(Json(TaskEventsResponse { events }))
}
```

Also add the route in `task_routes`:

```rust
.route("/{id}/events", axum::routing::get(get_task_events))
```

- [ ] **Step 3: Wire into `src/api/mod.rs`**

Add `mod metrics;` and update `api_router`:

```rust
use sqlx::MySqlPool;

pub fn api_router(manager: StreamManager, task_manager: Arc<TaskManager>, db_pool: Option<MySqlPool>) -> Router {
    let mut router = Router::new()
        .nest("/api/v1/streams", streams::stream_routes(manager.clone()))
        .nest("/api/v1/streams/{id}/rules", rules::rules_routes(manager))
        .nest("/api/v1/tasks", tasks::task_routes(task_manager));

    if let Some(pool) = db_pool {
        router = router.merge(metrics::metrics_routes(Arc::new(pool)));
    }

    router
}
```

- [ ] **Step 4: Update `src/main.rs` call to `api_router`**

Change:
```rust
let api_router = api::api_router(stream_manager.clone(), task_manager);
```
To:
```rust
let api_router = api::api_router(stream_manager.clone(), task_manager, db_pool.clone());
```

- [ ] **Step 5: Commit**

```bash
git add src/api/metrics.rs src/api/mod.rs src/api/tasks.rs src/main.rs
git commit -m "feat: add metrics/history and tasks/{id}/events API endpoints"
```

---

### Task 7: Frontend — install recharts and add API clients

**Files:**
- Modify: `web/package.json`
- Create: `web/src/api/metrics.ts`
- Modify: `web/src/api/tasks.ts`
- Create: `web/src/types/metrics.ts`

- [ ] **Step 1: Install recharts**

```bash
cd web && npm install recharts
```

- [ ] **Step 2: Create `web/src/types/metrics.ts`**

```typescript
export interface MetricsPoint {
  recorded_at: string
  streams_active: number
  frames_ps: number
  errors_decode: number
  errors_storage: number
  errors_kafka: number
  streams_claimed: number
}

export interface MetricsHistoryResponse {
  points: MetricsPoint[]
}
```

- [ ] **Step 3: Create `web/src/api/metrics.ts`**

```typescript
import { request } from "./client"
import type { MetricsHistoryResponse } from "@/types/metrics"

export const metricsApi = {
  history: (minutes = 30) =>
    request<MetricsHistoryResponse>(`/metrics/history?minutes=${minutes}`),
}
```

- [ ] **Step 4: Add events API to `web/src/api/tasks.ts`**

Append to the existing `tasksApi` object:

```typescript
export interface TaskEvent {
  event_type: string
  event_data: Record<string, unknown> | null
  recorded_at: string
}

export interface TaskEventsResponse {
  events: TaskEvent[]
}

// Inside tasksApi:
  events: (id: string) =>
    request<TaskEventsResponse>(`/tasks/${id}/events`),
```

- [ ] **Step 5: Commit**

```bash
git add web/package.json web/src/api/metrics.ts web/src/types/metrics.ts web/src/api/tasks.ts
git commit -m "feat(frontend): add recharts, metrics API, task events API"
```

---

### Task 8: Frontend — Dashboard charts

**Files:**
- Create: `web/src/components/MetricsChart.tsx`
- Modify: `web/src/pages/Dashboard.tsx`

- [ ] **Step 1: Create `web/src/components/MetricsChart.tsx`**

```tsx
import { useMemo } from "react"
import {
  LineChart, Line, BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip,
  ResponsiveContainer, Legend,
} from "recharts"
import type { MetricsPoint } from "@/types/metrics"

interface MetricsChartProps {
  points: MetricsPoint[]
}

export function MetricsChart({ points }: MetricsChartProps) {
  const data = useMemo(() => points.map((p) => ({
    time: new Date(p.recorded_at).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" }),
    active: p.streams_active,
    claimed: p.streams_claimed,
    fps: Math.round(p.frames_ps * 10) / 10,
    errDecode: p.errors_decode,
    errStorage: p.errors_storage,
    errKafka: p.errors_kafka,
  })), [points])

  if (data.length === 0) {
    return <div className="text-gray-400 text-sm text-center py-8">暂无指标数据</div>
  }

  return (
    <div className="grid grid-cols-2 gap-6">
      <div className="bg-white border rounded-xl p-5 shadow-sm">
        <h3 className="font-semibold mb-3">活跃流趋势</h3>
        <ResponsiveContainer width="100%" height={200}>
          <LineChart data={data}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="time" fontSize={12} />
            <YAxis fontSize={12} />
            <Tooltip />
            <Legend />
            <Line type="monotone" dataKey="active" stroke="#2563eb" name="活跃" strokeWidth={2} dot={false} />
            <Line type="monotone" dataKey="claimed" stroke="#7c3aed" name="已认领" strokeWidth={2} dot={false} />
          </LineChart>
        </ResponsiveContainer>
      </div>

      <div className="bg-white border rounded-xl p-5 shadow-sm">
        <h3 className="font-semibold mb-3">抽帧速率</h3>
        <ResponsiveContainer width="100%" height={200}>
          <LineChart data={data}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="time" fontSize={12} />
            <YAxis fontSize={12} />
            <Tooltip />
            <Legend />
            <Line type="monotone" dataKey="fps" stroke="#059669" name="帧/秒" strokeWidth={2} dot={false} />
          </LineChart>
        </ResponsiveContainer>
      </div>

      <div className="bg-white border rounded-xl p-5 shadow-sm col-span-2">
        <h3 className="font-semibold mb-3">错误率（60s 窗口）</h3>
        <ResponsiveContainer width="100%" height={200}>
          <BarChart data={data}>
            <CartesianGrid strokeDasharray="3 3" />
            <XAxis dataKey="time" fontSize={12} />
            <YAxis fontSize={12} />
            <Tooltip />
            <Legend />
            <Bar dataKey="errDecode" fill="#ef4444" name="解码" stackId="a" />
            <Bar dataKey="errStorage" fill="#f59e0b" name="存储" stackId="a" />
            <Bar dataKey="errKafka" fill="#6366f1" name="Kafka" stackId="a" />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Modify `web/src/pages/Dashboard.tsx`**

Add import and state:
```tsx
import { MetricsChart } from "@/components/MetricsChart"
import { metricsApi } from "@/api/metrics"
import type { MetricsPoint } from "@/types/metrics"
```

Add state:
```tsx
const [metrics, setMetrics] = useState<MetricsPoint[]>([])
```

Add fetch in load:
```tsx
metricsApi.history(30).then((res) => setMetrics(res.points)).catch(() => {})
```

Add chart between StatCards and recent lists:
```tsx
<MetricsChart points={metrics} />
```

Full modified Dashboard load function:
```tsx
const load = useCallback(() => {
  setRefreshing(true)
  setRefreshToken((t) => t + 1)
  Promise.all([
    streamsApi.list().then((res) => setStreams(res.streams)).catch(() => {}),
    tasksApi.list().then((res) => setTasks(res.tasks)).catch(() => {}),
    metricsApi.history(30).then((res) => setMetrics(res.points)).catch(() => {}),
  ]).finally(() => setRefreshing(false))
}, [])
```

- [ ] **Step 3: Commit**

```bash
git add web/src/components/MetricsChart.tsx web/src/pages/Dashboard.tsx
git commit -m "feat(frontend): add metrics charts to Dashboard"
```

---

### Task 9: Frontend — TaskDetail event timeline

**Files:**
- Create: `web/src/components/EventTimeline.tsx`
- Modify: `web/src/pages/TaskDetail.tsx`

- [ ] **Step 1: Create `web/src/components/EventTimeline.tsx`**

```tsx
import { Circle } from "lucide-react"
import type { TaskEvent } from "@/api/tasks"

interface EventTimelineProps {
  events: TaskEvent[]
}

const labelMap: Record<string, string> = {
  Started: "启动",
  Paused: "已暂停",
  Resumed: "已恢复",
  Stopped: "已停止",
  Error: "错误",
}

const colorMap: Record<string, string> = {
  Started: "text-green-600 border-green-400",
  Paused: "text-yellow-600 border-yellow-400",
  Resumed: "text-blue-600 border-blue-400",
  Stopped: "text-gray-600 border-gray-400",
  Error: "text-red-600 border-red-400",
}

export function EventTimeline({ events }: EventTimelineProps) {
  if (events.length === 0) {
    return <div className="text-gray-400 text-sm text-center py-8">暂无事件记录</div>
  }

  return (
    <div className="relative">
      {/* 垂直线 */}
      <div className="absolute left-4 top-2 bottom-2 w-0.5 bg-gray-200" />

      <div className="space-y-4">
        {events.map((ev, i) => (
          <div key={i} className="flex gap-4 pl-4 relative">
            <div className={`absolute left-2.5 top-1 w-3 h-3 rounded-full border-2 bg-white ${colorMap[ev.event_type] || "border-gray-300"}`} />
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className={`text-sm font-medium ${colorMap[ev.event_type]?.split(" ")[0] || "text-gray-700"}`}>
                  {labelMap[ev.event_type] || ev.event_type}
                </span>
                <span className="text-xs text-gray-400">
                  {new Date(ev.recorded_at).toLocaleString("zh-CN")}
                </span>
              </div>
              {ev.event_data && (
                <p className="text-xs text-gray-500 mt-0.5">
                  {ev.event_data.message || JSON.stringify(ev.event_data)}
                </p>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
```

- [ ] **Step 2: Modify `web/src/pages/TaskDetail.tsx`**

Add imports:
```tsx
import { EventTimeline } from "@/components/EventTimeline"
import { tasksApi, type TaskEvent } from "@/api/tasks"
```

Add state:
```tsx
const [events, setEvents] = useState<TaskEvent[]>([])
```

Add fetch in the useEffect:
```tsx
tasksApi.events(id).then((res) => setEvents(res.events)).catch(() => {})
```

Add timeline card after the two-column grid:
```tsx
<div className="bg-white border rounded-xl p-5 shadow-sm">
  <h2 className="font-semibold mb-3">事件时间线</h2>
  <EventTimeline events={events} />
</div>
```

- [ ] **Step 3: Commit**

```bash
git add web/src/components/EventTimeline.tsx web/src/pages/TaskDetail.tsx
git commit -m "feat(frontend): add event timeline to TaskDetail"
```

---

### Task 10: Sync to VM and Docker build

**Files:** N/A (deployment)

- [ ] **Step 1: SCP all changed files to VM**

```bash
scp Cargo.toml Cargo.lock taplo@127.0.0.1:/home/taplo/getframe/
scp -r src/ taplo@127.0.0.1:/home/taplo/getframe/
scp -r migrations/ taplo@127.0.0.1:/home/taplo/getframe/
scp -r web/ taplo@127.0.0.1:/home/taplo/getframe/
```

- [ ] **Step 2: Build Docker image on VM**

```bash
ssh taplo@127.0.0.1 'cd /home/taplo/getframe && docker compose down && docker buildx build --network host -t getframe-worker:latest . && docker compose up -d'
```

- [ ] **Step 3: Verify**

```bash
ssh taplo@127.0.0.1 'curl -s http://localhost:8080/health && echo "" && curl -s http://localhost:8080/api/v1/metrics/history?minutes=5 | head -c 200'
```

Expected: health OK + `{"points":[...]}` or `{"points":[]}`

- [ ] **Step 4: Commit plan and spec**

```bash
git add docs/superpowers/specs/2026-05-28-phase8-dashboard-enhancement-design.md docs/superpowers/plans/2026-05-28-phase8-dashboard-enhancement.md
git commit -m "docs: add Phase 8 design spec and implementation plan"
```
