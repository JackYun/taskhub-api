use axum::{extract::State, http::StatusCode, Json};

use crate::{db, models::health::HealthResponse, state::AppState};

/// Liveness + database connectivity check.
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service and database are healthy", body = HealthResponse),
        (status = 503, description = "Database unreachable", body = HealthResponse),
    )
)]
pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let db_ok = db::ping(&state.db).await;
    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(HealthResponse {
            status: if db_ok { "ok" } else { "degraded" },
            database: if db_ok { "ok" } else { "unavailable" },
            version: env!("CARGO_PKG_VERSION"),
        }),
    )
}
