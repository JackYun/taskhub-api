mod common;

use axum::http::{header, StatusCode};
use common::{error_code, TestApp, PASSWORD};
use serde_json::json;

#[tokio::test]
async fn register_creates_user_without_leaking_hash() {
    let app = TestApp::new().await;
    let res = app.register("alice").await;

    assert_eq!(res.status, StatusCode::CREATED);
    assert_eq!(res.body["username"], "alice");
    assert_eq!(res.body["email"], "alice@example.com");
    assert!(res.body["id"].is_i64());
    assert!(res.body.get("password_hash").is_none());
    assert!(res.body.get("password").is_none());
}

#[tokio::test]
async fn duplicate_username_is_conflict() {
    let app = TestApp::new().await;
    assert_eq!(app.register("alice").await.status, StatusCode::CREATED);

    let res = app
        .post(
            "/api/v1/auth/register",
            None,
            json!({ "username": "ALICE", "email": "other@example.com", "password": PASSWORD }),
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(error_code(&res), "conflict");
    assert_eq!(res.body["error"]["details"]["field"], "username");
}

#[tokio::test]
async fn duplicate_email_is_conflict() {
    let app = TestApp::new().await;
    assert_eq!(app.register("alice").await.status, StatusCode::CREATED);

    let res = app
        .post(
            "/api/v1/auth/register",
            None,
            json!({ "username": "alice2", "email": "Alice@Example.com", "password": PASSWORD }),
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.body["error"]["details"]["field"], "email");
}

#[tokio::test]
async fn register_validation_reports_each_field() {
    let app = TestApp::new().await;
    let res = app
        .post(
            "/api/v1/auth/register",
            None,
            json!({ "username": "a!", "email": "not-an-email", "password": "short" }),
        )
        .await;

    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(&res), "validation_error");
    let details = &res.body["error"]["details"];
    assert!(details["username"].is_array(), "{details}");
    assert!(details["email"].is_array(), "{details}");
    assert!(details["password"].is_array(), "{details}");
}

#[tokio::test]
async fn missing_field_is_validation_error() {
    let app = TestApp::new().await;
    let res = app
        .post("/api/v1/auth/register", None, json!({ "username": "bob" }))
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(&res), "validation_error");
}

#[tokio::test]
async fn login_returns_bearer_token_usable_on_me() {
    let app = TestApp::new().await;
    app.register("alice").await;

    let res = app
        .post(
            "/api/v1/auth/login",
            None,
            json!({ "username": "alice", "password": PASSWORD }),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["token_type"], "Bearer");
    assert_eq!(res.body["expires_in"], 3600);
    let token = res.body["access_token"].as_str().unwrap();
    assert_eq!(token.split('.').count(), 3, "looks like a JWT");

    let me = app.get("/api/v1/auth/me", Some(token)).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["username"], "alice");
}

#[tokio::test]
async fn login_with_wrong_password_is_unauthorized() {
    let app = TestApp::new().await;
    app.register("alice").await;

    let res = app
        .post(
            "/api/v1/auth/login",
            None,
            json!({ "username": "alice", "password": "wrong-passw0rd" }),
        )
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "unauthorized");
}

#[tokio::test]
async fn login_with_unknown_user_is_unauthorized_with_same_message() {
    let app = TestApp::new().await;
    app.register("alice").await;

    let wrong_pw = app
        .post(
            "/api/v1/auth/login",
            None,
            json!({ "username": "alice", "password": "wrong-passw0rd" }),
        )
        .await;
    let unknown = app
        .post(
            "/api/v1/auth/login",
            None,
            json!({ "username": "nobody", "password": PASSWORD }),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::UNAUTHORIZED);
    // Same message for both cases: no username enumeration.
    assert_eq!(
        unknown.body["error"]["message"],
        wrong_pw.body["error"]["message"]
    );
}

#[tokio::test]
async fn protected_route_requires_token() {
    let app = TestApp::new().await;
    let res = app.get("/api/v1/tasks", None).await;

    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(&res), "unauthorized");
    assert_eq!(res.headers[header::WWW_AUTHENTICATE], "Bearer");
}

#[tokio::test]
async fn invalid_or_tampered_token_is_rejected() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;

    let garbage = app.get("/api/v1/tasks", Some("not-a-jwt")).await;
    assert_eq!(garbage.status, StatusCode::UNAUTHORIZED);

    // Change one character inside the signature (not the final one, whose
    // low bits are only base64 padding).
    let mut chars: Vec<char> = token.chars().collect();
    let idx = chars.len() - 5;
    chars[idx] = if chars[idx] == 'A' { 'B' } else { 'A' };
    let tampered: String = chars.into_iter().collect();
    let res = app.get("/api/v1/tasks", Some(&tampered)).await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);

    // Sanity check: the untouched token works.
    assert_eq!(
        app.get("/api/v1/tasks", Some(&token)).await.status,
        StatusCode::OK
    );
}
