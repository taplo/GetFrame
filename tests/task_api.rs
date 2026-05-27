use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode, Method},
};
use serde_json::{json, Value};
use tower::ServiceExt;
use http_body_util::BodyExt;

async fn build_test_app() -> axum::Router {
    use getframe_worker::config::{StorageConfig, KafkaConfig};
    use getframe_worker::storage::StorageClient;
    use getframe_worker::kafka::KafkaProducer;
    use getframe_worker::stream::StreamManager;
    use getframe_worker::task::TaskManager;
    use getframe_worker::api;

    let storage_cfg = StorageConfig {
        bucket: "test-bucket".into(),
        endpoint_url: Some("http://localhost:9000".into()),
        region: Some("us-east-1".into()),
        access_key_id: Some("minioadmin".into()),
        secret_access_key: Some("minioadmin".into()),
        retention_days: None,
    };
    let storage = Arc::new(StorageClient::new(&storage_cfg).await);

    let kafka_cfg = KafkaConfig {
        brokers: "localhost:9092".into(),
        topic: "test-topic".into(),
        acks: "all".into(),
        compression: "none".into(),
        schema_registry_url: None,
        partition_key_field: None,
    };
    let kafka = Arc::new(KafkaProducer::new(&kafka_cfg).unwrap());

    let stream_manager = StreamManager::new(storage, kafka);
    let task_manager = Arc::new(TaskManager::new(Arc::new(stream_manager.clone()), None));
    api::api_router(stream_manager, task_manager)
}

async fn body_to_json(body: Body) -> Value {
    let collected = body.collect().await.unwrap();
    serde_json::from_slice(&collected.to_bytes()).unwrap()
}

async fn create_test_stream(app: &axum::Router) -> String {
    let config = json!({
        "config": {
            "name": "test-stream",
            "source_url": "rtsp://test:8554/test",
            "source_type": "rtsp",
            "extract_interval_seconds": 5.0,
            "jpeg_quality": 85,
            "ffmpeg_threads": 1,
            "rtsp_transport": "tcp"
        }
    });
    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::POST)
            .uri("/api/v1/streams")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&config).unwrap()))
            .unwrap()
    ).await.unwrap();
    let body: Value = body_to_json(response.into_body()).await;
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_task_lifecycle() {
    let app = build_test_app().await;
    let stream_id = create_test_stream(&app).await;

    let req_body = json!({
        "name": "lifecycle-task",
        "stream_id": stream_id,
        "rules": []
    });

    // POST /api/v1/tasks -> 201 Created
    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::POST)
            .uri("/api/v1/tasks")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&req_body).unwrap()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let task: Value = body_to_json(response.into_body()).await;
    let task_id = task["id"].as_str().unwrap().to_string();
    assert_eq!(task["status"], "Created");
    assert_eq!(task["name"], "lifecycle-task");

    // GET /api/v1/tasks/{id} -> 200 Created
    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::GET)
            .uri(format!("/api/v1/tasks/{}", task_id))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let task: Value = body_to_json(response.into_body()).await;
    assert_eq!(task["status"], "Created");

    // DELETE /api/v1/tasks/{id} -> 204
    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::DELETE)
            .uri(format!("/api/v1/tasks/{}", task_id))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_list_tasks() {
    let app = build_test_app().await;
    let stream_id = create_test_stream(&app).await;

    let req_body = json!({
        "name": "task-1",
        "stream_id": stream_id,
        "rules": []
    });

    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::POST)
            .uri("/api/v1/tasks")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&req_body).unwrap()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let req_body2 = json!({
        "name": "task-2",
        "stream_id": stream_id,
        "rules": []
    });

    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::POST)
            .uri("/api/v1/tasks")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&req_body2).unwrap()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // GET /api/v1/tasks -> 200 with 2 tasks
    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::GET)
            .uri("/api/v1/tasks")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = body_to_json(response.into_body()).await;
    let tasks = body["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 2);
}

#[tokio::test]
async fn test_get_nonexistent_task() {
    let app = build_test_app().await;
    let fake_id = uuid::Uuid::new_v4();

    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::GET)
            .uri(format!("/api/v1/tasks/{}", fake_id))
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_task_invalid_body() {
    let app = build_test_app().await;

    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::POST)
            .uri("/api/v1/tasks")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name": "incomplete"}"#))
            .unwrap()
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
