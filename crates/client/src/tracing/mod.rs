//! Tracing client abstraction for distributed trace backends.
//!
//! This module provides a unified interface for querying traces from different
//! backends (Grafana Tempo, Jaeger, etc.) using the TracingClient trait.
//!
//! # Architecture
//!
//! The [`TracingClient`] trait defines a promise-based async interface that all
//! backends implement. Methods return [`Promise`] objects that can be polled
//! each frame in immediate mode GUIs like egui.
//!
//! # Example
//!
//! ```ignore
//! use enya_client::tracing::{TracingClient, TraceSearchParams};
//! use enya_client::tracing::tempo::TempoClient;
//!
//! // Create a client for your backend
//! let client = TempoClient::new("http://localhost:3200");
//!
//! // Fire off a trace query - returns a promise
//! let promise = client.get_trace("abc123def456", &ctx);
//!
//! // In your update loop, poll for results
//! if let Some(result) = promise.ready() {
//!     match result {
//!         Ok(trace) => { /* render waterfall */ }
//!         Err(e) => { /* show error */ }
//!     }
//! }
//! ```

pub mod tempo;

// Re-export common types from tempo (these are backend-agnostic)
pub use poll_promise::Promise;
pub use tempo::types::{
    Span, SpanLog, SpanStatus, Trace, TraceId, TraceSearchParams, TraceSummary, format_duration_us,
};

use crate::error::ClientError;

/// Result type for trace fetch operations.
pub type TraceResult = Result<Trace, ClientError>;

/// Result type for trace search operations.
pub type SearchResult = Result<Vec<TraceSummary>, ClientError>;

/// Tracing client trait - promise-based async interface.
///
/// Implementations handle the HTTP communication with the backend. All async methods
/// return [`Promise`] objects that can be polled each frame.
pub trait TracingClient {
    /// Fetch a trace by its ID (non-blocking).
    ///
    /// Returns a promise that resolves to the full trace with all spans.
    /// The `egui::Context` is used to request a repaint when the response is ready.
    fn get_trace(&self, trace_id: &str, ctx: &egui::Context) -> Promise<TraceResult>;

    /// Search for traces matching the given parameters (non-blocking).
    ///
    /// Returns a promise that resolves to a list of trace summaries.
    fn search_traces(
        &self,
        params: TraceSearchParams,
        ctx: &egui::Context,
    ) -> Promise<SearchResult>;

    /// Get the backend type identifier (e.g., "tempo", "jaeger").
    fn backend_type(&self) -> &'static str;
}

/// Manages in-flight trace fetch requests using promises.
///
/// Similar to [`QueryManager`](crate::QueryManager) for metrics, but for trace operations.
pub struct TraceManager {
    /// Pending trace fetch promise.
    trace_promise: Option<Promise<TraceResult>>,
    /// Pending search promise.
    search_promise: Option<Promise<SearchResult>>,
}

impl Default for TraceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceManager {
    /// Create a new trace manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            trace_promise: None,
            search_promise: None,
        }
    }

    /// Check if a trace fetch is in flight.
    #[must_use]
    pub fn is_fetching_trace(&self) -> bool {
        self.trace_promise.is_some()
    }

    /// Check if a search is in flight.
    #[must_use]
    pub fn is_searching(&self) -> bool {
        self.search_promise.is_some()
    }

    /// Fetch a trace by ID.
    ///
    /// If a fetch is already in flight, it is cancelled and replaced.
    pub fn fetch_trace<C: TracingClient + ?Sized>(
        &mut self,
        client: &C,
        trace_id: &str,
        ctx: &egui::Context,
    ) {
        self.trace_promise = Some(client.get_trace(trace_id, ctx));
    }

    /// Search for traces.
    ///
    /// If a search is already in flight, it is cancelled and replaced.
    pub fn search<C: TracingClient + ?Sized>(
        &mut self,
        client: &C,
        params: TraceSearchParams,
        ctx: &egui::Context,
    ) {
        self.search_promise = Some(client.search_traces(params, ctx));
    }

    /// Poll for trace fetch result.
    ///
    /// Returns `Some(result)` if a fetch just completed, `None` otherwise.
    pub fn poll_trace(&mut self) -> Option<TraceResult> {
        let promise = self.trace_promise.as_ref()?;
        if let Some(result) = promise.ready() {
            let result = result.clone();
            self.trace_promise = None;
            Some(result)
        } else {
            None
        }
    }

    /// Poll for search result.
    ///
    /// Returns `Some(result)` if a search just completed, `None` otherwise.
    pub fn poll_search(&mut self) -> Option<SearchResult> {
        let promise = self.search_promise.as_ref()?;
        if let Some(result) = promise.ready() {
            let result = result.clone();
            self.search_promise = None;
            Some(result)
        } else {
            None
        }
    }

    /// Cancel any pending trace fetch.
    pub fn cancel_trace(&mut self) {
        self.trace_promise = None;
    }

    /// Cancel any pending search.
    pub fn cancel_search(&mut self) {
        self.search_promise = None;
    }

    /// Cancel all pending operations.
    pub fn cancel_all(&mut self) {
        self.trace_promise = None;
        self.search_promise = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_manager_initial_state() {
        let manager = TraceManager::new();
        assert!(!manager.is_fetching_trace());
        assert!(!manager.is_searching());
    }
}
