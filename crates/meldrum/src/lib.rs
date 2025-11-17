//! Meldrum is an embeddable observability agent that works with open standards
//!
//! For when you don't want to set up a whole Prometheus + Grafana stack.

/// Options enabling Meldrum customization
pub mod options;

/// Internal meldrum runtime where each component runs
mod runtime;
/// Axum server hosting API endpoints and Websocket connections
mod server;

use options::Options;

/// Serves the meldrum UI at 'addr'
pub fn serve(addr: impl Into<String>) {
    serve_with_options(addr, Options::default())
}

/// Serves the meldrum UI at 'addr' with custom options
pub fn serve_with_options(addr: impl Into<String>, _options: Options) {
    let _addr = addr.into();
    //tracing::info!("Starting meldrum at {:?}", addr);

    // 1. Set up Meldrum runtime with options
    // 2. Set up Axum router
}
