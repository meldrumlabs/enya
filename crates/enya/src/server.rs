//! Axum-based server for hosting Enya endpoints

// v1/api/search/
// v1/api/metrics/
// v1/api/memory/
// v1/api/cpu/

use super::core::Core;
use axum::{Json, Router, response::IntoResponse, routing::get};
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
pub(crate) async fn setup_and_serve(
    core: Core,
    addr: SocketAddr,
) -> axum::serve::Serve<tokio::net::TcpListener, axum::Router, axum::Router> {
    // Build the router
    let app = build_router(core);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    //tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app)
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
}

pub async fn health_handler() -> impl IntoResponse {
    Json(Health {
        msg: "Enya is up".to_string(),
    })
}
