//! Error types for datafusion-enya.

use datafusion::error::DataFusionError;
use std::fmt;

/// Errors that can occur during DataFusion metrics collection.
#[derive(Debug)]
pub enum Error {
    /// An error from DataFusion itself.
    DataFusion(DataFusionError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::DataFusion(e) => write!(f, "DataFusion error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::DataFusion(e) => Some(e),
        }
    }
}

impl From<DataFusionError> for Error {
    fn from(e: DataFusionError) -> Self {
        Error::DataFusion(e)
    }
}
