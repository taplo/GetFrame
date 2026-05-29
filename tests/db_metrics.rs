mod common;

use chrono::{Utc, Duration};
use getframe_worker::db::metrics_history::{MetricsPoint, insert, query_recent, cleanup_old};

fn make_point(recorded_at: chrono::DateTime<Utc>) -> MetricsPoint {
    MetricsPoint {
        recorded_at,
        streams_active: 5,
        frames_delta: 300,
        errors_decode: 1,
        errors_storage: 0,
        errors_kafka: 0,
        streams_claimed: 3,
    }
}

#[tokio::test]
async fn test_db_metrics_insert_and_query() {
    let pool = common::db::setup_db().await;

    insert(&pool, &make_point(Utc::now())).await.unwrap();

    let rows = query_recent(&pool, 60).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].streams_active, 5);
    assert!((rows[0].frames_ps - 5.0).abs() < 0.001);
}

#[tokio::test]
async fn test_db_metrics_query_time_range() {
    let pool = common::db::setup_db().await;

    insert(&pool, &make_point(Utc::now() - Duration::hours(2))).await.unwrap();
    insert(&pool, &make_point(Utc::now())).await.unwrap();

    let rows = query_recent(&pool, 60).await.unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn test_db_metrics_cleanup_old() {
    let pool = common::db::setup_db().await;

    insert(&pool, &make_point(Utc::now() - Duration::days(8))).await.unwrap();
    insert(&pool, &make_point(Utc::now())).await.unwrap();

    let all = query_recent(&pool, 99999).await.unwrap();
    assert_eq!(all.len(), 2);

    cleanup_old(&pool, 7).await.unwrap();

    let rows = query_recent(&pool, 99999).await.unwrap();
    assert_eq!(rows.len(), 1); // only the fresh point remains
}
