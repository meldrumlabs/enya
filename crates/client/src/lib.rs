//! Metrics client abstraction supporting multiple backends.
//!
//! This crate provides a unified interface for querying metrics from different
//! backends (Prometheus, Enya, etc.) using PromQL as the query language.
//!
//! # Architecture
//!
//! The [`MetricsClient`] trait defines a promise-based async interface that all
//! backends implement. Methods return [`Promise`] objects that can be polled
//! each frame in immediate mode GUIs like egui. HTTP requests are handled by
//! `reqwest` which works on both native (with tokio) and WASM (with browser fetch).
//!
//! # Example
//!
//! ```ignore
//! use enya_client::{MetricsClient, QueryRequest};
//! use enya_client::prometheus::PrometheusClient;
//!
//! // Create a client for your backend
//! let client = PrometheusClient::new("http://localhost:9090");
//!
//! // Fire off a query - returns a promise
//! let request = QueryRequest::new("cpu_usage", r#"sum(cpu_usage{env="prod"}) by (host)"#);
//! let promise = client.query(request, &ctx);
//!
//! // In your update loop, poll for results
//! if let Some(result) = promise.ready() {
//!     match result {
//!         Ok(response) => { /* update visualization */ }
//!         Err(e) => { /* show error */ }
//!     }
//! }
//! ```

pub mod demo;
pub mod error;
pub mod logs;
pub mod otlp;
pub mod prometheus;
pub mod promise;
pub mod request;
pub mod tracing;
pub mod types;

pub use demo::DemoMetricsClient;
pub use error::ClientError;
pub use poll_promise::Promise;
pub use promise::promise_channel;
pub use request::QueryRequest;
pub use types::{MetricsBucket, MetricsGroup, QueryResponse, ResultType, Timestamp};

// Re-export MetricLabels for per-metric label data
pub use prometheus::response::MetricLabels;

// Re-export logs types for convenience
pub use logs::{
    DemoLogsClient, LogEntry, LogLevel, LogsClient, LogsQuery, LogsResponse, LogsResult,
    LokiClient, QueryDirection, StreamsResult,
};

/// Get the current Unix timestamp in seconds.
/// Works on both native and WASM platforms.
#[inline]
pub fn now_unix_secs() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        use web_time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        #[allow(clippy::disallowed_types)]
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// Result type for query operations.
pub type QueryResult = Result<QueryResponse, ClientError>;

/// Result type for label list operations.
pub type LabelsResult = Result<Vec<String>, ClientError>;

/// Result type for metric series label operations.
pub type MetricLabelsResult = Result<MetricLabels, ClientError>;

/// Result type for health check operations.
pub type HealthCheckResult = Result<BackendInfo, ClientError>;

/// Backend health/version information from a health check.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackendInfo {
    /// Backend type (e.g., "prometheus", "enya")
    pub backend_type: String,
    /// Version string from the backend
    pub version: String,
}

/// Metrics client trait - promise-based async interface.
///
/// Implementations handle the HTTP communication with the backend. All async methods
/// return [`Promise`] objects that can be polled each frame.
pub trait MetricsClient {
    /// Execute a query request (non-blocking).
    ///
    /// Returns a promise that resolves to the query result.
    /// The `egui::Context` is used to request a repaint when the response is ready.
    fn query(&self, request: QueryRequest, ctx: &egui::Context) -> Promise<QueryResult>;

    /// Fetch all available label names (tag keys) from the backend.
    ///
    /// For Prometheus, this calls `/api/v1/labels`.
    fn fetch_label_names(&self, ctx: &egui::Context) -> Promise<LabelsResult>;

    /// Fetch all values for a specific label (tag key) from the backend.
    ///
    /// For Prometheus, this calls `/api/v1/label/{label}/values`.
    fn fetch_label_values(&self, label: &str, ctx: &egui::Context) -> Promise<LabelsResult>;

    /// Fetch all metric names from the backend.
    ///
    /// For Prometheus, this calls `/api/v1/label/__name__/values`.
    fn fetch_metric_names(&self, ctx: &egui::Context) -> Promise<LabelsResult>;

    /// Fetch labels for a specific metric.
    ///
    /// Returns all label names and their possible values for the given metric.
    /// For Prometheus, this calls `/api/v1/series?match[]={__name__="metric"}`.
    fn fetch_metric_labels(&self, metric: &str, ctx: &egui::Context)
    -> Promise<MetricLabelsResult>;

    /// Get the backend type identifier (e.g., "prometheus", "enya").
    fn backend_type(&self) -> &'static str;

    /// Check backend health and connectivity.
    ///
    /// For Prometheus, this calls `/api/v1/status/buildinfo`.
    /// Returns backend version information on success.
    fn health_check(&self, ctx: &egui::Context) -> Promise<HealthCheckResult>;
}

/// Normalize a base URL: ensure it has an `http://` scheme and strip trailing slashes.
pub fn normalize_url(url: impl Into<String>) -> String {
    let mut url = url.into();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        url = format!("http://{url}");
    }
    if url.ends_with('/') {
        url.pop();
    }
    url
}

/// Simple URL encoding for query parameters.
///
/// Encodes characters that are unsafe in URL query strings. This is intentionally
/// minimal — only characters that would break query parameter parsing are encoded.
pub fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            ' ' => result.push_str("%20"),
            '"' => result.push_str("%22"),
            '#' => result.push_str("%23"),
            '%' => result.push_str("%25"),
            '&' => result.push_str("%26"),
            '+' => result.push_str("%2B"),
            '=' => result.push_str("%3D"),
            '{' => result.push_str("%7B"),
            '}' => result.push_str("%7D"),
            '[' => result.push_str("%5B"),
            ']' => result.push_str("%5D"),
            '|' => result.push_str("%7C"),
            '~' => result.push_str("%7E"),
            _ => result.push(c),
        }
    }
    result
}

/// Default query timeout in seconds.
/// If a query doesn't complete within this time, it will be cancelled with a timeout error.
pub const DEFAULT_QUERY_TIMEOUT_SECS: u64 = 30;

/// Tracks a single in-flight query with its metadata.
struct PendingQuery {
    /// The promise for this query.
    promise: Promise<QueryResult>,
    /// When the query started (Unix timestamp in seconds).
    started_at: u64,
}

/// Manages multiple in-flight queries in parallel using promises.
///
/// Tracks queries by unique ID (typically a pane ID), enabling Grafana-style
/// parallel refresh where all panels query simultaneously.
///
/// Includes timeout detection to prevent queries from hanging indefinitely.
///
/// # Example
///
/// ```ignore
/// let mut manager = QueryManager::new();
///
/// // Fire multiple queries in parallel
/// manager.execute(pane_id_1, &client, request1, &ctx);
/// manager.execute(pane_id_2, &client, request2, &ctx);
///
/// // In update loop, poll for all completed results
/// for (id, result) in manager.poll_all() {
///     // Handle result for pane with this id
/// }
/// ```
pub struct QueryManager {
    /// Pending queries keyed by their unique ID.
    pending: rustc_hash::FxHashMap<usize, PendingQuery>,
    /// Timeout duration in seconds.
    timeout_secs: u64,
}

impl Default for QueryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryManager {
    /// Create a new query manager with the default timeout.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: rustc_hash::FxHashMap::default(),
            timeout_secs: DEFAULT_QUERY_TIMEOUT_SECS,
        }
    }

    /// Create a new query manager with a custom timeout.
    #[must_use]
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            pending: rustc_hash::FxHashMap::default(),
            timeout_secs,
        }
    }

    /// Check if any queries are currently in flight.
    #[must_use]
    pub fn is_querying(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Check if a specific query is in flight.
    #[must_use]
    pub fn is_querying_id(&self, id: usize) -> bool {
        self.pending.contains_key(&id)
    }

    /// Get the number of queries currently in flight.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Execute a query for the given ID using the given client.
    ///
    /// If a query for this ID is already in flight, it is cancelled and replaced.
    /// Call `poll_all()` each frame to check for results.
    pub fn execute<C: MetricsClient + ?Sized>(
        &mut self,
        id: usize,
        client: &C,
        request: QueryRequest,
        ctx: &egui::Context,
    ) {
        let promise = client.query(request, ctx);
        self.pending.insert(
            id,
            PendingQuery {
                promise,
                started_at: now_unix_secs(),
            },
        );
    }

    /// Poll for all completed query results.
    ///
    /// Returns a vector of `(id, result)` pairs for queries that completed or timed out.
    /// Completed queries are removed from the pending set.
    pub fn poll_all(&mut self) -> Vec<(usize, QueryResult)> {
        let now = now_unix_secs();
        let mut completed = Vec::new();
        let mut to_remove = Vec::new();

        for (&id, pending) in &self.pending {
            // Check if completed
            if let Some(result) = pending.promise.ready() {
                completed.push((id, result.clone()));
                to_remove.push(id);
                continue;
            }

            // Check for timeout
            let elapsed = now.saturating_sub(pending.started_at);
            if elapsed >= self.timeout_secs {
                log::warn!(
                    "Query {id} timed out after {elapsed} seconds (timeout: {}s)",
                    self.timeout_secs
                );
                completed.push((
                    id,
                    Err(ClientError::Timeout {
                        elapsed_secs: elapsed,
                        timeout_secs: self.timeout_secs,
                    }),
                ));
                to_remove.push(id);
            }
        }

        // Remove completed/timed-out queries
        for id in to_remove {
            self.pending.remove(&id);
        }

        completed
    }

    /// Cancel a specific query by ID.
    ///
    /// Note: This doesn't actually cancel the HTTP request, but it will ignore
    /// the result when it arrives.
    pub fn cancel(&mut self, id: usize) {
        self.pending.remove(&id);
    }

    /// Cancel all pending queries.
    pub fn cancel_all(&mut self) {
        self.pending.clear();
    }
}

/// Manages in-flight label/metadata fetches using promises.
///
/// Similar to [`QueryManager`], but for metadata operations like
/// fetching label names, label values, and metric names.
///
/// # Example
///
/// ```ignore
/// let mut manager = LabelsManager::new();
///
/// // Fetch all label names
/// manager.fetch_label_names(&client, &ctx);
///
/// // In update loop
/// if let Some(result) = manager.poll() {
///     match result {
///         Ok(labels) => { /* update autocomplete */ }
///         Err(e) => { /* show error */ }
///     }
/// }
/// ```
pub struct LabelsManager {
    /// The pending promise, if any.
    promise: Option<Promise<LabelsResult>>,
}

impl Default for LabelsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LabelsManager {
    /// Create a new labels manager.
    #[must_use]
    pub fn new() -> Self {
        Self { promise: None }
    }

    /// Check if a fetch is currently in flight.
    #[must_use]
    pub fn is_fetching(&self) -> bool {
        self.promise.is_some()
    }

    /// Fetch all label names from the backend.
    ///
    /// If a fetch is already in flight, this does nothing.
    pub fn fetch_label_names<C: MetricsClient + ?Sized>(
        &mut self,
        client: &C,
        ctx: &egui::Context,
    ) {
        if self.promise.is_some() {
            return;
        }

        self.promise = Some(client.fetch_label_names(ctx));
    }

    /// Fetch all values for a specific label.
    ///
    /// If a fetch is already in flight, this does nothing.
    pub fn fetch_label_values<C: MetricsClient + ?Sized>(
        &mut self,
        client: &C,
        label: &str,
        ctx: &egui::Context,
    ) {
        if self.promise.is_some() {
            return;
        }

        self.promise = Some(client.fetch_label_values(label, ctx));
    }

    /// Fetch all metric names from the backend.
    ///
    /// If a fetch is already in flight, this does nothing.
    pub fn fetch_metric_names<C: MetricsClient + ?Sized>(
        &mut self,
        client: &C,
        ctx: &egui::Context,
    ) {
        if self.promise.is_some() {
            return;
        }

        self.promise = Some(client.fetch_metric_names(ctx));
    }

    /// Poll for the fetch result.
    ///
    /// Returns `Some(result)` if a fetch just completed, `None` otherwise.
    pub fn poll(&mut self) -> Option<LabelsResult> {
        let promise = self.promise.as_ref()?;
        if let Some(result) = promise.ready() {
            let result = result.clone();
            self.promise = None;
            Some(result)
        } else {
            None
        }
    }

    /// Cancel any pending fetch.
    pub fn cancel(&mut self) {
        self.promise = None;
    }
}

/// Manages in-flight per-metric label fetches using promises.
///
/// Similar to [`LabelsManager`], but specifically for fetching
/// label names and values for a single metric.
pub struct MetricLabelsManager {
    /// The pending promise, if any.
    promise: Option<Promise<MetricLabelsResult>>,
    /// The metric name being fetched (for cache key purposes).
    metric: Option<String>,
}

impl Default for MetricLabelsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricLabelsManager {
    /// Create a new metric labels manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            promise: None,
            metric: None,
        }
    }

    /// Check if a fetch is currently in flight.
    #[must_use]
    pub fn is_fetching(&self) -> bool {
        self.promise.is_some()
    }

    /// Get the metric name currently being fetched.
    #[must_use]
    pub fn fetching_metric(&self) -> Option<&str> {
        self.metric.as_deref()
    }

    /// Fetch labels for a specific metric.
    ///
    /// If a fetch is already in flight for a different metric, it is cancelled.
    pub fn fetch<C: MetricsClient + ?Sized>(
        &mut self,
        client: &C,
        metric: &str,
        ctx: &egui::Context,
    ) {
        // If already fetching this metric, do nothing
        if self.metric.as_deref() == Some(metric) && self.promise.is_some() {
            return;
        }

        // Cancel any existing fetch
        self.cancel();

        self.metric = Some(metric.to_string());
        self.promise = Some(client.fetch_metric_labels(metric, ctx));
    }

    /// Poll for the fetch result.
    ///
    /// Returns `Some((metric_name, result))` if a fetch just completed, `None` otherwise.
    pub fn poll(&mut self) -> Option<(String, MetricLabelsResult)> {
        let promise = self.promise.as_ref()?;
        if let Some(result) = promise.ready() {
            let result = result.clone();
            let metric = self.metric.take().unwrap_or_default();
            self.promise = None;
            Some((metric, result))
        } else {
            None
        }
    }

    /// Cancel any pending fetch.
    pub fn cancel(&mut self) {
        self.promise = None;
        self.metric = None;
    }
}

/// Manages in-flight health check requests using promises.
///
/// Similar to [`LabelsManager`], but specifically for checking
/// backend connectivity and version information.
pub struct HealthCheckManager {
    /// The pending promise, if any.
    promise: Option<Promise<HealthCheckResult>>,
}

impl Default for HealthCheckManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthCheckManager {
    /// Create a new health check manager.
    #[must_use]
    pub fn new() -> Self {
        Self { promise: None }
    }

    /// Check if a health check is currently in flight.
    #[must_use]
    pub fn is_checking(&self) -> bool {
        self.promise.is_some()
    }

    /// Initiate a health check on the given client.
    ///
    /// If a check is already in flight, this does nothing.
    pub fn check<C: MetricsClient + ?Sized>(&mut self, client: &C, ctx: &egui::Context) {
        if self.promise.is_some() {
            return;
        }

        self.promise = Some(client.health_check(ctx));
    }

    /// Poll for the health check result.
    ///
    /// Returns `Some(result)` if a check just completed, `None` otherwise.
    pub fn poll(&mut self) -> Option<HealthCheckResult> {
        let promise = self.promise.as_ref()?;
        if let Some(result) = promise.ready() {
            let result = result.clone();
            self.promise = None;
            Some(result)
        } else {
            None
        }
    }

    /// Cancel any pending health check.
    pub fn cancel(&mut self) {
        self.promise = None;
    }
}

/// Tracks a single in-flight logs query with its metadata.
struct PendingLogsQuery {
    /// The promise for this query.
    promise: Promise<LogsResult>,
    /// When the query started (Unix timestamp in seconds).
    started_at: u64,
}

/// Manages multiple in-flight log queries in parallel using promises.
///
/// Similar to [`QueryManager`] but for log queries. Tracks queries by unique ID,
/// enabling parallel log fetching for multiple time ranges or filters.
///
/// Includes timeout detection to prevent queries from hanging indefinitely.
///
/// # Example
///
/// ```ignore
/// let mut manager = LogsQueryManager::new();
///
/// // Fire multiple log queries in parallel
/// manager.execute(pane_id_1, &client, query1, &ctx);
/// manager.execute(pane_id_2, &client, query2, &ctx);
///
/// // In update loop, poll for all completed results
/// for (id, result) in manager.poll_all() {
///     // Handle result for pane with this id
/// }
/// ```
pub struct LogsQueryManager {
    /// Pending queries keyed by their unique ID.
    pending: rustc_hash::FxHashMap<usize, PendingLogsQuery>,
    /// Timeout duration in seconds.
    timeout_secs: u64,
}

impl Default for LogsQueryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LogsQueryManager {
    /// Create a new logs query manager with the default timeout.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: rustc_hash::FxHashMap::default(),
            timeout_secs: DEFAULT_QUERY_TIMEOUT_SECS,
        }
    }

    /// Create a new logs query manager with a custom timeout.
    #[must_use]
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            pending: rustc_hash::FxHashMap::default(),
            timeout_secs,
        }
    }

    /// Check if any queries are currently in flight.
    #[must_use]
    pub fn is_querying(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Check if a specific query is in flight.
    #[must_use]
    pub fn is_querying_id(&self, id: usize) -> bool {
        self.pending.contains_key(&id)
    }

    /// Get the number of queries currently in flight.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Execute a logs query for the given ID using the given client.
    ///
    /// If a query for this ID is already in flight, it is cancelled and replaced.
    /// Call `poll_all()` each frame to check for results.
    pub fn execute<C: LogsClient + ?Sized>(
        &mut self,
        id: usize,
        client: &C,
        query: LogsQuery,
        ctx: &egui::Context,
    ) {
        let promise = client.query_logs(query, ctx);
        self.pending.insert(
            id,
            PendingLogsQuery {
                promise,
                started_at: now_unix_secs(),
            },
        );
    }

    /// Poll for all completed query results.
    ///
    /// Returns a vector of `(id, result)` pairs for queries that completed or timed out.
    /// Completed queries are removed from the pending set.
    pub fn poll_all(&mut self) -> Vec<(usize, LogsResult)> {
        let now = now_unix_secs();
        let mut completed = Vec::new();
        let mut to_remove = Vec::new();

        for (&id, pending) in &self.pending {
            // Check if completed
            if let Some(result) = pending.promise.ready() {
                completed.push((id, result.clone()));
                to_remove.push(id);
                continue;
            }

            // Check for timeout
            let elapsed = now.saturating_sub(pending.started_at);
            if elapsed >= self.timeout_secs {
                log::warn!(
                    "Logs query {id} timed out after {elapsed} seconds (timeout: {}s)",
                    self.timeout_secs
                );
                completed.push((
                    id,
                    Err(ClientError::Timeout {
                        elapsed_secs: elapsed,
                        timeout_secs: self.timeout_secs,
                    }),
                ));
                to_remove.push(id);
            }
        }

        // Remove completed/timed-out queries
        for id in to_remove {
            self.pending.remove(&id);
        }

        completed
    }

    /// Cancel a specific query by ID.
    ///
    /// Note: This doesn't actually cancel the HTTP request, but it will ignore
    /// the result when it arrives.
    pub fn cancel(&mut self, id: usize) {
        self.pending.remove(&id);
    }

    /// Cancel all pending queries.
    pub fn cancel_all(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_request_builder() {
        let request = QueryRequest::new("cpu_usage", "sum(env:prod)")
            .with_step(30)
            .with_range(1000, 2000);

        assert_eq!(request.metric, "cpu_usage");
        assert_eq!(request.query, "sum(env:prod)");
        assert_eq!(request.step_secs, 30);
        assert_eq!(request.start, Some(1000));
        assert_eq!(request.end, Some(2000));
    }

    #[test]
    fn test_query_manager_initial_state() {
        let manager = QueryManager::new();
        assert!(!manager.is_querying());
    }

    #[test]
    fn test_labels_manager_initial_state() {
        let manager = LabelsManager::new();
        assert!(!manager.is_fetching());
    }

    #[test]
    fn test_logs_query_manager_initial_state() {
        let manager = LogsQueryManager::new();
        assert!(!manager.is_querying());
        assert_eq!(manager.pending_count(), 0);
    }

    #[test]
    fn test_normalize_url_adds_http() {
        assert_eq!(normalize_url("localhost:9090"), "http://localhost:9090");
    }

    #[test]
    fn test_normalize_url_preserves_https() {
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
    }

    #[test]
    fn test_normalize_url_strips_trailing_slash() {
        assert_eq!(
            normalize_url("http://localhost:9090/"),
            "http://localhost:9090"
        );
    }

    #[test]
    fn test_normalize_url_no_change_needed() {
        assert_eq!(
            normalize_url("http://localhost:9090"),
            "http://localhost:9090"
        );
    }

    #[test]
    fn test_url_encode_simple() {
        assert_eq!(url_encode("simple"), "simple");
        assert_eq!(url_encode("hello world"), "hello%20world");
    }

    #[test]
    fn test_url_encode_special_chars() {
        assert_eq!(url_encode("{app=\"test\"}"), "%7Bapp%3D%22test%22%7D");
        assert_eq!(url_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(url_encode("[1,2]"), "%5B1,2%5D");
        assert_eq!(url_encode("a+b"), "a%2Bb");
        assert_eq!(url_encode("a|b"), "a%7Cb");
        assert_eq!(url_encode("100%"), "100%25");
    }

    #[test]
    fn test_client_error_display() {
        let err = ClientError::TranslationError("OR not supported".to_string());
        assert_eq!(
            err.to_string(),
            "query translation failed: OR not supported"
        );

        let err = ClientError::BackendError {
            status: 400,
            message: "bad query".to_string(),
        };
        assert_eq!(err.to_string(), "backend error (HTTP 400): bad query");
    }
}
