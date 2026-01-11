//! Shared types for log query responses.

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// A single log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp in nanoseconds since Unix epoch.
    pub timestamp_ns: i64,
    /// The log message content.
    pub message: String,
    /// Labels/metadata associated with this log line.
    pub labels: FxHashMap<String, String>,
    /// Parsed log level, if detectable.
    pub level: Option<LogLevel>,
}

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Parse a log level from common string formats.
    ///
    /// Recognizes: trace, debug, info, warn/warning, error/err, and uppercase variants.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "trace" | "trc" => Some(Self::Trace),
            "debug" | "dbg" => Some(Self::Debug),
            "info" | "inf" => Some(Self::Info),
            "warn" | "warning" | "wrn" => Some(Self::Warn),
            "error" | "err" => Some(Self::Error),
            _ => None,
        }
    }

    /// Try to detect log level from a log message.
    ///
    /// Looks for common patterns like `[INFO]`, `level=info`, `INFO:`, etc.
    #[must_use]
    pub fn detect_from_message(message: &str) -> Option<Self> {
        let upper = message.to_uppercase();

        // Check for bracketed format: [INFO], [ERROR], etc.
        for level in ["TRACE", "DEBUG", "INFO", "WARN", "WARNING", "ERROR", "ERR"] {
            if upper.contains(&format!("[{level}]")) || upper.contains(&format!("{level}:")) {
                return Self::parse(level);
            }
        }

        // Check for key=value format: level=info, level="error"
        if let Some(pos) = upper.find("LEVEL=") {
            let after = &message[pos + 6..];
            let value: String = after
                .trim_start_matches('"')
                .chars()
                .take_while(|c| c.is_alphabetic())
                .collect();
            if let Some(level) = Self::parse(&value) {
                return Some(level);
            }
        }

        None
    }
}

/// Direction for log query results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryDirection {
    /// Oldest entries first.
    Forward,
    /// Newest entries first (default).
    #[default]
    Backward,
}

/// A query for fetching logs.
#[derive(Debug, Clone)]
pub struct LogsQuery {
    /// Backend-specific query string (e.g., LogQL for Loki).
    /// If None, uses labels and contains for filtering.
    pub query: Option<String>,
    /// Filter by these label key-value pairs.
    pub labels: FxHashMap<String, String>,
    /// Simple text search within log messages.
    pub contains: Option<String>,
    /// Start of time range (nanoseconds since Unix epoch).
    pub start_ns: i64,
    /// End of time range (nanoseconds since Unix epoch).
    pub end_ns: i64,
    /// Maximum number of entries to return.
    pub limit: usize,
    /// Sort direction for results.
    pub direction: QueryDirection,
}

impl LogsQuery {
    /// Create a new logs query for the given time range.
    #[must_use]
    pub fn new(start_ns: i64, end_ns: i64) -> Self {
        Self {
            query: None,
            labels: FxHashMap::default(),
            contains: None,
            start_ns,
            end_ns,
            limit: 1000,
            direction: QueryDirection::Backward,
        }
    }

    /// Set a backend-specific query string.
    #[must_use]
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Add a label filter.
    #[must_use]
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Set a text search filter.
    #[must_use]
    pub fn with_contains(mut self, text: impl Into<String>) -> Self {
        self.contains = Some(text.into());
        self
    }

    /// Set the maximum number of results.
    #[must_use]
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set the sort direction.
    #[must_use]
    pub fn with_direction(mut self, direction: QueryDirection) -> Self {
        self.direction = direction;
        self
    }
}

/// Response from a logs query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsResponse {
    /// The log entries returned.
    pub entries: Vec<LogEntry>,
    /// Number of distinct streams that matched.
    pub streams_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parse() {
        assert_eq!(LogLevel::parse("info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::parse("INFO"), Some(LogLevel::Info));
        assert_eq!(LogLevel::parse("warn"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse("warning"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse("error"), Some(LogLevel::Error));
        assert_eq!(LogLevel::parse("err"), Some(LogLevel::Error));
        assert_eq!(LogLevel::parse("unknown"), None);
    }

    #[test]
    fn test_log_level_detect_bracketed() {
        assert_eq!(
            LogLevel::detect_from_message("[INFO] Starting server"),
            Some(LogLevel::Info)
        );
        assert_eq!(
            LogLevel::detect_from_message("[ERROR] Connection failed"),
            Some(LogLevel::Error)
        );
        assert_eq!(
            LogLevel::detect_from_message("[WARN] Deprecated API"),
            Some(LogLevel::Warn)
        );
    }

    #[test]
    fn test_log_level_detect_colon() {
        assert_eq!(
            LogLevel::detect_from_message("INFO: Starting server"),
            Some(LogLevel::Info)
        );
        assert_eq!(
            LogLevel::detect_from_message("ERROR: Connection failed"),
            Some(LogLevel::Error)
        );
    }

    #[test]
    fn test_log_level_detect_key_value() {
        assert_eq!(
            LogLevel::detect_from_message("level=info msg=\"hello\""),
            Some(LogLevel::Info)
        );
        assert_eq!(
            LogLevel::detect_from_message("level=\"error\" msg=\"failed\""),
            Some(LogLevel::Error)
        );
    }

    #[test]
    fn test_logs_query_builder() {
        let query = LogsQuery::new(1000, 2000)
            .with_query("{app=\"myservice\"}")
            .with_label("env", "prod")
            .with_contains("SELECT")
            .with_limit(500)
            .with_direction(QueryDirection::Forward);

        assert_eq!(query.query, Some("{app=\"myservice\"}".to_string()));
        assert_eq!(query.labels.get("env"), Some(&"prod".to_string()));
        assert_eq!(query.contains, Some("SELECT".to_string()));
        assert_eq!(query.limit, 500);
        assert_eq!(query.direction, QueryDirection::Forward);
        assert_eq!(query.start_ns, 1000);
        assert_eq!(query.end_ns, 2000);
    }
}
