//! SQL pane module for REPL-style SQL query execution.
//!
//! This module provides a SQL interface for connecting to Flight SQL servers
//! and executing queries. When the `sql` feature is disabled or on WASM,
//! it shows a "SQL Feature Not Available" message.
//!
//! # Module Structure
//!
//! - [`native`] - Full implementation (native + sql feature only)
//!   - [`command`](native::command) - SQL pane commands (triggered with `/`)
//!   - [`connections`](native::connections) - Connection management types
//!   - [`suggestions`](native::suggestions) - Autocomplete suggestion types
//!   - [`types`](native::types) - Core types (modes, overlays, query cells)
//!   - Query plan visualization
//! - [`highlighting`] - SQL syntax highlighting (always available)
//! - [`stub`] - Stub implementation (WASM or sql feature disabled)

// Syntax highlighting (shared between all builds)
pub mod highlighting;

/// Actions that can be triggered by the SQL pane.
#[derive(Debug, Clone)]
pub enum SqlPaneAction {
    /// No action.
    None,
    /// Share a query result to the agent panel as an inline table.
    ShareResultToAgent(crate::components::pane::inline_content::InlineTable),
}

// Native SQL implementation - requires non-WASM + sql feature
#[cfg(all(not(target_arch = "wasm32"), feature = "sql"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "sql"))]
pub use native::*;

// Stub for WASM or when sql feature is disabled
#[cfg(any(target_arch = "wasm32", not(feature = "sql")))]
mod stub;
#[cfg(any(target_arch = "wasm32", not(feature = "sql")))]
pub use stub::SqlPane;
