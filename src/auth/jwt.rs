//! JSON Web Token issuing and verification (HS256).

use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

const ISSUER: &str = "taskhub-api";

/// Claims embedded in every access token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject: the user id.
    pub sub: String,
    pub username: String,
    pub iss: String,
    /// Issued-at (unix seconds).
    pub iat: i64,
    /// Expiry (unix seconds).
    pub exp: i64,
}

/// Signs and verifies tokens. Holds key material, so it deliberately does
/// not implement `Debug`.
pub struct JwtManager {
    encoding: EncodingKey,
    decoding: DecodingKey,
    validation: Validation,
    expiry_seconds: i64,
}

impl JwtManager {
    pub fn new(secret: &[u8], expiry_seconds: i64) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[ISSUER]);
        validation.set_required_spec_claims(&["exp", "sub", "iss"]);
        validation.leeway = 5;
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
            validation,
            expiry_seconds,
        }
    }

    pub fn expiry_seconds(&self) -> i64 {
        self.expiry_seconds
    }

    /// Issue a signed access token for the given user.
    pub fn issue(&self, user_id: i64, username: &str) -> Result<String, AppError> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_owned(),
            iss: ISSUER.to_owned(),
            iat: now,
            exp: now + self.expiry_seconds,
        };
        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to sign JWT: {e}")))
    }

    /// Verify signature, issuer and expiry; return the claims.
    pub fn verify(&self, token: &str) -> Result<Claims, AppError> {
        decode::<Claims>(token, &self.decoding, &self.validation)
            .map(|data| data.claims)
            .map_err(|e| {
                tracing::debug!(error = %e, "rejected JWT");
                AppError::unauthorized("Invalid or expired token")
            })
    }
}
