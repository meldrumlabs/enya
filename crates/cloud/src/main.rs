//! Enya Cloud - Team collaboration backend.
//!
//! This is the SaaS backend for Enya's team collaboration features:
//! - User authentication (GitHub, Slack OAuth)
//! - Team management
//! - Annotations and threaded discussions
//! - Real-time updates via WebSocket
//! - War room mode for incidents

use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod auth;
mod config;
mod db;
mod error;
pub mod metrics;
mod realtime;
mod state;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "enya_cloud=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize Prometheus metrics
    let metrics_handle = metrics::init_metrics();

    // Load configuration
    let config = Config::from_env()?;
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;

    // Initialize database
    let pool = db::create_pool(&config.database_url).await?;

    // Run migrations
    tracing::info!("Running database migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Create app state
    let state = AppState::new(config.clone(), pool);

    // Build router
    let app = api::router(state, metrics_handle);

    // Start server
    tracing::info!("Starting server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
