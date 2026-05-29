mod common;

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;
use http_body_util::BodyExt;

#[tokio::test]
async fn test_api_list_streams_empty() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app
        .oneshot(Request::get("/api/v1/streams").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_api_get_stream_not_found() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app
        .oneshot(
            Request::get("/api/v1/streams/00000000-0000-0000-0000-000000000000")
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_api_create_and_get_stream() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let body = serde_json::json!({
        "config": {
            "name": "test-stream",
            "source_url": "file:///tmp/test.h264",
            "source_type": "file",
        }
    });
    let response = app.clone()
        .oneshot(
            Request::post("/api/v1/streams")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap()
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let stream_id = created["id"].as_str().unwrap().to_string();

    let response = app
        .oneshot(
            Request::get(&format!("/api/v1/streams/{}", stream_id))
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
}
