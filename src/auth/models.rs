use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
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
    #[allow(dead_code)]
    pub key_hash: String,
    pub key_prefix: String,
    pub name: String,
    pub last_used_at: Option<chrono::NaiveDateTime>,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
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

pub fn generate_api_key() -> (String, String, String) {
    let bytes: [u8; 24] = rand::random();
    let key = format!("gfk_{}", hex::encode(bytes));
    let prefix = key[..8].to_string();
    let hash = hash_api_key(&key);
    (key, hash, prefix)
}

pub async fn find_user_by_username(pool: &MySqlPool, username: &str) -> Result<Option<UserRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, chrono::NaiveDateTime)>(
        "SELECT id, username, password_hash, role, created_at FROM users WHERE username = ?"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    Ok(rows.map(|(id, username, password_hash, role, created_at)| UserRow { id, username, password_hash, role, created_at }))
}

pub async fn find_user_by_id(pool: &MySqlPool, id: &str) -> Result<Option<UserRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, chrono::NaiveDateTime)>(
        "SELECT id, username, password_hash, role, created_at FROM users WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(rows.map(|(id, username, password_hash, role, created_at)| UserRow { id, username, password_hash, role, created_at }))
}

pub async fn find_api_key_by_hash(pool: &MySqlPool, key_hash: &str) -> Result<Option<ApiKeyRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<chrono::NaiveDateTime>, Option<chrono::NaiveDateTime>, chrono::NaiveDateTime)>(
        "SELECT id, user_id, key_hash, key_prefix, name, last_used_at, expires_at, created_at FROM api_keys WHERE key_hash = ?"
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await?;
    Ok(rows.map(|(id, user_id, key_hash, key_prefix, name, last_used_at, expires_at, created_at)| ApiKeyRow { id, user_id, key_hash, key_prefix, name, last_used_at, expires_at, created_at }))
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
        let (key, hash, _prefix) = generate_api_key();
        assert!(key.starts_with("gfk_"));
        assert_eq!(key.len(), 52);
        assert_eq!(hash_api_key(&key), hash);
    }

    #[test]
    fn test_hash_api_key_deterministic() {
        let h1 = hash_api_key("test-key");
        let h2 = hash_api_key("test-key");
        assert_eq!(h1, h2);
    }
}
