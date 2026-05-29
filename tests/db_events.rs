mod common;

use uuid::Uuid;
use getframe_worker::db::task_events::{insert, query_by_task};

#[tokio::test]
async fn test_db_events_insert_and_query() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let task_id = Uuid::new_v4();

    insert(&pool, "Started", &task_id, None).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    insert(&pool, "Stopped", &task_id, None).await.unwrap();

    let events = query_by_task(&pool, &task_id).await.unwrap();
    assert_eq!(events.len(), 2);
    // ORDER BY recorded_at DESC — most recent first
    assert_eq!(events[0].event_type, "Stopped");
    assert_eq!(events[1].event_type, "Started");
    assert!(events[0].recorded_at >= events[1].recorded_at);
}

#[tokio::test]
async fn test_db_events_with_data() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let task_id = Uuid::new_v4();
    let data = Some(serde_json::json!({"reason": "user_requested", "by": "admin"}));

    insert(&pool, "Stopped", &task_id, data.clone()).await.unwrap();

    let events = query_by_task(&pool, &task_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "Stopped");
    assert_eq!(events[0].event_data, data);
}

#[tokio::test]
async fn test_db_events_empty_for_unknown_task() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let events = query_by_task(&pool, &Uuid::new_v4()).await.unwrap();
    assert!(events.is_empty());
}
