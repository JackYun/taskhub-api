//! HTTP routing. Handlers stay thin: extract + validate, call the repository
//! layer in `crate::db`, map the result to a response.

pub mod auth;
pub mod health;
pub mod tasks;

use axum::{
    routing::{get, post},
    Router,
};

use crate::{error::AppError, state::AppState};

/// All API routes (without middleware or Swagger UI).
pub fn router() -> Router<AppState> {
    let api = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/me", get(auth::me))
        .route("/tasks", get(tasks::list_tasks).post(tasks::create_task))
        .route(
            "/tasks/{id}",
            get(tasks::get_task)
                .patch(tasks::update_task)
                .delete(tasks::delete_task),
        );

    Router::new()
        .route("/health", get(health::health))
        .nest("/api/v1", api)
}

/// JSON 404 for unknown routes.
pub async fn fallback() -> AppError {
    AppError::NotFound("Route not found".to_owned())
}
