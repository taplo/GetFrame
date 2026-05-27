use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::config::StreamConfig;
use crate::stream::health::StreamStatus;
use crate::stream::StreamManager;
use crate::types::StreamId;

#[derive(Serialize, ToSchema)]
pub struct StreamResponse {
    #[schema(value_type = String)]
    pub id: StreamId,
    pub name: String,
    pub source_url: String,
    pub source_type: String,
    pub status: String,
    pub tags: HashMap<String, String>,
    pub description: String,
    pub last_online: Option<String>,
    pub last_error: Option<String>,
    pub error_count: u64,
    pub uptime_seconds: u64,
    pub frames_decoded: u64,
    pub frames_extracted: u64,
    pub frames_per_hour: f64,
    pub reconnect_count: u64,
    pub latest_frame_key: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateStreamRequest {
    pub config: StreamConfig,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateStreamRequest {
    pub config: StreamConfig,
}

#[derive(Serialize, ToSchema)]
pub struct StreamListResponse {
    pub streams: Vec<StreamResponse>,
}

pub fn stream_routes(manager: StreamManager) -> Router {
    Router::new()
        .route("/", axum::routing::get(list_streams).post(create_stream))
        .route("/{id}", axum::routing::get(get_stream).put(update_stream).delete(delete_stream))
        .route("/{id}/test", axum::routing::post(test_connection))
        .route("/{id}/frames/latest", axum::routing::get(get_latest_frame))
        .with_state(manager)
}

#[utoipa::path(
    get,
    path = "/api/v1/streams",
    tag = "streams",
    responses(
        (status = 200, description = "List of all streams", body = StreamListResponse),
    )
)]
pub async fn list_streams(
    State(manager): State<StreamManager>,
) -> Json<StreamListResponse> {
    let streams = manager.registry().list();
    let responses: Vec<StreamResponse> = streams.into_iter()
        .map(to_response)
        .collect();
    Json(StreamListResponse { streams: responses })
}

#[utoipa::path(
    post,
    path = "/api/v1/streams",
    tag = "streams",
    request_body = CreateStreamRequest,
    responses(
        (status = 201, description = "Stream created", body = StreamResponse),
    )
)]
pub async fn create_stream(
    State(manager): State<StreamManager>,
    Json(req): Json<CreateStreamRequest>,
) -> (StatusCode, Json<StreamResponse>) {
    let id = manager.add_stream(req.config);
    let info = manager.registry().get(&id).unwrap();
    (StatusCode::CREATED, Json(to_response(info)))
}

#[utoipa::path(
    get,
    path = "/api/v1/streams/{id}",
    tag = "streams",
    params(
        ("id" = String, Path, description = "Stream ID"),
    ),
    responses(
        (status = 200, description = "Stream details", body = StreamResponse),
        (status = 404, description = "Stream not found"),
    )
)]
pub async fn get_stream(
    State(manager): State<StreamManager>,
    Path(id): Path<StreamId>,
) -> Result<Json<StreamResponse>, (StatusCode, Json<serde_json::Value>)> {
    match manager.registry().get(&id) {
        Some(info) => Ok(Json(to_response(info))),
        None => Err(not_found(id)),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/streams/{id}",
    tag = "streams",
    params(
        ("id" = String, Path, description = "Stream ID"),
    ),
    request_body = UpdateStreamRequest,
    responses(
        (status = 200, description = "Stream updated", body = StreamResponse),
        (status = 404, description = "Stream not found"),
    )
)]
pub async fn update_stream(
    State(manager): State<StreamManager>,
    Path(id): Path<StreamId>,
    Json(req): Json<UpdateStreamRequest>,
) -> Result<Json<StreamResponse>, (StatusCode, Json<serde_json::Value>)> {
    let registry = manager.registry();
    if !registry.exists(&id) {
        return Err(not_found(id));
    }
    manager.update_stream_config(&id, req.config);
    let info = registry.get(&id).unwrap();
    Ok(Json(to_response(info)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/streams/{id}",
    tag = "streams",
    params(
        ("id" = String, Path, description = "Stream ID"),
    ),
    responses(
        (status = 204, description = "Stream deleted"),
        (status = 404, description = "Stream not found"),
    )
)]
pub async fn delete_stream(
    State(manager): State<StreamManager>,
    Path(id): Path<StreamId>,
) -> StatusCode {
    if manager.remove_stream(&id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/streams/{id}/test",
    tag = "streams",
    params(
        ("id" = String, Path, description = "Stream ID"),
    ),
    responses(
        (status = 200, description = "Connection test result", body = serde_json::Value),
        (status = 404, description = "Stream not found"),
    )
)]
pub async fn test_connection(
    State(manager): State<StreamManager>,
    Path(id): Path<StreamId>,
) -> Json<serde_json::Value> {
    let info = match manager.registry().get(&id) {
        Some(info) => info,
        None => return Json(serde_json::json!({
            "error": "Stream not found",
            "stream_id": id.to_string(),
        })),
    };

    let url = info.config.source_url.clone();
    let start = std::time::Instant::now();

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::task::spawn_blocking(move || {
            ffmpeg_next::format::input(&url).map(|_| ())
        }),
    ).await;

    match result {
        Ok(Ok(Ok(()))) => {
            let latency_ms = start.elapsed().as_millis() as u64;
            Json(serde_json::json!({
                "reachable": true,
                "latency_ms": latency_ms,
                "message": "Connection successful"
            }))
        }
        Ok(Ok(Err(e))) => {
            Json(serde_json::json!({
                "reachable": false,
                "latency_ms": start.elapsed().as_millis() as u64,
                "error": e.to_string(),
                "message": "Connection failed"
            }))
        }
        Ok(Err(_)) => {
            Json(serde_json::json!({
                "reachable": false,
                "latency_ms": start.elapsed().as_millis() as u64,
                "error": "Internal error",
                "message": "Spawn blocking task failed"
            }))
        }
        Err(_) => {
            Json(serde_json::json!({
                "reachable": false,
                "latency_ms": 10000,
                "error": "Timeout",
                "message": "Connection timed out after 10 seconds"
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/streams/{id}/frames/latest",
    tag = "streams",
    params(
        ("id" = String, Path, description = "Stream ID"),
    ),
    responses(
        (status = 200, description = "Latest frame JPEG", content_type = "image/jpeg"),
        (status = 404, description = "Stream or frame not found"),
    )
)]
pub async fn get_latest_frame(
    State(manager): State<StreamManager>,
    Path(id): Path<StreamId>,
) -> Result<(StatusCode, [(&'static str, &'static str); 1], Vec<u8>), (StatusCode, Json<serde_json::Value>)> {
    let info = manager.registry().get(&id).ok_or_else(|| not_found(id))?;
    let key = info.health.latest_frame_key.clone()
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "No frames available yet"})),
        ))?;
    let st = manager.storage_client();
    match st.get_object_bytes(&key).await {
        Ok(bytes) => Ok((
            StatusCode::OK,
            [("content-type", "image/jpeg")],
            bytes,
        )),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Frame not found: {}", e)})),
        )),
    }
}

fn not_found(id: StreamId) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "Stream not found",
            "stream_id": id.to_string(),
        })),
    )
}

fn to_response(info: crate::stream::registry::StreamInfo) -> StreamResponse {
    let fph = if info.health.uptime_seconds > 0 {
        info.health.frames_extracted as f64 / (info.health.uptime_seconds as f64 / 3600.0)
    } else {
        0.0
    };
    StreamResponse {
        id: info.id,
        name: info.config.name,
        source_url: info.config.source_url,
        source_type: info.config.source_type,
        tags: info.config.tags,
        description: info.config.description,
        status: match &info.health.status {
            StreamStatus::Online => "online".to_string(),
            StreamStatus::Offline => "offline".to_string(),
            StreamStatus::Error(e) => format!("error: {}", e),
            StreamStatus::Connecting => "connecting".to_string(),
        },
        last_online: info.health.last_online.map(|t| t.to_rfc3339()),
        last_error: info.health.last_error.map(|t| t.to_rfc3339()),
        error_count: info.health.error_count,
        uptime_seconds: info.health.uptime_seconds,
        frames_decoded: info.health.frames_decoded,
        frames_extracted: info.health.frames_extracted,
        frames_per_hour: (fph * 10.0).round() / 10.0,
        reconnect_count: info.health.reconnect_count,
        latest_frame_key: info.health.latest_frame_key.clone(),
        created_at: info.created_at.to_rfc3339(),
    }
}
