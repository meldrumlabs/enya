use std::collections::HashSet;

use egui_tiles::{SimplificationOptions, Tile, TileId, Tiles};

use crate::app::AppState;
use crate::components::{
    Component, CustomQueriesPanel, InspectorPanel, InspectorTarget, MetricStats, MetricsTree,
    TimeRangeToolbar, TimeSeriesChart, inspector_toggle_button,
};
use crate::theme::AppTheme;
use crate::ui::colors::text_color;

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
    /// Unified filter text for searching both metrics and custom queries
    filter: String,
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
            open_charts: HashSet::new(),
            pending_chart: None,
            time_range_toolbar: TimeRangeToolbar::new(),
            inspector: InspectorPanel::new(),
            last_selected_metric: None,
            filter: String::new(),
        }
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

        // Add a demo chart to show the UI
        let demo_chart: Box<dyn Component> = Box::new(TimeSeriesChart::with_demo_data(
            "tokio.runtime.total_park_count",
        ));
        let chart_tile = tiles.insert_pane(demo_chart);

        let root = tiles.insert_tab_tile(vec![chart_tile]);

        let viewport_tree = egui_tiles::Tree::new("viewport_tree", root, tiles);

        let mut open_charts = HashSet::new();
        open_charts.insert("tokio.runtime.total_park_count".to_string());

        Self {
            metrics_tree: MetricsTree::with_demo_metrics(),
            custom_queries: CustomQueriesPanel::with_demo_queries(),
            provided_expanded: true,
            custom_expanded: false,
            viewport_tree,
            behavior: TreeBehavior::default(),
            left_panel_width: Self::DEFAULT_PANEL_WIDTH,
            open_charts,
            pending_chart: None,
            time_range_toolbar: TimeRangeToolbar::new(),
            inspector: InspectorPanel::new(),
            last_selected_metric: None,
            filter: String::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, app_state: &AppState) {
        self.behavior.set_theme(app_state.theme);
        self.behavior
            .set_keys(app_state.settings.api_key.to_owned());

        // Update component themes
        self.metrics_tree.set_theme(app_state.theme);
        self.custom_queries.set_theme(app_state.theme);
        self.time_range_toolbar.set_theme(app_state.theme);
        self.inspector.set_theme(app_state.theme);

        // Handle adding a pending chart to the viewport
        if let Some(metric_name) = self.pending_chart.take() {
            self.add_chart_for_metric(&metric_name);
        }

        // Update inspector when metric selection changes
        self.update_inspector_from_selection();

        // Pass filter to both metrics tree and custom queries
        self.metrics_tree.set_filter(&self.filter);
        self.custom_queries.set_filter(&self.filter);

        // Left panel with Provided (metrics) and Custom (queries) sections
        let text_color = text_color(app_state.theme);
        egui::SidePanel::left("metrics_panel")
            .resizable(true)
            .default_width(self.left_panel_width)
            .width_range(Self::MIN_PANEL_WIDTH..=Self::MAX_PANEL_WIDTH)
            .show_inside(ui, |ui| {
                // Unified search box at the top
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                            .color(text_color.gamma_multiply(0.6)),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("Filter...")
                            .desired_width(ui.available_width() - 8.0),
                    );
                });

                ui.add_space(8.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Auto-expand sections when filter has matches
                    let has_filter = !self.filter.is_empty();
                    let provided_has_matches = self.metrics_tree.has_matching_metrics();
                    let custom_has_matches = self.custom_queries.has_matching_queries();

                    // "Provided" section - contains the metrics tree
                    let provided_header = format!("{} Provided", egui_phosphor::regular::PACKAGE);
                    let mut provided_header_builder = egui::CollapsingHeader::new(
                        egui::RichText::new(provided_header)
                            .color(text_color)
                            .strong(),
                    )
                    .id_salt("provided_section")
                    .default_open(self.provided_expanded);

                    // Force open when filtering and there are matches
                    if has_filter && provided_has_matches {
                        provided_header_builder = provided_header_builder.open(Some(true));
                    }

                    let provided_response = provided_header_builder.show(ui, |ui| {
                        self.metrics_tree.show(ui);

                        // Check if a metric was double-clicked (add chart action)
                        if let Some(metric_name) = self.metrics_tree.take_pending_chart() {
                            self.pending_chart = Some(metric_name);
                        }
                    });

                    // Update provided expanded state (only when not filtering)
                    if !has_filter {
                        if provided_response.fully_open() {
                            self.provided_expanded = true;
                        } else if provided_response.openness < 0.5 {
                            self.provided_expanded = false;
                        }
                    }

                    ui.add_space(4.0);

                    // "Custom" section - contains the custom queries
                    let custom_header = format!(
                        "{} Custom ({})",
                        egui_phosphor::regular::CODE,
                        self.custom_queries.queries().len()
                    );
                    let mut custom_header_builder = egui::CollapsingHeader::new(
                        egui::RichText::new(custom_header)
                            .color(text_color)
                            .strong(),
                    )
                    .id_salt("custom_section")
                    .default_open(self.custom_expanded);

                    // Force open when filtering and there are matches
                    if has_filter && custom_has_matches {
                        custom_header_builder = custom_header_builder.open(Some(true));
                    }

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

                    // Update custom expanded state (only when not filtering)
                    if !has_filter {
                        if custom_response.fully_open() {
                            self.custom_expanded = true;
                        } else if custom_response.openness < 0.5 {
                            self.custom_expanded = false;
                        }
                    }
                });
            });

        // Right area with toolbar and viewport
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Top toolbar with time range controls and inspector toggle
            egui::TopBottomPanel::top("time_range_toolbar")
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        // Time range controls on the left
                        self.time_range_toolbar.show(ui);

                        // Inspector toggle on the far right
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if inspector_toggle_button(
                                ui,
                                self.inspector.is_visible(),
                                app_state.theme,
                            )
                            .clicked()
                            {
                                self.inspector.toggle();
                            }
                        });
                    });
                    ui.add_space(4.0);
                });

            // Inspector panel (right side, collapsible)
            self.inspector.show(ui);

            // Main viewport area (tabbed charts/views)
            egui::CentralPanel::default().show_inside(ui, |ui| {
                self.viewport_tree.ui(&mut self.behavior, ui);
            });
        });
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

    /// Add a chart for the given metric to the viewport
    fn add_chart_for_metric(&mut self, metric_name: &str) {
        // Don't add duplicate charts
        if self.open_charts.contains(metric_name) {
            log::debug!("Chart for {metric_name} already open");
            return;
        }

        // Create the chart (with demo data for now)
        let chart: Box<dyn Component> = Box::new(TimeSeriesChart::with_demo_data(metric_name));
        let chart_tile = self.viewport_tree.tiles.insert_pane(chart);

        // Find the root tabs container and add the chart to it
        if let Some(root_id) = self.viewport_tree.root() {
            if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                self.viewport_tree.tiles.get_mut(root_id)
            {
                tabs.add_child(chart_tile);
                tabs.set_active(chart_tile);
                self.open_charts.insert(metric_name.to_string());
                log::debug!("Added chart for {metric_name}");
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

        // Find the root tabs container and add the chart to it
        if let Some(root_id) = self.viewport_tree.root() {
            if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                self.viewport_tree.tiles.get_mut(root_id)
            {
                tabs.add_child(chart_tile);
                tabs.set_active(chart_tile);
                self.open_charts.insert(chart_key);
                log::debug!("Added chart for query '{query_name}'");
            }
        }
    }

    /// Get a reference to the metrics tree for reading selection state
    pub fn metrics_tree(&self) -> &MetricsTree {
        &self.metrics_tree
    }

    /// Get a mutable reference to the metrics tree
    pub fn metrics_tree_mut(&mut self) -> &mut MetricsTree {
        &mut self.metrics_tree
    }
}

#[derive(Default, Clone)]
struct TreeBehavior {
    add_child_to: Option<egui_tiles::TileId>,
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
