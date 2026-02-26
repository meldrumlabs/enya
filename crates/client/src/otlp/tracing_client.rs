//! OTLP tracing client that reads from an in-memory [`TelemetryStore`].

use std::sync::Arc;

use poll_promise::Promise;

use super::store::TelemetryStore;
use crate::error::ClientError;
use crate::tracing::{SearchResult, TraceResult, TracingClient};

/// Tracing client backed by the in-memory OTLP telemetry store.
///
/// Unlike [`TempoClient`](crate::tracing::tempo::TempoClient) which makes HTTP
/// requests, this reads directly from shared memory. Promises resolve immediately.
pub struct OtlpTracingClient {
    store: Arc<TelemetryStore>,
}

impl OtlpTracingClient {
    /// Create a new OTLP tracing client reading from the given store.
    pub fn new(store: Arc<TelemetryStore>) -> Self {
        Self { store }
    }
}

impl TracingClient for OtlpTracingClient {
    fn get_trace(&self, trace_id: &str, _ctx: &egui::Context) -> Promise<TraceResult> {
        let result = self
            .store
            .get_trace(trace_id)
            .ok_or_else(|| ClientError::BackendError {
                status: 404,
                message: format!("Trace {trace_id} not found in OTLP store"),
            });
        Promise::from_ready(result)
    }

    fn search_traces(
        &self,
        params: crate::tracing::tempo::types::TraceSearchParams,
        _ctx: &egui::Context,
    ) -> Promise<SearchResult> {
        let summaries = self.store.search_traces(
            params.service_name.as_deref(),
            params.operation_name.as_deref(),
            params.min_duration_ms.map(|ms| ms * 1000),
            params.max_duration_ms.map(|ms| ms * 1000),
            params.start_time_secs.map(|s| s * 1_000_000),
            params.end_time_secs.map(|s| s * 1_000_000),
            params.limit.unwrap_or(20),
        );
        Promise::from_ready(Ok(summaries))
    }

    fn backend_type(&self) -> &'static str {
        "otlp"
    }
}
