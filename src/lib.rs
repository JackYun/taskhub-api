//! TaskHub API – a production-style task / notes REST service.
//!
//! The crate is split into a library (everything testable) and a thin binary
//! (`main.rs`) that only loads configuration, opens the database and serves
//! the router returned by [`build_app`].

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod extract;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod state;

use std::time::Duration;

use axum::{
    body::Body,
    http::{header, HeaderName, HeaderValue, Method, Request},
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub use config::Config;
pub use error::AppError;
pub use state::AppState;

/// Header used to correlate a request across logs and responses.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Build the complete application router (API routes, Swagger UI and
/// middleware stack) for the given state.
pub fn build_app(state: AppState) -> Router {
    let cors = cors_layer(&state.config.cors_allowed_origins);
    let request_id = HeaderName::from_static(REQUEST_ID_HEADER);

    let middleware = ServiceBuilder::new()
        // Outermost: assign an id first so every later layer (and the logs) can see it.
        .layer(SetRequestIdLayer::new(request_id.clone(), MakeRequestUuid))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| {
                    let request_id = request
                        .headers()
                        .get(REQUEST_ID_HEADER)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("-");
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        request_id = %request_id,
                    )
                })
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(PropagateRequestIdLayer::new(request_id))
        .layer(cors);

    Router::new()
        .merge(routes::router())
        .merge(
            SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi::ApiDoc::openapi()),
        )
        .fallback(routes::fallback)
        .with_state(state)
        .layer(middleware)
}

fn cors_layer(origins: &[String]) -> CorsLayer {
    let base = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .expose_headers([HeaderName::from_static(REQUEST_ID_HEADER)])
        .max_age(Duration::from_secs(3600));

    if origins.is_empty() || origins.iter().any(|o| o == "*") {
        base.allow_origin(Any)
    } else {
        let list: Vec<HeaderValue> = origins
            .iter()
            .filter_map(|o| HeaderValue::from_str(o).ok())
            .collect();
        base.allow_origin(AllowOrigin::list(list))
    }
}
