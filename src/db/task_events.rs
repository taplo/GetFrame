#![allow(dead_code)]

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
