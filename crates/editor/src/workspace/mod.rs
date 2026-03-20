use rustc_hash::{FxHashMap, FxHashSet};

use egui_tiles::{Tile, TileId, Tiles};

use crate::AsyncRuntime;
use crate::app::AppState;
#[cfg(not(target_arch = "wasm32"))]
use crate::codebase::CodebaseManager;
#[cfg(target_arch = "wasm32")]
use crate::components::NativePromoOverlay;
use crate::components::overlay::{AnnotationEditor, AnnotationEditorResult};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::overlay::{CodebaseFinder, CodebaseFinderStatus};
use crate::components::overlay::{DiffViewerOverlay, DiffViewerResult};
use crate::components::overlay::{FinderMode, UnifiedFinder};
use crate::components::{
    AboutOverlay, AgentCommand, AgentInputBar, AgentInputBarResult, AgentPanel, AgentPanelResult,
    Buffer, BufferEditor, BufferEditorResult, CommandPalette, CommandResult, Component,
    ContextPane, DiagnosticsPane, InfoOverlay, LandingPage, LandingPageAction, LeaderKey,
    LeaderPopup, LogsPane, MultiBufferMode, MultiBufferState, MultiEditOverlay, MultiEditResult,
    PluginChartPane, PluginGaugePane, PluginStatPane, PluginTablePane, PluginsOverlay,
    PluginsOverlayResult, QueryExecutor, QueryLanguage, QueryPane, QueryState, QuickCommand,
    SourcePreviewOverlay, SourcePreviewResult, SqlPane, TimeRangePicker, TimeRangePickerResult,
    TimeRangeToolbar, TracingPane, TutorialOverlay, ViewportFilter, ViewportFilterResult, WhichKey,
    WorkspaceCreator, WorkspaceCreatorResult,
};
use crate::ui::settings_screen::EditorFont;
use crate::ui::theme::AppTheme;
use enya_plugin::{
    CustomChartConfig, CustomChartData, CustomTableConfig, CustomTableData, FocusedPaneInfo,
    GaugePaneConfig, GaugePaneData, StatPaneConfig, StatPaneData,
};

// Workspace configuration module (serialization)
pub mod config;

// Input handling (navigation, visual-multi mode)
mod input;
pub use input::{
    LEADER_KEY_TIMEOUT_MS, LEADER_POPUP_DELAY_MS, LeaderKeyState, NavDirection, VisualMultiState,
};

// Tile tree behavior (egui_tiles integration)
mod tiles;
use tiles::TreeBehavior;

// Keyboard input handling
mod keyboard;

// Pure keyboard decision logic (testable without egui::Context)
mod keyboard_logic;
pub use keyboard_logic::{
    KeyboardContext, KeyboardDecision, check_navigation_blocked, determine_agent_operator_action,
    determine_ctrl_w_action, determine_ctrl_w_t_action, determine_goto_action,
    determine_space_action, determine_time_range_action,
};

// Overlay management (diagnostics, etc.)
mod overlays;

// Query execution coordination
mod query;

// Workspace serialization/deserialization
mod serialization;

// Finder modals (metrics finder, workspace finder)
mod finders;

// Pane management (adding, closing, splitting)
mod panes;

// UI rendering (filtered view, scrollbar, scroll-to-focus)
mod rendering;

// Floating panes (detachable windows above the tile layout)
mod floating;
pub use floating::{FloatingPaneAction, FloatingPaneId, FloatingPaneManager};

// Undo system for workspace operations
mod undo;
pub use undo::{ClosedPaneInfo, DockedPaneInfo, FloatedPaneInfo, UndoAction, UndoStack};

// Layout animation (smooth transitions when splitting/closing panes)
mod layout_animation;
use layout_animation::LayoutAnimator;

// Re-export config types for convenience
pub use config::{
    ConnectionConfig, GOLDEN_SIGNALS_TOML, GitConfig, INFRASTRUCTURE_TOML, LayoutConfig,
    LayoutContainer, LayoutNode, LayoutType, LogsConfig, MULTI_SERVICE_TOML, MetricsConfig,
    PaneConfig, PaneConfigExt, PluginsConfig, RefreshInterval, TimeConfig, TimeConfigExt,
    TracingConfig, ViewConfig, ViewConfigExt, WORKSPACE_VERSION, WorkspaceConfig, WorkspaceError,
    WorkspaceMeta, pane_from_query_state, pane_from_query_state_with_viz, time_config_from_preset,
    time_config_from_preset_with_refresh,
};

/// Actions that the Workspace needs the App to handle
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceAction {
    /// No action needed
    None,
    /// Set a specific builtin theme
    SetTheme(AppTheme),
    /// Set a custom theme by name (from plugins)
    SetCustomTheme(String),
    /// Cycle to the next theme
    NextTheme,
    /// Set the editor font
    SetFont(EditorFont),
    /// Set both theme and font (used when cancelling style picker to restore originals)
    SetThemeAndFont(AppTheme, EditorFont),
    /// Set custom theme and font (used when cancelling style picker to restore custom theme)
    SetCustomThemeAndFont(String, EditorFont),
    /// Show a notification
    Notify { level: String, message: String },
    /// Track a recently opened plot
    TrackRecentPlot {
        name: String,
        metric_name: String,
        is_query: bool,
    },
    /// Take a screenshot of the window (optionally with a custom path)
    TakeScreenshot(Option<String>),
    /// Save workspace with optional name, project assignment, and Flight SQL endpoint
    SaveWorkspace {
        name: Option<String>,
        project: Option<String>,
        flight_sql_endpoint: Option<String>,
    },
    /// Load workspace by name (with optional project)
    LoadWorkspace {
        name: String,
        project: Option<String>,
    },
    /// Start creating a new project in the sidebar
    CreateProject,
    /// List available workspaces
    ListWorkspaces,
    /// Share workspace as URL (snapshot if data loaded, config-only otherwise)
    ShareWorkspace,
    /// Share a single pane as URL (snapshot if data loaded, config-only otherwise)
    SharePane(usize),
    /// Share selected panes as URL (snapshot if data loaded, config-only otherwise)
    ShareSelectedPanes(Vec<usize>),
    /// Upload snapshot to blob server (workspace + data + conversation + optional title)
    UploadSnapshot(Option<String>),
    /// Focus the project sidebar (vim h at left edge)
    FocusProjectSidebar,
    /// Toggle project sidebar visibility (Space+b)
    ToggleProjectSidebar,
    /// Quit the application
    QuitApp,
    /// Open the annotation editor for the focused pane
    OpenAnnotationEditor,
    /// Open diff viewer from a commit reference in chat
    OpenDiffViewer {
        hash: String,
        message: String,
        diff: String,
    },
    /// Execute a plugin command (command name, args)
    PluginCommand { command: String, args: String },
    /// Save settings from the settings overlay
    SaveSettings {
        ai_provider: crate::components::util::AiProvider,
        ai_model: Option<String>,
        git_repo_url: String,
        default_prometheus_endpoint: String,
        default_loki_endpoint: String,
        default_tempo_endpoint: String,
        flight_sql_connections: Vec<crate::ui::settings_screen::FlightSqlConnection>,
    },
    /// Open the full-page settings
    OpenSettings,
}

/// The main viewport layout with a flexible tile tree for views/charts.
///
/// The Workspace manages the tile-based pane layout (using egui_tiles) and handles:
/// - Pane management (adding, removing, splitting)
/// - Modal overlays (command palette, metrics finder, buffer editor)
/// - Keyboard navigation (vim-style h/j/k/l)
/// - Visual-multi mode for batch pane operations
/// - Zen mode and fullscreen pane display
/// - Query execution coordination
pub struct Workspace {
    /// The tile tree for the viewport area
    viewport_tree: egui_tiles::Tree<Box<dyn Component>>,
    behavior: TreeBehavior,
    /// Track which metrics already have charts open (by metric name)
    open_charts: FxHashSet<String>,
    /// Pending chart to add (metric name)
    pending_chart: Option<String>,
    /// Time range toolbar
    time_range_toolbar: TimeRangeToolbar,
    /// Command palette (neovim-style `:` commands)
    command_palette: CommandPalette,
    /// Buffer editor modal (for editing queries)
    buffer_editor: BufferEditor,
    /// Track which tile is being edited (to apply changes back)
    editing_tile_id: Option<TileId>,
    /// Zen mode - hide all panels for distraction-free viewing
    zen_mode: bool,
    /// Fullscreen mode - show only one pane maximized
    fullscreen_tile: Option<TileId>,
    /// Landing page component (shown when no charts are open)
    landing_page: LandingPage,
    /// Whether to show the landing page
    show_landing: bool,
    /// Name of the currently loaded workspace (filename stem, e.g. "my-workspace")
    pub(crate) loaded_name: Option<String>,
    /// Project the currently loaded workspace belongs to
    pub(crate) loaded_project: Option<String>,
    /// State for leader key sequences (t, Space, y, c)
    leader_keys: LeaderKeyState,
    /// Info overlay (shows build/version info)
    info_overlay: InfoOverlay,
    /// About overlay (shows project information)
    about_overlay: AboutOverlay,
    /// Which-key overlay (shows available keybindings)
    which_key: WhichKey,
    /// Leader popup (dynamic Space+X command hints, like which-key.nvim)
    leader_popup: LeaderPopup,
    /// Time range picker overlay (custom time range selection)
    time_range_picker: TimeRangePicker,
    /// Tutorial overlay (interactive walkthrough)
    tutorial_overlay: TutorialOverlay,
    /// Plugins overlay (view and manage plugins)
    plugins_overlay: PluginsOverlay,
    /// Workspace creator overlay (new workspace wizard)
    workspace_creator: WorkspaceCreator,
    /// Current scroll offset for smooth scrolling (0.0 to 1.0, percentage)
    viewport_scroll_offset: f32,
    /// Target scroll offset for smooth animation
    viewport_scroll_target: f32,
    /// Total content height of the viewport (for scrollbar calculations)
    viewport_content_height: f32,
    /// Visible height of the viewport
    viewport_visible_height: f32,
    /// Visual multi-select mode state (None = not in visual-multi mode)
    visual_multi_state: Option<VisualMultiState>,
    /// Multi-buffer state for synchronized editing across panes
    multi_buffer_state: MultiBufferState,
    /// Multi-edit overlay for editing multiple panes simultaneously
    multi_edit_overlay: MultiEditOverlay,
    /// Diagnostics pane for showing query validation errors (as overlay)
    diagnostics_pane: DiagnosticsPane,
    /// Whether the diagnostics overlay is visible
    diagnostics_visible: bool,
    /// Flag to open settings page (set by command, handled in show)
    pending_open_settings: bool,
    /// Cached Flight SQL connection definitions from settings (for syncing to new SQL panes)
    cached_flight_sql_connections: Vec<crate::ui::settings_screen::FlightSqlConnection>,
    /// Flag to open time range picker (set by keyboard tc or button click)
    pending_open_time_range_picker: bool,
    /// Query executor for running queries against backends (Prometheus, Enya)
    query_executor: QueryExecutor,
    /// Counter for sequential query pane naming (Query 1, Query 2, ...)
    next_query_number: usize,
    /// Pending inline chart queries awaiting results from QueryExecutor
    pending_inline_charts: Vec<panes::PendingInlineChart>,
    /// Counter for inline chart query IDs
    next_inline_chart_id: usize,
    /// Workspace filter for filtering visible panes by query content
    viewport_filter: ViewportFilter,
    /// Source code preview overlay for "go to definition"
    source_preview: SourcePreviewOverlay,
    /// Agent panel for Claude Code integration
    agent_panel: AgentPanel,
    /// Agent input bar for lightweight agent mode interactions
    agent_input_bar: AgentInputBar,
    /// Whether agent mode is active (vim-style modal interaction)
    agent_mode_active: bool,
    /// Whether the agent panel has keyboard focus (vim h/l to transfer)
    agent_panel_focused: bool,
    /// Panes in agent context (from visual mode selection or manual +/-)
    agent_context_panes: FxHashSet<TileId>,
    /// Codebase manager for git repo and metrics discovery (native only with codebase feature)
    #[cfg(not(target_arch = "wasm32"))]
    codebase_manager: CodebaseManager,
    /// Codebase finder overlay (Space+c) for searching metrics, alerts, commits
    #[cfg(not(target_arch = "wasm32"))]
    codebase_finder: CodebaseFinder,
    /// Diff viewer overlay for viewing commit diffs
    diff_viewer: DiffViewerOverlay,
    /// Pending git config URL to initialize (set during load, executed in show())
    #[cfg(not(target_arch = "wasm32"))]
    pending_git_config: Option<String>,
    /// Pending connection endpoint to apply (set during load, executed in show())
    pending_connection_endpoint: Option<String>,
    /// Auto-refresh interval (None = disabled)
    refresh_interval: Option<RefreshInterval>,
    /// Pending plugin install action (name, file)
    pending_install_plugin: Option<(String, String)>,
    /// Pending plugin remove action (name)
    pending_remove_plugin: Option<String>,
    /// Pending plugin refresh action
    pending_refresh_plugins: bool,
    /// Last time queries were auto-refreshed
    last_refresh: Option<crate::util::Instant>,
    /// Pending workspace load (set by agent command, consumed in show())
    pending_load_workspace: Option<String>,
    /// Cached GitHub access token (set each frame from app_state)
    github_token: Option<String>,
    /// Token from `git credential fill` for PR review (native only).
    /// Preferred over the OAuth token since it has org repo access.
    #[cfg(not(target_arch = "wasm32"))]
    git_credential_token: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    pending_git_credential: std::sync::Arc<parking_lot::Mutex<Option<Result<String, String>>>>,
    #[cfg(not(target_arch = "wasm32"))]
    git_credential_fetched: bool,
    /// Native app promo overlay (WASM only)
    #[cfg(target_arch = "wasm32")]
    native_promo_overlay: NativePromoOverlay,
    /// Whether the user is on a mobile browser (WASM only)
    #[cfg(target_arch = "wasm32")]
    is_mobile: bool,
    /// Unified finder (Telescope-style fuzzy finder)
    unified_finder: UnifiedFinder,
    /// Annotation editor overlay
    annotation_editor: AnnotationEditor,

    // ==================== Floating Panes ====================
    /// Floating panes that hover above the tile layout
    floating_panes: FloatingPaneManager,

    // ==================== Undo System ====================
    /// Stack of undoable actions (vim-style 'u' to undo)
    undo_stack: UndoStack,

    // ==================== Layout Animation ====================
    /// Animator for smooth layout transitions
    layout_animator: LayoutAnimator,
    /// Whether this workspace was loaded from an immutable snapshot
    is_snapshot: bool,
    /// Title of the snapshot (if loaded from a named blob snapshot)
    snapshot_title: Option<String>,

    // ==================== Active Theme Colors ====================
    /// Resolved theme colors (from custom or builtin theme)
    /// Used for components that need custom theme support
    active_colors: Option<crate::ui::ActiveThemeColors>,
    /// Cached effective theme for the current render frame.
    /// This is computed at the start of `show()` and should be used for all
    /// theme-related rendering via the `theme()` getter.
    ///
    /// IMPORTANT: Always use `self.theme()` instead of `app_state.theme` when
    /// rendering to ensure custom plugin themes are properly applied.
    render_theme: AppTheme,

    // ==================== Plugin Custom Panes ====================
    /// Registry of custom table pane configurations (by pane type name)
    custom_table_configs: FxHashMap<String, CustomTableConfig>,
    /// Data for custom table panes (by pane type name)
    custom_table_data: FxHashMap<String, CustomTableData>,
    /// Registry of custom chart pane configurations (by pane type name)
    custom_chart_configs: FxHashMap<String, CustomChartConfig>,
    /// Data for custom chart panes (by pane type name)
    custom_chart_data: FxHashMap<String, CustomChartData>,
    /// Registry of custom stat pane configurations (by pane type name)
    custom_stat_configs: FxHashMap<String, StatPaneConfig>,
    /// Data for custom stat panes (by pane type name)
    custom_stat_data: FxHashMap<String, StatPaneData>,
    /// Registry of custom gauge pane configurations (by pane type name)
    custom_gauge_configs: FxHashMap<String, GaugePaneConfig>,
    /// Data for custom gauge panes (by pane type name)
    custom_gauge_data: FxHashMap<String, GaugePaneData>,
    /// Last refresh time for plugin panes (by pane type name)
    plugin_pane_last_refresh: FxHashMap<String, crate::util::Instant>,

    // ==================== OTLP Telemetry ====================
    /// Shared in-memory telemetry store for OTLP datasource.
    telemetry_store: Option<std::sync::Arc<enya_client::otlp::TelemetryStore>>,
    /// Whether to connect the OTLP telemetry store on the next frame.
    /// Deferred because `set_telemetry_store` is called before `ctx` is available.
    pending_otlp_connect: bool,

    // ==================== Tracing Backend ====================
    /// Client for fetching distributed traces (Tempo, OTLP HTTP, or in-process OTLP).
    tracing_client: Option<std::sync::Arc<dyn enya_client::tracing::TracingClient + Send + Sync>>,
    /// Manager for in-flight trace fetch/search requests.
    trace_manager: enya_client::tracing::TraceManager,

    // ==================== Async Runtime ====================
    /// Async runtime for spawning background tasks (Clone is cheap — wraps Arc).
    async_runtime: AsyncRuntime,
}

impl Workspace {
    /// Create a new empty dashboard (no landing page)
    pub fn new_empty(async_runtime: AsyncRuntime) -> Self {
        let mut dashboard = Self::new(async_runtime);
        dashboard.show_landing = false;
        dashboard
    }

    /// Create a new workspace with the given async runtime.
    pub fn new(async_runtime: AsyncRuntime) -> Self {
        let mut tiles: Tiles<Box<dyn Component>> = egui_tiles::Tiles::default();

        // Start with empty tabs - show landing page first
        let root = tiles.insert_tab_tile(vec![]);

        let viewport_tree = egui_tiles::Tree::new("viewport_tree", root, tiles);

        let mut behavior = TreeBehavior::default();
        behavior.set_dim_inactive(true); // Enable dim inactive panes by default

        let workspace = Self {
            viewport_tree,
            behavior,
            open_charts: FxHashSet::default(),
            pending_chart: None,
            time_range_toolbar: TimeRangeToolbar::new(),
            command_palette: CommandPalette::new(),
            buffer_editor: BufferEditor::new(),
            editing_tile_id: None,
            zen_mode: false,
            fullscreen_tile: None,
            landing_page: LandingPage::new(),
            show_landing: true, // Start with landing page
            loaded_name: None,
            loaded_project: None,
            leader_keys: LeaderKeyState::new(),
            info_overlay: InfoOverlay::new(enya_build_info::build_info!()),
            about_overlay: AboutOverlay::new(),
            which_key: WhichKey::new(),
            leader_popup: LeaderPopup::new(),
            time_range_picker: TimeRangePicker::new(),
            tutorial_overlay: TutorialOverlay::new(),
            plugins_overlay: PluginsOverlay::new(),
            workspace_creator: WorkspaceCreator::new(),
            viewport_scroll_offset: 0.0,
            viewport_scroll_target: 0.0,
            viewport_content_height: 0.0,
            viewport_visible_height: 0.0,
            visual_multi_state: None,
            multi_buffer_state: MultiBufferState::new(),
            multi_edit_overlay: MultiEditOverlay::new(),
            diagnostics_pane: DiagnosticsPane::new(),
            diagnostics_visible: false,
            pending_open_settings: false,
            cached_flight_sql_connections: Vec::new(),
            pending_open_time_range_picker: false,
            query_executor: QueryExecutor::new(async_runtime.clone()),
            next_query_number: 1,
            pending_inline_charts: Vec::new(),
            next_inline_chart_id: 0,
            viewport_filter: ViewportFilter::new(),
            source_preview: SourcePreviewOverlay::new(),
            #[cfg(not(target_arch = "wasm32"))]
            agent_panel: AgentPanel::new(async_runtime.handle().clone()),
            #[cfg(target_arch = "wasm32")]
            agent_panel: AgentPanel::new(),
            #[cfg(not(target_arch = "wasm32"))]
            agent_input_bar: AgentInputBar::new_with_runtime(async_runtime.handle()),
            #[cfg(target_arch = "wasm32")]
            agent_input_bar: AgentInputBar::new(),
            agent_mode_active: false,
            agent_panel_focused: false,
            agent_context_panes: FxHashSet::default(),
            #[cfg(not(target_arch = "wasm32"))]
            codebase_manager: CodebaseManager::new(),
            #[cfg(not(target_arch = "wasm32"))]
            codebase_finder: CodebaseFinder::new(),
            diff_viewer: DiffViewerOverlay::new(),
            #[cfg(not(target_arch = "wasm32"))]
            pending_git_config: None,
            pending_connection_endpoint: None,
            refresh_interval: None,
            pending_install_plugin: None,
            pending_remove_plugin: None,
            pending_refresh_plugins: false,
            last_refresh: None,
            pending_load_workspace: None,
            github_token: None,
            #[cfg(not(target_arch = "wasm32"))]
            git_credential_token: None,
            #[cfg(not(target_arch = "wasm32"))]
            pending_git_credential: std::sync::Arc::new(parking_lot::Mutex::new(None)),
            #[cfg(not(target_arch = "wasm32"))]
            git_credential_fetched: false,
            #[cfg(target_arch = "wasm32")]
            native_promo_overlay: NativePromoOverlay::new(),
            #[cfg(target_arch = "wasm32")]
            is_mobile: Self::detect_mobile(),
            unified_finder: UnifiedFinder::new(),
            annotation_editor: AnnotationEditor::new(),
            // Floating panes
            floating_panes: FloatingPaneManager::new(),
            // Undo system
            undo_stack: UndoStack::new(),
            // Layout animation
            layout_animator: LayoutAnimator::new(),
            is_snapshot: false,
            snapshot_title: None,
            // Active theme colors
            active_colors: None,
            render_theme: AppTheme::default(),
            // Plugin custom panes
            custom_table_configs: FxHashMap::default(),
            custom_table_data: FxHashMap::default(),
            custom_chart_configs: FxHashMap::default(),
            custom_chart_data: FxHashMap::default(),
            custom_stat_configs: FxHashMap::default(),
            custom_stat_data: FxHashMap::default(),
            custom_gauge_configs: FxHashMap::default(),
            custom_gauge_data: FxHashMap::default(),
            plugin_pane_last_refresh: FxHashMap::default(),
            // OTLP telemetry store
            telemetry_store: None,
            pending_otlp_connect: false,
            // Tracing backend
            tracing_client: None,
            trace_manager: enya_client::tracing::TraceManager::new(),
            // Async runtime
            async_runtime,
        };

        // Eagerly warm up the agent subprocess at app startup so the first
        // AI prompt is fast (npx cold-start happens in the background).
        #[cfg(not(target_arch = "wasm32"))]
        workspace.agent_input_bar.warmup();

        workspace
    }

    /// Get the name of the currently loaded workspace (filename stem).
    pub fn loaded_name(&self) -> Option<&str> {
        self.loaded_name.as_deref()
    }

    /// Set the loaded workspace name and update conversation storage scope.
    pub(crate) fn set_loaded_name(&mut self, name: Option<String>) {
        self.loaded_name = name.clone();
        #[cfg(not(target_arch = "wasm32"))]
        self.agent_panel
            .set_conversation_scope(name, self.loaded_project.clone());
    }

    /// Get the project of the currently loaded workspace.
    pub fn loaded_project(&self) -> Option<&str> {
        self.loaded_project.as_deref()
    }

    /// Set the project for the currently loaded workspace.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn set_loaded_project(&mut self, project: Option<String>) {
        self.loaded_project = project;
    }

    /// Whether the workspace has any panes.
    pub fn has_panes(&self) -> bool {
        !self.get_pane_tile_ids().is_empty()
    }

    /// Set the active theme colors (from custom plugin theme)
    pub fn set_active_colors(&mut self, colors: crate::ui::ActiveThemeColors) {
        self.active_colors = Some(colors);
    }

    /// Clear active theme colors (when no custom plugin theme is active)
    pub fn clear_active_colors(&mut self) {
        self.active_colors = None;
    }

    /// Set the shared OTLP telemetry store.
    ///
    /// When set, the editor connects the OTLP metrics backend on the next
    /// frame and uses in-memory clients for logs and traces.
    pub fn set_telemetry_store(
        &mut self,
        store: std::sync::Arc<enya_client::otlp::TelemetryStore>,
    ) {
        self.telemetry_store = Some(store);
        self.pending_otlp_connect = true;
    }

    /// Get the shared OTLP telemetry store (if available).
    pub fn telemetry_store(&self) -> Option<&std::sync::Arc<enya_client::otlp::TelemetryStore>> {
        self.telemetry_store.as_ref()
    }

    /// Get the effective theme for rendering.
    /// Returns `AppTheme::Custom(colors)` if a custom theme is active,
    /// otherwise returns the builtin theme from app_state.
    ///
    /// NOTE: Prefer using `self.theme()` after the start of `show()` for cleaner code.
    /// This method is kept for cases where app_state is needed to compute the fallback.
    fn effective_theme(&self, app_state: &AppState) -> AppTheme {
        if let Some(colors) = self.active_colors {
            AppTheme::Custom(colors)
        } else {
            app_state.theme
        }
    }

    /// Get the current render theme.
    ///
    /// This is the preferred method to access the theme during rendering.
    /// It returns the effective theme (custom plugin theme if active, otherwise builtin).
    ///
    /// IMPORTANT: Always use this instead of `app_state.theme` to ensure
    /// custom plugin themes are properly applied throughout the UI.
    #[inline]
    fn theme(&self) -> AppTheme {
        self.render_theme
    }

    #[profiling::function]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_state: &AppState,
    ) -> WorkspaceAction {
        // Cache the effective theme for this frame - use self.theme() throughout rendering
        self.render_theme = self.effective_theme(app_state);

        self.behavior.set_theme(self.theme());

        // Pass GitHub token to any PR review panes.
        // On native, prefer token from `git credential fill` (has org repo access)
        // and fall back to the OAuth token from settings.
        let oauth_token = app_state
            .settings
            .github_credentials
            .as_ref()
            .map(|c| c.access_token.clone());
        self.github_token = oauth_token.clone();

        #[cfg(not(target_arch = "wasm32"))]
        {
            // Kick off git credential fetch once
            if !self.git_credential_fetched {
                self.git_credential_fetched = true;
                let pending = std::sync::Arc::clone(&self.pending_git_credential);
                self.async_runtime.spawn(async move {
                    let result = crate::github_auth::git_credential_fill().await;
                    *pending.lock() = Some(result);
                });
            }
            // Poll for completion
            if let Some(result) = self.pending_git_credential.lock().take() {
                match result {
                    Ok(token) => {
                        log::info!("Git credential token acquired for PR review");
                        self.git_credential_token = Some(token);
                    }
                    Err(e) => {
                        log::debug!("git credential fill: {e}");
                    }
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        let pr_token = self.git_credential_token.clone().or(oauth_token);
        #[cfg(target_arch = "wasm32")]
        let pr_token = oauth_token;

        let focused_tile = self.behavior.focused_tile();
        for tile_id in self.get_pane_tile_ids() {
            if let Some(egui_tiles::Tile::Pane(pane)) = self.viewport_tree.tiles.get_mut(tile_id) {
                if let Some(pr_pane) = pane
                    .as_any_mut()
                    .downcast_mut::<crate::components::PrReviewPane>()
                {
                    pr_pane.set_token(pr_token.clone());
                    pr_pane.set_focused(focused_tile == Some(tile_id));
                }
            }
        }

        // Update visual effects (focus pulse detection, cleanup)
        self.behavior.update_focus_effects();
        self.behavior.cleanup_effects();
        if self.behavior.has_active_effects() {
            ctx.request_repaint();
        }

        // Update layout animations and apply shares to the tree
        if self.layout_animator.has_active_animations() {
            let share_updates = self.layout_animator.update();
            for (container_id, shares) in share_updates {
                if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) =
                    self.viewport_tree.tiles.get_mut(container_id)
                {
                    for (tile_id, share) in shares {
                        linear.shares.set_share(tile_id, share);
                    }
                }
            }
            ctx.request_repaint();
        }

        // Disable terminal keyboard input when modals are open
        // This prevents terminal from capturing j/k/h/l keys meant for overlays
        #[cfg(not(target_arch = "wasm32"))]
        {
            let modal_open = self.time_range_picker.is_open()
                || self.unified_finder.is_open()
                || self.command_palette.is_open()
                || self.buffer_editor.is_open()
                || self.multi_edit_overlay.is_open()
                || self.which_key.is_open()
                || self.viewport_filter.is_open()
                || self.tutorial_overlay.is_open()
                || self.plugins_overlay.is_open()
                || self.source_preview.is_open()
                || self.agent_panel.is_open();
            self.set_terminal_keyboard_enabled(!modal_open);
        }

        // Poll and process agent input bar commands BEFORE query execution
        // This ensures panes created by AI are available for immediate query execution
        if self.agent_mode_active {
            self.agent_input_bar.poll(ctx);
            let commands = self.agent_input_bar.take_pending_commands();
            if !commands.is_empty() {
                log::debug!(
                    "Processing {} agent command(s) before query execution",
                    commands.len()
                );
                for cmd in &commands {
                    log::debug!("Executing agent command: {cmd:?}");
                }
                let activities = self.handle_agent_commands(commands, ctx);
                // Add activities to input bar for display
                for activity in &activities {
                    self.agent_input_bar.add_activity(activity.clone());
                }
                // Only auto-exit if commands were executed AND there's no response text
                // This allows conversational flows where the agent explains what it's doing
                let has_response_text = !self.agent_input_bar.display_text().is_empty();
                if !activities.is_empty() && !has_response_text {
                    log::debug!("Agent command executed (no response text), exiting agent mode");
                    self.exit_agent_mode();
                }
            }
        }

        // Handle pending workspace load from agent command
        if let Some(name) = self.pending_load_workspace.take() {
            return WorkspaceAction::LoadWorkspace {
                name,
                project: None,
            };
        }

        // Check auto-refresh timer and trigger refresh if due
        self.check_auto_refresh();

        // Check if automatic git fetch is due (native only)
        #[cfg(not(target_arch = "wasm32"))]
        self.codebase_manager.check_auto_git_sync(ctx);

        // Process query execution: poll for results and execute pending queries
        let query_action = self.process_query_execution(ctx);
        if query_action != WorkspaceAction::None {
            return query_action;
        }

        // Handle pending git config initialization (native only with codebase feature)
        // This deferred pattern is needed because load_workspace_config() doesn't have ctx
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(url) = self.pending_git_config.take() {
            self.codebase_manager.clone_repo(&url, ctx);
        }

        // Handle pending connection initialization
        // This deferred pattern is needed because load_workspace_config() doesn't have ctx
        if let Some(endpoint) = self.pending_connection_endpoint.take() {
            log::info!("Applying connection from workspace config: {endpoint}");
            self.query_executor.connect_prometheus(&endpoint, ctx);
            // Start fetching metadata for autocomplete
            self.query_executor.fetch_metric_names(ctx);
            self.query_executor.fetch_label_names(ctx);
        }

        // Connect the OTLP metrics backend if a telemetry store was set.
        // On initial startup (from set_telemetry_store), only auto-connect if no
        // When a telemetry store is available, set it as a supplementary OTLP
        // data source on the query executor (merges metric names alongside the
        // primary Prometheus backend) and set up the tracing client.
        // If no Prometheus endpoint is configured, OTLP becomes the primary backend.
        #[cfg(not(target_arch = "wasm32"))]
        if self.pending_otlp_connect {
            self.pending_otlp_connect = false;
            if let Some(store) = &self.telemetry_store {
                let store = std::sync::Arc::clone(store);
                // Always set as supplementary source for metric name merging
                self.query_executor.set_otlp_client(store.clone());
                // If no other backend is active, make OTLP the primary
                if !self.query_executor.is_connected() {
                    log::info!(
                        "Connecting OTLP as primary metrics backend (no Prometheus configured)"
                    );
                    self.query_executor.connect_otlp(store.clone(), ctx);
                    self.query_executor.fetch_metric_names(ctx);
                    self.query_executor.fetch_label_names(ctx);
                    // Enable auto-refresh for live OTLP data
                    if self.refresh_interval.is_none() {
                        self.set_refresh_interval(RefreshInterval::TenSeconds);
                    }
                }
                // Set up tracing client for in-process OTLP
                if self.tracing_client.is_none() {
                    self.tracing_client = Some(std::sync::Arc::new(
                        enya_client::otlp::OtlpTracingClient::new(store),
                    ));
                }
            }
        }

        // Eagerly fetch metric names for @ mention autocomplete (demo backend only).
        // Skip if a Prometheus connection is pending or already active — those paths
        // trigger their own fetch_metric_names() call.
        if self.query_executor.metric_names().is_empty()
            && !self.query_executor.is_fetching_metrics()
            && !self.query_executor.is_connected()
            && self.pending_connection_endpoint.is_none()
        {
            self.query_executor.fetch_metric_names(ctx);
        }

        // Poll codebase manager for clone/index completion (native only with codebase feature)
        #[cfg(not(target_arch = "wasm32"))]
        self.codebase_manager.poll(ctx);

        // Sync git commits to panes when codebase is ready (native only with codebase feature)
        #[cfg(not(target_arch = "wasm32"))]
        self.sync_commits_to_panes(ctx);

        // Handle edit button clicks from panes (opens buffer editor)
        for tile_id in self.get_pane_tile_ids() {
            if let Some(egui_tiles::Tile::Pane(component)) =
                self.viewport_tree.tiles.get_mut(tile_id)
            {
                if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                    if query_pane.edit_requested() {
                        query_pane.clear_edit_requested();
                        // Focus this tile and open the buffer editor
                        self.behavior.set_focused_tile(Some(tile_id));
                        self.editing_tile_id = Some(tile_id);

                        let query = query_pane.saved_query().to_string();
                        let name = query_pane.name().to_string();
                        let state = query_pane.query_state().clone();
                        self.buffer_editor.open_with_state(&query, &name, state);

                        // Populate completions from cached metric labels
                        if let Some(labels) = self.query_executor.get_metric_labels(&name) {
                            self.buffer_editor
                                .set_completions_from_labels(&labels.labels);
                        } else if self.query_executor.is_connected() {
                            self.buffer_editor.clear_completions();
                        }

                        // Set known metric names for completion
                        let metric_names = self.query_executor.metric_names().to_vec();
                        self.buffer_editor.set_metric_names(metric_names);
                        break;
                    }
                }

                // Handle LogsPane edit button click - opens modal BufferEditor with LogQL mode
                if let Some(logs_pane) = component.as_any_mut().downcast_mut::<LogsPane>() {
                    if logs_pane.edit_requested() {
                        logs_pane.clear_edit_requested();
                        // Focus this tile and open the buffer editor
                        self.behavior.set_focused_tile(Some(tile_id));
                        self.editing_tile_id = Some(tile_id);

                        let query = logs_pane.saved_query().to_string();
                        let name = logs_pane.name().to_string();
                        // Use LogQL completion mode for LogsPane
                        self.buffer_editor
                            .open_with_language(&query, &name, QueryLanguage::LogQL);

                        log::debug!("Opening buffer editor for LogsPane (button click)");
                        break;
                    }
                }
            }
        }

        // Sync visual-multi state to behavior for rendering
        let (is_visual_multi, selected_ids, tile_queries) = match &self.visual_multi_state {
            Some(state) => {
                // Build query mapping for selected tiles
                let mut queries = FxHashMap::default();
                for &tile_id in &state.selected_tile_ids {
                    if let Some(egui_tiles::Tile::Pane(component)) =
                        self.viewport_tree.tiles.get(tile_id)
                    {
                        if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                            queries.insert(tile_id, query_pane.query().to_string());
                        }
                    }
                }
                (true, state.selected_tile_ids.clone(), queries)
            }
            None => (false, FxHashSet::default(), FxHashMap::default()),
        };
        self.behavior
            .set_visual_multi_state(is_visual_multi, selected_ids, tile_queries);

        // Sync viewport filter state to behavior for rendering
        let filtered_out_tiles = if self.viewport_filter.is_active() {
            self.get_pane_tile_ids()
                .into_iter()
                .filter(|&tile_id| {
                    if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                        // Check QueryPane
                        if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                            return !self.viewport_filter.matches(query_pane.saved_query());
                        }
                        // Check Buffer
                        if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                            return !self.viewport_filter.matches(buffer.saved_content());
                        }
                    }
                    false // Unknown component types are always shown
                })
                .collect()
        } else {
            FxHashSet::default()
        };
        self.behavior
            .set_filter_state(self.viewport_filter.is_active(), filtered_out_tiles);

        // Update component themes
        self.time_range_toolbar.set_theme(self.theme());
        self.landing_page.set_theme(self.theme());
        #[cfg(target_arch = "wasm32")]
        self.landing_page.set_mobile(self.is_mobile);

        // Handle adding a pending chart to the viewport
        if let Some(metric_name) = self.pending_chart.take() {
            let action = self.add_chart_for_metric_with_tracking(&metric_name);
            if action != WorkspaceAction::None {
                return action;
            }
        }

        // Handle pending settings page open
        if self.pending_open_settings {
            self.pending_open_settings = false;
            return WorkspaceAction::OpenSettings;
        }

        // Handle pending time range picker open
        if self.pending_open_time_range_picker {
            self.pending_open_time_range_picker = false;
            self.time_range_picker.open();
        }

        // Show landing page only if explicitly enabled and no charts open
        // (new workspaces start with show_landing=false for a clean empty state)
        if self.show_landing && self.open_charts.is_empty() {
            return self.show_landing_page(ui, ctx, app_state);
        }

        // Check if any overlay is open that should block keyboard input
        let overlay_blocks_input = self.time_range_picker.is_open()
            || self.unified_finder.is_open()
            || self.command_palette.is_open()
            || self.which_key.is_open();

        // Propagate overlay_blocks_input to all pane components
        for (_tile_id, tile) in self.viewport_tree.tiles.iter_mut() {
            if let egui_tiles::Tile::Pane(component) = tile {
                component.set_overlay_blocks_input(overlay_blocks_input);
            }
        }

        // Right sidebar: Agent panel (Claude Code integration)
        // Rendered as a layout participant - viewport shrinks when panel is open
        // Update context before showing so the agent has awareness of editor state
        self.update_agent_context();
        self.agent_panel.set_theme(self.theme());
        self.agent_panel.set_focus(self.agent_panel_focused);
        // Provide available metrics for @mention autocomplete
        let metric_names = self.query_executor.metric_names().to_vec();
        self.agent_panel.set_available_metrics(metric_names);
        // Disable keyboard when diff viewer is open
        self.agent_panel
            .set_keyboard_disabled(self.diff_viewer.is_open());
        match self.agent_panel.show_inside(ui, ctx) {
            AgentPanelResult::Closed => {
                self.agent_panel_focused = false;
                self.agent_panel.set_focus(false);
                // Restore focus to last pane if nothing else has focus
                if self.behavior.focused_tile().is_none() {
                    if let Some(last_pane) = self.get_pane_tile_ids().last() {
                        self.behavior.set_focused_tile(Some(*last_pane));
                    }
                }
            }
            AgentPanelResult::Commands(commands) => {
                let activities = self.handle_agent_commands(commands, ctx);
                self.agent_panel.add_activities(activities);
            }
            AgentPanelResult::ReturnFocusToViewport => {
                // Vim h key pressed - return focus to viewport
                self.agent_panel_focused = false;
                self.agent_panel.set_focus(false);
            }
            AgentPanelResult::EnteredInputMode => {
                // User pressed i or Enter to enter chat input - release vim focus
                // but keep panel open (user is now typing in the text input)
                self.agent_panel_focused = false;
            }
            AgentPanelResult::Error(msg) => {
                return WorkspaceAction::Notify {
                    level: "error".to_string(),
                    message: msg,
                };
            }
            AgentPanelResult::None => {
                // Don't sync - agent_panel_focused should only change via explicit actions
                // (ReturnFocusToViewport, Closed, or keyboard transfer).
                // The panel's internal has_focus tracks vim focus within the panel,
                // while agent_panel_focused tracks whether the workspace considers the panel active.
            }
            AgentPanelResult::OpenDiffViewer { hash, message } => {
                // Open the full diff viewer for this commit
                log::debug!("Opening diff viewer from inline diff: {hash}");
                self.open_diff_viewer_for_commit(&hash, &message);
            }
        }

        // Detect clicks in the viewport area to transfer focus from agent panel.
        // After the SidePanel renders, the remaining available rect is the viewport.
        // Without this, agent_panel_focused gets stuck when clicking viewport panes
        // instead of pressing 'h' to return focus, blocking all keyboard shortcuts.
        if self.agent_panel_focused && self.agent_panel.is_open() {
            let viewport_area = ui.available_rect_before_wrap();
            if ctx.input(|i| {
                i.pointer.any_pressed()
                    && i.pointer
                        .press_origin()
                        .is_some_and(|pos| viewport_area.contains(pos))
            }) {
                self.agent_panel_focused = false;
                self.agent_panel.set_focus(false);
            }
        }

        {
            // Main area with toolbar and viewport
            egui::CentralPanel::default().show_inside(ui, |ui| {
            // Top toolbar with filter (left) and time range controls (right)
            // Hidden in zen mode or when workspace is empty (landing page shows its own hints)
            let total_panes = self.get_pane_tile_ids().len();
            if !self.zen_mode && total_panes > 0 {
                // Get countdown before borrowing self mutably
                let countdown = self.time_until_refresh();
                let matching_panes = if self.viewport_filter.is_active() {
                    self.get_pane_tile_ids()
                        .iter()
                        .filter(|tile_id| {
                            if let Some(egui_tiles::Tile::Pane(pane)) =
                                self.viewport_tree.tiles.get(**tile_id)
                            {
                                self.viewport_filter.matches(&pane.name())
                            } else {
                                true
                            }
                        })
                        .count()
                } else {
                    total_panes
                };
                self.viewport_filter.update_counts(matching_panes, total_panes);
                self.viewport_filter.set_theme(self.theme());

                egui::TopBottomPanel::top("time_range_toolbar")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            // Left side: Filter input
                            match self.viewport_filter.show_inline(ui) {
                                ViewportFilterResult::Applied(pattern) => {
                                    log::debug!("Toolbar filter applied: {pattern}");
                                }
                                ViewportFilterResult::Cleared => {
                                    log::debug!("Toolbar filter cleared");
                                }
                                ViewportFilterResult::None => {}
                            }

                            // Right side: time range controls (laid out right-to-left)
                            // Measure remaining space so we can center hints in the gap
                            let remaining_rect = ui.available_rect_before_wrap();
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    self.time_range_toolbar.show_with_countdown(ui, countdown);

                                    // Keyboard hints centered in whatever space remains
                                    // between the filter (left) and time controls (right).
                                    // ui.available_width() here is the gap after time controls
                                    // consumed their share from the right.
                                    if total_panes > 0 {
                                        let gap = ui.available_width();
                                        let hint_text = "hjkl navigate   : cmd   ? help";
                                        let hint_color = self.theme().text_tertiary();
                                        let font = egui::FontId::proportional(11.0);
                                        let hint_galley = ui.painter().layout_no_wrap(
                                            hint_text.to_string(),
                                            font,
                                            hint_color,
                                        );
                                        // Only paint if the hints fit with some breathing room
                                        if hint_galley.size().x + 32.0 < gap {
                                            let hint_pos = egui::pos2(
                                                remaining_rect.left()
                                                    + (gap - hint_galley.size().x) / 2.0,
                                                remaining_rect.center().y
                                                    - hint_galley.size().y / 2.0,
                                            );
                                            ui.painter().galley(
                                                hint_pos,
                                                hint_galley,
                                                hint_color,
                                            );
                                        }
                                    }
                                },
                            );
                        });

                        ui.add_space(4.0);
                    });
            }

            // Trigger global refresh when time range changes (Grafana-style)
            if self.time_range_toolbar.changed() {
                self.refresh_all_panes();
            }

            // Open time range picker when custom button is clicked
            if self.time_range_toolbar.custom_clicked() {
                self.pending_open_time_range_picker = true;
            }

            // Main viewport area (tabbed charts/views)
            // Use a frame that clips content to prevent overflow onto status bar
            egui::CentralPanel::default()
                .frame(egui::Frame::central_panel(ui.style()).inner_margin(0.0))
                .show_inside(ui, |ui| {
                // Get the exact available rect and set it as the clip rect on the painter
                // This prevents any content from painting outside this area
                let panel_rect = ui.available_rect_before_wrap();
                ui.set_clip_rect(panel_rect);

                if let Some(fullscreen_id) = self.fullscreen_tile {
                    // Render only the fullscreen pane
                    if let Some(Tile::Pane(component)) =
                        self.viewport_tree.tiles.get_mut(fullscreen_id)
                    {
                        component.set_theme(self.behavior.theme());
                        component.show(ui);
                    } else {
                        // Tile no longer exists, exit fullscreen
                        self.fullscreen_tile = None;
                        self.viewport_tree.ui(&mut self.behavior, ui);
                    }
                } else if self.viewport_filter.is_active() {
                    // Render filtered view - only matching panes in a grid
                    self.render_filtered_view(ui);
                } else if self.get_pane_tile_ids().is_empty() {
                    // Show empty workspace hint
                    self.render_empty_workspace_hint(ui);
                } else {
                    // Store available rect before layout for scrollbar positioning
                    let full_rect = ui.available_rect_before_wrap();

                    // Scrollbar dimensions
                    const SCROLLBAR_WIDTH: f32 = 10.0; // Width including padding

                    // Calculate if we need scrolling based on absolute minimum pane height
                    // This matches TreeBehavior::min_size() (200px) which is the floor enforced by egui_tiles
                    // Scrolling only kicks in when panes would be smaller than the absolute minimum
                    const MIN_PANE_HEIGHT: f32 = 200.0;
                    let pane_count = self.get_pane_tile_ids().len();
                    let min_content_height = pane_count as f32 * MIN_PANE_HEIGHT;
                    let needs_scrollbar = min_content_height > full_rect.height();

                    // Use the exact available height
                    self.viewport_visible_height = full_rect.height();

                    // Animate scroll offset towards target (smooth scrolling)
                    let scroll_speed = 12.0; // Higher = faster animation
                    let dt = ctx.input(|i| i.predicted_dt);
                    let diff = self.viewport_scroll_target - self.viewport_scroll_offset;
                    if diff.abs() > 0.5 {
                        self.viewport_scroll_offset += diff * scroll_speed * dt;
                        ctx.request_repaint();
                    } else {
                        self.viewport_scroll_offset = self.viewport_scroll_target;
                    }

                    // Always set tree height to viewport height - the tree's min_size()
                    // ensures panes don't get smaller than 200px, and ScrollArea handles
                    // any overflow via clipping
                    self.viewport_tree.set_height(self.viewport_visible_height);

                    // Calculate viewport and scrollbar rects
                    let viewport_width = if needs_scrollbar {
                        full_rect.width() - SCROLLBAR_WIDTH
                    } else {
                        full_rect.width()
                    };

                    let viewport_rect = egui::Rect::from_min_size(
                        full_rect.min,
                        egui::vec2(viewport_width, full_rect.height()),
                    );

                    // Create a child UI constrained to the viewport rect with explicit clip rect
                    let scroll_output = ui
                        .new_child(
                            egui::UiBuilder::new()
                                .max_rect(viewport_rect)
                                .layout(egui::Layout::top_down(egui::Align::LEFT)),
                        )
                        .with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                            // Set clip rect to viewport bounds - this is critical
                            ui.set_clip_rect(viewport_rect);

                            let tiles_before_ui = self.viewport_tree.tiles.len();
                            let output = egui::ScrollArea::vertical()
                                .id_salt("viewport_scroll")
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                )
                                .vertical_scroll_offset(self.viewport_scroll_offset)
                                .auto_shrink([false, false])
                                .max_height(self.viewport_visible_height)
                                .show(ui, |ui| {
                                    self.viewport_tree.ui(&mut self.behavior, ui);
                                });
                            let tiles_after_ui = self.viewport_tree.tiles.len();
                            if tiles_before_ui != tiles_after_ui {
                                log::warn!(
                                    "viewport_tree.ui() changed tile count: {tiles_before_ui} -> {tiles_after_ui} (GC may have removed tiles)"
                                );
                            }
                            output
                        })
                        .inner;

                    // Update scroll state from ScrollArea output
                    self.viewport_content_height = scroll_output.content_size.y;

                    // Sync scroll offset if user scrolled with mouse
                    let current_offset = scroll_output.state.offset.y;
                    if (current_offset - self.viewport_scroll_offset).abs() > 1.0 {
                        self.viewport_scroll_offset = current_offset;
                        self.viewport_scroll_target = current_offset;
                    }

                    // Scrollbar gutter on the right (only if needed)
                    if needs_scrollbar {
                        let scrollbar_rect = egui::Rect::from_min_size(
                            egui::pos2(viewport_rect.right(), full_rect.top()),
                            egui::vec2(SCROLLBAR_WIDTH, full_rect.height()),
                        );
                        self.draw_scrollbar(ui.painter(), scrollbar_rect, self.theme());
                    }
                }
            });
        });
        }

        // ==================== Floating Panes ====================
        // Render floating panes above the tile layout but below modal overlays
        self.floating_panes.set_theme(self.theme());
        // Use the available rect as the viewport for floating pane snapping/maximize
        let floating_viewport = ctx.available_rect();
        let floating_actions = self
            .floating_panes
            .show(ctx, self.theme(), floating_viewport);
        for (pane_id, action) in floating_actions {
            match action {
                FloatingPaneAction::Close => {
                    self.floating_panes.remove_pane(pane_id);
                }
                FloatingPaneAction::Dock => {
                    // Capture floating pane state BEFORE removal for undo
                    let pane_state = self
                        .floating_panes
                        .panes
                        .iter()
                        .find(|p| p.id == pane_id)
                        .map(|p| (p.component.name(), p.position, p.size, p.pinned));

                    // Dock the floating pane back into the tile layout
                    if let Some(component) = self.floating_panes.remove_pane(pane_id) {
                        let pane_tile = self.viewport_tree.tiles.insert_pane(component);
                        if self.add_tile_to_viewport(pane_tile) {
                            self.behavior.set_focused_tile(Some(pane_tile));
                            self.show_landing = false;

                            // Push undo action with captured state (use component name, not tile_id)
                            if let Some((name, position, size, pinned)) = pane_state {
                                let docked_info = DockedPaneInfo {
                                    component_name: name,
                                    position,
                                    size,
                                    pinned,
                                };
                                self.undo_stack.push(UndoAction::UndockPane(docked_info));
                                log::debug!("Pushed dock pane to undo stack");
                            }
                        }
                    }
                }
                FloatingPaneAction::BringToFront => {
                    self.floating_panes.bring_to_front(pane_id);
                    self.floating_panes.set_focus(Some(pane_id));
                }
                FloatingPaneAction::ToggleMinimize => {
                    self.floating_panes.toggle_minimize(pane_id);
                }
                FloatingPaneAction::TogglePin => {
                    if let Some(pane) = self
                        .floating_panes
                        .panes
                        .iter_mut()
                        .find(|p| p.id == pane_id)
                    {
                        pane.pinned = !pane.pinned;
                    }
                }
                FloatingPaneAction::ToggleMaximize => {
                    self.floating_panes
                        .toggle_maximize(pane_id, floating_viewport);
                }
                #[cfg(not(target_arch = "wasm32"))]
                FloatingPaneAction::PopOut | FloatingPaneAction::PopIn => {
                    self.floating_panes.toggle_pop_out(pane_id);
                }
                FloatingPaneAction::None => {}
            }
        }

        // Show diff viewer overlay modal
        {
            self.diff_viewer.set_theme(self.theme());
            // Set repo root for file opener (native only)
            #[cfg(not(target_arch = "wasm32"))]
            self.diff_viewer.set_repo_root(
                self.codebase_manager
                    .index()
                    .map(|idx| idx.repo_path.clone()),
            );
            // Disable keyboard when another overlay is on top
            self.diff_viewer.set_keyboard_disabled(
                self.time_range_picker.is_open() || self.command_palette.is_open(),
            );
            match self.diff_viewer.show(ctx) {
                DiffViewerResult::Error(msg) => {
                    return WorkspaceAction::Notify {
                        level: "error".to_string(),
                        message: msg,
                    };
                }
                DiffViewerResult::Closed | DiffViewerResult::None => {}
            }
        }

        // Show time range picker modal
        self.time_range_picker.set_theme(self.theme());
        match self.time_range_picker.show(ctx) {
            TimeRangePickerResult::Selected {
                start_secs,
                end_secs,
            } => {
                self.time_range_toolbar
                    .set_custom_range(start_secs, end_secs);
                self.refresh_all_panes();
            }
            TimeRangePickerResult::Cancelled | TimeRangePickerResult::None => {}
        }

        // Show unified finder modal (Telescope-style)
        if let Some(action) = self.show_unified_finder(ctx, app_state) {
            return action;
        }

        // Show codebase finder modal (native only with Tantivy search)
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Update codebase status for the finder
            let status = match self.codebase_manager.status() {
                crate::codebase::CodebaseStatus::None => CodebaseFinderStatus::NoCodebase,
                crate::codebase::CodebaseStatus::Cloning { .. }
                | crate::codebase::CodebaseStatus::Fetching { .. }
                | crate::codebase::CodebaseStatus::Indexing { .. } => {
                    CodebaseFinderStatus::Indexing
                }
                crate::codebase::CodebaseStatus::Ready { metrics_count, .. } => {
                    CodebaseFinderStatus::Ready {
                        metric_count: *metrics_count,
                    }
                }
                crate::codebase::CodebaseStatus::Error { .. } => CodebaseFinderStatus::NoCodebase,
            };
            self.codebase_finder.set_status(status);

            // Update search results when query or filter changes
            let query = self.codebase_finder.query().to_string();
            let filter = self.codebase_finder.filter();
            if self.codebase_finder.is_open() && !query.is_empty() {
                let results = self.codebase_manager.search_ranked(&query, filter, 50);
                self.codebase_finder.set_results(results);
            }

            self.codebase_finder.set_theme(self.theme());
            if let Some(finder_result) = self.codebase_finder.show(ctx) {
                // Handle the selected result - navigate to source
                self.handle_codebase_finder_result(finder_result.result);
            }
        }

        // Show command palette modal
        self.command_palette.set_theme(self.theme());
        let cmd_result = self.command_palette.show(ctx);

        // Show buffer editor modal
        self.buffer_editor.set_theme(self.theme());
        match self.buffer_editor.show(ctx) {
            BufferEditorResult::Saved(query, query_state) => {
                self.apply_buffer_editor_result(query, query_state);
            }
            BufferEditorResult::Cancelled => {
                self.editing_tile_id = None;
            }
            BufferEditorResult::None => {}
        }

        // Show multi-edit overlay
        self.multi_edit_overlay.set_theme(self.theme());
        match self.multi_edit_overlay.show(ctx) {
            MultiEditResult::Applied(changes) => {
                self.apply_multi_edit_changes(changes);
            }
            MultiEditResult::Cancelled | MultiEditResult::None => {}
        }

        // Show info overlay modal
        self.info_overlay.set_theme(self.theme());
        self.info_overlay.show(ctx);

        // Show about overlay modal
        self.about_overlay.set_theme(self.theme());
        self.about_overlay.show(ctx);

        // Show which-key overlay modal
        self.which_key.set_theme(self.theme());
        self.which_key.show(ctx);

        // Show leader popup (dynamic hints, like which-key.nvim)
        self.leader_popup.set_theme(self.theme());
        // Space has no timeout - stays active until cleared
        self.leader_popup
            .update_visibility(LeaderKey::Space, self.leader_keys.last_space_press);
        #[cfg(not(target_arch = "wasm32"))]
        let is_native = true;
        #[cfg(target_arch = "wasm32")]
        let is_native = false;
        self.leader_popup.show(ctx, LeaderKey::Space, is_native);

        // Show tutorial overlay modal
        self.tutorial_overlay.set_theme(self.theme());
        self.tutorial_overlay.show(ctx);

        // Show plugins overlay modal
        self.plugins_overlay.set_theme(self.theme());
        match self.plugins_overlay.show(ctx) {
            PluginsOverlayResult::OpenPluginDirectory => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let plugin_dir = enya_config::plugins_dir();
                    if let Err(e) = open::that(&plugin_dir) {
                        log::warn!("Failed to open plugin directory: {e}");
                    }
                }
            }
            PluginsOverlayResult::TogglePlugin(name) => {
                log::debug!("Toggle plugin: {name}");
                // TODO: Implement plugin enable/disable via PluginRegistry
            }
            PluginsOverlayResult::InstallPlugin(name, file) => {
                log::debug!("Install plugin: {name} from {file}");
                // Set installing state to show spinner
                self.plugins_overlay
                    .set_installing_plugin(Some(name.clone()));
                self.pending_install_plugin = Some((name, file));
            }
            PluginsOverlayResult::RemovePlugin(name) => {
                log::debug!("Remove plugin: {name}");
                self.pending_remove_plugin = Some(name);
            }
            PluginsOverlayResult::RefreshAvailable => {
                log::debug!("Refresh available plugins");
                self.pending_refresh_plugins = true;
            }
            PluginsOverlayResult::Closed | PluginsOverlayResult::None => {}
        }

        // Show workspace creator overlay modal
        self.workspace_creator.set_theme(self.theme());
        match self.workspace_creator.show(ctx) {
            WorkspaceCreatorResult::Created {
                name,
                endpoint,
                git_repo,
                flight_sql_endpoint,
                project,
            } => {
                self.pending_connection_endpoint = Some(endpoint);
                // Store git repo path for codebase integration (native only with codebase feature)
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let new_url = git_repo.as_deref();
                    let current_url = self.codebase_manager.status().url();
                    if new_url != current_url {
                        self.codebase_manager.reset();
                    }
                    self.pending_git_config = git_repo;
                }
                #[cfg(target_arch = "wasm32")]
                let _ = git_repo; // Silence unused warning
                self.show_landing = false;
                ctx.request_repaint();
                // Return action to save the workspace and create/assign project
                return WorkspaceAction::SaveWorkspace {
                    name: Some(name),
                    project: Some(project),
                    flight_sql_endpoint,
                };
            }
            WorkspaceCreatorResult::Cancelled | WorkspaceCreatorResult::None => {}
        }

        // Show diagnostics overlay modal
        self.diagnostics_pane.set_theme(self.theme());
        self.diagnostics_pane.show_overlay(ctx);

        // Show source preview overlay modal
        if self.source_preview.is_open() {
            log::debug!("source_preview.is_open() = true, calling show()");
        }
        self.source_preview.set_theme(self.theme());
        match self.source_preview.show(ctx) {
            SourcePreviewResult::Closed => {
                log::debug!("Source preview closed");
            }
            SourcePreviewResult::Error(msg) => {
                return WorkspaceAction::Notify {
                    level: "error".to_string(),
                    message: msg,
                };
            }
            SourcePreviewResult::None => {}
        }

        // Note: diff_viewer is now rendered earlier to ensure proper z-order

        // Note: Agent panel is now rendered in the layout section (show_inside)
        // to participate in layout flow like the channels panel

        // Note: agent_input_bar.poll() is now called at the start of show()
        // to ensure agent-created panes are available for immediate query execution

        // Poll codebase manager for async operations (native only with codebase feature)
        #[cfg(not(target_arch = "wasm32"))]
        self.codebase_manager.poll(ctx);

        // Poll agent panes for pending commands
        self.poll_agent_pane_commands(ctx);

        // Update viewport filter state (rendering happens in bottom panel)
        self.viewport_filter.set_theme(self.theme());
        let (match_count, total_count) = self.count_filtered_panes();
        self.viewport_filter.update_counts(match_count, total_count);

        // Handle / key for viewport filter (vim-style search)
        // NOTE: Must run BEFORE the ? handler since both use the Slash key
        // Only available in Normal mode (not Visual, Insert, or Agent mode)
        #[cfg(not(target_arch = "wasm32"))]
        let codebase_finder_open = self.codebase_finder.is_open();
        #[cfg(target_arch = "wasm32")]
        let codebase_finder_open = false;

        // Don't intercept '/' when any text widget has focus (e.g., SQL pane input)
        let text_widget_focused = ctx.memory(|mem| mem.focused().is_some());

        if !self.which_key.is_open()
            && !self.unified_finder.is_open()
            && !self.command_palette.is_open()
            && !self.buffer_editor.is_open()
            && !self.viewport_filter.is_open()
            && !self.plugins_overlay.is_open()
            && !self.tutorial_overlay.is_open()
            && !codebase_finder_open
            && !self.is_any_buffer_in_insert_mode()
            && !self.agent_mode_active
            && !self.is_visual_multi_mode()
            && !text_widget_focused
        {
            ctx.input_mut(|input| {
                // Check for '/' character in text input (works across keyboard layouts)
                let has_slash = input
                    .events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t == "/"));
                if has_slash || input.consume_key(egui::Modifiers::NONE, egui::Key::Slash) {
                    // Consume the text event to prevent it from being handled elsewhere
                    input
                        .events
                        .retain(|e| !matches!(e, egui::Event::Text(t) if t == "/"));
                    self.viewport_filter.open();
                }
            });
        }

        // Handle ? key for which-key overlay (don't intercept when text widget has focus)
        if !self.which_key.is_open()
            && !self.unified_finder.is_open()
            && !self.command_palette.is_open()
            && !self.buffer_editor.is_open()
            && !self.viewport_filter.is_open()
            && !self.plugins_overlay.is_open()
            && !self.tutorial_overlay.is_open()
            && !codebase_finder_open
            && !self.agent_mode_active
            && !text_widget_focused
        {
            ctx.input_mut(|input| {
                // Check for '?' character in text input (works across keyboard layouts)
                let has_question_mark = input
                    .events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Text(t) if t == "?"));
                if has_question_mark || input.consume_key(egui::Modifiers::SHIFT, egui::Key::Slash)
                {
                    // Consume the text event to prevent it from being handled elsewhere
                    input
                        .events
                        .retain(|e| !matches!(e, egui::Event::Text(t) if t == "?"));
                    self.which_key.open();
                }
            });
        }

        // Clear leader key state when window/canvas loses focus.
        // In WASM, this prevents stuck keybindings when the browser tab is switched
        // or the user clicks outside the canvas (keyup events are lost on blur).
        if !ctx.input(|i| i.focused) {
            self.leader_keys.clear_all();
        }

        // Handle vim-style keyboard navigation for viewport
        // (only if no modal is open)
        if !self.buffer_editor.is_open() {
            if let Some(action) = self.handle_viewport_keyboard(ctx) {
                return action;
            }
        }

        self.handle_command_result(cmd_result, ctx)
    }

    /// Show the landing page and handle its actions
    fn show_landing_page(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_state: &AppState,
    ) -> WorkspaceAction {
        // On WASM, show native app promo overlay only if user clicked to open it
        // Skip processing when another modal overlay (settings, etc.) is active to
        // prevent native_promo from consuming keyboard events meant for that overlay
        #[cfg(target_arch = "wasm32")]
        let native_promo_open = {
            let other_modal_open = self.plugins_overlay.is_open() || self.which_key.is_open();
            self.native_promo_overlay.set_theme(self.theme());
            if !other_modal_open {
                self.native_promo_overlay.show(ctx);
            }
            self.native_promo_overlay.is_open()
        };
        #[cfg(not(target_arch = "wasm32"))]
        let native_promo_open = false;

        // Disable landing page keyboard when any modal overlay is open
        // This prevents the landing page from consuming keyboard input meant for modals
        let modal_open = native_promo_open
            || self.command_palette.is_open()
            || self.which_key.is_open()
            || self.plugins_overlay.is_open()
            || self.workspace_creator.is_open();
        self.landing_page.set_keyboard_disabled(modal_open);

        // Show the landing page in the central panel
        let mut landing_action = LandingPageAction::None;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            landing_action = self.landing_page.show(ui, ctx);
        });

        // Handle landing page actions
        // On mobile, block actions that navigate into the editor
        #[cfg(target_arch = "wasm32")]
        if self.is_mobile && landing_action != LandingPageAction::None {
            return WorkspaceAction::Notify {
                level: "info".to_string(),
                message: "Enya only works on Desktop".to_string(),
            };
        }

        match landing_action {
            LandingPageAction::OpenTutorial => {
                // Hide landing page and load the quick-start workspace
                self.show_landing = false;
                if let Ok(config) = WorkspaceConfig::from_toml(GOLDEN_SIGNALS_TOML) {
                    self.load_workspace_config(&config);
                    self.set_loaded_name(Some("quick-start".to_string()));
                }
                self.tutorial_overlay.open();
                ctx.request_repaint();
            }
            LandingPageAction::ShowAbout => {
                self.about_overlay.open();
            }
            LandingPageAction::ShowShortcuts => {
                self.which_key.open();
            }
            LandingPageAction::OpenPlugins => {
                self.plugins_overlay.open();
            }
            LandingPageAction::OpenSettings => {
                return WorkspaceAction::OpenSettings;
            }
            LandingPageAction::CreateProject => {
                return WorkspaceAction::CreateProject;
            }
            LandingPageAction::Dismiss => {
                self.show_landing = false;
                ctx.request_repaint();
            }
            LandingPageAction::ShowNativeAppInfo => {
                // Open the native app promo overlay (WASM only)
                #[cfg(target_arch = "wasm32")]
                self.native_promo_overlay.open_force();
            }
            LandingPageAction::None => {}
        }

        // Show time range picker modal
        self.time_range_picker.set_theme(self.theme());
        match self.time_range_picker.show(ctx) {
            TimeRangePickerResult::Selected {
                start_secs,
                end_secs,
            } => {
                self.time_range_toolbar
                    .set_custom_range(start_secs, end_secs);
                self.refresh_all_panes();
            }
            TimeRangePickerResult::Cancelled | TimeRangePickerResult::None => {}
        }

        // Show unified finder modal (Telescope-style)
        if let Some(action) = self.show_unified_finder(ctx, app_state) {
            return action;
        }

        // Show codebase finder modal (native only with Tantivy search)
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Update codebase status for the finder
            let status = match self.codebase_manager.status() {
                crate::codebase::CodebaseStatus::None => CodebaseFinderStatus::NoCodebase,
                crate::codebase::CodebaseStatus::Cloning { .. }
                | crate::codebase::CodebaseStatus::Fetching { .. }
                | crate::codebase::CodebaseStatus::Indexing { .. } => {
                    CodebaseFinderStatus::Indexing
                }
                crate::codebase::CodebaseStatus::Ready { metrics_count, .. } => {
                    CodebaseFinderStatus::Ready {
                        metric_count: *metrics_count,
                    }
                }
                crate::codebase::CodebaseStatus::Error { .. } => CodebaseFinderStatus::NoCodebase,
            };
            self.codebase_finder.set_status(status);

            // Update search results when query or filter changes
            let query = self.codebase_finder.query().to_string();
            let filter = self.codebase_finder.filter();
            if self.codebase_finder.is_open() && !query.is_empty() {
                let results = self.codebase_manager.search_ranked(&query, filter, 50);
                self.codebase_finder.set_results(results);
            }

            self.codebase_finder.set_theme(self.theme());
            if let Some(finder_result) = self.codebase_finder.show(ctx) {
                // Handle the selected result - navigate to source
                self.handle_codebase_finder_result(finder_result.result);
            }
        }

        // Show command palette modal
        self.command_palette.set_theme(self.theme());
        let cmd_result = self.command_palette.show(ctx);

        // Show info overlay modal
        self.info_overlay.set_theme(self.theme());
        self.info_overlay.show(ctx);

        // Show about overlay modal
        self.about_overlay.set_theme(self.theme());
        self.about_overlay.show(ctx);

        // Show which-key overlay modal
        self.which_key.set_theme(self.theme());
        self.which_key.show(ctx);

        // Note: Leader popup is NOT shown on landing page - Space+X commands
        // only make sense in the workspace view with open panes

        // Show tutorial overlay modal
        self.tutorial_overlay.set_theme(self.theme());
        self.tutorial_overlay.show(ctx);

        // Show plugins overlay modal
        self.plugins_overlay.set_theme(self.theme());
        match self.plugins_overlay.show(ctx) {
            PluginsOverlayResult::OpenPluginDirectory => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let plugin_dir = enya_config::plugins_dir();
                    if let Err(e) = open::that(&plugin_dir) {
                        log::warn!("Failed to open plugin directory: {e}");
                    }
                }
            }
            PluginsOverlayResult::TogglePlugin(name) => {
                log::debug!("Toggle plugin: {name}");
                // TODO: Implement plugin enable/disable via PluginRegistry
            }
            PluginsOverlayResult::InstallPlugin(name, file) => {
                log::debug!("Install plugin: {name} from {file}");
                // Set installing state to show spinner
                self.plugins_overlay
                    .set_installing_plugin(Some(name.clone()));
                self.pending_install_plugin = Some((name, file));
            }
            PluginsOverlayResult::RemovePlugin(name) => {
                log::debug!("Remove plugin: {name}");
                self.pending_remove_plugin = Some(name);
            }
            PluginsOverlayResult::RefreshAvailable => {
                log::debug!("Refresh available plugins");
                self.pending_refresh_plugins = true;
            }
            PluginsOverlayResult::Closed | PluginsOverlayResult::None => {}
        }

        // Show workspace creator overlay modal
        self.workspace_creator.set_theme(self.theme());
        match self.workspace_creator.show(ctx) {
            WorkspaceCreatorResult::Created {
                name,
                endpoint,
                git_repo,
                flight_sql_endpoint,
                project,
            } => {
                self.pending_connection_endpoint = Some(endpoint);
                // Store git repo path for codebase integration (native only with codebase feature)
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let new_url = git_repo.as_deref();
                    let current_url = self.codebase_manager.status().url();
                    if new_url != current_url {
                        self.codebase_manager.reset();
                    }
                    self.pending_git_config = git_repo;
                }
                #[cfg(target_arch = "wasm32")]
                let _ = git_repo; // Silence unused warning
                self.show_landing = false;
                ctx.request_repaint();
                // Return action to save the workspace and create/assign project
                return WorkspaceAction::SaveWorkspace {
                    name: Some(name),
                    project: Some(project),
                    flight_sql_endpoint,
                };
            }
            WorkspaceCreatorResult::Cancelled | WorkspaceCreatorResult::None => {}
        }

        // Show diagnostics overlay modal
        self.diagnostics_pane.set_theme(self.theme());
        self.diagnostics_pane.show_overlay(ctx);

        // Show annotation editor overlay modal
        self.annotation_editor.set_theme(self.theme());
        match self.annotation_editor.show(ctx) {
            AnnotationEditorResult::Created(annotation) => {
                // Add annotation to the focused pane's chart
                if let Some(focused_id) = self.behavior.focused_tile() {
                    if let Some(Tile::Pane(pane)) = self.viewport_tree.tiles.get_mut(focused_id) {
                        if let Some(query_pane) = pane.as_any_mut().downcast_mut::<QueryPane>() {
                            query_pane.add_annotation(annotation);
                            log::debug!("Added annotation to focused pane");
                        }
                    }
                }
            }
            AnnotationEditorResult::Updated(annotation) => {
                // Update annotation in the focused pane
                if let Some(focused_id) = self.behavior.focused_tile() {
                    if let Some(Tile::Pane(pane)) = self.viewport_tree.tiles.get_mut(focused_id) {
                        if let Some(query_pane) = pane.as_any_mut().downcast_mut::<QueryPane>() {
                            query_pane.update_annotation(annotation);
                            log::debug!("Updated annotation in focused pane");
                        }
                    }
                }
            }
            AnnotationEditorResult::Deleted(id) => {
                // Remove annotation from the focused pane
                if let Some(focused_id) = self.behavior.focused_tile() {
                    if let Some(Tile::Pane(pane)) = self.viewport_tree.tiles.get_mut(focused_id) {
                        if let Some(query_pane) = pane.as_any_mut().downcast_mut::<QueryPane>() {
                            query_pane.remove_annotation(id);
                            log::debug!("Removed annotation from focused pane");
                        }
                    }
                }
            }
            AnnotationEditorResult::Cancelled | AnnotationEditorResult::None => {}
        }

        // Note: No Space+X keyboard shortcuts on landing page - the UI already provides
        // clickable options for workspace finder and plugins. All Space+X shortcuts
        // are workspace-specific and handled in handle_viewport_keyboard().

        self.handle_command_result(cmd_result, ctx)
    }

    /// Handle a command result from the command palette
    fn handle_command_result(
        &mut self,
        result: CommandResult,
        ctx: &egui::Context,
    ) -> WorkspaceAction {
        match result {
            CommandResult::ShowInfo => {
                self.info_overlay.open();
                WorkspaceAction::None
            }
            CommandResult::SplitHorizontal => {
                self.split_panes_horizontal();
                WorkspaceAction::None
            }
            CommandResult::SplitVertical => {
                self.split_panes_vertical();
                WorkspaceAction::None
            }
            CommandResult::QuitWorkspace => WorkspaceAction::QuitApp,
            CommandResult::WriteWorkspace => WorkspaceAction::SaveWorkspace {
                name: None,
                project: None,
                flight_sql_endpoint: None,
            },
            CommandResult::TakeScreenshot(path) => WorkspaceAction::TakeScreenshot(path),
            CommandResult::ShareWorkspace => WorkspaceAction::ShareWorkspace,
            CommandResult::UploadSnapshot(title) => WorkspaceAction::UploadSnapshot(title),
            CommandResult::OpenPrReview(repo_arg) => {
                // Try explicit owner/repo arg first, then fall back to git remote
                let owner_repo = repo_arg.as_deref().and_then(|arg| {
                    arg.split_once('/')
                        .map(|(o, r)| (o.to_string(), r.to_string()))
                });

                #[cfg(not(target_arch = "wasm32"))]
                let owner_repo = owner_repo.or_else(|| {
                    self.codebase_manager
                        .status()
                        .url()
                        .and_then(crate::github_api::parse_owner_repo)
                });

                if let Some((owner, repo)) = owner_repo {
                    #[cfg(not(target_arch = "wasm32"))]
                    let token = self
                        .git_credential_token
                        .clone()
                        .or(self.github_token.clone());
                    #[cfg(target_arch = "wasm32")]
                    let token = self.github_token.clone();
                    self.add_pr_review_pane(&owner, &repo, token);
                } else {
                    log::warn!("No repo specified and no git remote available for PR review");
                }
                WorkspaceAction::None
            }
            CommandResult::OpenLogs => {
                // Use a default time range of the last hour for the logs pane
                let now_ns = crate::util::now_unix_secs() * 1_000_000_000;
                let one_hour_ns = 3600 * 1_000_000_000;
                self.add_logs_pane(now_ns - one_hour_ns, now_ns);
                WorkspaceAction::None
            }
            CommandResult::OpenTerminal => {
                self.add_terminal_pane();
                WorkspaceAction::None
            }
            CommandResult::OpenTracing(trace_id) => {
                self.add_tracing_pane(trace_id.as_deref());
                WorkspaceAction::None
            }
            CommandResult::OpenSql => {
                self.add_sql_pane();
                WorkspaceAction::None
            }
            CommandResult::FloatPane => {
                self.float_focused_pane(None);
                WorkspaceAction::None
            }
            CommandResult::DockAllPanes => {
                self.dock_all_floating_panes();
                WorkspaceAction::None
            }
            CommandResult::ArrangeFloatingPanes => {
                // Use the available rect as viewport for arrange
                let viewport = ctx.available_rect();
                self.floating_panes.arrange_panes(viewport);
                WorkspaceAction::None
            }
            CommandResult::SyncCodebase => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.codebase_manager.fetch_updates(ctx);
                    log::debug!("Triggered repository sync and re-indexing via :sync command");
                }
                WorkspaceAction::None
            }
            CommandResult::OpenTutorial => {
                self.tutorial_overlay.open();
                WorkspaceAction::None
            }
            CommandResult::OpenSettings => {
                self.pending_open_settings = true;
                WorkspaceAction::None
            }
            CommandResult::AddMetric(name) => {
                if let Some(metric) = name {
                    self.add_query_pane(&metric, None);
                } else {
                    // Open an empty buffer editor for the user to type a metric name
                    self.buffer_editor.open("", "");
                }
                WorkspaceAction::None
            }
            CommandResult::PluginCommand(command, args) => {
                WorkspaceAction::PluginCommand { command, args }
            }
            CommandResult::Success | CommandResult::Error(_) | CommandResult::None => {
                WorkspaceAction::None
            }
        }
    }

    /// Enter edit mode on the focused buffer - opens the modal editor
    fn edit_focused_buffer(&mut self) {
        if let Some(tile_id) = self.behavior.focused_tile() {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                // Try to downcast to QueryPane and get query info
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    let query = query_pane.saved_query().to_string();
                    let name = query_pane.name().to_string();
                    let state = query_pane.query_state().clone();
                    self.buffer_editor.open_with_state(&query, &name, state);
                    self.editing_tile_id = Some(tile_id);

                    // Populate completions from cached metric labels
                    if let Some(labels) = self.query_executor.get_metric_labels(&name) {
                        self.buffer_editor
                            .set_completions_from_labels(&labels.labels);
                        log::debug!(
                            "Set buffer editor completions from {} labels for '{}'",
                            labels.labels.len(),
                            name
                        );
                    } else if self.query_executor.is_connected() {
                        // Clear default completions if connected but no labels cached
                        self.buffer_editor.clear_completions();
                    }

                    // Set known metric names for completion
                    let metric_names = self.query_executor.metric_names().to_vec();
                    log::debug!(
                        "Setting {} metric names for completion: {:?}",
                        metric_names.len(),
                        metric_names.iter().take(5).collect::<Vec<_>>()
                    );
                    self.buffer_editor.set_metric_names(metric_names);

                    log::debug!("Opening buffer editor for QueryPane");
                } else if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                    let query = buffer.saved_content().to_string();
                    let name = buffer.name().to_string();
                    self.buffer_editor.open(&query, &name);
                    self.editing_tile_id = Some(tile_id);

                    // Set known metric names for completion
                    let metric_names = self.query_executor.metric_names().to_vec();
                    self.buffer_editor.set_metric_names(metric_names);

                    log::debug!("Opening buffer editor for Buffer");
                } else if let Some(logs_pane) = component.as_any().downcast_ref::<LogsPane>() {
                    // LogsPane uses modal BufferEditor like QueryPane, with LogQL completion mode
                    let query = logs_pane.saved_query().to_string();
                    let name = logs_pane.name().to_string();
                    // Use LogQL completion mode for LogsPane
                    self.buffer_editor
                        .open_with_language(&query, &name, QueryLanguage::LogQL);
                    self.editing_tile_id = Some(tile_id);

                    log::debug!("Opening buffer editor for LogsPane");
                }
            }
        }
    }

    /// Cycle the visualization type for the focused pane (time series -> stat -> ...)
    fn cycle_focused_visualization(&mut self) {
        if let Some(tile_id) = self.behavior.focused_tile() {
            if let Some(egui_tiles::Tile::Pane(component)) =
                self.viewport_tree.tiles.get_mut(tile_id)
            {
                // Only QueryPane supports multiple visualization types
                if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                    query_pane.cycle_visualization();
                    log::debug!(
                        "Cycled visualization to {:?}",
                        query_pane.visualization_type()
                    );
                }
            }
        }
    }

    /// Apply the result from the buffer editor modal
    fn apply_buffer_editor_result(&mut self, query: String, query_state: QueryState) {
        if let Some(tile_id) = self.editing_tile_id.take() {
            if let Some(egui_tiles::Tile::Pane(component)) =
                self.viewport_tree.tiles.get_mut(tile_id)
            {
                // Try to downcast to QueryPane and apply
                if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                    query_pane.set_query_state_and_save(&query, query_state.clone());
                    log::debug!("Applied query to QueryPane: {query}");
                } else if let Some(buffer) = component.as_any_mut().downcast_mut::<Buffer>() {
                    buffer.set_content(&query);
                    buffer.save();
                    log::debug!("Applied query to Buffer: {query}");
                } else if let Some(logs_pane) = component.as_any_mut().downcast_mut::<LogsPane>() {
                    logs_pane.set_query(&query);
                    log::debug!("Applied query to LogsPane: {query}");
                }
            }
        } else if !query.is_empty() {
            // No existing tile — create a new query pane (e.g., from :new or :metric)
            self.add_query_pane(&query, None);
            log::debug!("Created new query pane: {query}");
        }
    }

    /// Open the command palette modal
    pub fn open_command_palette(&mut self) {
        self.command_palette.open();
    }

    /// Open the command palette with pre-filled text
    pub fn open_command_palette_with_text(&mut self, text: &str) {
        self.command_palette.open_with_text(text);
    }

    /// Set plugin commands for the command palette
    pub fn set_plugin_commands(&mut self, commands: Vec<crate::components::DynamicCommand>) {
        self.command_palette.set_plugin_commands(commands);
    }

    /// Set plugins info for the plugins overlay
    pub fn set_plugins(&mut self, plugins: Vec<crate::components::PluginDisplayInfo>) {
        self.plugins_overlay.set_plugins(plugins);
    }

    /// Set available community plugins for the plugins overlay
    pub fn set_available_plugins(
        &mut self,
        plugins: Vec<crate::components::overlay::plugins::CommunityPluginInfo>,
    ) {
        self.plugins_overlay.set_available_plugins(plugins);
    }

    /// Set loading state for the plugins overlay
    pub fn set_plugins_loading(&mut self, loading: bool) {
        self.plugins_overlay.set_loading_available(loading);
    }

    /// Set the plugin currently being installed
    pub fn set_installing_plugin(&mut self, name: Option<String>) {
        self.plugins_overlay.set_installing_plugin(name);
    }

    /// Toggle zen mode (distraction-free view)
    pub fn toggle_zen_mode(&mut self) {
        self.zen_mode = !self.zen_mode;
        log::debug!("Zen mode: {}", self.zen_mode);
    }

    /// Check if zen mode is active
    pub fn is_zen_mode(&self) -> bool {
        self.zen_mode
    }

    /// Toggle fullscreen for the currently focused pane
    pub fn toggle_fullscreen(&mut self) {
        if self.fullscreen_tile.is_some() {
            // Exit fullscreen
            self.fullscreen_tile = None;
            log::debug!("Exited fullscreen mode");
        } else if let Some(focused_id) = self.behavior.focused_tile() {
            // Enter fullscreen for focused pane
            // Verify it's actually a pane (not a container)
            if matches!(
                self.viewport_tree.tiles.get(focused_id),
                Some(Tile::Pane(_))
            ) {
                self.fullscreen_tile = Some(focused_id);
                log::debug!("Entered fullscreen mode for tile {focused_id:?}");
            }
        } else {
            // No pane focused - try to fullscreen the first available pane
            let pane_ids = self.get_pane_tile_ids();
            if let Some(&first_pane) = pane_ids.first() {
                self.fullscreen_tile = Some(first_pane);
                self.behavior.set_focused_tile(Some(first_pane));
                log::debug!("Entered fullscreen mode for first pane {first_pane:?}");
            }
        }
    }

    /// Check if fullscreen mode is active
    pub fn is_fullscreen(&self) -> bool {
        self.fullscreen_tile.is_some()
    }

    // =========================================================================
    /// Open annotation editor for the focused pane.
    /// Uses current time as the default annotation point.
    pub fn open_annotation_editor(&mut self) {
        // Get current time as default timestamp
        let timestamp = crate::util::now_unix_secs() as f64;
        self.annotation_editor.open_new(timestamp, Some("You"));
    }

    /// Check if the viewport filter input is open
    pub fn is_viewport_filter_open(&self) -> bool {
        self.viewport_filter.is_open()
    }

    /// Check if the viewport filter is active (has an applied pattern)
    pub fn is_viewport_filter_active(&self) -> bool {
        self.viewport_filter.is_active()
    }

    /// Check if the landing page is currently being displayed
    pub fn is_landing_page(&self) -> bool {
        self.show_landing && self.open_charts.is_empty()
    }

    /// Detect mobile browsers via user-agent string or narrow viewport.
    #[cfg(target_arch = "wasm32")]
    fn detect_mobile() -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };

        let has_mobile_ua = window
            .navigator()
            .user_agent()
            .ok()
            .map(|ua| {
                let ua = ua.to_lowercase();
                ua.contains("mobi") || ua.contains("android")
            })
            .unwrap_or(false);

        let is_narrow = window
            .inner_width()
            .ok()
            .and_then(|w| w.as_f64())
            .map(|w| w < 768.0)
            .unwrap_or(false);

        has_mobile_ua || is_narrow
    }

    /// Check if the command palette is currently open
    pub fn is_command_palette_open(&self) -> bool {
        self.command_palette.is_open()
    }

    /// Check if the unified finder is currently open
    pub fn is_unified_finder_open(&self) -> bool {
        self.unified_finder.is_open()
    }

    /// Check if multi-buffer is in input mode (capturing text input)
    pub fn is_multi_buffer_input_mode(&self) -> bool {
        matches!(
            self.multi_buffer_state.mode,
            MultiBufferMode::PatternInput | MultiBufferMode::Editing
        )
    }

    /// Get the number of open tabs/charts
    pub fn open_tabs_count(&self) -> usize {
        self.open_charts.len()
    }

    /// Get the currently selected metric name
    pub fn selected_metric(&self) -> Option<String> {
        // No longer tracking selection (metrics tree removed)
        None
    }

    /// Get viewport info (e.g., pane layout description)
    pub fn viewport_info(&self) -> Option<String> {
        let pane_count = self.get_pane_tile_ids().len();
        if pane_count > 1 {
            Some(format!("{pane_count} panes"))
        } else {
            None
        }
    }

    /// Check if the connection is validated and online.
    pub fn is_online(&self) -> bool {
        self.query_executor.is_online()
    }

    // =========================================================================
    // Visual Multi-Select Mode (public API)
    // =========================================================================

    /// Check if we're in visual-multi mode
    pub fn is_visual_multi_mode(&self) -> bool {
        self.visual_multi_state.is_some()
    }

    /// Get the number of selected panes in visual-multi mode
    pub fn visual_multi_selection_count(&self) -> usize {
        self.visual_multi_state
            .as_ref()
            .map(|s| s.selection_count())
            .unwrap_or(0)
    }

    /// Get the multi-buffer status text for display in status line
    pub fn multi_buffer_status_text(&self) -> String {
        if self.multi_buffer_state.is_active() {
            self.multi_buffer_state.status_text()
        } else if let Some(state) = &self.visual_multi_state {
            format!(
                "VISUAL-MULTI ({} selected) [e]dit [r]efresh [x]close [Space]toggle [Esc]exit",
                state.selection_count()
            )
        } else if self.viewport_filter.is_active() {
            let (match_count, total_count) = self.count_filtered_panes();
            format!(
                "FILTER: /{} ({}/{} panes) [/]edit [Esc]clear",
                self.viewport_filter.applied_pattern(),
                match_count,
                total_count
            )
        } else {
            String::new()
        }
    }

    /// Check if multi-edit overlay is open
    pub fn is_multi_edit_open(&self) -> bool {
        self.multi_edit_overlay.is_open()
    }

    /// Apply changes from multi-edit back to source panes
    fn apply_multi_edit_changes(&mut self, changes: Vec<(usize, String)>) {
        for (source_id, new_content) in changes {
            // Find the pane with this source_id and update its content
            for tile_id in self.get_pane_tile_ids() {
                if let Some(egui_tiles::Tile::Pane(component)) =
                    self.viewport_tree.tiles.get_mut(tile_id)
                {
                    // Try QueryPane
                    if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                        if query_pane.id() == source_id {
                            query_pane.set_query_and_save(&new_content);
                            log::debug!(
                                "Applied multi-edit change to QueryPane {} ({})",
                                source_id,
                                query_pane.name()
                            );
                            break;
                        }
                    }
                    // Try Buffer
                    else if let Some(buffer) = component.as_any_mut().downcast_mut::<Buffer>() {
                        if buffer.id() == source_id {
                            buffer.set_content(&new_content);
                            buffer.save();
                            log::debug!(
                                "Applied multi-edit change to Buffer {} ({})",
                                source_id,
                                buffer.name()
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    // =========================================================================
    // Agent Mode (AI-Assisted Interaction)
    // =========================================================================

    /// Check if agent mode is active
    pub fn is_agent_mode(&self) -> bool {
        self.agent_mode_active
    }

    /// Check if this workspace was loaded from an immutable snapshot
    pub fn is_snapshot(&self) -> bool {
        self.is_snapshot
    }

    /// Get the snapshot title (if loaded from a named blob snapshot)
    pub fn snapshot_title(&self) -> Option<String> {
        self.snapshot_title.clone()
    }

    /// Get the current agent provider name (e.g., "Claude", "Codex")
    pub fn agent_provider_name(&self) -> String {
        self.agent_panel.provider_name()
    }

    /// Sets the git auto-sync interval on the codebase manager.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_git_sync_interval(&mut self, seconds: u64) {
        self.codebase_manager.set_git_sync_interval(seconds);
    }

    /// Update the agent panel's and input bar's provider and model from settings.
    pub fn set_agent_provider_and_model(
        &mut self,
        provider: crate::components::util::AiProvider,
        model: Option<String>,
    ) {
        self.agent_panel
            .set_provider_and_model(provider, model.clone());
        #[cfg(not(target_arch = "wasm32"))]
        self.agent_input_bar.set_provider_and_model(provider, model);
    }

    /// Send a query to the agent (public wrapper for inline input)
    pub fn send_agent_query(&mut self, query: &str, ctx: &egui::Context) {
        self.send_agent_query_with_context(query, ctx);
    }

    /// Enter agent mode, optionally with panes from visual selection.
    /// Shows quick command hints in the input bar.
    pub fn enter_agent_mode(&mut self) {
        self.enter_agent_mode_impl(false);
    }

    /// Enter agent mode and go directly to typing mode (no quick command hints).
    /// Used when entering via `aa` for freeform input.
    pub fn enter_agent_mode_typing(&mut self) {
        self.enter_agent_mode_impl(true);
    }

    /// Internal implementation of enter_agent_mode
    fn enter_agent_mode_impl(&mut self, start_typing: bool) {
        // Transfer visual selection to agent context if in visual mode
        if let Some(visual_state) = &self.visual_multi_state {
            self.agent_context_panes = visual_state.selected_tile_ids.clone();
        }

        // Exit visual mode if active
        self.visual_multi_state = None;

        // Enter agent mode
        self.agent_mode_active = true;

        if start_typing {
            self.agent_input_bar.reset_to_typing();
        } else {
            self.agent_input_bar.reset();
        }

        // Build context pane info
        self.sync_agent_context_panes();

        log::debug!(
            "Entered agent mode with {} context panes (typing={})",
            self.agent_context_panes.len(),
            start_typing
        );
    }

    /// Enter agent mode and immediately execute a quick command.
    /// This is used for vim-style operator patterns like `aw`, `ae`, etc.
    pub fn enter_agent_mode_with_command(&mut self, command: QuickCommand) {
        use crate::components::overlay::format_pane_context;

        // Enter agent mode first
        self.enter_agent_mode();

        // Build context from selected panes (or focused pane if none selected)
        if self.agent_context_panes.is_empty() {
            if let Some(tile_id) = self.behavior.focused_tile() {
                self.agent_context_panes.insert(tile_id);
                self.sync_agent_context_panes();
            }
        }

        // Build rich context from selected panes with data summaries
        let context_blocks: Vec<String> = self
            .agent_context_panes
            .iter()
            .filter_map(|&tile_id| {
                let (info, query_text) = self.collect_pane_info_for_tile(tile_id)?;
                Some(format_pane_context(&info.name, &query_text, &info))
            })
            .collect();

        let context = if context_blocks.is_empty() {
            None
        } else {
            Some(format!("## Context Panes\n\n{}", context_blocks.join("\n")))
        };

        // Send the quick command query
        self.agent_input_bar
            .send_query(command.prompt(), context.as_deref());

        log::debug!(
            "Agent operator command: {:?} with {} context panes",
            command,
            self.agent_context_panes.len()
        );
    }

    /// Exit agent mode
    pub fn exit_agent_mode(&mut self) {
        self.agent_mode_active = false;
        self.agent_context_panes.clear();
        log::debug!("Exited agent mode");
    }

    /// Sync agent context panes with the input bar display
    fn sync_agent_context_panes(&mut self) {
        let context_panes: Vec<ContextPane> = self
            .agent_context_panes
            .iter()
            .filter_map(|&tile_id| {
                if let Some(egui_tiles::Tile::Pane(component)) =
                    self.viewport_tree.tiles.get(tile_id)
                {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        return Some(ContextPane {
                            tile_id,
                            name: query_pane.name().to_string(),
                        });
                    }
                }
                None
            })
            .collect();

        self.agent_input_bar.set_context_panes(context_panes);
    }

    /// Add the focused pane to agent context
    pub fn add_focused_to_agent_context(&mut self) {
        if let Some(tile_id) = self.behavior.focused_tile() {
            self.agent_context_panes.insert(tile_id);
            self.sync_agent_context_panes();
        }
    }

    /// Remove the focused pane from agent context
    pub fn remove_focused_from_agent_context(&mut self) {
        if let Some(tile_id) = self.behavior.focused_tile() {
            self.agent_context_panes.remove(&tile_id);
            self.sync_agent_context_panes();
        }
    }

    /// Clear all agent context panes
    pub fn clear_agent_context(&mut self) {
        self.agent_context_panes.clear();
        self.agent_input_bar.clear_context();
    }

    /// Get the number of panes in agent context
    pub fn agent_context_count(&self) -> usize {
        self.agent_context_panes.len()
    }

    /// Show the agent input bar and handle its results.
    /// Called from the app's bottom panel to render above the status line.
    pub fn show_agent_input_bar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        theme: AppTheme,
    ) {
        if !self.agent_mode_active {
            return;
        }

        self.agent_input_bar.set_theme(theme);
        self.agent_input_bar
            .set_provider_name(self.agent_panel.provider().display_name());

        // Provide available metrics for @ mention autocomplete
        let metric_names = self.query_executor.metric_names().to_vec();
        self.agent_input_bar.set_available_metrics(metric_names);

        // Estimate context size for token usage indicator
        let pane_count = if self.agent_context_panes.is_empty() {
            if self.behavior.focused_tile().is_some() {
                1
            } else {
                0
            }
        } else {
            self.agent_context_panes.len()
        };
        let estimated_context = 2000 + (pane_count * 500);
        self.agent_input_bar
            .set_context_char_count(estimated_context);

        let result = self.agent_input_bar.show(ui);
        self.handle_agent_input_result(result, ctx);
    }

    /// Show the agent input bar in inline mode (for status line embedding)
    /// Called from the app's bottom panel to render within the status line.
    pub fn show_agent_input_bar_inline(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        theme: AppTheme,
    ) {
        if !self.agent_mode_active {
            return;
        }

        self.agent_input_bar.set_theme(theme);
        self.agent_input_bar
            .set_provider_name(self.agent_panel.provider().display_name());

        // Provide available metrics for @ mention autocomplete
        let metric_names = self.query_executor.metric_names().to_vec();
        self.agent_input_bar.set_available_metrics(metric_names);

        // Estimate context size for token usage indicator
        // Base: ~2000 chars for editor context (commands, workspace info)
        // Per pane: ~500 chars average (varies with data)
        let pane_count = if self.agent_context_panes.is_empty() {
            if self.behavior.focused_tile().is_some() {
                1
            } else {
                0
            }
        } else {
            self.agent_context_panes.len()
        };
        let estimated_context = 2000 + (pane_count * 500);
        self.agent_input_bar
            .set_context_char_count(estimated_context);

        let result = self.agent_input_bar.show_inline(ui);
        self.handle_agent_input_result(result, ctx);
    }

    /// Show the viewport filter bar and handle its results.
    /// Called from the app's bottom panel to render above the status line.
    pub fn show_viewport_filter_bar(&mut self, ui: &mut egui::Ui) {
        match self.viewport_filter.show(ui) {
            ViewportFilterResult::Applied(pattern) => {
                log::debug!("Workspace filter applied: {pattern}");
            }
            ViewportFilterResult::Cleared => {
                log::debug!("Workspace filter cleared");
            }
            ViewportFilterResult::None => {}
        }
    }

    /// Handle results from the agent input bar
    fn handle_agent_input_result(&mut self, result: AgentInputBarResult, ctx: &egui::Context) {
        // Handle exit request (Escape key)
        if result.exit_requested {
            self.exit_agent_mode();
            ctx.request_repaint();
            return;
        }

        // Handle Tab to open in agent panel (side panel handoff)
        if result.open_in_pane {
            if let Some(handoff) = self.agent_input_bar.export_for_handoff() {
                log::debug!("Handing off conversation to agent panel");
                self.agent_panel.import_from_handoff(handoff);
                self.exit_agent_mode();
                ctx.request_repaint();
                return;
            }
        }

        // Handle context operations
        if result.add_pane_to_context {
            self.add_focused_to_agent_context();
        }
        if result.remove_pane_from_context {
            self.remove_focused_from_agent_context();
        }
        if result.clear_context {
            self.clear_agent_context();
        }

        // Handle undo - for now just log, will implement with agent commands
        if result.undo_requested {
            log::debug!("Agent undo requested");
        }

        // Handle quick commands
        if let Some(quick_cmd) = result.quick_command {
            log::debug!("Agent quick command: {quick_cmd:?}");
            self.send_agent_query_with_context(quick_cmd.prompt(), ctx);
        }

        // Handle natural language query
        if let Some(query) = result.query {
            log::debug!("Agent query: {query}");
            self.send_agent_query_with_context(&query, ctx);
        }

        // Handle Enya commands from AI response (e.g., create_pane, set_time_range)
        // Convert inline commands to create_pane since Agent Input Bar doesn't support inline rendering
        if !result.commands.is_empty() {
            use crate::components::AgentCommand;

            let converted_commands: Vec<AgentCommand> = result
                .commands
                .into_iter()
                .map(|cmd| match cmd {
                    // Convert ShowInlineChart to CreatePane (Agent Input Bar doesn't render inline)
                    AgentCommand::ShowInlineChart {
                        query,
                        title,
                        time_range: _,
                        height: _,
                    } => {
                        log::debug!("Converting ShowInlineChart to CreatePane for Agent Input Bar");
                        AgentCommand::CreatePane {
                            query,
                            title,
                            floating: None,
                            position: None,
                        }
                    }
                    // Pass through all other commands unchanged
                    other => other,
                })
                .collect();

            log::debug!(
                "Executing {} enya command(s) from agent input bar",
                converted_commands.len()
            );
            let activities = self.handle_agent_commands(converted_commands, ctx);
            // Add activities to input bar for display
            for activity in &activities {
                self.agent_input_bar.add_activity(activity.clone());
            }
            // Only auto-exit if commands were executed AND there's no response text
            let has_response_text = !self.agent_input_bar.display_text().is_empty();
            if !activities.is_empty() && !has_response_text {
                log::debug!("Agent command executed (no response text), exiting agent mode");
                self.exit_agent_mode();
            }
        }
    }

    /// Send a query to the agent with current context panes
    fn send_agent_query_with_context(&mut self, query: &str, ctx: &egui::Context) {
        use crate::components::overlay::format_pane_context;

        // Build full editor context (includes available commands documentation)
        let editor_context = self.build_editor_context();
        let editor_context_block = editor_context
            .as_ref()
            .map(|c| c.to_prompt_block())
            .unwrap_or_default();

        // Determine which panes to include: explicit selection, or fallback to focused pane
        let pane_ids: Vec<egui_tiles::TileId> = if self.agent_context_panes.is_empty() {
            // Auto-include focused pane so "explain this spike" works naturally
            self.behavior.focused_tile().into_iter().collect()
        } else {
            self.agent_context_panes.iter().copied().collect()
        };

        // Build rich context from selected panes with data summaries
        let context_blocks: Vec<String> = pane_ids
            .iter()
            .filter_map(|&tile_id| {
                let (info, query_text) = self.collect_pane_info_for_tile(tile_id)?;
                Some(format_pane_context(&info.name, &query_text, &info))
            })
            .collect();

        // Build full context string with editor context and pane context
        let context = {
            let mut parts = vec![editor_context_block];
            if !context_blocks.is_empty() {
                let header = if self.agent_context_panes.is_empty() {
                    "\n## Focused Pane\n"
                } else {
                    "\n## Selected Panes\n"
                };
                parts.push(header.to_string());
                parts.push(context_blocks.join("\n"));
            }
            Some(parts.join("\n"))
        };

        // Send query directly to input bar (it handles AI communication)
        log::debug!(
            "Sending query to agent: '{}' with context ({} chars, {} panes)",
            query,
            context.as_ref().map(|c| c.len()).unwrap_or(0),
            pane_ids.len()
        );
        self.agent_input_bar.send_query(query, context.as_deref());

        ctx.request_repaint();
    }

    // =========================================================================
    // Codebase Integration (Go to Definition)
    // =========================================================================

    /// Open the source preview overlay for a metric definition.
    ///
    /// Looks up the metric in the codebase index and shows the source file
    /// context around the instrumentation point.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_metric_definition(&mut self, metric_name: &str) {
        use crate::codebase::CodebaseStatus;

        // Check if codebase is ready
        if !self.codebase_manager.status().is_ready() {
            let status_msg = match self.codebase_manager.status() {
                CodebaseStatus::None => "No codebase configured",
                CodebaseStatus::Cloning { .. } => "Codebase is being cloned...",
                CodebaseStatus::Fetching { .. } => "Fetching updates...",
                CodebaseStatus::Indexing { .. } => "Indexing codebase...",
                CodebaseStatus::Ready { .. } => unreachable!(),
                CodebaseStatus::Error { message, .. } => message,
            };
            self.source_preview.open_error(metric_name, status_msg);
            return;
        }

        // Look up the metric in the index
        let Some(index) = self.codebase_manager.index() else {
            self.source_preview
                .open_error(metric_name, "Codebase index not available");
            return;
        };

        let matches = index.find_by_name(metric_name);
        if matches.is_empty() {
            self.source_preview.open_error(
                metric_name,
                &format!("Metric '{metric_name}' not found in codebase"),
            );
            return;
        }

        // Pass all matches to allow cycling with N/P keys
        let locations: Vec<_> = matches.into_iter().cloned().collect();
        log::debug!(
            "Opening source preview for '{}' with {} location(s)",
            metric_name,
            locations.len()
        );
        self.source_preview
            .open_metric_with_locations(locations, &index.repo_path);
    }

    /// WASM stub for open_metric_definition - shows not available message.
    #[cfg(target_arch = "wasm32")]
    pub fn open_metric_definition(&mut self, metric_name: &str) {
        self.source_preview
            .open_error(metric_name, "Go to definition is not available in browser");
    }

    /// Open the source preview overlay for an alert that references a metric.
    ///
    /// Looks up alerts in the codebase index that reference the given metric name
    /// and shows the source file context around the alert definition.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_alert_for_metric(&mut self, metric_name: &str) {
        use crate::codebase::CodebaseStatus;

        // Check if codebase is ready
        if !self.codebase_manager.status().is_ready() {
            let status_msg = match self.codebase_manager.status() {
                CodebaseStatus::None => "No codebase configured",
                CodebaseStatus::Cloning { .. } => "Codebase is being cloned...",
                CodebaseStatus::Fetching { .. } => "Fetching updates...",
                CodebaseStatus::Indexing { .. } => "Indexing codebase...",
                CodebaseStatus::Ready { .. } => unreachable!(),
                CodebaseStatus::Error { message, .. } => message,
            };
            self.source_preview.open_error(metric_name, status_msg);
            return;
        }

        // Look up alerts in the index
        let Some(index) = self.codebase_manager.index() else {
            self.source_preview
                .open_error(metric_name, "Codebase index not available");
            return;
        };

        let matches = index.find_alerts_by_metric(metric_name);
        if matches.is_empty() {
            self.source_preview.open_error(
                metric_name,
                &format!("No alerts found for metric '{metric_name}'"),
            );
            return;
        }

        // Use the first match (TODO: show picker if multiple)
        let alert = matches[0];
        self.source_preview.open_alert(alert, &index.repo_path);
        log::debug!(
            "Opening alert preview for '{}' at {}:{}",
            alert.name,
            alert.file.display(),
            alert.line
        );
    }

    /// WASM stub for open_alert_for_metric - shows not available message.
    #[cfg(target_arch = "wasm32")]
    pub fn open_alert_for_metric(&mut self, metric_name: &str) {
        self.source_preview
            .open_error(metric_name, "Go to alert is not available in browser");
    }

    /// Open the source preview for an alert rule by its name.
    ///
    /// Looks up the alert by name in the codebase index and shows the
    /// source file context around the alert definition.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_alert_definition(&mut self, alert_name: &str) {
        use crate::codebase::CodebaseStatus;

        // Check if codebase is ready
        if !self.codebase_manager.status().is_ready() {
            let status_msg = match self.codebase_manager.status() {
                CodebaseStatus::None => "No codebase configured",
                CodebaseStatus::Cloning { .. } => "Codebase is being cloned...",
                CodebaseStatus::Fetching { .. } => "Fetching updates...",
                CodebaseStatus::Indexing { .. } => "Indexing codebase...",
                CodebaseStatus::Ready { .. } => unreachable!(),
                CodebaseStatus::Error { message, .. } => message,
            };
            self.source_preview.open_error(alert_name, status_msg);
            return;
        }

        // Look up alert in the index by name
        let Some(index) = self.codebase_manager.index() else {
            self.source_preview
                .open_error(alert_name, "Codebase index not available");
            return;
        };

        let Some(alert) = index.find_alert_by_name(alert_name) else {
            self.source_preview.open_error(
                alert_name,
                &format!("Alert '{alert_name}' not found in codebase"),
            );
            return;
        };

        self.source_preview.open_alert(alert, &index.repo_path);
        log::debug!(
            "Opening alert preview for '{}' at {}:{}",
            alert.name,
            alert.file.display(),
            alert.line
        );
    }

    /// WASM stub for open_alert_definition - shows not available message.
    #[cfg(target_arch = "wasm32")]
    pub fn open_alert_definition(&mut self, alert_name: &str) {
        self.source_preview
            .open_error(alert_name, "Go to alert is not available in browser");
    }

    /// Get the metric name from the currently focused pane (if it's a QueryPane).
    ///
    /// Parses the saved PromQL query to extract the primary metric name.
    pub fn get_focused_metric_name(&self) -> Option<String> {
        let tile_id = self.behavior.focused_tile()?;
        let tile = self.viewport_tree.tiles.get(tile_id)?;

        if let egui_tiles::Tile::Pane(component) = tile {
            if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                // Extract metric name from the saved PromQL query
                let query = query_pane.saved_query();
                return enya_promql::extract_metric_name(query);
            }
        }
        None
    }

    /// Get information about the currently focused pane.
    ///
    /// Returns `FocusedPaneInfo` with type, title, query, and metric name (if applicable).
    /// This is used by plugins to share context to external services like Slack/Discord.
    pub fn get_focused_pane_info(&self) -> Option<FocusedPaneInfo> {
        let tile_id = self.behavior.focused_tile()?;
        let tile = self.viewport_tree.tiles.get(tile_id)?;

        if let egui_tiles::Tile::Pane(component) = tile {
            // Check each pane type
            if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                let query = query_pane.saved_query().to_string();
                let metric_name = enya_promql::extract_metric_name(&query);
                let mut info = FocusedPaneInfo::new("query").with_query(query.clone());
                // Use metric name as title if available, otherwise use truncated query
                if let Some(ref metric) = metric_name {
                    info = info
                        .with_title(metric.clone())
                        .with_metric_name(metric.clone());
                } else if !query.is_empty() {
                    // Use first 50 chars of query as title
                    let title = crate::components::util::text_formatting::truncate_with_ellipsis(
                        &query, 50,
                    );
                    info = info.with_title(title);
                }
                return Some(info);
            }

            if component.as_any().downcast_ref::<LogsPane>().is_some() {
                return Some(FocusedPaneInfo::new("logs").with_title("Logs"));
            }

            if component.as_any().downcast_ref::<TracingPane>().is_some() {
                return Some(FocusedPaneInfo::new("tracing").with_title("Tracing"));
            }

            if component.as_any().downcast_ref::<SqlPane>().is_some() {
                return Some(FocusedPaneInfo::new("sql").with_title("SQL"));
            }

            // Plugin pane types - use Component::name() which returns the title
            if let Some(table_pane) = component.as_any().downcast_ref::<PluginTablePane>() {
                return Some(
                    FocusedPaneInfo::new("custom_table").with_title(Component::name(table_pane)),
                );
            }

            if let Some(chart_pane) = component.as_any().downcast_ref::<PluginChartPane>() {
                return Some(
                    FocusedPaneInfo::new("custom_chart").with_title(Component::name(chart_pane)),
                );
            }

            if let Some(stat_pane) = component.as_any().downcast_ref::<PluginStatPane>() {
                return Some(
                    FocusedPaneInfo::new("custom_stat").with_title(Component::name(stat_pane)),
                );
            }

            if let Some(gauge_pane) = component.as_any().downcast_ref::<PluginGaugePane>() {
                return Some(
                    FocusedPaneInfo::new("custom_gauge").with_title(Component::name(gauge_pane)),
                );
            }

            // Unknown pane type
            return Some(FocusedPaneInfo::new("unknown"));
        }

        None
    }

    /// Check if source preview overlay is open
    pub fn is_source_preview_open(&self) -> bool {
        self.source_preview.is_open()
    }

    /// Open the source preview with demo data (for testing/showcase).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_source_preview_demo(&mut self) {
        self.source_preview.open_demo();
    }

    /// Get the current codebase status for StatusLine display.
    ///
    /// Returns None if no codebase operation is active, or a `CodebaseStatusInfo`
    /// with details about the current operation (cloning, indexing, ready, error).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn codebase_status_info(
        &self,
    ) -> Option<crate::components::widget::status_line::CodebaseStatusInfo> {
        use crate::codebase::CodebaseStatus;
        use crate::components::widget::status_line::CodebaseStatusInfo;

        match self.codebase_manager.status() {
            CodebaseStatus::None => None,
            CodebaseStatus::Cloning { url } => {
                // Extract repo name from URL for better UX
                let repo_name = url
                    .rsplit(['/', ':'])
                    .next()
                    .unwrap_or(url)
                    .trim_end_matches(".git");
                Some(CodebaseStatusInfo {
                    message: format!("Cloning {repo_name}..."),
                    is_loading: true,
                    ..Default::default()
                })
            }
            CodebaseStatus::Fetching { .. } => Some(CodebaseStatusInfo {
                message: "Fetching...".to_string(),
                is_loading: true,
                ..Default::default()
            }),
            CodebaseStatus::Indexing {
                current,
                total,
                current_file,
                language,
                ..
            } => {
                let message = if *total > 0 {
                    let remaining = total.saturating_sub(*current);
                    match (current_file, remaining) {
                        (Some(file), 0) => format!("Indexing {file}"),
                        (Some(file), n) => format!("Indexing {file} + {n} more"),
                        (None, _) => format!("Indexing [{current}/{total}]..."),
                    }
                } else {
                    "Indexing...".to_string()
                };
                Some(CodebaseStatusInfo {
                    message,
                    language: language.clone(),
                    is_loading: true,
                    ..Default::default()
                })
            }
            CodebaseStatus::Ready {
                repo_name,
                metrics_count,
                language,
                head_commit_msg,
                head_commit_hash,
                ..
            } => {
                let is_tantivy_indexing = self.codebase_manager.is_tantivy_indexing();

                // Get Tantivy progress details if indexing
                let (tantivy_phase, tantivy_item, tantivy_progress) =
                    if let Some(progress) = self.codebase_manager.tantivy_progress() {
                        let phase_label = progress.phase().label().to_string();
                        let item = progress.current_item();
                        let (current, total) = progress.get();
                        (
                            Some(phase_label),
                            item,
                            if total > 0 {
                                Some((current, total))
                            } else {
                                None
                            },
                        )
                    } else {
                        (None, None, None)
                    };

                Some(CodebaseStatusInfo {
                    message: format!("{metrics_count} metrics"),
                    repo_name: Some(repo_name.clone()),
                    metrics_count: Some(*metrics_count),
                    language: language.clone(),
                    commit_msg: head_commit_msg.clone(),
                    commit_hash: head_commit_hash.clone(),
                    is_loading: false,
                    is_error: false,
                    is_tantivy_indexing,
                    tantivy_phase,
                    tantivy_item,
                    tantivy_progress,
                })
            }
            CodebaseStatus::Error { message, .. } => Some(CodebaseStatusInfo {
                message: format!("Error: {message}"),
                is_loading: false,
                is_error: true,
                ..Default::default()
            }),
        }
    }

    /// Get the current codebase status for StatusLine display (stub when codebase feature disabled).
    #[cfg(target_arch = "wasm32")]
    pub fn codebase_status_info(
        &self,
    ) -> Option<crate::components::widget::status_line::CodebaseStatusInfo> {
        None
    }

    // =========================================================================
    // Agent Panel Context Building
    // =========================================================================

    /// Build and update the EditorContext for the agent panel.
    ///
    /// This provides the AI agent with awareness of the current editor state,
    /// including connection info, available metrics, codebase status, and
    /// dashboard configuration.
    ///
    /// Uses helper functions from `agent_context` module to build individual
    /// context pieces, ensuring consistency with `build_editor_context`.
    fn update_agent_context(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        use crate::components::overlay::agent_context::{
            CommitSummary, build_codebase_context, load_project_context,
        };
        use crate::components::overlay::agent_context::{
            EditorContext, build_connection_context, build_workspace_context,
        };

        // Build connection context using shared helper
        let connection = build_connection_context(&self.query_executor);

        // Get available metrics (limited to top 50 in EditorContext)
        let metrics: Vec<String> = self.query_executor.metric_names().to_vec();

        // Build codebase context (native only with codebase feature) - includes recent commits
        #[cfg(not(target_arch = "wasm32"))]
        let codebase = {
            use crate::codebase::CodebaseStatus;
            match self.codebase_manager.status() {
                CodebaseStatus::Ready { .. } => {
                    let repo_path = self
                        .codebase_manager
                        .index()
                        .map(|idx| idx.repo_path.display().to_string())
                        .unwrap_or_default();
                    let metric_count = self.codebase_manager.all_metrics().len();
                    let file_count = self
                        .codebase_manager
                        .index()
                        .map(|idx| idx.metrics.len())
                        .unwrap_or(0);

                    // Get recent commits if available
                    let recent_commits = self
                        .codebase_manager
                        .index()
                        .and_then(|_idx| {
                            let time_range = self.time_range_toolbar.time_range();
                            self.codebase_manager
                                .get_commits(time_range.start, time_range.end)
                                .map(|commits| {
                                    commits
                                        .iter()
                                        .take(5)
                                        .map(|c| CommitSummary {
                                            hash: c.short_hash().to_string(),
                                            message: c.message.clone(),
                                        })
                                        .collect::<Vec<_>>()
                                })
                        })
                        .unwrap_or_default();

                    Some(build_codebase_context(
                        repo_path,
                        metric_count,
                        file_count,
                        recent_commits,
                    ))
                }
                _ => None,
            }
        };

        // Build dashboard context using shared helper
        let dashboard = {
            let time_range = self.time_range_toolbar.time_range();
            let pane_count = self.get_pane_tile_ids().len();
            let queries = self.collect_pane_queries();
            let filter = {
                let p = self.viewport_filter.applied_pattern();
                if p.is_empty() {
                    None
                } else {
                    Some(p.to_string())
                }
            };

            build_workspace_context(
                time_range.preset.label().to_string(),
                pane_count,
                queries,
                filter,
            )
        };

        // Build the full context
        let context = EditorContext::new()
            .with_connection(connection)
            .with_metrics(metrics)
            .with_workspace(dashboard);

        // Add codebase context if available
        #[cfg(not(target_arch = "wasm32"))]
        let context = if let Some(cb) = codebase {
            context.with_codebase(cb)
        } else {
            context
        };

        // Load project context from ENYA.md (native only)
        #[cfg(not(target_arch = "wasm32"))]
        let context = {
            let project_ctx = self
                .codebase_manager
                .index()
                .and_then(|idx| load_project_context(&idx.repo_path));
            if let Some(pc) = project_ctx {
                context.with_project_context(pc)
            } else {
                context
            }
        };

        // Add PR review context if a PR is open
        let context = {
            let mut pr_ctx = None;
            for tile_id in self.get_pane_tile_ids() {
                if let Some(egui_tiles::Tile::Pane(pane)) = self.viewport_tree.tiles.get(tile_id) {
                    if let Some(pr_pane) = pane
                        .as_any()
                        .downcast_ref::<crate::components::PrReviewPane>()
                    {
                        if let Some(number) = pr_pane.current_pr_number() {
                            pr_ctx =
                                Some(crate::components::overlay::agent_context::PrReviewContext {
                                    pr_number: number,
                                    pr_title: pr_pane.name(),
                                    draft_comment_count: pr_pane.draft_comments.len(),
                                    changed_files: pr_pane.changed_file_paths(),
                                });
                            break;
                        }
                    }
                }
            }
            if let Some(pr) = pr_ctx {
                context.with_pr_review(pr)
            } else {
                context
            }
        };

        // Update the agent panel's context
        self.agent_panel.set_context(context);
    }

    /// Build the editor context for AI agents.
    ///
    /// This can be used to provide context to agent panes as well as the panel.
    /// Unlike `update_agent_context`, this version does not fetch recent commits
    /// (which would require a mutable borrow of the codebase manager).
    ///
    /// Uses helper functions from `agent_context` module to build individual
    /// context pieces, ensuring consistency with `update_agent_context`.
    fn build_editor_context(&self) -> Option<crate::components::EditorContext> {
        use crate::components::overlay::agent_context::{
            EditorContext, build_connection_context, build_workspace_context,
        };
        #[cfg(not(target_arch = "wasm32"))]
        use crate::components::overlay::agent_context::{
            build_codebase_context, load_project_context,
        };

        // Build connection context using shared helper
        let connection = build_connection_context(&self.query_executor);

        // Get available metrics
        let metrics: Vec<String> = self.query_executor.metric_names().to_vec();

        // Build codebase context (native only with codebase feature) - skips commits (requires mutable borrow)
        #[cfg(not(target_arch = "wasm32"))]
        let codebase = {
            use crate::codebase::CodebaseStatus;
            match self.codebase_manager.status() {
                CodebaseStatus::Ready { .. } => {
                    let repo_path = self
                        .codebase_manager
                        .index()
                        .map(|idx| idx.repo_path.display().to_string())
                        .unwrap_or_default();
                    let metric_count = self.codebase_manager.all_metrics().len();
                    let file_count = self
                        .codebase_manager
                        .index()
                        .map(|idx| idx.metrics.len())
                        .unwrap_or(0);

                    // Note: We skip commits here since get_commits requires &mut self
                    Some(build_codebase_context(
                        repo_path,
                        metric_count,
                        file_count,
                        Vec::new(),
                    ))
                }
                _ => None,
            }
        };

        // Build dashboard context using shared helper
        let dashboard = {
            let time_range = self.time_range_toolbar.time_range();
            let pane_count = self.get_pane_tile_ids().len();
            let queries = self.collect_pane_queries();
            let filter = {
                let p = self.viewport_filter.applied_pattern();
                if p.is_empty() {
                    None
                } else {
                    Some(p.to_string())
                }
            };

            build_workspace_context(
                time_range.preset.label().to_string(),
                pane_count,
                queries,
                filter,
            )
        };

        // Build the full context
        let context = EditorContext::new()
            .with_connection(connection)
            .with_metrics(metrics)
            .with_workspace(dashboard);

        #[cfg(not(target_arch = "wasm32"))]
        let context = if let Some(cb) = codebase {
            context.with_codebase(cb)
        } else {
            context
        };

        // Load project context from ENYA.md (native only)
        #[cfg(not(target_arch = "wasm32"))]
        let context = {
            let project_ctx = self
                .codebase_manager
                .index()
                .and_then(|idx| load_project_context(&idx.repo_path));
            if let Some(pc) = project_ctx {
                context.with_project_context(pc)
            } else {
                context
            }
        };

        // Add PR review context if a PR is open
        let context = {
            let mut pr_ctx = None;
            for tile_id in self.get_pane_tile_ids() {
                if let Some(egui_tiles::Tile::Pane(pane)) = self.viewport_tree.tiles.get(tile_id) {
                    if let Some(pr_pane) = pane
                        .as_any()
                        .downcast_ref::<crate::components::PrReviewPane>()
                    {
                        if let Some(number) = pr_pane.current_pr_number() {
                            pr_ctx =
                                Some(crate::components::overlay::agent_context::PrReviewContext {
                                    pr_number: number,
                                    pr_title: pr_pane.name(),
                                    draft_comment_count: pr_pane.draft_comments.len(),
                                    changed_files: pr_pane.changed_file_paths(),
                                });
                            break;
                        }
                    }
                }
            }
            if let Some(pr) = pr_ctx {
                context.with_pr_review(pr)
            } else {
                context
            }
        };

        Some(context)
    }

    /// Sync git commit history to all time-series panes.
    ///
    /// Called each frame when the codebase is ready. Fetches commits for the
    /// current time range and propagates them to all QueryPane visualizations.
    #[cfg(not(target_arch = "wasm32"))]
    fn sync_commits_to_panes(&mut self, ctx: &egui::Context) {
        // Only sync if codebase is ready
        if !self.codebase_manager.status().is_ready() {
            return;
        }

        // Get current time range
        let time_range = self.time_range_toolbar.time_range();
        let start = time_range.start;
        let end = time_range.end;

        // Trigger fetch if not cached
        self.codebase_manager.fetch_history(start, end, ctx);

        // If commits were just updated OR this is the first sync, propagate to panes
        if self.codebase_manager.commits_updated() {
            if let Some(commits) = self.codebase_manager.get_commits(start, end) {
                let commits_vec = commits.to_vec();
                for tile_id in self.get_pane_tile_ids() {
                    if let Some(egui_tiles::Tile::Pane(component)) =
                        self.viewport_tree.tiles.get_mut(tile_id)
                    {
                        if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>()
                        {
                            query_pane.set_commits(commits_vec.clone());
                        }
                    }
                }
            }
        }
    }

    // ==================== Community Plugin Actions ====================

    /// Check if there's a pending install plugin action.
    pub fn has_pending_install_plugin(&self) -> bool {
        self.pending_install_plugin.is_some()
    }

    /// Take the pending install plugin action if any.
    /// Returns (name, file) tuple if there's a pending install.
    pub fn take_pending_install_plugin(&mut self) -> Option<(String, String)> {
        self.pending_install_plugin.take()
    }

    /// Check if there's a pending remove plugin action.
    pub fn has_pending_remove_plugin(&self) -> bool {
        self.pending_remove_plugin.is_some()
    }

    /// Take the pending remove plugin action if any.
    /// Returns plugin name if there's a pending remove.
    pub fn take_pending_remove_plugin(&mut self) -> Option<String> {
        self.pending_remove_plugin.take()
    }

    /// Take and clear the pending refresh plugins flag.
    /// Returns true if refresh was requested.
    pub fn take_pending_refresh_plugins(&mut self) -> bool {
        std::mem::take(&mut self.pending_refresh_plugins)
    }
}
