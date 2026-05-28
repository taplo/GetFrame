#![allow(dead_code)]

use sqlx::{FromRow, MySqlPool};
use crate::pipeline::rule::RuleConfig;
use crate::task::registry::{TaskId, TaskInfo, TaskStatus};

#[derive(FromRow)]
struct TaskRow {
    id: uuid::Uuid,
    name: String,
    stream_id: uuid::Uuid,
    stream_name: String,
    status: String,
    rules: serde_json::Value,
    frames_extracted: i64,
    created_at: chrono::DateTime<chrono::Utc>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    stopped_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn load_all(pool: &MySqlPool) -> Result<Vec<TaskInfo>, sqlx::Error> {
    let rows = sqlx::query_as::<_, TaskRow>(
        r#"SELECT id, name, stream_id, stream_name, status, rules, frames_extracted,
                  created_at, started_at, stopped_at
           FROM tasks
           ORDER BY created_at"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().filter_map(|r| {
        let rules: Vec<RuleConfig> = serde_json::from_value(r.rules).unwrap_or_default();
        Some(TaskInfo {
            id: r.id,
            name: r.name,
            stream_id: r.stream_id,
            stream_name: r.stream_name,
            rules,
            status: parse_status(&r.status),
            frames_extracted: r.frames_extracted as u64,
            created_at: r.created_at,
            started_at: r.started_at,
            stopped_at: r.stopped_at,
        })
    }).collect())
}

pub async fn upsert(pool: &MySqlPool, task: &TaskInfo) -> Result<(), sqlx::Error> {
    let rules = serde_json::to_value(&task.rules).unwrap_or_default();
    let status_str = status_to_string(&task.status);

    sqlx::query(
        r#"INSERT INTO tasks (id, name, stream_id, stream_name, status, rules, frames_extracted,
                              created_at, started_at, stopped_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE
               name = VALUES(name), status = VALUES(status), rules = VALUES(rules),
               frames_extracted = VALUES(frames_extracted),
               started_at = VALUES(started_at), stopped_at = VALUES(stopped_at)"#
    )
    .bind(task.id.to_string())
    .bind(&task.name)
    .bind(task.stream_id.to_string())
    .bind(&task.stream_name)
    .bind(&status_str)
    .bind(rules)
    .bind(task.frames_extracted as i64)
    .bind(task.created_at)
    .bind(task.started_at)
    .bind(task.stopped_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete(pool: &MySqlPool, id: &TaskId) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

fn parse_status(s: &str) -> TaskStatus {
    match s {
        "Created" => TaskStatus::Created,
        "Running" => TaskStatus::Running,
        "Paused" => TaskStatus::Paused,
        "Stopped" => TaskStatus::Stopped,
        _ => TaskStatus::Error(s.to_string()),
    }
}

fn status_to_string(s: &TaskStatus) -> String {
    match s {
        TaskStatus::Created => "Created".into(),
        TaskStatus::Running => "Running".into(),
        TaskStatus::Paused => "Paused".into(),
        TaskStatus::Stopped => "Stopped".into(),
        TaskStatus::Error(e) => format!("Error:{}", e),
    }
}
