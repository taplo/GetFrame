# Phase 8 — Web Dashboard & Monitoring Enhancement

## Scope

Medium richness (用户已选 B 方案)：Dashboard 增加 recharts 图表，TaskDetail 增加活动时间线。

---

## 1. Backend: Metrics History

### 1.1 Table `metrics_history`

```sql
CREATE TABLE metrics_history (
  id              BIGINT AUTO_INCREMENT PRIMARY KEY,
  recorded_at     TIMESTAMP(6) NOT NULL,
  streams_active  INT NOT NULL,
  frames_ps       DOUBLE NOT NULL,
  -- 以下四个字段为距上次采样（60s）的 delta，用于计算速率
  frames_delta    INT NOT NULL,
  errors_decode   INT NOT NULL,
  errors_storage  INT NOT NULL,
  errors_kafka    INT NOT NULL,
  streams_claimed INT NOT NULL
);
```

Retention: 7 days (数据清理在 MetricsRecorder 内每 1h 运行一次).

### 1.2 Background Recorder: `MetricsRecorder`

- 启动时获取 `PrometheusHandle` 引用
- 每 60s tick:
  1. 读取当前 counter/gauge 值
  2. 计算与上次采样值的 delta → `frames_delta`, `errors_decode` 等
  3. `frames_ps = frames_delta / 60.0`
  4. 写入 `metrics_history`
- 每 3600s tick: `DELETE FROM metrics_history WHERE recorded_at < NOW() - INTERVAL 7 DAY`

### 1.3 API: `GET /api/v1/metrics/history?minutes=30`

```json
{
  "points": [
    {
      "recorded_at": "2026-05-28T12:00:00Z",
      "streams_active": 42,
      "frames_ps": 15.3,
      "errors_decode": 5,
      "errors_storage": 2,
      "errors_kafka": 1,
      "streams_claimed": 40
    }
  ]
}
```

---

## 2. Backend: Task Events

### 2.1 Table `task_events`

```sql
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

`event_type` 取值: `Started`, `Paused`, `Resumed`, `Stopped`, `Error`

### 2.2 Event Recording Points

在以下位置插入 `task_events` INSERT:
- `TaskService::start()` → `Started`
- `TaskService::pause()` → `Paused`
- `TaskService::resume()` → `Resumed`
- `TaskService::stop()` → `Stopped`

> **注意:** Pipeline decode loop error 分支的 `Error` 事件记录已推迟 — 需要将 `task_id` 贯穿到 pipeline 层，涉及较大重构。

## Migration

### 5.1 SQL Migration

新增 `migrations/20260528_000001_metrics_and_events.sql`:
```sql
CREATE TABLE metrics_history ( ... );
CREATE TABLE task_events ( ... );
```

---

## 6. Implementation Order

1. Backend: migration + `MetricsHistory` struct + repository
2. Backend: `MetricsRecorder` background task
3. Backend: `GET /metrics/history` handler
4. Backend: task_events recording at state transition points
5. Backend: `GET /tasks/{id}/events` handler
6. Frontend: install recharts + add `metricsHistoryApi` + `taskEventsApi`
7. Frontend: Dashboard charts
8. Frontend: TaskDetail timeline
9. Sync to VM → Docker build → verify
