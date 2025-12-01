//! Error types for talna-v2

use std::fmt;

/// Error type for talna-v2 operations
#[derive(Debug)]
pub enum Error {
    /// I/O error
    Io(std::io::Error),
    /// Storage engine error (`SlateDB`)
    Storage(slatedb::Error),
    /// Object store error
    ObjectStore(slatedb::object_store::Error),
    /// Invalid query expression
    InvalidQuery,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Storage(e) => write!(f, "Storage error: {e}"),
            Self::ObjectStore(e) => write!(f, "Object store error: {e}"),
            Self::InvalidQuery => write!(f, "Invalid query"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Storage(e) => Some(e),
            Self::ObjectStore(e) => Some(e),
            Self::InvalidQuery => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<slatedb::Error> for Error {
    fn from(e: slatedb::Error) -> Self {
        Self::Storage(e)
    }
}

impl From<slatedb::object_store::Error> for Error {
    fn from(e: slatedb::object_store::Error) -> Self {
        Self::ObjectStore(e)
    }
}

/// Result type for talna-v2 operations
pub type Result<T> = std::result::Result<T, Error>;
