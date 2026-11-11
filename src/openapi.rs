//! OpenAPI document (served at `/api-docs/openapi.json`, UI at `/swagger-ui`).

use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

use crate::{
    error::{ErrorBody, ErrorResponse},
    models::{
        health::HealthResponse,
        task::{
            CreateTaskRequest, Pagination, SortField, SortOrder, Task, TaskList, TaskStatus,
            UpdateTaskRequest,
        },
        user::{LoginRequest, RegisterRequest, TokenResponse, UserResponse},
    },
    routes,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "TaskHub API",
        description = "Task / notes management REST API with JWT authentication.",
        license(name = "MIT")
    ),
    paths(
        routes::health::health,
        routes::auth::register,
        routes::auth::login,
        routes::auth::me,
        routes::tasks::create_task,
        routes::tasks::list_tasks,
        routes::tasks::get_task,
        routes::tasks::update_task,
        routes::tasks::delete_task,
    ),
    components(schemas(
        HealthResponse,
        RegisterRequest,
        LoginRequest,
        TokenResponse,
        UserResponse,
        Task,
        TaskStatus,
        TaskList,
        Pagination,
        CreateTaskRequest,
        UpdateTaskRequest,
        SortField,
        SortOrder,
        ErrorResponse,
        ErrorBody,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Service health"),
        (name = "auth", description = "Registration and login"),
        (name = "tasks", description = "Task CRUD, filtering and pagination"),
    )
)]
pub struct ApiDoc;

/// Registers the `bearer_auth` (JWT) security scheme.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}
