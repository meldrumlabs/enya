//! Typed errors for the Enya agent crate.

/// Errors that can occur in the agent.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// I/O error (stdin/stdout in session, socket binding, runtime).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// SQLite database error.
    #[cfg(feature = "serve")]
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    /// Configuration or setup error.
    #[error("{0}")]
    Config(String),
}
