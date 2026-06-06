pub mod jwt;
pub mod models;
pub mod middleware;
pub mod handlers;

use std::sync::Arc;
use axum::Router;
use sqlx::MySqlPool;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub role: String,
}

#[derive(Debug, Clone)]
pub struct AuthState {
    pub pool: Arc<MySqlPool>,
    pub jwt_secret: Arc<String>,
    pub jwt_expiry: u64,
}

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for AuthUser {
    type Rejection = axum::http::StatusCode;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<AuthUser>().cloned().ok_or(axum::http::StatusCode::UNAUTHORIZED)
    }
}

pub fn auth_router(state: AuthState) -> Router {
    Router::new()
        .route("/api/v1/auth/login", axum::routing::post(handlers::login_handler))
        .route("/api/v1/auth/users", axum::routing::get(handlers::list_users_handler).post(handlers::create_user_handler))
        .route("/api/v1/auth/users/{id}", axum::routing::delete(handlers::delete_user_handler))
        .route("/api/v1/auth/api-keys", axum::routing::get(handlers::list_api_keys_handler).post(handlers::create_api_key_handler))
        .route("/api/v1/auth/api-keys/{id}", axum::routing::delete(handlers::delete_api_key_handler))
        .with_state(state)
}
