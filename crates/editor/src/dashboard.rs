use std::collections::HashSet;

use egui_tiles::{SimplificationOptions, Tile, TileId, Tiles};

use crate::app::AppState;
use egui::RichText;

use crate::components::{
    Buffer, BufferEditor, BufferEditorResult, BufferMode, CommandPalette, CommandResult, Component,
    CustomQueriesPanel, DiffOffset, DiffView, DiffViewAction, EditExcerpt, InfoOverlay,
    LandingPage, LandingPageAction, MetricItem, MetricsFinder, MetricsTree, MultiEditOverlay,
    MultiEditResult, QueryFinder, QueryItem, QueryPane, QueryState, TagFilter, TagPath,
    TimeRangeToolbar, WhichKey,
};
use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;

/// Toggle button for the metrics panel visibility
fn metrics_panel_toggle_button(
    ui: &mut egui::Ui,
    is_visible: bool,
    theme: AppTheme,
) -> egui::Response {
    let text_col = text_color(theme);
    let icon = semantic_icons::nav::SIDEBAR;

    let button = if is_visible {
        egui::Button::new(RichText::new(icon).strong()).fill(ui.visuals().selection.bg_fill)
    } else {
        egui::Button::new(RichText::new(icon).color(text_col.gamma_multiply(0.7)))
    };

    ui.add(button).on_hover_text(if is_visible {
        "Hide metrics panel"
    } else {
        "Show metrics panel"
    })
}
use crate::workspace::{
    ConnectionConfig, PaneConfig, TimeConfig, ViewConfig, Workspace, WorkspaceMeta,
};

/// A floating window containing a chart/component
pub struct FloatingWindow {
    /// Unique identifier for the floating window
    pub id: u64,
    /// The component being displayed
    pub component: Box<dyn Component>,
    /// Whether the window is open
    pub open: bool,
}

impl FloatingWindow {
    /// Create a new floating window from a component
    pub fn new(id: u64, component: Box<dyn Component>) -> Self {
        Self {
            id,
            component,
            open: true,
        }
    }
}

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
}

/// The main dashboard layout with a fixed left panel for the MetricsTree
/// and a flexible right area for tabbed views/charts.
pub struct Dashboard {
    /// The metrics tree browser (always visible in left panel)
    metrics_tree: MetricsTree,
    /// Custom queries panel (below metrics tree in left panel)
    custom_queries: CustomQueriesPanel,
    /// Whether the "Provided" section is expanded
    provided_expanded: bool,
    /// Whether the "Custom" section is expanded
    custom_expanded: bool,
    /// The tile tree for the viewport area (right side)
    viewport_tree: egui_tiles::Tree<Box<dyn Component>>,
    behavior: TreeBehavior,
    /// Width of the left panel in pixels
    left_panel_width: f32,
    /// Whether the left panel (metrics tree) is visible
    left_panel_visible: bool,
    /// Track which metrics already have charts open (by metric name)
    open_charts: HashSet<String>,
    /// Pending chart to add (metric name)
    pending_chart: Option<String>,
    /// Time range toolbar
    time_range_toolbar: TimeRangeToolbar,
    /// Fuzzy finder modal for metrics (telescope-style search)
    metrics_finder: MetricsFinder,
    /// Query finder modal for saved queries (with side-by-side preview)
    query_finder: QueryFinder,
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
    /// Floating windows (popped out charts)
    floating_windows: Vec<FloatingWindow>,
    /// Next floating window ID
    next_floating_id: u64,
    /// Landing page component (shown when no charts are open)
    landing_page: LandingPage,
    /// Whether to show the landing page
    show_landing: bool,
    /// Last time 'y' was pressed (for yy detection)
    last_y_press: Option<crate::util::Instant>,
    /// Tag filter for filtering queries/buffers
    tag_filter: TagFilter,
    /// Info overlay (shows build/version info)
    info_overlay: InfoOverlay,
    /// Which-key overlay (shows available keybindings)
    which_key: WhichKey,
    /// Diff view for comparing time periods (shown when in diff mode)
    diff_view: Option<DiffView>,
    /// Whether diff mode is active
    diff_mode: bool,
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
    /// Multi-edit overlay for editing multiple panes simultaneously
    multi_edit_overlay: MultiEditOverlay,
}

impl Default for Dashboard {
    fn default() -> Self {
        let mut tiles: Tiles<Box<dyn Component>> = egui_tiles::Tiles::default();
        let tabs = Vec::new();
        let root = tiles.insert_tab_tile(tabs);

        let viewport_tree = egui_tiles::Tree::new("viewport_tree", root, tiles);
        Self {
            metrics_tree: MetricsTree::default(),
            custom_queries: CustomQueriesPanel::new(),
            provided_expanded: true,
            custom_expanded: false,
            viewport_tree,
            behavior: TreeBehavior::default(),
            left_panel_width: 280.0,
            left_panel_visible: true,
            open_charts: HashSet::new(),
            pending_chart: None,
            time_range_toolbar: TimeRangeToolbar::new(),
            metrics_finder: MetricsFinder::new(),
            query_finder: QueryFinder::new(),
            command_palette: CommandPalette::new(),
            buffer_editor: BufferEditor::new(),
            editing_tile_id: None,
            zen_mode: false,
            fullscreen_tile: None,
            floating_windows: Vec::new(),
            next_floating_id: 1,
            landing_page: LandingPage::new(),
            show_landing: true,
            last_y_press: None,
            tag_filter: TagFilter::new(),
            info_overlay: InfoOverlay::new(enya_build_info::build_info!()),
            which_key: WhichKey::new(),
            diff_view: None,
            diff_mode: false,
            viewport_scroll_offset: 0.0,
            viewport_scroll_target: 0.0,
            viewport_content_height: 0.0,
            viewport_visible_height: 0.0,
            visual_multi_state: None,
            multi_edit_overlay: MultiEditOverlay::new(),
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
    /// Default left panel width
    const DEFAULT_PANEL_WIDTH: f32 = 280.0;
    /// Minimum left panel width
    const MIN_PANEL_WIDTH: f32 = 200.0;
    /// Maximum left panel width
    const MAX_PANEL_WIDTH: f32 = 500.0;

    pub fn example(_api_key: String) -> Self {
        let mut tiles: Tiles<Box<dyn Component>> = egui_tiles::Tiles::default();

        // Start with empty tabs - show landing page first
        let root = tiles.insert_tab_tile(vec![]);

        let viewport_tree = egui_tiles::Tree::new("viewport_tree", root, tiles);

        Self {
            metrics_tree: MetricsTree::with_demo_metrics(),
            custom_queries: CustomQueriesPanel::with_demo_queries(),
            provided_expanded: true,
            custom_expanded: false,
            viewport_tree,
            behavior: TreeBehavior::default(),
            left_panel_width: Self::DEFAULT_PANEL_WIDTH,
            left_panel_visible: true,
            open_charts: HashSet::new(),
            pending_chart: None,
            time_range_toolbar: TimeRangeToolbar::new(),
            metrics_finder: MetricsFinder::new(),
            query_finder: QueryFinder::new(),
            command_palette: CommandPalette::new(),
            buffer_editor: BufferEditor::new(),
            editing_tile_id: None,
            zen_mode: false,
            fullscreen_tile: None,
            floating_windows: Vec::new(),
            next_floating_id: 1,
            landing_page: LandingPage::new(),
            show_landing: true, // Start with landing page
            last_y_press: None,
            tag_filter: TagFilter::new(),
            info_overlay: InfoOverlay::new(enya_build_info::build_info!()),
            which_key: WhichKey::new(),
            diff_view: None,
            diff_mode: false,
            viewport_scroll_offset: 0.0,
            viewport_scroll_target: 0.0,
            viewport_content_height: 0.0,
            viewport_visible_height: 0.0,
            visual_multi_state: None,
            multi_edit_overlay: MultiEditOverlay::new(),
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

        // Sync visual-multi state to behavior for rendering
        let (is_visual_multi, selected_ids) = match &self.visual_multi_state {
            Some(state) => (true, state.selected_tile_ids.clone()),
            None => (false, HashSet::new()),
        };
        self.behavior
            .set_visual_multi_state(is_visual_multi, selected_ids);

        // Update component themes
        self.metrics_tree.set_theme(app_state.theme);
        self.custom_queries.set_theme(app_state.theme);
        self.time_range_toolbar.set_theme(app_state.theme);
        self.landing_page.set_theme(app_state.theme);

        // Handle adding a pending chart to the viewport
        if let Some(metric_name) = self.pending_chart.take() {
            let action = self.add_chart_for_metric_with_tracking(&metric_name);
            if action != DashboardAction::None {
                return action;
            }
        }

        // Check if we should show landing page (no open charts)
        let has_charts = !self.open_charts.is_empty() || !self.floating_windows.is_empty();
        if !has_charts && !self.show_landing {
            self.show_landing = true;
        }

        // Show landing page if enabled and no charts open
        if self.show_landing && !has_charts {
            return self.show_landing_page(ui, ctx, app_state);
        }

        // Track if we need to open fuzzy finder (set inside closure, used after)
        let mut open_fuzzy = false;

        // Left panel with Provided (metrics) and Custom (queries) sections
        let text_color = text_color(app_state.theme);

        // In zen mode, hide the left panel
        if self.left_panel_visible && !self.zen_mode {
            egui::SidePanel::left("metrics_panel")
                .resizable(true)
                .default_width(self.left_panel_width)
                .width_range(Self::MIN_PANEL_WIDTH..=Self::MAX_PANEL_WIDTH)
                .show_inside(ui, |ui| {
                    // Search button at the top (opens fuzzy finder)
                    let search_btn = egui::Button::new(
                        egui::RichText::new(format!(
                            "{}  Search...",
                            semantic_icons::action::SEARCH
                        ))
                        .color(text_color.gamma_multiply(0.6)),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(1.0, text_color.gamma_multiply(0.2)))
                    .corner_radius(4.0);

                    if ui
                        .add_sized([ui.available_width(), 28.0], search_btn)
                        .on_hover_text("Search metrics and queries (Cmd+K)")
                        .clicked()
                    {
                        open_fuzzy = true;
                    }

                    ui.add_space(8.0);

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // "Provided" section - contains the metrics tree
                        let provided_header = format!("{} Provided", semantic_icons::file::FOLDER);
                        let provided_header_builder = egui::CollapsingHeader::new(
                            egui::RichText::new(provided_header)
                                .color(text_color)
                                .strong(),
                        )
                        .id_salt("provided_section")
                        .default_open(self.provided_expanded);

                        let provided_response = provided_header_builder.show(ui, |ui| {
                            self.metrics_tree.show(ui);

                            // Check if a metric was double-clicked (add chart action)
                            if let Some(metric_name) = self.metrics_tree.take_pending_chart() {
                                self.pending_chart = Some(metric_name);
                            }
                        });

                        // Update provided expanded state
                        if provided_response.fully_open() {
                            self.provided_expanded = true;
                        } else if provided_response.openness < 0.5 {
                            self.provided_expanded = false;
                        }

                        ui.add_space(4.0);

                        // "Custom" section - contains the custom queries
                        let custom_header = format!(
                            "{} Custom ({})",
                            semantic_icons::file::CODE,
                            self.custom_queries.queries().len()
                        );
                        let custom_header_builder = egui::CollapsingHeader::new(
                            egui::RichText::new(custom_header)
                                .color(text_color)
                                .strong(),
                        )
                        .id_salt("custom_section")
                        .default_open(self.custom_expanded);

                        let custom_response = custom_header_builder.show(ui, |ui| {
                            self.custom_queries.show(ui);

                            // Check if a custom query was double-clicked (add chart action)
                            if let Some(query_id) = self.custom_queries.take_pending_chart() {
                                if let Some(query) = self.custom_queries.get_query(query_id) {
                                    // Clone the values to avoid borrow issues
                                    let name = query.name.clone();
                                    let query_str = query.query.clone();
                                    self.add_chart_for_query(&name, &query_str);
                                }
                            }
                        });

                        // Update custom expanded state
                        if custom_response.fully_open() {
                            self.custom_expanded = true;
                        } else if custom_response.openness < 0.5 {
                            self.custom_expanded = false;
                        }
                    });
                });
        }

        // Open fuzzy finder if search button was clicked
        if open_fuzzy {
            self.open_metrics_finder();
        }

        // Right area with toolbar and viewport
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Top toolbar with time range controls (hidden in zen mode)
            if !self.zen_mode {
                egui::TopBottomPanel::top("time_range_toolbar")
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            // Metrics panel toggle on the left
                            if metrics_panel_toggle_button(
                                ui,
                                self.left_panel_visible,
                                app_state.theme,
                            )
                            .clicked()
                            {
                                self.left_panel_visible = !self.left_panel_visible;
                            }

                            ui.add_space(8.0);

                            // Time range controls
                            self.time_range_toolbar.show(ui);
                        });
                        ui.add_space(4.0);
                    });
            }

            // Main viewport area (tabbed charts/views)
            egui::CentralPanel::default().show_inside(ui, |ui| {
                // Check if we're in diff mode
                if self.diff_mode {
                    if let Some(ref mut diff_view) = self.diff_view {
                        diff_view.set_theme(app_state.theme);
                        let action = diff_view.show(ui);
                        match action {
                            DiffViewAction::Close => {
                                self.diff_mode = false;
                                self.diff_view = None;
                            }
                            DiffViewAction::OffsetChanged(offset) => {
                                // Update the diff view with new offset
                                diff_view.set_offset(offset);
                            }
                            _ => {}
                        }
                    }
                } else if let Some(fullscreen_id) = self.fullscreen_tile {
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

        // Show query finder modal (rendered on top of everything)
        self.query_finder.set_theme(app_state.theme);
        if let Some(selected_query) = self.query_finder.show(ctx) {
            return self.handle_query_selection_with_tracking(selected_query);
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

        // Show floating windows
        self.show_floating_windows(ctx, app_state.theme);

        // Show info overlay modal
        self.info_overlay.set_theme(app_state.theme);
        self.info_overlay.show(ctx);

        // Show which-key overlay modal
        self.which_key.set_theme(app_state.theme);
        self.which_key.show(ctx);

        // Handle ? key for which-key overlay (bypasses focus check so it works even with chart focus)
        if !self.which_key.is_open()
            && !self.metrics_finder.is_open()
            && !self.command_palette.is_open()
            && !self.buffer_editor.is_open()
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

        self.handle_command_result(cmd_result)
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
                is_query,
            } => {
                self.show_landing = false;
                if is_query {
                    // For queries, we need to find the query by name
                    if let Some(query) = self
                        .custom_queries
                        .queries()
                        .iter()
                        .find(|q| q.name == metric_name)
                    {
                        let name = query.name.clone();
                        let query_str = query.query.clone();
                        self.add_chart_for_query(&name, &query_str);
                    }
                } else {
                    self.pending_chart = Some(metric_name);
                }
            }
            LandingPageAction::OpenWorkspace { name } => {
                return DashboardAction::LoadWorkspace(name);
            }
            LandingPageAction::OpenFuzzyFinder => {
                self.open_metrics_finder();
            }
            LandingPageAction::OpenQueryFinder => {
                self.open_query_finder();
            }
            LandingPageAction::ShowInfo => {
                self.info_overlay.open();
            }
            LandingPageAction::ShowHelp => {
                self.which_key.open();
            }
            LandingPageAction::None => {}
        }

        // Show fuzzy finder modal (rendered on top of everything)
        self.metrics_finder.set_theme(app_state.theme);
        if let Some(selected_item) = self.metrics_finder.show(ctx) {
            return self.handle_metric_selection_with_tracking(selected_item);
        }

        // Show query finder modal (rendered on top of everything)
        self.query_finder.set_theme(app_state.theme);
        if let Some(selected_query) = self.query_finder.show(ctx) {
            return self.handle_query_selection_with_tracking(selected_query);
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

        self.handle_command_result(cmd_result)
    }

    /// Add a chart for a metric and return a tracking action
    fn add_chart_for_metric_with_tracking(&mut self, metric_name: &str) -> DashboardAction {
        // Don't add duplicate charts
        if self.open_charts.contains(metric_name) {
            log::debug!("Chart for {metric_name} already open");
            return DashboardAction::None;
        }

        // Create a QueryPane (buffer + chart) for the metric
        let pane: Box<dyn Component> = Box::new(QueryPane::with_demo_metric(metric_name));
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

    /// Handle query selection and return tracking action
    fn handle_query_selection_with_tracking(&mut self, item: QueryItem) -> DashboardAction {
        self.show_landing = false;
        self.add_chart_for_query(&item.name, &item.query);
        DashboardAction::TrackRecentPlot {
            name: item.name.clone(),
            metric_name: item.name,
            is_query: true,
        }
    }

    /// Show all floating windows
    fn show_floating_windows(&mut self, ctx: &egui::Context, theme: AppTheme) {
        let text_col = text_color(theme);
        let mut windows_to_dock: Vec<u64> = Vec::new();
        let mut windows_to_close: Vec<u64> = Vec::new();

        for floating in &mut self.floating_windows {
            if !floating.open {
                windows_to_close.push(floating.id);
                continue;
            }

            floating.component.set_theme(theme);

            let title = floating.component.name();
            let mut open = floating.open;

            egui::Window::new(
                egui::RichText::new(format!("{} {}", semantic_icons::action::CHART, title))
                    .color(text_col),
            )
            .id(egui::Id::new(format!("floating_window_{}", floating.id)))
            .open(&mut open)
            .resizable(true)
            .collapsible(true)
            .default_size([600.0, 350.0])
            .min_width(400.0)
            .min_height(250.0)
            .show(ctx, |ui| {
                // Dock button at top right
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(egui::RichText::new(semantic_icons::nav::GRID))
                            .on_hover_text("Dock back to tiled layout (D)")
                            .clicked()
                        {
                            windows_to_dock.push(floating.id);
                        }
                    });
                });

                // Give the chart most of the remaining space
                let available = ui.available_size();
                ui.allocate_ui(available, |ui| {
                    floating.component.show(ui);
                });
            });

            floating.open = open;
        }

        // Remove closed windows
        for id in windows_to_close {
            self.floating_windows.retain(|w| w.id != id);
        }

        // Dock windows back (handled after the loop to avoid borrow issues)
        for id in windows_to_dock {
            self.dock_floating_window(id);
        }
    }

    /// Handle a command result from the command palette
    fn handle_command_result(&mut self, result: CommandResult) -> DashboardAction {
        match result {
            CommandResult::ToggleTheme => DashboardAction::ToggleTheme,
            CommandResult::SetTheme(theme) => DashboardAction::SetTheme(theme),
            CommandResult::ToggleMetricsPanel => {
                self.left_panel_visible = !self.left_panel_visible;
                DashboardAction::None
            }
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
            CommandResult::SaveBuffer(name) => {
                self.save_focused_buffer(name);
                DashboardAction::None
            }
            CommandResult::EditBuffer => {
                self.edit_focused_buffer();
                DashboardAction::None
            }
            CommandResult::NewBuffer => {
                self.create_new_buffer();
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
            CommandResult::FloatPane => {
                self.float_focused_pane();
                DashboardAction::None
            }
            CommandResult::DockAll => {
                self.dock_all_floating();
                DashboardAction::None
            }
            CommandResult::TestNotify(level) => DashboardAction::Notify {
                level: level.clone(),
                message: format!("Test notification ({level})"),
            },
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
            CommandResult::SetTagFilter(path) => {
                self.set_tag_filter(path);
                DashboardAction::None
            }
            CommandResult::AddTag(path) => {
                if let Some(query_name) = self.add_tag_to_focused(&path) {
                    DashboardAction::Notify {
                        level: "info".to_string(),
                        message: format!("Added #{path} to \"{query_name}\""),
                    }
                } else {
                    DashboardAction::Notify {
                        level: "warn".to_string(),
                        message:
                            "No query focused. Focus a query chart or select one in the panel."
                                .to_string(),
                    }
                }
            }
            CommandResult::RemoveTag(path) => {
                if let Some(query_name) = self.remove_tag_from_focused(&path) {
                    DashboardAction::Notify {
                        level: "info".to_string(),
                        message: format!("Removed #{path} from \"{query_name}\""),
                    }
                } else {
                    DashboardAction::Notify {
                        level: "warn".to_string(),
                        message:
                            "No query focused. Focus a query chart or select one in the panel."
                                .to_string(),
                    }
                }
            }
            CommandResult::ShowTags => {
                // Show all tags as a notification for now
                let tags = self.custom_queries.all_tags();
                if tags.is_empty() {
                    DashboardAction::Notify {
                        level: "info".to_string(),
                        message: "No tags defined".to_string(),
                    }
                } else {
                    DashboardAction::Notify {
                        level: "info".to_string(),
                        message: format!("Tags: {}", tags.join(", ")),
                    }
                }
            }
            CommandResult::ToggleCommits => {
                self.toggle_commits_on_focused();
                DashboardAction::None
            }
            CommandResult::EnterDiffMode(offset) => {
                self.enter_diff_mode(offset);
                DashboardAction::None
            }
            CommandResult::SwapDiff => {
                self.swap_diff();
                DashboardAction::None
            }
            CommandResult::Success | CommandResult::Error(_) | CommandResult::None => {
                DashboardAction::None
            }
        }
    }

    /// Save the focused buffer (execute its query), optionally setting a name
    fn save_focused_buffer(&mut self, name: Option<String>) {
        if let Some(tile_id) = self.behavior.focused_tile() {
            if let Some(egui_tiles::Tile::Pane(component)) =
                self.viewport_tree.tiles.get_mut(tile_id)
            {
                // Try to downcast to QueryPane and save
                if let Some(query_pane) = component.as_any_mut().downcast_mut::<QueryPane>() {
                    if let Some(ref new_name) = name {
                        query_pane.set_name(new_name);
                    }
                    if query_pane.save() {
                        log::debug!("Buffer saved");
                    }
                } else if let Some(buffer) = component.as_any_mut().downcast_mut::<Buffer>() {
                    if let Some(ref new_name) = name {
                        buffer.set_name(new_name);
                    }
                    if buffer.save() {
                        log::debug!("Buffer saved");
                    }
                }
            }
        }
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

    /// Enter diff mode - shows side-by-side comparison view
    fn enter_diff_mode(&mut self, offset_str: Option<String>) {
        // Parse the offset or default to 7 days
        let offset = offset_str
            .as_ref()
            .and_then(|s| DiffOffset::parse(s))
            .unwrap_or(DiffOffset::OneWeek);

        // Get the focused metric name if available
        let metric_name = if let Some(tile_id) = self.behavior.focused_tile() {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    query_pane.name().to_string()
                } else {
                    "Untitled Metric".to_string()
                }
            } else {
                "Untitled Metric".to_string()
            }
        } else {
            "Demo Metric".to_string()
        };

        // Create a diff view with demo data
        let diff_view = DiffView::with_demo_data(&metric_name, offset);
        self.diff_view = Some(diff_view);
        self.diff_mode = true;

        log::debug!("Entered diff mode with offset: {}", offset.label());
    }

    /// Exit diff mode
    fn exit_diff_mode(&mut self) {
        self.diff_mode = false;
        self.diff_view = None;
        log::debug!("Exited diff mode");
    }

    /// Swap diff base and compare
    fn swap_diff(&mut self) {
        if let Some(ref mut diff_view) = self.diff_view {
            diff_view.swap();
            log::debug!("Swapped diff base and compare");
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
                    log::debug!("Opening buffer editor for QueryPane");
                } else if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                    let query = buffer.saved_content().to_string();
                    let name = buffer.name().to_string();
                    self.buffer_editor.open(&query, &name);
                    self.editing_tile_id = Some(tile_id);
                    log::debug!("Opening buffer editor for Buffer");
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
        // Populate the metrics finder with all metric info including tags
        let items: Vec<MetricItem> = self
            .metrics_tree
            .metrics()
            .iter()
            .map(|metric| MetricItem {
                name: metric.name.clone(),
                category: metric.category.label().to_string(),
                description: metric.description.clone(),
                unit: metric.unit.clone(),
                tags: metric.tags.clone(),
                series_count: metric.series_count,
            })
            .collect();

        self.metrics_finder.set_items(items);
        self.metrics_finder.open();
    }

    /// Open the query finder modal (for saved queries)
    pub fn open_query_finder(&mut self) {
        // Populate the query finder with saved queries
        let queries: Vec<QueryItem> = self
            .custom_queries
            .queries()
            .iter()
            .map(|q| QueryItem {
                id: q.id,
                name: q.name.clone(),
                query: q.query.clone(),
                tags: q.tags.clone(),
            })
            .collect();

        self.query_finder.set_queries(queries);
        self.query_finder.open();
    }

    /// Open the command palette modal
    pub fn open_command_palette(&mut self) {
        self.command_palette.open();
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

    /// Float the currently focused pane into a draggable window
    pub fn float_focused_pane(&mut self) {
        let focused_id = if let Some(id) = self.behavior.focused_tile() {
            id
        } else {
            // No pane focused - try to float the first available pane
            let pane_ids = self.get_pane_tile_ids();
            if let Some(&first_pane) = pane_ids.first() {
                self.behavior.set_focused_tile(Some(first_pane));
                first_pane
            } else {
                log::debug!("No panes to float");
                return;
            }
        };

        // Make sure it's actually a pane
        if let Some(Tile::Pane(_)) = self.viewport_tree.tiles.get(focused_id) {
            // Remove the pane from the tile tree and move it to floating
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.remove(focused_id) {
                let id = self.next_floating_id;
                self.next_floating_id += 1;

                let floating = FloatingWindow::new(id, component);
                self.floating_windows.push(floating);

                // Clear focus since the pane is now floating
                self.behavior.set_focused_tile(None);

                log::debug!("Floated pane {focused_id:?} as floating window {id}");
            }
        } else {
            log::debug!("Focused tile {focused_id:?} is not a pane");
        }
    }

    /// Dock a floating window back into the tiled layout
    fn dock_floating_window(&mut self, floating_id: u64) {
        // Find and remove the floating window
        let floating_idx = self
            .floating_windows
            .iter()
            .position(|w| w.id == floating_id);

        if let Some(idx) = floating_idx {
            let floating = self.floating_windows.remove(idx);

            // Insert the component back into the tile tree
            let new_tile_id = self.viewport_tree.tiles.insert_pane(floating.component);

            // Add to viewport
            if self.add_tile_to_viewport(new_tile_id) {
                self.behavior.set_focused_tile(Some(new_tile_id));
                log::debug!("Docked floating window {floating_id} back as tile {new_tile_id:?}");
            }
        }
    }

    /// Dock all floating windows back into the tiled layout
    pub fn dock_all_floating(&mut self) {
        let floating_ids: Vec<u64> = self.floating_windows.iter().map(|w| w.id).collect();
        for id in floating_ids {
            self.dock_floating_window(id);
        }
    }

    /// Check if there are any floating windows
    pub fn has_floating_windows(&self) -> bool {
        !self.floating_windows.is_empty()
    }

    /// Get the count of floating windows
    pub fn floating_window_count(&self) -> usize {
        self.floating_windows.len()
    }

    /// Add a chart for a custom query to the viewport
    fn add_chart_for_query(&mut self, query_name: &str, query_str: &str) {
        // Use query name as the unique key for duplicate detection
        let chart_key = format!("query:{query_name}");
        if self.open_charts.contains(&chart_key) {
            log::debug!("Chart for query '{query_name}' already open");
            return;
        }

        // Create a QueryPane with the query
        let pane: Box<dyn Component> = Box::new(QueryPane::with_name(query_str, query_name));
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.open_charts.insert(chart_key);
            self.behavior.set_focused_tile(Some(pane_tile));
            log::debug!("Added query pane for '{query_name}'");
        }
    }

    /// Create a new empty buffer in the viewport
    fn create_new_buffer(&mut self) {
        let buffer: Box<dyn Component> = Box::new(Buffer::new(""));
        let buffer_tile = self.viewport_tree.tiles.insert_pane(buffer);

        if self.add_tile_to_viewport(buffer_tile) {
            self.behavior.set_focused_tile(Some(buffer_tile));
            log::debug!("Created new buffer");
        }
    }

    /// Add a tile to the viewport, handling different container types
    /// Returns true if the tile was successfully added
    fn add_tile_to_viewport(&mut self, tile_id: TileId) -> bool {
        let Some(root_id) = self.viewport_tree.root() else {
            return false;
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
        // Close all floating windows
        self.floating_windows.clear();

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

    /// Get a reference to the metrics tree for reading selection state
    pub fn metrics_tree(&self) -> &MetricsTree {
        &self.metrics_tree
    }

    /// Get a mutable reference to the metrics tree
    pub fn metrics_tree_mut(&mut self) -> &mut MetricsTree {
        &mut self.metrics_tree
    }

    /// Check if the command palette is currently open
    pub fn is_command_palette_open(&self) -> bool {
        self.command_palette.is_open()
    }

    /// Check if the fuzzy finder is currently open
    pub fn is_metrics_finder_open(&self) -> bool {
        self.metrics_finder.is_open()
    }

    /// Get the number of open tabs/charts
    pub fn open_tabs_count(&self) -> usize {
        self.open_charts.len()
    }

    /// Get the currently selected metric name
    pub fn selected_metric(&self) -> Option<String> {
        self.metrics_tree.selection().metric.clone()
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

    /// Get the current tag filter display string
    pub fn tag_filter_display(&self) -> String {
        self.tag_filter.display()
    }

    /// Set the tag filter
    pub fn set_tag_filter(&mut self, path: Option<TagPath>) {
        self.tag_filter.set(path.clone());

        // Update the custom queries panel filter
        let tag_str = path.as_ref().map(|p| p.as_str());
        self.custom_queries.set_tag_filter(tag_str.as_deref());

        log::debug!("Tag filter set to: {}", self.tag_filter.display());
    }

    /// Get the name of the currently focused chart in the viewport
    fn focused_chart_name(&self) -> Option<String> {
        let tile_id = self.behavior.focused_tile()?;
        if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
            Some(component.name())
        } else {
            None
        }
    }

    /// Find a query by name and return its ID
    fn find_query_id_by_name(&self, name: &str) -> Option<u64> {
        self.custom_queries
            .queries()
            .iter()
            .find(|q| q.name == name)
            .map(|q| q.id)
    }

    /// Add a tag to the focused chart's query
    /// Returns Some(query_name) if successful, None if no matching query
    /// If the focused chart is a raw metric (not a saved query), auto-creates a query entry for it
    pub fn add_tag_to_focused(&mut self, tag: &TagPath) -> Option<String> {
        // First try the focused chart in the viewport
        if let Some(tile_id) = self.behavior.focused_tile() {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                let chart_name = component.name();

                // Check if this is already a saved query
                if let Some(query_id) = self.find_query_id_by_name(&chart_name) {
                    self.custom_queries
                        .add_tag_to_query(query_id, &tag.as_str());
                    log::debug!("Added tag '{tag}' to query '{chart_name}' (id: {query_id})");
                    return Some(chart_name);
                }

                // Not a saved query - try to auto-save it as a query
                // Get the query string from the pane
                let query_str = component
                    .as_any()
                    .downcast_ref::<QueryPane>()
                    .map(|qp| qp.saved_query().to_string())
                    .or_else(|| {
                        component
                            .as_any()
                            .downcast_ref::<Buffer>()
                            .map(|b| b.saved_content().to_string())
                    });

                if let Some(query_str) = query_str {
                    // Auto-create a custom query entry with the tag
                    self.custom_queries.add_query_with_tags(
                        &chart_name,
                        &query_str,
                        vec![tag.as_str()],
                    );
                    log::debug!(
                        "Auto-created query '{chart_name}' with tag '{tag}' (query: {query_str})"
                    );
                    return Some(chart_name);
                }

                log::debug!("Focused chart '{chart_name}' could not be converted to a query");
                return None;
            }
        }

        // Fall back to selected query in left panel
        if let Some(query_id) = self.custom_queries.selected() {
            self.custom_queries
                .add_tag_to_query(query_id, &tag.as_str());
            let query_name = self
                .custom_queries
                .queries()
                .iter()
                .find(|q| q.id == query_id)
                .map(|q| q.name.clone());
            log::debug!("Added tag '{tag}' to selected query {query_id}");
            return query_name;
        }

        log::debug!("No focused chart or selected query to add tag to");
        None
    }

    /// Remove a tag from the focused chart's query
    /// Returns Some(query_name) if successful, None if no matching query
    pub fn remove_tag_from_focused(&mut self, tag: &TagPath) -> Option<String> {
        // First try the focused chart in the viewport
        if let Some(chart_name) = self.focused_chart_name() {
            if let Some(query_id) = self.find_query_id_by_name(&chart_name) {
                self.custom_queries
                    .remove_tag_from_query(query_id, &tag.as_str());
                log::debug!("Removed tag '{tag}' from query '{chart_name}' (id: {query_id})");
                return Some(chart_name);
            }
            // Chart exists but is not a custom query (might be a metric)
            log::debug!(
                "Focused chart '{chart_name}' is not a custom query - tags only work on queries"
            );
            return None;
        }

        // Fall back to selected query in left panel
        if let Some(query_id) = self.custom_queries.selected() {
            self.custom_queries
                .remove_tag_from_query(query_id, &tag.as_str());
            let query_name = self
                .custom_queries
                .queries()
                .iter()
                .find(|q| q.id == query_id)
                .map(|q| q.name.clone());
            log::debug!("Removed tag '{tag}' from selected query {query_id}");
            return query_name;
        }

        log::debug!("No focused chart or selected query to remove tag from");
        None
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

        // Don't handle if fuzzy finder, command palette, buffer editor, multi-edit, or which-key is open
        if self.metrics_finder.is_open()
            || self.command_palette.is_open()
            || self.buffer_editor.is_open()
            || self.multi_edit_overlay.is_open()
            || self.which_key.is_open()
        {
            return None;
        }

        // Handle diff mode keyboard shortcuts first
        if self.diff_mode {
            let mut should_exit = false;
            ctx.input_mut(|input| {
                // X or Escape exits diff mode
                if input.consume_key(egui::Modifiers::NONE, egui::Key::X)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                {
                    should_exit = true;
                }
            });
            if should_exit {
                self.exit_diff_mode();
                ctx.request_repaint();
                return None;
            }
            // Don't process other keyboard shortcuts while in diff mode
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
        let mut should_edit_buffer = false;
        let mut should_toggle_zen = false;
        let mut should_toggle_fullscreen = false;
        let mut should_float_pane = false;
        let mut should_dock_all = false;
        let mut should_share_pane = false;
        let mut should_open_which_key = false;
        let mut should_enter_visual_multi = false;
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
            // W - float focused pane into a window
            if input.consume_key(egui::Modifiers::NONE, egui::Key::W) {
                should_float_pane = true;
                consumed = true;
                return;
            }
            // D - dock all floating windows
            if input.consume_key(egui::Modifiers::NONE, egui::Key::D) {
                should_dock_all = true;
                consumed = true;
                return;
            }

            // Ctrl+V - enter visual-block (multi-select) mode (requires a focused pane)
            if input.consume_key(egui::Modifiers::CTRL, egui::Key::V) && current_focus.is_some() {
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

        if should_open_which_key {
            self.which_key.open();
        } else if should_enter_visual_multi {
            if let Some(tile_id) = current_focus {
                self.enter_visual_multi_mode(tile_id);
            }
        } else if should_edit_buffer {
            self.edit_focused_buffer();
        } else if should_toggle_zen {
            self.toggle_zen_mode();
        } else if should_toggle_fullscreen {
            self.toggle_fullscreen();
        } else if should_float_pane {
            self.float_focused_pane();
        } else if should_dock_all {
            self.dock_all_floating();
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

    /// Enter visual-multi mode starting from the given pane
    fn enter_visual_multi_mode(&mut self, starting_tile_id: TileId) {
        log::debug!("Entering visual-multi mode with tile {starting_tile_id:?}");
        self.visual_multi_state = Some(VisualMultiState::new(starting_tile_id));
        // Sync the cursor to the behavior so the focus border is drawn
        self.behavior.set_focused_tile(Some(starting_tile_id));
    }

    /// Exit visual-multi mode
    fn exit_visual_multi_mode(&mut self) {
        log::debug!("Exiting visual-multi mode");
        self.visual_multi_state = None;
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
                "Visual-multi mode: cursor is now {:?}, {} selected",
                self.visual_multi_state
                    .as_ref()
                    .and_then(|s| s.cursor_tile_id),
                self.visual_multi_selection_count()
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

        if selected_ids.is_empty() {
            log::debug!("No panes selected for multi-edit");
            return;
        }

        // Collect excerpts from selected panes
        let mut excerpts = Vec::new();
        for tile_id in selected_ids {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                // Try to get query content from QueryPane
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    excerpts.push(EditExcerpt::new(
                        query_pane.id(),
                        query_pane.name().to_string(),
                        query_pane.query().to_string(),
                    ));
                }
                // Try to get content from Buffer
                else if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                    excerpts.push(EditExcerpt::new(
                        buffer.id(),
                        buffer.name().to_string(),
                        buffer.content().to_string(),
                    ));
                }
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
                metrics_panel: self.left_panel_visible,
                inspector: false, // Inspector panel removed
                zen_mode: self.zen_mode,
            },
            time: TimeConfig::from_preset(self.time_range_toolbar.time_range().preset),
            panes,
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
        self.left_panel_visible = workspace.view.metrics_panel;
        self.zen_mode = workspace.view.zen_mode;

        // Apply time range
        self.time_range_toolbar
            .set_preset(workspace.time.to_preset());

        // Clear existing panes
        self.clear_all_panes();

        // Add panes from workspace
        for pane_config in &workspace.panes {
            let mut query_pane = QueryPane::new(&pane_config.query);
            if !pane_config.name.is_empty() {
                query_pane.set_name(&pane_config.name);
            }
            if !pane_config.tag.is_empty() {
                query_pane.set_tag(&pane_config.tag);
            }

            // Apply query state
            let state = pane_config.to_query_state(&workspace.time.preset);
            query_pane.set_query_state(state);

            // Track the chart
            self.open_charts.insert(pane_config.query.clone());

            // Add to viewport
            let tile_id = self.viewport_tree.tiles.insert_pane(Box::new(query_pane));
            self.add_tile_to_viewport(tile_id);
        }

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
    theme: AppTheme,
    api_key: String,
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
    ) {
        self.is_visual_multi_mode = is_active;
        self.selected_tile_ids = selected_ids;
    }
}

impl egui_tiles::Behavior<Box<dyn Component>> for TreeBehavior {
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

        // In visual-multi mode, draw selection indicator for selected panes
        if is_selected {
            // Magenta/purple selection color to match V-MULTI status line color
            let selection_color = match self.theme {
                AppTheme::Light => egui::Color32::from_rgba_unmultiplied(180, 100, 180, 60),
                AppTheme::Dark => egui::Color32::from_rgba_unmultiplied(220, 140, 220, 50),
            };

            // Fill the entire tile with a subtle selection tint
            painter.rect_filled(rect, 4.0, selection_color);

            // Draw selection border
            let border_color = match self.theme {
                AppTheme::Light => egui::Color32::from_rgb(180, 100, 180),
                AppTheme::Dark => egui::Color32::from_rgb(220, 140, 220),
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
