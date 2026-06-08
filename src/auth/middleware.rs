use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::auth::jwt;
use crate::auth::models;
use crate::auth::{AuthState, AuthUser};

pub fn is_public_path(path: &str) -> bool {
    path == "/health"
        || path == "/ready"
        || path == "/metrics"
        || path.starts_with("/swagger-ui")
        || path == "/api/v1/auth/login"
}

pub async fn auth_middleware(
    State(state): State<AuthState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path().to_string();
    if is_public_path(&path) {
        return next.run(req).await;
    }

    let api_key_header = req
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(ref key) = api_key_header {
        let key_hash = models::hash_api_key(key);
        match models::find_api_key_by_hash(&state.pool, &key_hash).await {
            Ok(Some(api_key_row)) => {
                match models::find_user_by_id(&state.pool, &api_key_row.user_id).await {
                    Ok(Some(user)) => {
                        let _ = models::update_api_key_last_used(&state.pool, &api_key_row.id).await;
                        let mut req = req;
                        req.extensions_mut().insert(AuthUser {
                            id: user.id,
                            username: user.username,
                            role: user.role,
                        });
                        return next.run(req).await;
                    }
                    _ => {
                        return (StatusCode::UNAUTHORIZED, "invalid API key").into_response();
                    }
                }
            }
            _ => {
                return (StatusCode::UNAUTHORIZED, "invalid API key").into_response();
            }
        }
    }

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(ref auth_val) = auth_header
        && let Some(token) = auth_val.strip_prefix("Bearer ") {
            match jwt::decode_jwt(token, &state.jwt_secret) {
                Ok(claims) => {
                    let mut req = req;
                    req.extensions_mut().insert(AuthUser {
                        id: claims.sub,
                        username: claims.username,
                        role: claims.role,
                    });
                    return next.run(req).await;
                }
                Err(_) => {
                    return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
                }
            }
    }

    (StatusCode::UNAUTHORIZED, "missing authorization").into_response()
}
