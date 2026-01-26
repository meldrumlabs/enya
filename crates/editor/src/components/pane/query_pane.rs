use egui::{Color32, RichText};

use crate::components::pane::time_series_chart::ChartInteraction;
use crate::components::pane::visualization::{
    Visualization, VisualizationType, populate_demo_data,
};
use crate::components::util::id_generator::next_id_usize;
use crate::components::util::query_state::QueryState;
use crate::components::widget::buffer::{Buffer, BufferAction, BufferMode};
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;

/// Render a skeleton loading state with shimmer effect
fn render_loading_state(ui: &mut egui::Ui, theme: AppTheme) {
    let time = ui.ctx().input(|i| i.time);
    let available = ui.available_size();

    // Skeleton colors - fully theme-aware styling
    let base = theme.bg_elevated();
    let accent = theme.accent_primary();
    // Blend base with a subtle amount of accent for theme-aware skeleton tint
    let skeleton_base = Color32::from_rgba_unmultiplied(
        (base.r() as f32 * 0.95 + accent.r() as f32 * 0.05) as u8,
        (base.g() as f32 * 0.95 + accent.g() as f32 * 0.05) as u8,
        (base.b() as f32 * 0.95 + accent.b() as f32 * 0.05) as u8,
        base.a(),
    );
    // Use theme accent for shimmer effect
    let shimmer_color = accent.gamma_multiply(0.3);

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
    /// Whether the buffer edit area is expanded (shown)
    buffer_expanded: bool,
    /// Query state (aggregation, granularity, time range)
    query_state: QueryState,
    /// User-defined tag for organizing panes (e.g., "Critical", "Warning")
    tag: String,
    /// Description providing context about the pane (shown on hover)
    description: String,
    /// Whether this pane needs a query refresh (set on save, cleared after execution)
    needs_refresh: bool,
    /// Whether a query is currently in flight (for loading state)
    is_loading: bool,
    /// Whether the user has manually overridden the visualization type.
    /// When true, auto-suggestion will not change the visualization type.
    has_user_override: bool,
    /// Whether edit was requested via button click (for workspace to pick up)
    edit_requested: bool,
    /// Whether the visualization type dropdown is open
    viz_dropdown_open: bool,
    /// Pending action to be consumed by the workspace (set during show, cleared on take)
    pending_action: Option<QueryPaneAction>,
    /// Whether this pane uses demo data (prevents re-querying on time range change)
    is_demo: bool,
    /// Pending demo refresh (deferred to next frame so loading animation can show)
    pending_demo_refresh: bool,
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
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            description: String::new(),
            needs_refresh: false,
            is_loading: false,
            has_user_override: false,
            edit_requested: false,
            viz_dropdown_open: false,
            pending_action: None,
            is_demo: false,
            pending_demo_refresh: false,
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
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            description: String::new(),
            needs_refresh: true, // Trigger query on first frame
            is_loading: false,
            has_user_override: false,
            edit_requested: false,
            viz_dropdown_open: false,
            pending_action: None,
            is_demo: false,
            pending_demo_refresh: false,
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
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            description: String::new(),
            needs_refresh: false,
            is_loading: false,
            has_user_override: false,
            edit_requested: false,
            viz_dropdown_open: false,
            pending_action: None,
            is_demo: true,
            pending_demo_refresh: false,
        }
    }

    /// Create a query pane with a PromQL query and custom name, for real backends.
    /// Triggers query on first frame.
    pub fn with_query_named(
        query: impl Into<String>,
        name: impl Into<String>,
        query_number: usize,
    ) -> Self {
        let query = query.into();
        let pane_name = name.into();
        let id = next_id_usize();

        let buffer = Buffer::with_name(query.clone(), format!("Query {query_number}"));
        let mut visualization = Visualization::new(VisualizationType::default(), &pane_name);
        visualization.set_metric_name(&pane_name);

        Self {
            id,
            buffer,
            visualization,
            theme: AppTheme::default(),
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            description: String::new(),
            needs_refresh: true, // Trigger query on first frame
            is_loading: false,
            has_user_override: false,
            edit_requested: false,
            viz_dropdown_open: false,
            pending_action: None,
            is_demo: false,
            pending_demo_refresh: false,
        }
    }

    /// Create a query pane with demo data and a custom name and query number.
    pub fn with_demo_query_named(
        query: impl Into<String>,
        name: impl Into<String>,
        query_number: usize,
    ) -> Self {
        let query = query.into();
        let pane_name = name.into();
        let id = next_id_usize();

        let buffer = Buffer::with_name(query.clone(), format!("Query {query_number}"));
        let mut visualization = Visualization::new(VisualizationType::default(), &pane_name);
        visualization.set_metric_name(&pane_name);
        populate_demo_data(&mut visualization, &query);

        Self {
            id,
            buffer,
            visualization,
            theme: AppTheme::default(),
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            description: String::new(),
            needs_refresh: false,
            is_loading: false,
            has_user_override: false,
            edit_requested: false,
            viz_dropdown_open: false,
            pending_action: None,
            is_demo: true,
            pending_demo_refresh: false,
        }
    }

    /// Create a query pane with a full PromQL query, custom name, and demo data
    /// Useful for tutorial where we want editable label selectors
    pub fn with_demo_query_tutorial(query: impl Into<String>, name: impl Into<String>) -> Self {
        Self::with_demo_query_named_unit(query, name, "")
    }

    /// Create a query pane with a full PromQL query, custom name, unit, and demo data
    /// Useful for tutorial where we want editable label selectors with proper units
    pub fn with_demo_query_named_unit(
        query: impl Into<String>,
        name: impl Into<String>,
        unit: impl Into<String>,
    ) -> Self {
        let query = query.into();
        let pane_name = name.into();
        let unit = unit.into();
        let id = next_id_usize();

        let buffer = Buffer::with_name(query.clone(), &pane_name);
        let mut visualization = Visualization::new(VisualizationType::default(), &pane_name);
        visualization.set_metric_name(&pane_name);
        if !unit.is_empty() {
            visualization.set_unit(&unit);
        }
        populate_demo_data(&mut visualization, &query);

        Self {
            id,
            buffer,
            visualization,
            theme: AppTheme::default(),
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            description: String::new(),
            needs_refresh: false,
            is_loading: false,
            has_user_override: false,
            edit_requested: false,
            viz_dropdown_open: false,
            pending_action: None,
            is_demo: true,
            pending_demo_refresh: false,
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
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
            description: String::new(),
            needs_refresh: true,
            is_loading: false,
            has_user_override: false,
            edit_requested: false,
            viz_dropdown_open: false,
            pending_action: None,
            is_demo: false,
            pending_demo_refresh: false,
        }
    }

    /// Get the current visualization type
    pub fn visualization_type(&self) -> VisualizationType {
        self.visualization.viz_type()
    }

    /// Cycle to the next visualization type (user action - sets override flag).
    pub fn cycle_visualization(&mut self) {
        self.visualization.cycle();
        self.has_user_override = true;
        // Re-populate demo data for the new visualization type
        let query = self.buffer.saved_content().to_string();
        populate_demo_data(&mut self.visualization, &query);
    }

    /// Set the visualization type explicitly (user action - sets override flag).
    pub fn set_visualization_type(&mut self, viz_type: VisualizationType) {
        if self.visualization.viz_type() != viz_type {
            let query = self.buffer.saved_content().to_string();
            self.visualization = Visualization::new(viz_type, &query);
            self.visualization.set_theme(self.theme);
            self.has_user_override = true;
            populate_demo_data(&mut self.visualization, &query);
        }
    }

    /// Set the visualization type from auto-suggestion (does not set override flag).
    ///
    /// Use this when applying automatic visualization suggestions based on query results.
    /// The visualization will be changed, but `has_user_override` remains false,
    /// allowing future auto-suggestions to update it again.
    pub fn set_visualization_type_auto(&mut self, viz_type: VisualizationType) {
        if self.visualization.viz_type() != viz_type {
            let query = self.buffer.saved_content().to_string();
            self.visualization = Visualization::new(viz_type, &query);
            self.visualization.set_theme(self.theme);
            // Note: does NOT set has_user_override = true
            // Note: does NOT populate demo data - real data is already being set
        }
    }

    /// Check if the user has manually overridden the visualization type.
    pub fn has_user_override(&self) -> bool {
        self.has_user_override
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

    /// Take the pending action (returns and clears it).
    /// Call this after rendering to check for drilldown interactions.
    pub fn take_pending_action(&mut self) -> Option<QueryPaneAction> {
        self.pending_action.take()
    }

    /// Set the user-defined tag
    pub fn set_tag(&mut self, tag: &str) {
        self.tag = tag.to_string();
    }

    /// Get the description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Set the description
    pub fn set_description(&mut self, description: &str) {
        self.description = description.to_string();
    }

    /// Set the unit suffix for values (e.g., "ms", "req/s", "%")
    pub fn set_unit(&mut self, unit: &str) {
        self.visualization.set_unit(unit);
    }

    /// Toggle commit markers visibility on the visualization (only for time series)
    pub fn toggle_commits(&mut self) {
        self.visualization.toggle_commits();
    }

    /// Set commit markers on the visualization (only for time series)
    pub fn set_commits(&mut self, commits: Vec<super::time_series_chart::CommitMarker>) {
        self.visualization.set_commits(commits);
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

    /// Refresh the visualization with demo data (for demo panes only)
    fn refresh_demo_chart(&mut self) {
        let query = self.buffer.saved_content().to_string();
        self.visualization.clear();
        self.visualization.set_metric_name(&query);
        populate_demo_data(&mut self.visualization, &query);
    }

    /// Public method to refresh/reload the pane data.
    /// For demo panes: defers refresh to next frame so loading animation shows.
    /// For real panes: marks as needing refresh (query executor will re-query).
    pub fn refresh(&mut self) {
        if self.is_demo {
            // Defer to next frame so loading animation can render
            self.pending_demo_refresh = true;
            self.is_loading = true;
        } else {
            self.needs_refresh = true;
        }
    }

    /// Process any pending demo refresh (called each frame in show())
    fn process_pending_demo_refresh(&mut self) {
        if self.pending_demo_refresh {
            self.refresh_demo_chart();
            self.pending_demo_refresh = false;
            self.is_loading = false;
        }
    }

    /// Get a reference to the visualization.
    pub fn visualization(&self) -> &Visualization {
        &self.visualization
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

    /// Mark pane as needing refresh (called after buffer is saved).
    /// Demo panes are skipped since they use synthetic data.
    pub fn mark_needs_refresh(&mut self) {
        // Don't mark demo panes for refresh - they use synthetic data
        if !self.is_demo {
            self.needs_refresh = true;
        }
    }

    /// Check if this pane uses demo data (synthetic data, not connected to a backend)
    pub fn is_demo(&self) -> bool {
        self.is_demo
    }

    /// Check if this pane is currently loading (query in flight)
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    /// Set the loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    /// Check if edit was requested via button click
    pub fn edit_requested(&self) -> bool {
        self.edit_requested
    }

    /// Clear the edit requested flag (called after workspace handles it)
    pub fn clear_edit_requested(&mut self) {
        self.edit_requested = false;
    }

    // ==================== Annotation Methods ====================

    /// Add an annotation to the visualization's chart.
    pub fn add_annotation(&mut self, annotation: super::annotation::Annotation) {
        self.visualization.add_annotation(annotation);
    }

    /// Update an existing annotation in the visualization's chart.
    pub fn update_annotation(&mut self, annotation: super::annotation::Annotation) {
        self.visualization.update_annotation(annotation);
    }

    /// Remove an annotation from the visualization's chart.
    pub fn remove_annotation(&mut self, id: super::annotation::AnnotationId) {
        self.visualization.remove_annotation(id);
    }

    /// Get all annotations from the visualization's chart.
    pub fn annotations(&self) -> Vec<&super::annotation::Annotation> {
        self.visualization.annotations()
    }

    /// Render the query pane
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) -> QueryPaneAction {
        // Process any pending demo refresh (deferred from previous frame)
        self.process_pending_demo_refresh();

        let mut action = QueryPaneAction::None;
        let text_col = text_color(self.theme);

        // Get the full pane rect for the edit button overlay
        let pane_rect = ui.available_rect_before_wrap();

        ui.vertical(|ui| {
            // Buffer editor bar (only visible when expanded for editing)
            if self.buffer_expanded {
                ui.horizontal(|ui| {
                    // Toggle button to collapse
                    let toggle_btn = egui::Button::new(
                        RichText::new(format!("{} Query", semantic_icons::nav::EXPAND))
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
                });
            }

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

                // Check for chart interactions (e.g., double-click for drilldown)
                if let Some(ChartInteraction::DrilldownLogs {
                    timestamp_secs,
                    metric_name,
                }) = self.visualization.take_interaction()
                {
                    action = QueryPaneAction::DrilldownLogs {
                        timestamp_secs,
                        metric_name,
                    };
                }
            }
        });

        // Toolbar overlay in top-right corner (only when buffer is collapsed)
        // Contains: info button (if description), visualization type dropdown, edit button
        if !self.buffer_expanded {
            let button_size = egui::vec2(24.0, 24.0);
            let spacing = 2.0;
            let has_description = !self.description.is_empty();

            // Edit button (rightmost)
            let edit_button_pos = egui::pos2(
                pane_rect.right() - button_size.x - 4.0,
                pane_rect.top() + 4.0,
            );
            let edit_button_rect = egui::Rect::from_min_size(edit_button_pos, button_size);

            let is_edit_hovered = ui.rect_contains_pointer(edit_button_rect);
            let edit_icon_color = if is_edit_hovered {
                text_col.gamma_multiply(0.9)
            } else {
                text_col.gamma_multiply(0.5)
            };

            let edit_response = ui.put(
                edit_button_rect,
                egui::Button::new(
                    RichText::new(semantic_icons::action::EDIT)
                        .color(edit_icon_color)
                        .size(14.0),
                )
                .fill(Color32::TRANSPARENT)
                .frame(false),
            );

            if edit_response.on_hover_text("Edit query (e)").clicked() {
                self.edit_requested = true;
            }

            // Visualization type dropdown (left of edit button)
            let viz_button_pos = egui::pos2(
                edit_button_pos.x - button_size.x - spacing,
                pane_rect.top() + 4.0,
            );

            // Info button (left of viz button, only if description exists)
            if has_description {
                let info_button_pos = egui::pos2(
                    viz_button_pos.x - button_size.x - spacing,
                    pane_rect.top() + 4.0,
                );
                let info_button_rect = egui::Rect::from_min_size(info_button_pos, button_size);

                let is_info_hovered = ui.rect_contains_pointer(info_button_rect);
                let info_icon_color = if is_info_hovered {
                    self.theme.accent_primary()
                } else {
                    text_col.gamma_multiply(0.5)
                };

                let info_response = ui.put(
                    info_button_rect,
                    egui::Button::new(
                        RichText::new(semantic_icons::status::INFO)
                            .color(info_icon_color)
                            .size(14.0),
                    )
                    .fill(Color32::TRANSPARENT)
                    .frame(false),
                );

                info_response.on_hover_text(&self.description);
            }
            let viz_button_rect = egui::Rect::from_min_size(viz_button_pos, button_size);

            let is_viz_hovered = ui.rect_contains_pointer(viz_button_rect);
            let viz_icon_color = if is_viz_hovered {
                text_col.gamma_multiply(0.9)
            } else {
                text_col.gamma_multiply(0.5)
            };

            let current_viz = self.visualization_type();
            let viz_response = ui.put(
                viz_button_rect,
                egui::Button::new(
                    RichText::new(semantic_icons::action::CHART)
                        .color(viz_icon_color)
                        .size(14.0),
                )
                .fill(Color32::TRANSPARENT)
                .frame(false),
            );

            let viz_tooltip = format!("Visualization: {} (click to change)", current_viz.label());
            let viz_response = viz_response.on_hover_text(viz_tooltip);

            // Toggle dropdown on click
            if viz_response.clicked() {
                self.viz_dropdown_open = !self.viz_dropdown_open;
            }

            // Show popup menu using Area
            if self.viz_dropdown_open {
                let popup_id = egui::Id::new(format!("viz_popup_{}", self.id));
                let area_response = egui::Area::new(popup_id)
                    .order(egui::Order::Foreground)
                    .fixed_pos(viz_response.rect.left_bottom() + egui::vec2(0.0, 2.0))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(120.0);
                            let mut clicked_type = None;
                            for viz_type in VisualizationType::all() {
                                let is_selected = *viz_type == current_viz;
                                let label = format!("{} {}", viz_type.icon(), viz_type.label());

                                let text = if is_selected {
                                    RichText::new(label).strong()
                                } else {
                                    RichText::new(label)
                                };

                                if ui.selectable_label(is_selected, text).clicked() {
                                    clicked_type = Some(*viz_type);
                                }
                            }
                            clicked_type
                        })
                    });

                // Handle selection and close popup
                if let Some(viz_type) = area_response.inner.inner {
                    self.set_visualization_type(viz_type);
                    self.viz_dropdown_open = false;
                }

                // Close if clicked outside
                if ui.input(|i| i.pointer.any_click())
                    && !area_response.response.contains_pointer()
                    && !viz_response.contains_pointer()
                {
                    self.viz_dropdown_open = false;
                }
            }
        }

        // Store actions that need to be consumed by the workspace (e.g., drilldown)
        if matches!(action, QueryPaneAction::DrilldownLogs { .. }) {
            self.pending_action = Some(action.clone());
        }

        action
    }
}

/// Actions that can result from query pane interaction
#[derive(Debug, Clone, PartialEq)]
pub enum QueryPaneAction {
    /// No action
    None,
    /// Query was changed (buffer saved)
    QueryChanged,
    /// User double-clicked on chart for logs drilldown
    DrilldownLogs {
        /// Timestamp in seconds where user clicked
        timestamp_secs: f64,
        /// The metric name for context
        metric_name: String,
    },
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

    fn label(&self) -> egui::RichText {
        let icon = match self.buffer.mode() {
            BufferMode::Normal => self.visualization.viz_type().icon(),
            BufferMode::Insert => semantic_icons::action::EDIT,
        };

        let title = self.buffer.display_title();
        egui::RichText::new(format!("{icon} {title}"))
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
