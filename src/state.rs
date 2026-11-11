//! Shared application state handed to every handler.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{auth::jwt::JwtManager, config::Config};

/// Cheap-to-clone state shared across requests.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub jwt: Arc<JwtManager>,
    pub config: Arc<Config>,
}

impl AppState {
    /// Build state from an already-migrated pool and the configuration.
    pub fn new(db: SqlitePool, config: Config) -> Self {
        let jwt = JwtManager::new(config.jwt_secret.as_bytes(), config.jwt_expiry_seconds);
        Self {
            db,
            jwt: Arc::new(jwt),
            config: Arc::new(config),
        }
    }
}
