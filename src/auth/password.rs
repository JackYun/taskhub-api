//! Argon2id password hashing (random per-password salt, PHC string format).
//!
//! Hashing is CPU-heavy by design, so both functions run on Tokio's blocking
//! thread pool instead of stalling the async executor.

use std::sync::LazyLock;

use argon2::{password_hash::PasswordHasher, Argon2, PasswordVerifier};

use crate::error::AppError;

/// A real Argon2 hash of a throwaway value. Verifying against it when a login
/// names an unknown user keeps response timing similar to the "wrong
/// password" case, which makes username enumeration harder.
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| {
    Argon2::default()
        .hash_password(b"taskhub-dummy-password")
        .map(|h| h.to_string())
        .unwrap_or_default()
});

/// Hash a password with Argon2id and a freshly generated random salt.
pub async fn hash_password(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|h| h.to_string())
            .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))
    })
    .await
    .map_err(|e| anyhow::anyhow!("hashing task panicked: {e}"))?
    .map_err(AppError::from)
}

/// Verify a password against a stored PHC hash. Returns `false` for any
/// mismatch or malformed hash.
pub async fn verify_password(password: String, hash: Option<String>) -> Result<bool, AppError> {
    tokio::task::spawn_blocking(move || {
        let hash = hash.unwrap_or_else(|| DUMMY_HASH.clone());
        Argon2::default()
            .verify_password(password.as_bytes(), hash.as_str())
            .is_ok()
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("verify task panicked: {e}")))
}
