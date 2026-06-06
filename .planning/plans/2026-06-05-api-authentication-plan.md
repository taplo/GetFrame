# API Authentication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add JWT Bearer and API Key dual authentication to GetFrame's REST API.

**Architecture:** New `src/auth/` module with 5 files (mod, middleware, jwt, models, handlers). AuthLayer as axum middleware checks `Authorization: Bearer` or `X-API-Key` headers, resolves to user + role injected as request extensions. Config-driven JWT secret, argon2 password hashing, SHA-256 hashed API keys in MySQL.

**Tech Stack:** jsonwebtoken 9, argon2 0.5, rand 0.8, sha2 0.10, axum middleware

---

### Task 1: Add dependencies, config, and DB migration

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/config.rs`
- Create: `migrations/20260605_000001_api_auth.sql`

- [ ] **Step 1: Add dependencies to Cargo.toml**

Add under `[dependencies]`:
```toml
jsonwebtoken = "9"
argon2 = "0.5"
sha2 = "0.10"
rand = "0.8"
```

- [ ] **Step 2: Add AuthConfig to src/config.rs**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub jwt_secret: String,
    #[serde(default = "default_jwt_expiry")]
    pub jwt_expiry_seconds: u64,
    #[serde(default)]
    pub initial_admin_password: String,
}

fn default_jwt_expiry() -> u64 { 86400 }
```

Add `auth` field to `Config`:
```rust
#[serde(default)]
pub auth: Option<AuthConfig>,
```

- [ ] **Step 3: Create DB migration**

`migrations/20260605_000001_api_auth.sql`:
```sql
CREATE TABLE IF NOT EXISTS users (
    id CHAR(36) PRIMARY KEY,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(16) NOT NULL DEFAULT 'viewer',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS api_keys (
    id CHAR(36) PRIMARY KEY,
    user_id CHAR(36) NOT NULL,
    key_hash VARCHAR(255) NOT NULL,
    key_prefix VARCHAR(8) NOT NULL,
    name VARCHAR(64) NOT NULL DEFAULT '',
    last_used_at TIMESTAMP NULL,
    expires_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    INDEX idx_key_hash (key_hash),
    INDEX idx_user_id (user_id)
);
```

- [ ] **Step 4: Verify compilation**

Run: `cd D:\projects\GetFrame && cargo check`
Expected: passes with only unused field warnings

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/config.rs migrations/20260605_000001_api_auth.sql
git commit -m "feat: add auth config, dependencies, and DB migration for API-06"
```

---

### Task 2: JWT module

**Files:**
- Create: `src/auth/jwt.rs`
- Create: `src/auth/mod.rs` (partial — just module decl + `AuthUser`)

- [ ] **Step 1: Create auth module directory and mod.rs**

`src/auth/mod.rs`:
```rust
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

pub fn auth_router(state: AuthState) -> Router {
    Router::new()
        .route("/api/v1/auth/login", axum::routing::post(handlers::login_handler))
        .route("/api/v1/auth/users", axum::routing::get(handlers::list_users_handler).post(handlers::create_user_handler))
        .route("/api/v1/auth/users/{id}", axum::routing::delete(handlers::delete_user_handler))
        .route("/api/v1/auth/api-keys", axum::routing::get(handlers::list_api_keys_handler).post(handlers::create_api_key_handler))
        .route("/api/v1/auth/api-keys/{id}", axum::routing::delete(handlers::delete_api_key_handler))
        .with_state(state)
}
```

- [ ] **Step 2: Create JWT module**

`src/auth/jwt.rs`:
```rust
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,       // user_id
    pub username: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn encode_jwt(user_id: &str, username: &str, role: &str, secret: &str, expiry_secs: u64) -> Result<String, jsonwebtoken::errors::Error> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        exp: now + expiry_secs as usize,
        iat: now,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())?;
    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_roundtrip() {
        let secret = "test-secret-key-256-bits-minimum-length-here-123456";
        let token = encode_jwt("user-1", "admin", "admin", secret, 3600).unwrap();
        let claims = decode_jwt(&token, secret).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.username, "admin");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_jwt_rejects_bad_secret() {
        let secret = "correct-secret-key-256-bits-minimum-length-here-12345";
        let token = encode_jwt("user-1", "admin", "admin", secret, 3600).unwrap();
        let wrong_secret = "wrong-secret-key-256-bits-minimum-length-here-12345";
        assert!(decode_jwt(&token, wrong_secret).is_err());
    }

    #[test]
    fn test_jwt_rejects_garbage() {
        let secret = "test-secret-key-256-bits-minimum-length-here-123456";
        assert!(decode_jwt("garbage-token", secret).is_err());
    }
}
```

- [ ] **Step 3: Run JWT tests**

Run: `cd D:\projects\GetFrame && cargo test auth::jwt::tests -v`
Expected: 3 passed

- [ ] **Step 4: Commit**

```bash
git add src/auth/jwt.rs src/auth/mod.rs
git commit -m "feat: add JWT encode/decode with tests"
```

---

### Task 3: Auth models + DB queries

**Files:**
- Create: `src/auth/models.rs`

- [ ] **Step 1: Create models module**

`src/auth/models.rs`:
```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use uuid::Uuid;

pub struct UserRow {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: chrono::NaiveDateTime,
}

pub struct ApiKeyRow {
    pub id: String,
    pub user_id: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub name: String,
    pub last_used_at: Option<chrono::NaiveDateTime>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed = PasswordHash::new(hash)?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn generate_api_key() -> (String, String) {
    let bytes: [u8; 32] = rand::Rng::random(rand::rngs::OsRng);
    let key = format!("gfk_{}", hex::encode(bytes));
    let prefix = key[..8].to_string();
    let hash = hash_api_key(&key);
    (key, hash, prefix)
}

pub async fn find_user_by_username(pool: &MySqlPool, username: &str) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, (String, String, String, String, chrono::NaiveDateTime)>(
        "SELECT id, username, password_hash, role, created_at FROM users WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .map(|row| row.map(|(id, username, password_hash, role, created_at)| UserRow { id, username, password_hash, role, created_at }))
}

pub async fn find_user_by_id(pool: &MySqlPool, id: &str) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, (String, String, String, String, chrono::NaiveDateTime)>(
        "SELECT id, username, password_hash, role, created_at FROM users WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map(|row| row.map(|(id, username, password_hash, role, created_at)| UserRow { id, username, password_hash, role, created_at }))
}

pub async fn find_api_key_by_hash(pool: &MySqlPool, key_hash: &str) -> Result<Option<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, (String, String, String, String, String, Option<chrono::NaiveDateTime>, Option<chrono::NaiveDateTime>, chrono::NaiveDateTime)>(
        "SELECT id, user_id, key_hash, key_prefix, name, last_used_at, expires_at, created_at FROM api_keys WHERE key_hash = ?"
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await
    .map(|row| row.map(|(id, user_id, key_hash, key_prefix, name, last_used_at, expires_at, created_at)| ApiKeyRow { id, user_id, key_hash, key_prefix, name, last_used_at, expires_at, created_at }))
}

pub async fn create_user(pool: &MySqlPool, username: &str, password_hash: &str, role: &str) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, ?, ?)")
        .bind(&id).bind(username).bind(password_hash).bind(role)
        .execute(pool).await?;
    Ok(id)
}

pub async fn delete_user(pool: &MySqlPool, id: &str) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("DELETE FROM users WHERE id = ?").bind(id).execute(pool).await?;
    Ok(r.rows_affected() > 0)
}

pub async fn list_users(pool: &MySqlPool) -> Result<Vec<UserRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, chrono::NaiveDateTime)>(
        "SELECT id, username, password_hash, role, created_at FROM users ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id, username, password_hash, role, created_at)| UserRow { id, username, password_hash, role, created_at }).collect())
}

pub async fn create_api_key(pool: &MySqlPool, user_id: &str, key_hash: &str, key_prefix: &str, name: &str) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO api_keys (id, user_id, key_hash, key_prefix, name) VALUES (?, ?, ?, ?, ?)")
        .bind(&id).bind(user_id).bind(key_hash).bind(key_prefix).bind(name)
        .execute(pool).await?;
    Ok(id)
}

pub async fn delete_api_key(pool: &MySqlPool, id: &str) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("DELETE FROM api_keys WHERE id = ?").bind(id).execute(pool).await?;
    Ok(r.rows_affected() > 0)
}

pub async fn list_api_keys(pool: &MySqlPool, user_id: &str) -> Result<Vec<ApiKeyRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<chrono::NaiveDateTime>, Option<chrono::NaiveDateTime>, chrono::NaiveDateTime)>(
        "SELECT id, user_id, key_hash, key_prefix, name, last_used_at, expires_at, created_at FROM api_keys WHERE user_id = ? ORDER BY created_at DESC"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id, user_id, key_hash, key_prefix, name, last_used_at, expires_at, created_at)| ApiKeyRow { id, user_id, key_hash, key_prefix, name, last_used_at, expires_at, created_at }).collect())
}

pub async fn update_api_key_last_used(pool: &MySqlPool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = ?").bind(id).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hash_roundtrip() {
        let hash = hash_password("hello123").unwrap();
        assert!(verify_password("hello123", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn test_api_key_format() {
        let (key, hash, prefix) = generate_api_key();
        assert!(key.starts_with("gfk_"));
        assert_eq!(key.len(), 52); // "gfk_" + 64 hex chars
        assert_eq!(hash_api_key(&key), hash);
    }

    #[test]
    fn test_hash_api_key_deterministic() {
        let h1 = hash_api_key("test-key");
        let h2 = hash_api_key("test-key");
        assert_eq!(h1, h2);
    }
}
```

- [ ] **Step 2: Add `hex` and `chrono` to Cargo.toml if not present**

Check if `hex` is already a dependency. If not:
```toml
hex = "0.4"
```
`chrono` should already be in the project (it's used in health.rs).

- [ ] **Step 3: Run auth model tests**

Run: `cd D:\projects\GetFrame && cargo test auth::models::tests -v`
Expected: 3 passed

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml src/auth/models.rs
git commit -m "feat: add auth DB models with password/API key hashing"
```

---

### Task 4: Auth middleware

**Files:**
- Create: `src/auth/middleware.rs`

- [ ] **Step 1: Create middleware**

`src/auth/middleware.rs`:
```rust
use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use crate::auth::{AuthState, AuthUser};
use crate::auth::jwt;
use crate::auth::models;

pub async fn auth_middleware(
    State(state): State<AuthState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    let method = req.method().clone();

    // Public routes
    if is_public(&method, path) {
        return Ok(next.run(req).await);
    }

    // Try Authorization: Bearer <jwt>
    if let Some(auth_val) = req.headers().get("Authorization").and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth_val.strip_prefix("Bearer ") {
            if let Ok(claims) = jwt::decode_jwt(token, &state.jwt_secret) {
                req.extensions_mut().insert(AuthUser {
                    id: claims.sub,
                    username: claims.username,
                    role: claims.role,
                });
                return Ok(next.run(req).await);
            }
        }
    }

    // Try X-API-Key
    if let Some(key_val) = req.headers().get("X-API-Key").and_then(|v| v.to_str().ok()) {
        let hash = models::hash_api_key(key_val);
        if let Ok(Some(api_key)) = models::find_api_key_by_hash(&state.pool, &hash).await {
            if let Ok(Some(user)) = models::find_user_by_id(&state.pool, &api_key.user_id).await {
                // Update last_used_at in background
                let pool = state.pool.clone();
                let ak_id = api_key.id.clone();
                tokio::spawn(async move {
                    let _ = models::update_api_key_last_used(&pool, &ak_id).await;
                });
                req.extensions_mut().insert(AuthUser {
                    id: user.id,
                    username: user.username,
                    role: user.role,
                });
                return Ok(next.run(req).await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

fn is_public(method: &axum::http::Method, path: &str) -> bool {
    // Health/metrics/swagger always public
    if path == "/health" || path == "/ready" || path == "/metrics" || path == "/" {
        return true;
    }
    if path.starts_with("/swagger-ui") || path.starts_with("/api-docs") {
        return true;
    }
    // POST /api/v1/auth/login is public
    if method == axum::http::Method::POST && path == "/api/v1/auth/login" {
        return true;
    }
    false
}
```

- [ ] **Step 2: Compile check**

Run: `cd D:\projects\GetFrame && cargo check`
Expected: passes

- [ ] **Step 3: Commit**

```bash
git add src/auth/middleware.rs
git commit -m "feat: add auth middleware (Bearer JWT + X-API-Key)"
```

---

### Task 5: Auth handlers

**Files:**
- Create: `src/auth/handlers.rs`

- [ ] **Step 1: Create auth handlers**

`src/auth/handlers.rs`:
```rust
use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use crate::auth::{AuthState, AuthUser, jwt, models};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    token: String,
    expires_in: u64,
    user: UserInfo,
}

#[derive(Serialize)]
pub struct UserInfo {
    id: String,
    username: String,
    role: String,
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    username: String,
    password: String,
    #[serde(default = "default_role")]
    role: String,
}

fn default_role() -> String { "viewer".to_string() }

#[derive(Serialize)]
pub struct UserResponse {
    id: String,
    username: String,
    role: String,
    created_at: String,
}

#[derive(Serialize)]
pub struct ApiKeyResponse {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    created_at: String,
}

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    name: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

pub async fn login_handler(
    State(state): State<AuthState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user = models::find_user_by_username(&state.pool, &req.username).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Database error".into() })))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: "Invalid credentials".into() })))?;

    let valid = models::verify_password(&req.password, &user.password_hash)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Password verification error".into() })))?;

    if !valid {
        return Err((StatusCode::UNAUTHORIZED, Json(ErrorResponse { error: "Invalid credentials".into() })));
    }

    let token = jwt::encode_jwt(&user.id, &user.username, &user.role, &state.jwt_secret, state.jwt_expiry)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Token generation failed".into() })))?;

    Ok(Json(LoginResponse {
        token,
        expires_in: state.jwt_expiry,
        user: UserInfo { id: user.id, username: user.username, role: user.role },
    }))
}

pub async fn require_admin(auth_user: &AuthUser) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if auth_user.role != "admin" {
        return Err((StatusCode::FORBIDDEN, Json(ErrorResponse { error: "Admin role required".into() })));
    }
    Ok(())
}

pub async fn create_user_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), (StatusCode, Json<ErrorResponse>)> {
    require_admin(&auth_user).await?;

    if req.username.len() < 3 || req.password.len() < 6 {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Username must be >= 3 chars, password >= 6 chars".into() })));
    }

    if !["admin", "viewer"].contains(&req.role.as_str()) {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Role must be admin or viewer".into() })));
    }

    let password_hash = models::hash_password(&req.password)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Password hashing failed".into() })))?;

    match models::create_user(&state.pool, &req.username, &password_hash, &req.role).await {
        Ok(id) => Ok((
            StatusCode::CREATED,
            Json(UserResponse {
                id, username: req.username, role: req.role,
                created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            }),
        )),
        Err(sqlx::Error::Database(db_err)) if db_err.constraint().is_some() => {
            Err((StatusCode::CONFLICT, Json(ErrorResponse { error: "Username already exists".into() })))
        }
        Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Database error".into() }))),
    }
}

pub async fn list_users_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<UserResponse>>, (StatusCode, Json<ErrorResponse>)> {
    require_admin(&auth_user).await?;
    let users = models::list_users(&state.pool).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Database error".into() })))?;
    Ok(Json(users.into_iter().map(|u| UserResponse {
        id: u.id, username: u.username, role: u.role,
        created_at: u.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    }).collect()))
}

pub async fn delete_user_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    require_admin(&auth_user).await?;
    if id == auth_user.id {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "Cannot delete yourself".into() })));
    }
    let deleted = models::delete_user(&state.pool, &id).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Database error".into() })))?;
    if deleted { Ok(StatusCode::NO_CONTENT) } else { Err((StatusCode::NOT_FOUND, Json(ErrorResponse { error: "User not found".into() }))) }
}

pub async fn create_api_key_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<ApiKeyResponse>), (StatusCode, Json<ErrorResponse>)> {
    require_admin(&auth_user).await?;
    let (key, hash, prefix) = models::generate_api_key();
    let id = models::create_api_key(&state.pool, &auth_user.id, &hash, &prefix, &req.name).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Database error".into() })))?;
    Ok((
        StatusCode::CREATED,
        Json(ApiKeyResponse {
            id, name: req.name, key: Some(key),
            created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        }),
    ))
}

pub async fn list_api_keys_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
) -> Result<Json<Vec<ApiKeyResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let keys = models::list_api_keys(&state.pool, &auth_user.id).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Database error".into() })))?;
    Ok(Json(keys.into_iter().map(|k| ApiKeyResponse {
        id: k.id, name: k.name, key: None,
        created_at: k.created_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    }).collect()))
}

pub async fn delete_api_key_handler(
    State(state): State<AuthState>,
    auth_user: AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    require_admin(&auth_user).await?;
    let deleted = models::delete_api_key(&state.pool, &id).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Database error".into() })))?;
    if deleted { Ok(StatusCode::NO_CONTENT) } else { Err((StatusCode::NOT_FOUND, Json(ErrorResponse { error: "API key not found".into() }))) }
}
```

- [ ] **Step 2: Add `serde` default for `password_hash` export in UserResponse**

Note: UserResponse intentionally excludes password_hash.

- [ ] **Step 3: Compile check**

Run: `cd D:\projects\GetFrame && cargo check`
Expected: passes

- [ ] **Step 4: Commit**

```bash
git add src/auth/handlers.rs
git commit -m "feat: add auth handlers (login, user CRUD, API key CRUD)"
```

---

### Task 6: Wire everything in main.rs + initial admin bootstrap

**Files:**
- Modify: `src/main.rs`
- Modify: `src/auth/mod.rs` (add `pub mod`)

- [ ] **Step 1: Add auth layer to existing API routes**

In `src/main.rs`, after reading config:
```rust
mod auth;
```

After creating the DB pool, add auth bootstrap:
```rust
// Auth initialization
let auth_state = if let Some(ref pool) = db_pool {
    if let Some(auth_cfg) = &config.auth {
        // Bootstrap: create initial admin if no users exist
        if !auth_cfg.initial_admin_password.is_empty() {
            let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
                .fetch_one(pool).await.unwrap_or((0,));
            if user_count.0 == 0 {
                let ph = crate::auth::models::hash_password(&auth_cfg.initial_admin_password)
                    .expect("Failed to hash initial admin password");
                sqlx::query("INSERT INTO users (id, username, password_hash, role) VALUES (?, ?, ?, ?)")
                    .bind(uuid::Uuid::new_v4().to_string())
                    .bind("admin")
                    .bind(&ph)
                    .bind("admin")
                    .execute(pool).await
                    .ok();
                tracing::info!("Created initial admin user (password from config)");
            }
        }
        let jwt_secret = if !auth_cfg.jwt_secret.is_empty() {
            auth_cfg.jwt_secret.clone()
        } else if let Ok(env_secret) = std::env::var("JWT_SECRET") {
            env_secret
        } else {
            let generated: String = (0..32).map(|_| { let c: u8 = rand::Rng::random(rand::rngs::OsRng); format!("{:02x}", c) }).collect();
            tracing::warn!("No JWT_SECRET configured; generated ephemeral secret. All tokens invalidated on restart.");
            generated
        };
        Some(crate::auth::AuthState {
            pool: pool.clone(),
            jwt_secret: Arc::new(jwt_secret),
            jwt_expiry: auth_cfg.jwt_expiry_seconds,
        })
    } else {
        None
    }
} else {
    None
};
```

Modify the app router construction:
```rust
let app = health_router
    .merge(api_router);

let app = if let Some(ref auth_state) = auth_state {
    let auth_router = crate::auth::auth_router(auth_state.clone());
    app.merge(auth_router)
        .layer(axum::middleware::from_fn_with_state(auth_state.clone(), crate::auth::middleware::auth_middleware))
} else {
    app
};

let app = app
    .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_doc))
    .route("/metrics", axum::routing::get(metrics::metrics_handler))
    .fallback_service(ServeDir::new("web/dist"));
```

- [ ] **Step 2: Update all existing handlers to accept optional auth**

For existing handlers, we need a way to extract AuthUser. Since the middleware injects it, handlers can extract it via:
```rust
use axum::Extension;
use crate::auth::AuthUser;

pub async fn list_streams(
    Extension(auth): Extension<AuthUser>,
    // ... other extractors
) -> impl IntoResponse {
```

But this requires ALL handlers to be modified. Instead, let me create an optional extractor that doesn't fail when auth is disabled:

In `src/auth/mod.rs`:
```rust
/// Implement FromRequestParts for AuthUser (extracts from extensions, injected by middleware)
impl<S: Send + Sync> axum::extract::FromRequestParts<S> for AuthUser {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut axum::http::request::Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<AuthUser>().cloned().ok_or(StatusCode::UNAUTHORIZED)
    }
}
```

And in handlers that don't strictly need it, we don't add the parameter. Only auth handlers (user CRUD, API key CRUD) need the auth_user parameter.

- [ ] **Step 3: Compile check with full build**

Run: `cd D:\projects\GetFrame && cargo build`
Expected: builds successfully

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/auth/mod.rs
git commit -m "feat: wire auth layer and initial admin bootstrap into main"
```

---

### Task 7: Update existing config files and test the full flow

**Files:**
- Modify: `benchmark/config/config.yaml`
- Modify: `config.docker.yaml`
- Modify: `config.example.yaml`

- [ ] **Step 1: Add auth section to config files**

In each config.yaml, add:
```yaml
auth:
  jwt_secret: "change-me-to-a-secure-random-string-at-least-32-chars"
  jwt_expiry_seconds: 86400
  initial_admin_password: "admin123"
```

- [ ] **Step 2: Build release binary**

Run: `cargo build --release --bin getframe-worker`
Expected: builds successfully

- [ ] **Step 3: Full E2E verification**

```bash
# On .123 VM
cd /home/taplo/getframe/benchmark
docker compose -f compose.yaml down -v
docker compose -f compose.yaml up -d
sleep 30

# Verify health still public
curl -s http://localhost:8080/health
# Expected: {"status":"healthy"...}

# Verify 401 without auth
curl -s -o /dev/null -w '%{http_code}' http://localhost:8080/api/v1/streams
# Expected: 401

# Login
TOKEN=$(curl -s -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}' | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])")

# Verify auth now works
curl -s -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/v1/streams
# Expected: 200

# Create API Key
KEY_JSON=$(curl -s -X POST http://localhost:8080/api/v1/auth/api-keys \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"test"}')
API_KEY=$(echo $KEY_JSON | python3 -c "import sys,json; print(json.load(sys.stdin)['key'])")

# Verify API Key works
curl -s -o /dev/null -w '%{http_code}' -H "X-API-Key: $API_KEY" http://localhost:8080/api/v1/streams
# Expected: 200

# Cleanup
docker compose -f compose.yaml down -v
```

- [ ] **Step 4: Commit**

```bash
git add benchmark/config/config.yaml config.docker.yaml config.example.yaml
git commit -m "feat: update config files with auth section"
```

---

### Task 8: Update E2E test suite

**Files:**
- Modify: `tests/e2e/test_full_flow.py`

- [ ] **Step 1: Add auth tests to E2E suite**

After the login step, add:
```python
def test_auth_flow():
    """Step 3: Verify authentication flow."""
    # Public endpoint
    health = requests.get(f"{BASE}/health")
    assert health.status_code == 200

    # Protected endpoint without auth -> 401
    r = requests.get(f"{BASE}/api/v1/streams")
    assert r.status_code == 401

    # Login
    r = requests.post(f"{BASE}/api/v1/auth/login", json={
        "username": "admin", "password": os.environ.get("ADMIN_PASSWORD", "admin123"),
    })
    assert r.status_code == 200
    token = r.json()["token"]
    assert len(token) > 20

    # Use token
    r = requests.get(f"{BASE}/api/v1/streams", headers={"Authorization": f"Bearer {token}"})
    assert r.status_code == 200

    # Create API key
    r = requests.post(f"{BASE}/api/v1/auth/api-keys", json={"name": "e2e-test"},
        headers={"Authorization": f"Bearer {token}"})
    assert r.status_code == 201
    api_key = r.json()["key"]
    assert api_key.startswith("gfk_")

    # Use API key
    r = requests.get(f"{BASE}/api/v1/streams", headers={"X-API-Key": api_key})
    assert r.status_code == 200
    return token, api_key
```

- [ ] **Step 2: Run E2E tests on .123**

```bash
cd /home/taplo/getframe
python3 tests/e2e/test_full_flow.py
```
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/test_full_flow.py
git commit -m "feat: add auth tests to E2E suite"
```

---

### Self-Review Checklist

- [ ] **Spec coverage:** Every section from the design doc has a corresponding task:
  - [x] JWT login / encode/decode → Task 2
  - [x] API Key generation + DB storage → Task 3
  - [x] Auth middleware (dual check) → Task 4
  - [x] Auth handlers (users CRUD, keys CRUD) → Task 5
  - [x] Initial admin bootstrap → Task 6
  - [x] Configuration → Task 1
  - [x] DB migration → Task 1
  - [x] E2E tests → Task 8
- [ ] **Placeholder check:** No TODOs, TBDs, or vague instructions
- [ ] **Type consistency:** AuthUser used consistently across all files
- [ ] **Role checking:** admin-only operations use `require_admin()` guard
