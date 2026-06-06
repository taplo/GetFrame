use axum::{extract::State, Json};
use serde_json::Value;
use crate::auth::AuthState;

pub async fn login_handler(State(_state): State<AuthState>, Json(_body): Json<Value>) -> Json<Value> {
    Json(serde_json::json!({"status": "stub"}))
}

pub async fn list_users_handler(State(_state): State<AuthState>) -> Json<Value> {
    Json(serde_json::json!({"users": []}))
}

pub async fn create_user_handler(State(_state): State<AuthState>, Json(_body): Json<Value>) -> Json<Value> {
    Json(serde_json::json!({"status": "stub"}))
}

pub async fn delete_user_handler(State(_state): State<AuthState>, axum::extract::Path(_id): axum::extract::Path<String>) -> Json<Value> {
    Json(serde_json::json!({"status": "stub"}))
}

pub async fn list_api_keys_handler(State(_state): State<AuthState>) -> Json<Value> {
    Json(serde_json::json!({"keys": []}))
}

pub async fn create_api_key_handler(State(_state): State<AuthState>, Json(_body): Json<Value>) -> Json<Value> {
    Json(serde_json::json!({"status": "stub"}))
}

pub async fn delete_api_key_handler(State(_state): State<AuthState>, axum::extract::Path(_id): axum::extract::Path<String>) -> Json<Value> {
    Json(serde_json::json!({"status": "stub"}))
}