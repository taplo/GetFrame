mod common;

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;
use http_body_util::BodyExt;

#[tokio::test]
async fn test_api_health_endpoint() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "healthy");
}

#[tokio::test]
async fn test_api_ready_endpoint() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app
        .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}
