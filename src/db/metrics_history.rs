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
    #[allow(dead_code)]
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

pub async fn query_recent(pool: &MySqlPool, minutes: i64) -> Result<Vec<MetricsSnapshot>, sqlx::Error> {
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
    Ok(rows.into_iter().map(|r| MetricsSnapshot {
        frames_ps: r.frames_delta as f64 / 60.0,
        recorded_at: r.recorded_at,
        streams_active: r.streams_active,
        frames_delta: r.frames_delta,
        errors_decode: r.errors_decode,
        errors_storage: r.errors_storage,
        errors_kafka: r.errors_kafka,
        streams_claimed: r.streams_claimed,
    }).collect())
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
