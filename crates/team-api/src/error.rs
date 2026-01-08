//! Error types for the team API.

use thiserror::Error;

/// Errors that can occur when using the team API.
#[derive(Debug, Clone, Error)]
pub enum TeamApiError {
    /// Network request failed.
    #[error("network error: {message}")]
    Network {
        /// Error message.
        message: String,
    },

    /// Server returned an error response.
    #[error("server error ({status}): {message}")]
    Server {
        /// HTTP status code.
        status: u16,
        /// Error message from server.
        message: String,
    },

    /// Authentication failed.
    #[error("authentication failed: {message}")]
    Auth {
        /// Error message.
        message: String,
    },

    /// Resource not found.
    #[error("not found: {resource}")]
    NotFound {
        /// Description of the resource.
        resource: String,
    },

    /// Invalid request parameters.
    #[error("invalid request: {message}")]
    InvalidRequest {
        /// Error message.
        message: String,
    },

    /// Rate limited by server.
    #[error("rate limited: retry after {retry_after_secs}s")]
    RateLimited {
        /// Seconds to wait before retrying.
        retry_after_secs: u64,
    },

    /// Request timed out.
    #[error("request timed out after {elapsed_secs}s")]
    Timeout {
        /// Seconds elapsed before timeout.
        elapsed_secs: u64,
    },

    /// WebSocket connection error.
    #[error("websocket error: {message}")]
    WebSocket {
        /// Error message.
        message: String,
    },

    /// Failed to parse response.
    #[error("parse error: {message}")]
    Parse {
        /// Error message.
        message: String,
    },

    /// Not connected to server.
    #[error("not connected to team server")]
    NotConnected,
}

impl TeamApiError {
    /// Create a network error.
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

    /// Create a server error.
    pub fn server(status: u16, message: impl Into<String>) -> Self {
        Self::Server {
            status,
            message: message.into(),
        }
    }

    /// Create an auth error.
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth {
            message: message.into(),
        }
    }

    /// Create a not found error.
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    /// Create a parse error.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
        }
    }

    /// Returns true if this is a retriable error.
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Network { .. } | Self::Timeout { .. } | Self::RateLimited { .. } => true,
            Self::Server { status, .. } => *status >= 500,
            _ => false,
        }
    }
}

/// Result type for team API operations.
pub type TeamApiResult<T> = Result<T, TeamApiError>;
