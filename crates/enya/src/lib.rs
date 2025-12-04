//! Enya is an embeddable observability agent that works with open standards
//!
//! For when you don't want to set up a whole Prometheus + Grafana stack.

/// Options enabling Enya customization
pub mod options;

/// DataFusion metrics integration.
///
/// Re-exports [`datafusion_enya`] when the `datafusion` feature is enabled.
#[cfg(feature = "datafusion")]
pub mod datafusion {
    pub use datafusion_enya::*;
}

/// Global registry for TaskMonitor instances.
///
/// Available when the `macros` feature is enabled.
#[cfg(feature = "macros")]
pub mod task_registry;

/// Procedural macros for task monitoring.
///
/// Re-exports [`enya_macros`] when the `macros` feature is enabled.
/// These macros provide TaskMonitor instrumentation for async functions.
///
/// When using `#[monitor]`, the monitor is automatically registered
/// with Enya and metrics will be collected periodically.
///
/// # Example
///
/// ```rust,ignore
/// use enya::macros::monitor;
///
/// #[monitor]
/// async fn my_background_task() {
///     // Task work here - metrics collected automatically
/// }
/// ```
#[cfg(feature = "macros")]
pub mod macros {
    pub use enya_macros::*;
}

/// Axum server hosting API endpoints and Websocket connections
mod server;

/// Core enya state used by the server
mod core;
mod util;

mod ingestor;

use std::{fs, net::SocketAddr, path::Path};

use enya_metrics_store::{Database, MetricsStore, object_store};
use ingestor::Ingestor;
use object_store::local::LocalFileSystem;
use options::Options;
use std::sync::Arc;

/// Serves the enya UI at 'addr'
pub async fn serve(addr: impl Into<String>) {
    serve_with_options(addr, Options::default()).await
}

/// Serves the enya UI at 'addr' with custom options
pub async fn serve_with_options(addr: impl Into<String>, options: Options) {
    let addr = addr.into();
    let socket_addr: SocketAddr = addr.parse().expect("Invalid SocketAddr format");

    let build_info = enya_build_info::build_info!();
    let metrics_store = init_metrics_store(options.data_dir(), &build_info).await;
    let core = core::Core::new(build_info, metrics_store);
    let ingestor = Ingestor::spawn(core.clone(), options.task_metrics());
    if let Err(err) = server::setup_and_serve(core, socket_addr).await {
        ingestor.shutdown().await;
        panic!("Failed to start enya server on {socket_addr}: {err}");
    }
    ingestor.shutdown().await;
}

async fn init_metrics_store(
    data_dir: &str,
    build_info: &enya_build_info::BuildInfo,
) -> MetricsStore {
    let metrics_dir = Path::new(data_dir).join("metrics");
    if let Err(err) = fs::create_dir_all(&metrics_dir) {
        panic!("Failed to create metrics directory {metrics_dir:?}: {err}");
    }

    let object_store = Arc::new(
        LocalFileSystem::new_with_prefix(&metrics_dir).unwrap_or_else(|err| {
            panic!("Failed to create object store at {metrics_dir:?}: {err}")
        }),
    );

    let database = Database::builder()
        .open(object_store, "/")
        .await
        .unwrap_or_else(|err| panic!("Failed to open metrics database at {metrics_dir:?}: {err}"));

    let git_ver = (!build_info.git_hash.is_empty()).then(|| build_info.git_hash.to_owned());
    let git_timestamp = (!build_info.datetime.is_empty()).then(|| build_info.datetime.to_owned());

    MetricsStore::new(database, git_ver, git_timestamp)
}
