use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui::{Color32, Key, RichText, Stroke};
use egui_plot::{Line, LineStyle, Plot, PlotBounds, PlotPoints, VLine};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;

// Re-export CommitMarker from common crate
pub use enya_common::CommitMarker;

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
enum ChartAction {
    None,
    ZoomInY,
    ZoomOutY,
    ZoomInX,
    ZoomOutX,
    ResetZoom,
    GoToStart,
    GoToEnd,
    /// Navigate to next commit marker (])
    NextCommit,
    /// Navigate to previous commit marker ([)
    PrevCommit,
}

/// A time series chart component
pub struct TimeSeriesChart {
    /// Unique identifier for this chart
    id: usize,
    /// The metric name being displayed
    metric_name: String,
    /// All series to display
    series: Vec<Series>,
    /// Git commit markers to display as vertical annotations
    commits: Vec<CommitMarker>,
    /// Whether to show commit markers
    show_commits: bool,
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
    /// Whether we're waiting for 'c' after '[' or ']' (for commit navigation)
    pending_bracket: Option<char>,
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
            commits: Vec::new(),
            show_commits: true,
            theme: AppTheme::default(),
            api_key: String::new(),
            show_legend: true,
            y_label: None,
            pending_g: false,
            pending_bracket: None,
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
                .with_color(Color32::from_rgb(99, 179, 237)), // Soft sky blue
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
                .with_color(Color32::from_rgb(94, 234, 212)), // Soft teal
        );

        // Add some demo commit markers spread across the time range
        chart.add_commit(CommitMarker::new(
            "a1b2c3d",
            now + duration * 0.1,
            "Fix connection pooling",
        ));
        chart.add_commit(CommitMarker::new(
            "e4f5g6h",
            now + duration * 0.35,
            "Add retry logic",
        ));
        chart.add_commit(CommitMarker::new(
            "i7j8k9l",
            now + duration * 0.5,
            "Update dependencies",
        ));
        chart.add_commit(CommitMarker::new(
            "m0n1o2p",
            now + duration * 0.7,
            "Refactor auth module",
        ));
        chart.add_commit(CommitMarker::new(
            "q3r4s5t",
            now + duration * 0.9,
            "Performance improvements",
        ));

        chart
    }

    /// Add a commit marker to the chart
    pub fn add_commit(&mut self, commit: CommitMarker) {
        self.commits.push(commit);
        // Keep commits sorted by timestamp for navigation
        self.commits.sort_by(|a, b| {
            a.timestamp
                .partial_cmp(&b.timestamp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Set all commit markers at once
    pub fn set_commits(&mut self, commits: Vec<CommitMarker>) {
        self.commits = commits;
        self.commits.sort_by(|a, b| {
            a.timestamp
                .partial_cmp(&b.timestamp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Clear all commit markers
    pub fn clear_commits(&mut self) {
        self.commits.clear();
    }

    /// Set whether to show commit markers
    pub fn set_show_commits(&mut self, show: bool) {
        self.show_commits = show;
    }

    /// Toggle commit markers visibility
    pub fn toggle_commits(&mut self) {
        self.show_commits = !self.show_commits;
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

    /// Handle keyboard input and return the appropriate chart action
    fn handle_keyboard(&mut self, ctx: &egui::Context) -> ChartAction {
        // Only handle keys when no text field is focused
        if ctx.memory(|mem| mem.focused().is_some()) {
            self.pending_g = false;
            self.pending_bracket = None;
            return ChartAction::None;
        }

        let action = ctx.input(|input| {
            // Check for 'c' after pending bracket for commit navigation
            if self.pending_bracket.is_some() && input.key_pressed(Key::C) {
                return match self.pending_bracket {
                    Some(']') => ChartAction::NextCommit,
                    Some('[') => ChartAction::PrevCommit,
                    _ => ChartAction::None,
                };
            }

            // Check for bracket keys ([ and ])
            if input.key_pressed(Key::OpenBracket) {
                return ChartAction::None; // Will be handled in state machine
            }
            if input.key_pressed(Key::CloseBracket) {
                return ChartAction::None; // Will be handled in state machine
            }

            // Zoom in Y-axis: + or =
            if input.key_pressed(Key::Plus)
                || (input.key_pressed(Key::Equals) && !input.modifiers.shift)
            {
                return ChartAction::ZoomInY;
            }

            // Zoom out Y-axis: -
            if input.key_pressed(Key::Minus) {
                return ChartAction::ZoomOutY;
            }

            // Zoom in X-axis: > or .
            if input.key_pressed(Key::Period) {
                return ChartAction::ZoomInX;
            }

            // Zoom out X-axis: < or ,
            if input.key_pressed(Key::Comma) {
                return ChartAction::ZoomOutX;
            }

            // Reset zoom: 0
            if input.key_pressed(Key::Num0) {
                return ChartAction::ResetZoom;
            }

            // Go to end: G (shift + g)
            if input.key_pressed(Key::G) && input.modifiers.shift {
                return ChartAction::GoToEnd;
            }

            // Go to start: g (without shift) - need double press
            if input.key_pressed(Key::G) && !input.modifiers.shift {
                return ChartAction::GoToStart;
            }

            ChartAction::None
        });

        // Handle bracket state machine for ]c and [c
        let bracket_pressed = ctx.input(|input| {
            if input.key_pressed(Key::CloseBracket) {
                Some(']')
            } else if input.key_pressed(Key::OpenBracket) {
                Some('[')
            } else {
                None
            }
        });

        if let Some(bracket) = bracket_pressed {
            self.pending_bracket = Some(bracket);
            self.pending_g = false;
            return ChartAction::None;
        }

        // Handle commit navigation from pending bracket
        if action == ChartAction::NextCommit || action == ChartAction::PrevCommit {
            self.pending_bracket = None;
            self.pending_g = false;
            return action;
        }

        // Clear pending bracket if another key was pressed
        if action != ChartAction::None {
            self.pending_bracket = None;
        }

        // Handle gg (double g) state machine
        if action == ChartAction::GoToStart {
            if self.pending_g {
                self.pending_g = false;
                return ChartAction::GoToStart;
            } else {
                self.pending_g = true;
                return ChartAction::None;
            }
        } else if action == ChartAction::GoToEnd {
            self.pending_g = false;
            return ChartAction::GoToEnd;
        } else if action != ChartAction::None {
            self.pending_g = false;
        }

        action
    }

    /// Get a default color for series index
    /// Uses a modern, muted palette inspired by PlanetScale's sleek dashboard aesthetic
    fn series_color(&self, index: usize) -> Color32 {
        // A modern, muted palette - teals, purples, and soft accent colors
        const PALETTE: &[Color32] = &[
            Color32::from_rgb(99, 179, 237),  // Soft sky blue
            Color32::from_rgb(129, 140, 248), // Soft indigo
            Color32::from_rgb(94, 234, 212),  // Soft teal
            Color32::from_rgb(192, 132, 252), // Soft purple
            Color32::from_rgb(251, 191, 36),  // Soft amber
            Color32::from_rgb(244, 114, 182), // Soft pink
            Color32::from_rgb(52, 211, 153),  // Soft emerald
            Color32::from_rgb(248, 113, 113), // Soft coral
        ];
        PALETTE[index % PALETTE.len()]
    }

    /// Render the chart
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_color = text_color(self.theme);

        if self.series.is_empty() {
            // Empty state with icon
            ui.centered_and_justified(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(semantic_icons::empty::NO_DATA)
                            .size(semantic_icons::SIZE_ITEM)
                            .color(text_color.gamma_multiply(0.5)),
                    );
                    ui.label(
                        RichText::new("No data to display")
                            .color(text_color.gamma_multiply(0.5))
                            .italics(),
                    );
                });
            });
            return;
        }

        // Handle keyboard zoom/navigation
        let chart_action = self.handle_keyboard(ui.ctx());
        let data_time_range = self.data_time_range();

        // Pre-compute commit navigation targets (need to do this outside the plot closure
        // since we need &self which would conflict with the mutable borrow for find_*_commit)
        let commits_for_nav: Vec<f64> = self.commits.iter().map(|c| c.timestamp).collect();

        // Clone commits for rendering (to avoid borrow issues in closure)
        let commits_to_render: Vec<_> = if self.show_commits {
            self.commits.clone()
        } else {
            Vec::new()
        };

        // Commit marker color - subtle but visible
        let commit_color = match self.theme {
            AppTheme::Dark => Color32::from_rgba_unmultiplied(255, 193, 7, 180), // Amber
            AppTheme::Light => Color32::from_rgba_unmultiplied(245, 158, 11, 200), // Darker amber
        };

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
            // Apply chart action inside the plot closure where we have access to plot_ui
            match chart_action {
                ChartAction::ZoomInY => {
                    plot_ui.zoom_bounds(
                        egui::Vec2::new(1.0, ZOOM_FACTOR as f32),
                        plot_ui.plot_bounds().center(),
                    );
                }
                ChartAction::ZoomOutY => {
                    plot_ui.zoom_bounds(
                        egui::Vec2::new(1.0, 1.0 / ZOOM_FACTOR as f32),
                        plot_ui.plot_bounds().center(),
                    );
                }
                ChartAction::ZoomInX => {
                    plot_ui.zoom_bounds(
                        egui::Vec2::new(ZOOM_FACTOR as f32, 1.0),
                        plot_ui.plot_bounds().center(),
                    );
                }
                ChartAction::ZoomOutX => {
                    plot_ui.zoom_bounds(
                        egui::Vec2::new(1.0 / ZOOM_FACTOR as f32, 1.0),
                        plot_ui.plot_bounds().center(),
                    );
                }
                ChartAction::ResetZoom => {
                    plot_ui.set_auto_bounds(egui::Vec2b::new(true, true));
                }
                ChartAction::GoToStart => {
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
                ChartAction::GoToEnd => {
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
                ChartAction::NextCommit => {
                    let current_center = plot_ui.plot_bounds().center().x;
                    if let Some(&next_t) = commits_for_nav.iter().find(|&&t| t > current_center) {
                        let bounds = plot_ui.plot_bounds();
                        let width = bounds.max()[0] - bounds.min()[0];
                        let half_width = width / 2.0;
                        let new_bounds = PlotBounds::from_min_max(
                            [next_t - half_width, bounds.min()[1]],
                            [next_t + half_width, bounds.max()[1]],
                        );
                        plot_ui.set_plot_bounds(new_bounds);
                    }
                }
                ChartAction::PrevCommit => {
                    let current_center = plot_ui.plot_bounds().center().x;
                    if let Some(&prev_t) =
                        commits_for_nav.iter().rev().find(|&&t| t < current_center)
                    {
                        let bounds = plot_ui.plot_bounds();
                        let width = bounds.max()[0] - bounds.min()[0];
                        let half_width = width / 2.0;
                        let new_bounds = PlotBounds::from_min_max(
                            [prev_t - half_width, bounds.min()[1]],
                            [prev_t + half_width, bounds.max()[1]],
                        );
                        plot_ui.set_plot_bounds(new_bounds);
                    }
                }
                ChartAction::None => {}
            }

            // Draw commit markers as vertical lines
            for commit in &commits_to_render {
                // Truncate message to ~30 chars for legend readability
                let msg_preview = if commit.message.len() > 30 {
                    format!("{}...", &commit.message[..27])
                } else {
                    commit.message.clone()
                };
                let label = format!("{} {}", commit.short_hash(), msg_preview);
                let vline = VLine::new(label, commit.timestamp)
                    .color(commit_color)
                    .style(LineStyle::dashed_dense())
                    .stroke(Stroke::new(1.5, commit_color));

                plot_ui.vline(vline);
            }

            // Check for hover near commit markers and show tooltip
            if let Some(pointer_pos) = plot_ui.pointer_coordinate() {
                let bounds = plot_ui.plot_bounds();
                let view_width = bounds.max()[0] - bounds.min()[0];
                // Hover threshold: 1% of the visible time range
                let hover_threshold = view_width * 0.01;

                for commit in &commits_to_render {
                    let distance = (pointer_pos.x - commit.timestamp).abs();
                    if distance < hover_threshold {
                        // Show tooltip at pointer using the new Tooltip API
                        egui::containers::Tooltip::for_widget(plot_ui.response())
                            .at_pointer()
                            .show(|ui| {
                                ui.set_max_width(350.0);
                                ui.vertical(|ui| {
                                    // Header with git icon and hash
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(semantic_icons::git::COMMIT)
                                                .size(semantic_icons::SIZE_ITEM)
                                                .color(commit_color),
                                        );
                                        ui.label(
                                            RichText::new(commit.short_hash())
                                                .monospace()
                                                .strong()
                                                .color(commit_color),
                                        );
                                    });
                                    ui.add_space(4.0);
                                    // Commit message
                                    ui.label(&commit.message);
                                });
                            });
                        break; // Only show one tooltip at a time
                    }
                }
            }

            // Draw all series with sleek styling (gradient fill + thin lines)
            for (i, series) in self.series.iter().enumerate() {
                let color = series.color.unwrap_or_else(|| self.series_color(i));

                let points: PlotPoints<'_> = series
                    .points
                    .iter()
                    .map(|p| [p.timestamp, p.value])
                    .collect();

                // PlanetScale-style: thin line with soft gradient fill underneath
                let line = Line::new(series.label(), points)
                    .color(color)
                    .stroke(Stroke::new(1.5, color))
                    .fill(0.0) // Fill down to y=0
                    .fill_alpha(0.15); // Subtle gradient fill

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
        egui::RichText::new(format!("{} {}", semantic_icons::action::CHART, self.title))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
