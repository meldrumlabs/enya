//! Error types for DataFusion operations.

use std::fmt;

/// Errors that can occur during DataFusion operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Query execution failed.
    #[error("query execution failed: {0}")]
    Execution(#[from] datafusion::error::DataFusionError),

    /// Failed to register a table.
    #[error("failed to register table '{name}': {source}")]
    TableRegistration {
        name: String,
        #[source]
        source: datafusion::error::DataFusionError,
    },

    /// File format not supported or detection failed.
    #[error("unsupported file format: {0}")]
    UnsupportedFormat(String),

    /// Object store error (S3, GCS, etc.).
    #[error("object store error: {0}")]
    ObjectStore(#[from] object_store::Error),

    /// Arrow/data conversion error.
    #[error("data conversion error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    /// Query was cancelled.
    #[error("query cancelled")]
    Cancelled,

    /// Query timed out.
    #[error("query timed out after {elapsed_secs}s (limit: {timeout_secs}s)")]
    Timeout {
        elapsed_secs: u64,
        timeout_secs: u64,
    },

    /// Channel communication error.
    #[error("internal channel error: {0}")]
    Channel(String),

    /// Invalid query ID.
    #[error("unknown query ID: {0}")]
    UnknownQuery(QueryId),

    /// Session not initialized.
    #[error("session not initialized")]
    NotInitialized,

    /// Flight SQL connection error.
    #[error("flight connection to '{endpoint}' failed: {message}")]
    FlightConnection { endpoint: String, message: String },

    /// Flight SQL query error.
    #[error("flight query failed: {message} (sql: {sql})")]
    FlightQuery { sql: String, message: String },

    /// Flight SQL metadata error.
    #[error("flight metadata error: {message}")]
    FlightMetadata { message: String },

    /// Flight SQL stream error.
    #[error("flight stream error: {message}")]
    FlightStream { message: String },
}

/// Unique identifier for a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueryId(u64);

impl QueryId {
    /// Create a new unique query ID.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for QueryId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for QueryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "query-{}", self.0)
    }
}
