//! Shared helpers for the integration tests: every test gets its own
//! in-memory SQLite database and drives the router with `oneshot` (no network).

#![allow(dead_code)]

use axum::{
    body::Body,
    http::{header, HeaderMap, Method, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use taskhub_api::{build_app, db, AppState, Config};
use tower::ServiceExt;

pub const PASSWORD: &str = "s3cure-passw0rd";

pub struct TestApp {
    router: Router,
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
}

impl TestApp {
    pub async fn new() -> Self {
        let pool = db::connect("sqlite::memory:")
            .await
            .expect("open in-memory db");
        db::migrate(&pool).await.expect("run migrations");
        let config = Config {
            database_url: "sqlite::memory:".to_owned(),
            bind_addr: "127.0.0.1:0".parse().expect("addr"),
            jwt_secret: "integration-test-secret-at-least-32-bytes".to_owned(),
            jwt_expiry_seconds: 3600,
            cors_allowed_origins: Vec::new(),
        };
        Self {
            router: build_app(AppState::new(pool, config)),
        }
    }

    /// Send a request; `body` is sent as JSON when present.
    pub async fn request(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> TestResponse {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let body = match body {
            Some(json) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(json.to_string())
            }
            None => Body::empty(),
        };
        self.send(builder.body(body).expect("build request")).await
    }

    /// Send a fully custom request.
    pub async fn send(&self, request: Request<Body>) -> TestResponse {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router is infallible");
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("read body")
            .to_bytes();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()))
        };
        TestResponse {
            status,
            headers,
            body,
        }
    }

    pub async fn get(&self, uri: &str, token: Option<&str>) -> TestResponse {
        self.request(Method::GET, uri, token, None).await
    }

    pub async fn post(&self, uri: &str, token: Option<&str>, body: Value) -> TestResponse {
        self.request(Method::POST, uri, token, Some(body)).await
    }

    pub async fn patch(&self, uri: &str, token: &str, body: Value) -> TestResponse {
        self.request(Method::PATCH, uri, Some(token), Some(body))
            .await
    }

    pub async fn delete(&self, uri: &str, token: &str) -> TestResponse {
        self.request(Method::DELETE, uri, Some(token), None).await
    }

    pub async fn register(&self, username: &str) -> TestResponse {
        self.post(
            "/api/v1/auth/register",
            None,
            json!({
                "username": username,
                "email": format!("{username}@example.com"),
                "password": PASSWORD,
            }),
        )
        .await
    }

    /// Register a user and return a valid access token.
    pub async fn token_for(&self, username: &str) -> String {
        let res = self.register(username).await;
        assert_eq!(res.status, StatusCode::CREATED, "register: {}", res.body);
        let res = self
            .post(
                "/api/v1/auth/login",
                None,
                json!({ "username": username, "password": PASSWORD }),
            )
            .await;
        assert_eq!(res.status, StatusCode::OK, "login: {}", res.body);
        res.body["access_token"]
            .as_str()
            .expect("access_token")
            .to_owned()
    }

    /// Create a task and return its JSON.
    pub async fn create_task(&self, token: &str, body: Value) -> Value {
        let res = self.post("/api/v1/tasks", Some(token), body).await;
        assert_eq!(res.status, StatusCode::CREATED, "create task: {}", res.body);
        res.body
    }
}

/// Assert the unified error envelope and return its `code`.
pub fn error_code(res: &TestResponse) -> &str {
    let error = &res.body["error"];
    assert!(
        error["message"].is_string(),
        "missing error.message: {}",
        res.body
    );
    error["code"].as_str().expect("error.code")
}
