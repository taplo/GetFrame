use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use serde::Serialize;
use utoipa::ToSchema;
use crate::task::registry::{TaskId, TaskInfo, CreateTaskRequest};
use crate::task::{TaskManager, TaskError};

#[derive(Serialize, ToSchema)]
pub struct TaskListResponse {
    pub tasks: Vec<TaskInfo>,
}

pub fn task_routes(manager: Arc<TaskManager>) -> Router {
    Router::new()
        .route("/", axum::routing::get(list_tasks).post(create_task))
        .route("/{id}", axum::routing::get(get_task).delete(delete_task))
        .route("/{id}/start", axum::routing::post(start_task))
        .route("/{id}/pause", axum::routing::post(pause_task))
        .route("/{id}/resume", axum::routing::post(resume_task))
        .route("/{id}/stop", axum::routing::post(stop_task))
        .route("/{id}/events", axum::routing::get(get_task_events))
        .with_state(manager)
}

#[utoipa::path(
    get,
    path = "/api/v1/tasks",
    tag = "tasks",
    responses(
        (status = 200, description = "List of all extraction tasks", body = TaskListResponse),
    )
)]
pub async fn list_tasks(
    State(manager): State<Arc<TaskManager>>,
) -> Json<TaskListResponse> {
    let tasks = manager.list_tasks();
    Json(TaskListResponse { tasks })
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks",
    tag = "tasks",
    request_body = CreateTaskRequest,
    responses(
        (status = 201, description = "Task created", body = TaskInfo),
    )
)]
pub async fn create_task(
    State(manager): State<Arc<TaskManager>>,
    Json(req): Json<CreateTaskRequest>,
) -> (StatusCode, Json<TaskInfo>) {
    let task = manager.create_task(req);
    (StatusCode::CREATED, Json(task))
}

#[utoipa::path(
    get,
    path = "/api/v1/tasks/{id}",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "Task ID"),
    ),
    responses(
        (status = 200, description = "Task details", body = TaskInfo),
        (status = 404, description = "Task not found"),
    )
)]
pub async fn get_task(
    State(manager): State<Arc<TaskManager>>,
    Path(id): Path<TaskId>,
) -> Result<Json<TaskInfo>, (StatusCode, Json<serde_json::Value>)> {
    match manager.get_task(id) {
        Some(task) => Ok(Json(task)),
        None => Err(not_found(id)),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/tasks/{id}",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "Task ID"),
    ),
    responses(
        (status = 204, description = "Task deleted"),
        (status = 404, description = "Task not found"),
    )
)]
pub async fn delete_task(
    State(manager): State<Arc<TaskManager>>,
    Path(id): Path<TaskId>,
) -> StatusCode {
    if manager.delete_task(id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/{id}/start",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "Task ID"),
    ),
    responses(
        (status = 200, description = "Task started", body = TaskInfo),
        (status = 404, description = "Task not found"),
        (status = 409, description = "Invalid state transition"),
    )
)]
pub async fn start_task(
    State(manager): State<Arc<TaskManager>>,
    Path(id): Path<TaskId>,
) -> Result<Json<TaskInfo>, (StatusCode, Json<serde_json::Value>)> {
    manager.start_task(id)
        .map(Json)
        .map_err(map_task_error)
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/{id}/pause",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "Task ID"),
    ),
    responses(
        (status = 200, description = "Task paused", body = TaskInfo),
        (status = 404, description = "Task not found"),
        (status = 409, description = "Invalid state transition"),
    )
)]
pub async fn pause_task(
    State(manager): State<Arc<TaskManager>>,
    Path(id): Path<TaskId>,
) -> Result<Json<TaskInfo>, (StatusCode, Json<serde_json::Value>)> {
    manager.pause_task(id)
        .map(Json)
        .map_err(map_task_error)
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/{id}/resume",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "Task ID"),
    ),
    responses(
        (status = 200, description = "Task resumed", body = TaskInfo),
        (status = 404, description = "Task not found"),
        (status = 409, description = "Invalid state transition"),
    )
)]
pub async fn resume_task(
    State(manager): State<Arc<TaskManager>>,
    Path(id): Path<TaskId>,
) -> Result<Json<TaskInfo>, (StatusCode, Json<serde_json::Value>)> {
    manager.resume_task(id)
        .map(Json)
        .map_err(map_task_error)
}

#[utoipa::path(
    post,
    path = "/api/v1/tasks/{id}/stop",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "Task ID"),
    ),
    responses(
        (status = 200, description = "Task stopped", body = TaskInfo),
        (status = 404, description = "Task not found"),
        (status = 409, description = "Invalid state transition"),
    )
)]
pub async fn stop_task(
    State(manager): State<Arc<TaskManager>>,
    Path(id): Path<TaskId>,
) -> Result<Json<TaskInfo>, (StatusCode, Json<serde_json::Value>)> {
    manager.stop_task(id)
        .map(Json)
        .map_err(map_task_error)
}

#[derive(Serialize, ToSchema)]
pub struct TaskEventsResponse {
    pub events: Vec<TaskEventItem>,
}

#[derive(Serialize, ToSchema)]
pub struct TaskEventItem {
    pub event_type: String,
    pub event_data: Option<serde_json::Value>,
    pub recorded_at: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/tasks/{id}/events",
    tag = "tasks",
    params(
        ("id" = String, Path, description = "Task ID"),
    ),
    responses(
        (status = 200, description = "Task event history", body = TaskEventsResponse),
        (status = 503, description = "Database not available"),
    )
)]
pub async fn get_task_events(
    State(manager): State<Arc<TaskManager>>,
    Path(id): Path<TaskId>,
) -> Result<Json<TaskEventsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let pool = match &manager.db_pool {
        Some(p) => p,
        None => return Err((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "no database"})))),
    };

    let rows = crate::db::task_events::query_by_task(pool, &id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))))?;

    let events = rows.into_iter().map(|r| TaskEventItem {
        event_type: r.event_type,
        event_data: r.event_data,
        recorded_at: r.recorded_at.to_rfc3339(),
    }).collect();

    Ok(Json(TaskEventsResponse { events }))
}

fn not_found(id: TaskId) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "Task not found",
            "task_id": id.to_string(),
        })),
    )
}

fn map_task_error(err: TaskError) -> (StatusCode, Json<serde_json::Value>) {
    match err {
        TaskError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": err.to_string() })),
        ),
        TaskError::InvalidTransition(msg) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": msg })),
        ),
        TaskError::Internal(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": msg })),
        ),
    }
}
