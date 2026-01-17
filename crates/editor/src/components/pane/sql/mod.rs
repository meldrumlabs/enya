//! SQL pane module for REPL-style SQL query execution.
//!
//! This module provides a SQL interface for connecting to Flight SQL servers
//! and executing queries. On WASM, it shows a "Native App Required" message.
//!
//! # Module Structure
//!
//! - [`command`] - SQL pane commands (triggered with `/`)
//! - [`connections`] - Connection management types
//! - [`suggestions`] - Autocomplete suggestion types
//! - [`types`] - Core types (modes, overlays, query cells)
//! - [`highlighting`] - SQL syntax highlighting
//! - [`plan_view`] - Query plan visualization (native-only)

// Syntax highlighting (shared between native and WASM)
pub mod highlighting;

// Native-only modules (depend on enya_datafusion)
#[cfg(not(target_arch = "wasm32"))]
pub mod command;
#[cfg(not(target_arch = "wasm32"))]
pub mod connections;
#[cfg(not(target_arch = "wasm32"))]
pub mod suggestions;
#[cfg(not(target_arch = "wasm32"))]
pub mod types;

// Re-export commonly used types (native-only)
#[cfg(not(target_arch = "wasm32"))]
pub use command::SqlCommand;
#[cfg(not(target_arch = "wasm32"))]
pub use connections::{ConnectionId, ConnectionTreeState, SavedConnection, TreeSelection};
#[cfg(not(target_arch = "wasm32"))]
pub use suggestions::{Suggestion, SuggestionIcon, SuggestionState};
#[cfg(not(target_arch = "wasm32"))]
pub use types::{ResultOverlay, SqlMode, SqlPaneAction};

// Query plan visualization (native-only)
#[cfg(not(target_arch = "wasm32"))]
mod plan_view;
#[cfg(not(target_arch = "wasm32"))]
pub use plan_view::{DiffView, PlanTreeView, PlanViewMode, PlanViewer, StatsView};

// Native implementation
#[cfg(not(target_arch = "wasm32"))]
mod pane;
#[cfg(not(target_arch = "wasm32"))]
pub use pane::SqlPane;

// WASM stub
#[cfg(target_arch = "wasm32")]
mod stub;
#[cfg(target_arch = "wasm32")]
pub use stub::{SqlPane, SqlPaneAction};
