//! DataFusion integration for Enya editor.
//!
//! This crate provides a unified interface for executing SQL queries using Apache DataFusion,
//! with support for local files, S3/GCS object stores, and various file formats (Parquet, CSV, JSON).
//!
//! # Architecture
//!
//! The core abstraction is [`Session`] which wraps a DataFusion `SessionContext` and provides:
//! - Async query execution via channels (suitable for egui's immediate mode)
//! - Query plan extraction for visualization
//! - Catalog management (tables, schemas)
//! - Execution statistics and profiling
//!
//! # Example
//!
//! ```ignore
//! use enya_datafusion::{Session, QueryRequest};
//!
//! // Create a session
//! let session = Session::new();
//!
//! // Register a Parquet file as a table
//! session.register_parquet("events", "s3://bucket/events.parquet").await?;
//!
//! // Execute a query (blocking, collects all results)
//! let batches = session.execute_collect("SELECT * FROM events LIMIT 10").await?;
//!
//! // Or use the async executor for streaming results
//! let runtime_handle = tokio::runtime::Handle::current();
//! let mut event_rx = session.init_executor(runtime_handle);
//! let request = QueryRequest::new("SELECT * FROM events");
//! session.execute(request)?;
//! // Poll event_rx for QueryEvent::Batch, QueryEvent::Completed, etc.
//! ```

pub mod catalog;
pub mod error;
pub mod executor;
pub mod flight;
pub mod plan;
pub mod session;
pub mod types;

pub use error::Error;
pub use flight::{ConnectionState, FlightClient, FlightConfig, QueryStream};
pub use plan::{
    parse_metric_bytes, parse_metric_duration, parse_metric_usize, parse_metrics, parse_plan_text,
};
pub use session::{Config, Session};
pub use types::*;

// Re-export arrow types needed by consumers
pub use arrow;

/// Result type for DataFusion operations.
pub type Result<T> = std::result::Result<T, Error>;
