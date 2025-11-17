//! Enya is an embeddable observability agent that works with open standards
//!
//! For when you don't want to set up a whole Prometheus + Grafana stack.

/// Options enabling Enya customization
pub mod options;

/// Internal enya runtime where each component runs
mod runtime;
/// Axum server hosting API endpoints and Websocket connections
mod server;

use options::Options;

/// Serves the enya UI at 'addr'
pub fn serve(addr: impl Into<String>) {
    serve_with_options(addr, Options::default())
}

/// Serves the enya UI at 'addr' with custom options
pub fn serve_with_options(addr: impl Into<String>, _options: Options) {
    let _addr = addr.into();
    //tracing::info!("Starting enya at {:?}", addr);

    // 1. Set up Enya runtime with options
    // 2. Set up Axum router
}
