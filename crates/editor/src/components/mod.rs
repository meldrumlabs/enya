use std::any::Any;

use crate::ui::theme::AppTheme;
use enya_config::{PaneConfig, SnapshotPaneData};

pub mod overlay;
pub mod pane;
pub mod project_sidebar;
pub mod settings_page;
pub mod util;
pub mod widget;

// Re-export from pane
pub use pane::{
    AgentAiProvider, Bar, BarChartViz, CommitMarker, DataPoint, GaugeChart, HeatmapCell,
    HeatmapLabels, HeatmapViz, InlineChart, InlineContent, InlineSearchResults, InlineSource,
    InlineTable, InlineTableColumn, LogsBackend, LogsPane, LogsPaneAction, PluginChartPane,
    PluginGaugePane, PluginStatPane, PluginTablePane, PrReviewPane, PrReviewPaneAction, QueryPane,
    QueryPaneAction, SearchResultItem, Series, SparklineViz, SqlPane, SqlPaneAction, StatChart,
    Threshold, TimeSeriesChart, TracingPane, TracingPaneAction, Visualization, VisualizationType,
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
    SourcePreviewResult, StyleTab, TimeRangePicker, TimeRangePickerResult, TutorialOverlay,
    ViewportFilter, ViewportFilterResult, WhichKey, WorkspaceCreator, WorkspaceCreatorResult,
    parse_commands, strip_command_blocks,
};

// Re-export from project_sidebar
pub use project_sidebar::{ProjectSidebar, ProjectSidebarResult};

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

    /// Whether this component handles its own hjkl / arrow-key navigation.
    /// When `true`, the workspace keyboard handler will NOT consume h/j/k/l
    /// for tile-to-tile focus changes, letting the component handle them instead.
    fn handles_own_navigation(&self) -> bool {
        false
    }

    /// Set whether a workspace overlay is blocking keyboard input.
    /// Default implementation does nothing - components can override if needed.
    fn set_overlay_blocks_input(&mut self, _blocks: bool) {}

    /// Extract snapshot data from this component for sharing.
    /// Returns `None` if this component doesn't support snapshots or has no data.
    fn extract_snapshot_data(&self) -> Option<SnapshotPaneData> {
        None
    }

    /// Load snapshot data into this component for read-only display.
    fn load_snapshot_data(&mut self, _data: &SnapshotPaneData) {}

    /// Returns the `PaneConfig` representation for workspace serialization.
    /// Returns `None` for components that aren't serializable panes.
    fn to_pane_config(&self) -> Option<PaneConfig> {
        None
    }

    /// Get a reference to self as Any (for downcasting)
    fn as_any(&self) -> &dyn Any;
    /// Get a mutable reference to self as Any (for downcasting)
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
