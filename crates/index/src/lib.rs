//! Metrics instrumentation indexer for source code.
//!
//! This crate provides functionality to scan source code repositories
//! and build an index of metric instrumentation points.
//!
//! # Architecture
//!
//! - [`scanner`]: Language-agnostic scanner framework with trait-based extensibility
//! - [`parser`]: Tree-sitter parsing utilities for Rust
//! - [`repo`]: Git operations (clone, fetch, update)
//! - [`index`]: In-memory index of discovered instrumentation

#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]

pub mod index;
pub mod parser;
pub mod repo;
pub mod scanner;

pub use index::{CodebaseIndex, IndexProgress, build_index_with_progress};
pub use parser::ParseError;
pub use repo::{CommitInfo, fetch_commit_history};
pub use scanner::{MetricInstrumentation, MetricKind, Scanner, ScannerRegistry};

/// Get the current Unix timestamp in seconds.
#[inline]
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn now_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
