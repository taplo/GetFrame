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
    pub kafka_delta: i32,
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
    pub kafka_ps: f64,
    pub streams_claimed: i32,
}

pub async fn insert(pool: &MySqlPool, point: &MetricsPoint) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO metrics_history (recorded_at, streams_active, frames_delta,
              errors_decode, errors_storage, errors_kafka, kafka_delta, streams_claimed)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#
    )
    .bind(point.recorded_at)
    .bind(point.streams_active)
    .bind(point.frames_delta)
    .bind(point.errors_decode)
    .bind(point.errors_storage)
    .bind(point.errors_kafka)
    .bind(point.kafka_delta)
    .bind(point.streams_claimed)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn query_recent(pool: &MySqlPool, minutes: i64) -> Result<Vec<MetricsSnapshot>, sqlx::Error> {
    let rows = sqlx::query_as::<_, MetricsPoint>(
        r#"SELECT recorded_at, streams_active, frames_delta,
                  errors_decode, errors_storage, errors_kafka, kafka_delta, streams_claimed
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
        kafka_ps: r.kafka_delta as f64 / 60.0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn make_point(kafka_delta: i32, frames_delta: i32) -> MetricsPoint {
        MetricsPoint {
            recorded_at: Utc.with_ymd_and_hms(2026, 6, 3, 12, 0, 0).unwrap(),
            streams_active: 10,
            frames_delta,
            errors_decode: 0,
            errors_storage: 1,
            errors_kafka: 2,
            kafka_delta,
            streams_claimed: 8,
        }
    }

    #[test]
    fn test_metrics_point_defaults() {
        let p = make_point(5, 100);
        assert_eq!(p.kafka_delta, 5);
        assert_eq!(p.frames_delta, 100);
        assert_eq!(p.errors_kafka, 2);
    }

    #[test]
    fn test_metrics_snapshot_kafka_ps_computation() {
        let p = make_point(30, 60);
        let s = MetricsSnapshot {
            frames_ps: p.frames_delta as f64 / 60.0,
            recorded_at: p.recorded_at,
            streams_active: p.streams_active,
            frames_delta: p.frames_delta,
            errors_decode: p.errors_decode,
            errors_storage: p.errors_storage,
            errors_kafka: p.errors_kafka,
            kafka_ps: p.kafka_delta as f64 / 60.0,
            streams_claimed: p.streams_claimed,
        };
        assert!((s.kafka_ps - 0.5).abs() < f64::EPSILON);
        assert!((s.frames_ps - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_snapshot_zero_delta() {
        let p = make_point(0, 0);
        let s = MetricsSnapshot {
            frames_ps: p.frames_delta as f64 / 60.0,
            recorded_at: p.recorded_at,
            streams_active: p.streams_active,
            frames_delta: p.frames_delta,
            errors_decode: p.errors_decode,
            errors_storage: p.errors_storage,
            errors_kafka: p.errors_kafka,
            kafka_ps: p.kafka_delta as f64 / 60.0,
            streams_claimed: p.streams_claimed,
        };
        assert!((s.kafka_ps - 0.0).abs() < f64::EPSILON);
        assert!((s.frames_ps - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_metrics_snapshot_large_delta() {
        let p = make_point(600, 0);
        let s = MetricsSnapshot {
            frames_ps: p.frames_delta as f64 / 60.0,
            recorded_at: p.recorded_at,
            streams_active: p.streams_active,
            frames_delta: p.frames_delta,
            errors_decode: p.errors_decode,
            errors_storage: p.errors_storage,
            errors_kafka: p.errors_kafka,
            kafka_ps: p.kafka_delta as f64 / 60.0,
            streams_claimed: p.streams_claimed,
        };
        assert!((s.kafka_ps - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_from_row_mapping_matches_snapshot() {
        let p = make_point(45, 120);
        let r = MetricsSnapshot {
            frames_ps: p.frames_delta as f64 / 60.0,
            recorded_at: p.recorded_at,
            streams_active: p.streams_active,
            frames_delta: p.frames_delta,
            errors_decode: p.errors_decode,
            errors_storage: p.errors_storage,
            errors_kafka: p.errors_kafka,
            kafka_ps: p.kafka_delta as f64 / 60.0,
            streams_claimed: p.streams_claimed,
        };
        assert_eq!(r.streams_active, 10);
        assert_eq!(r.streams_claimed, 8);
        assert!((r.kafka_ps - 0.75).abs() < f64::EPSILON);
        assert!((r.frames_ps - 2.0).abs() < f64::EPSILON);
        assert_eq!(r.errors_decode, 0);
        assert_eq!(r.errors_storage, 1);
        assert_eq!(r.errors_kafka, 2);
    }
}
