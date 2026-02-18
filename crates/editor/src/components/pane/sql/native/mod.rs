//! Native SQL implementation - requires non-WASM build with `sql` feature.
//!
//! This module contains the full SQL pane implementation including:
//! - Flight SQL connection management
//! - Query execution and result display
//! - Query plan visualization
//! - Autocomplete suggestions

// Public modules (accessible as sql::command, sql::connections, etc.)
pub mod command;
pub mod connections;
pub mod suggestions;
pub mod types;

// Private implementation modules
mod diff;
mod diff_rendering;
mod pane;
mod plan_parsing;
mod plan_view;
mod query_card;

// Re-export commonly used types at the sql:: level
pub use command::SqlCommand;
pub use connections::{
    ConnectionAction, ConnectionId, ConnectionSnapshot, ConnectionTreeState, SavedConnection,
    TreeSelection,
};
pub use pane::SqlPane;
pub use plan_view::{DiffView, PlanTreeView, PlanViewMode, PlanViewer, StatsView};
pub use suggestions::{Suggestion, SuggestionIcon, SuggestionState};
pub use types::{ResultOverlay, SqlMode};
