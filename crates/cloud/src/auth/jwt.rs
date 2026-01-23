//! JWT token handling.

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

/// JWT claims.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID).
    pub sub: Uuid,
    /// Expiration time (Unix timestamp).
    pub exp: i64,
    /// Issued at (Unix timestamp).
    pub iat: i64,
}

impl Claims {
    /// Create new claims for a user.
    pub fn new(user_id: Uuid, expiry_secs: u64) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id,
            exp: (now + Duration::seconds(expiry_secs as i64)).timestamp(),
            iat: now.timestamp(),
        }
    }

    /// Get the user ID from claims.
    pub fn user_id(&self) -> Uuid {
        self.sub
    }
}

/// Create a JWT token for a user.
pub fn create_token(user_id: Uuid, secret: &str, expiry_secs: u64) -> Result<String, ApiError> {
    let claims = Claims::new(user_id, expiry_secs);
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| ApiError::internal(format!("Failed to create token: {e}")))
}

/// Verify a JWT token and return the claims.
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, ApiError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(ApiError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_verify_token() {
        let user_id = Uuid::new_v4();
        let secret = "test_secret_key_for_testing";

        let token = create_token(user_id, secret, 3600).unwrap();
        let claims = verify_token(&token, secret).unwrap();

        assert_eq!(claims.user_id(), user_id);
    }

    #[test]
    fn test_invalid_token() {
        let result = verify_token("invalid_token", "secret");
        assert!(result.is_err());
    }
}
