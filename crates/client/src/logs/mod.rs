//! Logs client abstraction supporting multiple backends.
//!
//! This module provides a unified interface for querying logs from different
//! backends (Loki, Elasticsearch, etc.).
//!
//! # Architecture
//!
//! The [`LogsClient`] trait defines a promise-based async interface that all
//! backends implement. Methods return [`Promise`] objects that can be polled
//! each frame in immediate mode GUIs like egui.
//!
//! # Example
//!
//! ```ignore
//! use enya_client::logs::{LogsClient, LogsQuery, LokiClient};
//!
//! // Create a client for your backend
//! let client = LokiClient::new("http://localhost:3100");
//!
//! // Fire off a query - returns a promise
//! let query = LogsQuery::new(start_ns, end_ns)
//!     .with_label("app", "myservice")
//!     .with_contains("SELECT");
//! let promise = client.query_logs(query, &ctx);
//!
//! // In your update loop, poll for results
//! if let Some(result) = promise.ready() {
//!     match result {
//!         Ok(response) => { /* display logs */ }
//!         Err(e) => { /* show error */ }
//!     }
//! }
//! ```

pub mod demo;
pub mod loki;
mod types;

pub use demo::DemoLogsClient;
pub use loki::LokiClient;
pub use types::{LogEntry, LogLevel, LogsQuery, LogsResponse, QueryDirection};

use crate::HealthCheckResult;
use crate::error::ClientError;
use poll_promise::Promise;

/// Result type for log query operations.
pub type LogsResult = Result<LogsResponse, ClientError>;

/// Result type for stream list operations.
pub type StreamsResult = Result<Vec<String>, ClientError>;

/// Logs client trait - promise-based async interface.
///
/// Implementations handle the HTTP communication with the backend. All async methods
/// return [`Promise`] objects that can be polled each frame.
pub trait LogsClient {
    /// Execute a logs query (non-blocking).
    ///
    /// Returns a promise that resolves to the query result.
    /// The `egui::Context` is used to request a repaint when the response is ready.
    fn query_logs(&self, query: LogsQuery, ctx: &egui::Context) -> Promise<LogsResult>;

    /// Fetch all available log streams/labels from the backend.
    ///
    /// For Loki, this calls `/loki/api/v1/labels`.
    fn fetch_streams(&self, ctx: &egui::Context) -> Promise<StreamsResult>;

    /// Get the backend type identifier (e.g., "loki", "elasticsearch").
    fn backend_type(&self) -> &'static str;

    /// Check backend health and connectivity.
    ///
    /// For Loki, this calls `/loki/api/v1/status/buildinfo`.
    /// Returns backend version information on success.
    fn health_check(&self, ctx: &egui::Context) -> Promise<HealthCheckResult>;
}
