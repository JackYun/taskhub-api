use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use validator::{Validate, ValidationError};

use super::{datetime, invalid, nullable, validate_not_blank};

pub const MAX_TAGS: usize = 20;
pub const MAX_TAG_LEN: usize = 32;
pub const DEFAULT_PER_PAGE: u32 = 20;
pub const MAX_PER_PAGE: u32 = 100;

/// Workflow state of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
}

/// Raw row as selected from SQLite (tags aggregated as a JSON array string).
#[derive(Debug, FromRow)]
pub struct TaskRow {
    pub id: i64,
    pub user_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A task as returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Task {
    #[schema(example = 1)]
    pub id: i64,
    #[schema(example = "Write project README")]
    pub title: String,
    #[schema(example = "Bilingual, with real curl output")]
    pub description: Option<String>,
    pub status: TaskStatus,
    pub due_date: Option<DateTime<Utc>>,
    #[schema(example = json!(["docs", "writing"]))]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TaskRow> for Task {
    fn from(row: TaskRow) -> Self {
        let mut tags: Vec<String> = serde_json::from_str(&row.tags).unwrap_or_default();
        tags.sort();
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            status: row.status,
            due_date: row.due_date,
            tags,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// `POST /api/v1/tasks` body.
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateTaskRequest {
    #[validate(
        length(min = 1, max = 200, message = "must be between 1 and 200 characters"),
        custom(function = "validate_not_blank")
    )]
    #[schema(example = "Write project README")]
    pub title: String,
    #[validate(length(max = 5000, message = "must be at most 5000 characters"))]
    pub description: Option<String>,
    /// Defaults to `todo`.
    pub status: Option<TaskStatus>,
    /// RFC 3339 timestamp or `YYYY-MM-DD` (midnight UTC).
    #[serde(default, deserialize_with = "datetime::option")]
    #[schema(value_type = Option<String>, example = "2026-12-01T17:00:00Z")]
    pub due_date: Option<DateTime<Utc>>,
    #[serde(default)]
    #[validate(custom(function = "validate_tags"))]
    #[schema(example = json!(["docs", "writing"]))]
    pub tags: Vec<String>,
}

/// `PATCH /api/v1/tasks/{id}` body. Every field is optional; omitted fields
/// are left untouched. `description` and `due_date` can be cleared with `null`.
#[derive(Debug, Default, Deserialize, Validate, ToSchema)]
pub struct UpdateTaskRequest {
    #[validate(
        length(min = 1, max = 200, message = "must be between 1 and 200 characters"),
        custom(function = "validate_not_blank")
    )]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "nullable")]
    #[validate(length(max = 5000, message = "must be at most 5000 characters"))]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
    pub status: Option<TaskStatus>,
    #[serde(default, deserialize_with = "datetime::double_option")]
    #[schema(value_type = Option<String>, example = "2026-12-01")]
    pub due_date: Option<Option<DateTime<Utc>>>,
    /// Replaces the full tag set when present.
    #[validate(custom(function = "validate_tags"))]
    pub tags: Option<Vec<String>>,
}

/// Columns a list can be sorted by (whitelisted – never interpolated from
/// user input).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    #[default]
    CreatedAt,
    DueDate,
    Title,
    Status,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

/// Query parameters of `GET /api/v1/tasks`.
#[derive(Debug, Default, Deserialize, Validate, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListTasksQuery {
    /// Page number, starting at 1 (default 1).
    #[validate(range(min = 1, max = 1_000_000, message = "must be >= 1"))]
    pub page: Option<u32>,
    /// Items per page, 1-100 (default 20).
    #[validate(range(min = 1, max = 100, message = "must be between 1 and 100"))]
    pub per_page: Option<u32>,
    /// Filter by status.
    pub status: Option<TaskStatus>,
    /// Only tasks carrying this tag (case-insensitive).
    #[validate(length(max = 32, message = "must be at most 32 characters"))]
    pub tag: Option<String>,
    /// Only tasks due at or before this instant (RFC 3339 or YYYY-MM-DD).
    #[serde(default, deserialize_with = "datetime::option")]
    #[param(value_type = Option<String>)]
    pub due_before: Option<DateTime<Utc>>,
    /// Only tasks due at or after this instant (RFC 3339 or YYYY-MM-DD).
    #[serde(default, deserialize_with = "datetime::option")]
    #[param(value_type = Option<String>)]
    pub due_after: Option<DateTime<Utc>>,
    /// Case-insensitive text search in title and description.
    #[validate(length(max = 200, message = "must be at most 200 characters"))]
    pub q: Option<String>,
    /// Sort column (default `created_at`).
    #[serde(default)]
    pub sort: SortField,
    /// Sort direction (default `desc`).
    #[serde(default)]
    pub order: SortOrder,
}

impl ListTasksQuery {
    pub fn page(&self) -> u32 {
        self.page.unwrap_or(1)
    }

    pub fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(DEFAULT_PER_PAGE).min(MAX_PER_PAGE)
    }
}

/// Pagination metadata.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct Pagination {
    #[schema(example = 1)]
    pub page: u32,
    #[schema(example = 20)]
    pub per_page: u32,
    #[schema(example = 42)]
    pub total: i64,
    #[schema(example = 3)]
    pub total_pages: i64,
}

impl Pagination {
    pub fn new(page: u32, per_page: u32, total: i64) -> Self {
        let per = i64::from(per_page.max(1));
        let total_pages = if total <= 0 { 0 } else { (total - 1) / per + 1 };
        Self {
            page,
            per_page,
            total,
            total_pages,
        }
    }
}

/// Paginated list of tasks.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TaskList {
    pub items: Vec<Task>,
    pub pagination: Pagination,
}

/// Trim, lowercase, de-duplicate and sort tags.
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tags.iter().map(|t| t.trim().to_lowercase()).collect();
    out.sort();
    out.dedup();
    out
}

fn validate_tags(tags: &[String]) -> Result<(), ValidationError> {
    if tags.len() > MAX_TAGS {
        return Err(invalid(
            "too_many_tags",
            format!("at most {MAX_TAGS} tags allowed"),
        ));
    }
    for tag in tags {
        let len = tag.trim().chars().count();
        if len == 0 || len > MAX_TAG_LEN {
            return Err(invalid(
                "tag_length",
                format!("each tag must be between 1 and {MAX_TAG_LEN} characters"),
            ));
        }
        if tag.chars().any(char::is_control) {
            return Err(invalid(
                "tag_charset",
                "tags must not contain control characters",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_pages_rounds_up() {
        assert_eq!(Pagination::new(1, 20, 0).total_pages, 0);
        assert_eq!(Pagination::new(1, 20, 20).total_pages, 1);
        assert_eq!(Pagination::new(1, 20, 21).total_pages, 2);
    }

    #[test]
    fn tags_are_normalized() {
        let tags = vec![" Work ".into(), "work".into(), "Home".into()];
        assert_eq!(normalize_tags(&tags), vec!["home", "work"]);
    }
}
