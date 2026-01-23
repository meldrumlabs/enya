//! Core types for the plugin system.
//!
//! These types are designed to be independent of any specific editor implementation,
//! allowing the plugin system to be used in different contexts.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use std::collections::BTreeMap;

use rustc_hash::FxHashMap;

/// Notification level for user-facing messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

impl NotificationLevel {
    /// Parse a notification level from a string.
    pub fn parse(s: &str) -> Self {
        match s {
            "error" => Self::Error,
            "warn" | "warning" => Self::Warning,
            _ => Self::Info,
        }
    }
}

/// Log level for plugin logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Parse a log level from a string (case-insensitive).
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "debug" => Self::Debug,
            "warn" | "warning" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }
}

/// Application theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    /// Custom theme identified by name
    Custom,
}

/// A boxed future for async operations.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// HTTP response returned from http_get/http_post.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code (e.g., 200, 404, 500)
    pub status: u16,
    /// Response body as a string
    pub body: String,
    /// Response headers
    pub headers: FxHashMap<String, String>,
}

/// HTTP request error.
#[derive(Debug, Clone)]
pub struct HttpError {
    /// Error message
    pub message: String,
}

// ==================== Custom Pane Types ====================

/// Column configuration for a custom table pane.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TableColumnConfig {
    /// Column header name
    pub name: String,
    /// Key to look up in row data (optional, defaults to name)
    pub key: Option<String>,
    /// Optional fixed width in pixels (as integer for Hash/Eq)
    pub width: Option<u32>,
}

impl TableColumnConfig {
    /// Create a new column configuration.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            key: None,
            width: None,
        }
    }

    /// Set the data key for this column.
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Set the width for this column (in pixels).
    pub fn with_width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    /// Get the width as f32 for rendering.
    pub fn width_f32(&self) -> Option<f32> {
        self.width.map(|w| w as f32)
    }

    /// Get the key to use for looking up data (falls back to name).
    pub fn data_key(&self) -> &str {
        self.key.as_deref().unwrap_or(&self.name)
    }
}

/// Configuration for a custom table pane type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomTableConfig {
    /// Unique identifier for this pane type
    pub name: String,
    /// Display title for the pane
    pub title: String,
    /// Column definitions
    pub columns: Vec<TableColumnConfig>,
    /// Auto-refresh interval in seconds (0 = manual only)
    pub refresh_interval: u32,
    /// Plugin that registered this pane type
    pub plugin_name: String,
}

/// A single row of data for a custom table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct CustomTableRow {
    /// Cell values keyed by column key (BTreeMap for deterministic ordering and Hash)
    pub cells: BTreeMap<String, String>,
}

impl CustomTableRow {
    /// Create a new empty row.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a cell value.
    pub fn with_cell(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.cells.insert(key.into(), value.into());
        self
    }

    /// Get a cell value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.cells.get(key).map(|s| s.as_str())
    }
}

/// Data returned by a custom table pane's fetch function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomTableData {
    /// Rows of data
    pub rows: Vec<CustomTableRow>,
    /// Error message if fetch failed
    pub error: Option<String>,
}

impl CustomTableData {
    /// Create successful table data with rows.
    pub fn with_rows(rows: Vec<CustomTableRow>) -> Self {
        Self { rows, error: None }
    }

    /// Create error result.
    pub fn with_error(message: impl Into<String>) -> Self {
        Self {
            rows: Vec::new(),
            error: Some(message.into()),
        }
    }
}

// ==================== Custom Chart Pane Types ====================

/// A single data point in a chart series.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartDataPoint {
    /// Unix timestamp in seconds
    pub timestamp: f64,
    /// Value at this timestamp
    pub value: f64,
}

impl ChartDataPoint {
    /// Create a new data point.
    pub fn new(timestamp: f64, value: f64) -> Self {
        Self { timestamp, value }
    }
}

/// A single series in a chart (line).
#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries {
    /// Display name for the series
    pub name: String,
    /// Tags/labels for the series (BTreeMap for deterministic ordering)
    pub tags: BTreeMap<String, String>,
    /// Data points in the series
    pub points: Vec<ChartDataPoint>,
}

impl ChartSeries {
    /// Create a new series with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tags: BTreeMap::new(),
            points: Vec::new(),
        }
    }

    /// Add a tag/label to the series.
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Add a data point to the series.
    pub fn with_point(mut self, timestamp: f64, value: f64) -> Self {
        self.points.push(ChartDataPoint::new(timestamp, value));
        self
    }

    /// Add multiple data points to the series.
    pub fn with_points(mut self, points: Vec<ChartDataPoint>) -> Self {
        self.points.extend(points);
        self
    }
}

/// Configuration for a custom chart pane type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomChartConfig {
    /// Unique identifier for this pane type
    pub name: String,
    /// Display title for the pane
    pub title: String,
    /// Unit label for the Y-axis (e.g., "ms", "bytes", "%")
    pub y_unit: Option<String>,
    /// Auto-refresh interval in seconds (0 = manual only)
    pub refresh_interval: u32,
    /// Plugin that registered this pane type
    pub plugin_name: String,
}

/// Data to display in a custom chart pane.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomChartData {
    /// Series to display
    pub series: Vec<ChartSeries>,
    /// Error message if fetch failed
    pub error: Option<String>,
}

impl CustomChartData {
    /// Create chart data with series.
    pub fn with_series(series: Vec<ChartSeries>) -> Self {
        Self {
            series,
            error: None,
        }
    }

    /// Create error result.
    pub fn with_error(message: impl Into<String>) -> Self {
        Self {
            series: Vec::new(),
            error: Some(message.into()),
        }
    }
}

// ==================== Custom Stat Pane Types ====================

/// Threshold configuration for stat/gauge visualizations.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdConfig {
    /// Value at which this threshold applies
    pub value: f64,
    /// Color name: "green", "yellow", "red", "blue", or hex "#RRGGBB"
    pub color: String,
    /// Optional label for the threshold
    pub label: Option<String>,
}

impl ThresholdConfig {
    /// Create a new threshold.
    pub fn new(value: f64, color: impl Into<String>) -> Self {
        Self {
            value,
            color: color.into(),
            label: None,
        }
    }

    /// Set a label for this threshold.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Configuration for a custom stat pane type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatPaneConfig {
    /// Unique identifier for this pane type
    pub name: String,
    /// Display title for the pane
    pub title: String,
    /// Unit label (e.g., "jobs", "ms", "%")
    pub unit: Option<String>,
    /// Auto-refresh interval in seconds (0 = manual only)
    pub refresh_interval: u32,
    /// Plugin that registered this pane type
    pub plugin_name: String,
}

/// Data to display in a stat pane.
#[derive(Debug, Clone, PartialEq)]
pub struct StatPaneData {
    /// Current value to display
    pub value: f64,
    /// Sparkline data (recent history)
    pub sparkline: Vec<f64>,
    /// Change from previous period (percentage)
    pub change_value: Option<f64>,
    /// Description of change period (e.g., "vs last hour")
    pub change_period: Option<String>,
    /// Thresholds for coloring the value
    pub thresholds: Vec<ThresholdConfig>,
    /// Error message if fetch failed
    pub error: Option<String>,
}

impl Default for StatPaneData {
    fn default() -> Self {
        Self {
            value: 0.0,
            sparkline: Vec::new(),
            change_value: None,
            change_period: None,
            thresholds: Vec::new(),
            error: None,
        }
    }
}

impl StatPaneData {
    /// Create stat data with a value.
    pub fn with_value(value: f64) -> Self {
        Self {
            value,
            ..Default::default()
        }
    }

    /// Create error result.
    pub fn with_error(message: impl Into<String>) -> Self {
        Self {
            error: Some(message.into()),
            ..Default::default()
        }
    }

    /// Set sparkline data.
    pub fn sparkline(mut self, data: Vec<f64>) -> Self {
        self.sparkline = data;
        self
    }

    /// Set change indicator.
    pub fn change(mut self, value: f64, period: impl Into<String>) -> Self {
        self.change_value = Some(value);
        self.change_period = Some(period.into());
        self
    }

    /// Add a threshold.
    pub fn threshold(mut self, threshold: ThresholdConfig) -> Self {
        self.thresholds.push(threshold);
        self
    }
}

// ==================== Custom Gauge Pane Types ====================

/// Configuration for a custom gauge pane type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GaugePaneConfig {
    /// Unique identifier for this pane type
    pub name: String,
    /// Display title for the pane
    pub title: String,
    /// Unit label (e.g., "%", "MB", "req/s")
    pub unit: Option<String>,
    /// Minimum value of the gauge range (stored as scaled i64 for Hash)
    pub min_scaled: i64,
    /// Maximum value of the gauge range (stored as scaled i64 for Hash)
    pub max_scaled: i64,
    /// Auto-refresh interval in seconds (0 = manual only)
    pub refresh_interval: u32,
    /// Plugin that registered this pane type
    pub plugin_name: String,
}

/// Scale factor for converting f64 to i64 for hashable storage.
const GAUGE_SCALE_FACTOR: f64 = 1_000_000.0;

impl GaugePaneConfig {
    /// Get minimum value as f64.
    pub fn min(&self) -> f64 {
        self.min_scaled as f64 / GAUGE_SCALE_FACTOR
    }

    /// Get maximum value as f64.
    pub fn max(&self) -> f64 {
        self.max_scaled as f64 / GAUGE_SCALE_FACTOR
    }

    /// Set range from f64 values.
    /// Values are clamped to representable range to avoid overflow.
    pub fn set_range(&mut self, min: f64, max: f64) {
        self.min_scaled = scale_f64_to_i64(min);
        self.max_scaled = scale_f64_to_i64(max);
    }
}

/// Safely scale an f64 value to i64, clamping to representable range.
/// Handles NaN, infinity, and values that would overflow after scaling.
fn scale_f64_to_i64(value: f64) -> i64 {
    if value.is_nan() {
        return 0;
    }
    let scaled = value * GAUGE_SCALE_FACTOR;
    if scaled >= i64::MAX as f64 {
        i64::MAX
    } else if scaled <= i64::MIN as f64 {
        i64::MIN
    } else {
        scaled as i64
    }
}

/// Data to display in a gauge pane.
#[derive(Debug, Clone, PartialEq)]
pub struct GaugePaneData {
    /// Current value
    pub value: f64,
    /// Thresholds for coloring
    pub thresholds: Vec<ThresholdConfig>,
    /// Error message if fetch failed
    pub error: Option<String>,
}

impl Default for GaugePaneData {
    fn default() -> Self {
        Self {
            value: 0.0,
            thresholds: Vec::new(),
            error: None,
        }
    }
}

impl GaugePaneData {
    /// Create gauge data with a value.
    pub fn with_value(value: f64) -> Self {
        Self {
            value,
            ..Default::default()
        }
    }

    /// Create error result.
    pub fn with_error(message: impl Into<String>) -> Self {
        Self {
            error: Some(message.into()),
            ..Default::default()
        }
    }

    /// Add a threshold.
    pub fn threshold(mut self, threshold: ThresholdConfig) -> Self {
        self.thresholds.push(threshold);
        self
    }
}

/// Trait for the host application to implement.
///
/// This provides the interface that plugins use to interact with the host
/// (typically the Enya editor). The host implements this trait and provides
/// it to plugins via the `PluginContext`.
pub trait PluginHost: Send + Sync {
    /// Send a notification to the user.
    fn notify(&self, level: NotificationLevel, message: &str);

    /// Request a UI repaint.
    fn request_repaint(&self);

    /// Log a message.
    fn log(&self, level: LogLevel, message: &str);

    /// Get the host application version.
    fn version(&self) -> &'static str;

    /// Check if running in WASM environment.
    fn is_wasm(&self) -> bool;

    /// Get the current theme.
    fn theme(&self) -> Theme;

    /// Get the current theme name as a string (e.g., "tokyo-night", "catppuccin").
    fn theme_name(&self) -> &'static str;

    /// Write text to the system clipboard.
    /// Returns true if successful, false if clipboard is unavailable.
    fn clipboard_write(&self, text: &str) -> bool;

    /// Read text from the system clipboard.
    /// Returns None if clipboard is empty or unavailable.
    fn clipboard_read(&self) -> Option<String>;

    /// Spawn an async task (may not be available in all environments).
    fn spawn(&self, future: BoxFuture<()>);

    /// Perform an HTTP GET request.
    /// Returns the response or an error message.
    fn http_get(
        &self,
        url: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError>;

    /// Perform an HTTP POST request.
    /// Returns the response or an error message.
    fn http_post(
        &self,
        url: &str,
        body: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError>;

    // ==================== Pane Management ====================

    /// Add a query pane with the given PromQL query and optional title.
    fn add_query_pane(&self, query: &str, title: Option<&str>);

    /// Add a logs pane.
    fn add_logs_pane(&self);

    /// Add a tracing pane, optionally pre-filled with a trace ID.
    fn add_tracing_pane(&self, trace_id: Option<&str>);

    /// Add a terminal pane (native only, no-op on WASM).
    fn add_terminal_pane(&self);

    /// Add a SQL pane.
    fn add_sql_pane(&self);

    /// Close the currently focused pane.
    fn close_focused_pane(&self);

    /// Focus pane in the given direction ("left", "right", "up", "down").
    fn focus_pane(&self, direction: &str);

    // ==================== Time Range ====================

    /// Set time range to a preset (e.g., "5m", "15m", "1h", "6h", "24h", "7d").
    fn set_time_range_preset(&self, preset: &str);

    /// Set absolute time range (start and end in seconds since Unix epoch).
    fn set_time_range_absolute(&self, start_secs: f64, end_secs: f64);

    /// Get the current time range as (start_secs, end_secs).
    /// Note: This returns cached values; may not reflect real-time updates.
    fn get_time_range(&self) -> (f64, f64);

    // ==================== Custom Panes ====================

    /// Register a custom table pane type.
    fn register_custom_table_pane(&self, config: CustomTableConfig);

    /// Add an instance of a custom table pane.
    fn add_custom_table_pane(&self, pane_type: &str);

    /// Update data for a custom table pane by pane ID.
    /// Called by plugins when they have new data to display.
    fn update_custom_table_data(&self, pane_id: usize, data: CustomTableData);

    /// Update data for all custom table panes of a given type.
    /// Called by plugins when they have new data to display.
    fn update_custom_table_data_by_type(&self, pane_type: &str, data: CustomTableData);

    // ==================== Custom Chart Panes ====================

    /// Register a custom chart pane type.
    fn register_custom_chart_pane(&self, config: CustomChartConfig);

    /// Add an instance of a custom chart pane.
    fn add_custom_chart_pane(&self, pane_type: &str);

    /// Update data for all custom chart panes of a given type.
    fn update_custom_chart_data_by_type(&self, pane_type: &str, data: CustomChartData);

    // ==================== Custom Stat Panes ====================

    /// Register a custom stat pane type.
    fn register_stat_pane(&self, config: StatPaneConfig);

    /// Add an instance of a stat pane.
    fn add_stat_pane(&self, pane_type: &str);

    /// Update data for all stat panes of a given type.
    fn update_stat_data_by_type(&self, pane_type: &str, data: StatPaneData);

    // ==================== Custom Gauge Panes ====================

    /// Register a custom gauge pane type.
    fn register_gauge_pane(&self, config: GaugePaneConfig);

    /// Add an instance of a gauge pane.
    fn add_gauge_pane(&self, pane_type: &str);

    /// Update data for all gauge panes of a given type.
    fn update_gauge_data_by_type(&self, pane_type: &str, data: GaugePaneData);
}

/// Reference-counted plugin host.
pub type PluginHostRef = Arc<dyn PluginHost>;

/// Context provided to plugins for interacting with the host.
pub struct PluginContext {
    host: PluginHostRef,
}

impl PluginContext {
    /// Create a new plugin context with the given host.
    pub fn new(host: PluginHostRef) -> Self {
        Self { host }
    }

    /// Send a notification to the user.
    pub fn notify(&self, level: &str, message: &str) {
        self.host.notify(NotificationLevel::parse(level), message);
    }

    /// Request a UI repaint.
    pub fn request_repaint(&self) {
        self.host.request_repaint();
    }

    /// Log a message.
    pub fn log(&self, level: LogLevel, message: &str) {
        self.host.log(level, message);
    }

    /// Get the host application version.
    pub fn editor_version(&self) -> &'static str {
        self.host.version()
    }

    /// Check if running in WASM environment.
    pub fn is_wasm(&self) -> bool {
        self.host.is_wasm()
    }

    /// Get the current theme.
    pub fn theme(&self) -> Theme {
        self.host.theme()
    }

    /// Get the current theme name as a string.
    pub fn theme_name(&self) -> &'static str {
        self.host.theme_name()
    }

    /// Write text to the system clipboard.
    pub fn clipboard_write(&self, text: &str) -> bool {
        self.host.clipboard_write(text)
    }

    /// Read text from the system clipboard.
    pub fn clipboard_read(&self) -> Option<String> {
        self.host.clipboard_read()
    }

    /// Spawn an async task.
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.host.spawn(Box::pin(future));
    }

    /// Get a reference to the underlying host.
    pub fn host(&self) -> &PluginHostRef {
        &self.host
    }

    /// Perform an HTTP GET request.
    pub fn http_get(
        &self,
        url: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError> {
        self.host.http_get(url, headers)
    }

    /// Perform an HTTP POST request.
    pub fn http_post(
        &self,
        url: &str,
        body: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError> {
        self.host.http_post(url, body, headers)
    }

    // ==================== Pane Management ====================

    /// Add a query pane with the given PromQL query and optional title.
    pub fn add_query_pane(&self, query: &str, title: Option<&str>) {
        self.host.add_query_pane(query, title);
    }

    /// Add a logs pane.
    pub fn add_logs_pane(&self) {
        self.host.add_logs_pane();
    }

    /// Add a tracing pane, optionally pre-filled with a trace ID.
    pub fn add_tracing_pane(&self, trace_id: Option<&str>) {
        self.host.add_tracing_pane(trace_id);
    }

    /// Add a terminal pane (native only, no-op on WASM).
    pub fn add_terminal_pane(&self) {
        self.host.add_terminal_pane();
    }

    /// Add a SQL pane.
    pub fn add_sql_pane(&self) {
        self.host.add_sql_pane();
    }

    /// Close the currently focused pane.
    pub fn close_focused_pane(&self) {
        self.host.close_focused_pane();
    }

    /// Focus pane in the given direction ("left", "right", "up", "down").
    pub fn focus_pane(&self, direction: &str) {
        self.host.focus_pane(direction);
    }

    // ==================== Time Range ====================

    /// Set time range to a preset (e.g., "5m", "15m", "1h", "6h", "24h", "7d").
    pub fn set_time_range_preset(&self, preset: &str) {
        self.host.set_time_range_preset(preset);
    }

    /// Set absolute time range (start and end in seconds since Unix epoch).
    pub fn set_time_range_absolute(&self, start_secs: f64, end_secs: f64) {
        self.host.set_time_range_absolute(start_secs, end_secs);
    }

    /// Get the current time range as (start_secs, end_secs).
    pub fn get_time_range(&self) -> (f64, f64) {
        self.host.get_time_range()
    }

    // ==================== Custom Panes ====================

    /// Register a custom table pane type.
    pub fn register_custom_table_pane(&self, config: CustomTableConfig) {
        self.host.register_custom_table_pane(config);
    }

    /// Add an instance of a custom table pane.
    pub fn add_custom_table_pane(&self, pane_type: &str) {
        self.host.add_custom_table_pane(pane_type);
    }

    /// Update data for a custom table pane.
    pub fn update_custom_table_data(&self, pane_id: usize, data: CustomTableData) {
        self.host.update_custom_table_data(pane_id, data);
    }

    /// Update data for all custom table panes of a given type.
    pub fn update_custom_table_data_by_type(&self, pane_type: &str, data: CustomTableData) {
        self.host.update_custom_table_data_by_type(pane_type, data);
    }

    // ==================== Custom Chart Panes ====================

    /// Register a custom chart pane type.
    pub fn register_custom_chart_pane(&self, config: CustomChartConfig) {
        self.host.register_custom_chart_pane(config);
    }

    /// Add an instance of a custom chart pane.
    pub fn add_custom_chart_pane(&self, pane_type: &str) {
        self.host.add_custom_chart_pane(pane_type);
    }

    /// Update data for all custom chart panes of a given type.
    pub fn update_custom_chart_data_by_type(&self, pane_type: &str, data: CustomChartData) {
        self.host.update_custom_chart_data_by_type(pane_type, data);
    }

    // ==================== Custom Stat Panes ====================

    /// Register a custom stat pane type.
    pub fn register_stat_pane(&self, config: StatPaneConfig) {
        self.host.register_stat_pane(config);
    }

    /// Add an instance of a stat pane.
    pub fn add_stat_pane(&self, pane_type: &str) {
        self.host.add_stat_pane(pane_type);
    }

    /// Update data for all stat panes of a given type.
    pub fn update_stat_data_by_type(&self, pane_type: &str, data: StatPaneData) {
        self.host.update_stat_data_by_type(pane_type, data);
    }

    // ==================== Custom Gauge Panes ====================

    /// Register a custom gauge pane type.
    pub fn register_gauge_pane(&self, config: GaugePaneConfig) {
        self.host.register_gauge_pane(config);
    }

    /// Add an instance of a gauge pane.
    pub fn add_gauge_pane(&self, pane_type: &str) {
        self.host.add_gauge_pane(pane_type);
    }

    /// Update data for all gauge panes of a given type.
    pub fn update_gauge_data_by_type(&self, pane_type: &str, data: GaugePaneData) {
        self.host.update_gauge_data_by_type(pane_type, data);
    }
}

/// Reference-counted plugin context.
pub type PluginContextRef = Arc<PluginContext>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_level_parse() {
        assert_eq!(NotificationLevel::parse("error"), NotificationLevel::Error);
        assert_eq!(NotificationLevel::parse("warn"), NotificationLevel::Warning);
        assert_eq!(
            NotificationLevel::parse("warning"),
            NotificationLevel::Warning
        );
        assert_eq!(NotificationLevel::parse("info"), NotificationLevel::Info);
        // Unknown values default to Info
        assert_eq!(NotificationLevel::parse("unknown"), NotificationLevel::Info);
        assert_eq!(NotificationLevel::parse(""), NotificationLevel::Info);
        assert_eq!(NotificationLevel::parse("ERROR"), NotificationLevel::Info); // case sensitive
    }

    #[test]
    fn test_log_level_parse() {
        assert_eq!(LogLevel::parse("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::parse("info"), LogLevel::Info);
        assert_eq!(LogLevel::parse("warn"), LogLevel::Warn);
        assert_eq!(LogLevel::parse("warning"), LogLevel::Warn);
        assert_eq!(LogLevel::parse("error"), LogLevel::Error);
        // Unknown values default to Info
        assert_eq!(LogLevel::parse("unknown"), LogLevel::Info);
        assert_eq!(LogLevel::parse(""), LogLevel::Info);
        // Case-insensitive parsing
        assert_eq!(LogLevel::parse("DEBUG"), LogLevel::Debug);
        assert_eq!(LogLevel::parse("WARN"), LogLevel::Warn);
        assert_eq!(LogLevel::parse("Error"), LogLevel::Error);
    }

    #[test]
    fn test_theme_default() {
        assert_eq!(Theme::default(), Theme::Dark);
    }

    #[test]
    fn test_http_response_clone() {
        let mut headers = FxHashMap::default();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpResponse {
            status: 200,
            body: "test body".to_string(),
            headers,
        };

        let cloned = response.clone();
        assert_eq!(cloned.status, 200);
        assert_eq!(cloned.body, "test body");
        assert_eq!(
            cloned.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn test_http_error_clone() {
        let error = HttpError {
            message: "Network error".to_string(),
        };

        let cloned = error.clone();
        assert_eq!(cloned.message, "Network error");
    }
}
