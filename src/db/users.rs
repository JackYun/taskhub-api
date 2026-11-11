//! User repository.

use chrono::{SubsecRound, Utc};
use serde_json::json;
use sqlx::SqlitePool;

use crate::{error::AppError, models::user::User};

/// Insert a new user. Unique-constraint violations become `409 Conflict`.
pub async fn create(
    pool: &SqlitePool,
    username: &str,
    email: &str,
    password_hash: &str,
) -> Result<User, AppError> {
    let now = Utc::now().trunc_subsecs(3);
    let result = sqlx::query_as::<_, User>(
        "INSERT INTO users (username, email, password_hash, created_at)
         VALUES (?, ?, ?, ?)
         RETURNING id, username, email, password_hash, created_at",
    )
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .bind(now)
    .fetch_one(pool)
    .await;

    match result {
        Ok(user) => Ok(user),
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
            let field = if db_err.message().contains("users.email") {
                "email"
            } else {
                "username"
            };
            Err(AppError::Conflict {
                message: format!("A user with this {field} already exists"),
                details: Some(json!({ "field": field })),
            })
        }
        Err(e) => Err(e.into()),
    }
}

/// Look up a user by username (case-insensitive).
pub async fn find_by_username(pool: &SqlitePool, username: &str) -> Result<Option<User>, AppError> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, created_at FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(pool)
    .await?)
}

/// Look up a user by id.
pub async fn find_by_id(pool: &SqlitePool, id: i64) -> Result<Option<User>, AppError> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, created_at FROM users WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}
