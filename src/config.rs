//! Runtime configuration loaded from environment variables.

use std::{env, net::SocketAddr};

/// Development-only fallback secret. Only ever used by debug builds when
/// `JWT_SECRET` is not set, and a loud warning is logged when it is.
const DEV_ONLY_JWT_SECRET: &str = "dev-only-insecure-secret-change-me-0123456789";

const DEFAULT_DATABASE_URL: &str = "sqlite://taskhub.db";
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3000;
const DEFAULT_JWT_EXPIRY_SECONDS: i64 = 3600;

/// Application configuration.
#[derive(Clone)]
pub struct Config {
    /// SQLx connection string, e.g. `sqlite://taskhub.db` or `sqlite::memory:`.
    pub database_url: String,
    /// Socket address the HTTP server binds to.
    pub bind_addr: SocketAddr,
    /// HMAC secret used to sign JWTs (HS256).
    pub jwt_secret: String,
    /// Access token lifetime in seconds.
    pub jwt_expiry_seconds: i64,
    /// Allowed CORS origins. Empty or `["*"]` means any origin.
    pub cors_allowed_origins: Vec<String>,
}

// Hand-written so the JWT secret can never end up in logs.
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("database_url", &self.database_url)
            .field("bind_addr", &self.bind_addr)
            .field("jwt_secret", &"<redacted>")
            .field("jwt_expiry_seconds", &self.jwt_expiry_seconds)
            .field("cors_allowed_origins", &self.cors_allowed_origins)
            .finish()
    }
}

/// Errors raised while reading configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("environment variable {0} is required but not set")]
    Missing(&'static str),
    #[error("environment variable {name} has an invalid value: {reason}")]
    Invalid { name: &'static str, reason: String },
}

impl Config {
    /// Read configuration from the process environment.
    ///
    /// Call `dotenvy::dotenv()` beforehand if a `.env` file should be honoured.
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = var("DATABASE_URL").unwrap_or_else(|| DEFAULT_DATABASE_URL.to_owned());

        let bind_addr = match var("BIND_ADDR") {
            Some(addr) => addr.parse().map_err(|e| ConfigError::Invalid {
                name: "BIND_ADDR",
                reason: format!("{e}"),
            })?,
            None => {
                let host = var("HOST").unwrap_or_else(|| DEFAULT_HOST.to_owned());
                let port = match var("PORT") {
                    Some(p) => p.parse::<u16>().map_err(|e| ConfigError::Invalid {
                        name: "PORT",
                        reason: format!("{e}"),
                    })?,
                    None => DEFAULT_PORT,
                };
                format!("{host}:{port}")
                    .parse()
                    .map_err(|e| ConfigError::Invalid {
                        name: "HOST",
                        reason: format!("{e}"),
                    })?
            }
        };

        let jwt_secret = match var("JWT_SECRET") {
            Some(secret) => {
                if secret.len() < 32 {
                    tracing::warn!("JWT_SECRET is shorter than 32 bytes; use a long random value");
                }
                secret
            }
            None if cfg!(debug_assertions) => {
                tracing::warn!(
                    "!!! JWT_SECRET is not set - using a DEV-ONLY insecure default secret. \
                     Never run like this in production (release builds refuse to start) !!!"
                );
                DEV_ONLY_JWT_SECRET.to_owned()
            }
            None => return Err(ConfigError::Missing("JWT_SECRET")),
        };

        let jwt_expiry_seconds = match var("JWT_EXPIRY_SECONDS") {
            Some(v) => match v.parse::<i64>() {
                Ok(n) if n > 0 => n,
                _ => {
                    return Err(ConfigError::Invalid {
                        name: "JWT_EXPIRY_SECONDS",
                        reason: "must be a positive integer (seconds)".to_owned(),
                    })
                }
            },
            None => DEFAULT_JWT_EXPIRY_SECONDS,
        };

        let cors_allowed_origins = var("CORS_ALLOWED_ORIGINS")
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            database_url,
            bind_addr,
            jwt_secret,
            jwt_expiry_seconds,
            cors_allowed_origins,
        })
    }
}

/// Read a non-empty environment variable.
fn var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}
