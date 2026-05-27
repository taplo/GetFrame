use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode, Method},
};
use serde_json::Value;
use tower::ServiceExt;
use http_body_util::BodyExt;

async fn build_app() -> axum::Router {
    use getframe_worker::config::{StorageConfig, KafkaConfig};
    use getframe_worker::storage::StorageClient;
    use getframe_worker::kafka::KafkaProducer;
    use getframe_worker::stream::StreamManager;
    use getframe_worker::task::TaskManager;

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

    use utoipa::OpenApi;
    use utoipa_swagger_ui::SwaggerUi;
    use getframe_worker::health;
    use getframe_worker::api;

    let health_state = health::HealthState::new(Some(Arc::new(stream_manager.registry().clone())));
    let health_router = health::health_router(health_state);
    let api_router = api::api_router(stream_manager, task_manager);
    let api_doc = getframe_worker::api::ApiDoc::openapi();

    health_router
        .merge(api_router)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_doc))
}

#[tokio::test]
async fn test_openapi_spec_returns_valid_json() {
    let app = build_app().await;

    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::GET)
            .uri("/api-docs/openapi.json")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = {
        let collected = response.into_body().collect().await.unwrap();
        serde_json::from_slice(&collected.to_bytes()).unwrap()
    };

    // Verify OpenAPI 3.0 version
    assert!(
        body["openapi"].as_str().unwrap_or("").starts_with("3."),
        "Expected OpenAPI 3.x version, got: {:?}",
        body["openapi"]
    );

    // Verify paths contain all endpoint groups
    let paths = body["paths"].as_object().expect("paths must be an object");
    let path_keys: Vec<&str> = paths.keys().map(|s| s.as_str()).collect();
    let path_str = path_keys.join(" ");

    assert!(path_str.contains("tasks"), "Expected 'tasks' in paths, got: {}", path_str);
    assert!(path_str.contains("streams"), "Expected 'streams' in paths, got: {}", path_str);

    // Verify components.schemas is present and non-empty
    let schemas = body["components"]["schemas"].as_object()
        .expect("components.schemas must be an object");
    assert!(!schemas.is_empty(), "components.schemas should not be empty");
}

#[tokio::test]
async fn test_swagger_ui_serves_html() {
    let app = build_app().await;

    let response = app.clone().oneshot(
        Request::builder()
            .method(Method::GET)
            .uri("/swagger-ui/")
            .body(Body::empty())
            .unwrap()
    ).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let collected = response.into_body().collect().await.unwrap();
    let body_bytes = collected.to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(body_str.contains("swagger"), "Swagger UI should contain 'swagger' in HTML");
}
