//! Enya is an embeddable observability agent that works with open standards
//!
//! For when you don't want to set up a whole Prometheus + Grafana stack.

/// Options enabling Enya customization
pub mod options;

/// Axum server hosting API endpoints and Websocket connections
mod server;

/// Core enya state used by the server
mod core;

use std::net::SocketAddr;

use options::Options;

/// Serves the enya UI at 'addr'
pub async fn serve(addr: impl Into<String>) {
    serve_with_options(addr, Options::default()).await
}

/// Serves the enya UI at 'addr' with custom options
pub async fn serve_with_options(addr: impl Into<String>, _options: Options) {
    let addr = addr.into();
    let socket_addr: SocketAddr = addr.parse().expect("Invalid SocketAddr format");

    let build_info = build_info::build_info!();
    let core = core::Core::new(build_info);
    if let Err(err) = server::setup_and_serve(core, socket_addr).await {
        panic!("Failed to start enya server on {socket_addr}: {err}");
    }
}
