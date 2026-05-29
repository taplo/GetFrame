mod common;

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;
use http_body_util::BodyExt;

#[tokio::test]
async fn test_api_list_tasks_empty() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app
        .oneshot(Request::get("/api/v1/tasks").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_api_create_task() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_body = serde_json::json!({
        "config": {
            "name": "test-stream",
            "source_url": "file:///tmp/test.h264",
            "source_type": "file",
        }
    });
    let resp = app.clone()
        .oneshot(
            Request::post("/api/v1/streams")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&stream_body).unwrap()))
                .unwrap()
        )
        .await
        .unwrap();
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let stream_id = created["id"].as_str().unwrap();

    let task_body = serde_json::json!({
        "name": "test-task",
        "stream_id": stream_id,
        "rules": [],
    });
    let resp = app.clone()
        .oneshot(
            Request::post("/api/v1/tasks")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&task_body).unwrap()))
                .unwrap()
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
}

#[tokio::test]
async fn test_api_task_events_no_db() {
    use std::sync::Arc;
    let storage = Arc::new(getframe_worker::storage::StorageClient::noop());
    let kafka = Arc::new(getframe_worker::kafka::KafkaProducer::noop());
    let sm = getframe_worker::stream::StreamManager::new(storage, kafka);
    let tm = Arc::new(getframe_worker::task::TaskManager::new(Arc::new(sm.clone()), None));
    let app = getframe_worker::api::api_router(sm, tm, None);

    let response = app
        .oneshot(
            Request::get("/api/v1/tasks/00000000-0000-0000-0000-000000000000/events")
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 503);
}

#[tokio::test]
async fn test_api_task_events_with_db() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app
        .oneshot(
            Request::get("/api/v1/tasks/00000000-0000-0000-0000-000000000000/events")
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}
