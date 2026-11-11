mod common;

use axum::http::StatusCode;
use common::{error_code, TestApp};
use serde_json::{json, Value};

fn titles(body: &Value) -> Vec<String> {
    body["items"]
        .as_array()
        .expect("items array")
        .iter()
        .map(|t| t["title"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn task_crud_lifecycle() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;

    // Create
    let task = app
        .create_task(
            &token,
            json!({
                "title": "Write README",
                "description": "Bilingual docs",
                "due_date": "2026-12-01",
                "tags": ["Docs", "writing", "docs"]
            }),
        )
        .await;
    let id = task["id"].as_i64().unwrap();
    assert_eq!(task["status"], "todo");
    assert_eq!(task["due_date"], "2026-12-01T00:00:00Z");
    assert_eq!(task["tags"], json!(["docs", "writing"]));
    assert!(task["created_at"].is_string());

    // Read
    let res = app.get(&format!("/api/v1/tasks/{id}"), Some(&token)).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body, task);

    // Update (partial)
    let res = app
        .patch(
            &format!("/api/v1/tasks/{id}"),
            &token,
            json!({ "status": "in_progress" }),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["status"], "in_progress");
    assert_eq!(res.body["title"], "Write README");

    // Delete
    let res = app.delete(&format!("/api/v1/tasks/{id}"), &token).await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    assert!(res.body.is_null());

    let res = app.get(&format!("/api/v1/tasks/{id}"), Some(&token)).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
    assert_eq!(error_code(&res), "not_found");

    let res = app.delete(&format!("/api/v1/tasks/{id}"), &token).await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn patch_updates_only_given_fields_and_null_clears() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;
    let task = app
        .create_task(
            &token,
            json!({
                "title": "Original",
                "description": "keep me?",
                "due_date": "2026-12-01T09:30:00+08:00",
                "tags": ["a"]
            }),
        )
        .await;
    let id = task["id"].as_i64().unwrap();
    assert_eq!(
        task["due_date"], "2026-12-01T01:30:00Z",
        "normalised to UTC"
    );

    let res = app
        .patch(
            &format!("/api/v1/tasks/{id}"),
            &token,
            json!({ "title": "Renamed", "description": null, "tags": ["b", "c"] }),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(res.body["title"], "Renamed");
    assert!(res.body["description"].is_null());
    assert_eq!(res.body["due_date"], "2026-12-01T01:30:00Z", "untouched");
    assert_eq!(res.body["tags"], json!(["b", "c"]));
    assert_eq!(res.body["status"], "todo");
    assert_eq!(res.body["created_at"], task["created_at"]);

    let res = app
        .patch(
            &format!("/api/v1/tasks/{id}"),
            &token,
            json!({ "due_date": null }),
        )
        .await;
    assert!(res.body["due_date"].is_null());
    assert_eq!(res.body["tags"], json!(["b", "c"]), "tags untouched");
}

#[tokio::test]
async fn create_task_validation_errors() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;

    let cases = [
        json!({ "title": "" }),
        json!({ "title": "   " }),
        json!({ "title": "x".repeat(201) }),
        json!({ "title": "ok", "tags": ["x".repeat(33)] }),
    ];
    for body in cases {
        let res = app.post("/api/v1/tasks", Some(&token), body.clone()).await;
        assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(error_code(&res), "validation_error");
    }

    let res = app
        .post("/api/v1/tasks", Some(&token), json!({ "title": "" }))
        .await;
    assert!(res.body["error"]["details"]["title"].is_array());

    // Exactly 200 characters is fine.
    app.create_task(&token, json!({ "title": "y".repeat(200) }))
        .await;
}

#[tokio::test]
async fn invalid_enum_and_date_values_are_rejected() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;

    let res = app
        .post(
            "/api/v1/tasks",
            Some(&token),
            json!({ "title": "t", "status": "archived" }),
        )
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);

    let res = app
        .post(
            "/api/v1/tasks",
            Some(&token),
            json!({ "title": "t", "due_date": "next friday" }),
        )
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn users_cannot_see_or_touch_each_others_tasks() {
    let app = TestApp::new().await;
    let alice = app.token_for("alice").await;
    let bob = app.token_for("bob").await;

    let task = app
        .create_task(&alice, json!({ "title": "Alice only" }))
        .await;
    let uri = format!("/api/v1/tasks/{}", task["id"]);

    assert_eq!(
        app.get(&uri, Some(&bob)).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.patch(&uri, &bob, json!({ "title": "hacked" }))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(app.delete(&uri, &bob).await.status, StatusCode::NOT_FOUND);

    let list = app.get("/api/v1/tasks", Some(&bob)).await;
    assert_eq!(list.body["pagination"]["total"], 0);

    // Alice's task is unchanged.
    let res = app.get(&uri, Some(&alice)).await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(res.body["title"], "Alice only");
}

#[tokio::test]
async fn list_is_paginated() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;
    for i in 1..=25 {
        app.create_task(&token, json!({ "title": format!("Task {i:02}") }))
            .await;
    }

    let res = app
        .get("/api/v1/tasks?page=1&per_page=10", Some(&token))
        .await;
    assert_eq!(res.status, StatusCode::OK);
    assert_eq!(
        res.body["pagination"],
        json!({ "page": 1, "per_page": 10, "total": 25, "total_pages": 3 })
    );
    assert_eq!(res.body["items"].as_array().unwrap().len(), 10);
    // Default sort: newest first.
    assert_eq!(titles(&res.body)[0], "Task 25");

    let res = app
        .get("/api/v1/tasks?page=3&per_page=10", Some(&token))
        .await;
    assert_eq!(res.body["items"].as_array().unwrap().len(), 5);
    assert_eq!(titles(&res.body)[4], "Task 01");

    let res = app
        .get("/api/v1/tasks?page=4&per_page=10", Some(&token))
        .await;
    assert_eq!(res.body["items"].as_array().unwrap().len(), 0);

    // Defaults: page 1, 20 per page.
    let res = app.get("/api/v1/tasks", Some(&token)).await;
    assert_eq!(res.body["pagination"]["per_page"], 20);
    assert_eq!(res.body["items"].as_array().unwrap().len(), 20);
}

#[tokio::test]
async fn invalid_pagination_and_sort_params_are_rejected() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;

    let res = app.get("/api/v1/tasks?per_page=500", Some(&token)).await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(res.body["error"]["details"]["per_page"].is_array());

    let res = app.get("/api/v1/tasks?page=0", Some(&token)).await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Only whitelisted sort columns are accepted.
    let res = app
        .get("/api/v1/tasks?sort=password_hash", Some(&token))
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&res), "bad_request");

    let res = app
        .get(
            "/api/v1/tasks?sort=title;DROP%20TABLE%20tasks",
            Some(&token),
        )
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);

    let res = app.get("/api/v1/tasks?order=sideways", Some(&token)).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_filters_by_status_tag_and_text() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;
    app.create_task(
        &token,
        json!({ "title": "Buy milk", "tags": ["home", "shopping"] }),
    )
    .await;
    app.create_task(
        &token,
        json!({ "title": "Deploy API", "status": "in_progress", "tags": ["work"] }),
    )
    .await;
    app.create_task(
        &token,
        json!({ "title": "Fix login bug", "description": "Users see a 500 on MILK day", "status": "done", "tags": ["work"] }),
    )
    .await;

    let res = app.get("/api/v1/tasks?status=done", Some(&token)).await;
    assert_eq!(titles(&res.body), vec!["Fix login bug"]);

    let res = app
        .get("/api/v1/tasks?tag=WORK&sort=title&order=asc", Some(&token))
        .await;
    assert_eq!(titles(&res.body), vec!["Deploy API", "Fix login bug"]);

    let res = app
        .get("/api/v1/tasks?tag=work&status=in_progress", Some(&token))
        .await;
    assert_eq!(titles(&res.body), vec!["Deploy API"]);

    // q searches title and description, case-insensitively.
    let res = app
        .get("/api/v1/tasks?q=milk&sort=title&order=asc", Some(&token))
        .await;
    assert_eq!(titles(&res.body), vec!["Buy milk", "Fix login bug"]);
    assert_eq!(res.body["pagination"]["total"], 2);

    // LIKE wildcards in q are matched literally.
    let res = app.get("/api/v1/tasks?q=%25", Some(&token)).await;
    assert_eq!(res.body["pagination"]["total"], 0);
}

#[tokio::test]
async fn list_filters_by_due_date_range() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;
    app.create_task(&token, json!({ "title": "Nov", "due_date": "2026-11-20" }))
        .await;
    app.create_task(
        &token,
        json!({ "title": "Dec", "due_date": "2026-12-15T12:00:00Z" }),
    )
    .await;
    app.create_task(&token, json!({ "title": "Jan", "due_date": "2027-01-10" }))
        .await;
    app.create_task(&token, json!({ "title": "No date" })).await;

    let res = app
        .get(
            "/api/v1/tasks?due_after=2026-12-01&due_before=2026-12-31&sort=due_date&order=asc",
            Some(&token),
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(titles(&res.body), vec!["Dec"]);

    let res = app
        .get(
            "/api/v1/tasks?due_before=2026-12-31T23:59:59Z&sort=due_date&order=asc",
            Some(&token),
        )
        .await;
    assert_eq!(titles(&res.body), vec!["Nov", "Dec"]);

    let res = app
        .get("/api/v1/tasks?due_before=someday", Some(&token))
        .await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_sorts_by_whitelisted_columns() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;
    app.create_task(
        &token,
        json!({ "title": "banana", "status": "done", "due_date": "2026-12-03" }),
    )
    .await;
    app.create_task(&token, json!({ "title": "Apple", "status": "todo" }))
        .await;
    app.create_task(
        &token,
        json!({ "title": "cherry", "status": "in_progress", "due_date": "2026-12-01" }),
    )
    .await;

    let res = app
        .get("/api/v1/tasks?sort=title&order=asc", Some(&token))
        .await;
    assert_eq!(titles(&res.body), vec!["Apple", "banana", "cherry"]);

    let res = app
        .get("/api/v1/tasks?sort=title&order=desc", Some(&token))
        .await;
    assert_eq!(titles(&res.body), vec!["cherry", "banana", "Apple"]);

    // Tasks without a due date are always listed last.
    let res = app
        .get("/api/v1/tasks?sort=due_date&order=asc", Some(&token))
        .await;
    assert_eq!(titles(&res.body), vec!["cherry", "banana", "Apple"]);
    let res = app
        .get("/api/v1/tasks?sort=due_date&order=desc", Some(&token))
        .await;
    assert_eq!(titles(&res.body), vec!["banana", "cherry", "Apple"]);

    // Status sorts by workflow order: todo < in_progress < done.
    let res = app
        .get("/api/v1/tasks?sort=status&order=asc", Some(&token))
        .await;
    assert_eq!(titles(&res.body), vec!["Apple", "cherry", "banana"]);

    // Default: created_at desc (newest first).
    let res = app.get("/api/v1/tasks", Some(&token)).await;
    assert_eq!(titles(&res.body), vec!["cherry", "Apple", "banana"]);
}

#[tokio::test]
async fn non_numeric_task_id_is_bad_request() {
    let app = TestApp::new().await;
    let token = app.token_for("alice").await;
    let res = app.get("/api/v1/tasks/abc", Some(&token)).await;
    assert_eq!(res.status, StatusCode::BAD_REQUEST);
    assert_eq!(error_code(&res), "bad_request");
}
