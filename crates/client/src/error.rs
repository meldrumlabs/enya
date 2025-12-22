//! Error types for the metrics client.

use std::fmt;

/// Errors that can occur when executing metrics queries.
#[derive(Debug, Clone)]
pub enum ClientError {
    /// The enya-lang query could not be translated to the backend's query language.
    TranslationError(String),
    /// Network error during request.
    NetworkError(String),
    /// Backend returned an error response.
    BackendError { status: u16, message: String },
    /// Failed to parse the backend's response.
    ParseError(String),
    /// Query timed out waiting for a response.
    Timeout {
        /// How long we waited before timing out.
        elapsed_secs: u64,
        /// The configured timeout threshold.
        timeout_secs: u64,
    },
    /// Invalid request parameters.
    InvalidRequest(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TranslationError(msg) => write!(f, "query translation failed: {msg}"),
            Self::NetworkError(msg) => write!(f, "network error: {msg}"),
            Self::BackendError { status, message } => {
                write!(f, "backend error (HTTP {status}): {message}")
            }
            Self::ParseError(msg) => write!(f, "failed to parse response: {msg}"),
            Self::Timeout {
                elapsed_secs,
                timeout_secs,
            } => write!(
                f,
                "query timed out after {elapsed_secs}s (limit: {timeout_secs}s)"
            ),
            Self::InvalidRequest(msg) => write!(f, "invalid request: {msg}"),
        }
    }
}

impl std::error::Error for ClientError {}
