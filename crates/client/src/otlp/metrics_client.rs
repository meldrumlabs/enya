//! OTLP metrics client that reads from an in-memory [`TelemetryStore`].

use std::sync::Arc;

use super::store::TelemetryStore;
use crate::prometheus::response::MetricLabels;
use crate::{
    BackendInfo, HealthCheckResult, LabelsResult, MetricLabelsResult, MetricsClient, QueryRequest,
    QueryResult,
};
use poll_promise::Promise;

/// Metrics client backed by the in-memory OTLP telemetry store.
///
/// Unlike [`PrometheusClient`](crate::prometheus::PrometheusClient) which makes
/// HTTP requests to Prometheus, this reads directly from shared memory.
/// Promises resolve immediately.
pub struct OtlpMetricsClient {
    store: Arc<TelemetryStore>,
}

impl OtlpMetricsClient {
    /// Create a new OTLP metrics client reading from the given store.
    pub fn new(store: Arc<TelemetryStore>) -> Self {
        Self { store }
    }
}

impl MetricsClient for OtlpMetricsClient {
    fn query(&self, request: QueryRequest, _ctx: &egui::Context) -> Promise<QueryResult> {
        // Parse time range from nanoseconds
        let now_ns = crate::now_unix_secs() * 1_000_000_000;
        let start_ns = request
            .start
            .unwrap_or((now_ns - 3_600_000_000_000) as u128) as u64;
        let end_ns = request.end.unwrap_or(now_ns as u128) as u64;
        let step_ns = request.step_secs * 1_000_000_000;

        let response = self.store.query_metric(
            &request.metric,
            &rustc_hash::FxHashMap::default(),
            start_ns,
            end_ns,
            step_ns,
        );

        Promise::from_ready(Ok(response))
    }

    fn fetch_label_names(&self, _ctx: &egui::Context) -> Promise<LabelsResult> {
        // Return all unique label names across all metrics
        let names = self.store.metric_names();
        let mut all_labels: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();
        for name in &names {
            for label in self.store.metric_label_names(name) {
                if !label.starts_with("__") {
                    all_labels.insert(label);
                }
            }
        }
        let mut sorted: Vec<String> = all_labels.into_iter().collect();
        sorted.sort();
        Promise::from_ready(Ok(sorted))
    }

    fn fetch_label_values(&self, label: &str, _ctx: &egui::Context) -> Promise<LabelsResult> {
        let names = self.store.metric_names();
        let mut all_values: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();
        for name in &names {
            for value in self.store.metric_label_values(name, label) {
                all_values.insert(value);
            }
        }
        let mut sorted: Vec<String> = all_values.into_iter().collect();
        sorted.sort();
        Promise::from_ready(Ok(sorted))
    }

    fn fetch_metric_names(&self, _ctx: &egui::Context) -> Promise<LabelsResult> {
        let names = self.store.metric_names();
        Promise::from_ready(Ok(names))
    }

    #[allow(clippy::disallowed_types)]
    fn fetch_metric_labels(
        &self,
        metric: &str,
        _ctx: &egui::Context,
    ) -> Promise<MetricLabelsResult> {
        let label_names = self.store.metric_label_names(metric);
        let mut label_values = std::collections::HashMap::new();
        for name in &label_names {
            if name.starts_with("__") {
                continue;
            }
            let values = self.store.metric_label_values(metric, name);
            label_values.insert(name.clone(), values);
        }
        Promise::from_ready(Ok(MetricLabels {
            labels: label_values,
        }))
    }

    fn backend_type(&self) -> &'static str {
        "otlp"
    }

    fn health_check(&self, _ctx: &egui::Context) -> Promise<HealthCheckResult> {
        Promise::from_ready(Ok(BackendInfo {
            backend_type: "otlp".to_string(),
            version: format!(
                "in-memory ({} series, {} points)",
                self.store.metric_series_count(),
                self.store.metric_point_count()
            ),
        }))
    }
}
