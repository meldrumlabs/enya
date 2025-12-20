use std::borrow::Cow;
use std::ops::RangeInclusive;

use nohash_hasher::IntMap;
use rustc_hash::FxHashMap;

use egui::{Color32, Key, RichText, Stroke};
use egui_plot::{
    AxisHints, GridMark, Line, LineStyle, Plot, PlotBounds, PlotPoints, Polygon, VLine,
};

use crate::components::util::id_generator::next_id_usize;
use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;

// Re-export CommitMarker from common crate
pub use enya_common::CommitMarker;

/// Zoom factor for keyboard-based zoom controls
const ZOOM_FACTOR: f64 = 1.25;

/// Minimum chart height in pixels for a sleek default view
const MIN_CHART_HEIGHT: f32 = 180.0;

/// Default chart height ratio (height:width) - similar to Grafana/PlanetScale
const DEFAULT_ASPECT_RATIO: f32 = 0.35;

/// Format a Unix timestamp (in seconds) to a human-readable string.
/// Adapts format based on the time range being displayed.
/// Uses UTC time for simplicity and cross-platform compatibility.
fn format_timestamp(timestamp: f64, range_secs: f64) -> String {
    // Handle invalid timestamps
    if !timestamp.is_finite() || timestamp < 0.0 {
        return String::new();
    }

    let secs = timestamp as i64;

    // Compute time components from Unix timestamp (UTC)
    const SECS_PER_MIN: i64 = 60;
    const SECS_PER_HOUR: i64 = 3600;
    const SECS_PER_DAY: i64 = 86400;

    let days_since_epoch = secs / SECS_PER_DAY;
    let time_of_day = secs % SECS_PER_DAY;
    let hours = (time_of_day / SECS_PER_HOUR) % 24;
    let minutes = (time_of_day % SECS_PER_HOUR) / SECS_PER_MIN;
    let seconds = time_of_day % SECS_PER_MIN;

    // Calculate date from days since 1970-01-01
    let mut days = days_since_epoch;
    let mut year = 1970i64;

    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_months: [i64; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for (i, &dim) in days_in_months.iter().enumerate() {
        if days < dim {
            month = (i + 1) as u32;
            break;
        }
        days -= dim;
    }
    let day = (days + 1) as u32;

    // Format based on the time range being viewed
    if range_secs < 300.0 {
        // Less than 5 minutes: show HH:MM:SS
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else if range_secs < 86400.0 {
        // Less than 1 day: show HH:MM
        format!("{hours:02}:{minutes:02}")
    } else if range_secs < 604800.0 {
        // Less than 1 week: show Mon DD HH:MM
        let month_name = match month {
            1 => "Jan",
            2 => "Feb",
            3 => "Mar",
            4 => "Apr",
            5 => "May",
            6 => "Jun",
            7 => "Jul",
            8 => "Aug",
            9 => "Sep",
            10 => "Oct",
            11 => "Nov",
            12 => "Dec",
            _ => "???",
        };
        format!("{month_name} {day} {hours:02}:{minutes:02}")
    } else {
        // More than 1 week: show YYYY-MM-DD
        format!("{year}-{month:02}-{day:02}")
    }
}

/// Format a numeric value with K, M, B suffixes for large numbers.
/// Makes Y-axis labels more readable.
fn format_value(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }

    let abs_value = value.abs();
    if abs_value >= 1_000_000_000.0 {
        format!("{:.1}B", value / 1_000_000_000.0)
    } else if abs_value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if abs_value >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

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
    pub tags: FxHashMap<String, String>,
    /// The data points
    pub points: Vec<DataPoint>,
    /// Color for this series (optional, will be auto-assigned if None)
    pub color: Option<Color32>,
}

impl Series {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tags: FxHashMap::default(),
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

    /// Set tags from a HashMap
    pub fn with_tags_map(mut self, tags: FxHashMap<String, String>) -> Self {
        self.tags = tags;
        self
    }

    /// Build a label from the series name and tags.
    /// Returns a `Cow<str>` to avoid allocation when there are no tags.
    pub fn label(&self) -> Cow<'_, str> {
        if self.tags.is_empty() {
            Cow::Borrowed(&self.name)
        } else {
            let mut tags: Vec<_> = Vec::with_capacity(self.tags.len());
            for (k, v) in &self.tags {
                tags.push(format!("{k}={v}"));
            }
            Cow::Owned(format!("{} {{{}}}", self.name, tags.join(", ")))
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
    /// Toggle stacked mode
    ToggleStacked,
}

/// A time series chart component
pub struct TimeSeriesChart {
    /// Unique identifier for this chart
    id: usize,
    /// The metric name being displayed
    pub(crate) metric_name: String,
    /// All series to display
    series: Vec<Series>,
    /// Git commit markers to display as vertical annotations
    commits: Vec<CommitMarker>,
    /// Whether to show commit markers
    show_commits: bool,
    /// Current theme
    pub(crate) theme: AppTheme,
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
    /// Whether the legend is expanded to show all series
    legend_expanded: bool,
    /// Whether to render as a stacked area chart
    stacked: bool,
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
            id: next_id_usize(),
            title: name.clone(),
            metric_name: name,
            series: Vec::new(),
            commits: Vec::new(),
            show_commits: false,
            theme: AppTheme::default(),
            api_key: String::new(),
            show_legend: true,
            y_label: None,
            pending_g: false,
            pending_bracket: None,
            legend_expanded: false,
            stacked: false,
        }
    }

    /// Set whether to render as a stacked area chart
    pub fn set_stacked(&mut self, stacked: bool) {
        self.stacked = stacked;
    }

    /// Toggle stacked mode
    pub fn toggle_stacked(&mut self) {
        self.stacked = !self.stacked;
    }

    /// Check if the chart is in stacked mode
    pub fn is_stacked(&self) -> bool {
        self.stacked
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

            // Toggle stacked mode: s
            if input.key_pressed(Key::S) && !input.modifiers.shift {
                return ChartAction::ToggleStacked;
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
            // Branded empty state - centered with Enya logo
            ui.vertical_centered(|ui| {
                let center_offset = (ui.available_height() / 2.0 - 50.0).max(20.0);
                ui.add_space(center_offset);

                // Enya logo (slightly transparent for subtle branding)
                let logo = egui::Image::new(egui::include_image!("../../../assets/logo.png"))
                    .max_width(64.0)
                    .max_height(64.0)
                    .tint(text_color.gamma_multiply(0.7));
                ui.add(logo);

                ui.add_space(16.0);

                // Primary message
                ui.label(
                    RichText::new("No data to display")
                        .color(text_color.gamma_multiply(0.6))
                        .size(14.0),
                );
            });
            return;
        }

        // Handle keyboard zoom/navigation
        let chart_action = self.handle_keyboard(ui.ctx());
        let data_time_range = self.data_time_range();

        // Handle stacked toggle outside the plot closure (since it modifies self)
        if chart_action == ChartAction::ToggleStacked {
            self.stacked = !self.stacked;
        }

        // Pre-compute commit navigation targets (need to do this outside the plot closure
        // since we need &self which would conflict with the mutable borrow for find_*_commit)
        let commits_for_nav: Vec<f64> = self.commits.iter().map(|c| c.timestamp).collect();

        // Clone commits for rendering (to avoid borrow issues in closure)
        let commits_to_render: Vec<_> = if self.show_commits {
            self.commits.clone()
        } else {
            Vec::new()
        };

        // Commit marker color - uses brand emerald
        let commit_color = palette::chart::COMMIT_MARKER;

        // Calculate time range for adaptive formatting
        let time_range_secs = data_time_range
            .map(|(min, max)| max - min)
            .unwrap_or(3600.0); // Default to 1 hour if no data

        // Custom x-axis formatter for human-readable timestamps
        let x_axis = AxisHints::new_x().label("Time").formatter(
            move |mark: GridMark, _range: &RangeInclusive<f64>| {
                format_timestamp(mark.value, time_range_secs)
            },
        );

        // Custom y-axis formatter with K/M/B suffixes for large numbers
        let y_label = self.y_label.as_deref().unwrap_or("Value");
        let y_axis = AxisHints::new_y()
            .label(y_label)
            .formatter(|mark: GridMark, _range: &RangeInclusive<f64>| format_value(mark.value));

        // Calculate optimal height for a sleek Grafana/PlanetScale-style view
        // Use available height if constrained by layout, otherwise calculate from aspect ratio
        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let aspect_height = available_width * DEFAULT_ASPECT_RATIO;
        // Use the smaller of available height or aspect-based height, but respect minimum
        let optimal_height = available_height.min(aspect_height).max(MIN_CHART_HEIGHT);

        // Center the plot vertically if there's extra space
        let vertical_padding = (available_height - optimal_height).max(0.0) / 2.0;
        if vertical_padding > 1.0 {
            ui.add_space(vertical_padding);
        }

        // Apply softer grid lines by overriding the style
        let grid_color = palette::border_subtle(self.theme).gamma_multiply(0.4);
        ui.style_mut().visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, grid_color);

        // The plot - let egui_plot manage bounds internally via its ID-based memory
        let plot = Plot::new(format!("plot_{}", self.id))
            .min_size(egui::vec2(100.0, MIN_CHART_HEIGHT))
            .height(optimal_height)
            .legend(egui_plot::Legend::default().position(egui_plot::Corner::RightTop))
            .custom_x_axes(vec![x_axis])
            .custom_y_axes(vec![y_axis])
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
                ChartAction::None | ChartAction::ToggleStacked => {}
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

            // Draw all series
            if self.stacked && self.series.len() > 1 {
                // Stacked area chart using polygons for proper fill-between effect
                // Each area fills from its cumulative baseline to the previous series

                // Build a lookup for each series: timestamp -> value
                // Use IntMap for O(1) lookup with no hashing overhead for i64 keys
                let series_values: Vec<IntMap<i64, f64>> = self
                    .series
                    .iter()
                    .map(|s| {
                        s.points
                            .iter()
                            .map(|p| ((p.timestamp * 1000.0) as i64, p.value))
                            .collect()
                    })
                    .collect();

                // Compute cumulative values for each series at each timestamp
                // cumulative[i] contains (timestamp, cumulative_value) pairs
                let mut cumulative: Vec<Vec<(f64, f64)>> = Vec::with_capacity(self.series.len());

                for (i, series) in self.series.iter().enumerate() {
                    let mut points_with_cumulative: Vec<(f64, f64)> =
                        Vec::with_capacity(series.points.len());

                    for point in &series.points {
                        let ts_key = (point.timestamp * 1000.0) as i64;
                        // Sum all previous series values at this timestamp
                        let baseline: f64 = (0..i)
                            .map(|j| series_values[j].get(&ts_key).copied().unwrap_or(0.0))
                            .sum();
                        points_with_cumulative.push((point.timestamp, baseline + point.value));
                    }

                    cumulative.push(points_with_cumulative);
                }

                // Draw areas as polygons (bottom to top so later series appear on top)
                for (i, series) in self.series.iter().enumerate() {
                    let color = series.color.unwrap_or_else(|| self.series_color(i));

                    // Build polygon points: top edge (current cumulative) + bottom edge (previous cumulative, reversed)
                    let top_points = &cumulative[i];

                    if !top_points.is_empty() {
                        // Estimate capacity: top points + bottom points (either 2 for y=0 or prev series len)
                        let bottom_len = if i == 0 { 2 } else { cumulative[i - 1].len() };
                        let mut polygon_points: Vec<[f64; 2]> =
                            Vec::with_capacity(top_points.len() + bottom_len);

                        // Top edge: current cumulative line (left to right)
                        for &(t, v) in top_points {
                            polygon_points.push([t, v]);
                        }

                        // Bottom edge: previous cumulative line reversed (right to left)
                        // For first series, bottom is y=0
                        if i == 0 {
                            // Close polygon along y=0
                            if let (Some(&(t_last, _)), Some(&(t_first, _))) =
                                (top_points.last(), top_points.first())
                            {
                                polygon_points.push([t_last, 0.0]);
                                polygon_points.push([t_first, 0.0]);
                            }
                        } else {
                            // Close along previous series line (reversed)
                            for &(t, v) in cumulative[i - 1].iter().rev() {
                                polygon_points.push([t, v]);
                            }
                        }

                        // Draw filled polygon
                        let fill_color = Color32::from_rgba_unmultiplied(
                            color.r(),
                            color.g(),
                            color.b(),
                            (0.6 * 255.0) as u8, // 60% opacity for stacked areas
                        );
                        let polygon =
                            Polygon::new(series.label(), PlotPoints::from(polygon_points))
                                .fill_color(fill_color)
                                .stroke(Stroke::new(1.0, color));
                        plot_ui.polygon(polygon);
                    }
                }

                // Draw lines on top for clarity
                for (i, series) in self.series.iter().enumerate() {
                    let color = series.color.unwrap_or_else(|| self.series_color(i));
                    let points: PlotPoints<'_> =
                        cumulative[i].iter().map(|&(t, v)| [t, v]).collect();
                    let line = Line::new(series.label(), points)
                        .color(color)
                        .stroke(Stroke::new(1.5, color));
                    plot_ui.line(line);
                }
            } else {
                // Regular (non-stacked) view: each series fills to y=0
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
            }
        });

        // Legend below chart (if enabled and multiple series)
        if self.show_legend && self.series.len() > 1 {
            ui.add_space(8.0);

            const MAX_VISIBLE_SERIES: usize = 10;
            let total_series = self.series.len();
            let show_all = self.legend_expanded || total_series <= MAX_VISIBLE_SERIES;
            let visible_count = if show_all {
                total_series
            } else {
                MAX_VISIBLE_SERIES
            };

            ui.horizontal_wrapped(|ui| {
                for (i, series) in self.series.iter().take(visible_count).enumerate() {
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

                // Show "N more..." button if there are hidden series
                if total_series > MAX_VISIBLE_SERIES {
                    let hidden_count = total_series - visible_count;
                    let button_text = if self.legend_expanded {
                        "show less".to_string()
                    } else {
                        format!("+{hidden_count} more...")
                    };

                    if ui
                        .small_button(
                            RichText::new(button_text).color(text_color.gamma_multiply(0.6)),
                        )
                        .clicked()
                    {
                        self.legend_expanded = !self.legend_expanded;
                    }
                }
            });
        }
    }
}

/// Implement Component trait so TimeSeriesChart can be used in the dashboard
impl crate::components::Component for TimeSeriesChart {
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
