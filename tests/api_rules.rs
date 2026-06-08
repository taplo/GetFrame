mod common;

use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;
use http_body_util::BodyExt;
use getframe_worker::auth::AuthUser;

/// Streams created via the test helper always get a default Interval rule
/// (interval_seconds matches the config's extract_interval_seconds, default 5.0).
const DEFAULT_RULES: usize = 1;

fn with_auth(req: &mut Request<Body>, id: &str, username: &str, role: &str) {
    req.extensions_mut().insert(AuthUser {
        id: id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
    });
}

fn admin_post(uri: &str, body: Body) -> Request<Body> {
    let mut req = Request::post(uri)
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    with_auth(&mut req, "1", "admin", "admin");
    req
}

fn admin_put(uri: &str, body: Body) -> Request<Body> {
    let mut req = Request::put(uri)
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    with_auth(&mut req, "1", "admin", "admin");
    req
}

fn admin_delete(uri: &str) -> Request<Body> {
    let mut req = Request::delete(uri).body(Body::empty()).unwrap();
    with_auth(&mut req, "1", "admin", "admin");
    req
}

fn viewer_post(uri: &str, body: Body) -> Request<Body> {
    let mut req = Request::post(uri)
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    with_auth(&mut req, "2", "viewer", "viewer");
    req
}

async fn create_stream(app: &axum::Router, name: &str) -> String {
    let body = serde_json::json!({
        "config": {
            "name": name,
            "source_url": "file:///tmp/test.h264",
            "source_type": "file",
        }
    });
    let mut req = Request::post("/api/v1/streams")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    with_auth(&mut req, "1", "admin", "admin");
    let response = app.clone().oneshot(req).await.unwrap();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    created["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_api_rules_global_empty() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app
        .oneshot(Request::get("/api/v1/rules").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["rules"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_api_rules_global_with_rules() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream1 = create_stream(&app, "stream-1").await;
    let stream2 = create_stream(&app, "stream-2").await;

    // Each stream has 1 default rule → 2 default + 3 added = 5 total
    let rule1 = serde_json::json!({"rule": {"type": "interval", "interval_seconds": 5}});
    let _ = app.clone().oneshot(admin_post(
        &format!("/api/v1/streams/{}/rules", stream1),
        Body::from(serde_json::to_vec(&rule1).unwrap()),
    )).await.unwrap();

    let rule2 = serde_json::json!({"rule": {"type": "fps", "fps": 10}});
    let _ = app.clone().oneshot(admin_post(
        &format!("/api/v1/streams/{}/rules", stream1),
        Body::from(serde_json::to_vec(&rule2).unwrap()),
    )).await.unwrap();

    let rule3 = serde_json::json!({"rule": {"type": "scene_change", "threshold": 0.3}});
    let _ = app.clone().oneshot(admin_post(
        &format!("/api/v1/streams/{}/rules", stream2),
        Body::from(serde_json::to_vec(&rule3).unwrap()),
    )).await.unwrap();

    let response = app
        .oneshot(Request::get("/api/v1/rules").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let rules = body["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 2 * DEFAULT_RULES + 3);
}

#[tokio::test]
async fn test_api_rules_global_filter_stream() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream1 = create_stream(&app, "stream-1").await;
    let _stream2 = create_stream(&app, "stream-2").await;

    // stream1: 1 default + 1 added = 2. stream2: 1 default.
    let rule = serde_json::json!({"rule": {"type": "fps", "fps": 15}});
    let _ = app.clone().oneshot(admin_post(
        &format!("/api/v1/streams/{}/rules", stream1),
        Body::from(serde_json::to_vec(&rule).unwrap()),
    )).await.unwrap();

    let response = app
        .oneshot(Request::get(format!("/api/v1/rules?stream_id={}", stream1)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let rules = body["rules"].as_array().unwrap();
    assert_eq!(rules.len(), DEFAULT_RULES + 1);
    for r in rules {
        assert_eq!(r["stream_id"], stream1);
    }
}

#[tokio::test]
async fn test_api_rules_global_filter_type() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream1 = create_stream(&app, "stream-1").await;

    // 1 default interval + 2 custom: fps + interval = 3 total
    let rule1 = serde_json::json!({"rule": {"type": "fps", "fps": 10}});
    let _ = app.clone().oneshot(admin_post(
        &format!("/api/v1/streams/{}/rules", stream1),
        Body::from(serde_json::to_vec(&rule1).unwrap()),
    )).await.unwrap();

    let rule2 = serde_json::json!({"rule": {"type": "scene_change", "threshold": 0.3}});
    let _ = app.clone().oneshot(admin_post(
        &format!("/api/v1/streams/{}/rules", stream1),
        Body::from(serde_json::to_vec(&rule2).unwrap()),
    )).await.unwrap();

    // Filter by "fps" → 1 (the custom fps rule, not the default interval)
    let response = app
        .oneshot(Request::get("/api/v1/rules?type=fps").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let rules = body["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["rule"]["type"], "fps");
}

#[tokio::test]
async fn test_api_rules_global_invalid_uuid_filter() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let _stream1 = create_stream(&app, "stream-1").await;

    let response = app
        .oneshot(Request::get("/api/v1/rules?stream_id=not-a-uuid").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["rules"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_api_rules_per_stream_has_default() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_id = create_stream(&app, "test-stream").await;

    let response = app
        .oneshot(Request::get(format!("/api/v1/streams/{}/rules", stream_id)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["rules"].as_array().unwrap().len(), DEFAULT_RULES);
    assert_eq!(body["stream_id"], stream_id);
    assert_eq!(body["rules"][0]["type"], "interval");
}

#[tokio::test]
async fn test_api_rules_per_stream_not_found() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app
        .oneshot(
            Request::get("/api/v1/streams/00000000-0000-0000-0000-000000000000/rules")
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_api_rules_create() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_id = create_stream(&app, "test-stream").await;

    // Stream has 1 default rule (index 0), new rule gets index 1
    let body = serde_json::json!({"rule": {"type": "fps", "fps": 25}});
    let response = app
        .clone()
        .oneshot(admin_post(
            &format!("/api/v1/streams/{}/rules", stream_id),
            Body::from(serde_json::to_vec(&body).unwrap()),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 201);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(created["index"], DEFAULT_RULES);
    assert_eq!(created["rule"]["type"], "fps");
    assert_eq!(created["rule"]["fps"].as_f64().unwrap() as i32, 25);
}

#[tokio::test]
async fn test_api_rules_create_duplicate() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_id = create_stream(&app, "test-stream").await;

    // SceneChange duplicate detection works regardless of default rules
    let body = serde_json::json!({"rule": {"type": "scene_change", "threshold": 0.5}});
    let response = app.clone().oneshot(admin_post(
        &format!("/api/v1/streams/{}/rules", stream_id),
        Body::from(serde_json::to_vec(&body).unwrap()),
    )).await.unwrap();
    assert_eq!(response.status(), 201);

    let response = app.clone().oneshot(admin_post(
        &format!("/api/v1/streams/{}/rules", stream_id),
        Body::from(serde_json::to_vec(&body).unwrap()),
    )).await.unwrap();
    assert_eq!(response.status(), 409);
}

#[tokio::test]
async fn test_api_rules_create_not_found() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let body = serde_json::json!({"rule": {"type": "interval", "interval_seconds": 5}});
    let response = app.clone().oneshot(admin_post(
        "/api/v1/streams/00000000-0000-0000-0000-000000000000/rules",
        Body::from(serde_json::to_vec(&body).unwrap()),
    )).await.unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_api_rules_create_requires_admin() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_id = create_stream(&app, "test-stream").await;

    let body = serde_json::json!({"rule": {"type": "interval", "interval_seconds": 5}});
    let response = app.clone().oneshot(viewer_post(
        &format!("/api/v1/streams/{}/rules", stream_id),
        Body::from(serde_json::to_vec(&body).unwrap()),
    )).await.unwrap();
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_api_rules_get_by_index() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_id = create_stream(&app, "test-stream").await;

    // Default rule is at index 0
    let response = app
        .oneshot(Request::get(format!("/api/v1/streams/{}/rules/0", stream_id)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let rule: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(rule["index"], 0);
    assert_eq!(rule["rule"]["type"], "interval");
}

#[tokio::test]
async fn test_api_rules_get_by_index_not_found() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_id = create_stream(&app, "test-stream").await;

    // Only 1 default rule exists, so index 1 should 404
    let response = app
        .oneshot(Request::get(format!("/api/v1/streams/{}/rules/1", stream_id)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_api_rules_update() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_id = create_stream(&app, "test-stream").await;

    // Update default rule (index 0) from interval to fps
    let update_body = serde_json::json!({"rule": {"type": "fps", "fps": 30}});
    let response = app.clone().oneshot(admin_put(
        &format!("/api/v1/streams/{}/rules/0", stream_id),
        Body::from(serde_json::to_vec(&update_body).unwrap()),
    )).await.unwrap();
    assert_eq!(response.status(), 200);

    let response = app
        .oneshot(Request::get(format!("/api/v1/streams/{}/rules/0", stream_id)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let rule: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(rule["rule"]["type"], "fps");
    assert_eq!(rule["rule"]["fps"].as_f64().unwrap() as i32, 30);
}

#[tokio::test]
async fn test_api_rules_update_not_found() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_id = create_stream(&app, "test-stream").await;

    let body = serde_json::json!({"rule": {"type": "interval", "interval_seconds": 5}});
    let response = app.clone().oneshot(admin_put(
        &format!("/api/v1/streams/{}/rules/{}", stream_id, DEFAULT_RULES),
        Body::from(serde_json::to_vec(&body).unwrap()),
    )).await.unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_api_rules_delete_default_then_add() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let stream_id = create_stream(&app, "test-stream").await;

    // Remove the default rule (index 0), leaving 0 rules
    let response = app.clone()
        .oneshot(admin_delete(
        &format!("/api/v1/streams/{}/rules/0", stream_id),
    )).await.unwrap();
    assert_eq!(response.status(), 204);

    let response = app.clone()
        .oneshot(Request::get(format!("/api/v1/streams/{}/rules", stream_id)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["rules"].as_array().unwrap().len(), 0);

    // Add a new rule — gets index 0 since rules were empty
    let new_rule = serde_json::json!({"rule": {"type": "fps", "fps": 15}});
    let response = app.clone().oneshot(admin_post(
        &format!("/api/v1/streams/{}/rules", stream_id),
        Body::from(serde_json::to_vec(&new_rule).unwrap()),
    )).await.unwrap();
    assert_eq!(response.status(), 201);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let created: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(created["index"], 0);
}

#[tokio::test]
async fn test_api_rules_delete_not_found() {
    let pool = common::db::setup_db().await;
    common::db::cleanup_tables(&pool).await;
    let app = common::api::test_app(pool);

    let response = app.clone().oneshot(admin_delete(
        "/api/v1/streams/00000000-0000-0000-0000-000000000000/rules/0",
    )).await.unwrap();
    assert_eq!(response.status(), 404);
}
