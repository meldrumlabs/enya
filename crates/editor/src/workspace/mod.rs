use std::collections::{HashMap, HashSet};

use egui_tiles::{Tile, TileId, Tiles};

use crate::app::AppState;
use crate::components::{
    Buffer, BufferEditor, BufferEditorResult, CommandPalette, CommandResult, Component,
    DiagnosticsPane, InfoOverlay, LandingPage, LandingPageAction, MetricItem, MetricsFinder,
    MultiBufferMode, MultiBufferState, MultiEditOverlay, MultiEditResult, QueryExecutor, QueryPane,
    QueryState, TimeRangeToolbar, TutorialOverlay, ViewportFilter, ViewportFilterResult, WhichKey,
    WorkspaceFinder, WorkspaceItem,
};
use crate::theme::AppTheme;
use crate::ui::colors::text_color;

// Workspace configuration module (serialization)
pub mod config;

// Input handling (navigation, visual-multi mode)
mod input;
pub use input::{NavDirection, VisualMultiState};

// Tile tree behavior (egui_tiles integration)
mod tiles;
use tiles::TreeBehavior;

// Keyboard input handling
mod keyboard;

// Overlay management (diagnostics, etc.)
mod overlays;

// Query execution coordination
mod query;

// Workspace serialization/deserialization
mod serialization;

// Re-export config types for convenience
pub use config::{
    COMPLEX_VIEWPORT_TOML, ConnectionConfig, DEFAULT_WORKSPACE_TOML, DEMO_WORKSPACE_TOML,
    LayoutConfig, LayoutContainer, LayoutNode, LayoutType, PaneConfig, TimeConfig, ViewConfig,
    WORKSPACE_VERSION, WorkspaceConfig, WorkspaceError, WorkspaceMeta,
};

/// Actions that the Workspace needs the App to handle
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceAction {
    /// No action needed
    None,
    /// Toggle the theme
    ToggleTheme,
    /// Set a specific theme
    SetTheme(AppTheme),
    /// Show help
    ShowHelp,
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
    /// Create a new workspace tab
    NewWorkspaceTab(Option<String>),
    /// Close current workspace tab
    CloseWorkspaceTab,
    /// Go to next workspace tab
    NextWorkspaceTab,
    /// Go to previous workspace tab
    PrevWorkspaceTab,
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
    open_charts: HashSet<String>,
    /// Pending chart to add (metric name)
    pending_chart: Option<String>,
    /// Time range toolbar
    time_range_toolbar: TimeRangeToolbar,
    /// Fuzzy finder modal for metrics (telescope-style search)
    metrics_finder: MetricsFinder,
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
    /// Last time 'y' was pressed (for yy detection)
    last_y_press: Option<crate::util::Instant>,
    /// Last time 'c' was pressed (for cv detection - cycle visualization)
    last_c_press: Option<crate::util::Instant>,
    /// Last time Space was pressed (for leader key sequences like Space+m, Space+q)
    last_space_press: Option<crate::util::Instant>,
    /// Info overlay (shows build/version info)
    info_overlay: InfoOverlay,
    /// Which-key overlay (shows available keybindings)
    which_key: WhichKey,
    /// Tutorial overlay (interactive walkthrough)
    tutorial_overlay: TutorialOverlay,
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
    /// Query executor for running queries against backends (Prometheus, Enya)
    query_executor: QueryExecutor,
    /// Track which pane is waiting for a query result
    pending_query_tile: Option<TileId>,
    /// Counter for sequential query pane naming (Query 1, Query 2, ...)
    next_query_number: usize,
    /// Workspace filter for filtering visible panes by query content
    viewport_filter: ViewportFilter,
}

impl Default for Workspace {
    fn default() -> Self {
        let mut tiles: Tiles<Box<dyn Component>> = egui_tiles::Tiles::default();
        let tabs = Vec::new();
        let root = tiles.insert_tab_tile(tabs);

        let viewport_tree = egui_tiles::Tree::new("viewport_tree", root, tiles);
        Self {
            viewport_tree,
            behavior: TreeBehavior::default(),
            open_charts: HashSet::new(),
            pending_chart: None,
            time_range_toolbar: TimeRangeToolbar::new(),
            metrics_finder: MetricsFinder::new(),
            workspace_finder: WorkspaceFinder::new(),
            command_palette: CommandPalette::new(),
            buffer_editor: BufferEditor::new(),
            editing_tile_id: None,
            zen_mode: false,
            fullscreen_tile: None,
            landing_page: LandingPage::new(),
            show_landing: true,
            last_y_press: None,
            last_c_press: None,
            last_space_press: None,
            info_overlay: InfoOverlay::new(enya_build_info::build_info!()),
            which_key: WhichKey::new(),
            tutorial_overlay: TutorialOverlay::new(),
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
            query_executor: QueryExecutor::new(),
            pending_query_tile: None,
            next_query_number: 1,
            viewport_filter: ViewportFilter::new(),
        }
    }
}

impl Workspace {
    /// Create a new empty dashboard (no landing page)
    pub fn new_empty() -> Self {
        let mut dashboard = Self::example(String::new());
        dashboard.show_landing = false;
        dashboard
    }

    pub fn example(_api_key: String) -> Self {
        let mut tiles: Tiles<Box<dyn Component>> = egui_tiles::Tiles::default();

        // Start with empty tabs - show landing page first
        let root = tiles.insert_tab_tile(vec![]);

        let viewport_tree = egui_tiles::Tree::new("viewport_tree", root, tiles);

        Self {
            viewport_tree,
            behavior: TreeBehavior::default(),
            open_charts: HashSet::new(),
            pending_chart: None,
            time_range_toolbar: TimeRangeToolbar::new(),
            metrics_finder: MetricsFinder::new(),
            workspace_finder: WorkspaceFinder::new(),
            command_palette: CommandPalette::new(),
            buffer_editor: BufferEditor::new(),
            editing_tile_id: None,
            zen_mode: false,
            fullscreen_tile: None,
            landing_page: LandingPage::new(),
            show_landing: true, // Start with landing page
            last_y_press: None,
            last_c_press: None,
            last_space_press: None,
            info_overlay: InfoOverlay::new(enya_build_info::build_info!()),
            which_key: WhichKey::new(),
            tutorial_overlay: TutorialOverlay::new(),
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
            query_executor: QueryExecutor::new(),
            pending_query_tile: None,
            next_query_number: 1,
            viewport_filter: ViewportFilter::new(),
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        app_state: &AppState,
    ) -> WorkspaceAction {
        self.behavior.set_theme(app_state.theme);
        self.behavior
            .set_keys(app_state.settings.api_key.to_owned());

        // Process query execution: poll for results and execute pending queries
        let query_action = self.process_query_execution(ctx);
        if query_action != WorkspaceAction::None {
            return query_action;
        }

        // Sync visual-multi state to behavior for rendering
        let (is_visual_multi, selected_ids, tile_queries) = match &self.visual_multi_state {
            Some(state) => {
                // Build query mapping for selected tiles
                let mut queries = HashMap::new();
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
            None => (false, HashSet::new(), HashMap::new()),
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
            HashSet::new()
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

        // Show landing page only if explicitly enabled and no charts open
        // (new workspaces start with show_landing=false for a clean empty state)
        if self.show_landing && self.open_charts.is_empty() {
            return self.show_landing_page(ui, ctx, app_state);
        }

        // Main area with toolbar and viewport
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Top toolbar with time range controls (hidden in zen mode)
            if !self.zen_mode {
                egui::TopBottomPanel::top("time_range_toolbar")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            // Time range controls
                            self.time_range_toolbar.show(ui);
                        });
                        ui.add_space(4.0);
                    });
            }

            // Main viewport area (tabbed charts/views)
            egui::CentralPanel::default().show_inside(ui, |ui| {
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
                } else {
                    // Store available rect before layout for scrollbar positioning
                    let full_rect = ui.available_rect_before_wrap();

                    // Scrollbar dimensions
                    const SCROLLBAR_WIDTH: f32 = 10.0; // Width including padding

                    // Calculate if we need scrolling
                    const MIN_PANE_HEIGHT: f32 = 300.0;
                    let pane_count = self.get_pane_tile_ids().len();
                    let min_content_height = pane_count as f32 * MIN_PANE_HEIGHT;
                    let needs_scrollbar = min_content_height > full_rect.height();

                    // Layout: main content area + scrollbar gutter on right
                    ui.horizontal(|ui| {
                        // Main viewport area (takes remaining space minus scrollbar)
                        let viewport_width = if needs_scrollbar {
                            full_rect.width() - SCROLLBAR_WIDTH
                        } else {
                            full_rect.width()
                        };

                        ui.allocate_ui_with_layout(
                            egui::vec2(viewport_width, full_rect.height()),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.viewport_visible_height = ui.available_height();

                                // Animate scroll offset towards target (smooth scrolling)
                                let scroll_speed = 12.0; // Higher = faster animation
                                let dt = ctx.input(|i| i.predicted_dt);
                                let diff =
                                    self.viewport_scroll_target - self.viewport_scroll_offset;
                                if diff.abs() > 0.5 {
                                    self.viewport_scroll_offset += diff * scroll_speed * dt;
                                    ctx.request_repaint();
                                } else {
                                    self.viewport_scroll_offset = self.viewport_scroll_target;
                                }

                                // Set tree height to enable scrolling when content exceeds viewport
                                if min_content_height > self.viewport_visible_height {
                                    self.viewport_tree.set_height(min_content_height);
                                } else {
                                    self.viewport_tree.set_height(f32::INFINITY);
                                }

                                // Create ScrollArea with controlled offset
                                let scroll_output = egui::ScrollArea::vertical()
                                    .id_salt("viewport_scroll")
                                    .scroll_bar_visibility(
                                        egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                    )
                                    .vertical_scroll_offset(self.viewport_scroll_offset)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        self.viewport_tree.ui(&mut self.behavior, ui);
                                    });

                                // Update scroll state from ScrollArea output
                                self.viewport_content_height = scroll_output.content_size.y;

                                // Sync scroll offset if user scrolled with mouse
                                let current_offset = scroll_output.state.offset.y;
                                if (current_offset - self.viewport_scroll_offset).abs() > 1.0 {
                                    self.viewport_scroll_offset = current_offset;
                                    self.viewport_scroll_target = current_offset;
                                }
                            },
                        );

                        // Scrollbar gutter on the right (only if needed)
                        if needs_scrollbar {
                            ui.allocate_ui_with_layout(
                                egui::vec2(SCROLLBAR_WIDTH, full_rect.height()),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    let scrollbar_rect = ui.available_rect_before_wrap();
                                    self.draw_scrollbar(
                                        ui.painter(),
                                        scrollbar_rect,
                                        app_state.theme,
                                    );
                                },
                            );
                        }
                    });
                }
            });
        });

        // Show fuzzy finder modal (rendered on top of everything)
        self.metrics_finder.set_theme(app_state.theme);
        if let Some(selected_item) = self.metrics_finder.show(ctx) {
            return self.handle_metric_selection_with_tracking(selected_item);
        }

        // Show workspace finder modal (rendered on top of everything)
        self.workspace_finder.set_theme(app_state.theme);
        if let Some(selected_workspace) = self.workspace_finder.show(ctx) {
            return WorkspaceAction::LoadWorkspace(selected_workspace);
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

        // Show diagnostics overlay modal
        self.diagnostics_pane.set_theme(app_state.theme);
        self.diagnostics_pane.show_overlay(ctx);

        // Show viewport filter overlay and handle results
        self.viewport_filter.set_theme(app_state.theme);
        // Update filter counts before showing
        let (match_count, total_count) = self.count_filtered_panes();
        self.viewport_filter.update_counts(match_count, total_count);
        match self.viewport_filter.show(ctx) {
            ViewportFilterResult::Applied(pattern) => {
                log::debug!("Workspace filter applied: {pattern}");
            }
            ViewportFilterResult::Cleared => {
                log::debug!("Workspace filter cleared");
            }
            ViewportFilterResult::None => {}
        }

        // Handle / key for viewport filter (vim-style search)
        // NOTE: Must run BEFORE the ? handler since both use the Slash key
        if !self.which_key.is_open()
            && !self.metrics_finder.is_open()
            && !self.command_palette.is_open()
            && !self.buffer_editor.is_open()
            && !self.viewport_filter.is_open()
            && !self.is_any_buffer_in_insert_mode()
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

        // Handle ? key for which-key overlay (bypasses focus check so it works even with chart focus)
        if !self.which_key.is_open()
            && !self.metrics_finder.is_open()
            && !self.command_palette.is_open()
            && !self.buffer_editor.is_open()
            && !self.viewport_filter.is_open()
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
        // Show the landing page in the central panel
        let mut landing_action = LandingPageAction::None;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            landing_action = self.landing_page.show(
                ui,
                ctx,
                &app_state.settings.recent_plots,
                &app_state.settings.recent_workspaces,
            );
        });

        // Handle landing page actions
        match landing_action {
            LandingPageAction::OpenPlot {
                metric_name,
                is_query: _,
            } => {
                self.show_landing = false;
                // Open the metric directly (queries are now handled via fuzzy finder)
                self.pending_chart = Some(metric_name);
            }
            LandingPageAction::OpenWorkspace { name } => {
                return WorkspaceAction::LoadWorkspace(name);
            }
            LandingPageAction::OpenFuzzyFinder => {
                self.open_metrics_finder();
            }
            LandingPageAction::OpenWorkspaceFinder => {
                self.open_workspace_finder(
                    app_state,
                    crate::app::EnyaApp::list_available_workspaces(),
                );
            }
            LandingPageAction::ShowHelp => {
                self.which_key.open();
            }
            LandingPageAction::OpenConnect => {
                self.open_command_palette_with_text("connect ");
            }
            LandingPageAction::OpenTutorial => {
                // Hide landing page and add demo panes for the tutorial
                self.show_landing = false;
                let demo_queries = [
                    (
                        "http_requests_total{env=\"prod\", service=\"api\"}",
                        "HTTP Requests",
                    ),
                    ("cpu_usage{env=\"prod\", service=\"api\"}", "CPU Usage"),
                    (
                        "memory_used_bytes{env=\"prod\", service=\"api\"}",
                        "Memory Used",
                    ),
                ];
                for (query, name) in demo_queries {
                    self.add_demo_query_pane(query, name);
                }
                self.tutorial_overlay.open();
                ctx.request_repaint();
            }
            LandingPageAction::None => {}
        }

        // Show fuzzy finder modal (rendered on top of everything)
        self.metrics_finder.set_theme(app_state.theme);
        if let Some(selected_item) = self.metrics_finder.show(ctx) {
            return self.handle_metric_selection_with_tracking(selected_item);
        }

        // Show workspace finder modal (rendered on top of everything)
        self.workspace_finder.set_theme(app_state.theme);
        if let Some(selected_workspace) = self.workspace_finder.show(ctx) {
            return WorkspaceAction::LoadWorkspace(selected_workspace);
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

        // Show diagnostics overlay modal
        self.diagnostics_pane.set_theme(app_state.theme);
        self.diagnostics_pane.show_overlay(ctx);

        // Handle Space+d for diagnostics on landing page
        // (viewport keyboard handling doesn't run on landing page)
        if !self.metrics_finder.is_open()
            && !self.workspace_finder.is_open()
            && !self.command_palette.is_open()
            && !self.which_key.is_open()
            && !self.diagnostics_pane.is_open()
        {
            ctx.input_mut(|input| {
                // Space - leader key for sequences
                if input.consume_key(egui::Modifiers::NONE, egui::Key::Space) {
                    self.last_space_press = Some(crate::util::Instant::now());
                }

                // Leader key sequences (must follow Space within 500ms)
                let space_active = self.last_space_press.is_some_and(|last| {
                    crate::util::Instant::now().duration_since(last).as_millis() < 500
                });

                if space_active {
                    // Space+d - toggle diagnostics overlay
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::D) {
                        self.diagnostics_pane.toggle();
                        self.diagnostics_visible = self.diagnostics_pane.is_open();
                        self.last_space_press = None;
                    }
                }
            });
        }

        self.handle_command_result(cmd_result, ctx)
    }

    /// Add a chart for a metric and return a tracking action
    fn add_chart_for_metric_with_tracking(&mut self, metric_name: &str) -> WorkspaceAction {
        // Don't add duplicate charts
        if self.open_charts.contains(metric_name) {
            log::debug!("Chart for {metric_name} already open");
            return WorkspaceAction::None;
        }

        // Create a QueryPane (buffer + chart) for the metric
        // Use real query pane when connected to a backend, demo pane otherwise
        let query_number = self.next_query_number;
        self.next_query_number += 1;
        let pane: Box<dyn Component> = if self.query_executor.is_connected() {
            Box::new(QueryPane::for_metric_with_number(metric_name, query_number))
        } else {
            Box::new(QueryPane::with_demo_metric_numbered(
                metric_name,
                query_number,
            ))
        };
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.open_charts.insert(metric_name.to_string());
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::debug!("Added query pane for {metric_name}");

            // Return action to track this in recent queries
            // Use "Query N" as the display name, metric_name for lookup
            return WorkspaceAction::TrackRecentPlot {
                name: format!("Query {query_number}"),
                metric_name: metric_name.to_string(),
                is_query: false,
            };
        }

        WorkspaceAction::None
    }

    /// Add a demo query pane with a full PromQL query and custom name (for tutorial)
    fn add_demo_query_pane(&mut self, query: &str, name: &str) {
        let pane: Box<dyn Component> = Box::new(QueryPane::with_demo_query_named(query, name));
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.open_charts.insert(query.to_string());
            self.behavior.set_focused_tile(Some(pane_tile));
            log::debug!("Added demo query pane: {name}");
        }
    }

    /// Handle fuzzy selection (metrics only) and return tracking action
    fn handle_metric_selection_with_tracking(&mut self, item: MetricItem) -> WorkspaceAction {
        self.show_landing = false;
        self.add_chart_for_metric_with_tracking(&item.name)
    }

    /// Handle a command result from the command palette
    fn handle_command_result(
        &mut self,
        result: CommandResult,
        ctx: &egui::Context,
    ) -> WorkspaceAction {
        match result {
            CommandResult::ToggleTheme => WorkspaceAction::ToggleTheme,
            CommandResult::SetTheme(theme) => WorkspaceAction::SetTheme(theme),
            CommandResult::OpenSearch => {
                self.open_metrics_finder();
                WorkspaceAction::None
            }
            CommandResult::ShowInfo => {
                self.info_overlay.open();
                WorkspaceAction::None
            }
            CommandResult::ShowHelp => WorkspaceAction::ShowHelp,
            CommandResult::CloseTab => {
                // Close the focused tile
                if let Some(tile_id) = self.behavior.focused_tile() {
                    self.close_tile(tile_id);
                }
                WorkspaceAction::None
            }
            CommandResult::QuitApp => WorkspaceAction::QuitApp,
            CommandResult::SplitHorizontal => {
                self.split_panes_horizontal();
                WorkspaceAction::None
            }
            CommandResult::SplitVertical => {
                self.split_panes_vertical();
                WorkspaceAction::None
            }
            CommandResult::ToggleZenMode => {
                self.toggle_zen_mode();
                WorkspaceAction::None
            }
            CommandResult::ToggleFullscreen => {
                self.toggle_fullscreen();
                WorkspaceAction::None
            }
            CommandResult::ShowLandingPage => {
                self.show_landing = true;
                // Close all charts to trigger landing page display
                self.close_all_charts();
                WorkspaceAction::None
            }
            CommandResult::TakeScreenshot(path) => WorkspaceAction::TakeScreenshot(path),
            CommandResult::SaveWorkspace(name) => WorkspaceAction::SaveWorkspace(name),
            CommandResult::LoadWorkspace(name) => WorkspaceAction::LoadWorkspace(name),
            CommandResult::ListWorkspaces => WorkspaceAction::ListWorkspaces,
            CommandResult::ShareWorkspace => WorkspaceAction::ShareWorkspace,
            CommandResult::ToggleCommits => {
                self.toggle_commits_on_focused();
                WorkspaceAction::None
            }
            CommandResult::Connect(endpoint) => {
                self.query_executor.connect_prometheus(&endpoint, ctx);
                // Immediately start fetching metric names and label names
                self.query_executor.fetch_metric_names(ctx);
                self.query_executor.fetch_label_names(ctx);
                // No notification here - health check result will show success/failure
                WorkspaceAction::None
            }
            CommandResult::Disconnect => {
                self.query_executor.disconnect();
                WorkspaceAction::Notify {
                    level: "info".to_string(),
                    message: "Disconnected from Prometheus, using demo data".to_string(),
                }
            }
            CommandResult::ToggleDiagnostics => {
                self.toggle_diagnostics();
                WorkspaceAction::None
            }
            CommandResult::ShowDiagnostics => {
                self.show_diagnostics();
                WorkspaceAction::None
            }
            CommandResult::HideDiagnostics => {
                self.hide_diagnostics();
                WorkspaceAction::None
            }
            CommandResult::ClearDiagnostics => {
                self.clear_diagnostics();
                WorkspaceAction::Notify {
                    level: "info".to_string(),
                    message: "Cleared all diagnostics".to_string(),
                }
            }
            CommandResult::NextDiagnostic => {
                self.diagnostics_pane.select_next();
                // Show notification with current diagnostic
                if let Some(pane_id) = self.diagnostics_pane.selected_pane_id() {
                    // Focus the pane associated with the diagnostic
                    if let Some(tile_id) = self.find_tile_by_pane_id(pane_id) {
                        self.behavior.set_focused_tile(Some(tile_id));
                    }
                }
                WorkspaceAction::None
            }
            CommandResult::PrevDiagnostic => {
                self.diagnostics_pane.select_prev();
                // Focus the pane associated with the diagnostic
                if let Some(pane_id) = self.diagnostics_pane.selected_pane_id() {
                    if let Some(tile_id) = self.find_tile_by_pane_id(pane_id) {
                        self.behavior.set_focused_tile(Some(tile_id));
                    }
                }
                WorkspaceAction::None
            }
            CommandResult::NewWorkspaceTab(name) => WorkspaceAction::NewWorkspaceTab(name),
            CommandResult::CloseWorkspaceTab => WorkspaceAction::CloseWorkspaceTab,
            CommandResult::NextWorkspaceTab => WorkspaceAction::NextWorkspaceTab,
            CommandResult::PrevWorkspaceTab => WorkspaceAction::PrevWorkspaceTab,
            CommandResult::OpenTutorial => {
                // Hide landing page and add multiple demo panes so users have something to interact with
                if self.show_landing || self.open_charts.is_empty() {
                    self.show_landing = false;
                    // Add multiple demo query panes with PromQL label selectors
                    // These use env="prod" so users can practice multi-edit to change to "staging"
                    let demo_queries = [
                        (
                            "http_requests_total{env=\"prod\", service=\"api\"}",
                            "HTTP Requests",
                        ),
                        ("cpu_usage{env=\"prod\", service=\"api\"}", "CPU Usage"),
                        (
                            "memory_used_bytes{env=\"prod\", service=\"api\"}",
                            "Memory Used",
                        ),
                    ];
                    for (query, name) in demo_queries {
                        self.add_demo_query_pane(query, name);
                    }
                }
                self.tutorial_overlay.open();
                ctx.request_repaint();
                WorkspaceAction::None
            }
            CommandResult::Success | CommandResult::Error(_) | CommandResult::None => {
                WorkspaceAction::None
            }
        }
    }

    /// Find a tile by the pane's component ID
    fn find_tile_by_pane_id(&self, pane_id: usize) -> Option<TileId> {
        for tile_id in self.get_pane_tile_ids() {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                if component.id() == pane_id {
                    return Some(tile_id);
                }
            }
        }
        None
    }

    /// Toggle commit markers on the focused chart
    fn toggle_commits_on_focused(&mut self) {
        if let Some(tile_id) = self.behavior.focused_tile() {
            if let Some(egui_tiles::Tile::Pane(component)) =
                self.viewport_tree.tiles.get_mut(tile_id)
            {
                if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                    query_pane.toggle_commits();
                    log::debug!("Toggled commit markers");
                }
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
                }
            }
        }
    }

    /// Open the metrics finder modal (for metrics only)
    pub fn open_metrics_finder(&mut self) {
        let items = if self.query_executor.is_connected() {
            // Use real metrics from Prometheus
            self.prometheus_metric_items()
        } else {
            // Fall back to demo metrics
            Self::demo_metric_items()
        };
        self.metrics_finder.set_items(items);
        self.metrics_finder.open();
    }

    /// Generate metric items from Prometheus metric names
    fn prometheus_metric_items(&self) -> Vec<MetricItem> {
        use std::collections::{HashMap, HashSet};

        self.query_executor
            .metric_names()
            .iter()
            .map(|name| {
                // Infer category from metric name prefix (common Prometheus conventions)
                let category = Self::infer_prometheus_category(name);

                // Check if we have cached labels for this metric
                let tags: HashMap<String, HashSet<String>> =
                    if let Some(labels) = self.query_executor.get_metric_labels(name) {
                        // Use actual per-metric labels
                        labels
                            .labels
                            .iter()
                            .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
                            .collect()
                    } else {
                        // No labels cached yet - show empty (will be fetched on selection)
                        HashMap::new()
                    };

                MetricItem {
                    name: name.clone(),
                    category,
                    description: None,
                    unit: None,
                    tags,
                    series_count: 0,
                }
            })
            .collect()
    }

    /// Infer category from Prometheus metric name conventions
    fn infer_prometheus_category(name: &str) -> String {
        // Common Prometheus metric prefixes
        if name.starts_with("node_") {
            "Node Exporter".to_string()
        } else if name.starts_with("go_") {
            "Go Runtime".to_string()
        } else if name.starts_with("process_") {
            "Process".to_string()
        } else if name.starts_with("promhttp_") || name.starts_with("prometheus_") {
            "Prometheus".to_string()
        } else if name.starts_with("http_") {
            "HTTP".to_string()
        } else if name.starts_with("grpc_") {
            "gRPC".to_string()
        } else if name.starts_with("scrape_") {
            "Scrape".to_string()
        } else if name.starts_with("up") || name == "up" {
            "Target".to_string()
        } else {
            // Default: extract first part before underscore
            name.split('_')
                .next()
                .map(|s| {
                    let mut chars = s.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().chain(chars).collect(),
                    }
                })
                .unwrap_or_else(|| "Metrics".to_string())
        }
    }

    /// Generate demo metric items for the fuzzy finder
    fn demo_metric_items() -> Vec<MetricItem> {
        use std::collections::{HashMap, HashSet};

        let mut items = Vec::new();

        // Tokio metrics
        for name in [
            "tokio.runtime.total_park_count",
            "tokio.runtime.blocking_queue_depth",
            "tokio.runtime.num_remote_schedules",
            "tokio.runtime.budget_forced_yield_count",
            "tokio.runtime.io_driver_ready_count",
            "tokio.runtime.mean_poll_duration_ns",
        ] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "Tokio Runtime".to_string(),
                description: None,
                unit: None,
                tags: HashMap::new(),
                series_count: 0,
            });
        }

        // Task metrics with tags
        let task_tags: HashMap<String, HashSet<String>> = [(
            "task".to_string(),
            ["ingestor", "query_handler", "compactor"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )]
        .into_iter()
        .collect();

        for name in [
            "task.poll.count",
            "task.poll.duration_ns",
            "task.poll.slow_count",
            "task.idle.duration_ns",
            "task.scheduled.duration_ns",
        ] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "Tasks".to_string(),
                description: None,
                unit: None,
                tags: task_tags.clone(),
                series_count: 3,
            });
        }

        // DataFusion metrics
        for name in [
            "datafusion.query.execution_time_ns",
            "datafusion.query.rows_produced",
            "datafusion.memory.pool_size",
        ] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "DataFusion".to_string(),
                description: None,
                unit: None,
                tags: HashMap::new(),
                series_count: 0,
            });
        }

        // System metrics
        let host_tags: HashMap<String, HashSet<String>> = [(
            "host".to_string(),
            ["server1", "server2", "server3"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )]
        .into_iter()
        .collect();

        items.push(MetricItem {
            name: "cpu.usage".to_string(),
            category: "System".to_string(),
            description: None,
            unit: None,
            tags: host_tags,
            series_count: 3,
        });

        for name in ["memory.used", "memory.available"] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "System".to_string(),
                description: None,
                unit: None,
                tags: HashMap::new(),
                series_count: 0,
            });
        }

        // Application metrics
        let app_tags: HashMap<String, HashSet<String>> = [
            (
                "env".to_string(),
                ["prod", "staging"].iter().map(|s| s.to_string()).collect(),
            ),
            (
                "service".to_string(),
                ["api", "web"].iter().map(|s| s.to_string()).collect(),
            ),
        ]
        .into_iter()
        .collect();

        items.push(MetricItem {
            name: "http.requests".to_string(),
            category: "Application".to_string(),
            description: None,
            unit: None,
            tags: app_tags,
            series_count: 4,
        });

        for name in ["request.count", "request.latency"] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "Application".to_string(),
                description: None,
                unit: None,
                tags: HashMap::new(),
                series_count: 0,
            });
        }

        items
    }

    /// Open the workspace finder modal (for loading saved workspaces)
    pub fn open_workspace_finder(
        &mut self,
        app_state: &AppState,
        available_workspaces: Vec<(String, Option<String>)>,
    ) {
        // Start with recent workspaces
        let mut workspaces: Vec<WorkspaceItem> = app_state
            .settings
            .recent_workspaces
            .iter()
            .map(|entry| WorkspaceItem {
                name: entry.name.clone(),
                description: if entry.description.is_empty() {
                    None
                } else {
                    Some(entry.description.clone())
                },
            })
            .collect();

        // Track names already in the list
        let existing_names: HashSet<String> = workspaces.iter().map(|w| w.name.clone()).collect();

        // Add available workspaces from filesystem that aren't already in recent
        for (name, description) in available_workspaces {
            if !existing_names.contains(&name) {
                workspaces.push(WorkspaceItem { name, description });
            }
        }

        self.workspace_finder.set_workspaces(workspaces);
        self.workspace_finder.open();
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

    /// Add a tile to the viewport, handling different container types
    /// Returns true if the tile was successfully added
    fn add_tile_to_viewport(&mut self, tile_id: TileId) -> bool {
        let Some(root_id) = self.viewport_tree.root() else {
            // No root exists (all panes were closed), create a new tabs container
            let new_root = self.viewport_tree.tiles.insert_tab_tile(vec![tile_id]);
            self.viewport_tree.root = Some(new_root);
            return true;
        };

        match self.viewport_tree.tiles.get_mut(root_id) {
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) => {
                tabs.add_child(tile_id);
                tabs.set_active(tile_id);
                true
            }
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) => {
                linear.add_child(tile_id);
                true
            }
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Grid(grid))) => {
                grid.add_child(tile_id);
                true
            }
            _ => false,
        }
    }

    /// Close a tile and remove it from the viewport
    fn close_tile(&mut self, tile_id: TileId) {
        // Get the pane's label before removing it (for open_charts tracking)
        let label = if let Some(egui_tiles::Tile::Pane(component)) =
            self.viewport_tree.tiles.get(tile_id)
        {
            Some(component.label().text().to_string())
        } else {
            None
        };

        // Find the next tile to focus before removing
        let pane_ids = self.get_pane_tile_ids();
        let next_focus = if pane_ids.len() > 1 {
            // Try to find a sibling to focus
            self.find_sibling_in_direction(tile_id, NavDirection::Right)
                .or_else(|| self.find_sibling_in_direction(tile_id, NavDirection::Left))
                .or_else(|| self.find_sibling_in_direction(tile_id, NavDirection::Down))
                .or_else(|| self.find_sibling_in_direction(tile_id, NavDirection::Up))
                .or_else(|| pane_ids.iter().find(|&&id| id != tile_id).copied())
        } else {
            None
        };

        // Remove the tile from the tree
        self.viewport_tree.tiles.remove(tile_id);

        // Remove from open_charts tracking
        if let Some(label) = label {
            self.open_charts.remove(&label);
            // Also try removing with query: prefix
            self.open_charts.remove(&format!("query:{label}"));
            log::debug!("Closed tile: {label}");
        }

        // Update focus to next tile
        self.behavior.set_focused_tile(next_focus);
    }

    /// Close all charts and reset the viewport to show landing page
    fn close_all_charts(&mut self) {
        // Get all pane tile IDs and close them
        let pane_ids = self.get_pane_tile_ids();
        for tile_id in pane_ids {
            self.viewport_tree.tiles.remove(tile_id);
        }

        // Clear tracking
        self.open_charts.clear();
        self.behavior.set_focused_tile(None);
        self.fullscreen_tile = None;
        self.zen_mode = false;

        log::debug!("Closed all charts, showing landing page");
    }

    /// Check if the command palette is currently open
    pub fn is_command_palette_open(&self) -> bool {
        self.command_palette.is_open()
    }

    /// Check if the fuzzy finder is currently open
    pub fn is_metrics_finder_open(&self) -> bool {
        self.metrics_finder.is_open()
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

    /// Split panes horizontally (`:split` - panes stacked vertically, one above another)
    fn split_panes_horizontal(&mut self) {
        let pane_ids = self.get_pane_tile_ids();
        if pane_ids.len() < 2 {
            log::debug!("Need at least 2 panes to split");
            return;
        }

        // Preserve focus on the currently focused pane, or first pane
        let focus_pane = self
            .behavior
            .focused_tile()
            .filter(|id| pane_ids.contains(id))
            .or_else(|| pane_ids.first().copied());

        // Create a new vertical container (panes stacked on top of each other)
        let new_root = self.viewport_tree.tiles.insert_vertical_tile(pane_ids);
        self.viewport_tree.root = Some(new_root);

        // Restore focus
        self.behavior.set_focused_tile(focus_pane);
        log::debug!("Split panes horizontally (vertical layout)");
    }

    /// Split panes vertically (`:vsplit` - panes side by side)
    fn split_panes_vertical(&mut self) {
        let pane_ids = self.get_pane_tile_ids();
        if pane_ids.len() < 2 {
            log::debug!("Need at least 2 panes to split");
            return;
        }

        // Preserve focus on the currently focused pane, or first pane
        let focus_pane = self
            .behavior
            .focused_tile()
            .filter(|id| pane_ids.contains(id))
            .or_else(|| pane_ids.first().copied());

        // Create a new horizontal container (panes side by side)
        let new_root = self.viewport_tree.tiles.insert_horizontal_tile(pane_ids);
        self.viewport_tree.root = Some(new_root);

        // Restore focus
        self.behavior.set_focused_tile(focus_pane);
        log::debug!("Split panes vertically (horizontal layout)");
    }

    /// Get all pane tile IDs in the viewport (for navigation)
    fn get_pane_tile_ids(&self) -> Vec<TileId> {
        let mut pane_ids = Vec::new();

        if let Some(root_id) = self.viewport_tree.root() {
            self.collect_pane_ids(root_id, &mut pane_ids);
        }

        pane_ids
    }

    /// Count how many panes match the current filter and total panes
    fn count_filtered_panes(&self) -> (usize, usize) {
        let pane_ids = self.get_pane_tile_ids();
        let total = pane_ids.len();

        if !self.viewport_filter.is_active() {
            return (total, total);
        }

        let matching = pane_ids
            .iter()
            .filter(|&&tile_id| {
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                    // Check QueryPane - match on query content OR tag
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        return self.viewport_filter.matches(query_pane.saved_query())
                            || self.viewport_filter.matches(query_pane.tag());
                    }
                    // Check Buffer
                    if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                        return self.viewport_filter.matches(buffer.saved_content());
                    }
                }
                true // Unknown component types are always shown
            })
            .count();

        (matching, total)
    }

    /// Recursively collect all pane tile IDs
    fn collect_pane_ids(&self, tile_id: TileId, pane_ids: &mut Vec<TileId>) {
        if let Some(tile) = self.viewport_tree.tiles.get(tile_id) {
            match tile {
                Tile::Pane(_) => {
                    pane_ids.push(tile_id);
                }
                Tile::Container(container) => {
                    for child_id in container.children() {
                        self.collect_pane_ids(*child_id, pane_ids);
                    }
                }
            }
        }
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

    /// Activate a tile (make it the active tab in its parent container)
    fn activate_tile(&mut self, tile_id: TileId) {
        // Find the parent tabs container and set this tile as active
        if let Some(root_id) = self.viewport_tree.root() {
            self.activate_tile_in_container(root_id, tile_id);
        }
    }

    /// Recursively find and activate a tile in its parent tabs container
    fn activate_tile_in_container(&mut self, container_id: TileId, target_id: TileId) -> bool {
        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(container_id) {
            let children: Vec<TileId> = container.children().copied().collect();

            // Check if target is a direct child
            if children.contains(&target_id) {
                // Set this tile as active in the tabs container
                if let Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                    self.viewport_tree.tiles.get_mut(container_id)
                {
                    tabs.set_active(target_id);
                    return true;
                }
            }

            // Recursively search children
            for child_id in children {
                if self.activate_tile_in_container(child_id, target_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Render only matching panes when viewport filter is active
    fn render_filtered_view(&mut self, ui: &mut egui::Ui) {
        // Get matching pane IDs - matches on query content AND tag
        let matching_panes: Vec<TileId> = self
            .get_pane_tile_ids()
            .into_iter()
            .filter(|&tile_id| {
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        // Match on query content OR tag
                        return self.viewport_filter.matches(query_pane.saved_query())
                            || self.viewport_filter.matches(query_pane.tag());
                    }
                    if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                        return self.viewport_filter.matches(buffer.saved_content());
                    }
                }
                true
            })
            .collect();

        if matching_panes.is_empty() {
            // Show "no matches" message
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("No panes match the filter")
                        .color(text_color(self.behavior.theme()).gamma_multiply(0.5))
                        .size(16.0),
                );
            });
            return;
        }

        // Calculate grid layout
        let available = ui.available_size();
        let pane_count = matching_panes.len();

        // Determine columns based on pane count and available width
        let columns = if pane_count == 1 {
            1
        } else if pane_count <= 4 {
            2.min(pane_count)
        } else {
            3.min(pane_count)
        };

        let rows = pane_count.div_ceil(columns);

        let pane_width = (available.x - (columns as f32 - 1.0) * 8.0) / columns as f32;
        let pane_height = ((available.y - (rows as f32 - 1.0) * 8.0) / rows as f32).max(200.0);

        egui::ScrollArea::vertical()
            .id_salt("filtered_view_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("filtered_panes_grid")
                    .num_columns(columns)
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        for (idx, &tile_id) in matching_panes.iter().enumerate() {
                            if let Some(Tile::Pane(component)) =
                                self.viewport_tree.tiles.get_mut(tile_id)
                            {
                                component.set_theme(self.behavior.theme());
                                component.set_api_key(self.behavior.api_key());

                                // Render pane with constrained size (no extra frame)
                                ui.allocate_ui(egui::vec2(pane_width - 8.0, pane_height), |ui| {
                                    component.show(ui);
                                });
                            }

                            // End row after 'columns' panes
                            if (idx + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });
    }

    /// Draw nvim-style scrollbar indicator in the scrollbar gutter
    fn draw_scrollbar(&self, painter: &egui::Painter, gutter_rect: egui::Rect, theme: AppTheme) {
        // Only draw if content is taller than visible area
        if self.viewport_content_height <= self.viewport_visible_height {
            return;
        }

        // Scrollbar dimensions - slim and elegant
        let scrollbar_width = 4.0;
        let margin_vertical = 8.0;
        let scrollbar_x = gutter_rect.center().x - scrollbar_width / 2.0;

        // Calculate scrollbar track area
        let track_top = gutter_rect.top() + margin_vertical;
        let track_bottom = gutter_rect.bottom() - margin_vertical;
        let track_height = track_bottom - track_top;

        // Calculate thumb position and size
        let visible_ratio = self.viewport_visible_height / self.viewport_content_height;
        let thumb_height = (track_height * visible_ratio).max(24.0); // Minimum thumb size

        let max_scroll = self.viewport_content_height - self.viewport_visible_height;
        let scroll_ratio = if max_scroll > 0.0 {
            (self.viewport_scroll_offset / max_scroll).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_top = track_top + (track_height - thumb_height) * scroll_ratio;

        let track_rect = egui::Rect::from_min_size(
            egui::pos2(scrollbar_x, track_top),
            egui::vec2(scrollbar_width, track_height),
        );

        let thumb_rect = egui::Rect::from_min_size(
            egui::pos2(scrollbar_x, thumb_top),
            egui::vec2(scrollbar_width, thumb_height),
        );

        // Theme-aware colors
        let (track_color, thumb_color, thumb_highlight) = match theme {
            AppTheme::Light => (
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 15),
                egui::Color32::from_rgba_unmultiplied(80, 80, 90, 140),
                egui::Color32::from_rgba_unmultiplied(60, 60, 70, 180),
            ),
            AppTheme::Dark => (
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8),
                egui::Color32::from_rgba_unmultiplied(140, 140, 160, 120),
                egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160),
            ),
        };

        // Draw track with rounded ends
        painter.rect_filled(track_rect, scrollbar_width / 2.0, track_color);

        // Draw thumb with subtle gradient effect (using layered rectangles)
        // Base thumb
        painter.rect_filled(thumb_rect, scrollbar_width / 2.0, thumb_color);

        // Inner highlight (slightly smaller, brighter) for depth
        let highlight_inset = 0.5;
        let highlight_rect = thumb_rect.shrink2(egui::vec2(highlight_inset, 1.0));
        painter.rect_filled(
            highlight_rect,
            (scrollbar_width - highlight_inset * 2.0) / 2.0,
            thumb_highlight,
        );

        // Top cap highlight for a glossy effect
        let cap_height = 3.0_f32.min(thumb_height / 4.0);
        let cap_rect =
            egui::Rect::from_min_size(thumb_rect.min, egui::vec2(scrollbar_width, cap_height));
        let cap_color = match theme {
            AppTheme::Light => egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40),
            AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25),
        };
        painter.rect_filled(cap_rect, scrollbar_width / 2.0, cap_color);
    }

    /// Scroll viewport to make the focused tile visible
    fn scroll_to_focused_tile(&mut self, ctx: &egui::Context) {
        let focused_id = match self.behavior.focused_tile() {
            Some(id) => id,
            None => return,
        };

        // Get all pane IDs in order
        let pane_ids = self.get_pane_tile_ids();
        if pane_ids.is_empty() {
            return;
        }

        // Find the index of the focused pane
        let focused_index = match pane_ids.iter().position(|&id| id == focused_id) {
            Some(idx) => idx,
            None => return,
        };

        // Calculate approximate position of the focused tile
        // Assume each pane takes equal height for simplicity
        let pane_count = pane_ids.len();
        if pane_count == 0 {
            return;
        }

        let pane_height = self.viewport_content_height / pane_count as f32;
        let target_top = focused_index as f32 * pane_height;
        let target_bottom = target_top + pane_height;

        // Calculate scroll target to bring tile into view (with some padding)
        let padding = 20.0;
        let view_top = self.viewport_scroll_offset;
        let view_bottom = view_top + self.viewport_visible_height;

        if target_top < view_top + padding {
            // Tile is above the visible area, scroll up
            self.viewport_scroll_target = (target_top - padding).max(0.0);
            ctx.request_repaint();
        } else if target_bottom > view_bottom - padding {
            // Tile is below the visible area, scroll down
            let max_scroll = (self.viewport_content_height - self.viewport_visible_height).max(0.0);
            self.viewport_scroll_target =
                (target_bottom - self.viewport_visible_height + padding).clamp(0.0, max_scroll);
            ctx.request_repaint();
        }
    }
}
