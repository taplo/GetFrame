use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
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
        let secret = "test-secret-key-thats-at-least-32-characters-long!!!!";
        let token = encode_jwt("user-1", "admin", "admin", secret, 3600).unwrap();
        let claims = decode_jwt(&token, secret).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.role, "admin");
        assert_eq!(claims.username, "admin");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_jwt_rejects_bad_secret() {
        let secret = "correct-secret-key-thats-at-least-32-chars!!!!!!";
        let token = encode_jwt("user-1", "admin", "admin", secret, 3600).unwrap();
        assert!(decode_jwt(&token, "wrong-secret-key-thats-also-32-chars-but-wrong!").is_err());
    }
}
