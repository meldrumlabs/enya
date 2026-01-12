//! Demo logs client for offline/showcase mode.
//!
//! Provides a `DemoLogsClient` that implements `LogsClient` with realistic
//! mock SQL query logs, enabling the editor to work without a real Loki connection.

use poll_promise::Promise;
use rustc_hash::FxHashMap;

use crate::logs::{
    LogEntry, LogLevel, LogsClient, LogsQuery, LogsResponse, LogsResult, StreamsResult,
};
use crate::{BackendInfo, HealthCheckResult};

/// Demo logs client providing realistic mock SQL query logs.
///
/// This client implements the `LogsClient` trait with generated log data
/// that simulates database query logs. It's useful for:
/// - Offline development and testing
/// - Showcasing the log viewer without a real backend
/// - Demonstrating metric→log correlation features
///
/// # Example
///
/// ```ignore
/// use enya_client::logs::{DemoLogsClient, LogsClient, LogsQuery};
///
/// let client = DemoLogsClient::new();
/// let query = LogsQuery::new(start_ns, end_ns);
/// let promise = client.query_logs(query, &ctx);
/// ```
pub struct DemoLogsClient {
    /// Available log streams (label combinations)
    streams: Vec<DemoStream>,
}

/// A demo log stream definition.
struct DemoStream {
    labels: FxHashMap<String, String>,
}

impl Default for DemoLogsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoLogsClient {
    /// Create a new demo logs client with predefined streams.
    #[must_use]
    pub fn new() -> Self {
        Self {
            streams: build_demo_streams(),
        }
    }

    /// Generate demo log entries for a query.
    fn generate_logs(&self, query: &LogsQuery) -> LogsResponse {
        let mut entries = Vec::new();

        // Filter streams based on query labels
        let matching_streams: Vec<&DemoStream> = self
            .streams
            .iter()
            .filter(|s| {
                query
                    .labels
                    .iter()
                    .all(|(k, v)| s.labels.get(k).map(|sv| sv == v).unwrap_or(false))
            })
            .collect();

        let streams_count = if matching_streams.is_empty() {
            // If no filters, use all streams
            self.streams.len()
        } else {
            matching_streams.len()
        };

        let streams_to_use: Vec<&DemoStream> = if matching_streams.is_empty() {
            self.streams.iter().collect()
        } else {
            matching_streams
        };

        // Generate logs for each stream
        for stream in &streams_to_use {
            let stream_entries = generate_stream_logs(
                &stream.labels,
                query.start_ns,
                query.end_ns,
                query.contains.as_deref(),
            );
            entries.extend(stream_entries);
        }

        // Sort by timestamp
        entries.sort_by_key(|e| e.timestamp_ns);

        // Apply limit
        if entries.len() > query.limit {
            entries.truncate(query.limit);
        }

        LogsResponse {
            entries,
            streams_count,
        }
    }

    /// Get all available stream labels.
    fn all_labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = self
            .streams
            .iter()
            .flat_map(|s| s.labels.keys().cloned())
            .collect();
        labels.sort();
        labels.dedup();
        labels
    }
}

impl LogsClient for DemoLogsClient {
    fn query_logs(&self, query: LogsQuery, _ctx: &egui::Context) -> Promise<LogsResult> {
        let response = self.generate_logs(&query);
        Promise::from_ready(Ok(response))
    }

    fn fetch_streams(&self, _ctx: &egui::Context) -> Promise<StreamsResult> {
        Promise::from_ready(Ok(self.all_labels()))
    }

    fn backend_type(&self) -> &'static str {
        "demo"
    }

    fn health_check(&self, _ctx: &egui::Context) -> Promise<HealthCheckResult> {
        Promise::from_ready(Ok(BackendInfo {
            backend_type: "demo".to_string(),
            version: "offline".to_string(),
        }))
    }
}

/// Build the demo stream definitions.
fn build_demo_streams() -> Vec<DemoStream> {
    let mut streams = Vec::new();

    // Database service streams
    for app in ["api-server", "order-service", "user-service"] {
        for db in ["postgres", "mysql"] {
            let mut labels = FxHashMap::default();
            labels.insert("app".to_string(), app.to_string());
            labels.insert("db".to_string(), db.to_string());
            labels.insert("component".to_string(), "database".to_string());
            streams.push(DemoStream { labels });
        }
    }

    streams
}

/// Generate log entries for a stream.
fn generate_stream_logs(
    labels: &FxHashMap<String, String>,
    start_ns: i64,
    end_ns: i64,
    contains_filter: Option<&str>,
) -> Vec<LogEntry> {
    let mut entries = Vec::new();
    let duration_ns = end_ns - start_ns;

    // Generate ~50-100 log entries per stream for the time range
    let num_entries = 50 + (hash_labels(labels) % 50) as usize;
    let interval_ns = duration_ns / num_entries as i64;

    for i in 0..num_entries {
        let timestamp_ns = start_ns + (i as i64) * interval_ns + (hash_u64(i as u64) % 1000) as i64;

        // Generate a SQL query log
        let (query_text, duration_ms, level) = generate_sql_log(i, labels);

        let message = format!("duration={duration_ms}ms query=\"{query_text}\"");

        // Apply contains filter
        if let Some(filter) = contains_filter {
            if !message.to_uppercase().contains(&filter.to_uppercase()) {
                continue;
            }
        }

        let mut entry_labels = labels.clone();
        entry_labels.insert("level".to_string(), level_to_string(level));

        entries.push(LogEntry {
            timestamp_ns,
            message,
            labels: entry_labels,
            level: Some(level),
        });
    }

    entries
}

/// Generate a realistic SQL query log entry.
fn generate_sql_log(index: usize, labels: &FxHashMap<String, String>) -> (String, u64, LogLevel) {
    let hash = hash_u64(index as u64).wrapping_add(hash_labels(labels));

    // Query templates
    let queries = [
        // SELECT queries
        (
            "SELECT id, name, email FROM users WHERE id = $1",
            2,
            LogLevel::Debug,
        ),
        (
            "SELECT * FROM orders WHERE user_id = $1 AND status = $2",
            5,
            LogLevel::Debug,
        ),
        (
            "SELECT COUNT(*) FROM products WHERE category = $1",
            3,
            LogLevel::Debug,
        ),
        (
            "SELECT u.*, o.total FROM users u JOIN orders o ON u.id = o.user_id WHERE o.created_at > $1",
            15,
            LogLevel::Info,
        ),
        (
            "SELECT * FROM sessions WHERE expires_at < NOW()",
            8,
            LogLevel::Debug,
        ),
        // Slow SELECT (simulating N+1 or missing index)
        (
            "SELECT * FROM order_items oi JOIN products p ON oi.product_id = p.id WHERE oi.order_id = $1",
            150,
            LogLevel::Warn,
        ),
        // INSERT queries
        (
            "INSERT INTO audit_log (user_id, action, timestamp) VALUES ($1, $2, NOW())",
            3,
            LogLevel::Debug,
        ),
        (
            "INSERT INTO orders (user_id, total, status) VALUES ($1, $2, $3) RETURNING id",
            5,
            LogLevel::Info,
        ),
        // UPDATE queries
        (
            "UPDATE users SET last_login = NOW() WHERE id = $1",
            2,
            LogLevel::Debug,
        ),
        (
            "UPDATE orders SET status = $1 WHERE id = $2",
            3,
            LogLevel::Debug,
        ),
        (
            "UPDATE inventory SET quantity = quantity - $1 WHERE product_id = $2",
            4,
            LogLevel::Info,
        ),
        // Slow UPDATE (table scan)
        (
            "UPDATE orders SET processed = true WHERE created_at < $1 AND processed = false",
            250,
            LogLevel::Warn,
        ),
        // DELETE queries
        (
            "DELETE FROM sessions WHERE expires_at < NOW()",
            10,
            LogLevel::Info,
        ),
        (
            "DELETE FROM cart_items WHERE cart_id = $1",
            3,
            LogLevel::Debug,
        ),
        // Very slow query (indicates problem)
        (
            "SELECT * FROM orders o JOIN order_items oi ON o.id = oi.order_id JOIN products p ON oi.product_id = p.id WHERE o.created_at BETWEEN $1 AND $2",
            500,
            LogLevel::Error,
        ),
        // Aggregation queries
        (
            "SELECT DATE(created_at), SUM(total) FROM orders GROUP BY DATE(created_at)",
            45,
            LogLevel::Info,
        ),
        (
            "SELECT category, COUNT(*) FROM products GROUP BY category ORDER BY COUNT(*) DESC",
            25,
            LogLevel::Info,
        ),
    ];

    let query_idx = (hash as usize) % queries.len();
    let (base_query, base_duration, base_level) = queries[query_idx];

    // Add some variation to duration (±50%)
    let duration_variation = ((hash >> 8) % 100) as i64 - 50;
    let duration =
        ((base_duration as i64) + (base_duration as i64 * duration_variation / 100)).max(1) as u64;

    // Occasionally make queries slower (simulating load spikes)
    let (final_duration, level) = if (hash >> 16) % 20 == 0 {
        // 5% chance of a spike
        let spike_multiplier = 3 + ((hash >> 24) % 5);
        let spiked_duration = duration * spike_multiplier;
        let spiked_level = if spiked_duration > 200 {
            LogLevel::Error
        } else if spiked_duration > 50 {
            LogLevel::Warn
        } else {
            base_level
        };
        (spiked_duration, spiked_level)
    } else {
        (duration, base_level)
    };

    (base_query.to_string(), final_duration, level)
}

/// Simple hash function for labels.
fn hash_labels(labels: &FxHashMap<String, String>) -> u64 {
    let mut hash: u64 = 0;
    for (k, v) in labels {
        for b in k.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(u64::from(b));
        }
        for b in v.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(u64::from(b));
        }
    }
    hash
}

/// Simple hash function for u64.
fn hash_u64(x: u64) -> u64 {
    let mut h = x;
    h = h.wrapping_mul(0x517cc1b727220a95);
    h ^= h >> 32;
    h
}

/// Convert log level to string.
fn level_to_string(level: LogLevel) -> String {
    match level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_client_query_logs() {
        let client = DemoLogsClient::new();
        let now_ns = 1_609_459_200_000_000_000_i64; // 2021-01-01 00:00:00
        let query = LogsQuery::new(now_ns - 3_600_000_000_000, now_ns);

        let ctx = egui::Context::default();
        let promise = client.query_logs(query, &ctx);

        // Demo client returns immediately
        let result = promise.ready().expect("should be ready");
        let response = result.as_ref().expect("should succeed");

        assert!(!response.entries.is_empty());
        assert!(response.streams_count > 0);

        // Check entries have expected structure
        for entry in &response.entries {
            assert!(!entry.message.is_empty());
            assert!(entry.message.contains("duration="));
            assert!(entry.message.contains("query="));
            assert!(entry.level.is_some());
        }
    }

    #[test]
    fn test_demo_client_query_with_filter() {
        let client = DemoLogsClient::new();
        let now_ns = 1_609_459_200_000_000_000_i64;
        let query = LogsQuery::new(now_ns - 3_600_000_000_000, now_ns).with_contains("SELECT");

        let ctx = egui::Context::default();
        let promise = client.query_logs(query, &ctx);

        let result = promise.ready().expect("should be ready");
        let response = result.as_ref().expect("should succeed");

        // All entries should contain SELECT
        for entry in &response.entries {
            assert!(
                entry.message.to_uppercase().contains("SELECT"),
                "entry should contain SELECT: {}",
                entry.message
            );
        }
    }

    #[test]
    fn test_demo_client_query_with_label_filter() {
        let client = DemoLogsClient::new();
        let now_ns = 1_609_459_200_000_000_000_i64;
        let query =
            LogsQuery::new(now_ns - 3_600_000_000_000, now_ns).with_label("app", "api-server");

        let ctx = egui::Context::default();
        let promise = client.query_logs(query, &ctx);

        let result = promise.ready().expect("should be ready");
        let response = result.as_ref().expect("should succeed");

        // All entries should have the app label
        for entry in &response.entries {
            assert_eq!(
                entry.labels.get("app"),
                Some(&"api-server".to_string()),
                "entry should have app=api-server"
            );
        }
    }

    #[test]
    fn test_demo_client_fetch_streams() {
        let client = DemoLogsClient::new();
        let ctx = egui::Context::default();
        let promise = client.fetch_streams(&ctx);

        let result = promise.ready().expect("should be ready");
        let labels = result.as_ref().expect("should succeed");

        assert!(labels.contains(&"app".to_string()));
        assert!(labels.contains(&"db".to_string()));
        assert!(labels.contains(&"component".to_string()));
    }

    #[test]
    fn test_demo_client_backend_type() {
        let client = DemoLogsClient::new();
        assert_eq!(client.backend_type(), "demo");
    }

    #[test]
    fn test_demo_client_health_check() {
        let client = DemoLogsClient::new();
        let ctx = egui::Context::default();
        let promise = client.health_check(&ctx);

        let result = promise.ready().expect("should be ready");
        let info = result.as_ref().expect("should succeed");

        assert_eq!(info.backend_type, "demo");
        assert_eq!(info.version, "offline");
    }

    #[test]
    fn test_generated_logs_have_variety() {
        let client = DemoLogsClient::new();
        let now_ns = 1_609_459_200_000_000_000_i64;
        let query = LogsQuery::new(now_ns - 3_600_000_000_000, now_ns).with_limit(500);

        let ctx = egui::Context::default();
        let promise = client.query_logs(query, &ctx);

        let result = promise.ready().expect("should be ready");
        let response = result.as_ref().expect("should succeed");

        // Check we have a mix of log levels
        let mut levels: rustc_hash::FxHashSet<LogLevel> = rustc_hash::FxHashSet::default();
        for entry in &response.entries {
            if let Some(level) = entry.level {
                levels.insert(level);
            }
        }

        // Should have at least debug, info, and some warnings
        assert!(levels.len() >= 2, "should have variety of log levels");
    }
}
