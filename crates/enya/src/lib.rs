//! Enya is an embeddable observability agent that works with open standards
//!
//! For when you don't want to set up a whole Prometheus + Grafana stack.

/// Options enabling Enya customization
pub mod options;

/// Axum server hosting API endpoints and Websocket connections
mod server;

/// Core enya state used by the server
mod core;
mod util;

mod ingestor;

use std::{fs, net::SocketAddr, path::Path};

use enya_metrics_store::MetricsStore;
use ingestor::Ingestor;
use options::Options;
use talna::Database;

/// Serves the enya UI at 'addr'
pub async fn serve(addr: impl Into<String>) {
    serve_with_options(addr, Options::default()).await
}

/// Serves the enya UI at 'addr' with custom options
pub async fn serve_with_options(addr: impl Into<String>, options: Options) {
    let addr = addr.into();
    let socket_addr: SocketAddr = addr.parse().expect("Invalid SocketAddr format");

    let build_info = build_info::build_info!();
    let metrics_store = init_metrics_store(options.data_dir(), &build_info);
    let core = core::Core::new(build_info, metrics_store);
    let ingestor = Ingestor::spawn(core.clone());
    if let Err(err) = server::setup_and_serve(core, socket_addr).await {
        ingestor.shutdown().await;
        panic!("Failed to start enya server on {socket_addr}: {err}");
    }
    ingestor.shutdown().await;
}

fn init_metrics_store(data_dir: &str, build_info: &build_info::BuildInfo) -> MetricsStore {
    let metrics_dir = Path::new(data_dir).join("metrics");
    if let Err(err) = fs::create_dir_all(&metrics_dir) {
        panic!("Failed to create metrics directory {metrics_dir:?}: {err}");
    }

    let database = Database::builder()
        .open(&metrics_dir)
        .unwrap_or_else(|err| panic!("Failed to open talna database at {metrics_dir:?}: {err}"));

    let git_ver = (!build_info.git_hash.is_empty()).then(|| build_info.git_hash.to_owned());
    let git_timestamp = (!build_info.datetime.is_empty()).then(|| build_info.datetime.to_owned());

    MetricsStore::new(database, git_ver, git_timestamp)
}
