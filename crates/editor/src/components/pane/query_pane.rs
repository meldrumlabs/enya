use egui::{Color32, RichText};

use crate::components::pane::visualization::{
    Visualization, VisualizationType, populate_demo_data,
};
use crate::components::util::id_generator::next_id_usize;
use crate::components::util::query_state::QueryState;
use crate::components::widget::buffer::{Buffer, BufferAction, BufferMode};
use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;

/// Render a skeleton loading state with shimmer effect
fn render_loading_state(ui: &mut egui::Ui, theme: AppTheme) {
    let time = ui.ctx().input(|i| i.time);
    let available = ui.available_size();

    // Skeleton colors - obsidian glass emerald style
    let base = palette::bg_elevated(theme);
    // Add subtle emerald tint to skeleton elements for cohesive look
    let skeleton_base = Color32::from_rgba_unmultiplied(
        base.r().saturating_sub(5),
        base.g().saturating_add(8), // subtle green tint
        base.b().saturating_add(3),
        base.a(),
    );
    // Richer emerald shimmer for glassy effect
    let shimmer_color = palette::accent::PRIMARY.gamma_multiply(0.3);

    // Calculate shimmer position (sweeps left to right)
    let shimmer_progress = ((time * 0.8) % 2.0) as f32; // 0.0 to 2.0, loops
    let shimmer_width = available.x * 0.4;
    let shimmer_x = (shimmer_progress - 0.5) * (available.x + shimmer_width);

    let padding = 24.0;
    let chart_area_top = 40.0;
    let chart_area_height = (available.y - chart_area_top - padding).max(60.0);

    // Allocate the full area
    let (full_rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
    let painter = ui.painter();

    // Y-axis skeleton (left side) - series of short horizontal lines
    let y_axis_x = full_rect.left() + padding;
    let y_axis_width = 40.0;
    for i in 0..5 {
        let y = full_rect.top() + chart_area_top + (i as f32 / 4.0) * chart_area_height;
        let line_rect =
            egui::Rect::from_min_size(egui::pos2(y_axis_x, y - 4.0), egui::vec2(y_axis_width, 8.0));
        painter.rect_filled(line_rect, 4.0, skeleton_base);
    }

    // Chart area skeleton (main area with grid-like pattern)
    let chart_left = y_axis_x + y_axis_width + 16.0;
    let chart_right = full_rect.right() - padding;
    let chart_width = chart_right - chart_left;

    // Horizontal grid lines
    for i in 0..5 {
        let y = full_rect.top() + chart_area_top + (i as f32 / 4.0) * chart_area_height;
        let line_rect = egui::Rect::from_min_size(
            egui::pos2(chart_left, y - 1.0),
            egui::vec2(chart_width, 2.0),
        );
        painter.rect_filled(line_rect, 1.0, skeleton_base.gamma_multiply(0.5));
    }

    // Fake data line skeleton (wavy placeholder)
    let line_y_center = full_rect.top() + chart_area_top + chart_area_height * 0.5;
    let line_rect = egui::Rect::from_min_size(
        egui::pos2(chart_left, line_y_center - 2.0),
        egui::vec2(chart_width, 4.0),
    );
    painter.rect_filled(line_rect, 2.0, skeleton_base);

    // X-axis skeleton (bottom) - time labels
    let x_axis_y = full_rect.top() + chart_area_top + chart_area_height + 8.0;
    for i in 0..6 {
        let x = chart_left + (i as f32 / 5.0) * chart_width - 20.0;
        let label_rect = egui::Rect::from_min_size(egui::pos2(x, x_axis_y), egui::vec2(40.0, 10.0));
        painter.rect_filled(label_rect, 4.0, skeleton_base);
    }

    // Shimmer overlay - diagonal gradient sweep
    let shimmer_rect = egui::Rect::from_min_size(
        egui::pos2(full_rect.left() + shimmer_x, full_rect.top()),
        egui::vec2(shimmer_width, available.y),
    );

    // Clip shimmer to our bounds
    let clipped = shimmer_rect.intersect(full_rect);
    if clipped.width() > 0.0 {
        // Create gradient effect with multiple rects
        let segments = 10;
        let segment_width = clipped.width() / segments as f32;
        for i in 0..segments {
            let alpha = {
                let t = i as f32 / segments as f32;
                // Bell curve for smooth fade in/out
                (-(t - 0.5).powi(2) * 8.0).exp()
            };
            let seg_rect = egui::Rect::from_min_size(
                egui::pos2(clipped.left() + i as f32 * segment_width, clipped.top()),
                egui::vec2(segment_width, clipped.height()),
            );
            painter.rect_filled(seg_rect, 0.0, shimmer_color.gamma_multiply(alpha));
        }
    }

    // Request repaint for smooth animation
    ui.ctx().request_repaint();
}

/// A QueryPane combines a Buffer (for editing queries) with a visualization.
/// This is the first-class "buffer" concept where:
/// - The buffer holds the query (e.g., "env:prod AND service:db")
/// - When saved (:w), the query is executed and the visualization updates
/// - Press 'e' to edit the query, Escape to return to normal mode
/// - Press 'cv' to cycle visualization type (time series, stat, etc.)
/// - Query state (aggregation, granularity) is a view preference set via keybindings
pub struct QueryPane {
    /// Unique identifier for this pane
    id: usize,
    /// The buffer holding the query
    buffer: Buffer,
    /// The visualization displaying results (time series, stat, etc.)
    visualization: Visualization,
    /// Current theme
    theme: AppTheme,
    /// API key
    api_key: String,
    /// Whether the buffer edit area is expanded (shown)
    buffer_expanded: bool,
    /// Query state (aggregation, granularity, time range)
    query_state: QueryState,
    /// User-defined tag for organizing panes (e.g., "Critical", "Warning")
    tag: String,
    /// Whether this pane needs a query refresh (set on save, cleared after execution)
    needs_refresh: bool,
    /// Whether a query is currently in flight (for loading state)
    is_loading: bool,
}

impl Default for QueryPane {
    fn default() -> Self {
        Self::new("")
    }
}

impl QueryPane {
    /// Create a new query pane with the given initial query
    pub fn new(query: impl Into<String>) -> Self {
        Self::with_visualization_type(query, VisualizationType::default())
    }

    /// Create a new query pane with a specific visualization type
    pub fn with_visualization_type(query: impl Into<String>, viz_type: VisualizationType) -> Self {
        let query = query.into();
        let id = next_id_usize();

        let buffer = Buffer::with_name(query.clone(), format!("Query {id}"));
        let mut visualization = Visualization::new(viz_type, &query);

        // Generate demo data for now
        populate_demo_data(&mut visualization, &query);

        Self {
            id,
            buffer,
            visualization,
            theme: AppTheme::default(),
            api_key: String::new(),
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            needs_refresh: false,
            is_loading: false,
        }
    }

    /// Create a query pane with a custom name
    pub fn with_name(query: impl Into<String>, name: impl Into<String>) -> Self {
        let mut pane = Self::new(query);
        pane.buffer.set_name(name);
        pane
    }

    /// Create a query pane with a custom name and tag
    pub fn with_name_and_tag(
        query: impl Into<String>,
        name: impl Into<String>,
        tag: impl Into<String>,
    ) -> Self {
        let mut pane = Self::new(query);
        pane.buffer.set_name(name);
        pane.tag = tag.into();
        pane
    }

    /// Create a query pane with demo data for a metric
    pub fn with_demo_metric(metric_name: impl Into<String>) -> Self {
        let metric = metric_name.into();
        let query = metric.clone(); // For demo, query is just the metric name
        let mut pane = Self::new(&query);
        // Use sequential "Query N" naming instead of metric name
        // so the pane name doesn't become misleading if user changes the query
        pane.visualization.set_metric_name(&metric);
        pane
    }

    /// Create a query pane for a real backend (no demo data, needs refresh)
    pub fn for_metric_with_number(metric_name: impl Into<String>, query_number: usize) -> Self {
        let metric = metric_name.into();
        let id = next_id_usize();
        let pane_name = format!("Query {query_number}");

        // Use the metric name as the default query (PromQL mode)
        let query = metric.clone();
        let buffer = Buffer::with_name(query.clone(), &pane_name);
        let visualization = Visualization::new(VisualizationType::default(), &pane_name);

        Self {
            id,
            buffer,
            visualization,
            theme: AppTheme::default(),
            api_key: String::new(),
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            needs_refresh: true, // Trigger query on first frame
            is_loading: false,
        }
    }

    /// Create a query pane with demo data and a specific query number
    pub fn with_demo_metric_numbered(metric_name: impl Into<String>, query_number: usize) -> Self {
        let metric = metric_name.into();
        let query = metric.clone();
        let id = next_id_usize();
        let pane_name = format!("Query {query_number}");

        let buffer = Buffer::with_name(query.clone(), &pane_name);
        let mut visualization = Visualization::new(VisualizationType::default(), &pane_name);
        visualization.set_metric_name(&metric);
        populate_demo_data(&mut visualization, &query);

        Self {
            id,
            buffer,
            visualization,
            theme: AppTheme::default(),
            api_key: String::new(),
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            needs_refresh: false,
            is_loading: false,
        }
    }

    /// Create a query pane with a full PromQL query, custom name, and demo data
    /// Useful for tutorial where we want editable label selectors
    pub fn with_demo_query_named(query: impl Into<String>, name: impl Into<String>) -> Self {
        let query = query.into();
        let pane_name = name.into();
        let id = next_id_usize();

        let buffer = Buffer::with_name(query.clone(), &pane_name);
        let mut visualization = Visualization::new(VisualizationType::default(), &pane_name);
        visualization.set_metric_name(&pane_name);
        populate_demo_data(&mut visualization, &query);

        Self {
            id,
            buffer,
            visualization,
            theme: AppTheme::default(),
            api_key: String::new(),
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            needs_refresh: false,
            is_loading: false,
        }
    }

    /// Create a query pane from workspace config with a specific query number
    pub fn from_config_numbered(query: &str, name: &str, query_number: usize) -> Self {
        let id = next_id_usize();
        // Use provided name if not empty, otherwise use sequential naming
        let pane_name = if name.is_empty() {
            format!("Query {query_number}")
        } else {
            name.to_string()
        };

        let buffer = Buffer::with_name(query.to_string(), &pane_name);
        let visualization = Visualization::new(VisualizationType::default(), &pane_name);

        Self {
            id,
            buffer,
            visualization,
            theme: AppTheme::default(),
            api_key: String::new(),
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            needs_refresh: true,
            is_loading: false,
        }
    }

    /// Get the current visualization type
    pub fn visualization_type(&self) -> VisualizationType {
        self.visualization.viz_type()
    }

    /// Cycle to the next visualization type
    pub fn cycle_visualization(&mut self) {
        self.visualization.cycle();
        // Re-populate demo data for the new visualization type
        let query = self.buffer.saved_content().to_string();
        populate_demo_data(&mut self.visualization, &query);
    }

    /// Set the visualization type explicitly
    pub fn set_visualization_type(&mut self, viz_type: VisualizationType) {
        if self.visualization.viz_type() != viz_type {
            let query = self.buffer.saved_content().to_string();
            self.visualization = Visualization::new(viz_type, &query);
            self.visualization.set_theme(self.theme);
            populate_demo_data(&mut self.visualization, &query);
        }
    }

    /// Get the pane ID
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get the current query (from buffer)
    pub fn query(&self) -> &str {
        self.buffer.content()
    }

    /// Get the saved query (what the chart is showing)
    pub fn saved_query(&self) -> &str {
        self.buffer.saved_content()
    }

    /// Get the buffer name
    pub fn name(&self) -> &str {
        self.buffer.name()
    }

    /// Set the buffer name
    pub fn set_name(&mut self, name: &str) {
        self.buffer.set_name(name);
    }

    /// Get the user-defined tag
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Set the user-defined tag
    pub fn set_tag(&mut self, tag: &str) {
        self.tag = tag.to_string();
    }

    /// Toggle commit markers visibility on the visualization (only for time series)
    pub fn toggle_commits(&mut self) {
        self.visualization.toggle_commits();
    }

    /// Get the buffer mode
    pub fn buffer_mode(&self) -> BufferMode {
        self.buffer.mode()
    }

    /// Check if buffer has unsaved changes
    pub fn is_modified(&self) -> bool {
        self.buffer.is_modified()
    }

    /// Enter edit mode (show buffer, enter insert mode)
    pub fn enter_edit_mode(&mut self) {
        self.buffer_expanded = true;
        self.buffer.enter_insert_mode();
    }

    /// Exit edit mode (optionally save first)
    pub fn exit_edit_mode(&mut self, save: bool) {
        if save && self.buffer.is_modified() {
            self.save();
        }
        self.buffer.enter_normal_mode();
    }

    /// Save the buffer and mark for refresh
    pub fn save(&mut self) -> bool {
        if self.buffer.save() {
            // Query changed, mark for refresh (Dashboard will execute query)
            self.needs_refresh = true;
            true
        } else {
            false
        }
    }

    /// Revert buffer changes
    pub fn revert(&mut self) {
        self.buffer.revert();
    }

    /// Set buffer content without saving (for real-time preview during multi-buffer editing)
    pub fn set_buffer_content(&mut self, content: &str) {
        self.buffer.set_content(content);
    }

    /// Set the query content and save it (used by the modal editor)
    pub fn set_query_and_save(&mut self, query: &str) {
        self.buffer.set_content(query);
        self.buffer.save();
        self.needs_refresh = true;
    }

    /// Set the query content, query state, and save (used by the modal editor)
    pub fn set_query_state_and_save(&mut self, query: &str, state: QueryState) {
        self.buffer.set_content(query);
        self.buffer.save();
        self.query_state = state;
        self.needs_refresh = true;
    }

    /// Get the current query state
    pub fn query_state(&self) -> &QueryState {
        &self.query_state
    }

    /// Set the query state (aggregation, granularity, time range)
    pub fn set_query_state(&mut self, state: QueryState) {
        self.query_state = state;
    }

    /// Toggle buffer visibility
    pub fn toggle_buffer(&mut self) {
        self.buffer_expanded = !self.buffer_expanded;
    }

    /// Set theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.buffer.set_theme(theme);
        self.visualization.set_theme(theme);
    }

    /// Set API key
    pub fn set_api_key(&mut self, key: &str) {
        self.api_key = key.to_string();
    }

    /// Refresh the visualization based on current saved query
    fn refresh_chart(&mut self) {
        let query = self.buffer.saved_content().to_string();
        self.visualization.clear();
        self.visualization.set_metric_name(&query);
        populate_demo_data(&mut self.visualization, &query);
    }

    /// Public method to refresh/reload the pane data
    pub fn refresh(&mut self) {
        self.refresh_chart();
    }

    /// Get a mutable reference to the visualization (for external query execution)
    pub fn visualization_mut(&mut self) -> &mut Visualization {
        &mut self.visualization
    }

    /// Check if this pane needs a query refresh
    pub fn needs_refresh(&self) -> bool {
        self.needs_refresh
    }

    /// Clear the refresh flag (called after query is executed)
    pub fn clear_refresh(&mut self) {
        self.needs_refresh = false;
    }

    /// Mark pane as needing refresh (called after buffer is saved)
    pub fn mark_needs_refresh(&mut self) {
        self.needs_refresh = true;
    }

    /// Check if this pane is currently loading (query in flight)
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    /// Set the loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    /// Render the query pane
    pub fn show(&mut self, ui: &mut egui::Ui) -> QueryPaneAction {
        let mut action = QueryPaneAction::None;
        let text_col = text_color(self.theme);

        ui.vertical(|ui| {
            // Buffer toggle bar (always visible)
            ui.horizontal(|ui| {
                // Toggle button for buffer visibility
                let toggle_icon = if self.buffer_expanded {
                    semantic_icons::nav::EXPAND
                } else {
                    semantic_icons::nav::COLLAPSE
                };

                let toggle_btn = egui::Button::new(
                    RichText::new(format!("{toggle_icon} Query"))
                        .color(text_col)
                        .size(12.0),
                )
                .fill(Color32::TRANSPARENT);

                if ui.add(toggle_btn).clicked() {
                    self.toggle_buffer();
                }

                // Mode indicator
                let mode_text = match self.buffer.mode() {
                    BufferMode::Normal => "NORMAL",
                    BufferMode::Insert => "INSERT",
                };
                let mode_color = match self.buffer.mode() {
                    BufferMode::Normal => text_col.gamma_multiply(0.5),
                    BufferMode::Insert => Color32::from_rgb(100, 180, 100),
                };
                ui.label(RichText::new(mode_text).color(mode_color).size(10.0));

                // Modified indicator
                if self.buffer.is_modified() {
                    ui.label(
                        RichText::new("[+]")
                            .color(Color32::from_rgb(220, 160, 50))
                            .size(10.0),
                    );
                }

                // Saved query preview (when collapsed)
                if !self.buffer_expanded {
                    ui.add_space(8.0);
                    let preview = self.buffer.saved_content();
                    let preview = if preview.len() > 50 {
                        format!("{}...", &preview[..50])
                    } else {
                        preview.to_string()
                    };
                    ui.label(
                        RichText::new(preview)
                            .color(text_col.gamma_multiply(0.6))
                            .size(11.0)
                            .italics(),
                    );
                }

                // Right side controls
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Edit button
                    if ui
                        .small_button(RichText::new(semantic_icons::action::EDIT).size(12.0))
                        .on_hover_text("Edit query (e)")
                        .clicked()
                    {
                        self.enter_edit_mode();
                    }
                });
            });

            // Buffer area (collapsible)
            if self.buffer_expanded {
                ui.add_space(4.0);

                let buffer_action = self.buffer.show(ui);

                match buffer_action {
                    BufferAction::ModeChanged(BufferMode::Normal) => {
                        // User pressed Escape - collapse buffer if no changes
                        if !self.buffer.is_modified() {
                            self.buffer_expanded = false;
                        }
                    }
                    BufferAction::Saved => {
                        // Query was saved - trigger refresh and collapse buffer
                        self.needs_refresh = true;
                        self.buffer_expanded = false;
                        action = QueryPaneAction::QueryChanged;
                    }
                    _ => {}
                }

                ui.add_space(4.0);
            }

            // Visualization area (takes remaining space)
            // Show loading state if query is in flight, otherwise show visualization
            if self.is_loading {
                render_loading_state(ui, self.theme);
            } else {
                self.visualization.show(ui);
            }
        });

        action
    }
}

/// Actions that can result from query pane interaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryPaneAction {
    /// No action
    None,
    /// Query was changed (buffer saved)
    QueryChanged,
}

/// Implement Component trait so QueryPane can be used in the dashboard
impl crate::components::Component for QueryPane {
    fn show(&mut self, ui: &mut egui::Ui) {
        QueryPane::show(self, ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        self.buffer.name().to_string()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        QueryPane::set_theme(self, theme);
    }

    fn set_api_key(&mut self, key: &str) {
        QueryPane::set_api_key(self, key);
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed
    }

    fn label(&self) -> egui::RichText {
        let icon = match self.buffer.mode() {
            BufferMode::Normal => self.visualization.viz_type().icon(),
            BufferMode::Insert => semantic_icons::action::EDIT,
        };

        let title = self.buffer.display_title();
        egui::RichText::new(format!("{icon} {title}"))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
