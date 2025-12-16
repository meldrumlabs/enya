//! Query request types.

use enya_common::api::Timestamp;

/// A metrics query request.
#[derive(Debug, Clone)]
pub struct QueryRequest {
    /// The metric name (e.g., "cpu_usage", "http_requests_total").
    pub metric: String,

    /// The enya-lang query string (e.g., "sum(env:prod) by (region)").
    /// This will be translated to the backend's native query language.
    pub query: String,

    /// Start of the query time range (nanoseconds since epoch).
    /// If None, defaults to backend-specific behavior (e.g., 1 hour ago).
    pub start: Option<Timestamp>,

    /// End of the query time range (nanoseconds since epoch).
    /// If None, defaults to now.
    pub end: Option<Timestamp>,

    /// Query step/granularity in seconds.
    /// This determines the resolution of the returned data points.
    pub step_secs: u64,
}

impl QueryRequest {
    /// Create a new query request with the given metric and query.
    #[must_use]
    pub fn new(metric: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            metric: metric.into(),
            query: query.into(),
            start: None,
            end: None,
            step_secs: 60, // Default 1 minute resolution
        }
    }

    /// Set the time range for the query.
    #[must_use]
    pub fn with_range(mut self, start: Timestamp, end: Timestamp) -> Self {
        self.start = Some(start);
        self.end = Some(end);
        self
    }

    /// Set the query step/granularity in seconds.
    #[must_use]
    pub fn with_step(mut self, step_secs: u64) -> Self {
        self.step_secs = step_secs;
        self
    }
}
