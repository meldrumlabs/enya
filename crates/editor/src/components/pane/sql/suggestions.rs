//! Autocomplete suggestion types for the SQL pane.

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
    /// Positions in label that matched the query.
    #[allow(dead_code)] // For future highlighting of matched chars
    pub match_positions: Vec<usize>,
}

/// Icon type for suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // History variant for future use
pub enum SuggestionIcon {
    Command,
    Table,
    Column,
    Connection,
    History,
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
