# API Authentication Design

**Date:** 2026-06-05
**Status:** Draft
**Requirement:** API-06 (JWT + API Key)

## Overview

Add dual authentication to GetFrame's REST API: **JWT Bearer tokens** for frontend users and **API Keys** for machine-to-machine communication. Both authenticate against a common user model stored in MySQL.

## Architecture

```
            ┌─────────────────────────┐
            │   axum::Router          │
            │                         │
            │  /health (public)       │
            │  /ready (public)        │
            │  /metrics (public)      │
            │  /swagger-ui (public)   │
            │                         │
            │  POST /api/v1/auth/     │
            │    login (public)       │
            │                         │
            │  ┌───────────────────┐  │
            │  │  AuthLayer        │  │
            │  │  (middleware)     │  │
            │  │  Bearer → JWT     │  │
            │  │  X-API-Key → DB   │  │
            │  └───────────────────┘  │
            │                         │
            │  /api/v1/streams/*      │
            │  /api/v1/rules/*        │
            │  /api/v1/tasks/*        │
            │  /api/v1/metrics/*      │
            │  /api/v1/auth/users/*   │
            │  /api/v1/auth/keys/*    │
            └─────────────────────────┘
```

## Module Structure

```
src/
  auth/
    ├── mod.rs          ← pub fn auth_router(), pub fn auth_layer()
    ├── middleware.rs   ← axum middleware (JWT + API Key dual check)
    ├── jwt.rs          ← JWT encode/decode, claims struct
    ├── models.rs       ← User, ApiKey structs (DB + API)
    └── handlers.rs     ← login, user CRUD, api key CRUD
```

## Authentication Flow

### JWT Bearer Token

1. Client sends `POST /api/v1/auth/login` with `{ "username": "...", "password": "..." }`
2. Server looks up user by username, verifies password with argon2
3. Server returns `{ "token": "<jwt>", "expires_in": 86400 }`
4. Client includes `Authorization: Bearer <jwt>` in subsequent requests
5. Middleware decodes JWT, extracts `user_id` + `role`, injects into request extensions

JWT claims:
```json
{
  "sub": "user-uuid",
  "role": "admin",
  "exp": 1717600000,
  "iat": 1717513600
}
```

### API Key

1. Admin generates key via `POST /api/v1/auth/api-keys` (requires JWT auth)
2. Server generates `gfk_<48-char-random>`, stores SHA-256 hash in DB
3. Server returns full key once (one-time display)
4. Client includes `X-API-Key: gfk_<...>` in requests
5. Middleware hashes the key, looks up in `api_keys` table by hash
6. Resolves to the owning user's `user_id` + `role`

### Middleware Resolution Order

```
Request arrives
├── Has Authorization: Bearer header?
│   ├── Yes → decode JWT → extract claims → inject AuthUser
│   └── No → fall through
├── Has X-API-Key header?
│   ├── Yes → SHA-256 hash → query api_keys table
│   │   ├── Found → resolve user → inject AuthUser
│   │   └── Not found → 401
│   └── No → 401 (if route requires auth)
└── Route is public?
    ├── Yes → allow
    └── No → 401
```

## Public Routes (no auth required)

- `GET /health`
- `GET /ready`
- `GET /metrics`
- `GET /swagger-ui/*`
- `GET /api-docs/openapi.json`
- `POST /api/v1/auth/login`

## Protected Routes (auth required)

All routes under `/api/v1/*` except `/api/v1/auth/login`.

### Role-Based Access

| Route | admin | viewer |
|-------|-------|--------|
| GET /api/v1/streams | ✓ | ✓ |
| POST /api/v1/streams | ✓ | ✗ |
| PUT /api/v1/streams/{id} | ✓ | ✗ |
| DELETE /api/v1/streams/{id} | ✓ | ✗ |
| GET /api/v1/rules | ✓ | ✓ |
| POST /api/v1/rules | ✓ | ✗ |
| PUT /api/v1/rules/{id} | ✓ | ✗ |
| DELETE /api/v1/rules/{id} | ✓ | ✗ |
| GET /api/v1/tasks | ✓ | ✓ |
| POST /api/v1/tasks | ✓ | ✗ |
| DELETE /api/v1/tasks/{id} | ✓ | ✗ |
| POST /api/v1/tasks/{id}/start| ✓ | ✗ |
| POST /api/v1/tasks/{id}/stop | ✓ | ✗ |
| GET /api/v1/metrics/history| ✓ | ✓ |
| POST /api/v1/auth/users | ✓ | ✗ |
| GET /api/v1/auth/users | ✓ | ✓ |
| DELETE /api/v1/auth/users/{id} | ✓ | ✗ |
| POST /api/v1/auth/api-keys | ✓ | ✗ |
| GET /api/v1/auth/api-keys | ✓ | ✓ |
| DELETE /api/v1/auth/api-keys/{id} | ✓ | ✗ |

## Database Schema

### Migration: `20260605_000001_api_auth.sql`

```sql
CREATE TABLE users (
    id CHAR(36) PRIMARY KEY,
    username VARCHAR(64) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(16) NOT NULL DEFAULT 'viewer',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE api_keys (
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

## Configuration

Add to `config.rs`:

```rust
pub struct AuthConfig {
    #[serde(default)]
    pub jwt_secret: String,     // falls back to JWT_SECRET env var, then auto-generate
    #[serde(default = "default_jwt_expiry")]
    pub jwt_expiry_seconds: u64,
}
```

```yaml
# config.yaml
auth:
  jwt_secret: "your-256-bit-secret"
  jwt_expiry_seconds: 86400
```

## API Endpoints

### POST /api/v1/auth/login

Request:
```json
{ "username": "admin", "password": "secret123" }
```

Response (200):
```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "expires_in": 86400,
  "user": { "id": "...", "username": "admin", "role": "admin" }
}
```

### POST /api/v1/auth/users

Request (admin only):
```json
{ "username": "operator1", "password": "secure-pass", "role": "viewer" }
```

Response (201):
```json
{ "id": "...", "username": "operator1", "role": "viewer", "created_at": "..." }
```

### POST /api/v1/auth/api-keys

Request (admin only):
```json
{ "name": "ci-cd-server" }
```

Response (201):
```json
{
  "id": "...",
  "name": "ci-cd-server",
  "key": "gfk_a1b2c3d4e5f6...",
  "created_at": "..."
}
```

> **Note:** The full `key` is returned only on creation. Save it immediately.

## Dependencies (Cargo.toml)

```toml
jsonwebtoken = "9"       # JWT encode/decode
argon2 = "0.5"           # password hashing
rand = "0.8"             # API Key generation
```

## Self-Bootstrap

The system needs at least one admin user to start. Approach:
1. If `users` table is empty on startup AND `auth.initial_admin_password` is set in config, create the default admin user automatically
2. Config field: `auth.initial_admin_password` (optional, default empty). If set and no users exist, create user `admin` with this password and role `admin`
3. If users table is empty and no initial password is configured, log a warning but still start (login impossible until someone creates a user via DB directly)

## Error Handling

- 401: Missing/invalid auth credentials
- 403: Valid auth but insufficient role (viewer trying to delete)
- 409: Username already exists (POST /auth/users)
- Standard error format: `{ "error": "message" }`

## Testing

- Unit tests for JWT encode/decode roundtrip
- Unit tests for password hash/verify
- Integration tests with axum test client:
  - Public routes return 200 without auth
  - Protected routes return 401 without auth
  - Protected routes return 200 with valid auth
  - Viewer cannot create/delete resources (403)
- API Key creation and usage roundtrip

## Security Considerations

- JWT secret should be at least 256 bits. Generate with `openssl rand -base64 32`
- API Keys are one-time display. Store only SHA-256 hash in DB
- No password returned in any API response
- Rate limiting on login endpoint (future work, not in scope)
- Audit logging for auth events (future work, not in scope)
