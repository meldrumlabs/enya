use rustc_hash::{FxHashMap, FxHashSet};

use egui_tiles::{Tile, TileId, Tiles};

use crate::AsyncRuntime;
use crate::app::AppState;
use crate::chat::{ChannelsPanel, ChannelsPanelAction};
#[cfg(not(target_arch = "wasm32"))]
use crate::codebase::CodebaseManager;
#[cfg(target_arch = "wasm32")]
use crate::components::NativePromoOverlay;
use crate::components::overlay::{AnnotationEditor, AnnotationEditorResult};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::overlay::{CodebaseFinder, CodebaseFinderStatus, DiffViewerOverlay};
use crate::components::overlay::{FinderMode, UnifiedFinder};
use crate::components::{
    AgentCommand, AgentInputBar, AgentInputBarResult, AgentPanel, AgentPanelResult, Buffer,
    BufferEditor, BufferEditorResult, CommandPalette, CommandResult, Component, ContextPane,
    DiagnosticsPane, InfoOverlay, LandingPage, LandingPageAction, LogsPane, MultiBufferMode,
    MultiBufferState, MultiEditOverlay, MultiEditResult, QueryExecutor, QueryLanguage, QueryPane,
    QueryState, QuickCommand, SourcePreviewOverlay, SourcePreviewResult, StylePicker,
    StylePickerResult, TeamMember, TeamMenu, TeamMenuAction, TeamStatusInfo, TimeRangeToolbar,
    TutorialOverlay, ViewportFilter, ViewportFilterResult, WhichKey, WorkspaceCreator,
    WorkspaceCreatorResult, WorkspaceFinder,
};
use crate::ui::settings_screen::EditorFont;
use crate::ui::theme::AppTheme;

// Workspace configuration module (serialization)
pub mod config;

// Grafana dashboard JSON import
pub mod grafana;

// Input handling (navigation, visual-multi mode)
mod input;
pub use input::{
    FocusTarget, LEADER_KEY_TIMEOUT_MS, LeaderKeyState, NavDirection, SectionState,
    VisualMultiState,
};

// Section rendering for collapsible sections
mod sections;
pub use sections::{
    MIN_PANE_SIZE, SECTION_CONTENT_PADDING, SECTION_GRID_CELL_HEIGHT, SECTION_HEADER_HEIGHT,
    SECTION_PANE_GAP, SECTION_PANE_HEIGHT, SectionRenderer,
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

// Re-export config types for convenience
pub use config::{
    ATLAS_WORKSPACE_TOML, COMPLEX_VIEWPORT_TOML, ConnectionConfig, DEFAULT_WORKSPACE_TOML,
    DEMO_WORKSPACE_TOML, GitConfig, LayoutConfig, LayoutContainer, LayoutNode, LayoutType,
    LogsConfig, MetricsConfig, PaneConfig, RefreshInterval, SectionConfig, SectionLayout,
    TimeConfig, ViewConfig, WORKSPACE_VERSION, WorkspaceConfig, WorkspaceError, WorkspaceMeta,
};

/// Actions that the Workspace needs the App to handle
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceAction {
    /// No action needed
    None,
    /// Set a specific theme
    SetTheme(AppTheme),
    /// Cycle to the next theme
    NextTheme,
    /// Set the editor font
    SetFont(EditorFont),
    /// Set both theme and font (used when cancelling style picker to restore originals)
    SetThemeAndFont(AppTheme, EditorFont),
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
    /// Save workspace with optional name
    SaveWorkspace(Option<String>),
    /// Load workspace by name
    LoadWorkspace(String),
    /// List available workspaces
    ListWorkspaces,
    /// Share workspace as URL (encodes to base64 and copies to clipboard)
    ShareWorkspace,
    /// Share a single pane as URL (encodes to base64 and copies to clipboard)
    SharePane(usize),
    /// Quit the application
    QuitApp,
    /// Toggle team demo mode (for testing UI without backend)
    ToggleTeamDemo,
    /// Connect to team server
    TeamConnect { url: String, token: String },
    /// Disconnect from team server
    TeamDisconnect,
    /// Open the annotation editor for the focused pane
    OpenAnnotationEditor,
    /// Send a chat message (with optional inline chart or visualization)
    SendChatMessage {
        text: String,
        chart: Option<crate::chat::InlineChart>,
        visualization: Option<crate::chat::InlineVisualization>,
        thread_id: Option<crate::chat::ThreadId>,
    },
    /// Create a new channel
    CreateChannel { name: String },
    /// Create a new thread in a channel
    CreateThread {
        channel_id: crate::chat::ChannelId,
        title: String,
    },
    /// Search commits for # autocomplete in chat
    SearchChatCommits { query: String },
    /// Open diff viewer from a commit reference in chat
    OpenDiffViewer {
        hash: String,
        message: String,
        diff: String,
    },
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
    /// Workspace finder modal (for loading saved workspaces)
    workspace_finder: WorkspaceFinder,
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
    /// State for leader key sequences (t, Space, y, c)
    leader_keys: LeaderKeyState,
    /// Info overlay (shows build/version info)
    info_overlay: InfoOverlay,
    /// Which-key overlay (shows available keybindings)
    which_key: WhichKey,
    /// Style picker overlay (unified theme + font selection)
    style_picker: StylePicker,
    /// Tutorial overlay (interactive walkthrough)
    tutorial_overlay: TutorialOverlay,
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
    /// Flag to open workspace finder (set by keyboard, handled in show with app_state)
    pending_open_workspace_finder: bool,
    /// Flag to open style picker (set by command, handled in show with app_state)
    pending_open_style_picker: bool,
    /// Query executor for running queries against backends (Prometheus, Enya)
    query_executor: QueryExecutor,
    /// Counter for sequential query pane naming (Query 1, Query 2, ...)
    next_query_number: usize,
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
    #[cfg(not(target_arch = "wasm32"))]
    diff_viewer: DiffViewerOverlay,
    /// Pending git config URL to initialize (set during load, executed in show())
    #[cfg(not(target_arch = "wasm32"))]
    pending_git_config: Option<String>,
    /// Pending connection endpoint to apply (set during load, executed in show())
    pending_connection_endpoint: Option<String>,
    /// Auto-refresh interval (None = disabled)
    refresh_interval: Option<RefreshInterval>,
    /// Last time queries were auto-refreshed
    last_refresh: Option<crate::util::Instant>,
    /// Pending git repo path to configure (set from workspace creator)
    #[cfg(not(target_arch = "wasm32"))]
    pending_git_repo: Option<String>,
    /// Native app promo overlay (WASM only)
    #[cfg(target_arch = "wasm32")]
    native_promo_overlay: NativePromoOverlay,
    /// Unified finder (Telescope-style fuzzy finder)
    unified_finder: UnifiedFinder,
    /// Team collaboration menu (only shown when connected to a team)
    team_menu: TeamMenu,
    /// Team collaboration status (only shown when connected to a team)
    team_status: Option<TeamStatusInfo>,
    /// Team members for the team menu (populated by EnyaApp from TeamState)
    team_members: Vec<TeamMember>,
    /// Annotation editor overlay
    annotation_editor: AnnotationEditor,
    /// Channels panel sidebar (shown when connected to a team)
    channels_panel: ChannelsPanel,
    /// Whether the channels panel sidebar is visible
    channels_panel_visible: bool,
    /// Whether the channels panel has keyboard focus (vim h/l to transfer)
    channels_panel_focused: bool,

    // ==================== Collapsible Sections ====================
    /// Section configurations (when workspace uses sections format)
    section_configs: Vec<SectionConfig>,
    /// Runtime state for each section (collapsed/expanded)
    section_states: Vec<SectionState>,
    /// Current focus target for section-aware navigation
    section_focus: FocusTarget,
    /// Section renderer for drawing section headers and layouts
    section_renderer: SectionRenderer,

    // ==================== Floating Panes ====================
    /// Floating panes that hover above the tile layout
    floating_panes: FloatingPaneManager,

    // ==================== Undo System ====================
    /// Stack of undoable actions (vim-style 'u' to undo)
    undo_stack: UndoStack,
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

        Self {
            viewport_tree,
            behavior: TreeBehavior::default(),
            open_charts: FxHashSet::default(),
            pending_chart: None,
            time_range_toolbar: TimeRangeToolbar::new(),
            workspace_finder: WorkspaceFinder::new(),
            command_palette: CommandPalette::new(),
            buffer_editor: BufferEditor::new(),
            editing_tile_id: None,
            zen_mode: false,
            fullscreen_tile: None,
            landing_page: LandingPage::new(),
            show_landing: true, // Start with landing page
            leader_keys: LeaderKeyState::new(),
            info_overlay: InfoOverlay::new(enya_build_info::build_info!()),
            which_key: WhichKey::new(),
            style_picker: StylePicker::new(),
            tutorial_overlay: TutorialOverlay::new(),
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
            pending_open_workspace_finder: false,
            pending_open_style_picker: false,
            query_executor: QueryExecutor::new(async_runtime.clone()),
            next_query_number: 1,
            viewport_filter: ViewportFilter::new(),
            source_preview: SourcePreviewOverlay::new(),
            #[cfg(not(target_arch = "wasm32"))]
            agent_panel: AgentPanel::new(async_runtime.handle().clone()),
            #[cfg(target_arch = "wasm32")]
            agent_panel: AgentPanel::new(),
            #[cfg(not(target_arch = "wasm32"))]
            agent_input_bar: AgentInputBar::new_with_runtime(async_runtime.handle().clone()),
            #[cfg(target_arch = "wasm32")]
            agent_input_bar: AgentInputBar::new(),
            agent_mode_active: false,
            agent_panel_focused: false,
            agent_context_panes: FxHashSet::default(),
            #[cfg(not(target_arch = "wasm32"))]
            codebase_manager: CodebaseManager::new(),
            #[cfg(not(target_arch = "wasm32"))]
            codebase_finder: CodebaseFinder::new(),
            #[cfg(not(target_arch = "wasm32"))]
            diff_viewer: DiffViewerOverlay::new(),
            #[cfg(not(target_arch = "wasm32"))]
            pending_git_config: None,
            pending_connection_endpoint: None,
            refresh_interval: None,
            last_refresh: None,
            #[cfg(not(target_arch = "wasm32"))]
            pending_git_repo: None,
            #[cfg(target_arch = "wasm32")]
            native_promo_overlay: NativePromoOverlay::new(),
            unified_finder: UnifiedFinder::new(),
            team_menu: TeamMenu::new(),
            team_status: None,
            team_members: Vec::new(),
            annotation_editor: AnnotationEditor::new(),
            channels_panel: ChannelsPanel::new(),
            channels_panel_visible: false,
            channels_panel_focused: false,
            // Section state
            section_configs: Vec::new(),
            section_states: Vec::new(),
            section_focus: FocusTarget::default(),
            section_renderer: SectionRenderer::default(),
            // Floating panes
            floating_panes: FloatingPaneManager::new(),
            // Undo system
            undo_stack: UndoStack::new(),
        }
    }

    #[profiling::function]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_state: &AppState,
        chat_state: Option<&crate::chat::ChatState>,
    ) -> WorkspaceAction {
        self.behavior.set_theme(app_state.theme);
        self.behavior
            .set_keys(app_state.settings.api_key.to_owned());

        // Disable terminal keyboard input when modals are open
        // This prevents terminal from capturing j/k/h/l keys meant for overlays
        #[cfg(not(target_arch = "wasm32"))]
        {
            let modal_open = self.style_picker.is_open()
                || self.workspace_finder.is_open()
                || self.unified_finder.is_open()
                || self.command_palette.is_open()
                || self.buffer_editor.is_open()
                || self.multi_edit_overlay.is_open()
                || self.which_key.is_open()
                || self.viewport_filter.is_open()
                || self.tutorial_overlay.is_open()
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
                log::info!(
                    "Processing {} agent command(s) before query execution",
                    commands.len()
                );
                for cmd in &commands {
                    log::info!("Executing agent command: {cmd:?}");
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
                    log::info!("Agent command executed (no response text), exiting agent mode");
                    self.exit_agent_mode();
                }
            }
        }

        // Check auto-refresh timer and trigger refresh if due
        self.check_auto_refresh();

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
        self.time_range_toolbar.set_theme(app_state.theme);
        self.landing_page.set_theme(app_state.theme);

        // Handle adding a pending chart to the viewport
        if let Some(metric_name) = self.pending_chart.take() {
            let action = self.add_chart_for_metric_with_tracking(&metric_name);
            if action != WorkspaceAction::None {
                return action;
            }
        }

        // Handle pending workspace finder open (needs app_state for workspace list)
        if self.pending_open_workspace_finder {
            self.pending_open_workspace_finder = false;
            self.open_workspace_finder(app_state, crate::app::EnyaApp::list_available_workspaces());
        }

        // Handle pending style picker open (needs app_state for current theme and font)
        if self.pending_open_style_picker {
            self.pending_open_style_picker = false;
            self.style_picker
                .open(app_state.theme, app_state.settings.font);
        }

        // Show landing page only if explicitly enabled and no charts open
        // (new workspaces start with show_landing=false for a clean empty state)
        if self.show_landing && self.open_charts.is_empty() {
            return self.show_landing_page(ui, ctx, app_state);
        }

        // Check if chat split view is active (takes over main area)
        let chat_split_view_active = self.channels_panel_visible
            && self.team_status.is_some()
            && self.channels_panel.is_split_view_active();

        // Left sidebar: Channels panel (when team mode is active, but NOT in split view)
        // When split view is active, the chat takes over the entire central panel
        let mut pending_message: Option<(
            String,
            Option<crate::chat::InlineChart>,
            Option<crate::chat::InlineVisualization>,
        )> = None;
        let mut pending_create_channel = false;
        let mut pending_create_thread: Option<crate::chat::ChannelId> = None;
        let mut pending_commit_search: Option<String> = None;
        let mut pending_diff_viewer: Option<(String, String, String)> = None;

        // Check if any overlay is open that should block keyboard input
        let overlay_blocks_input = self.style_picker.is_open()
            || self.workspace_finder.is_open()
            || self.unified_finder.is_open()
            || self.command_palette.is_open()
            || self.which_key.is_open();
        self.channels_panel
            .set_overlay_blocks_input(overlay_blocks_input);

        // Propagate overlay_blocks_input to all pane components
        for (_tile_id, tile) in self.viewport_tree.tiles.iter_mut() {
            if let egui_tiles::Tile::Pane(component) = tile {
                component.set_overlay_blocks_input(overlay_blocks_input);
            }
        }

        if self.channels_panel_visible && self.team_status.is_some() && !chat_split_view_active {
            if let Some(chat_state) = chat_state {
                // Update available panes for @-mention autocomplete
                let pane_info = self.collect_pane_info();
                self.channels_panel.set_available_panes(pane_info);

                egui::SidePanel::left("channels_panel")
                    .resizable(true)
                    .min_width(220.0)
                    .default_width(280.0)
                    .max_width(400.0)
                    .show_inside(ui, |ui| {
                        self.channels_panel.set_theme(app_state.theme);
                        match self.channels_panel.show(
                            ui,
                            chat_state.threads(),
                            chat_state.channels(),
                            &self.team_members,
                            chat_state,
                        ) {
                            ChannelsPanelAction::SelectChannel(id) => {
                                log::info!("Selected channel: {id:?}");
                                self.channels_panel.select_channel(id);
                            }
                            ChannelsPanelAction::SelectThread(id) => {
                                log::info!("Selected thread: {id:?}");
                                self.channels_panel.select_thread(id);
                            }
                            ChannelsPanelAction::CreateThread(channel_id) => {
                                log::info!("Create thread in channel: {channel_id:?}");
                                pending_create_thread = Some(channel_id);
                            }
                            ChannelsPanelAction::CreateChannel => {
                                log::info!("Create new channel");
                                pending_create_channel = true;
                            }
                            ChannelsPanelAction::SelectMember(user_id) => {
                                log::info!("Selected member: {user_id:?}");
                            }
                            ChannelsPanelAction::StartDM(user_id) => {
                                log::info!("Start DM with: {user_id:?}");
                            }
                            ChannelsPanelAction::SendMessage {
                                text,
                                chart,
                                visualization,
                            } => {
                                pending_message = Some((text, chart, visualization));
                            }
                            ChannelsPanelAction::SearchCommits(query) => {
                                pending_commit_search = Some(query);
                            }
                            ChannelsPanelAction::OpenDiffViewer {
                                hash,
                                message,
                                diff,
                            } => {
                                pending_diff_viewer = Some((hash, message, diff));
                            }
                            ChannelsPanelAction::ReturnFocusToViewport => {
                                // Vim l key pressed - return focus to viewport
                                self.channels_panel_focused = false;
                                self.channels_panel.set_focus(false);
                                // Restore focus to first pane if nothing else has focus
                                if self.behavior.focused_tile().is_none() {
                                    if let Some(first_pane) = self.get_pane_tile_ids().first() {
                                        self.behavior.set_focused_tile(Some(*first_pane));
                                    }
                                }
                            }
                            ChannelsPanelAction::None => {}
                        }
                    });
            }
        }

        // Full-screen chat view (when split view is active, takes over entire central panel)
        if chat_split_view_active {
            if let Some(chat_state) = chat_state {
                // Update available panes for @-mention autocomplete
                let pane_info = self.collect_pane_info();
                self.channels_panel.set_available_panes(pane_info);

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.channels_panel.set_theme(app_state.theme);
                    match self.channels_panel.show(
                        ui,
                        chat_state.threads(),
                        chat_state.channels(),
                        &self.team_members,
                        chat_state,
                    ) {
                        ChannelsPanelAction::SelectChannel(id) => {
                            log::info!("Selected channel: {id:?}");
                            self.channels_panel.select_channel(id);
                        }
                        ChannelsPanelAction::SelectThread(id) => {
                            log::info!("Selected thread: {id:?}");
                            self.channels_panel.select_thread(id);
                        }
                        ChannelsPanelAction::CreateThread(channel_id) => {
                            log::info!("Create thread in channel: {channel_id:?}");
                            pending_create_thread = Some(channel_id);
                        }
                        ChannelsPanelAction::CreateChannel => {
                            log::info!("Create new channel");
                            pending_create_channel = true;
                        }
                        ChannelsPanelAction::SelectMember(user_id) => {
                            log::info!("Selected member: {user_id:?}");
                        }
                        ChannelsPanelAction::StartDM(user_id) => {
                            log::info!("Start DM with: {user_id:?}");
                        }
                        ChannelsPanelAction::SendMessage {
                            text,
                            chart,
                            visualization,
                        } => {
                            pending_message = Some((text, chart, visualization));
                        }
                        ChannelsPanelAction::SearchCommits(query) => {
                            pending_commit_search = Some(query);
                        }
                        ChannelsPanelAction::OpenDiffViewer {
                            hash,
                            message,
                            diff,
                        } => {
                            pending_diff_viewer = Some((hash, message, diff));
                        }
                        ChannelsPanelAction::ReturnFocusToViewport => {
                            // Vim l key pressed - return focus to viewport
                            self.channels_panel_focused = false;
                            self.channels_panel.set_focus(false);
                            // Restore focus to first pane if nothing else has focus
                            if self.behavior.focused_tile().is_none() {
                                if let Some(first_pane) = self.get_pane_tile_ids().first() {
                                    self.behavior.set_focused_tile(Some(*first_pane));
                                }
                            }
                        }
                        ChannelsPanelAction::None => {}
                    }
                });
            }
        }

        // Handle pending message (after chat_state borrow is released)
        // Return action to app so message is added to team_state.chat_state (not the clone)
        if let Some((text, chart, visualization)) = pending_message {
            let thread_id = self.channels_panel.selected_thread();
            return WorkspaceAction::SendChatMessage {
                text,
                chart,
                visualization,
                thread_id,
            };
        }

        // Handle pending create channel (after chat_state borrow is released)
        if pending_create_channel {
            // Generate unique channel name with timestamp
            let timestamp = crate::util::now_unix_secs();
            return WorkspaceAction::CreateChannel {
                name: format!("channel-{}", timestamp % 10000),
            };
        }

        // Handle pending create thread (after chat_state borrow is released)
        if let Some(channel_id) = pending_create_thread {
            // Generate unique thread title with timestamp
            let timestamp = crate::util::now_unix_secs();
            return WorkspaceAction::CreateThread {
                channel_id,
                title: format!("thread-{}", timestamp % 10000),
            };
        }

        // Handle pending commit search (for # autocomplete in chat)
        if let Some(query) = pending_commit_search {
            return WorkspaceAction::SearchChatCommits { query };
        }

        // Handle pending diff viewer request (from commit click in chat)
        if let Some((hash, message, diff)) = pending_diff_viewer {
            return WorkspaceAction::OpenDiffViewer {
                hash,
                message,
                diff,
            };
        }

        // Right sidebar: Agent panel (Claude Code integration)
        // Rendered as a layout participant - viewport shrinks when panel is open
        // Update context before showing so the agent has awareness of editor state
        self.update_agent_context();
        self.agent_panel.set_theme(app_state.theme);
        self.agent_panel.set_focus(self.agent_panel_focused);
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
                // Set section_focus to first focusable target (this controls visual focus)
                self.section_focus = self.first_focusable_target();
                // Sync behavior.focused_tile() with section_focus
                let tile_id = self.section_focus_to_tile_id();
                self.behavior.set_focused_tile(tile_id);
            }
            AgentPanelResult::EnteredInputMode => {
                // User pressed i or Enter to enter chat input - release vim focus
                // but keep panel open (user is now typing in the text input)
                self.agent_panel_focused = false;
            }
            AgentPanelResult::None => {
                // Don't sync - agent_panel_focused should only change via explicit actions
                // (ReturnFocusToViewport, Closed, or keyboard transfer).
                // The panel's internal has_focus tracks vim focus within the panel,
                // while agent_panel_focused tracks whether the workspace considers the panel active.
            }
        }

        if chat_split_view_active {
            // Already rendered chat in central panel above
        } else {
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
                self.viewport_filter.set_theme(app_state.theme);

                egui::TopBottomPanel::top("time_range_toolbar")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.add_space(4.0);

                        let toolbar_rect = ui.available_rect_before_wrap();

                        // Only show keyboard hints when there are panes open
                        // (landing page already shows hints when workspace is empty)
                        if total_panes > 0 {
                            let hint_color = app_state.theme.text_tertiary();
                            let hint_text = "hjkl navigate   : cmd   ? help";
                            let font = egui::FontId::proportional(11.0);
                            let hint_galley = ui.painter().layout_no_wrap(
                                hint_text.to_string(),
                                font.clone(),
                                hint_color,
                            );
                            let hint_pos = egui::pos2(
                                toolbar_rect.center().x - hint_galley.size().x / 2.0,
                                toolbar_rect.center().y - hint_galley.size().y / 2.0,
                            );
                            ui.painter().galley(hint_pos, hint_galley, hint_color);
                        }

                        // Now draw the interactive elements on top
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

                            // Flexible spacer to push time controls to the right
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    self.time_range_toolbar.show_with_countdown(ui, countdown);
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
                        component.set_api_key(self.behavior.api_key());
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
                } else if !self.section_configs.is_empty() {
                    // Render sections with collapsible headers (Grafana-style)
                    egui::ScrollArea::vertical()
                        .id_salt("sections_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            self.render_sections(ui);
                        });
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
                        self.draw_scrollbar(ui.painter(), scrollbar_rect, app_state.theme);
                    }
                }
            });
        });
        } // end else (not chat_split_view_active)

        // ==================== Floating Panes ====================
        // Render floating panes above the tile layout but below modal overlays
        self.floating_panes.set_theme(app_state.theme);
        // Use the available rect as the viewport for floating pane snapping/maximize
        let floating_viewport = ctx.available_rect();
        let floating_actions = self
            .floating_panes
            .show(ctx, app_state.theme, floating_viewport);
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

        // Show diff viewer overlay modal (native only)
        // NOTE: This is rendered BEFORE style_picker and command_palette so they appear on top
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.diff_viewer.set_theme(app_state.theme);
            // Disable keyboard when another overlay is on top
            self.diff_viewer.set_keyboard_disabled(
                self.style_picker.is_open() || self.command_palette.is_open(),
            );
            let _ = self.diff_viewer.show(ctx);
        }

        // Show workspace finder modal (rendered on top of everything)
        self.workspace_finder.set_theme(app_state.theme);
        if let Some(selected_workspace) = self.workspace_finder.show(ctx) {
            return WorkspaceAction::LoadWorkspace(selected_workspace);
        }

        // Show style picker modal (unified theme + font picker)
        match self
            .style_picker
            .show(ctx, app_state.theme, app_state.settings.font)
        {
            StylePickerResult::ThemeSelected(theme) => return WorkspaceAction::SetTheme(theme),
            StylePickerResult::Cancelled(original_theme, original_font) => {
                return WorkspaceAction::SetThemeAndFont(original_theme, original_font);
            }
            StylePickerResult::ThemePreview(theme) => return WorkspaceAction::SetTheme(theme),
            StylePickerResult::FontSelected(font) => return WorkspaceAction::SetFont(font),
            StylePickerResult::FontPreview(font) => return WorkspaceAction::SetFont(font),
            StylePickerResult::None => {}
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

            self.codebase_finder.set_theme(app_state.theme);
            if let Some(finder_result) = self.codebase_finder.show(ctx) {
                // Handle the selected result - navigate to source
                self.handle_codebase_finder_result(finder_result.result);
            }
        }

        // Show command palette modal
        self.command_palette.set_theme(app_state.theme);
        let cmd_result = self.command_palette.show(ctx);

        // Show buffer editor modal
        self.buffer_editor.set_theme(app_state.theme);
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
        self.multi_edit_overlay.set_theme(app_state.theme);
        match self.multi_edit_overlay.show(ctx) {
            MultiEditResult::Applied(changes) => {
                self.apply_multi_edit_changes(changes);
            }
            MultiEditResult::Cancelled | MultiEditResult::None => {}
        }

        // Show info overlay modal
        self.info_overlay.set_theme(app_state.theme);
        self.info_overlay.show(ctx);

        // Show which-key overlay modal
        self.which_key.set_theme(app_state.theme);
        self.which_key.show(ctx);

        // Show tutorial overlay modal
        self.tutorial_overlay.set_theme(app_state.theme);
        self.tutorial_overlay.show(ctx);

        // Show workspace creator overlay modal
        self.workspace_creator.set_theme(app_state.theme);
        match self.workspace_creator.show(ctx) {
            WorkspaceCreatorResult::Created {
                name,
                endpoint,
                git_repo,
            } => {
                // Set pending connection endpoint to apply
                self.pending_connection_endpoint = Some(endpoint);
                // Store git repo path for codebase integration (native only with codebase feature)
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.pending_git_repo = git_repo;
                }
                #[cfg(target_arch = "wasm32")]
                let _ = git_repo; // Silence unused warning
                self.show_landing = false;
                ctx.request_repaint();
                // Return action to save the workspace
                return WorkspaceAction::SaveWorkspace(Some(name));
            }
            WorkspaceCreatorResult::Cancelled | WorkspaceCreatorResult::None => {}
        }

        // Show diagnostics overlay modal
        self.diagnostics_pane.set_theme(app_state.theme);
        self.diagnostics_pane.show_overlay(ctx);

        // Show source preview overlay modal
        if self.source_preview.is_open() {
            log::debug!("source_preview.is_open() = true, calling show()");
        }
        self.source_preview.set_theme(app_state.theme);
        match self.source_preview.show(ctx) {
            SourcePreviewResult::Closed => {
                log::debug!("Source preview closed");
            }
            SourcePreviewResult::None => {}
        }

        // Note: diff_viewer is now rendered earlier (before style_picker) to ensure proper z-order

        // Note: Agent panel is now rendered in the layout section (show_inside)
        // to participate in layout flow like the channels panel

        // Note: agent_input_bar.poll() is now called at the start of show()
        // to ensure agent-created panes are available for immediate query execution

        // Show team menu (only when connected to a team)
        // The menu renders as a centered overlay (like unified finder)
        if let Some(ref status) = self.team_status {
            self.team_menu.set_theme(app_state.theme);

            // Get team name and current user ID from status
            let team_name = status.team_name.as_deref();
            let current_user_id = None; // TODO: Get from TeamState

            match self
                .team_menu
                .show(ctx, &self.team_members, team_name, current_user_id)
            {
                TeamMenuAction::AddAnnotation => {
                    log::info!("Team action: Add annotation");
                    // Open annotation editor on the focused pane
                    self.open_annotation_editor();
                }
                TeamMenuAction::ShareView => {
                    log::info!("Team action: Share current view");
                    // TODO: Implement share view via TeamClient
                }
                TeamMenuAction::StartWarRoom => {
                    log::info!("Team action: Start war room");
                    // TODO: Implement war room mode
                }
                TeamMenuAction::OpenSettings => {
                    log::info!("Team action: Open settings");
                    // TODO: Implement team settings
                }
                TeamMenuAction::SwitchTeam => {
                    log::info!("Team action: Switch team");
                    // TODO: Implement team switcher
                }
                TeamMenuAction::SignOut => {
                    log::info!("Team action: Sign out");
                    // Clear team status to disconnect
                    self.team_status = None;
                }
                TeamMenuAction::None => {}
            }
        }

        // Poll codebase manager for async operations (native only with codebase feature)
        #[cfg(not(target_arch = "wasm32"))]
        self.codebase_manager.poll(ctx);

        // Poll agent panes for pending commands
        self.poll_agent_pane_commands(ctx);

        // Update viewport filter state (rendering happens in bottom panel)
        self.viewport_filter.set_theme(app_state.theme);
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
        // Process overlay FIRST so it can consume keyboard input before landing page
        #[cfg(target_arch = "wasm32")]
        let native_promo_open = {
            // Don't auto-open - let user click the footer link to see details
            self.native_promo_overlay.set_theme(app_state.theme);
            self.native_promo_overlay.show(ctx);
            self.native_promo_overlay.is_open()
        };
        #[cfg(not(target_arch = "wasm32"))]
        let native_promo_open = false;

        // Disable landing page keyboard when any modal overlay is open
        // This prevents the landing page from consuming keyboard input meant for modals
        let modal_open = native_promo_open
            || self.style_picker.is_open()
            || self.workspace_finder.is_open()
            || self.command_palette.is_open()
            || self.which_key.is_open();
        self.landing_page.set_keyboard_disabled(modal_open);

        // Show the landing page in the central panel
        let mut landing_action = LandingPageAction::None;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            landing_action = self.landing_page.show(ui, ctx);
        });

        // Handle landing page actions
        match landing_action {
            LandingPageAction::OpenWorkspaceFinder => {
                self.open_workspace_finder(
                    app_state,
                    crate::app::EnyaApp::list_available_workspaces(),
                );
            }
            LandingPageAction::CreateWorkspace => {
                // Open the workspace creator overlay (works on both native and WASM)
                // On WASM, only the endpoint step is shown
                self.workspace_creator.open();
            }
            LandingPageAction::OpenTutorial => {
                // Hide landing page and setup tutorial layout
                // Layout: HTTP Requests | Requests by Endpoint (side by side at top)
                //         CPU Usage
                //         Memory Used
                self.show_landing = false;
                self.setup_tutorial_layout();
                self.tutorial_overlay.open();
                ctx.request_repaint();
            }
            LandingPageAction::ShowAbout => {
                self.info_overlay.open();
            }
            LandingPageAction::ShowShortcuts => {
                self.which_key.open();
            }
            LandingPageAction::OpenDocs => {
                ctx.open_url(egui::OpenUrl::new_tab("https://enya.build/docs"));
            }
            LandingPageAction::ShowNativeAppInfo => {
                // Open the native app promo overlay (WASM only)
                #[cfg(target_arch = "wasm32")]
                self.native_promo_overlay.open_force();
            }
            LandingPageAction::None => {}
        }

        // Show workspace finder modal (rendered on top of everything)
        self.workspace_finder.set_theme(app_state.theme);
        if let Some(selected_workspace) = self.workspace_finder.show(ctx) {
            return WorkspaceAction::LoadWorkspace(selected_workspace);
        }

        // Show style picker modal (unified theme + font picker)
        match self
            .style_picker
            .show(ctx, app_state.theme, app_state.settings.font)
        {
            StylePickerResult::ThemeSelected(theme) => return WorkspaceAction::SetTheme(theme),
            StylePickerResult::Cancelled(original_theme, original_font) => {
                return WorkspaceAction::SetThemeAndFont(original_theme, original_font);
            }
            StylePickerResult::ThemePreview(theme) => return WorkspaceAction::SetTheme(theme),
            StylePickerResult::FontSelected(font) => return WorkspaceAction::SetFont(font),
            StylePickerResult::FontPreview(font) => return WorkspaceAction::SetFont(font),
            StylePickerResult::None => {}
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

            self.codebase_finder.set_theme(app_state.theme);
            if let Some(finder_result) = self.codebase_finder.show(ctx) {
                // Handle the selected result - navigate to source
                self.handle_codebase_finder_result(finder_result.result);
            }
        }

        // Show command palette modal
        self.command_palette.set_theme(app_state.theme);
        let cmd_result = self.command_palette.show(ctx);

        // Show info overlay modal
        self.info_overlay.set_theme(app_state.theme);
        self.info_overlay.show(ctx);

        // Show which-key overlay modal
        self.which_key.set_theme(app_state.theme);
        self.which_key.show(ctx);

        // Show tutorial overlay modal
        self.tutorial_overlay.set_theme(app_state.theme);
        self.tutorial_overlay.show(ctx);

        // Show workspace creator overlay modal
        self.workspace_creator.set_theme(app_state.theme);
        match self.workspace_creator.show(ctx) {
            WorkspaceCreatorResult::Created {
                name,
                endpoint,
                git_repo,
            } => {
                // Set pending connection endpoint to apply
                self.pending_connection_endpoint = Some(endpoint);
                // Store git repo path for codebase integration (native only with codebase feature)
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.pending_git_repo = git_repo;
                }
                #[cfg(target_arch = "wasm32")]
                let _ = git_repo; // Silence unused warning
                self.show_landing = false;
                ctx.request_repaint();
                // Return action to save the workspace
                return WorkspaceAction::SaveWorkspace(Some(name));
            }
            WorkspaceCreatorResult::Cancelled | WorkspaceCreatorResult::None => {}
        }

        // Show diagnostics overlay modal
        self.diagnostics_pane.set_theme(app_state.theme);
        self.diagnostics_pane.show_overlay(ctx);

        // Show annotation editor overlay modal
        self.annotation_editor.set_theme(app_state.theme);
        match self.annotation_editor.show(ctx) {
            AnnotationEditorResult::Created(annotation) => {
                // Add annotation to the focused pane's chart
                if let Some(focused_id) = self.behavior.focused_tile() {
                    if let Some(Tile::Pane(pane)) = self.viewport_tree.tiles.get_mut(focused_id) {
                        if let Some(query_pane) = pane.as_any_mut().downcast_mut::<QueryPane>() {
                            query_pane.add_annotation(annotation);
                            log::info!("Added annotation to focused pane");
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
                            log::info!("Updated annotation in focused pane");
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
                            log::info!("Removed annotation from focused pane");
                        }
                    }
                }
            }
            AnnotationEditorResult::Cancelled | AnnotationEditorResult::None => {}
        }

        // Handle Space+d for diagnostics on landing page
        // (viewport keyboard handling doesn't run on landing page)
        if !self.workspace_finder.is_open()
            && !self.command_palette.is_open()
            && !self.which_key.is_open()
            && !self.diagnostics_pane.is_open()
        {
            ctx.input_mut(|input| {
                // Space - leader key for sequences
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Space) {
                    self.leader_keys.press_space();
                }

                // Leader key sequences (must follow Space within timeout)
                if self.leader_keys.is_space_active() {
                    // Space+d - toggle diagnostics overlay
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::D) {
                        self.diagnostics_pane.toggle();
                        self.diagnostics_visible = self.diagnostics_pane.is_open();
                        self.leader_keys.clear_space();
                    }
                }
            });
        }

        self.handle_command_result(cmd_result, ctx)
    }

    /// Handle a command result from the command palette
    fn handle_command_result(
        &mut self,
        result: CommandResult,
        ctx: &egui::Context,
    ) -> WorkspaceAction {
        match result {
            CommandResult::OpenStylePicker => {
                // Style picker needs current theme and font - flag it to open on next show()
                self.pending_open_style_picker = true;
                WorkspaceAction::None
            }
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
            CommandResult::WriteWorkspace => WorkspaceAction::SaveWorkspace(None),
            CommandResult::TakeScreenshot(path) => WorkspaceAction::TakeScreenshot(path),
            CommandResult::LoadWorkspace(name) => WorkspaceAction::LoadWorkspace(name),
            CommandResult::ShareWorkspace => WorkspaceAction::ShareWorkspace,
            CommandResult::SetProvider(provider_name) => {
                use crate::components::util::AiProvider;
                if let Some(provider) = AiProvider::parse(&provider_name) {
                    self.agent_panel.set_provider(provider);
                    log::info!("Set AI provider to: {}", provider.display_name());
                } else {
                    log::warn!("Unknown AI provider: {provider_name}. Use 'claude' or 'codex'.");
                }
                WorkspaceAction::None
            }
            CommandResult::SetRefresh(interval_str) => {
                let interval = RefreshInterval::parse(&interval_str);
                self.set_refresh_interval(interval);
                if interval == RefreshInterval::Off {
                    WorkspaceAction::Notify {
                        level: "info".to_string(),
                        message: "Auto-refresh disabled".to_string(),
                    }
                } else {
                    WorkspaceAction::Notify {
                        level: "info".to_string(),
                        message: format!("Auto-refresh set to {}", interval.label()),
                    }
                }
            }
            CommandResult::OpenLogs => {
                // Use a default time range of the last hour for the logs pane
                let now_ns = crate::util::now_unix_secs() * 1_000_000_000;
                let one_hour_ns = 3600 * 1_000_000_000;
                self.add_logs_pane(now_ns - one_hour_ns, now_ns);
                WorkspaceAction::None
            }
            CommandResult::OpenLoki(url) => {
                // Connect to a real Loki server
                let now_ns = crate::util::now_unix_secs() * 1_000_000_000;
                let one_hour_ns = 3600 * 1_000_000_000;
                self.add_loki_pane(now_ns - one_hour_ns, now_ns, url);
                WorkspaceAction::None
            }
            CommandResult::TeamDemo => WorkspaceAction::ToggleTeamDemo,
            CommandResult::TeamConnect { url, token } => {
                WorkspaceAction::TeamConnect { url, token }
            }
            CommandResult::TeamDisconnect => WorkspaceAction::TeamDisconnect,
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
    // Section Folding Operations (for collapsible sections)
    // =========================================================================

    /// Check if sections are active (workspace uses sections format)
    pub fn has_sections(&self) -> bool {
        !self.section_configs.is_empty()
    }

    /// Check if current section focus is at the left edge (for channels panel transfer)
    ///
    /// Returns true when:
    /// - Focus is on section header 0 (first section)
    /// - Focus is on pane 0 of section 0 (first pane of first section)
    /// - No focus (will transfer to channels panel)
    pub fn is_at_section_left_edge(&self) -> bool {
        matches!(
            self.section_focus,
            FocusTarget::None
                | FocusTarget::SectionHeader(0)
                | FocusTarget::Pane {
                    section: 0,
                    pane: 0
                }
        )
    }

    /// Check if current section focus is at the right edge (for agent panel transfer)
    ///
    /// Returns true when:
    /// - Focus is on any section header (section headers span the full width)
    /// - Focus is on the last pane of ANY section (rightmost pane in that section)
    pub fn is_at_section_right_edge(&self) -> bool {
        match self.section_focus {
            FocusTarget::None => false,
            // Any section header is at the right edge (headers span full width)
            FocusTarget::SectionHeader(_) => true,
            // Check if we're on the last pane of the current section
            FocusTarget::Pane { section, pane } => self
                .section_configs
                .get(section)
                .map(|s| pane == s.panes.len().saturating_sub(1))
                .unwrap_or(false),
        }
    }

    /// Navigate in a direction within sections (hjkl navigation)
    /// Returns true if navigation was handled, false if sections are not active
    pub fn navigate_sections(&mut self, direction: NavDirection) -> bool {
        if !self.has_sections() {
            return false;
        }

        match self.section_focus {
            FocusTarget::None => {
                // No focus - focus first pane of first expanded section
                self.section_focus = self.first_focusable_target();
            }
            FocusTarget::SectionHeader(section_idx) => {
                self.navigate_from_section_header(section_idx, direction);
            }
            FocusTarget::Pane { section, pane } => {
                self.navigate_from_pane(section, pane, direction);
            }
        }

        // Sync behavior.focused_tile() with section_focus for compatibility
        // with features that rely on tile-based focus (visual-multi, etc.)
        let tile_id = self.section_focus_to_tile_id();
        self.behavior.set_focused_tile(tile_id);

        log::debug!("Section navigation: focus is now {:?}", self.section_focus);
        true
    }

    /// Convert current section_focus to a tile ID (if focusing a pane)
    fn section_focus_to_tile_id(&self) -> Option<egui_tiles::TileId> {
        if let FocusTarget::Pane { section, pane } = self.section_focus {
            // Calculate the flat pane index from section + pane
            let mut flat_idx = 0;
            for (s_idx, section_config) in self.section_configs.iter().enumerate() {
                if s_idx == section {
                    flat_idx += pane;
                    break;
                }
                flat_idx += section_config.panes.len();
            }
            // Get the tile ID at that index
            let pane_ids = self.get_pane_tile_ids();
            pane_ids.get(flat_idx).copied()
        } else {
            None // Section headers don't have tile IDs
        }
    }

    /// Get the first focusable target (first pane of first expanded section, or first header)
    fn first_focusable_target(&self) -> FocusTarget {
        for (section_idx, state) in self.section_states.iter().enumerate() {
            if !state.collapsed {
                if let Some(section) = self.section_configs.get(section_idx) {
                    if !section.panes.is_empty() {
                        return FocusTarget::Pane {
                            section: section_idx,
                            pane: 0,
                        };
                    }
                }
            }
        }
        // All sections collapsed or empty - focus first header
        if !self.section_configs.is_empty() {
            FocusTarget::SectionHeader(0)
        } else {
            FocusTarget::None
        }
    }

    /// Navigate from a section header
    fn navigate_from_section_header(&mut self, section_idx: usize, direction: NavDirection) {
        match direction {
            NavDirection::Down => {
                // Down from header -> enter section (first pane) or next header
                if let Some(state) = self.section_states.get(section_idx) {
                    if !state.collapsed {
                        if let Some(section) = self.section_configs.get(section_idx) {
                            if !section.panes.is_empty() {
                                self.section_focus = FocusTarget::Pane {
                                    section: section_idx,
                                    pane: 0,
                                };
                                return;
                            }
                        }
                    }
                }
                // Section is collapsed or empty - go to next header
                if section_idx + 1 < self.section_configs.len() {
                    self.section_focus = FocusTarget::SectionHeader(section_idx + 1);
                }
            }
            NavDirection::Up => {
                // Up from header -> previous section's last pane or previous header
                if section_idx > 0 {
                    let prev_idx = section_idx - 1;
                    if let Some(state) = self.section_states.get(prev_idx) {
                        if !state.collapsed {
                            if let Some(section) = self.section_configs.get(prev_idx) {
                                if !section.panes.is_empty() {
                                    self.section_focus = FocusTarget::Pane {
                                        section: prev_idx,
                                        pane: section.panes.len() - 1,
                                    };
                                    return;
                                }
                            }
                        }
                    }
                    // Previous section is collapsed or empty
                    self.section_focus = FocusTarget::SectionHeader(prev_idx);
                }
            }
            NavDirection::Left => {
                // Left on header - collapse it (like vim zc)
                if let Some(state) = self.section_states.get_mut(section_idx) {
                    state.collapse();
                }
            }
            NavDirection::Right => {
                // Right on header - expand it and enter (like vim zo then l)
                if let Some(state) = self.section_states.get_mut(section_idx) {
                    state.expand();
                }
                // Enter the section's first pane
                if let Some(section) = self.section_configs.get(section_idx) {
                    if !section.panes.is_empty() {
                        self.section_focus = FocusTarget::Pane {
                            section: section_idx,
                            pane: 0,
                        };
                    }
                }
            }
        }
    }

    /// Navigate from a pane within a section
    fn navigate_from_pane(&mut self, section_idx: usize, pane_idx: usize, direction: NavDirection) {
        let section = match self.section_configs.get(section_idx) {
            Some(s) => s,
            None => return,
        };

        let pane_count = section.panes.len();

        match direction {
            NavDirection::Left => {
                if pane_idx > 0 {
                    // Move to previous pane in section
                    self.section_focus = FocusTarget::Pane {
                        section: section_idx,
                        pane: pane_idx - 1,
                    };
                } else {
                    // At first pane - go to section header
                    self.section_focus = FocusTarget::SectionHeader(section_idx);
                }
            }
            NavDirection::Right => {
                if pane_idx + 1 < pane_count {
                    // Move to next pane in section
                    self.section_focus = FocusTarget::Pane {
                        section: section_idx,
                        pane: pane_idx + 1,
                    };
                }
                // At last pane - stay (could optionally go to next section)
            }
            NavDirection::Up => {
                // For vertical/grid layouts, might want to go up within section
                // For now, go to section header or previous section's last pane
                if pane_idx == 0 {
                    // At first pane - go to header
                    self.section_focus = FocusTarget::SectionHeader(section_idx);
                } else {
                    // Try to move up within grid layout
                    let columns = section.columns.unwrap_or(2);
                    if pane_idx >= columns {
                        self.section_focus = FocusTarget::Pane {
                            section: section_idx,
                            pane: pane_idx - columns,
                        };
                    } else {
                        // Top row - go to header
                        self.section_focus = FocusTarget::SectionHeader(section_idx);
                    }
                }
            }
            NavDirection::Down => {
                // For vertical/grid layouts, move down within section or to next section
                let columns = section.columns.unwrap_or(2);
                let next_pane = pane_idx + columns;

                if next_pane < pane_count {
                    // Can move down within section
                    self.section_focus = FocusTarget::Pane {
                        section: section_idx,
                        pane: next_pane,
                    };
                } else {
                    // At bottom - go to next section
                    self.go_to_next_section_from_pane(section_idx);
                }
            }
        }
    }

    /// Move to the next section after the current one
    fn go_to_next_section_from_pane(&mut self, current_section: usize) {
        let next_section = current_section + 1;
        if next_section >= self.section_configs.len() {
            return; // Already at last section
        }

        // Check if next section is expanded and has panes
        if let Some(state) = self.section_states.get(next_section) {
            if !state.collapsed {
                if let Some(section) = self.section_configs.get(next_section) {
                    if !section.panes.is_empty() {
                        self.section_focus = FocusTarget::Pane {
                            section: next_section,
                            pane: 0,
                        };
                        return;
                    }
                }
            }
        }
        // Next section is collapsed or empty - focus its header
        self.section_focus = FocusTarget::SectionHeader(next_section);
    }

    /// Get team collaboration status (for status line display)
    pub fn team_status(&self) -> Option<TeamStatusInfo> {
        self.team_status.clone()
    }

    /// Set team collaboration status (called when connection status changes)
    pub fn set_team_status(&mut self, status: Option<TeamStatusInfo>) {
        self.team_status = status;
    }

    /// Set team members for the team menu (called from EnyaApp with TeamState data)
    pub fn set_team_members(&mut self, members: Vec<TeamMember>) {
        self.team_members = members;
    }

    /// Show channels panel when team mode becomes active.
    pub fn show_channels_panel(&mut self) {
        if self.team_status.is_some() && !self.channels_panel_visible {
            self.channels_panel_visible = true;
        }
    }

    /// Hide channels panel (called when disconnecting from team).
    pub fn hide_channels_panel(&mut self) {
        self.channels_panel_visible = false;
        self.channels_panel_focused = false;
        self.channels_panel.set_focus(false);
    }

    /// Toggle channels panel sidebar visibility
    pub fn toggle_channels_panel(&mut self) {
        self.channels_panel_visible = !self.channels_panel_visible;
        // Reset focus state when hiding
        if !self.channels_panel_visible {
            self.channels_panel_focused = false;
            self.channels_panel.set_focus(false);
        }
    }

    /// Check if channels panel is visible
    pub fn is_channels_panel_visible(&self) -> bool {
        self.channels_panel_visible
    }

    /// Toggle team menu visibility
    pub fn toggle_team_menu(&mut self) {
        self.team_menu.toggle();
    }

    /// Open annotation editor for the focused pane.
    /// Uses current time as the default annotation point.
    pub fn open_annotation_editor(&mut self) {
        // Get current time as default timestamp
        let timestamp = crate::util::now_unix_secs() as f64;
        // TODO: Get current user name from TeamState
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

    /// Get the current agent provider name (e.g., "Claude", "Codex")
    pub fn agent_provider_name(&self) -> String {
        self.agent_panel.provider_name()
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
        // Enter agent mode first
        self.enter_agent_mode();

        // Build context from selected panes (or focused pane if none selected)
        if self.agent_context_panes.is_empty() {
            if let Some(tile_id) = self.behavior.focused_tile() {
                self.agent_context_panes.insert(tile_id);
                self.sync_agent_context_panes();
            }
        }

        // Build context info for the query
        let context_info: Vec<String> = self
            .agent_context_panes
            .iter()
            .filter_map(|&tile_id| {
                if let Some(egui_tiles::Tile::Pane(component)) =
                    self.viewport_tree.tiles.get(tile_id)
                {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        let name = query_pane.name();
                        let query_text = query_pane.query();
                        return Some(format!("Pane '{name}': {query_text}"));
                    }
                }
                None
            })
            .collect();

        let context = if context_info.is_empty() {
            None
        } else {
            Some(format!("Context panes:\n{}", context_info.join("\n")))
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
                log::info!("Handing off conversation to agent panel");
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
                        AgentCommand::CreatePane { query, title }
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
                log::info!("Agent command executed (no response text), exiting agent mode");
                self.exit_agent_mode();
            }
        }
    }

    /// Send a query to the agent with current context panes
    fn send_agent_query_with_context(&mut self, query: &str, ctx: &egui::Context) {
        // Build full editor context (includes available commands documentation)
        let editor_context = self.build_editor_context();
        let editor_context_block = editor_context
            .as_ref()
            .map(|c| c.to_prompt_block())
            .unwrap_or_default();

        // Build context from selected panes
        let context_info: Vec<String> = self
            .agent_context_panes
            .iter()
            .filter_map(|&tile_id| {
                if let Some(egui_tiles::Tile::Pane(component)) =
                    self.viewport_tree.tiles.get(tile_id)
                {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        // Get pane details for context
                        let name = query_pane.name();
                        let query_text = query_pane.query();
                        return Some(format!("Pane '{name}': {query_text}"));
                    }
                }
                None
            })
            .collect();

        // Build full context string with editor context and pane context
        let context = {
            let mut parts = vec![editor_context_block];
            if !context_info.is_empty() {
                parts.push(format!("\n## Selected Panes\n{}", context_info.join("\n")));
            }
            Some(parts.join("\n"))
        };

        // Send query directly to input bar (it handles AI communication)
        log::info!(
            "Sending query to agent: '{}' with context ({} chars)",
            query,
            context.as_ref().map(|c| c.len()).unwrap_or(0)
        );
        self.agent_input_bar.send_query(query, context.as_deref());

        ctx.request_repaint();
        log::debug!(
            "Sent query to agent with {} context panes",
            self.agent_context_panes.len()
        );
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
            EditorContext, build_connection_context, build_dashboard_context,
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

            build_dashboard_context(time_range.preset.label().to_string(), pane_count, queries)
        };

        // Build the full context
        let context = EditorContext::new()
            .with_connection(connection)
            .with_metrics(metrics)
            .with_dashboard(dashboard);

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
            EditorContext, build_connection_context, build_dashboard_context,
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

            build_dashboard_context(time_range.preset.label().to_string(), pane_count, queries)
        };

        // Build the full context
        let context = EditorContext::new()
            .with_connection(connection)
            .with_metrics(metrics)
            .with_dashboard(dashboard);

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
}
