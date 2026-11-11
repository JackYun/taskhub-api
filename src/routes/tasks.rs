use axum::{extract::State, http::StatusCode, Json};

use crate::{
    auth::AuthUser,
    db,
    error::{AppError, ErrorResponse},
    extract::{Path, ValidatedJson, ValidatedQuery},
    models::task::{
        CreateTaskRequest, ListTasksQuery, Pagination, Task, TaskList, UpdateTaskRequest,
    },
    state::AppState,
};

/// Create a task for the authenticated user.
#[utoipa::path(
    post,
    path = "/api/v1/tasks",
    tag = "tasks",
    security(("bearer_auth" = [])),
    request_body = CreateTaskRequest,
    responses(
        (status = 201, description = "Task created", body = Task),
        (status = 401, description = "Missing or invalid token", body = ErrorResponse),
        (status = 422, description = "Validation failed", body = ErrorResponse),
    )
)]
pub async fn create_task(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedJson(input): ValidatedJson<CreateTaskRequest>,
) -> Result<(StatusCode, Json<Task>), AppError> {
    let task = db::tasks::create(&state.db, auth.id, input).await?;
    Ok((StatusCode::CREATED, Json(task)))
}

/// List the caller's tasks with pagination, filtering and sorting.
#[utoipa::path(
    get,
    path = "/api/v1/tasks",
    tag = "tasks",
    security(("bearer_auth" = [])),
    params(ListTasksQuery),
    responses(
        (status = 200, description = "A page of tasks", body = TaskList),
        (status = 400, description = "Malformed query string (e.g. unknown sort column)", body = ErrorResponse),
        (status = 401, description = "Missing or invalid token", body = ErrorResponse),
        (status = 422, description = "Invalid pagination values", body = ErrorResponse),
    )
)]
pub async fn list_tasks(
    State(state): State<AppState>,
    auth: AuthUser,
    ValidatedQuery(params): ValidatedQuery<ListTasksQuery>,
) -> Result<Json<TaskList>, AppError> {
    let (items, total) = db::tasks::list(&state.db, auth.id, &params).await?;
    Ok(Json(TaskList {
        items,
        pagination: Pagination::new(params.page(), params.per_page(), total),
    }))
}

/// Fetch one task by id.
#[utoipa::path(
    get,
    path = "/api/v1/tasks/{id}",
    tag = "tasks",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Task id")),
    responses(
        (status = 200, description = "The task", body = Task),
        (status = 401, description = "Missing or invalid token", body = ErrorResponse),
        (status = 404, description = "No such task for this user", body = ErrorResponse),
    )
)]
pub async fn get_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<Task>, AppError> {
    db::tasks::get(&state.db, auth.id, id)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::not_found("Task"))
}

/// Partially update a task.
#[utoipa::path(
    patch,
    path = "/api/v1/tasks/{id}",
    tag = "tasks",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Task id")),
    request_body = UpdateTaskRequest,
    responses(
        (status = 200, description = "Updated task", body = Task),
        (status = 401, description = "Missing or invalid token", body = ErrorResponse),
        (status = 404, description = "No such task for this user", body = ErrorResponse),
        (status = 422, description = "Validation failed", body = ErrorResponse),
    )
)]
pub async fn update_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    ValidatedJson(input): ValidatedJson<UpdateTaskRequest>,
) -> Result<Json<Task>, AppError> {
    db::tasks::update(&state.db, auth.id, id, input)
        .await?
        .map(Json)
        .ok_or_else(|| AppError::not_found("Task"))
}

/// Delete a task.
#[utoipa::path(
    delete,
    path = "/api/v1/tasks/{id}",
    tag = "tasks",
    security(("bearer_auth" = [])),
    params(("id" = i64, Path, description = "Task id")),
    responses(
        (status = 204, description = "Task deleted"),
        (status = 401, description = "Missing or invalid token", body = ErrorResponse),
        (status = 404, description = "No such task for this user", body = ErrorResponse),
    )
)]
pub async fn delete_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    if db::tasks::delete(&state.db, auth.id, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::not_found("Task"))
    }
}
