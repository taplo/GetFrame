use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::pipeline::rule::RuleConfig;
use crate::stream::StreamManager;
use crate::types::StreamId;

#[derive(Serialize, ToSchema)]
pub struct RulesResponse {
    #[schema(value_type = String)]
    pub stream_id: StreamId,
    pub rules: Vec<RuleConfig>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateRuleRequest {
    pub rule: RuleConfig,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateRuleRequest {
    pub rule: RuleConfig,
}

#[derive(Serialize, ToSchema)]
pub struct RuleOperationResponse {
    #[schema(value_type = String)]
    pub stream_id: StreamId,
    pub rule: RuleConfig,
    pub index: usize,
}

pub fn rules_routes(manager: StreamManager) -> Router {
    Router::new()
        .route("/", axum::routing::get(list_rules).post(create_rule))
        .route("/{index}", axum::routing::get(get_rule).put(update_rule).delete(delete_rule))
        .with_state(manager)
}

#[utoipa::path(
    get,
    path = "/api/v1/streams/{id}/rules",
    tag = "rules",
    params(
        ("id" = String, Path, description = "Stream ID"),
    ),
    responses(
        (status = 200, description = "List of rules for stream", body = RulesResponse),
        (status = 404, description = "Stream not found"),
    )
)]
pub async fn list_rules(
    State(manager): State<StreamManager>,
    Path(stream_id): Path<StreamId>,
) -> Result<Json<RulesResponse>, (StatusCode, Json<serde_json::Value>)> {
    let registry = manager.registry();
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let rules = registry.get_rules(&stream_id).unwrap_or_default();
    Ok(Json(RulesResponse { stream_id, rules }))
}

fn is_duplicate_rule(existing: &RuleConfig, new_rule: &RuleConfig) -> bool {
    matches!((existing, new_rule),
        (RuleConfig::SceneChange { .. }, RuleConfig::SceneChange { .. })
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/streams/{id}/rules",
    tag = "rules",
    params(
        ("id" = String, Path, description = "Stream ID"),
    ),
    request_body = CreateRuleRequest,
    responses(
        (status = 201, description = "Rule created", body = RuleOperationResponse),
        (status = 404, description = "Stream not found"),
        (status = 409, description = "Duplicate rule"),
    )
)]
pub async fn create_rule(
    State(manager): State<StreamManager>,
    Path(stream_id): Path<StreamId>,
    Json(req): Json<CreateRuleRequest>,
) -> Result<(StatusCode, Json<RuleOperationResponse>), (StatusCode, Json<serde_json::Value>)> {
    let registry = manager.registry();
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let mut rules = registry.get_rules(&stream_id).unwrap_or_default();
    if rules.iter().any(|r| is_duplicate_rule(r, &req.rule)) {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "A rule of this type already exists",
                "stream_id": stream_id.to_string(),
            })),
        ));
    }
    rules.push(req.rule.clone());
    registry.update_rules(&stream_id, rules);
    let rules = registry.get_rules(&stream_id).unwrap_or_default();
    let index = rules.len() - 1;
    Ok((
        StatusCode::CREATED,
        Json(RuleOperationResponse { stream_id, rule: req.rule, index }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/streams/{id}/rules/{index}",
    tag = "rules",
    params(
        ("id" = String, Path, description = "Stream ID"),
        ("index" = usize, Path, description = "Rule index"),
    ),
    responses(
        (status = 200, description = "Rule details", body = RuleOperationResponse),
        (status = 404, description = "Stream or rule not found"),
    )
)]
pub async fn get_rule(
    State(manager): State<StreamManager>,
    Path((stream_id, index)): Path<(StreamId, usize)>,
) -> Result<Json<RuleOperationResponse>, (StatusCode, Json<serde_json::Value>)> {
    let registry = manager.registry();
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let rules = registry.get_rules(&stream_id).unwrap_or_default();
    rules.get(index).map(|rule| Json(RuleOperationResponse {
        stream_id, rule: rule.clone(), index,
    })).ok_or_else(|| index_error(stream_id, index))
}

#[utoipa::path(
    put,
    path = "/api/v1/streams/{id}/rules/{index}",
    tag = "rules",
    params(
        ("id" = String, Path, description = "Stream ID"),
        ("index" = usize, Path, description = "Rule index"),
    ),
    request_body = UpdateRuleRequest,
    responses(
        (status = 200, description = "Rule updated", body = RuleOperationResponse),
        (status = 404, description = "Stream or rule not found"),
    )
)]
pub async fn update_rule(
    State(manager): State<StreamManager>,
    Path((stream_id, index)): Path<(StreamId, usize)>,
    Json(req): Json<UpdateRuleRequest>,
) -> Result<Json<RuleOperationResponse>, (StatusCode, Json<serde_json::Value>)> {
    let registry = manager.registry();
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let mut rules = registry.get_rules(&stream_id).unwrap_or_default();
    if index >= rules.len() {
        return Err(index_error(stream_id, index));
    }
    rules[index] = req.rule.clone();
    registry.update_rules(&stream_id, rules);
    Ok(Json(RuleOperationResponse { stream_id, rule: req.rule, index }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/streams/{id}/rules/{index}",
    tag = "rules",
    params(
        ("id" = String, Path, description = "Stream ID"),
        ("index" = usize, Path, description = "Rule index"),
    ),
    responses(
        (status = 204, description = "Rule deleted"),
        (status = 404, description = "Stream or rule not found"),
    )
)]
pub async fn delete_rule(
    State(manager): State<StreamManager>,
    Path((stream_id, index)): Path<(StreamId, usize)>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let registry = manager.registry();
    if !registry.exists(&stream_id) {
        return Err(not_found(stream_id));
    }
    let mut rules = registry.get_rules(&stream_id).unwrap_or_default();
    if index >= rules.len() {
        return Err(index_error(stream_id, index));
    }
    rules.remove(index);
    registry.update_rules(&stream_id, rules);
    Ok(StatusCode::NO_CONTENT)
}

fn not_found(stream_id: StreamId) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "Stream not found",
            "stream_id": stream_id.to_string(),
        })),
    )
}

fn index_error(stream_id: StreamId, index: usize) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": format!("Rule index {} not found", index),
            "stream_id": stream_id.to_string(),
        })),
    )
}
