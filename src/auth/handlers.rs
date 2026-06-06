use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::auth::{jwt, models, AuthState, AuthUser};

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    token: String,
    token_type: String,
    expires_in: u64,
}

pub async fn login_handler(
    State(state): State<AuthState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<serde_json::Value>)> {
    let user = models::find_user_by_username(&state.pool, &body.username)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))))?;

    let valid = models::verify_password(&body.password, &user.password_hash)
        .map_err(|_| (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))))?;

    if !valid {
        return Err((StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))));
    }

    let token = jwt::encode_jwt(&user.id, &user.username, &user.role, &state.jwt_secret, state.jwt_expiry)
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?;

    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer".to_string(),
        expires_in: state.jwt_expiry,
    }))
}

#[derive(Serialize)]
pub struct UserResponse {
    id: String,
    username: String,
    role: String,
    created_at: String,
}

pub async fn list_users_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<UserResponse>>, (StatusCode, Json<serde_json::Value>)> {
    if auth_user.role != "admin" {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "admin role required"}))));
    }

    let users = models::list_users(&state.pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?;

    Ok(Json(users.into_iter().map(|u| UserResponse {
        id: u.id,
        username: u.username,
        role: u.role,
        created_at: u.created_at.to_string(),
    }).collect()))
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    username: String,
    password: String,
    role: Option<String>,
}

#[derive(Serialize)]
pub struct CreateUserResponse {
    id: String,
    username: String,
    role: String,
}

pub async fn create_user_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<CreateUserResponse>), (StatusCode, Json<serde_json::Value>)> {
    if auth_user.role != "admin" {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "admin role required"}))));
    }

    if body.username.len() < 3 || body.username.len() > 64 {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "username must be 3-64 characters"}))));
    }

    if body.password.len() < 6 {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "password must be at least 6 characters"}))));
    }

    let role = body.role.as_deref().unwrap_or("viewer");
    if role != "admin" && role != "viewer" {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "role must be 'admin' or 'viewer'"}))));
    }

    let password_hash = models::hash_password(&body.password)
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?;

    let id = models::create_user(&state.pool, &body.username, &password_hash, role)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?;

    Ok((StatusCode::CREATED, Json(CreateUserResponse {
        id,
        username: body.username,
        role: role.to_string(),
    })))
}

pub async fn delete_user_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    if auth_user.role != "admin" {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "admin role required"}))));
    }

    if auth_user.id == id {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "cannot delete yourself"}))));
    }

    let deleted = models::delete_user(&state.pool, &id)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "user not found"}))))
    }
}

#[derive(Deserialize)]
pub struct ListApiKeysQuery {
    user_id: Option<String>,
}

#[derive(Serialize)]
pub struct ApiKeyResponse {
    id: String,
    user_id: String,
    key_prefix: String,
    name: String,
    last_used_at: Option<String>,
    expires_at: Option<String>,
    created_at: String,
}

pub async fn list_api_keys_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    Query(query): Query<ListApiKeysQuery>,
) -> Result<Json<Vec<ApiKeyResponse>>, (StatusCode, Json<serde_json::Value>)> {
    let target_user_id = if let Some(ref uid) = query.user_id {
        if auth_user.role != "admin" {
            return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "admin role required to list other user's keys"}))));
        }
        uid.clone()
    } else {
        auth_user.id.clone()
    };

    let keys = models::list_api_keys(&state.pool, &target_user_id)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?;

    Ok(Json(keys.into_iter().map(|k| ApiKeyResponse {
        id: k.id,
        user_id: k.user_id,
        key_prefix: k.key_prefix,
        name: k.name,
        last_used_at: k.last_used_at.map(|t| t.to_string()),
        expires_at: k.expires_at.map(|t| t.to_string()),
        created_at: k.created_at.to_string(),
    }).collect()))
}

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    name: Option<String>,
    user_id: Option<String>,
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    id: String,
    key: String,
    key_prefix: String,
    name: String,
}

pub async fn create_api_key_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), (StatusCode, Json<serde_json::Value>)> {
    let target_user_id = body.user_id.unwrap_or_else(|| auth_user.id.clone());

    if target_user_id != auth_user.id && auth_user.role != "admin" {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "admin role required to create key for other users"}))));
    }

    let name = body.name.unwrap_or_default();
    let (raw_key, key_hash, key_prefix) = models::generate_api_key();

    let id = models::create_api_key(&state.pool, &target_user_id, &key_hash, &key_prefix, &name)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?;

    Ok((StatusCode::CREATED, Json(CreateApiKeyResponse {
        id,
        key: raw_key,
        key_prefix,
        name,
    })))
}

pub async fn delete_api_key_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let row = sqlx::query_as::<_, (String, String)>("SELECT id, user_id FROM api_keys WHERE id = ?")
        .bind(&id)
        .fetch_optional(&*state.pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
        })?;

    match row {
        Some((_key_id, owner_id)) => {
            if owner_id != auth_user.id && auth_user.role != "admin" {
                return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"error": "not authorized to delete this key"}))));
            }

            let deleted = models::delete_api_key(&state.pool, &id)
                .await
                .map_err(|e| {
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
                })?;

            if deleted {
                Ok(StatusCode::NO_CONTENT)
            } else {
                Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "API key not found"}))))
            }
        }
        None => Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "API key not found"})))),
    }
}
