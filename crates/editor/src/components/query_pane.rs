use std::sync::atomic::{AtomicUsize, Ordering};

use egui::{Color32, RichText};

use crate::components::buffer::{Buffer, BufferAction, BufferMode};
use crate::components::query_state::QueryState;
use crate::components::time_series_chart::{DataPoint, Series, TimeSeriesChart};
use crate::theme::AppTheme;
use crate::ui::colors::text_color;

/// Global counter for unique pane IDs
static NEXT_PANE_ID: AtomicUsize = AtomicUsize::new(1000);

/// A QueryPane combines a Buffer (for editing queries) with a TimeSeriesChart (for visualization).
/// This is the first-class "buffer" concept where:
/// - The buffer holds the query (e.g., "env:prod AND service:db")
/// - When saved (:w), the query is executed and the chart updates
/// - Press 'e' to edit the query, Escape to return to normal mode
/// - Query state (aggregation, granularity) is a view preference set via keybindings
pub struct QueryPane {
    /// Unique identifier for this pane
    id: usize,
    /// The buffer holding the query
    buffer: Buffer,
    /// The chart displaying results
    chart: TimeSeriesChart,
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
}

impl Default for QueryPane {
    fn default() -> Self {
        Self::new("")
    }
}

impl QueryPane {
    /// Create a new query pane with the given initial query
    pub fn new(query: impl Into<String>) -> Self {
        let query = query.into();
        let id = NEXT_PANE_ID.fetch_add(1, Ordering::Relaxed);

        let buffer = Buffer::with_name(query.clone(), format!("Query {id}"));
        let mut chart = TimeSeriesChart::new(&query);

        // Generate demo data for now
        Self::populate_demo_data(&mut chart, &query);

        Self {
            id,
            buffer,
            chart,
            theme: AppTheme::default(),
            api_key: String::new(),
            buffer_expanded: false,
            query_state: QueryState::default(),
            tag: String::new(),
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
        let name = metric_name.into();
        let query = name.clone(); // For demo, query is just the metric name
        let mut pane = Self::new(&query);
        pane.buffer.set_name(&name);
        pane.chart.set_metric_name(&name);
        pane
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

    /// Save the buffer and refresh the chart
    pub fn save(&mut self) -> bool {
        if self.buffer.save() {
            // Query changed, refresh the chart
            self.refresh_chart();
            true
        } else {
            false
        }
    }

    /// Revert buffer changes
    pub fn revert(&mut self) {
        self.buffer.revert();
    }

    /// Set the query content and save it (used by the modal editor)
    pub fn set_query_and_save(&mut self, query: &str) {
        self.buffer.set_content(query);
        self.buffer.save();
        self.refresh_chart();
    }

    /// Set the query content, query state, and save (used by the modal editor)
    pub fn set_query_state_and_save(&mut self, query: &str, state: QueryState) {
        self.buffer.set_content(query);
        self.buffer.save();
        self.query_state = state;
        self.refresh_chart();
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
        self.chart.set_theme(theme);
    }

    /// Set API key
    pub fn set_api_key(&mut self, key: &str) {
        self.api_key = key.to_string();
    }

    /// Refresh the chart based on current saved query
    fn refresh_chart(&mut self) {
        let query = self.buffer.saved_content().to_string();
        self.chart.clear();
        self.chart.set_metric_name(&query);
        Self::populate_demo_data(&mut self.chart, &query);
    }

    /// Populate demo data for a chart (temporary until real data fetching)
    fn populate_demo_data(chart: &mut TimeSeriesChart, query: &str) {
        // Generate some demo data based on query hash for variety
        let hash = query
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_add(b as u64));
        let now = 1_700_000_000.0;
        let duration = 86400.0; // 24 hours of data (easier to test gg/G navigation)
        let num_points = 240; // One point every 6 minutes

        // Series 1
        let base1 = 50.0 + (hash % 50) as f64;
        let freq1 = 200.0 + (hash % 100) as f64;
        let points1: Vec<DataPoint> = (0..num_points)
            .map(|i| {
                let t = now + (i as f64 / num_points as f64) * duration;
                let base = base1 + 20.0 * (t / freq1).sin();
                let noise = (t * 17.0).sin() * 5.0;
                DataPoint {
                    timestamp: t,
                    value: base + noise,
                }
            })
            .collect();

        chart.add_series(
            Series::new(query)
                .with_tag("host", "server1")
                .with_points(points1)
                .with_color(Color32::from_rgb(59, 130, 246)),
        );

        // Series 2
        let base2 = 70.0 + (hash % 30) as f64;
        let freq2 = 150.0 + (hash % 80) as f64;
        let points2: Vec<DataPoint> = (0..num_points)
            .map(|i| {
                let t = now + (i as f64 / num_points as f64) * duration;
                let base = base2 + 15.0 * (t / freq2).cos();
                let noise = (t * 23.0).sin() * 3.0;
                DataPoint {
                    timestamp: t,
                    value: base + noise,
                }
            })
            .collect();

        chart.add_series(
            Series::new(query)
                .with_tag("host", "server2")
                .with_points(points2)
                .with_color(Color32::from_rgb(16, 185, 129)),
        );
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
                    egui_phosphor::regular::CARET_DOWN
                } else {
                    egui_phosphor::regular::CARET_RIGHT
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
                        .small_button(
                            RichText::new(egui_phosphor::regular::PENCIL_SIMPLE).size(12.0),
                        )
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
                        action = QueryPaneAction::QueryChanged;
                    }
                    _ => {}
                }

                ui.add_space(4.0);
            }

            // Chart area (takes remaining space)
            self.chart.show(ui);
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
impl super::Component for QueryPane {
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
            BufferMode::Normal => egui_phosphor::regular::CHART_LINE,
            BufferMode::Insert => egui_phosphor::regular::PENCIL_SIMPLE,
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
