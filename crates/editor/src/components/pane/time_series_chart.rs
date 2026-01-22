use std::borrow::Cow;
use std::ops::RangeInclusive;

use nohash_hasher::IntMap;
use rustc_hash::FxHashMap;

use egui::{Color32, Key, Rangef, RichText, Stroke};
use egui_plot::{
    AxisHints, GridMark, Line, LineStyle, Plot, PlotBounds, PlotPoints, Polygon, VLine,
};

use super::annotation::{Annotation, AnnotationId, AnnotationTarget};
use crate::components::util::id_generator::next_id_usize;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::tinted_logo::get_tinted_logo_with_opacity;

/// A marker representing a git commit at a specific point in time.
///
/// Used to annotate time-series charts with commit information,
/// allowing correlation between metric changes and code changes.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitMarker {
    /// Git commit hash (full or abbreviated)
    pub hash: String,
    /// Timestamp of the commit in seconds (Unix epoch)
    pub timestamp: f64,
    /// Commit message (first line / subject)
    pub message: String,
}

impl CommitMarker {
    pub fn new(hash: impl Into<String>, timestamp: f64, message: impl Into<String>) -> Self {
        Self {
            hash: hash.into(),
            timestamp,
            message: message.into(),
        }
    }

    /// Get abbreviated hash (first 7 characters)
    #[must_use]
    pub fn short_hash(&self) -> &str {
        if self.hash.len() > 7 {
            &self.hash[..7]
        } else {
            &self.hash
        }
    }
}

/// Zoom factor for keyboard-based zoom controls
const ZOOM_FACTOR: f64 = 1.25;

/// Minimum chart height in pixels for a sleek default view
const MIN_CHART_HEIGHT: f32 = 180.0;

/// Default chart height ratio (height:width)
const DEFAULT_ASPECT_RATIO: f32 = 0.35;

/// Format a Unix timestamp (in seconds) to a human-readable string.
/// Adapts format based on the time range being displayed.
/// Uses UTC time for simplicity and cross-platform compatibility.
#[profiling::function]
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

/// Format a numeric value with K, M, B suffixes and an optional unit suffix.
/// Used for Y-axis labels and legend values.
#[profiling::function]
pub fn format_value_with_unit(value: f64, unit: &str) -> String {
    if !value.is_finite() {
        return String::new();
    }

    let abs_value = value.abs();
    let formatted = if abs_value >= 1_000_000_000.0 {
        format!("{:.1}B", value / 1_000_000_000.0)
    } else if abs_value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if abs_value >= 1_000.0 {
        format!("{:.1}K", value / 1_000.0)
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    };

    if unit.is_empty() {
        formatted
    } else {
        format!("{formatted} {unit}")
    }
}

/// A single data point in the time series
#[derive(Debug, Clone, PartialEq)]
pub struct DataPoint {
    /// Timestamp in seconds (Unix epoch or relative)
    pub timestamp: f64,
    /// The metric value
    pub value: f64,
}

/// A single series of data points
#[derive(Debug, Clone, PartialEq)]
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

    /// Build a short label for the legend (prefers tag values, truncates long names)
    pub fn short_label(&self) -> Cow<'_, str> {
        // If we have tags, just show the tag values (e.g., "GET", "POST" for method=GET)
        if !self.tags.is_empty() {
            let values: Vec<_> = self.tags.values().cloned().collect();
            return Cow::Owned(values.join(", "));
        }

        // Otherwise truncate the name
        let name = &self.name;
        if name.len() <= 20 {
            Cow::Borrowed(name)
        } else {
            // Try to find a meaningful short form
            // If it looks like a PromQL query, extract the metric name
            if let Some(paren_idx) = name.find('(') {
                let after_paren = &name[paren_idx + 1..];
                if let Some(end) = after_paren.find(|c: char| !c.is_alphanumeric() && c != '_') {
                    let metric = &after_paren[..end];
                    if !metric.is_empty() {
                        return Cow::Owned(metric.to_string());
                    }
                }
            }
            // Fallback: just truncate
            Cow::Owned(format!("{}…", &name[..17]))
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
    /// Toggle commit markers visibility (gc)
    ToggleCommits,
    /// Toggle annotations visibility (gn)
    ToggleAnnotations,
    /// Navigate to next annotation (]a)
    NextAnnotation,
    /// Navigate to previous annotation ([a)
    PrevAnnotation,
}

/// Actions returned from chart interaction for the workspace to handle.
#[derive(Debug, Clone)]
pub enum ChartInteraction {
    /// No interaction.
    None,
    /// User clicked to add an annotation at this timestamp.
    AddAnnotation { timestamp: f64 },
    /// User wants to edit an existing annotation.
    EditAnnotation { id: AnnotationId },
    /// User wants to delete an annotation.
    DeleteAnnotation { id: AnnotationId },
    /// User wants to resolve an annotation.
    ResolveAnnotation { id: AnnotationId },
    /// User double-clicked on the chart for logs drilldown.
    /// Opens a logs pane centered around this timestamp.
    DrilldownLogs {
        /// The timestamp in seconds (Unix epoch) where the user clicked
        timestamp_secs: f64,
        /// The metric name for context
        metric_name: String,
    },
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
    /// Team annotations (comments pinned to chart points/ranges)
    annotations: Vec<Annotation>,
    /// Whether to show annotations
    show_annotations: bool,
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
    /// Unit suffix for values (e.g., "ms", "req/s", "%")
    unit: String,
    /// Whether we're waiting for a second 'g' press (for gg command)
    pending_g: bool,
    /// Whether we're waiting for 'c' or 'a' after '[' or ']' (for navigation)
    pending_bracket: Option<char>,
    /// Whether to render as a stacked area chart
    stacked: bool,
    /// Whether annotation mode is active (click to add)
    annotation_mode: bool,
    /// Compact mode for inline display (no background, no interaction)
    compact: bool,
    /// Pending interaction to be consumed by the parent (set on double-click, cleared on take)
    pending_interaction: Option<ChartInteraction>,
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
            annotations: Vec::new(),
            show_annotations: true,
            theme: AppTheme::default(),
            api_key: String::new(),
            show_legend: true,
            y_label: None,
            unit: String::new(),
            pending_g: false,
            pending_bracket: None,
            stacked: false,
            annotation_mode: false,
            compact: false,
            pending_interaction: None,
        }
    }

    /// Enable compact mode for inline display (no background, no interaction).
    pub fn set_compact(&mut self, compact: bool) {
        self.compact = compact;
    }

    /// Set the unit suffix for values (e.g., "ms", "req/s", "%")
    pub fn set_unit(&mut self, unit: impl Into<String>) {
        self.unit = unit.into();
    }

    /// Get the unit suffix
    pub fn unit(&self) -> &str {
        &self.unit
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
                .with_color(chart.theme.chart_color(0)), // Sky blue
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
                .with_color(chart.theme.chart_color(2)), // Teal
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

        // Enable commit visibility for demo
        chart.show_commits = true;

        // Add demo annotations
        use super::annotation::{AnnotationAuthor, AnnotationPriority};

        chart.add_annotation(
            Annotation::at_point(
                now + duration * 0.15,
                "Latency spike after deploy - investigating",
            )
            .with_author(AnnotationAuthor::local("Alice Chen"))
            .with_priority(AnnotationPriority::Important),
        );
        chart.add_annotation(
            Annotation::at_range(
                now + duration * 0.4,
                now + duration * 0.45,
                "Planned maintenance window",
            )
            .with_author(AnnotationAuthor::local("Bob Smith")),
        );
        chart.add_annotation(
            Annotation::at_point(
                now + duration * 0.6,
                "Root cause: connection pool exhaustion",
            )
            .with_author(AnnotationAuthor::local("Alice Chen"))
            .with_priority(AnnotationPriority::Critical),
        );
        let mut resolved_ann = Annotation::at_point(now + duration * 0.8, "Fixed in v2.3.1")
            .with_author(AnnotationAuthor::local("Carol Davis"));
        resolved_ann.resolve();
        chart.add_annotation(resolved_ann);

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
        self.show_commits = !commits.is_empty();
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

    // ==================== Annotation Methods ====================

    /// Add an annotation to the chart.
    pub fn add_annotation(&mut self, annotation: Annotation) {
        self.annotations.push(annotation);
        // Keep annotations sorted by timestamp for navigation
        self.annotations.sort_by(|a, b| {
            a.timestamp()
                .partial_cmp(&b.timestamp())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Set all annotations at once.
    pub fn set_annotations(&mut self, annotations: Vec<Annotation>) {
        self.annotations = annotations;
        self.annotations.sort_by(|a, b| {
            a.timestamp()
                .partial_cmp(&b.timestamp())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Get all annotations.
    pub fn annotations(&self) -> &[Annotation] {
        &self.annotations
    }

    /// Get a mutable reference to annotations.
    pub fn annotations_mut(&mut self) -> &mut Vec<Annotation> {
        &mut self.annotations
    }

    /// Find an annotation by ID.
    pub fn find_annotation(&self, id: AnnotationId) -> Option<&Annotation> {
        self.annotations.iter().find(|a| a.id == id)
    }

    /// Find a mutable annotation by ID.
    pub fn find_annotation_mut(&mut self, id: AnnotationId) -> Option<&mut Annotation> {
        self.annotations.iter_mut().find(|a| a.id == id)
    }

    /// Remove an annotation by ID.
    pub fn remove_annotation(&mut self, id: AnnotationId) -> Option<Annotation> {
        if let Some(idx) = self.annotations.iter().position(|a| a.id == id) {
            Some(self.annotations.remove(idx))
        } else {
            None
        }
    }

    /// Clear all annotations.
    pub fn clear_annotations(&mut self) {
        self.annotations.clear();
    }

    /// Set whether to show annotations.
    pub fn set_show_annotations(&mut self, show: bool) {
        self.show_annotations = show;
    }

    /// Toggle annotations visibility.
    pub fn toggle_annotations(&mut self) {
        self.show_annotations = !self.show_annotations;
    }

    /// Check if annotations are visible.
    pub fn annotations_visible(&self) -> bool {
        self.show_annotations
    }

    /// Enable annotation mode (click to add).
    pub fn enter_annotation_mode(&mut self) {
        self.annotation_mode = true;
    }

    /// Disable annotation mode.
    pub fn exit_annotation_mode(&mut self) {
        self.annotation_mode = false;
    }

    /// Check if annotation mode is active.
    pub fn is_annotation_mode(&self) -> bool {
        self.annotation_mode
    }

    /// Take the pending interaction (returns and clears it).
    /// Call this after `show()` to check if the user triggered a drilldown.
    pub fn take_interaction(&mut self) -> Option<ChartInteraction> {
        self.pending_interaction.take()
    }

    /// Get the number of unresolved annotations.
    pub fn unresolved_annotation_count(&self) -> usize {
        self.annotations.iter().filter(|a| !a.resolved).count()
    }

    /// Get all series in the chart (for creating snapshots).
    pub fn series(&self) -> &[Series] {
        &self.series
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

            // Check for 'a' after pending bracket for annotation navigation
            if self.pending_bracket.is_some() && input.key_pressed(Key::A) {
                return match self.pending_bracket {
                    Some(']') => ChartAction::NextAnnotation,
                    Some('[') => ChartAction::PrevAnnotation,
                    _ => ChartAction::None,
                };
            }

            // Check for 'c' after pending 'g' for gc (toggle commits)
            if self.pending_g && input.key_pressed(Key::C) {
                return ChartAction::ToggleCommits;
            }

            // Check for 'n' after pending 'g' for gn (toggle annotations/notes)
            if self.pending_g && input.key_pressed(Key::N) {
                return ChartAction::ToggleAnnotations;
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

        // Handle annotation navigation from pending bracket
        if action == ChartAction::NextAnnotation || action == ChartAction::PrevAnnotation {
            self.pending_bracket = None;
            self.pending_g = false;
            return action;
        }

        // Clear pending bracket if another key was pressed
        if action != ChartAction::None {
            self.pending_bracket = None;
        }

        // Handle gc (toggle commits) - must come before gg handling
        if action == ChartAction::ToggleCommits {
            self.pending_g = false;
            return ChartAction::ToggleCommits;
        }

        // Handle gn (toggle annotations/notes)
        if action == ChartAction::ToggleAnnotations {
            self.pending_g = false;
            return ChartAction::ToggleAnnotations;
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
    /// Uses the theme's chart palette for consistent colors
    fn series_color(&self, index: usize) -> Color32 {
        self.theme.chart_color(index)
    }

    /// Render the chart
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_color = self.theme.text_primary();

        // Calculate scale factor based on available space
        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let base_size = available_width.min(available_height * 1.5);
        let scale_factor = (base_size / 400.0).clamp(0.8, 1.6);

        // Scaled dimensions
        let legend_text_size = (14.0 * scale_factor).clamp(11.0, 18.0);
        let legend_dot_size = (14.0 * scale_factor).clamp(10.0, 20.0);
        let legend_dot_radius = (6.0 * scale_factor).clamp(4.0, 9.0);
        let legend_item_spacing = (24.0 * scale_factor).clamp(16.0, 36.0);
        let legend_inner_spacing = (8.0 * scale_factor).clamp(6.0, 12.0);
        let line_stroke_width = (1.5 * scale_factor).clamp(1.0, 2.5);
        let logo_size = (64.0 * scale_factor).clamp(48.0, 96.0);

        if self.series.is_empty() {
            // Branded empty state - centered with Enya logo
            ui.vertical_centered(|ui| {
                let center_offset = (ui.available_height() / 2.0 - 50.0).max(20.0);
                ui.add_space(center_offset);

                // Get the overlay-blended tinted logo (subtle for empty state)
                let texture = get_tinted_logo_with_opacity(ui.ctx(), self.theme, 0.5);
                let logo =
                    egui::Image::from_texture(egui::load::SizedTexture::from_handle(&texture))
                        .max_width(logo_size)
                        .max_height(logo_size);
                ui.add(logo);

                ui.add_space(16.0 * scale_factor);

                // Primary message
                ui.label(
                    RichText::new("No data to display")
                        .color(text_color.gamma_multiply(0.6))
                        .size(legend_text_size),
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
        if chart_action == ChartAction::ToggleCommits {
            self.toggle_commits();
        }
        if chart_action == ChartAction::ToggleAnnotations {
            self.toggle_annotations();
        }

        // Pre-compute commit navigation targets (need to do this outside the plot closure
        // since we need &self which would conflict with the mutable borrow for find_*_commit)
        let commits_for_nav: Vec<f64> = self.commits.iter().map(|c| c.timestamp).collect();

        // Pre-compute annotation navigation targets
        let annotations_for_nav: Vec<f64> =
            self.annotations.iter().map(|a| a.timestamp()).collect();

        // Clone commits for rendering (to avoid borrow issues in closure)
        let commits_to_render: Vec<_> = if self.show_commits {
            self.commits.clone()
        } else {
            Vec::new()
        };

        // Clone annotations for rendering
        let annotations_to_render: Vec<_> = if self.show_annotations {
            self.annotations.clone()
        } else {
            Vec::new()
        };

        // Commit marker color - uses theme color
        let commit_color = self.theme.chart_commit_marker();

        // Capture theme for annotation colors inside closures
        let chart_theme = self.theme;

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

        // Custom y-axis formatter with K/M/B suffixes for large numbers and unit suffix
        let unit = self.unit.clone();
        let y_axis =
            AxisHints::new_y().formatter(move |mark: GridMark, _range: &RangeInclusive<f64>| {
                format_value_with_unit(mark.value, &unit)
            });

        // Calculate optimal height for a sleek dashboard-style view
        // Use available height if constrained by layout, otherwise calculate from aspect ratio
        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let aspect_height = available_width * DEFAULT_ASPECT_RATIO;

        // When available height is constrained (split panes), use available height directly
        // to ensure the x-axis labels remain visible and don't get clipped.
        // Only use aspect ratio when we have ample vertical space.
        let use_available_height = available_height < MIN_CHART_HEIGHT * 1.5;
        let optimal_height = if use_available_height {
            // Constrained: use available height (allows plot to shrink and show axis)
            available_height
        } else {
            // Ample space: use aspect ratio for aesthetic proportions
            aspect_height.max(MIN_CHART_HEIGHT).min(available_height)
        };

        // Note: We align the chart to the top of the pane (no vertical centering).
        // This looks cleaner in vsplit layouts where panes are tall and narrow.

        // Apply very soft grid lines for premium look - barely visible structure
        let grid_color = self.theme.border_subtle().gamma_multiply(0.25);
        ui.style_mut().visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, grid_color);

        // Legend above the chart (show if any series exist)
        if self.show_legend && !self.series.is_empty() {
            const MAX_VISIBLE_SERIES: usize = 5;
            let total_series = self.series.len();
            let show_overflow = total_series > MAX_VISIBLE_SERIES;
            let visible_count = if show_overflow {
                MAX_VISIBLE_SERIES
            } else {
                total_series
            };

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = legend_item_spacing;

                // Show first N series
                for (i, series) in self.series.iter().take(visible_count).enumerate() {
                    let color = series.color.unwrap_or_else(|| self.series_color(i));
                    let latest_value = series.points.last().map(|p| p.value).unwrap_or(0.0);

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = legend_inner_spacing;

                        // Colored dot
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(legend_dot_size, legend_dot_size),
                            egui::Sense::hover(),
                        );
                        ui.painter()
                            .circle_filled(rect.center(), legend_dot_radius, color);

                        // Series name with value: "series: 1.2K unit"
                        let formatted_value = format_value_with_unit(latest_value, &self.unit);
                        ui.label(
                            RichText::new(format!("{}: {}", series.short_label(), formatted_value))
                                .color(text_color.gamma_multiply(0.9))
                                .size(legend_text_size),
                        );
                    });
                }

                // Show "+ N more" if there are overflow series (with hover tooltip)
                if show_overflow {
                    let overflow_count = total_series - visible_count;

                    // Collect overflow series data for tooltip
                    let overflow_data: Vec<_> = self
                        .series
                        .iter()
                        .enumerate()
                        .skip(visible_count)
                        .map(|(i, series)| {
                            let latest_value = series.points.last().map(|p| p.value).unwrap_or(0.0);
                            let formatted_value = format_value_with_unit(latest_value, &self.unit);
                            let color = series.color.unwrap_or_else(|| self.series_color(i));
                            let label = series.short_label().to_string();
                            (color, label, formatted_value)
                        })
                        .collect();

                    let more_text = format!("+ {overflow_count} more");

                    // Use a Label with sense for reliable hover detection
                    let response = ui.add(
                        egui::Label::new(
                            RichText::new(&more_text)
                                .color(text_color.gamma_multiply(0.5))
                                .size(legend_text_size),
                        )
                        .sense(egui::Sense::hover()),
                    );

                    // Highlight on hover by repainting with brighter color
                    if response.hovered() {
                        let rect = response.rect;
                        // Paint over with highlighted text
                        ui.painter().rect_filled(rect, 0.0, self.theme.bg_surface());
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &more_text,
                            egui::FontId::proportional(legend_text_size),
                            text_color.gamma_multiply(0.9),
                        );
                    }

                    // Tooltip dot sizes (slightly smaller than legend)
                    let tooltip_dot_size = legend_dot_size * 0.85;
                    let tooltip_dot_radius = legend_dot_radius * 0.85;

                    response.on_hover_ui(|ui| {
                        for (color, label, value) in &overflow_data {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = legend_inner_spacing;
                                // Colored dot
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(tooltip_dot_size, tooltip_dot_size),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(
                                    rect.center(),
                                    tooltip_dot_radius,
                                    *color,
                                );
                                // Label and value
                                ui.label(format!("{label}: {value}"));
                            });
                        }
                    });
                }
            });
            ui.add_space(8.0);
        }

        // The plot - let egui_plot manage bounds internally via its ID-based memory
        // Note: We use our own custom legend above the chart, so no egui_plot legend
        // Use the ACTUAL remaining height after legend, but cap it for good aspect ratio
        let remaining_height = ui.available_height();
        let remaining_width = ui.available_width();

        // Calculate a reasonable plot height:
        // - When height is constrained (stacked panes): use all remaining height
        // - When pane is tall & narrow (vsplit): use aspect ratio to avoid stretching
        // - Otherwise: use the pre-calculated optimal height
        //
        // Detect vsplit by checking if pane is portrait-oriented (height > width * 1.2)
        let is_portrait = remaining_height > remaining_width * 1.2;
        let plot_height = if remaining_height < MIN_CHART_HEIGHT * 1.5 {
            // Height constrained (horizontal split) - use all remaining height
            remaining_height
        } else if is_portrait {
            // Portrait pane (vsplit) - use standard aspect ratio
            // and cap to 20% of pane height for compact dashboard appearance
            let aspect_height = remaining_width * DEFAULT_ASPECT_RATIO;
            let max_height = remaining_height * 0.20; // Cap to 20% of pane
            aspect_height.max(MIN_CHART_HEIGHT).min(max_height)
        } else {
            // Ample space - use optimal height but don't exceed remaining
            optimal_height.min(remaining_height)
        };

        let mut plot = Plot::new(format!("plot_{}", self.id))
            .min_size(egui::vec2(100.0, 80.0)) // Reduced min height to allow smaller panes
            .height(plot_height)
            .show_axes([true, true])
            .custom_x_axes(vec![x_axis])
            .custom_y_axes(vec![y_axis])
            .show_grid(true)
            .allow_zoom(!self.compact)
            .allow_drag(!self.compact)
            .allow_scroll(!self.compact);

        // In compact mode, create a cleaner display
        if self.compact {
            // Use simpler axes with fixed minimum thickness for alignment
            let compact_y_axis = AxisHints::new_y()
                .min_thickness(45.0) // Fixed width for consistent alignment
                .formatter(move |mark: GridMark, _range: &RangeInclusive<f64>| {
                    // Simpler formatting for compact mode
                    let value = mark.value;
                    if !value.is_finite() {
                        return String::new();
                    }
                    let abs_value = value.abs();
                    if abs_value >= 1_000_000.0 {
                        format!("{:.0}M", value / 1_000_000.0)
                    } else if abs_value >= 1_000.0 {
                        format!("{:.0}K", value / 1_000.0)
                    } else if value.fract() == 0.0 {
                        format!("{value:.0}")
                    } else {
                        format!("{value:.1}")
                    }
                });

            let compact_x_axis = AxisHints::new_x()
                .label_spacing(Rangef::new(80.0, 120.0)) // More spacing to avoid edge overlap
                .formatter(move |mark: GridMark, _range: &RangeInclusive<f64>| {
                    format_timestamp(mark.value, time_range_secs)
                });

            plot = plot
                .custom_x_axes(vec![compact_x_axis])
                .custom_y_axes(vec![compact_y_axis])
                .show_background(false)
                .allow_boxed_zoom(false)
                .clamp_grid(true) // Clamp labels to data range
                .show_x(false)
                .show_y(false)
                .allow_double_click_reset(false)
                .cursor_color(Color32::TRANSPARENT) // Hide hover cursor lines
                .sense(egui::Sense::empty()); // No interaction at all
        }

        let plot_response = plot.show(ui, |plot_ui| {
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
                ChartAction::NextAnnotation => {
                    let current_center = plot_ui.plot_bounds().center().x;
                    if let Some(&next_t) = annotations_for_nav.iter().find(|&&t| t > current_center)
                    {
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
                ChartAction::PrevAnnotation => {
                    let current_center = plot_ui.plot_bounds().center().x;
                    if let Some(&prev_t) = annotations_for_nav
                        .iter()
                        .rev()
                        .find(|&&t| t < current_center)
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
                ChartAction::None
                | ChartAction::ToggleStacked
                | ChartAction::ToggleCommits
                | ChartAction::ToggleAnnotations => {}
            }

            // Draw commit markers as vertical dashed lines
            for (i, commit) in commits_to_render.iter().enumerate() {
                let vline = VLine::new(format!("commit_{i}"), commit.timestamp)
                    .color(commit_color)
                    .style(LineStyle::dashed_dense())
                    .stroke(Stroke::new(2.5, commit_color));
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

            // Draw annotation markers as vertical lines (like commit markers)
            // Note: We use VLines instead of Polygons to avoid affecting plot auto-bounds
            for annotation in &annotations_to_render {
                let ann_color = annotation.color_for_theme(chart_theme);
                match &annotation.target {
                    AnnotationTarget::Point { timestamp } => {
                        // Draw as vertical line
                        let vline = VLine::new(format!("ann_{}", annotation.id.0), *timestamp)
                            .color(ann_color)
                            .style(LineStyle::Solid)
                            .stroke(Stroke::new(2.0, ann_color));
                        plot_ui.vline(vline);
                    }
                    AnnotationTarget::Range { start, end } => {
                        // Draw as two vertical lines for start and end
                        // (Polygons cause plot bounds instability)
                        let start_line =
                            VLine::new(format!("ann_range_start_{}", annotation.id.0), *start)
                                .color(ann_color)
                                .style(LineStyle::Solid)
                                .stroke(Stroke::new(2.0, ann_color));
                        plot_ui.vline(start_line);

                        let end_line =
                            VLine::new(format!("ann_range_end_{}", annotation.id.0), *end)
                                .color(ann_color)
                                .style(LineStyle::dashed_dense())
                                .stroke(Stroke::new(2.0, ann_color));
                        plot_ui.vline(end_line);
                    }
                    AnnotationTarget::DataPoint { timestamp, .. } => {
                        // Draw as vertical line at the timestamp
                        // (Diamond polygon causes plot bounds instability)
                        let vline =
                            VLine::new(format!("ann_point_{}", annotation.id.0), *timestamp)
                                .color(ann_color)
                                .style(LineStyle::Solid)
                                .stroke(Stroke::new(2.5, ann_color));
                        plot_ui.vline(vline);
                    }
                }
            }

            // Check for hover near annotations and show tooltip
            if let Some(pointer_pos) = plot_ui.pointer_coordinate() {
                let bounds = plot_ui.plot_bounds();
                let view_width = bounds.max()[0] - bounds.min()[0];
                let hover_threshold = view_width * 0.015; // 1.5% of visible time range

                for annotation in &annotations_to_render {
                    let is_near = annotation
                        .target
                        .contains_timestamp(pointer_pos.x, hover_threshold);
                    if is_near {
                        let ann_color = annotation.color_for_theme(chart_theme);
                        egui::containers::Tooltip::for_widget(plot_ui.response())
                            .at_pointer()
                            .show(|ui| {
                                ui.set_max_width(300.0);
                                ui.vertical(|ui| {
                                    // Header with priority icon and author
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(annotation.priority.icon())
                                                .size(semantic_icons::SIZE_ITEM)
                                                .color(ann_color),
                                        );
                                        ui.label(
                                            RichText::new(&annotation.author.display_name)
                                                .strong()
                                                .color(ann_color),
                                        );
                                        if annotation.resolved {
                                            ui.label(
                                                RichText::new("(resolved)")
                                                    .italics()
                                                    .color(Color32::GRAY),
                                            );
                                        }
                                    });
                                    ui.add_space(4.0);
                                    // Message
                                    ui.label(&annotation.message);
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
                        .stroke(Stroke::new(line_stroke_width, color));
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

                    // Thin line with soft gradient fill underneath
                    let line = Line::new(series.label(), points)
                        .color(color)
                        .stroke(Stroke::new(line_stroke_width, color))
                        .fill(0.0) // Fill down to y=0
                        .fill_alpha(0.15); // Subtle gradient fill

                    plot_ui.line(line);
                }
            }
        });

        // Detect double-click for logs drilldown (only when not in compact or annotation mode)
        if !self.compact && !self.annotation_mode && plot_response.response.double_clicked() {
            // Get the pointer position and convert to plot coordinates
            if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                // Convert screen position to plot coordinates
                let plot_point = plot_response.transform.value_from_position(pointer_pos);
                let timestamp_secs = plot_point.x;

                self.pending_interaction = Some(ChartInteraction::DrilldownLogs {
                    timestamp_secs,
                    metric_name: self.metric_name.clone(),
                });

                log::debug!(
                    "Chart drilldown triggered at timestamp {} for metric '{}'",
                    timestamp_secs,
                    self.metric_name
                );
            }
        }

        // Render commit labels below the plot, positioned at their timestamp's X coordinate
        if self.show_commits && !commits_to_render.is_empty() {
            let transform = plot_response.transform;
            let plot_rect = plot_response.response.rect;

            // Allocate space for the labels row and get its rect
            ui.add_space(4.0);
            let (label_row_rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 16.0), egui::Sense::hover());

            let painter = ui.painter();

            // Track the right edge of the last rendered label to avoid overlaps
            let mut last_label_right: f32 = f32::NEG_INFINITY;
            const MIN_LABEL_SPACING: f32 = 100.0;

            for commit in &commits_to_render {
                // Convert timestamp to screen X coordinate
                let screen_x = transform.position_from_point_x(commit.timestamp);

                // Only render if within the plot's horizontal bounds
                if screen_x >= plot_rect.left() && screen_x <= plot_rect.right() {
                    // Skip if this label would overlap with the previous one
                    if screen_x < last_label_right + MIN_LABEL_SPACING {
                        continue;
                    }

                    // Truncate message
                    let msg = if commit.message.len() > 18 {
                        format!("{}…", &commit.message[..17])
                    } else {
                        commit.message.clone()
                    };

                    // Position centered under the commit's line
                    let label_pos = egui::pos2(screen_x, label_row_rect.top());

                    // Draw git icon + message, centered under the line
                    let label_text = format!("{} {}", semantic_icons::git::COMMIT, msg);

                    // Update the right edge for collision detection
                    // Estimate label width as ~8px per character for proportional 12pt font
                    let estimated_width = label_text.len() as f32 * 8.0;
                    last_label_right = screen_x + estimated_width / 2.0;

                    painter.text(
                        label_pos,
                        egui::Align2::CENTER_TOP,
                        label_text,
                        egui::FontId::proportional(12.0),
                        commit_color,
                    );
                }
            }
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
