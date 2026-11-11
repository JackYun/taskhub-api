use anyhow::Context;
use taskhub_api::{build_app, db, AppState, Config};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // A missing .env file is fine; real deployments use real env vars.
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("taskhub_api=info,tower_http=info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env().context("invalid configuration")?;
    tracing::info!(?config, "configuration loaded");

    let pool = db::connect(&config.database_url)
        .await
        .context("failed to open database")?;
    db::migrate(&pool)
        .await
        .context("failed to run migrations")?;
    tracing::info!("database migrations applied");

    let bind_addr = config.bind_addr;
    let app = build_app(AppState::new(pool.clone(), config));

    let listener = TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind {bind_addr}"))?;
    tracing::info!("listening on http://{bind_addr} (Swagger UI at /swagger-ui)");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    pool.close().await;
    tracing::info!("shutdown complete");
    Ok(())
}

/// Resolves on Ctrl+C, or SIGTERM on Unix (e.g. `docker stop`).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!(error = %e, "failed to listen for Ctrl+C");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => tracing::error!(error = %e, "failed to listen for SIGTERM"),
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received, draining connections");
}
