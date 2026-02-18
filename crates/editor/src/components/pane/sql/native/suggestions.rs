//! Autocomplete suggestion types for the SQL pane.

use crate::ui::semantic_icons::{action, category, completion, file, time};

/// SQL keywords that introduce a table reference (FROM, JOIN, etc.).
pub const TABLE_KEYWORDS: &[&str] = &["FROM", "JOIN", "UPDATE", "INTO", "TABLE"];

/// SQL keywords that introduce a column reference (SELECT, WHERE, etc.).
pub const COLUMN_KEYWORDS: &[&str] = &["SELECT", "WHERE", "HAVING", "ON", "SET", "BY"];

/// A suggestion item for the completion popup.
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// Display text.
    pub label: String,
    /// Secondary text (description, row count, etc.).
    pub detail: String,
    /// Text to insert when selected.
    pub insert: String,
    /// Icon/indicator.
    pub icon: SuggestionIcon,
    /// Fuzzy match score (higher is better).
    pub score: i64,
    /// Positions in label that matched the query (used for highlight rendering).
    pub match_positions: Vec<usize>,
}

/// Icon type for suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionIcon {
    Command,
    Table,
    Column,
    Connection,
    #[allow(dead_code)] // For future use
    History,
    Keyword,
    Function,
}

impl SuggestionIcon {
    /// Get the Phosphor icon string for this suggestion type.
    pub fn icon_str(self) -> &'static str {
        match self {
            Self::Command => action::TERMINAL,
            Self::Table => file::DATA,
            Self::Column => file::DATA,
            Self::Connection => category::DATAFUSION,
            Self::History => time::HISTORY,
            Self::Keyword => completion::KEYWORD,
            Self::Function => completion::FUNCTION,
        }
    }
}

/// State for the suggestion popup.
#[derive(Debug, Clone, Default)]
pub struct SuggestionState {
    /// Current suggestions to show.
    pub items: Vec<Suggestion>,
    /// Selected index.
    pub selected: usize,
    /// Whether the popup is visible.
    pub visible: bool,
}

impl SuggestionState {
    /// Set new suggestion items, updating visibility and resetting selection.
    pub fn set(&mut self, items: Vec<Suggestion>) {
        self.visible = !items.is_empty();
        self.items = items;
        self.selected = 0;
    }

    /// Clear all suggestions and hide the popup.
    pub fn clear(&mut self) {
        self.items.clear();
        self.visible = false;
    }
}
