use std::collections::{HashMap, HashSet};

use egui_tiles::{SimplificationOptions, Tile, TileId, Tiles};

use crate::app::AppState;

use crate::components::{
    Buffer, BufferEditor, BufferEditorResult, BufferMode, CommandPalette, CommandResult, Component,
    Diagnostic, DiagnosticSource, DiagnosticsPane, EditExcerpt, ExecuteParams, InfoOverlay,
    LandingPage, LandingPageAction, MetricItem, MetricsFinder, MultiBufferMode, MultiBufferState,
    MultiEditOverlay, MultiEditResult, QueryExecutor, QueryPane, QueryPollResult, QueryState,
    TimeRangeToolbar, ViewportFilter, ViewportFilterResult, WhichKey, WorkspaceFinder,
    WorkspaceItem,
};
use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;

use crate::workspace::{
    ConnectionConfig, LayoutConfig, LayoutContainer, LayoutNode, LayoutType, PaneConfig,
    TimeConfig, ViewConfig, Workspace, WorkspaceMeta,
};

/// Actions that the Dashboard needs the App to handle
#[derive(Debug, Clone, PartialEq)]
pub enum DashboardAction {
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

/// The main dashboard layout with a flexible viewport for tabbed views/charts.
pub struct Dashboard {
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
    /// Viewport filter for filtering visible panes by query content
    viewport_filter: ViewportFilter,
}

impl Default for Dashboard {
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

/// Direction for vim-style navigation
#[derive(Debug, Clone, Copy, PartialEq)]
enum NavDirection {
    Left,
    Right,
    Up,
    Down,
}

/// State for visual multi-select mode
/// Allows selecting multiple panes for batch operations (e.g., find & replace across queries)
#[derive(Debug, Clone, Default)]
pub struct VisualMultiState {
    /// The panes that are currently selected
    pub selected_tile_ids: HashSet<TileId>,
    /// The pane that currently has the cursor (for j/k navigation)
    pub cursor_tile_id: Option<TileId>,
}

impl VisualMultiState {
    /// Create a new visual multi state with the given starting pane
    pub fn new(starting_tile_id: TileId) -> Self {
        let mut selected = HashSet::new();
        selected.insert(starting_tile_id);
        Self {
            selected_tile_ids: selected,
            cursor_tile_id: Some(starting_tile_id),
        }
    }

    /// Toggle selection of a pane
    pub fn toggle_selection(&mut self, tile_id: TileId) {
        if self.selected_tile_ids.contains(&tile_id) {
            self.selected_tile_ids.remove(&tile_id);
        } else {
            self.selected_tile_ids.insert(tile_id);
        }
    }

    /// Check if a pane is selected
    pub fn is_selected(&self, tile_id: TileId) -> bool {
        self.selected_tile_ids.contains(&tile_id)
    }

    /// Get the number of selected panes
    pub fn selection_count(&self) -> usize {
        self.selected_tile_ids.len()
    }

    /// Move cursor to a new pane
    pub fn set_cursor(&mut self, tile_id: TileId) {
        self.cursor_tile_id = Some(tile_id);
    }

    /// Select all given panes
    pub fn select_all(&mut self, tile_ids: &[TileId]) {
        for &tile_id in tile_ids {
            self.selected_tile_ids.insert(tile_id);
        }
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        self.selected_tile_ids.clear();
    }
}

impl Dashboard {
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
    ) -> DashboardAction {
        self.behavior.set_theme(app_state.theme);
        self.behavior
            .set_keys(app_state.settings.api_key.to_owned());

        // Process query execution: poll for results and execute pending queries
        let query_action = self.process_query_execution(ctx);
        if query_action != DashboardAction::None {
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
            if action != DashboardAction::None {
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
                        component.set_theme(self.behavior.theme);
                        component.set_api_key(&self.behavior.api_key);
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
            return DashboardAction::LoadWorkspace(selected_workspace);
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
                log::debug!("Viewport filter applied: {pattern}");
            }
            ViewportFilterResult::Cleared => {
                log::debug!("Viewport filter cleared");
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
    ) -> DashboardAction {
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
                return DashboardAction::LoadWorkspace(name);
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
            return DashboardAction::LoadWorkspace(selected_workspace);
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
    fn add_chart_for_metric_with_tracking(&mut self, metric_name: &str) -> DashboardAction {
        // Don't add duplicate charts
        if self.open_charts.contains(metric_name) {
            log::debug!("Chart for {metric_name} already open");
            return DashboardAction::None;
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

            // Return action to track this in recent plots
            return DashboardAction::TrackRecentPlot {
                name: metric_name.to_string(),
                metric_name: metric_name.to_string(),
                is_query: false,
            };
        }

        DashboardAction::None
    }

    /// Handle fuzzy selection (metrics only) and return tracking action
    fn handle_metric_selection_with_tracking(&mut self, item: MetricItem) -> DashboardAction {
        self.show_landing = false;
        self.add_chart_for_metric_with_tracking(&item.name)
    }

    /// Handle a command result from the command palette
    fn handle_command_result(
        &mut self,
        result: CommandResult,
        ctx: &egui::Context,
    ) -> DashboardAction {
        match result {
            CommandResult::ToggleTheme => DashboardAction::ToggleTheme,
            CommandResult::SetTheme(theme) => DashboardAction::SetTheme(theme),
            CommandResult::OpenSearch => {
                self.open_metrics_finder();
                DashboardAction::None
            }
            CommandResult::ShowInfo => {
                self.info_overlay.open();
                DashboardAction::None
            }
            CommandResult::ShowHelp => DashboardAction::ShowHelp,
            CommandResult::CloseTab => {
                // Close the focused tile
                if let Some(tile_id) = self.behavior.focused_tile() {
                    self.close_tile(tile_id);
                }
                DashboardAction::None
            }
            CommandResult::QuitApp => DashboardAction::QuitApp,
            CommandResult::SplitHorizontal => {
                self.split_panes_horizontal();
                DashboardAction::None
            }
            CommandResult::SplitVertical => {
                self.split_panes_vertical();
                DashboardAction::None
            }
            CommandResult::ToggleZenMode => {
                self.toggle_zen_mode();
                DashboardAction::None
            }
            CommandResult::ToggleFullscreen => {
                self.toggle_fullscreen();
                DashboardAction::None
            }
            CommandResult::ShowLandingPage => {
                self.show_landing = true;
                // Close all charts to trigger landing page display
                self.close_all_charts();
                DashboardAction::None
            }
            CommandResult::TakeScreenshot(path) => DashboardAction::TakeScreenshot(path),
            CommandResult::SaveWorkspace(name) => DashboardAction::SaveWorkspace(name),
            CommandResult::LoadWorkspace(name) => DashboardAction::LoadWorkspace(name),
            CommandResult::ListWorkspaces => DashboardAction::ListWorkspaces,
            CommandResult::ShareWorkspace => DashboardAction::ShareWorkspace,
            CommandResult::ToggleCommits => {
                self.toggle_commits_on_focused();
                DashboardAction::None
            }
            CommandResult::Connect(endpoint) => {
                self.query_executor.connect_prometheus(&endpoint, ctx);
                // Immediately start fetching metric names and label names
                self.query_executor.fetch_metric_names(ctx);
                self.query_executor.fetch_label_names(ctx);
                // No notification here - health check result will show success/failure
                DashboardAction::None
            }
            CommandResult::Disconnect => {
                self.query_executor.disconnect();
                DashboardAction::Notify {
                    level: "info".to_string(),
                    message: "Disconnected from Prometheus, using demo data".to_string(),
                }
            }
            CommandResult::ToggleDiagnostics => {
                self.toggle_diagnostics();
                DashboardAction::None
            }
            CommandResult::ShowDiagnostics => {
                self.show_diagnostics();
                DashboardAction::None
            }
            CommandResult::HideDiagnostics => {
                self.hide_diagnostics();
                DashboardAction::None
            }
            CommandResult::ClearDiagnostics => {
                self.clear_diagnostics();
                DashboardAction::Notify {
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
                DashboardAction::None
            }
            CommandResult::PrevDiagnostic => {
                self.diagnostics_pane.select_prev();
                // Focus the pane associated with the diagnostic
                if let Some(pane_id) = self.diagnostics_pane.selected_pane_id() {
                    if let Some(tile_id) = self.find_tile_by_pane_id(pane_id) {
                        self.behavior.set_focused_tile(Some(tile_id));
                    }
                }
                DashboardAction::None
            }
            CommandResult::NewWorkspaceTab(name) => DashboardAction::NewWorkspaceTab(name),
            CommandResult::CloseWorkspaceTab => DashboardAction::CloseWorkspaceTab,
            CommandResult::NextWorkspaceTab => DashboardAction::NextWorkspaceTab,
            CommandResult::PrevWorkspaceTab => DashboardAction::PrevWorkspaceTab,
            CommandResult::Success | CommandResult::Error(_) | CommandResult::None => {
                DashboardAction::None
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

    /// Process query execution: poll for pending results and execute queries for panes that need refresh
    /// Returns a notification action if a connection status changed.
    fn process_query_execution(&mut self, ctx: &egui::Context) -> DashboardAction {
        // 0. Poll for health check completion
        let mut notification_action = DashboardAction::None;
        if let Some(success) = self.query_executor.poll_health_check() {
            if success {
                if let super::components::query_executor::ConnectionHealth::Online { ref version } =
                    self.query_executor.connection_health().clone()
                {
                    log::info!("Connected to Prometheus v{version}");
                    // Add success diagnostic
                    let diagnostic = super::components::diagnostics_pane::Diagnostic::info(
                        format!("Connected to Prometheus v{version}"),
                    )
                    .with_source(
                        super::components::diagnostics_pane::DiagnosticSource::DataConnection,
                    );
                    self.diagnostics_pane.add(diagnostic);
                    // Show success notification
                    notification_action = DashboardAction::Notify {
                        level: "success".to_string(),
                        message: format!("Connected to Prometheus v{version}"),
                    };
                }
            } else if let super::components::query_executor::ConnectionHealth::Failed {
                ref error,
            } = self.query_executor.connection_health().clone()
            {
                log::error!("Connection failed: {error}");
                // Add error diagnostic
                let diagnostic = super::components::diagnostics_pane::Diagnostic::error(format!(
                    "Connection failed: {error}"
                ))
                .with_source(super::components::diagnostics_pane::DiagnosticSource::DataConnection);
                self.diagnostics_pane.add(diagnostic);
                // Show error notification
                notification_action = DashboardAction::Notify {
                    level: "error".to_string(),
                    message: format!("Connection failed: {error}"),
                };
            }
        }

        // 0a. Poll for metric names and label names fetch completion
        if self.query_executor.poll_metric_names() {
            // Update buffer editor if it's open
            if self.buffer_editor.is_open() {
                let metric_names = self.query_executor.metric_names().to_vec();
                log::debug!(
                    "Updating buffer editor with {} newly fetched metric names",
                    metric_names.len()
                );
                self.buffer_editor.set_metric_names(metric_names);
            }
        }
        self.query_executor.poll_label_names();

        // 0b. Poll for per-metric labels and update the finder/buffer editor if labels were received
        if let Some(metric_name) = self.query_executor.poll_metric_labels() {
            // Convert MetricLabels to HashMap<String, HashSet<String>> for the finder
            if let Some(labels) = self.query_executor.get_metric_labels(&metric_name) {
                let tags: std::collections::HashMap<String, std::collections::HashSet<String>> =
                    labels
                        .labels
                        .iter()
                        .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
                        .collect();
                self.metrics_finder.update_metric_tags(&metric_name, tags);

                // Also update buffer editor completions if editing this metric
                if self.buffer_editor.editing_metric_name() == Some(metric_name.as_str()) {
                    self.buffer_editor
                        .set_completions_from_labels(&labels.labels);
                    log::debug!(
                        "Updated buffer editor completions from {} labels for '{}'",
                        labels.labels.len(),
                        metric_name
                    );
                }
            }
        }

        // 0c. If metrics finder is open and connected, fetch labels for selected metric
        if self.metrics_finder.is_open() && self.query_executor.is_connected() {
            if let Some(metric_name) = self.metrics_finder.selected_metric_name() {
                // Only fetch if not already cached and not currently fetching this metric
                if !self.query_executor.has_metric_labels(metric_name)
                    && self.query_executor.fetching_metric() != Some(metric_name)
                {
                    self.query_executor.fetch_metric_labels(metric_name, ctx);
                }
            }
        }

        // 0d. If buffer editor is open and connected, fetch labels for the metric being edited
        if self.buffer_editor.is_open() && self.query_executor.is_connected() {
            if let Some(metric_name) = self.buffer_editor.editing_metric_name() {
                // Only fetch if not already cached and not currently fetching this metric
                if !self.query_executor.has_metric_labels(metric_name)
                    && self.query_executor.fetching_metric() != Some(metric_name)
                {
                    self.query_executor.fetch_metric_labels(metric_name, ctx);
                }
            }
        }

        // 1. Poll for query results if there's a pending query
        if let Some(tile_id) = self.pending_query_tile {
            if let Some(egui_tiles::Tile::Pane(component)) =
                self.viewport_tree.tiles.get_mut(tile_id)
            {
                if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                    let pane_id = query_pane.id();
                    let pane_name = query_pane.name().to_string();

                    match self.query_executor.poll(query_pane.visualization_mut()) {
                        QueryPollResult::Complete {
                            series_count,
                            point_count,
                        } => {
                            // Query completed
                            self.pending_query_tile = None;
                            query_pane.set_loading(false);
                            // Clear any previous errors for this pane
                            self.diagnostics_pane.clear_for_pane(pane_id);

                            if series_count == 0 || point_count == 0 {
                                // Query succeeded but returned no data - add info diagnostic
                                let diagnostic = Diagnostic::info(
                                    "Query returned no data. Check the metric name and time range.",
                                )
                                .with_source(DiagnosticSource::DataConnection)
                                .with_pane(pane_id, &pane_name);
                                self.diagnostics_pane.add(diagnostic);
                                log::info!(
                                    "Query for tile {tile_id:?} returned no data (0 series, 0 points)"
                                );
                            } else {
                                log::debug!(
                                    "Query completed for tile {tile_id:?}: {series_count} series, {point_count} points"
                                );
                            }
                        }
                        QueryPollResult::Error(error) => {
                            // Query failed - add diagnostic
                            self.pending_query_tile = None;
                            query_pane.set_loading(false);
                            // Clear previous diagnostics for this pane and add the new error
                            self.diagnostics_pane.clear_for_pane(pane_id);
                            let diagnostic = Diagnostic::error(&error)
                                .with_source(DiagnosticSource::DataConnection)
                                .with_pane(pane_id, &pane_name);
                            self.diagnostics_pane.add(diagnostic);
                            log::error!("Query failed for tile {tile_id:?}: {error}");
                        }
                        QueryPollResult::Pending => {
                            // Still waiting for results
                        }
                    }
                }
            }
        }

        // 2. If no query in flight, check for panes that need refresh and execute
        if self.pending_query_tile.is_none() {
            let (start_ns, end_ns) = self.time_range_toolbar.get_range_ns();

            // Find the first pane that needs refresh
            let pane_ids: Vec<TileId> = self
                .viewport_tree
                .tiles
                .iter()
                .filter_map(|(id, tile)| {
                    if let egui_tiles::Tile::Pane(component) = tile {
                        if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                            if query_pane.needs_refresh() {
                                log::debug!(
                                    "Found pane {:?} that needs refresh: {}",
                                    id,
                                    query_pane.name()
                                );
                                return Some(*id);
                            }
                        }
                    }
                    None
                })
                .collect();

            // Execute query for the first pane that needs refresh
            if let Some(tile_id) = pane_ids.first().copied() {
                if let Some(egui_tiles::Tile::Pane(component)) =
                    self.viewport_tree.tiles.get_mut(tile_id)
                {
                    if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                        // Get query parameters from the pane
                        let metric = query_pane.name().to_string();
                        let query = query_pane.saved_query().to_string();
                        let step_secs = query_pane.query_state().granularity.seconds();

                        // Clear the refresh flag
                        query_pane.clear_refresh();

                        // Execute the query
                        let params = ExecuteParams {
                            metric: &metric,
                            query: &query,
                            step_secs,
                            start_ns: Some(start_ns),
                            end_ns: Some(end_ns),
                        };
                        self.query_executor
                            .execute(&params, query_pane.visualization_mut(), ctx);
                        self.pending_query_tile = Some(tile_id);
                        query_pane.set_loading(true);

                        log::debug!("Executing query for tile {tile_id:?}: {query}");
                    }
                }
            }
        }

        notification_action
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

    // ==================== Diagnostics Methods ====================

    /// Toggle the diagnostics overlay visibility
    pub fn toggle_diagnostics(&mut self) {
        self.diagnostics_pane.toggle();
        self.diagnostics_visible = self.diagnostics_pane.is_open();
    }

    /// Show the diagnostics overlay
    pub fn show_diagnostics(&mut self) {
        self.diagnostics_pane.open();
        self.diagnostics_visible = true;
    }

    /// Hide the diagnostics overlay
    pub fn hide_diagnostics(&mut self) {
        self.diagnostics_pane.close();
        self.diagnostics_visible = false;
    }

    /// Add a diagnostic to the diagnostics pane
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics_pane.add(diagnostic);
    }

    /// Clear all diagnostics
    pub fn clear_diagnostics(&mut self) {
        self.diagnostics_pane.clear();
    }

    /// Clear diagnostics for a specific pane
    pub fn clear_diagnostics_for_pane(&mut self, pane_id: usize) {
        self.diagnostics_pane.clear_for_pane(pane_id);
    }

    /// Get diagnostics count
    pub fn diagnostics_count(&self) -> usize {
        self.diagnostics_pane.count()
    }

    /// Get diagnostics count by level (errors, warnings, infos)
    pub fn diagnostics_count_by_level(&self) -> (usize, usize, usize) {
        let (errors, warnings, infos, _) = self.diagnostics_pane.count_by_level();
        (errors, warnings, infos)
    }

    /// Check if there are any errors
    pub fn has_diagnostic_errors(&self) -> bool {
        self.diagnostics_pane.has_errors()
    }

    /// Check if the diagnostics pane is visible
    pub fn is_diagnostics_visible(&self) -> bool {
        self.diagnostics_visible
    }

    // ==================== End Diagnostics Methods ====================

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

    /// Find sibling tile in a given direction, respecting container layout
    fn find_sibling_in_direction(
        &self,
        current_id: TileId,
        direction: NavDirection,
    ) -> Option<TileId> {
        // Find the parent container of the current tile
        if let Some(root_id) = self.viewport_tree.root() {
            return self.find_sibling_recursive(root_id, current_id, direction);
        }
        None
    }

    /// Recursively search for a sibling in the given direction
    fn find_sibling_recursive(
        &self,
        container_id: TileId,
        target_id: TileId,
        direction: NavDirection,
    ) -> Option<TileId> {
        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(container_id) {
            let children: Vec<TileId> = container.children().copied().collect();

            // Check if target is a direct child
            if let Some(idx) = children.iter().position(|&id| id == target_id) {
                // Determine if direction matches container orientation
                let container_kind = container.kind();
                let container_is_horizontal = matches!(
                    container_kind,
                    egui_tiles::ContainerKind::Tabs
                        | egui_tiles::ContainerKind::Horizontal
                        | egui_tiles::ContainerKind::Grid
                );
                let container_is_vertical =
                    matches!(container_kind, egui_tiles::ContainerKind::Vertical);

                let nav_is_horizontal =
                    matches!(direction, NavDirection::Left | NavDirection::Right);
                let nav_is_vertical = matches!(direction, NavDirection::Up | NavDirection::Down);

                // Navigate within this container if orientation matches
                if (container_is_horizontal && nav_is_horizontal)
                    || (container_is_vertical && nav_is_vertical)
                {
                    let next_idx = match direction {
                        NavDirection::Left | NavDirection::Up => {
                            if idx > 0 {
                                Some(idx - 1)
                            } else {
                                None
                            }
                        }
                        NavDirection::Right | NavDirection::Down => {
                            if idx + 1 < children.len() {
                                Some(idx + 1)
                            } else {
                                None
                            }
                        }
                    };

                    if let Some(next_idx) = next_idx {
                        // Get the target tile (might be a container, so get first/last pane)
                        let next_tile_id = children[next_idx];
                        return Some(self.get_edge_pane(next_tile_id, direction));
                    }
                }
                // Target is direct child but direction doesn't match container orientation
                // No sibling in this direction at this level
                return None;
            }

            // Check if target is in a nested container (target is NOT a direct child)
            for &child_id in &children {
                if child_id != target_id && self.contains_tile(child_id, target_id) {
                    // First try to find sibling within the nested container
                    if let Some(sibling) =
                        self.find_sibling_recursive(child_id, target_id, direction)
                    {
                        return Some(sibling);
                    }
                    // If not found in nested container, try to find sibling at this level
                    // by treating the nested container as the target
                    return self.find_sibling_recursive(container_id, child_id, direction);
                }
            }
        }
        None
    }

    /// Check if a container (recursively) contains a specific tile
    fn contains_tile(&self, container_id: TileId, target_id: TileId) -> bool {
        if container_id == target_id {
            return true;
        }
        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(container_id) {
            for child_id in container.children() {
                if self.contains_tile(*child_id, target_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Get the first or last pane within a tile (handles nested containers)
    fn get_edge_pane(&self, tile_id: TileId, direction: NavDirection) -> TileId {
        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(tile_id) {
            let children: Vec<TileId> = container.children().copied().collect();
            if !children.is_empty() {
                // When navigating right/down, get the first child; when left/up, get the last
                let edge_child = match direction {
                    NavDirection::Right | NavDirection::Down => children[0],
                    NavDirection::Left | NavDirection::Up => children[children.len() - 1],
                };
                return self.get_edge_pane(edge_child, direction);
            }
        }
        // It's a pane or empty container
        tile_id
    }

    /// Handle vim-style keyboard navigation for the viewport
    /// Returns an optional DashboardAction if a key triggered an action
    pub fn handle_viewport_keyboard(&mut self, ctx: &egui::Context) -> Option<DashboardAction> {
        // Don't handle keys if a text field or modal has focus
        if ctx.memory(|mem| mem.focused().is_some()) {
            return None;
        }

        // Don't handle if any modal is open
        if self.metrics_finder.is_open()
            || self.workspace_finder.is_open()
            || self.command_palette.is_open()
            || self.buffer_editor.is_open()
            || self.multi_edit_overlay.is_open()
            || self.which_key.is_open()
            || self.viewport_filter.is_open()
        {
            return None;
        }

        // Handle visual-multi mode keyboard shortcuts
        if self.visual_multi_state.is_some() {
            return self.handle_visual_multi_keyboard(ctx);
        }

        // Check if any buffer is in insert mode - if so, don't handle navigation keys
        if self.is_any_buffer_in_insert_mode() {
            return None;
        }

        let pane_ids = self.get_pane_tile_ids();
        let current_focus = self.behavior.focused_tile();

        let mut consumed = false;
        let mut should_clear_focus = false;
        let mut should_close_focused = false;
        let mut should_toggle_zen = false;
        let mut should_toggle_fullscreen = false;
        let mut should_share_pane = false;
        let mut should_open_which_key = false;
        let mut should_enter_visual_multi = false;
        let mut should_cycle_visualization = false;
        let mut should_next_workspace_tab = false;
        let mut should_prev_workspace_tab = false;
        let mut should_open_workspace_finder = false;
        let mut should_open_metrics_finder = false;
        let mut should_show_home = false;
        let mut should_toggle_diagnostics = false;
        let mut should_edit_buffer = false;
        let mut new_tile_id: Option<TileId> = None;

        ctx.input_mut(|input| {
            // yy - share focused pane (vim-style yank)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Y) && current_focus.is_some() {
                let now = crate::util::Instant::now();
                if let Some(last_press) = self.last_y_press {
                    // If second y within 500ms, trigger share
                    if now.duration_since(last_press).as_millis() < 500 {
                        should_share_pane = true;
                        self.last_y_press = None;
                        consumed = true;
                        return;
                    }
                }
                // First y - record time
                self.last_y_press = Some(now);
                consumed = true;
                return;
            }

            // cv - cycle visualization type on focused pane (time series -> stat -> ...)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::C) && current_focus.is_some() {
                let now = crate::util::Instant::now();
                // Record c press time for cv detection
                self.last_c_press = Some(now);
                consumed = true;
                return;
            }

            if input.consume_key(egui::Modifiers::NONE, egui::Key::V) && current_focus.is_some() {
                // Check if this is part of a cv sequence
                if let Some(last_press) = self.last_c_press {
                    let now = crate::util::Instant::now();
                    if now.duration_since(last_press).as_millis() < 500 {
                        should_cycle_visualization = true;
                        self.last_c_press = None;
                        consumed = true;
                        return;
                    }
                }
            }

            // N - go to next workspace tab
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::N) {
                should_next_workspace_tab = true;
                consumed = true;
                return;
            }

            // P - go to previous workspace tab
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::P) {
                should_prev_workspace_tab = true;
                consumed = true;
                return;
            }

            // Space - leader key for sequences (Space+m, Space+q, Space+w)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Space) {
                let now = crate::util::Instant::now();
                self.last_space_press = Some(now);
                consumed = true;
                return;
            }

            // Leader key sequences (must follow Space within 500ms)
            let space_active = self.last_space_press.is_some_and(|last| {
                crate::util::Instant::now().duration_since(last).as_millis() < 500
            });

            if space_active {
                // Space+m - open metrics finder
                if input.consume_key(egui::Modifiers::NONE, egui::Key::M) {
                    should_open_metrics_finder = true;
                    self.last_space_press = None;
                    consumed = true;
                    return;
                }

                // Space+w - open workspace finder
                if input.consume_key(egui::Modifiers::NONE, egui::Key::W) {
                    should_open_workspace_finder = true;
                    self.last_space_press = None;
                    consumed = true;
                    return;
                }

                // Space+h - show home/landing page
                if input.consume_key(egui::Modifiers::NONE, egui::Key::H) {
                    should_show_home = true;
                    self.last_space_press = None;
                    consumed = true;
                    return;
                }

                // Space+d - toggle diagnostics overlay
                if input.consume_key(egui::Modifiers::NONE, egui::Key::D) {
                    should_toggle_diagnostics = true;
                    self.last_space_press = None;
                    consumed = true;
                    return;
                }
            }

            // e - enter edit mode on focused pane (vim-style)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::E) && current_focus.is_some() {
                should_edit_buffer = true;
                consumed = true;
                return;
            }

            // Z - toggle zen mode (works even with no panes)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Z) {
                should_toggle_zen = true;
                consumed = true;
                return;
            }
            // F - toggle fullscreen for focused pane
            if input.consume_key(egui::Modifiers::NONE, egui::Key::F) {
                should_toggle_fullscreen = true;
                consumed = true;
                return;
            }

            // Ctrl+V - enter visual-block (multi-select) mode
            // If no pane is focused, auto-focus the first (topmost) pane
            if input.consume_key(egui::Modifiers::CTRL, egui::Key::V) {
                if current_focus.is_none() {
                    new_tile_id = pane_ids.first().copied();
                }
                should_enter_visual_multi = true;
                consumed = true;
                return;
            }

            // ? - open which-key help overlay (Shift+/ on US keyboards)
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::Slash) {
                should_open_which_key = true;
                consumed = true;
                return;
            }

            // h or left arrow - move left
            if input.consume_key(egui::Modifiers::NONE, egui::Key::H)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
            {
                if let Some(current_id) = current_focus {
                    new_tile_id = self.find_sibling_in_direction(current_id, NavDirection::Left);
                } else {
                    new_tile_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // l or right arrow - move right
            if input.consume_key(egui::Modifiers::NONE, egui::Key::L)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
            {
                if let Some(current_id) = current_focus {
                    new_tile_id = self.find_sibling_in_direction(current_id, NavDirection::Right);
                } else {
                    new_tile_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // j or down arrow - move down
            if input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                if let Some(current_id) = current_focus {
                    new_tile_id = self.find_sibling_in_direction(current_id, NavDirection::Down);
                } else {
                    new_tile_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // k or up arrow - move up
            if input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                if let Some(current_id) = current_focus {
                    new_tile_id = self.find_sibling_in_direction(current_id, NavDirection::Up);
                } else {
                    new_tile_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // x - close focused pane
            if input.consume_key(egui::Modifiers::NONE, egui::Key::X) && current_focus.is_some() {
                should_close_focused = true;
                consumed = true;
                return;
            }

            // Escape - clear focus
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                should_clear_focus = true;
                consumed = true;
            }
        });

        // Handle share pane action (yy)
        if should_share_pane {
            if let Some(tile_id) = current_focus {
                // Find the pane index for the focused tile
                if let Some(pane_index) = self.get_pane_index(tile_id) {
                    ctx.request_repaint();
                    return Some(DashboardAction::SharePane(pane_index));
                }
            }
        }

        // Handle workspace tab navigation (gt/gT)
        if should_next_workspace_tab {
            ctx.request_repaint();
            return Some(DashboardAction::NextWorkspaceTab);
        }
        if should_prev_workspace_tab {
            ctx.request_repaint();
            return Some(DashboardAction::PrevWorkspaceTab);
        }

        // Handle workspace finder (w key)
        if should_open_workspace_finder {
            self.pending_open_workspace_finder = true;
            ctx.request_repaint();
        }

        // Handle metrics finder (m key)
        if should_open_metrics_finder {
            self.open_metrics_finder();
            ctx.request_repaint();
        }

        if should_show_home {
            self.show_landing = true;
            self.close_all_charts();
            ctx.request_repaint();
        }

        if should_toggle_diagnostics {
            self.toggle_diagnostics();
            ctx.request_repaint();
        }

        if should_open_which_key {
            self.which_key.open();
        } else if should_enter_visual_multi {
            // Use the newly auto-focused tile if we set one, otherwise use current focus
            let starting_tile = new_tile_id.or(current_focus);
            if let Some(tile_id) = starting_tile {
                self.enter_visual_multi_mode(tile_id);
            }
        } else if should_edit_buffer {
            self.edit_focused_buffer();
        } else if should_cycle_visualization {
            self.cycle_focused_visualization();
        } else if should_toggle_zen {
            self.toggle_zen_mode();
        } else if should_toggle_fullscreen {
            self.toggle_fullscreen();
        } else if should_close_focused {
            if let Some(tile_id) = current_focus {
                self.close_tile(tile_id);
            }
        } else if should_clear_focus {
            self.behavior.set_focused_tile(None);
        } else if let Some(tile_id) = new_tile_id {
            // Set focus and also switch to that tab if it's in a tabs container
            self.behavior.set_focused_tile(Some(tile_id));
            self.activate_tile(tile_id);
            // Trigger smooth scroll to bring the focused tile into view
            self.scroll_to_focused_tile(ctx);
        }

        if consumed {
            ctx.request_repaint();
            log::debug!(
                "Viewport navigation: focus is now {:?}",
                self.behavior.focused_tile()
            );
        }

        None
    }

    /// Get the pane index for a given tile ID (0-indexed position in the pane list)
    fn get_pane_index(&self, tile_id: TileId) -> Option<usize> {
        self.get_pane_tile_ids()
            .iter()
            .position(|&id| id == tile_id)
    }

    /// Check if any buffer in the viewport is currently in insert mode
    fn is_any_buffer_in_insert_mode(&self) -> bool {
        for tile_id in self.get_pane_tile_ids() {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                // Check QueryPane
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    if query_pane.buffer_mode() == BufferMode::Insert {
                        return true;
                    }
                }
                // Check Buffer
                if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                    if buffer.mode() == BufferMode::Insert {
                        return true;
                    }
                }
            }
        }
        false
    }

    // =========================================================================
    // Visual Multi-Select Mode
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

    /// Enter visual-multi mode starting from the given pane
    fn enter_visual_multi_mode(&mut self, starting_tile_id: TileId) {
        let pane_ids = self.get_pane_tile_ids();

        // Validate that the starting tile exists in the current pane list
        // (it might be stale after a :split operation)
        let valid_starting_tile = if pane_ids.contains(&starting_tile_id) {
            starting_tile_id
        } else {
            // Fall back to first pane if the starting tile is invalid
            log::debug!(
                "Starting tile {starting_tile_id:?} not found in panes, falling back to first pane"
            );
            match pane_ids.first() {
                Some(&first) => first,
                None => {
                    log::debug!("No panes available for visual-multi mode");
                    return;
                }
            }
        };

        log::debug!("Entering visual-multi mode with tile {valid_starting_tile:?}");
        self.visual_multi_state = Some(VisualMultiState::new(valid_starting_tile));
        // Sync the cursor to the behavior so the focus border is drawn
        self.behavior.set_focused_tile(Some(valid_starting_tile));
    }

    /// Exit visual-multi mode
    fn exit_visual_multi_mode(&mut self) {
        log::debug!("Exiting visual-multi mode");
        self.visual_multi_state = None;
    }

    /// Close all selected panes in visual-multi mode
    fn close_selected_panes(&mut self) {
        let selected_ids: Vec<TileId> = self
            .visual_multi_state
            .as_ref()
            .map(|s| s.selected_tile_ids.iter().copied().collect())
            .unwrap_or_default();

        if selected_ids.is_empty() {
            log::debug!("No panes selected to close");
            return;
        }

        log::debug!(
            "Closing {} selected panes: {:?}",
            selected_ids.len(),
            selected_ids
        );

        // Close each selected tile
        for tile_id in selected_ids {
            self.close_tile(tile_id);
        }

        // Exit visual-multi mode after closing
        self.exit_visual_multi_mode();
        self.multi_buffer_state.reset();
    }

    /// Refresh all selected panes in visual-multi mode
    fn refresh_selected_panes(&mut self) {
        let selected_ids: Vec<TileId> = self
            .visual_multi_state
            .as_ref()
            .map(|s| s.selected_tile_ids.iter().copied().collect())
            .unwrap_or_default();

        if selected_ids.is_empty() {
            log::debug!("No panes selected to refresh");
            return;
        }

        log::debug!(
            "Refreshing {} selected panes: {:?}",
            selected_ids.len(),
            selected_ids
        );

        // Refresh each selected pane
        for tile_id in selected_ids {
            if let Some(egui_tiles::Tile::Pane(pane)) = self.viewport_tree.tiles.get_mut(tile_id) {
                if let Some(query_pane) = pane.as_any_mut().downcast_mut::<QueryPane>() {
                    query_pane.refresh();
                }
            }
        }
    }

    /// Handle keyboard input while in visual-multi mode
    fn handle_visual_multi_keyboard(&mut self, ctx: &egui::Context) -> Option<DashboardAction> {
        let pane_ids = self.get_pane_tile_ids();

        // Get current cursor position from visual-multi state
        let cursor_tile_id = self
            .visual_multi_state
            .as_ref()
            .and_then(|s| s.cursor_tile_id);

        let mut consumed = false;
        let mut should_exit = false;
        let mut should_toggle_selection = false;
        let mut should_select_all = false;
        let mut should_clear_selection = false;
        let mut should_open_multi_edit = false;
        let mut should_close_selected = false;
        let mut should_refresh_selected = false;
        let mut new_cursor_id: Option<TileId> = None;

        ctx.input_mut(|input| {
            // Escape - exit visual-multi mode
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                should_exit = true;
                consumed = true;
                return;
            }

            // e - open multi-edit overlay for selected panes
            if input.consume_key(egui::Modifiers::NONE, egui::Key::E) {
                should_open_multi_edit = true;
                consumed = true;
                return;
            }

            // x - close all selected panes
            if input.consume_key(egui::Modifiers::NONE, egui::Key::X) {
                should_close_selected = true;
                consumed = true;
                return;
            }

            // r - refresh all selected panes
            if input.consume_key(egui::Modifiers::NONE, egui::Key::R) {
                should_refresh_selected = true;
                consumed = true;
                return;
            }

            // Space - toggle selection on current pane
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Space) {
                should_toggle_selection = true;
                consumed = true;
                return;
            }

            // a - select all panes
            if input.consume_key(egui::Modifiers::NONE, egui::Key::A) {
                should_select_all = true;
                consumed = true;
                return;
            }

            // n - clear all selections (select none)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::N) {
                should_clear_selection = true;
                consumed = true;
                return;
            }

            // j or down arrow - move cursor down
            if input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                if let Some(current_id) = cursor_tile_id {
                    new_cursor_id = self.find_sibling_in_direction(current_id, NavDirection::Down);
                } else {
                    new_cursor_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // k or up arrow - move cursor up
            if input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                if let Some(current_id) = cursor_tile_id {
                    new_cursor_id = self.find_sibling_in_direction(current_id, NavDirection::Up);
                } else {
                    new_cursor_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // h or left arrow - move cursor left
            if input.consume_key(egui::Modifiers::NONE, egui::Key::H)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
            {
                if let Some(current_id) = cursor_tile_id {
                    new_cursor_id = self.find_sibling_in_direction(current_id, NavDirection::Left);
                } else {
                    new_cursor_id = pane_ids.first().copied();
                }
                consumed = true;
                return;
            }

            // l or right arrow - move cursor right
            if input.consume_key(egui::Modifiers::NONE, egui::Key::L)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
            {
                if let Some(current_id) = cursor_tile_id {
                    new_cursor_id = self.find_sibling_in_direction(current_id, NavDirection::Right);
                } else {
                    new_cursor_id = pane_ids.first().copied();
                }
                consumed = true;
            }
        });

        // Apply actions
        if should_exit {
            self.exit_visual_multi_mode();
            self.multi_buffer_state.reset();
        } else if should_close_selected {
            self.close_selected_panes();
        } else if should_refresh_selected {
            self.refresh_selected_panes();
        } else if should_open_multi_edit {
            self.open_multi_edit_for_selected();
        } else if should_toggle_selection {
            if let (Some(state), Some(tile_id)) = (self.visual_multi_state.as_mut(), cursor_tile_id)
            {
                state.toggle_selection(tile_id);
            }
        } else if should_select_all {
            if let Some(state) = self.visual_multi_state.as_mut() {
                state.select_all(&pane_ids);
            }
        } else if should_clear_selection {
            if let Some(state) = self.visual_multi_state.as_mut() {
                state.clear_selection();
            }
        } else if let Some(tile_id) = new_cursor_id {
            // Move cursor to the new pane and select it (visual-line style)
            if let Some(state) = self.visual_multi_state.as_mut() {
                state.set_cursor(tile_id);
                // Auto-select the pane when navigating to it
                state.selected_tile_ids.insert(tile_id);
            }
            // Also update the behavior's focused tile to show the focus border
            self.behavior.set_focused_tile(Some(tile_id));
            self.activate_tile(tile_id);
            self.scroll_to_focused_tile(ctx);
        }

        if consumed {
            ctx.request_repaint();
            log::debug!(
                "Visual-multi mode: cursor is now {:?}, {} selected, IDs: {:?}",
                self.visual_multi_state
                    .as_ref()
                    .and_then(|s| s.cursor_tile_id),
                self.visual_multi_selection_count(),
                self.visual_multi_state.as_ref().map(|s| s
                    .selected_tile_ids
                    .iter()
                    .copied()
                    .collect::<Vec<_>>())
            );
        }

        None
    }

    /// Open the multi-edit overlay for all selected panes in visual-multi mode
    fn open_multi_edit_for_selected(&mut self) {
        let selected_ids: Vec<TileId> = self
            .visual_multi_state
            .as_ref()
            .map(|s| s.selected_tile_ids.iter().copied().collect())
            .unwrap_or_default();

        log::debug!(
            "open_multi_edit_for_selected: {} tile IDs selected: {:?}",
            selected_ids.len(),
            selected_ids
        );

        if selected_ids.is_empty() {
            log::debug!("No panes selected for multi-edit");
            return;
        }

        // Collect excerpts from selected panes
        let mut excerpts = Vec::new();
        for tile_id in &selected_ids {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(*tile_id)
            {
                // Try to get query content from QueryPane
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    log::debug!(
                        "  tile {:?} -> QueryPane '{}' with query '{}'",
                        tile_id,
                        query_pane.name(),
                        query_pane.query()
                    );
                    excerpts.push(EditExcerpt::new(
                        query_pane.id(),
                        query_pane.name().to_string(),
                        query_pane.query().to_string(),
                    ));
                }
                // Try to get content from Buffer
                else if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                    log::debug!(
                        "  tile {:?} -> Buffer '{}' with content '{}'",
                        tile_id,
                        buffer.name(),
                        buffer.content()
                    );
                    excerpts.push(EditExcerpt::new(
                        buffer.id(),
                        buffer.name().to_string(),
                        buffer.content().to_string(),
                    ));
                } else {
                    log::debug!(
                        "  tile {tile_id:?} -> Unknown component type (not QueryPane or Buffer)"
                    );
                }
            } else {
                log::debug!("  tile {tile_id:?} -> Not found or not a Pane");
            }
        }

        if excerpts.is_empty() {
            log::debug!("No query panes found in selection");
            return;
        }

        log::debug!("Opening multi-edit with {} excerpts", excerpts.len());
        self.multi_edit_overlay.open(excerpts);

        // Exit visual-multi mode when opening the overlay
        self.exit_visual_multi_mode();
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

    // =========================================================================
    // Workspace serialization/deserialization
    // =========================================================================

    /// Serialize the current dashboard state to a Workspace
    pub fn to_workspace(&self, name: &str, theme: AppTheme, endpoint: Option<&str>) -> Workspace {
        let mut panes = Vec::new();

        // Collect all QueryPane data from the viewport tree
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    let state = query_pane.query_state();
                    panes.push(PaneConfig::from_query_state(
                        query_pane.saved_query(),
                        query_pane.name(),
                        query_pane.tag(),
                        state,
                    ));
                }
            }
        }

        Workspace {
            workspace: WorkspaceMeta {
                name: name.to_string(),
                description: String::new(),
                version: crate::workspace::WORKSPACE_VERSION,
            },
            connection: endpoint.map_or_else(ConnectionConfig::default, |e| {
                ConnectionConfig::with_endpoint(e)
            }),
            view: ViewConfig {
                theme: match theme {
                    AppTheme::Light => "light".to_string(),
                    AppTheme::Dark => "dark".to_string(),
                },
                metrics_panel: false, // Left panel removed
                inspector: false,     // Inspector panel removed
                zen_mode: self.zen_mode,
            },
            time: TimeConfig::from_preset(self.time_range_toolbar.time_range().preset),
            panes,
            layout: self.extract_layout_from_tree(),
        }
    }

    /// Load a workspace into the dashboard, replacing current state
    /// Returns the connection config if specified in the workspace
    pub fn load_workspace(
        &mut self,
        workspace: &Workspace,
        theme: &mut AppTheme,
    ) -> Option<ConnectionConfig> {
        // Apply view settings
        *theme = workspace.view.app_theme();
        // Note: metrics_panel setting ignored (left panel removed)
        self.zen_mode = workspace.view.zen_mode;

        // Apply time range
        self.time_range_toolbar
            .set_preset(workspace.time.to_preset());

        // Clear existing panes and reset the tree
        self.clear_all_panes();

        // Reset query counter for new workspace
        self.next_query_number = 1;

        // Phase 1: Insert all panes and collect their TileIds
        let mut pane_tile_ids: Vec<TileId> = Vec::with_capacity(workspace.panes.len());

        for pane_config in &workspace.panes {
            let query_number = self.next_query_number;
            self.next_query_number += 1;

            let mut query_pane = QueryPane::from_config_numbered(
                &pane_config.query,
                &pane_config.name,
                query_number,
            );
            if !pane_config.tag.is_empty() {
                query_pane.set_tag(&pane_config.tag);
            }

            // Apply query state
            let state = pane_config.to_query_state(&workspace.time.preset);
            query_pane.set_query_state(state);

            // Apply visualization type from config
            query_pane.set_visualization_type(pane_config.visualization_type());

            // Track the chart
            self.open_charts.insert(pane_config.query.clone());

            // Insert pane and record its TileId (don't add to viewport yet)
            let tile_id = self.viewport_tree.tiles.insert_pane(Box::new(query_pane));
            pane_tile_ids.push(tile_id);
        }

        // Phase 2: Build the layout tree
        let root_id = if let Some(layout) = &workspace.layout {
            // Validate layout references before building
            if let Err(e) = layout.validate(workspace.panes.len()) {
                log::warn!("Invalid layout config: {e}. Falling back to tabs.");
                self.viewport_tree
                    .tiles
                    .insert_tab_tile(pane_tile_ids.clone())
            } else {
                // Use explicit layout configuration
                self.build_layout_tree(layout, &pane_tile_ids)
            }
        } else {
            // Backward compatibility: no layout = tabs container
            self.viewport_tree
                .tiles
                .insert_tab_tile(pane_tile_ids.clone())
        };

        // Set the root
        self.viewport_tree.root = Some(root_id);

        // Hide landing page if we have panes
        if !workspace.panes.is_empty() {
            self.show_landing = false;
        }

        // Return connection config if present
        if workspace.connection.is_empty() {
            None
        } else {
            Some(workspace.connection.clone())
        }
    }

    /// Clear all panes from the viewport
    fn clear_all_panes(&mut self) {
        // Get all pane IDs
        let pane_ids = self.get_pane_tile_ids();

        // Remove each pane
        for tile_id in pane_ids {
            self.viewport_tree.tiles.remove(tile_id);
        }

        // Reset the tree with an empty tabs container
        let mut tiles: Tiles<Box<dyn Component>> = egui_tiles::Tiles::default();
        let tabs = Vec::new();
        let root = tiles.insert_tab_tile(tabs);
        self.viewport_tree = egui_tiles::Tree::new("viewport_tree", root, tiles);

        // Clear tracking
        self.open_charts.clear();
        self.behavior.set_focused_tile(None);
        self.show_landing = true;
    }

    // ==================== Layout Tree Building ====================

    /// Build the tile tree from a layout configuration
    fn build_layout_tree(&mut self, layout: &LayoutConfig, pane_tile_ids: &[TileId]) -> TileId {
        let container = LayoutContainer {
            layout_type: layout.layout_type,
            children: layout.children.clone(),
            shares: layout.shares.clone(),
        };
        self.build_container(&container, pane_tile_ids)
    }

    /// Recursively build a container and its children
    fn build_container(&mut self, container: &LayoutContainer, pane_tile_ids: &[TileId]) -> TileId {
        // First, resolve all children to TileIds
        let child_ids: Vec<TileId> = container
            .children
            .iter()
            .filter_map(|node| self.resolve_layout_node(node, pane_tile_ids))
            .collect();

        if child_ids.is_empty() {
            // Fallback: create empty tabs container
            return self.viewport_tree.tiles.insert_tab_tile(vec![]);
        }

        match container.layout_type {
            LayoutType::Tabs => self.viewport_tree.tiles.insert_tab_tile(child_ids),
            LayoutType::Horizontal => {
                let container_id = self
                    .viewport_tree
                    .tiles
                    .insert_horizontal_tile(child_ids.clone());

                // Apply shares - use specified shares or default to equal (1.0) for all
                let shares = if container.shares.is_empty() {
                    vec![1.0; child_ids.len()]
                } else {
                    container.shares.clone()
                };
                self.apply_shares(container_id, &child_ids, &shares);

                container_id
            }
            LayoutType::Vertical => {
                let container_id = self
                    .viewport_tree
                    .tiles
                    .insert_vertical_tile(child_ids.clone());

                // Apply shares - use specified shares or default to equal (1.0) for all
                let shares = if container.shares.is_empty() {
                    vec![1.0; child_ids.len()]
                } else {
                    container.shares.clone()
                };
                self.apply_shares(container_id, &child_ids, &shares);

                container_id
            }
        }
    }

    /// Resolve a layout node to a TileId
    fn resolve_layout_node(
        &mut self,
        node: &LayoutNode,
        pane_tile_ids: &[TileId],
    ) -> Option<TileId> {
        match node {
            LayoutNode::Pane(index) => {
                // Get the pre-inserted pane's TileId
                pane_tile_ids.get(*index).copied()
            }
            LayoutNode::Container(container) => {
                // Recursively build nested container
                Some(self.build_container(container, pane_tile_ids))
            }
        }
    }

    /// Apply shares to a linear container
    fn apply_shares(&mut self, container_id: TileId, child_ids: &[TileId], shares: &[f32]) {
        if let Some(Tile::Container(egui_tiles::Container::Linear(linear))) =
            self.viewport_tree.tiles.get_mut(container_id)
        {
            for (i, &child_id) in child_ids.iter().enumerate() {
                let share = shares.get(i).copied().unwrap_or(1.0);
                linear.shares.set_share(child_id, share);
            }
        }
    }

    // ==================== Layout Tree Extraction ====================

    /// Extract layout configuration from the current tile tree
    fn extract_layout_from_tree(&self) -> Option<LayoutConfig> {
        let root_id = self.viewport_tree.root()?;

        // Build a mapping from TileId to pane index
        let pane_ids = self.get_pane_tile_ids();
        let pane_index_map: HashMap<TileId, usize> = pane_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        // Extract the root container
        match self.viewport_tree.tiles.get(root_id)? {
            Tile::Container(container) => {
                let (layout_type, children, shares) =
                    self.extract_container(container, &pane_index_map);
                Some(LayoutConfig {
                    layout_type,
                    children,
                    shares,
                })
            }
            Tile::Pane(_) => {
                // Single pane - wrap in tabs
                let index = pane_index_map.get(&root_id)?;
                Some(LayoutConfig {
                    layout_type: LayoutType::Tabs,
                    children: vec![LayoutNode::Pane(*index)],
                    shares: Vec::new(),
                })
            }
        }
    }

    /// Extract a container's layout configuration
    fn extract_container(
        &self,
        container: &egui_tiles::Container,
        pane_index_map: &HashMap<TileId, usize>,
    ) -> (LayoutType, Vec<LayoutNode>, Vec<f32>) {
        match container {
            egui_tiles::Container::Tabs(tabs) => {
                let children: Vec<LayoutNode> = tabs
                    .children
                    .iter()
                    .filter_map(|&id| self.tile_to_layout_node(id, pane_index_map))
                    .collect();
                (LayoutType::Tabs, children, Vec::new())
            }
            egui_tiles::Container::Linear(linear) => {
                let layout_type = match linear.dir {
                    egui_tiles::LinearDir::Horizontal => LayoutType::Horizontal,
                    egui_tiles::LinearDir::Vertical => LayoutType::Vertical,
                };

                let children: Vec<LayoutNode> = linear
                    .children
                    .iter()
                    .filter_map(|&id| self.tile_to_layout_node(id, pane_index_map))
                    .collect();

                // Extract shares
                let shares: Vec<f32> = linear
                    .children
                    .iter()
                    .map(|&id| linear.shares[id])
                    .collect();

                // Only include shares if they differ from default (all 1.0)
                let all_default = shares.iter().all(|&s| (s - 1.0).abs() < 0.01);
                let shares = if all_default { Vec::new() } else { shares };

                (layout_type, children, shares)
            }
            egui_tiles::Container::Grid(_) => {
                // Grid not supported in this schema - convert to tabs
                let children: Vec<LayoutNode> = container
                    .children()
                    .filter_map(|&id| self.tile_to_layout_node(id, pane_index_map))
                    .collect();
                (LayoutType::Tabs, children, Vec::new())
            }
        }
    }

    /// Convert a tile to a layout node
    fn tile_to_layout_node(
        &self,
        tile_id: TileId,
        pane_index_map: &HashMap<TileId, usize>,
    ) -> Option<LayoutNode> {
        match self.viewport_tree.tiles.get(tile_id)? {
            Tile::Pane(_) => {
                let index = pane_index_map.get(&tile_id)?;
                Some(LayoutNode::Pane(*index))
            }
            Tile::Container(container) => {
                let (layout_type, children, shares) =
                    self.extract_container(container, pane_index_map);
                Some(LayoutNode::Container(LayoutContainer {
                    layout_type,
                    children,
                    shares,
                }))
            }
        }
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
                        .color(text_color(self.behavior.theme).gamma_multiply(0.5))
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
                                component.set_theme(self.behavior.theme);
                                component.set_api_key(&self.behavior.api_key);

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

#[derive(Default, Clone)]
struct TreeBehavior {
    add_child_to: Option<egui_tiles::TileId>,
    /// Currently focused tile for vim-style navigation
    focused_tile_id: Option<egui_tiles::TileId>,
    /// Selected tiles in visual-multi mode (empty when not in visual-multi mode)
    selected_tile_ids: HashSet<egui_tiles::TileId>,
    /// Whether we're currently in visual-multi mode
    is_visual_multi_mode: bool,
    /// Query content per tile (for display in visual-multi mode)
    tile_queries: HashMap<egui_tiles::TileId, String>,
    theme: AppTheme,
    api_key: String,
    /// Tile IDs that are filtered out (should be dimmed)
    filtered_out_tiles: HashSet<egui_tiles::TileId>,
    /// Whether viewport filter is active
    is_filter_active: bool,
}

impl TreeBehavior {
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }
    pub fn set_keys(&mut self, api_key: String) {
        self.api_key = api_key;
    }
    pub fn set_focused_tile(&mut self, tile_id: Option<egui_tiles::TileId>) {
        self.focused_tile_id = tile_id;
    }
    pub fn focused_tile(&self) -> Option<egui_tiles::TileId> {
        self.focused_tile_id
    }
    pub fn set_visual_multi_state(
        &mut self,
        is_active: bool,
        selected_ids: HashSet<egui_tiles::TileId>,
        tile_queries: HashMap<egui_tiles::TileId, String>,
    ) {
        self.is_visual_multi_mode = is_active;
        self.selected_tile_ids = selected_ids;
        self.tile_queries = tile_queries;
    }

    pub fn set_filter_state(
        &mut self,
        is_active: bool,
        filtered_out_tiles: HashSet<egui_tiles::TileId>,
    ) {
        self.is_filter_active = is_active;
        self.filtered_out_tiles = filtered_out_tiles;
    }
}

impl egui_tiles::Behavior<Box<dyn Component>> for TreeBehavior {
    /// Gap between panes in horizontal/vertical layouts
    fn gap_width(&self, _style: &egui::Style) -> f32 {
        4.0 // Subtle gap for visual separation
    }

    /// Stroke for the resize handle between panes
    fn resize_stroke(
        &self,
        _style: &egui::Style,
        resize_state: egui_tiles::ResizeState,
    ) -> egui::Stroke {
        let color = match resize_state {
            egui_tiles::ResizeState::Idle => palette::border_subtle(self.theme),
            egui_tiles::ResizeState::Hovering => palette::border_default(self.theme),
            egui_tiles::ResizeState::Dragging => palette::border::FOCUS,
        };
        egui::Stroke::new(1.0, color)
    }

    /// Height of the tab bar
    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        28.0 // Slightly taller for better visual presence
    }

    /// Background color of the tab bar
    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        palette::bg_surface(self.theme)
    }

    /// Background color of individual tabs
    fn tab_bg_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<Box<dyn Component>>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            palette::bg_elevated(self.theme)
        } else if state.is_being_dragged {
            palette::bg_hover(self.theme)
        } else {
            palette::bg_surface(self.theme)
        }
    }

    /// Stroke for the line separating tab bar from content
    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
        egui::Stroke::new(1.0, palette::border_subtle(self.theme))
    }

    /// Outline stroke around tabs (emerald for active, subtle for inactive)
    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<Box<dyn Component>>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Stroke {
        if state.active {
            egui::Stroke::new(1.0, palette::accent::PRIMARY)
        } else {
            egui::Stroke::new(1.0, palette::border_subtle(self.theme))
        }
    }

    fn tab_title_for_pane(&mut self, component: &Box<dyn Component>) -> egui::WidgetText {
        component
            .label()
            .color(text_color(self.theme))
            .strong()
            .into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        component: &mut Box<dyn Component>,
    ) -> egui_tiles::UiResponse {
        // Make sure theme + keys are updated for the component
        component.set_theme(self.theme);
        component.set_api_key(&self.api_key);

        component.show(ui);

        egui_tiles::UiResponse::None
    }

    fn paint_on_top_of_tile(
        &self,
        painter: &egui::Painter,
        _style: &egui::Style,
        tile_id: egui_tiles::TileId,
        rect: egui::Rect,
    ) {
        let is_focused = self.focused_tile_id == Some(tile_id);
        let is_selected = self.is_visual_multi_mode && self.selected_tile_ids.contains(&tile_id);
        let is_filtered_out = self.is_filter_active && self.filtered_out_tiles.contains(&tile_id);

        // When viewport filter is active, dim non-matching panes
        if is_filtered_out {
            let dim_color = match self.theme {
                AppTheme::Light => egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200),
                AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200),
            };
            painter.rect_filled(rect, 4.0, dim_color);

            // Draw "filtered" indicator text
            let text_color = match self.theme {
                AppTheme::Light => egui::Color32::from_rgba_unmultiplied(100, 100, 100, 150),
                AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(150, 150, 150, 150),
            };
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "filtered",
                egui::FontId::proportional(12.0),
                text_color,
            );
            return; // Don't draw other overlays on filtered panes
        }

        // In visual-multi mode, draw selection indicator for selected panes
        if is_selected {
            // Emerald selection color to match brand
            let selection_color = match self.theme {
                AppTheme::Light => egui::Color32::from_rgba_unmultiplied(5, 150, 105, 50),
                AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(16, 185, 129, 40),
            };

            // Fill the entire tile with a subtle selection tint
            painter.rect_filled(rect, 4.0, selection_color);

            // Draw selection border
            let border_color = match self.theme {
                AppTheme::Light => palette::accent::LIGHT,
                AppTheme::Dark => palette::accent::PRIMARY,
            };
            let border_width = 2.0;
            let inset_rect = rect.shrink(border_width / 2.0);
            painter.rect_stroke(
                inset_rect,
                4.0,
                egui::Stroke::new(border_width, border_color),
                egui::StrokeKind::Outside,
            );
        }

        // Draw focus border on top of the entire tile (including tab bar)
        // This shows which pane has the cursor in visual-multi mode
        if is_focused {
            // White/gray focus color to match Enya's color scheme
            // Use brighter color in visual-multi mode to distinguish cursor from selection
            let focus_color = if self.is_visual_multi_mode {
                match self.theme {
                    AppTheme::Light => egui::Color32::from_rgb(100, 100, 110),
                    AppTheme::Dark => egui::Color32::from_rgb(255, 255, 255),
                }
            } else {
                match self.theme {
                    AppTheme::Light => egui::Color32::from_rgb(120, 120, 130),
                    AppTheme::Dark => egui::Color32::from_rgb(200, 200, 210),
                }
            };

            // Shrink the rect inward so the border stroke is fully visible
            let border_width = 3.0;
            let inset_rect = rect.shrink(border_width / 2.0);

            painter.rect_stroke(
                inset_rect,
                4.0,
                egui::Stroke::new(border_width, focus_color),
                egui::StrokeKind::Outside,
            );
        }

        // In visual-multi mode, show query content at the bottom of each selected pane
        if is_selected {
            if let Some(query) = self.tile_queries.get(&tile_id) {
                // Style for query overlay
                let bg_color = match self.theme {
                    AppTheme::Light => egui::Color32::from_rgba_unmultiplied(255, 255, 255, 230),
                    AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(30, 30, 35, 230),
                };
                let text_color = match self.theme {
                    AppTheme::Light => egui::Color32::from_rgb(50, 50, 60),
                    AppTheme::Dark => egui::Color32::from_rgb(220, 220, 230),
                };

                // Truncate query if too long
                let display_query = if query.len() > 60 {
                    format!("{}...", &query[..57])
                } else {
                    query.clone()
                };

                // Calculate text layout
                let font_id = egui::FontId::monospace(11.0);
                let galley = painter.layout_no_wrap(display_query, font_id, text_color);

                // Position at bottom of tile with padding
                let padding = 6.0;
                let overlay_height = galley.rect.height() + padding * 2.0;
                let overlay_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x, rect.max.y - overlay_height),
                    egui::vec2(rect.width(), overlay_height),
                );

                // Draw background
                painter.rect_filled(overlay_rect, 0.0, bg_color);

                // Draw text centered vertically in the overlay
                let text_pos = egui::pos2(
                    overlay_rect.min.x + padding,
                    overlay_rect.center().y - galley.rect.height() / 2.0,
                );
                painter.galley(text_pos, galley, text_color);
            }
        }
    }
    fn top_bar_right_ui(
        &mut self,
        _tiles: &egui_tiles::Tiles<Box<dyn Component>>,
        ui: &mut egui::Ui,
        tile_id: egui_tiles::TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        if ui.button("➕").clicked() {
            self.add_child_to = Some(tile_id);
        }
    }
    fn is_tab_closable(&self, _tiles: &Tiles<Box<dyn Component>>, _tile_id: TileId) -> bool {
        true
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        SimplificationOptions {
            all_panes_must_have_tabs: true,
            prune_empty_tabs: true,
            prune_empty_containers: true,
            ..SimplificationOptions::OFF
        }
    }

    fn on_tab_close(&mut self, tiles: &mut Tiles<Box<dyn Component>>, tile_id: TileId) -> bool {
        if let Some(tile) = tiles.get(tile_id) {
            match tile {
                Tile::Pane(pane) => {
                    // Single pane removal
                    let tab_title = self.tab_title_for_pane(pane);
                    log::debug!("Closing tab: {}, tile ID: {tile_id:?}", tab_title.text());
                }
                Tile::Container(container) => {
                    // Container removal
                    log::debug!("Closing container: {:?}", container.kind());
                    let children_ids = container.children();
                    for child_id in children_ids {
                        if let Some(Tile::Pane(pane)) = tiles.get(*child_id) {
                            let tab_title = self.tab_title_for_pane(pane);
                            log::debug!("Closing tab: {}, tile ID: {tile_id:?}", tab_title.text());
                        }
                    }
                }
            }
        }

        // Proceed to removing the tab
        true
    }
}
