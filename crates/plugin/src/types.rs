//! Core types for the plugin system.
//!
//! These types are designed to be independent of any specific editor implementation,
//! allowing the plugin system to be used in different contexts.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

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
    /// Parse a log level from a string.
    pub fn parse(s: &str) -> Self {
        match s {
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
        assert_eq!(LogLevel::parse("DEBUG"), LogLevel::Info); // case sensitive
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
