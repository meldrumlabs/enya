//! Axum-based server for hosting Enya endpoints

// v1/api/search/
// v1/api/metrics/
// v1/api/memory/
// v1/api/cpu/

use super::core::Core;
use axum::{Json, Router, extract::State, response::IntoResponse, routing::get};
use build_info::BuildInfo;
use std::net::SocketAddr;

/// Setup and serve the application on the specified port.
///
/// This function builds the router using the provided core and starts the HTTP server
/// on the specified port. It returns a future that resolves to the server.
///
/// # Arguments
///
/// * `core` - The core application state.
/// * `port` - The port number to listen on.
pub(crate) async fn setup_and_serve(core: Core, addr: SocketAddr) -> Result<(), std::io::Error> {
    // Build the router
    let app = build_router(core);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

/// Set up the Axum router using the core Enya state
pub fn build_router(core: Core) -> Router {
    Router::new()
        .route("/api/health", get(health_handler))
        .with_state(core)
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct Health {
    msg: String,
    version: String,
    git_hash: String,
    git_branch: String,
    built_at: String,
    build_summary: String,
}

impl From<BuildInfo> for Health {
    fn from(build_info: BuildInfo) -> Self {
        Self {
            msg: "Enya is up".to_owned(),
            version: build_info.version.to_string(),
            git_hash: build_info.git_hash_or_tag(),
            git_branch: build_info.git_branch.to_owned(),
            built_at: build_info.datetime.to_owned(),
            build_summary: build_info.to_string(),
        }
    }
}

pub async fn health_handler(State(core): State<Core>) -> impl IntoResponse {
    Json(Health::from(core.build_info()))
}
