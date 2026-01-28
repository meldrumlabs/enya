//! Inline content types for embedding visualizations in chat messages.
//!
//! These types are used by the AgentPanel overlay to render charts, source code
//! previews, and search results directly within conversation messages.

/// Inline content block that can be embedded in chat messages.
///
/// These are rendered inline within the message, allowing the agent to
/// show visualizations and source code directly in the conversation.
#[derive(Debug, Clone)]
pub enum InlineContent {
    /// An inline time series chart with data
    Chart(InlineChart),
    /// An inline source code preview
    Source(InlineSource),
    /// Inline search results
    SearchResults(InlineSearchResults),
    /// An inline git diff view
    Diff(InlineDiff),
}

/// Inline time series chart data.
///
/// Contains the data needed to render a compact chart within a message.
#[derive(Debug, Clone)]
pub struct InlineChart {
    /// Chart title (e.g., metric name)
    pub title: String,
    /// Data series to plot
    pub series: Vec<super::time_series_chart::Series>,
    /// Optional height override (default: 120px)
    pub height: Option<f32>,
}

/// Inline source code preview.
///
/// Contains the data needed to render a syntax-highlighted code snippet.
#[derive(Debug, Clone)]
pub struct InlineSource {
    /// File path (relative)
    pub file_path: String,
    /// Target line number (1-indexed)
    pub line: usize,
    /// Source lines to display
    pub lines: Vec<String>,
    /// Start line number (1-indexed)
    pub start_line: usize,
    /// Language for syntax highlighting (e.g., "rust", "go")
    pub language: String,
    /// Pre-computed tree-sitter syntax highlighting data
    pub highlight_data: crate::components::util::SyntaxHighlightData,
}

/// Inline search results.
///
/// Contains search results from the Tantivy codebase index.
#[derive(Debug, Clone)]
pub struct InlineSearchResults {
    /// Search query
    pub query: String,
    /// Filter applied (all, metrics, alerts, commits)
    pub filter: String,
    /// Search results
    pub results: Vec<SearchResultItem>,
}

/// A single search result item for display.
#[derive(Debug, Clone)]
pub struct SearchResultItem {
    /// Result type (metric, alert, commit)
    pub kind: String,
    /// Name (metric name, alert name, or commit message)
    pub name: String,
    /// File path (relative)
    pub file_path: String,
    /// Line number
    pub line: usize,
    /// Relevance score
    pub score: f32,
    /// Optional snippet or context
    pub snippet: Option<String>,
}

/// Inline git diff view.
///
/// Contains the data needed to render a compact diff within a message.
#[derive(Debug, Clone)]
pub struct InlineDiff {
    /// Commit hash (short form)
    pub commit_hash: String,
    /// Commit message
    pub commit_message: String,
    /// File diffs
    pub file_diffs: Vec<InlineDiffFile>,
    /// Total additions
    pub additions: usize,
    /// Total deletions
    pub deletions: usize,
}

/// A single file's diff for inline display.
#[derive(Debug, Clone)]
pub struct InlineDiffFile {
    /// File path
    pub path: String,
    /// Diff lines
    pub lines: Vec<InlineDiffLine>,
    /// Number of additions
    pub additions: usize,
    /// Number of deletions
    pub deletions: usize,
}

/// A single line in an inline diff.
#[derive(Debug, Clone)]
pub struct InlineDiffLine {
    /// Line content
    pub content: String,
    /// Line type: "context", "addition", or "deletion"
    pub kind: InlineDiffLineKind,
    /// Old line number
    pub old_line: Option<usize>,
    /// New line number
    pub new_line: Option<usize>,
}

/// The type of diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineDiffLineKind {
    /// Context line (unchanged)
    Context,
    /// Added line
    Addition,
    /// Removed line
    Deletion,
    /// Hunk header (@@)
    Hunk,
}
