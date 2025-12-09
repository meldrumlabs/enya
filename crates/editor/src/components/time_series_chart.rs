use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui::{Color32, Key, RichText, Stroke};
use egui_plot::{Line, Plot, PlotBounds, PlotPoints};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;

/// Zoom factor for keyboard-based zoom controls
const ZOOM_FACTOR: f64 = 1.25;

/// Global counter for unique component IDs
static NEXT_ID: AtomicUsize = AtomicUsize::new(100);

/// A single data point in the time series
#[derive(Debug, Clone)]
pub struct DataPoint {
    /// Timestamp in seconds (Unix epoch or relative)
    pub timestamp: f64,
    /// The metric value
    pub value: f64,
}

/// A single series of data points
#[derive(Debug, Clone)]
pub struct Series {
    /// Display name for this series
    pub name: String,
    /// Tag values that identify this series (e.g., {"host": "server1"})
    pub tags: HashMap<String, String>,
    /// The data points
    pub points: Vec<DataPoint>,
    /// Color for this series (optional, will be auto-assigned if None)
    pub color: Option<Color32>,
}

impl Series {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tags: HashMap::new(),
            points: Vec::new(),
            color: None,
        }
    }

    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    pub fn with_points(mut self, points: Vec<DataPoint>) -> Self {
        self.points = points;
        self
    }

    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Build a label from the series name and tags
    pub fn label(&self) -> String {
        if self.tags.is_empty() {
            self.name.clone()
        } else {
            let tags: Vec<_> = self.tags.iter().map(|(k, v)| format!("{k}={v}")).collect();
            format!("{} {{{}}}", self.name, tags.join(", "))
        }
    }
}

/// Actions that can be triggered by zoom keybindings
#[derive(Debug, Clone, Copy, PartialEq)]
enum ZoomAction {
    None,
    ZoomInY,
    ZoomOutY,
    ZoomInX,
    ZoomOutX,
    ResetZoom,
    GoToStart,
    GoToEnd,
}

/// A time series chart component
pub struct TimeSeriesChart {
    /// Unique identifier for this chart
    id: usize,
    /// The metric name being displayed
    metric_name: String,
    /// All series to display
    series: Vec<Series>,
    /// Current theme
    theme: AppTheme,
    /// API key (not used currently, but required by Component trait)
    api_key: String,
    /// Whether to show the legend
    show_legend: bool,
    /// Y-axis label
    y_label: Option<String>,
    /// Chart title (shown in tab)
    title: String,
    /// Whether we're waiting for a second 'g' press (for gg command)
    pending_g: bool,
}

impl Default for TimeSeriesChart {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl TimeSeriesChart {
    pub fn new(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            title: name.clone(),
            metric_name: name,
            series: Vec::new(),
            theme: AppTheme::default(),
            api_key: String::new(),
            show_legend: true,
            y_label: None,
            pending_g: false,
        }
    }

    /// Create a chart with demo data for testing
    pub fn with_demo_data(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        let mut chart = Self::new(name.clone());

        // Generate some demo data
        let now = 1700000000.0; // Some fixed timestamp
        let duration = 86400.0; // 24 hours of data (easier to test gg/G navigation)
        let num_points = 240; // One point every 6 minutes

        // Series 1: Baseline with some noise
        let points1: Vec<DataPoint> = (0..num_points)
            .map(|i| {
                let t = now + (i as f64 / num_points as f64) * duration;
                let base = 50.0 + 20.0 * (t / 300.0).sin();
                let noise = (t * 17.0).sin() * 5.0;
                DataPoint {
                    timestamp: t,
                    value: base + noise,
                }
            })
            .collect();

        chart.add_series(
            Series::new(name.clone())
                .with_tag("host", "server1")
                .with_points(points1)
                .with_color(Color32::from_rgb(59, 130, 246)), // Blue
        );

        // Series 2: Higher values
        let points2: Vec<DataPoint> = (0..num_points)
            .map(|i| {
                let t = now + (i as f64 / num_points as f64) * duration;
                let base = 80.0 + 15.0 * (t / 200.0).cos();
                let noise = (t * 23.0).sin() * 3.0;
                DataPoint {
                    timestamp: t,
                    value: base + noise,
                }
            })
            .collect();

        chart.add_series(
            Series::new(name)
                .with_tag("host", "server2")
                .with_points(points2)
                .with_color(Color32::from_rgb(16, 185, 129)), // Green
        );

        chart
    }

    /// Add a series to the chart
    pub fn add_series(&mut self, series: Series) {
        self.series.push(series);
    }

    /// Clear all series
    pub fn clear(&mut self) {
        self.series.clear();
    }

    /// Set the metric name
    pub fn set_metric_name(&mut self, name: impl Into<String>) {
        self.metric_name = name.into();
        self.title = self.metric_name.clone();
    }

    /// Set the title (shown in tab)
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Set the Y-axis label
    pub fn set_y_label(&mut self, label: impl Into<String>) {
        self.y_label = Some(label.into());
    }

    /// Set whether to show the legend
    pub fn set_show_legend(&mut self, show: bool) {
        self.show_legend = show;
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Get the time range (min, max timestamps) of all data in the chart
    fn data_time_range(&self) -> Option<(f64, f64)> {
        let mut min_t = f64::MAX;
        let mut max_t = f64::MIN;

        for series in &self.series {
            for point in &series.points {
                min_t = min_t.min(point.timestamp);
                max_t = max_t.max(point.timestamp);
            }
        }

        if min_t <= max_t {
            Some((min_t, max_t))
        } else {
            None
        }
    }

    /// Handle keyboard input and return the appropriate zoom action
    fn handle_keyboard(&mut self, ctx: &egui::Context) -> ZoomAction {
        // Only handle keys when no text field is focused
        if ctx.memory(|mem| mem.focused().is_some()) {
            self.pending_g = false;
            return ZoomAction::None;
        }

        let action = ctx.input(|input| {
            // Zoom in Y-axis: + or =
            if input.key_pressed(Key::Plus)
                || (input.key_pressed(Key::Equals) && !input.modifiers.shift)
            {
                return ZoomAction::ZoomInY;
            }

            // Zoom out Y-axis: -
            if input.key_pressed(Key::Minus) {
                return ZoomAction::ZoomOutY;
            }

            // Zoom in X-axis: > or .
            if input.key_pressed(Key::Period) {
                return ZoomAction::ZoomInX;
            }

            // Zoom out X-axis: < or ,
            if input.key_pressed(Key::Comma) {
                return ZoomAction::ZoomOutX;
            }

            // Reset zoom: 0
            if input.key_pressed(Key::Num0) {
                return ZoomAction::ResetZoom;
            }

            // Go to end: G (shift + g)
            if input.key_pressed(Key::G) && input.modifiers.shift {
                return ZoomAction::GoToEnd;
            }

            // Go to start: g (without shift) - need double press
            if input.key_pressed(Key::G) && !input.modifiers.shift {
                return ZoomAction::GoToStart;
            }

            ZoomAction::None
        });

        // Handle gg (double g) state machine
        if action == ZoomAction::GoToStart {
            if self.pending_g {
                self.pending_g = false;
                return ZoomAction::GoToStart;
            } else {
                self.pending_g = true;
                return ZoomAction::None;
            }
        } else if action == ZoomAction::GoToEnd {
            self.pending_g = false;
            return ZoomAction::GoToEnd;
        } else if action != ZoomAction::None {
            self.pending_g = false;
        }

        action
    }

    /// Get a default color for series index
    fn series_color(&self, index: usize) -> Color32 {
        // A palette of distinct colors
        const PALETTE: &[Color32] = &[
            Color32::from_rgb(59, 130, 246), // Blue
            Color32::from_rgb(16, 185, 129), // Green
            Color32::from_rgb(245, 158, 11), // Amber
            Color32::from_rgb(239, 68, 68),  // Red
            Color32::from_rgb(139, 92, 246), // Purple
            Color32::from_rgb(236, 72, 153), // Pink
            Color32::from_rgb(14, 165, 233), // Cyan
            Color32::from_rgb(34, 197, 94),  // Emerald
        ];
        PALETTE[index % PALETTE.len()]
    }

    /// Render the chart
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_color = text_color(self.theme);

        if self.series.is_empty() {
            // Empty state
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new("No data to display")
                        .color(text_color.gamma_multiply(0.5))
                        .italics(),
                );
            });
            return;
        }

        // Handle keyboard zoom/navigation
        let zoom_action = self.handle_keyboard(ui.ctx());
        let data_time_range = self.data_time_range();

        // The plot - let egui_plot manage bounds internally via its ID-based memory
        let plot = Plot::new(format!("plot_{}", self.id))
            .legend(egui_plot::Legend::default().position(egui_plot::Corner::RightTop))
            .x_axis_label("Time")
            .y_axis_label(self.y_label.as_deref().unwrap_or("Value"))
            .show_axes(true)
            .show_grid(true)
            .allow_zoom(true)
            .allow_drag(true)
            .allow_scroll(true);

        plot.show(ui, |plot_ui| {
            // Apply zoom action inside the plot closure where we have access to plot_ui
            match zoom_action {
                ZoomAction::ZoomInY => {
                    plot_ui.zoom_bounds(
                        egui::Vec2::new(1.0, ZOOM_FACTOR as f32),
                        plot_ui.plot_bounds().center(),
                    );
                }
                ZoomAction::ZoomOutY => {
                    plot_ui.zoom_bounds(
                        egui::Vec2::new(1.0, 1.0 / ZOOM_FACTOR as f32),
                        plot_ui.plot_bounds().center(),
                    );
                }
                ZoomAction::ZoomInX => {
                    plot_ui.zoom_bounds(
                        egui::Vec2::new(ZOOM_FACTOR as f32, 1.0),
                        plot_ui.plot_bounds().center(),
                    );
                }
                ZoomAction::ZoomOutX => {
                    plot_ui.zoom_bounds(
                        egui::Vec2::new(1.0 / ZOOM_FACTOR as f32, 1.0),
                        plot_ui.plot_bounds().center(),
                    );
                }
                ZoomAction::ResetZoom => {
                    plot_ui.set_auto_bounds(egui::Vec2b::new(true, true));
                }
                ZoomAction::GoToStart => {
                    if let Some((min_t, _max_t)) = data_time_range {
                        let bounds = plot_ui.plot_bounds();
                        let width = bounds.max()[0] - bounds.min()[0];
                        let new_bounds = PlotBounds::from_min_max(
                            [min_t, bounds.min()[1]],
                            [min_t + width, bounds.max()[1]],
                        );
                        plot_ui.set_plot_bounds(new_bounds);
                    }
                }
                ZoomAction::GoToEnd => {
                    if let Some((_min_t, max_t)) = data_time_range {
                        let bounds = plot_ui.plot_bounds();
                        let width = bounds.max()[0] - bounds.min()[0];
                        let new_bounds = PlotBounds::from_min_max(
                            [max_t - width, bounds.min()[1]],
                            [max_t, bounds.max()[1]],
                        );
                        plot_ui.set_plot_bounds(new_bounds);
                    }
                }
                ZoomAction::None => {}
            }

            // Draw all series
            for (i, series) in self.series.iter().enumerate() {
                let color = series.color.unwrap_or_else(|| self.series_color(i));

                let points: PlotPoints<'_> = series
                    .points
                    .iter()
                    .map(|p| [p.timestamp, p.value])
                    .collect();

                let line = Line::new(series.label(), points)
                    .color(color)
                    .stroke(Stroke::new(2.0, color));

                plot_ui.line(line);
            }
        });

        // Legend below chart (if enabled and multiple series)
        if self.show_legend && self.series.len() > 1 {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (i, series) in self.series.iter().enumerate() {
                    let color = series.color.unwrap_or_else(|| self.series_color(i));

                    // Color indicator
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, color);

                    ui.label(
                        RichText::new(series.label())
                            .color(text_color.gamma_multiply(0.8))
                            .small(),
                    );

                    ui.add_space(16.0);
                }
            });
        }
    }
}

/// Implement Component trait so TimeSeriesChart can be used in the dashboard
impl super::Component for TimeSeriesChart {
    fn show(&mut self, ui: &mut egui::Ui) {
        TimeSeriesChart::show(self, ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        self.metric_name.clone()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    fn set_api_key(&mut self, key: &str) {
        self.api_key = key.to_string();
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed
    }

    fn label(&self) -> egui::RichText {
        egui::RichText::new(format!(
            "{} {}",
            egui_phosphor::regular::CHART_LINE,
            self.title
        ))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
