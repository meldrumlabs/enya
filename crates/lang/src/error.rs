//! Error types for the query language.

use std::fmt;

/// Query language errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Invalid query syntax.
    InvalidQuery,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQuery => write!(f, "invalid query syntax"),
        }
    }
}

impl std::error::Error for Error {}

/// Result type for query language operations.
pub type Result<T> = std::result::Result<T, Error>;
