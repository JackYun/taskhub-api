//! Database connection, migrations and repository functions.

pub mod tasks;
pub mod users;

use std::{str::FromStr, time::Duration};

use sqlx::{
    migrate::MigrateError,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};

/// Open a SQLite connection pool.
///
/// In-memory databases (`sqlite::memory:`) live only as long as their
/// connection, so they get a single connection that is never recycled –
/// this is what the integration tests rely on.
pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let in_memory = database_url.contains(":memory:") || database_url.contains("mode=memory");

    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    if in_memory {
        SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await
    } else {
        let options = options
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
        SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await
    }
}

/// Apply the embedded migrations from `migrations/`.
pub async fn migrate(pool: &SqlitePool) -> Result<(), MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// Cheap liveness probe used by `/health`.
pub async fn ping(pool: &SqlitePool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}
