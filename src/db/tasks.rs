//! Task repository. Every query is scoped by `user_id`, so a user can never
//! read or modify somebody else's task (they simply get "not found").

use chrono::{SubsecRound, Utc};
use sqlx::{QueryBuilder, Sqlite, SqliteConnection, SqlitePool};

use crate::{
    error::AppError,
    models::task::{
        normalize_tags, CreateTaskRequest, ListTasksQuery, SortField, SortOrder, Task, TaskRow,
        TaskStatus, UpdateTaskRequest,
    },
};

/// Shared SELECT list; tags are aggregated into a JSON array string.
macro_rules! select_task {
    () => {
        "SELECT t.id, t.user_id, t.title, t.description, t.status, t.due_date,
                t.created_at, t.updated_at,
                (SELECT json_group_array(tt.tag) FROM task_tags tt WHERE tt.task_id = t.id) AS tags
         FROM tasks t"
    };
}

/// Fetch a single task owned by `user_id`.
pub async fn get(pool: &SqlitePool, user_id: i64, id: i64) -> Result<Option<Task>, AppError> {
    let mut conn = pool.acquire().await?;
    fetch_one(&mut conn, user_id, id).await
}

async fn fetch_one(
    conn: &mut SqliteConnection,
    user_id: i64,
    id: i64,
) -> Result<Option<Task>, AppError> {
    let row =
        sqlx::query_as::<_, TaskRow>(concat!(select_task!(), " WHERE t.id = ? AND t.user_id = ?"))
            .bind(id)
            .bind(user_id)
            .fetch_optional(conn)
            .await?;
    Ok(row.map(Task::from))
}

async fn replace_tags(
    conn: &mut SqliteConnection,
    task_id: i64,
    tags: &[String],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM task_tags WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut *conn)
        .await?;
    for tag in normalize_tags(tags) {
        sqlx::query("INSERT INTO task_tags (task_id, tag) VALUES (?, ?)")
            .bind(task_id)
            .bind(tag)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Create a task (and its tags) in a single transaction.
pub async fn create(
    pool: &SqlitePool,
    user_id: i64,
    input: CreateTaskRequest,
) -> Result<Task, AppError> {
    let now = Utc::now().trunc_subsecs(3);
    let mut tx = pool.begin().await?;

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO tasks (user_id, title, description, status, due_date, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(user_id)
    .bind(input.title.trim().to_owned())
    .bind(input.description)
    .bind(input.status.unwrap_or(TaskStatus::Todo))
    .bind(input.due_date)
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    replace_tags(&mut tx, id, &input.tags).await?;
    let task = fetch_one(&mut tx, user_id, id).await?;
    tx.commit().await?;

    task.ok_or_else(|| AppError::Internal(anyhow::anyhow!("created task {id} vanished")))
}

/// Apply a partial update. Returns `None` when the task does not exist or
/// belongs to another user.
pub async fn update(
    pool: &SqlitePool,
    user_id: i64,
    id: i64,
    input: UpdateTaskRequest,
) -> Result<Option<Task>, AppError> {
    let mut tx = pool.begin().await?;

    let Some(current) = fetch_one(&mut tx, user_id, id).await? else {
        return Ok(None);
    };

    let title = input.title.map_or(current.title, |t| t.trim().to_owned());
    let description = input.description.unwrap_or(current.description);
    let status = input.status.unwrap_or(current.status);
    let due_date = input.due_date.unwrap_or(current.due_date);
    let now = Utc::now().trunc_subsecs(3);

    sqlx::query(
        "UPDATE tasks
         SET title = ?, description = ?, status = ?, due_date = ?, updated_at = ?
         WHERE id = ? AND user_id = ?",
    )
    .bind(title)
    .bind(description)
    .bind(status)
    .bind(due_date)
    .bind(now)
    .bind(id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    if let Some(tags) = input.tags {
        replace_tags(&mut tx, id, &tags).await?;
    }

    let task = fetch_one(&mut tx, user_id, id).await?;
    tx.commit().await?;
    Ok(task)
}

/// Delete a task. Returns `false` when nothing was deleted.
pub async fn delete(pool: &SqlitePool, user_id: i64, id: i64) -> Result<bool, AppError> {
    let result = sqlx::query("DELETE FROM tasks WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// List a page of tasks plus the total number of matches.
pub async fn list(
    pool: &SqlitePool,
    user_id: i64,
    params: &ListTasksQuery,
) -> Result<(Vec<Task>, i64), AppError> {
    let mut count_qb = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM tasks t");
    push_filters(&mut count_qb, user_id, params);
    let total: i64 = count_qb.build_query_scalar().fetch_one(pool).await?;

    let mut qb = QueryBuilder::<Sqlite>::new(select_task!());
    push_filters(&mut qb, user_id, params);

    // Sorting: both the column and the direction come from closed enums, so
    // only these fixed SQL fragments can ever be appended (no injection).
    let dir = match params.order {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    };
    let column = match params.sort {
        SortField::CreatedAt => "t.created_at",
        // Tasks without a due date always go last.
        SortField::DueDate => "t.due_date IS NULL, t.due_date",
        SortField::Title => "t.title COLLATE NOCASE",
        SortField::Status => {
            "CASE t.status WHEN 'todo' THEN 0 WHEN 'in_progress' THEN 1 ELSE 2 END"
        }
    };
    qb.push(format_args!(" ORDER BY {column} {dir}, t.id {dir}"));

    let per_page = params.per_page();
    let offset = i64::from(params.page().saturating_sub(1)) * i64::from(per_page);
    qb.push(" LIMIT ")
        .push_bind(i64::from(per_page))
        .push(" OFFSET ")
        .push_bind(offset);

    let rows: Vec<TaskRow> = qb.build_query_as().fetch_all(pool).await?;
    Ok((rows.into_iter().map(Task::from).collect(), total))
}

fn push_filters(qb: &mut QueryBuilder<Sqlite>, user_id: i64, params: &ListTasksQuery) {
    qb.push(" WHERE t.user_id = ").push_bind(user_id);

    if let Some(status) = params.status {
        qb.push(" AND t.status = ").push_bind(status);
    }
    if let Some(tag) = params
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        qb.push(" AND EXISTS (SELECT 1 FROM task_tags ft WHERE ft.task_id = t.id AND ft.tag = ")
            .push_bind(tag.to_lowercase())
            .push(")");
    }
    if let Some(before) = params.due_before {
        qb.push(" AND t.due_date <= ").push_bind(before);
    }
    if let Some(after) = params.due_after {
        qb.push(" AND t.due_date >= ").push_bind(after);
    }
    if let Some(q) = params.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        let pattern = like_pattern(q);
        qb.push(" AND (t.title LIKE ")
            .push_bind(pattern.clone())
            .push(" ESCAPE '\\' OR t.description LIKE ")
            .push_bind(pattern)
            .push(" ESCAPE '\\')");
    }
}

/// Escape LIKE wildcards so user input is matched literally.
fn like_pattern(q: &str) -> String {
    let escaped = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

#[cfg(test)]
mod tests {
    use super::like_pattern;

    #[test]
    fn like_wildcards_are_escaped() {
        assert_eq!(like_pattern("50%_off"), "%50\\%\\_off%");
    }
}
