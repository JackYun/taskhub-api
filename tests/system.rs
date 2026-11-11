mod common;

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use common::{error_code, TestApp};

#[tokio::test]
async fn health_reports_ok_with_database_check() {
    let app = TestApp::new().await;
    let res = app.get("/health", None).await;

    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["status"], "ok");
    assert_eq!(res.body["database"], "ok");
    assert_eq!(res.body["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn every_response_carries_a_request_id() {
    let app = TestApp::new().await;
    let res = app.get("/health", None).await;
    let id = res.headers["x-request-id"].to_str().unwrap();
    assert_eq!(id.len(), 36, "uuid: {id}");

    // A client-supplied id is propagated unchanged.
    let req = Request::builder()
        .uri("/health")
        .header("x-request-id", "client-abc-123")
        .body(Body::empty())
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.headers["x-request-id"], "client-abc-123");
}

#[tokio::test]
async fn unknown_route_returns_json_404() {
    let app = TestApp::new().await;
    let res = app.get("/api/v1/nope", None).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(error_code(&res), "not_found");
}

#[tokio::test]
async fn malformed_json_is_bad_request() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/register")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{\"username\": "))
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&res), "bad_request");

    // Missing content type is also a client error with the same envelope.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .body(Body::from("{}"))
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&res), "bad_request");
}

#[tokio::test]
async fn openapi_spec_is_served_with_bearer_scheme() {
    let app = TestApp::new().await;
    let res = app.get("/api-docs/openapi.json", None).await;

    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["info"]["title"], "TaskHub API");
    let paths = res.body["paths"].as_object().unwrap();
    for path in [
        "/health",
        "/api/v1/auth/register",
        "/api/v1/auth/login",
        "/api/v1/tasks",
        "/api/v1/tasks/{id}",
    ] {
        assert!(paths.contains_key(path), "missing {path}");
    }
    assert_eq!(
        res.body["components"]["securitySchemes"]["bearer_auth"]["scheme"],
        "bearer"
    );
}

#[tokio::test]
async fn swagger_ui_is_served() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .uri("/swagger-ui/")
        .body(Body::empty())
        .unwrap();
    let res = app.send(req).await;
    assert_eq!(res.status, StatusCode::OK);
    let html = res.body.as_str().unwrap_or_default();
    assert!(html.contains("swagger"), "unexpected body");
}

#[tokio::test]
async fn cors_preflight_is_answered() {
    let app = TestApp::new().await;
    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/tasks")
        .header(header::ORIGIN, "https://example.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            "authorization,content-type",
        )
        .body(Body::empty())
        .unwrap();
    let res = app.send(req).await;
    assert!(res.status.is_success());
    assert_eq!(res.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
}
