use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use validator::{Validate, ValidationError};

use super::invalid;

/// Database row for `users`.
#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

/// Public view of a user (never exposes the password hash).
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserResponse {
    #[schema(example = 1)]
    pub id: i64,
    #[schema(example = "alice")]
    pub username: String,
    #[schema(example = "alice@example.com")]
    pub email: String,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            created_at: user.created_at,
        }
    }
}

/// `POST /api/v1/auth/register` body.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RegisterRequest {
    /// 3-32 characters: letters, digits and underscores.
    #[validate(
        length(min = 3, max = 32, message = "must be between 3 and 32 characters"),
        custom(function = "validate_username")
    )]
    #[schema(example = "alice")]
    pub username: String,
    #[validate(
        email(message = "must be a valid email address"),
        length(max = 254, message = "must be at most 254 characters")
    )]
    #[schema(example = "alice@example.com")]
    pub email: String,
    /// 8-128 characters, must contain at least one letter and one digit.
    #[validate(
        length(min = 8, max = 128, message = "must be between 8 and 128 characters"),
        custom(function = "validate_password_strength")
    )]
    #[schema(example = "correct-horse-42", format = Password)]
    pub password: String,
}

/// `POST /api/v1/auth/login` body.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    #[validate(length(min = 1, max = 254, message = "is required"))]
    #[schema(example = "alice")]
    pub username: String,
    #[validate(length(min = 1, max = 128, message = "is required"))]
    #[schema(example = "correct-horse-42", format = Password)]
    pub password: String,
}

/// Successful login response.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    #[schema(example = "Bearer")]
    pub token_type: String,
    /// Token lifetime in seconds.
    #[schema(example = 3600)]
    pub expires_in: i64,
}

fn validate_username(username: &str) -> Result<(), ValidationError> {
    if username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        Ok(())
    } else {
        Err(invalid(
            "username_charset",
            "may only contain letters, digits and underscores",
        ))
    }
}

fn validate_password_strength(password: &str) -> Result<(), ValidationError> {
    let has_letter = password.chars().any(char::is_alphabetic);
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if has_letter && has_digit {
        Ok(())
    } else {
        Err(invalid(
            "password_strength",
            "must contain at least one letter and one digit",
        ))
    }
}
