//! SQL pane module for REPL-style SQL query execution.
//!
//! This module provides a SQL interface for connecting to Flight SQL servers
//! and executing queries. On WASM, it shows a "Native App Required" message.

// Syntax highlighting (shared)
pub mod highlighting;

// Query plan visualization (native-only)
#[cfg(not(target_arch = "wasm32"))]
mod plan_view;
#[cfg(not(target_arch = "wasm32"))]
pub use plan_view::{DiffView, PlanTreeView, PlanViewMode, PlanViewer, TimelineView};

// Native implementation
#[cfg(not(target_arch = "wasm32"))]
mod pane;
#[cfg(not(target_arch = "wasm32"))]
pub use pane::{SqlPane, SqlPaneAction};

// WASM stub
#[cfg(target_arch = "wasm32")]
mod stub;
#[cfg(target_arch = "wasm32")]
pub use stub::{SqlPane, SqlPaneAction};
