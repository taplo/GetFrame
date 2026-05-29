mod common;

use uuid::Uuid;
use chrono::Utc;
use getframe_worker::db::tasks::{load_all, upsert, delete};
use getframe_worker::task::registry::{TaskInfo, TaskStatus};

fn make_task(id: Uuid, status: TaskStatus) -> TaskInfo {
    TaskInfo {
        id,
        name: "test-task".into(),
        stream_id: Uuid::new_v4(),
        stream_name: "test-stream".into(),
        rules: vec![],
        status,
        frames_extracted: 0,
        created_at: Utc::now(),
        started_at: None,
        stopped_at: None,
    }
}

#[tokio::test]
async fn test_db_tasks_empty_list() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let tasks = load_all(&pool).await.unwrap();
    assert!(tasks.is_empty());
}

#[tokio::test]
async fn test_db_tasks_insert_and_list() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let id = Uuid::new_v4();
    let task = make_task(id, TaskStatus::Created);
    upsert(&pool, &task).await.unwrap();

    let tasks = load_all(&pool).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, id);
    assert_eq!(tasks[0].frames_extracted, 0);

    delete(&pool, &id).await.unwrap();
    assert!(load_all(&pool).await.unwrap().is_empty());
}

#[tokio::test]
async fn test_db_tasks_status_persistence() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let id = Uuid::new_v4();
    upsert(&pool, &make_task(id, TaskStatus::Running)).await.unwrap();

    let tasks = load_all(&pool).await.unwrap();
    assert!(matches!(tasks[0].status, TaskStatus::Running));
}

#[tokio::test]
async fn test_db_tasks_update_status() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;

    let id = Uuid::new_v4();
    upsert(&pool, &make_task(id, TaskStatus::Created)).await.unwrap();

    let running = TaskInfo {
        status: TaskStatus::Running,
        started_at: Some(Utc::now()),
        ..make_task(id, TaskStatus::Created)
    };
    upsert(&pool, &running).await.unwrap();

    let tasks = load_all(&pool).await.unwrap();
    assert!(matches!(tasks[0].status, TaskStatus::Running));
    assert!(tasks[0].started_at.is_some());
}
