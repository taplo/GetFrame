mod common;

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;
use http_body_util::BodyExt;

#[tokio::test]
async fn test_api_metrics_history_no_db() {
    use std::sync::Arc;
    let storage = Arc::new(getframe_worker::storage::StorageClient::noop());
    let kafka = Arc::new(getframe_worker::kafka::KafkaProducer::noop());
    let sm = getframe_worker::stream::StreamManager::new(storage, kafka);
    let tm = Arc::new(getframe_worker::task::TaskManager::new(Arc::new(sm.clone()), None));
    let app = getframe_worker::api::api_router(sm, tm, None);

    let response = app
        .oneshot(
            Request::get("/api/v1/metrics/history?minutes=30")
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_api_metrics_history_with_db() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app
        .oneshot(
            Request::get("/api/v1/metrics/history?minutes=30")
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["points"].is_array());
}
