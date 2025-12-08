use std::collections::HashSet;

use egui_tiles::{SimplificationOptions, Tile, TileId, Tiles};

use crate::app::AppState;
use crate::components::{
    CommandPalette, CommandResult, Component, CustomQueriesPanel, FuzzyFinder, FuzzyItem,
    InspectorPanel, InspectorTarget, LandingPage, LandingPageAction, MetricStats, MetricsTree,
    TimeRangeToolbar, TimeSeriesChart, inspector_toggle_button, metrics_panel_toggle_button,
};
use crate::theme::AppTheme;
use crate::ui::colors::text_color;

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
    /// Open settings
    OpenSettings,
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
    /// Inspector panel (right side, collapsible)
    inspector: InspectorPanel,
    /// Track the last selected metric to detect changes
    last_selected_metric: Option<String>,
    /// Fuzzy finder modal (telescope-style search)
    fuzzy_finder: FuzzyFinder,
    /// Command palette (neovim-style `:` commands)
    command_palette: CommandPalette,
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
            inspector: InspectorPanel::new(),
            last_selected_metric: None,
            fuzzy_finder: FuzzyFinder::new(),
            command_palette: CommandPalette::new(),
            zen_mode: false,
            fullscreen_tile: None,
            floating_windows: Vec::new(),
            next_floating_id: 1,
            landing_page: LandingPage::new(),
            show_landing: true,
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
            inspector: InspectorPanel::new(),
            last_selected_metric: None,
            fuzzy_finder: FuzzyFinder::new(),
            command_palette: CommandPalette::new(),
            zen_mode: false,
            fullscreen_tile: None,
            floating_windows: Vec::new(),
            next_floating_id: 1,
            landing_page: LandingPage::new(),
            show_landing: true, // Start with landing page
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

        // Update component themes
        self.metrics_tree.set_theme(app_state.theme);
        self.custom_queries.set_theme(app_state.theme);
        self.time_range_toolbar.set_theme(app_state.theme);
        self.inspector.set_theme(app_state.theme);
        self.landing_page.set_theme(app_state.theme);

        // Handle adding a pending chart to the viewport
        if let Some(metric_name) = self.pending_chart.take() {
            let action = self.add_chart_for_metric_with_tracking(&metric_name);
            if action != DashboardAction::None {
                return action;
            }
        }

        // Update inspector when metric selection changes
        self.update_inspector_from_selection();

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
                            egui_phosphor::regular::MAGNIFYING_GLASS
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
                        let provided_header =
                            format!("{} Provided", egui_phosphor::regular::PACKAGE);
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
                            egui_phosphor::regular::CODE,
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
            self.open_fuzzy_finder();
        }

        // Right area with toolbar and viewport
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Top toolbar with time range controls and inspector toggle (hidden in zen mode)
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

                            // Inspector toggle on the far right
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if inspector_toggle_button(
                                        ui,
                                        self.inspector.is_visible(),
                                        app_state.theme,
                                    )
                                    .clicked()
                                    {
                                        self.inspector.toggle();
                                    }
                                },
                            );
                        });
                        ui.add_space(4.0);
                    });

                // Inspector panel (right side, collapsible) - hidden in zen mode
                self.inspector.show(ui);
            }

            // Main viewport area (tabbed charts/views)
            egui::CentralPanel::default().show_inside(ui, |ui| {
                // Check if we're in fullscreen mode for a specific pane
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
                } else {
                    self.viewport_tree.ui(&mut self.behavior, ui);
                }
            });
        });

        // Show fuzzy finder modal (rendered on top of everything)
        self.fuzzy_finder.set_theme(app_state.theme);
        if let Some(selected_item) = self.fuzzy_finder.show(ctx) {
            return self.handle_fuzzy_selection_with_tracking(selected_item);
        }

        // Show command palette modal
        self.command_palette.set_theme(app_state.theme);
        let cmd_result = self.command_palette.show(ctx);

        // Show floating windows
        self.show_floating_windows(ctx, app_state.theme);

        // Handle vim-style keyboard navigation for viewport
        self.handle_viewport_keyboard(ctx);

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
            LandingPageAction::OpenWorkspace { name: _ } => {
                // TODO: Implement workspace loading
                log::debug!("Workspace loading not yet implemented");
            }
            LandingPageAction::OpenFuzzyFinder => {
                self.open_fuzzy_finder();
            }
            LandingPageAction::OpenSettings => {
                return DashboardAction::OpenSettings;
            }
            LandingPageAction::ShowHelp => {
                return DashboardAction::ShowHelp;
            }
            LandingPageAction::NewPlot => {
                // Open fuzzy finder to select a metric for a new plot
                self.open_fuzzy_finder();
            }
            LandingPageAction::None => {}
        }

        // Show fuzzy finder modal (rendered on top of everything)
        self.fuzzy_finder.set_theme(app_state.theme);
        if let Some(selected_item) = self.fuzzy_finder.show(ctx) {
            return self.handle_fuzzy_selection_with_tracking(selected_item);
        }

        // Show command palette modal
        self.command_palette.set_theme(app_state.theme);
        let cmd_result = self.command_palette.show(ctx);

        self.handle_command_result(cmd_result)
    }

    /// Add a chart for a metric and return a tracking action
    fn add_chart_for_metric_with_tracking(&mut self, metric_name: &str) -> DashboardAction {
        // Don't add duplicate charts
        if self.open_charts.contains(metric_name) {
            log::debug!("Chart for {metric_name} already open");
            return DashboardAction::None;
        }

        // Create the chart (with demo data for now)
        let chart: Box<dyn Component> = Box::new(TimeSeriesChart::with_demo_data(metric_name));
        let chart_tile = self.viewport_tree.tiles.insert_pane(chart);

        if self.add_tile_to_viewport(chart_tile) {
            self.open_charts.insert(metric_name.to_string());
            self.behavior.set_focused_tile(Some(chart_tile));
            self.show_landing = false;
            log::debug!("Added chart for {metric_name}");

            // Return action to track this in recent plots
            return DashboardAction::TrackRecentPlot {
                name: metric_name.to_string(),
                metric_name: metric_name.to_string(),
                is_query: false,
            };
        }

        DashboardAction::None
    }

    /// Handle fuzzy selection and return tracking action
    fn handle_fuzzy_selection_with_tracking(&mut self, item: FuzzyItem) -> DashboardAction {
        match item {
            FuzzyItem::Metric { name, .. } => {
                self.show_landing = false;
                self.add_chart_for_metric_with_tracking(&name)
            }
            FuzzyItem::CustomQuery { name, query, .. } => {
                self.show_landing = false;
                self.add_chart_for_query(&name, &query);
                DashboardAction::TrackRecentPlot {
                    name: name.clone(),
                    metric_name: name,
                    is_query: true,
                }
            }
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
                egui::RichText::new(format!("{} {}", egui_phosphor::regular::CHART_LINE, title))
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
                            .small_button(egui::RichText::new(egui_phosphor::regular::LAYOUT))
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
            CommandResult::ToggleInspectorPanel => {
                self.inspector.toggle();
                DashboardAction::None
            }
            CommandResult::OpenSearch => {
                self.open_fuzzy_finder();
                DashboardAction::None
            }
            CommandResult::OpenSettings => DashboardAction::OpenSettings,
            CommandResult::ShowHelp => DashboardAction::ShowHelp,
            CommandResult::CloseTab => {
                // TODO: Implement tab closing
                log::debug!("Close tab command received");
                DashboardAction::None
            }
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
            CommandResult::Success | CommandResult::Error(_) | CommandResult::None => {
                DashboardAction::None
            }
        }
    }

    /// Open the fuzzy finder modal
    pub fn open_fuzzy_finder(&mut self) {
        // Populate the fuzzy finder with current items
        let mut items = Vec::new();

        // Add all metrics
        for metric in self.metrics_tree.metrics() {
            items.push(FuzzyItem::Metric {
                name: metric.name.clone(),
                category: metric.category.label().to_string(),
                description: metric.description.clone(),
            });
        }

        // Add all custom queries
        for query in self.custom_queries.queries() {
            items.push(FuzzyItem::CustomQuery {
                id: query.id,
                name: query.name.clone(),
                query: query.query.clone(),
            });
        }

        self.fuzzy_finder.set_items(items);
        self.fuzzy_finder.open();
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

    /// Update the inspector panel based on the current metric selection
    fn update_inspector_from_selection(&mut self) {
        let current_selection = self.metrics_tree.selection().metric.clone();

        // Only update if selection changed
        if current_selection != self.last_selected_metric {
            self.last_selected_metric = current_selection.clone();

            if let Some(metric_name) = current_selection {
                // Find the metric info
                if let Some(metric_info) = self.metrics_tree.get_metric(&metric_name) {
                    let target = InspectorTarget::Metric {
                        name: metric_info.name.clone(),
                        description: metric_info.description.clone(),
                        unit: metric_info.unit.clone(),
                        tags: metric_info
                            .tags
                            .iter()
                            .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
                            .collect(),
                        series_count: metric_info.series_count,
                    };
                    self.inspector.set_target(target);
                    // Set demo stats for now
                    self.inspector.set_stats(Some(MetricStats::demo()));
                }
            } else {
                self.inspector.clear();
            }
        }
    }

    /// Add a chart for a custom query to the viewport
    fn add_chart_for_query(&mut self, query_name: &str, query_str: &str) {
        // Use query name as the unique key for duplicate detection
        let chart_key = format!("query:{query_name}");
        if self.open_charts.contains(&chart_key) {
            log::debug!("Chart for query '{query_name}' already open");
            return;
        }

        // Create the chart with a custom title showing the query
        let mut chart = TimeSeriesChart::with_demo_data(query_name);
        chart.set_title(format!("{query_name} [{query_str}]"));
        let chart: Box<dyn Component> = Box::new(chart);
        let chart_tile = self.viewport_tree.tiles.insert_pane(chart);

        if self.add_tile_to_viewport(chart_tile) {
            self.open_charts.insert(chart_key);
            self.behavior.set_focused_tile(Some(chart_tile));
            log::debug!("Added chart for query '{query_name}'");
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
    pub fn is_fuzzy_finder_open(&self) -> bool {
        self.fuzzy_finder.is_open()
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
    /// Returns true if a key was consumed
    pub fn handle_viewport_keyboard(&mut self, ctx: &egui::Context) -> bool {
        // Don't handle keys if a text field or modal has focus
        if ctx.memory(|mem| mem.focused().is_some()) {
            return false;
        }

        // Don't handle if fuzzy finder or command palette is open
        if self.fuzzy_finder.is_open() || self.command_palette.is_open() {
            return false;
        }

        let pane_ids = self.get_pane_tile_ids();
        let current_focus = self.behavior.focused_tile();

        let mut consumed = false;
        let mut should_clear_focus = false;
        let mut should_close_focused = false;
        let mut should_toggle_zen = false;
        let mut should_toggle_fullscreen = false;
        let mut should_float_pane = false;
        let mut should_dock_all = false;
        let mut new_tile_id: Option<TileId> = None;

        ctx.input_mut(|input| {
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

        if should_toggle_zen {
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
        }

        if consumed {
            ctx.request_repaint();
            log::debug!(
                "Viewport navigation: focus is now {:?}",
                self.behavior.focused_tile()
            );
        }

        consumed
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
}

#[derive(Default, Clone)]
struct TreeBehavior {
    add_child_to: Option<egui_tiles::TileId>,
    /// Currently focused tile for vim-style navigation
    focused_tile_id: Option<egui_tiles::TileId>,
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
        // Draw focus border on top of the entire tile (including tab bar)
        if self.focused_tile_id == Some(tile_id) {
            // White/gray focus color to match Enya's color scheme
            let focus_color = match self.theme {
                AppTheme::Light => egui::Color32::from_rgb(120, 120, 130),
                AppTheme::Dark => egui::Color32::from_rgb(200, 200, 210),
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
