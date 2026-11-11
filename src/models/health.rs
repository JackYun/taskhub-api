use serde::Serialize;
use utoipa::ToSchema;

/// Response of `GET /health`.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// `ok` when every dependency is reachable, otherwise `degraded`.
    #[schema(example = "ok")]
    pub status: &'static str,
    /// Database connectivity: `ok` or `unavailable`.
    #[schema(example = "ok")]
    pub database: &'static str,
    #[schema(example = "0.1.0")]
    pub version: &'static str,
}
