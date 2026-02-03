pub mod plugins;
pub mod query;
pub mod watch;
pub mod workspace;

/// Shared result type for headless operations.
pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
