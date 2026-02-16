use std::any::Any;

use crate::ui::theme::AppTheme;

pub mod overlay;
pub mod pane;
pub mod settings_page;
pub mod util;
pub mod widget;

// Re-export from pane
pub use pane::{
    AgentAiProvider, Bar, BarChartViz, CommitMarker, DataPoint, GaugeChart, HeatmapCell,
    HeatmapLabels, HeatmapViz, InlineChart, InlineContent, InlineSearchResults, InlineSource,
    LogsBackend, LogsPane, LogsPaneAction, PluginChartPane, PluginGaugePane, PluginStatPane,
    PluginTablePane, QueryPane, QueryPaneAction, SearchResultItem, Series, SparklineViz, SqlPane,
    SqlPaneAction, StatChart, Threshold, TimeSeriesChart, TracingPane, TracingPaneAction,
    Visualization, VisualizationType,
};
#[cfg(all(not(target_arch = "wasm32"), feature = "terminal"))]
pub use pane::{TerminalPane, TerminalPaneAction};

// Re-export from overlay
#[cfg(target_arch = "wasm32")]
pub use overlay::NativePromoOverlay;
pub use overlay::{
    AboutOverlay, AgentCommand, AgentPanel, AgentPanelResult, AiProvider, BufferEditor,
    BufferEditorResult, ChatMessage, CodebaseContext, CommandPalette, CommandResult,
    ConnectionContext, Diagnostic, DiagnosticLevel, DiagnosticSource, DiagnosticsFilter,
    DiagnosticsPane, DiagnosticsPaneAction, DynamicCommand, EditExcerpt, EditorContext,
    InfoOverlay, LeaderKey, LeaderPopup, MessageRole, MultiEditOverlay, MultiEditResult,
    PluginDisplayInfo, PluginSource, PluginsOverlay, PluginsOverlayResult, SourcePreviewOverlay,
    SourcePreviewResult, StylePicker, StylePickerResult, StyleTab, TimeRangePicker,
    TimeRangePickerResult, TutorialAction, TutorialOverlay, ViewportFilter, ViewportFilterResult,
    WhichKey, WorkspaceCreator, WorkspaceCreatorResult, WorkspaceFinder, WorkspaceFinderResult,
    WorkspaceItem, parse_commands, strip_command_blocks,
};

// Re-export from settings_page
pub use settings_page::{SettingsPage, SettingsPageResult};

// Re-export from widget
pub use widget::{
    AgentInputBar, AgentInputBarResult, AgentInputState, Buffer, BufferAction, BufferMode,
    ContextPane, InlineAgentInput, LandingPage, LandingPageAction, Notification, NotificationLevel,
    NotificationManager, QuickCommand, Sparkline, StatusLine, StatusLineResult, StatusMode,
    TimeRange, TimeRangePreset, TimeRangeToolbar,
};
#[cfg(not(target_arch = "wasm32"))]
pub use widget::{UpdateBanner, UpdateBannerAction};

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
    /// Returns a RichText label for the given component
    fn label(&self) -> egui::RichText;

    /// Returns an optional description for the component (shown on hover)
    fn description(&self) -> &str {
        ""
    }

    /// Set whether a workspace overlay is blocking keyboard input.
    /// Default implementation does nothing - components can override if needed.
    fn set_overlay_blocks_input(&mut self, _blocks: bool) {}

    /// Get a reference to self as Any (for downcasting)
    fn as_any(&self) -> &dyn Any;
    /// Get a mutable reference to self as Any (for downcasting)
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
