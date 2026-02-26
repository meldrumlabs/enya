//! OTLP logs client that reads from an in-memory [`TelemetryStore`].

use std::sync::Arc;

use poll_promise::Promise;
use rustc_hash::FxHashMap;

use super::store::TelemetryStore;
use crate::logs::{LogsClient, LogsQuery, LogsResponse};
use crate::{BackendInfo, HealthCheckResult, LogsResult, StreamsResult};

/// Logs client backed by the in-memory OTLP telemetry store.
///
/// Unlike [`LokiClient`](crate::logs::LokiClient) which makes HTTP requests,
/// this reads directly from shared memory. Promises resolve immediately.
pub struct OtlpLogsClient {
    store: Arc<TelemetryStore>,
}

impl OtlpLogsClient {
    /// Create a new OTLP logs client reading from the given store.
    pub fn new(store: Arc<TelemetryStore>) -> Self {
        Self { store }
    }
}

impl LogsClient for OtlpLogsClient {
    fn query_logs(&self, query: LogsQuery, _ctx: &egui::Context) -> Promise<LogsResult> {
        let entries = self.store.query_logs(
            query.start_ns,
            query.end_ns,
            &query.labels,
            query.contains.as_deref(),
            query.limit,
        );
        let streams_count = {
            let mut services: FxHashMap<&str, ()> = FxHashMap::default();
            for entry in &entries {
                if let Some(svc) = entry.labels.get("service") {
                    services.insert(svc, ());
                }
            }
            services.len().max(1)
        };
        Promise::from_ready(Ok(LogsResponse {
            entries,
            streams_count,
        }))
    }

    fn fetch_streams(&self, _ctx: &egui::Context) -> Promise<StreamsResult> {
        let labels = self.store.known_log_labels();
        Promise::from_ready(Ok(labels))
    }

    fn backend_type(&self) -> &'static str {
        "otlp"
    }

    fn health_check(&self, _ctx: &egui::Context) -> Promise<HealthCheckResult> {
        Promise::from_ready(Ok(BackendInfo {
            backend_type: "otlp".to_string(),
            version: format!(
                "in-memory ({} traces, {} logs)",
                self.store.trace_count(),
                self.store.log_count()
            ),
        }))
    }
}
