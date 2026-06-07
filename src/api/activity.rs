use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use serde::Serialize;
use utoipa::ToSchema;
use crate::db::activity_log::{self, ActivityLogRow, ActivityLogQuery};

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityListResponse {
    pub items: Vec<ActivityLogRow>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

pub fn activity_routes(pool: Option<sqlx::MySqlPool>) -> Router {
    if let Some(p) = pool {
        Router::new()
            .route("/", axum::routing::get(list_handler))
            .route("/export", axum::routing::get(export_handler))
            .with_state(p)
    } else {
        Router::new()
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct ActivityQueryParams {
    pub event_type: Option<String>,
    pub resource_type: Option<String>,
    pub actor: Option<String>,
    pub search: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/activity",
    params(
        ("event_type" = Option<String>, Query, description = "Filter by event type"),
        ("resource_type" = Option<String>, Query, description = "Filter by resource type"),
        ("actor" = Option<String>, Query, description = "Filter by actor"),
        ("search" = Option<String>, Query, description = "Search description"),
        ("since" = Option<String>, Query, description = "Start time (ISO 8601)"),
        ("until" = Option<String>, Query, description = "End time (ISO 8601)"),
        ("page" = Option<i64>, Query, description = "Page number"),
        ("page_size" = Option<i64>, Query, description = "Items per page"),
    ),
    responses(
        (status = 200, description = "Activity list", body = ActivityListResponse),
    ),
    tag = "Activity",
)]
pub async fn list_handler(
    State(pool): State<sqlx::MySqlPool>,
    Query(params): Query<ActivityQueryParams>,
) -> Result<Json<ActivityListResponse>, (StatusCode, Json<serde_json::Value>)> {
    let query = build_query(params);
    match activity_log::query(&pool, &query).await {
        Ok((items, total)) => Ok(Json(ActivityListResponse {
            items,
            total,
            page: query.page,
            page_size: query.page_size,
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Query failed: {}", e)})),
        )),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/activity/export",
    params(
        ("event_type" = Option<String>, Query, description = "Filter by event type"),
        ("resource_type" = Option<String>, Query, description = "Filter by resource type"),
        ("actor" = Option<String>, Query, description = "Filter by actor"),
        ("search" = Option<String>, Query, description = "Search description"),
        ("since" = Option<String>, Query, description = "Start time (ISO 8601)"),
        ("until" = Option<String>, Query, description = "End time (ISO 8601)"),
    ),
    responses(
        (status = 200, description = "CSV export", content_type = "text/csv"),
    ),
    tag = "Activity",
)]
pub async fn export_handler(
    State(pool): State<sqlx::MySqlPool>,
    Query(params): Query<ActivityQueryParams>,
) -> Result<axum::response::Response, (StatusCode, Json<serde_json::Value>)> {
    let query = build_query(params);
    match activity_log::query_export(&pool, &query, 100000).await {
        Ok(rows) => {
            let mut csv = String::from("id,event_type,resource_type,resource_id,actor,description,recorded_at\n");
            for row in rows {
                let desc = row.description.replace('"', "\"\"");
                csv.push_str(&format!(
                    "{},{},{},{},{},\"{}\",{}\n",
                    row.id,
                    row.event_type,
                    row.resource_type,
                    row.resource_id.unwrap_or_default(),
                    row.actor,
                    desc,
                    row.recorded_at.to_rfc3339(),
                ));
            }
            Ok((
                StatusCode::OK,
                [("content-type", "text/csv; charset=utf-8"), ("content-disposition", "attachment; filename=\"activity-log.csv\"")],
                csv,
            ).into_response())
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Export failed: {}", e)})),
        )),
    }
}

fn build_query(params: ActivityQueryParams) -> ActivityLogQuery {
    ActivityLogQuery {
        event_type: params.event_type,
        resource_type: params.resource_type,
        actor: params.actor,
        search: params.search,
        since: params.since.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc))
        }),
        until: params.until.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc))
        }),
        page: params.page.unwrap_or(1),
        page_size: params.page_size.unwrap_or(50),
    }
}
