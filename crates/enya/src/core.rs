use build_info::BuildInfo;
use enya_metrics_store::MetricsStore;

#[derive(Clone)]
pub struct Core {
    build_info: BuildInfo,
    metrics_store: MetricsStore,
}

impl Core {
    pub fn new(build_info: BuildInfo, metrics_store: MetricsStore) -> Self {
        Self {
            build_info,
            metrics_store,
        }
    }

    pub fn build_info(&self) -> BuildInfo {
        self.build_info
    }

    /// Returns the metrics store shared across the Axum handlers.
    pub fn metrics(&self) -> &MetricsStore {
        &self.metrics_store
    }
}
