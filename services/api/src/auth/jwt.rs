use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JwtError {
    #[error("Token creation failed: {0}")]
    Creation(String),
    #[error("Invalid token: {0}")]
    InvalidToken(String),
    #[error("Token expired")]
    Expired,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,      // user id (UUID)
    pub username: String, // handle without @
    pub exp: i64,         // expiration timestamp (seconds)
    pub iat: i64,         // issued at timestamp (seconds)
}

pub fn generate_access_token(
    user_id: Uuid,
    username: &str,
    secret: &str,
    expiration_minutes: i64,
) -> Result<String, JwtError> {
    let now = Utc::now();
    let exp = (now + Duration::minutes(expiration_minutes)).timestamp();
    let iat = now.timestamp();

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        exp,
        iat,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| JwtError::Creation(e.to_string()))
}

pub fn validate_access_token(token: &str, secret: &str) -> Result<Claims, JwtError> {
    let validation = Validation::default();
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| JwtError::InvalidToken(e.to_string()))?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_generation_and_validation() {
        let secret = "test-jwt-secret-key-1234567890123456";
        let user_id = Uuid::new_v4();
        let username = "jaykon";

        let token = generate_access_token(user_id, username, secret, 15)
            .expect("Token generation should succeed");

        let claims =
            validate_access_token(&token, secret).expect("Token validation should succeed");
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.username, username);
    }
}
