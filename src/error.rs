//! Unified error type. Every handler returns `Result<_, AppError>` and every
//! error is rendered as
//! `{"error": {"code": "...", "message": "...", "details": ...}}`.

use axum::{
    extract::rejection::{JsonRejection, PathRejection, QueryRejection},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use utoipa::ToSchema;
use validator::{ValidationErrors, ValidationErrorsKind};

/// Application error. Client errors carry a safe, user-facing message;
/// `Database` / `Internal` are logged server-side and rendered generically.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{message}")]
    Conflict {
        message: String,
        details: Option<Value>,
    },
    #[error("{message}")]
    Validation {
        message: String,
        details: Option<Value>,
    },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// JSON envelope for every error response (documented in OpenAPI).
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    /// Stable machine-readable error code, e.g. `validation_error`.
    #[schema(example = "validation_error")]
    pub code: &'static str,
    /// Human-readable description.
    pub message: String,
    /// Optional structured details (e.g. per-field validation errors).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub details: Option<Value>,
}

impl AppError {
    pub fn not_found(what: &str) -> Self {
        Self::NotFound(format!("{what} not found"))
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict { .. } => StatusCode::CONFLICT,
            Self::Validation { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Database(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::Unauthorized(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::Conflict { .. } => "conflict",
            Self::Validation { .. } => "validation_error",
            Self::Database(_) | Self::Internal(_) => "internal_error",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let code = self.code();

        let (message, details) = match self {
            Self::Database(ref e) => {
                tracing::error!(error = %e, "database error");
                ("An internal server error occurred".to_owned(), None)
            }
            Self::Internal(ref e) => {
                tracing::error!(error = ?e, "internal error");
                ("An internal server error occurred".to_owned(), None)
            }
            Self::Conflict { message, details } | Self::Validation { message, details } => {
                (message, details)
            }
            Self::BadRequest(m)
            | Self::Unauthorized(m)
            | Self::Forbidden(m)
            | Self::NotFound(m) => (m, None),
        };

        let body = ErrorResponse {
            error: ErrorBody {
                code,
                message,
                details,
            },
        };
        let mut response = (status, Json(body)).into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

impl From<ValidationErrors> for AppError {
    fn from(errors: ValidationErrors) -> Self {
        Self::Validation {
            message: "Request validation failed".to_owned(),
            details: Some(validation_details(&errors)),
        }
    }
}

/// Convert `validator` errors into `{"field": [{"code": "...", "message": "..."}]}`.
fn validation_details(errors: &ValidationErrors) -> Value {
    let mut map = serde_json::Map::new();
    for (field, kind) in errors.errors() {
        let value = match kind {
            ValidationErrorsKind::Field(list) => Value::Array(
                list.iter()
                    .map(|e| {
                        json!({
                            "code": e.code,
                            "message": e
                                .message
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_else(|| format!("{field} is invalid")),
                        })
                    })
                    .collect(),
            ),
            ValidationErrorsKind::Struct(inner) => validation_details(inner),
            ValidationErrorsKind::List(items) => Value::Object(
                items
                    .iter()
                    .map(|(idx, inner)| (idx.to_string(), validation_details(inner)))
                    .collect(),
            ),
        };
        map.insert(field.to_string(), value);
    }
    Value::Object(map)
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        match rejection {
            // Well-formed JSON with the wrong shape (missing field, bad enum value, ...).
            JsonRejection::JsonDataError(e) => Self::Validation {
                message: e.body_text(),
                details: None,
            },
            other => Self::BadRequest(other.body_text()),
        }
    }
}

impl From<QueryRejection> for AppError {
    fn from(rejection: QueryRejection) -> Self {
        Self::BadRequest(rejection.body_text())
    }
}

impl From<PathRejection> for AppError {
    fn from(rejection: PathRejection) -> Self {
        Self::BadRequest(rejection.body_text())
    }
}
