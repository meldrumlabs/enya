use std::any::Any;

use crate::ui::theme::AppTheme;

pub mod overlay;
pub mod pane;
pub mod util;
pub mod widget;

// Re-export from pane
pub use pane::{
    AgentAiProvider, Bar, BarChartViz, CommitMarker, DataPoint, GaugeChart, HeatmapCell,
    HeatmapLabels, HeatmapViz, InlineChart, InlineContent, InlineSearchResults, InlineSource,
    LogsBackend, LogsPane, LogsPaneAction, QueryPane, QueryPaneAction, SearchResultItem, Series,
    SparklineViz, StatChart, Threshold, TimeSeriesChart, TracingPane, TracingPaneAction,
    Visualization, VisualizationType,
};
#[cfg(not(target_arch = "wasm32"))]
pub use pane::{TerminalPane, TerminalPaneAction};

// Re-export from overlay
#[cfg(target_arch = "wasm32")]
pub use overlay::NativePromoOverlay;
pub use overlay::{
    AgentCommand, AgentPanel, AgentPanelResult, AiProvider, BufferEditor, BufferEditorResult,
    ChatMessage, CodebaseContext, CommandPalette, CommandResult, ConnectionContext,
    DashboardContext, Diagnostic, DiagnosticLevel, DiagnosticSource, DiagnosticsFilter,
    DiagnosticsPane, DiagnosticsPaneAction, EditExcerpt, EditorContext, InfoOverlay, MessageRole,
    MultiEditOverlay, MultiEditResult, SourcePreviewOverlay, SourcePreviewResult, StylePicker,
    StylePickerResult, StyleTab, TutorialOverlay, ViewportFilter, ViewportFilterResult, WhichKey,
    WorkspaceCreator, WorkspaceCreatorResult, WorkspaceFinder, WorkspaceItem, parse_commands,
    strip_command_blocks,
};

// Re-export from widget
pub use widget::{
    AgentInputBar, AgentInputBarResult, AgentInputState, Buffer, BufferAction, BufferMode,
    ContextPane, LandingPage, LandingPageAction, MemberPresence, Notification, NotificationLevel,
    NotificationManager, QuickCommand, Sparkline, StatusLine, StatusMode, TeamMember, TeamMenu,
    TeamMenuAction, TeamStatusInfo, TimeRange, TimeRangePreset, TimeRangeToolbar,
};

// Re-export from util
pub use util::{
    Backend, CompletionItem, CompletionKind, CompletionResult, ExecuteParams, Finder, FinderColors,
    FinderConfig, FinderItem, FinderKeyboardInput, FinderResult, Granularity, MultiBufferMode,
    MultiBufferState, OverlayColors, OverlayStyle, OverlayStyleVariant, QueryCompletion,
    QueryExecutor, QueryLanguage, QueryPollResult, QueryState, Selection, ValidationResult,
    draw_backdrop, draw_separator, draw_separator_colored, is_valid_query, next_id, next_id_usize,
    render_key_badge, render_key_badge_large, validate_query,
};

/// Trait that defines an Enya Component
pub trait Component: Any {
    /// The core function that is responsible for drawing the component
    fn show(&mut self, ui: &mut egui::Ui);
    /// Returns the identifier for the component
    fn id(&self) -> usize;
    /// Returns the name for the component (e.g., SQL)
    fn name(&self) -> String;
    /// Saves the current theme for the component
    fn set_theme(&mut self, theme: AppTheme);
    fn set_api_key(&mut self, key: &str);
    fn set_staging_api_key(&mut self, key: &str);
    /// Returns a RichText label for the given component
    fn label(&self) -> egui::RichText;

    /// Returns an optional description for the component (shown on hover)
    fn description(&self) -> &str {
        ""
    }

    /// Get a reference to self as Any (for downcasting)
    fn as_any(&self) -> &dyn Any;
    /// Get a mutable reference to self as Any (for downcasting)
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
