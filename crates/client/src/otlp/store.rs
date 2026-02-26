//! In-memory telemetry store for OTLP data.
//!
//! Provides thread-safe bounded storage for traces and logs received via OTLP.
//! Uses `parking_lot::RwLock` for low-contention concurrent access between
//! the OTLP receiver (writer) and the editor clients (readers).

use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::logs::LogEntry;
use crate::tracing::tempo::types::{Trace, TraceSummary};

/// Configuration for the telemetry store's bounded capacity.
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Maximum number of traces to retain (default: 1000).
    pub max_traces: usize,
    /// Maximum number of log entries to retain (default: 50_000).
    pub max_log_entries: usize,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            max_traces: 1000,
            max_log_entries: 50_000,
        }
    }
}

/// Thread-safe in-memory telemetry store.
///
/// Stores recently-received OTLP data in bounded collections.
/// The OTLP receiver writes to this store, and the OTLP client trait
/// implementations read from it.
pub struct TelemetryStore {
    traces: RwLock<TraceStore>,
    logs: RwLock<LogStore>,
}

impl TelemetryStore {
    /// Create a new telemetry store with the given configuration.
    pub fn new(config: StoreConfig) -> Arc<Self> {
        Arc::new(Self {
            traces: RwLock::new(TraceStore::new(config.max_traces)),
            logs: RwLock::new(LogStore::new(config.max_log_entries)),
        })
    }

    // === Trace operations ===

    /// Insert or update a trace in the store.
    pub fn insert_trace(&self, trace: Trace) {
        self.traces.write().insert(trace);
    }

    /// Get a trace by its ID.
    pub fn get_trace(&self, trace_id: &str) -> Option<Trace> {
        self.traces.read().get(trace_id)
    }

    /// Search traces matching the given criteria. Returns summaries.
    #[allow(clippy::too_many_arguments)]
    pub fn search_traces(
        &self,
        service_name: Option<&str>,
        operation_name: Option<&str>,
        min_duration_us: Option<u64>,
        max_duration_us: Option<u64>,
        start_time_us: Option<u64>,
        end_time_us: Option<u64>,
        limit: usize,
    ) -> Vec<TraceSummary> {
        self.traces.read().search(
            service_name,
            operation_name,
            min_duration_us,
            max_duration_us,
            start_time_us,
            end_time_us,
            limit,
        )
    }

    /// Get the number of stored traces.
    pub fn trace_count(&self) -> usize {
        self.traces.read().len()
    }

    // === Log operations ===

    /// Insert log entries into the store.
    pub fn insert_logs(&self, entries: Vec<LogEntry>) {
        self.logs.write().insert_batch(entries);
    }

    /// Query logs matching the given filters.
    pub fn query_logs(
        &self,
        start_ns: i64,
        end_ns: i64,
        labels: &FxHashMap<String, String>,
        contains: Option<&str>,
        limit: usize,
    ) -> Vec<LogEntry> {
        self.logs
            .read()
            .query(start_ns, end_ns, labels, contains, limit)
    }

    /// Get all known label keys from stored logs.
    pub fn known_log_labels(&self) -> Vec<String> {
        self.logs.read().known_labels()
    }

    /// Get the number of stored log entries.
    pub fn log_count(&self) -> usize {
        self.logs.read().len()
    }
}

// ============================================================================
// TraceStore: bounded trace storage with LRU eviction
// ============================================================================

struct TraceStore {
    /// Traces indexed by trace_id.
    traces: FxHashMap<String, Trace>,
    /// Insertion order for LRU eviction (oldest first).
    order: VecDeque<String>,
    /// Maximum number of traces to retain.
    max_traces: usize,
}

impl TraceStore {
    fn new(max_traces: usize) -> Self {
        Self {
            traces: FxHashMap::default(),
            order: VecDeque::new(),
            max_traces,
        }
    }

    fn insert(&mut self, trace: Trace) {
        let trace_id = trace.trace_id.clone();

        // If trace already exists, update it in place (don't change order)
        if let Some(existing) = self.traces.get_mut(&trace_id) {
            *existing = trace;
            return;
        }

        // Evict oldest if at capacity
        while self.traces.len() >= self.max_traces {
            if let Some(old_id) = self.order.pop_front() {
                self.traces.remove(&old_id);
            } else {
                break;
            }
        }

        self.order.push_back(trace_id.clone());
        self.traces.insert(trace_id, trace);
    }

    fn get(&self, trace_id: &str) -> Option<Trace> {
        self.traces.get(trace_id).cloned()
    }

    fn len(&self) -> usize {
        self.traces.len()
    }

    #[allow(clippy::too_many_arguments)]
    fn search(
        &self,
        service_name: Option<&str>,
        operation_name: Option<&str>,
        min_duration_us: Option<u64>,
        max_duration_us: Option<u64>,
        start_time_us: Option<u64>,
        end_time_us: Option<u64>,
        limit: usize,
    ) -> Vec<TraceSummary> {
        let mut results: Vec<TraceSummary> = self
            .traces
            .values()
            .filter(|trace| {
                // Filter by time range
                if let Some(start) = start_time_us {
                    if trace.start_time_us < start {
                        return false;
                    }
                }
                if let Some(end) = end_time_us {
                    if trace.start_time_us > end {
                        return false;
                    }
                }
                // Filter by duration
                if let Some(min) = min_duration_us {
                    if trace.duration_us < min {
                        return false;
                    }
                }
                if let Some(max) = max_duration_us {
                    if trace.duration_us > max {
                        return false;
                    }
                }
                // Filter by service name
                if let Some(svc) = service_name {
                    if !trace.services.iter().any(|s| s == svc) {
                        return false;
                    }
                }
                // Filter by operation name (check root span)
                if let Some(op) = operation_name {
                    let has_op = trace.spans.iter().any(|s| s.operation_name == op);
                    if !has_op {
                        return false;
                    }
                }
                true
            })
            .map(|trace| {
                let root = trace
                    .root_span_id
                    .as_ref()
                    .and_then(|id| trace.get_span(id));
                let error_count = trace
                    .spans
                    .iter()
                    .filter(|s| s.status == crate::tracing::tempo::types::SpanStatus::Error)
                    .count();
                TraceSummary {
                    trace_id: trace.trace_id.clone(),
                    root_service_name: root.map(|s| s.service_name.clone()).unwrap_or_default(),
                    root_operation_name: root.map(|s| s.operation_name.clone()).unwrap_or_default(),
                    start_time_us: trace.start_time_us,
                    duration_us: trace.duration_us,
                    span_count: trace.spans.len(),
                    error_count,
                }
            })
            .collect();

        // Sort by start time descending (newest first)
        results.sort_by(|a, b| b.start_time_us.cmp(&a.start_time_us));
        results.truncate(limit);
        results
    }
}

// ============================================================================
// LogStore: bounded log entry ring buffer
// ============================================================================

struct LogStore {
    /// Log entries (oldest first).
    entries: VecDeque<LogEntry>,
    /// Maximum number of entries.
    max_entries: usize,
}

impl LogStore {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
        }
    }

    fn insert_batch(&mut self, entries: Vec<LogEntry>) {
        for entry in entries {
            if self.entries.len() >= self.max_entries {
                self.entries.pop_front();
            }
            self.entries.push_back(entry);
        }
    }

    fn query(
        &self,
        start_ns: i64,
        end_ns: i64,
        labels: &FxHashMap<String, String>,
        contains: Option<&str>,
        limit: usize,
    ) -> Vec<LogEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                // Time range filter
                if entry.timestamp_ns < start_ns || entry.timestamp_ns > end_ns {
                    return false;
                }
                // Label filter
                for (key, value) in labels {
                    match entry.labels.get(key) {
                        Some(v) if v == value => {}
                        _ => return false,
                    }
                }
                // Text search
                if let Some(text) = contains {
                    if !entry.message.to_lowercase().contains(&text.to_lowercase()) {
                        return false;
                    }
                }
                true
            })
            .rev() // Newest first
            .take(limit)
            .cloned()
            .collect()
    }

    fn known_labels(&self) -> Vec<String> {
        let mut labels: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();
        for entry in &self.entries {
            for key in entry.labels.keys() {
                labels.insert(key.clone());
            }
        }
        let mut sorted: Vec<String> = labels.into_iter().collect();
        sorted.sort();
        sorted
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracing::tempo::types::{Span, SpanStatus};

    fn make_trace(id: &str, service: &str, duration_us: u64) -> Trace {
        let spans = vec![Span {
            span_id: format!("{id}-span1"),
            trace_id: id.to_string(),
            parent_span_id: None,
            operation_name: "root".to_string(),
            service_name: service.to_string(),
            start_time_us: 1_000_000,
            duration_us,
            status: SpanStatus::Ok,
            tags: FxHashMap::default(),
            logs: vec![],
            depth: 0,
        }];
        Trace::from_spans(id.to_string(), spans)
    }

    #[test]
    fn test_store_insert_and_get_trace() {
        let store = TelemetryStore::new(StoreConfig::default());
        let trace = make_trace("trace1", "svc-a", 5000);

        store.insert_trace(trace);
        assert_eq!(store.trace_count(), 1);

        let fetched = store.get_trace("trace1").unwrap();
        assert_eq!(fetched.trace_id, "trace1");
    }

    #[test]
    fn test_store_evicts_oldest() {
        let store = TelemetryStore::new(StoreConfig {
            max_traces: 2,
            ..Default::default()
        });

        store.insert_trace(make_trace("t1", "svc", 100));
        store.insert_trace(make_trace("t2", "svc", 200));
        store.insert_trace(make_trace("t3", "svc", 300));

        assert_eq!(store.trace_count(), 2);
        assert!(store.get_trace("t1").is_none()); // evicted
        assert!(store.get_trace("t2").is_some());
        assert!(store.get_trace("t3").is_some());
    }

    #[test]
    fn test_log_store_query() {
        let store = TelemetryStore::new(StoreConfig::default());

        let mut labels = FxHashMap::default();
        labels.insert("service".to_string(), "api".to_string());

        store.insert_logs(vec![
            LogEntry {
                timestamp_ns: 1000,
                message: "hello world".to_string(),
                labels: labels.clone(),
                level: None,
            },
            LogEntry {
                timestamp_ns: 2000,
                message: "error occurred".to_string(),
                labels: labels.clone(),
                level: None,
            },
        ]);

        let results = store.query_logs(0, 3000, &FxHashMap::default(), None, 100);
        assert_eq!(results.len(), 2);

        // Filter by text
        let results = store.query_logs(0, 3000, &FxHashMap::default(), Some("error"), 100);
        assert_eq!(results.len(), 1);
        assert!(results[0].message.contains("error"));
    }

    #[test]
    fn test_search_by_service_name() {
        let store = TelemetryStore::new(StoreConfig::default());
        store.insert_trace(make_trace("t1", "api", 5000));
        store.insert_trace(make_trace("t2", "worker", 3000));
        store.insert_trace(make_trace("t3", "api", 7000));

        let results = store.search_traces(Some("api"), None, None, None, None, None, 100);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.root_service_name == "api"));
    }

    #[test]
    fn test_search_by_duration_range() {
        let store = TelemetryStore::new(StoreConfig::default());
        store.insert_trace(make_trace("t1", "svc", 1000));
        store.insert_trace(make_trace("t2", "svc", 5000));
        store.insert_trace(make_trace("t3", "svc", 10000));

        // Min duration filter
        let results = store.search_traces(None, None, Some(4000), None, None, None, 100);
        assert_eq!(results.len(), 2);

        // Max duration filter
        let results = store.search_traces(None, None, None, Some(6000), None, None, 100);
        assert_eq!(results.len(), 2);

        // Both min and max
        let results = store.search_traces(None, None, Some(2000), Some(8000), None, None, 100);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].trace_id, "t2");
    }

    #[test]
    fn test_search_with_limit() {
        let store = TelemetryStore::new(StoreConfig::default());
        for i in 0..10 {
            store.insert_trace(make_trace(&format!("t{i}"), "svc", 1000));
        }

        let results = store.search_traces(None, None, None, None, None, None, 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_search_returns_newest_first() {
        let store = TelemetryStore::new(StoreConfig::default());

        // All traces have start_time_us = 1_000_000 from make_trace,
        // so ordering is deterministic by start_time
        let mut spans = vec![Span {
            span_id: "s1".to_string(),
            trace_id: "early".to_string(),
            parent_span_id: None,
            operation_name: "root".to_string(),
            service_name: "svc".to_string(),
            start_time_us: 100,
            duration_us: 1000,
            status: SpanStatus::Ok,
            tags: FxHashMap::default(),
            logs: vec![],
            depth: 0,
        }];
        store.insert_trace(Trace::from_spans("early".to_string(), spans.clone()));

        spans[0].trace_id = "late".to_string();
        spans[0].span_id = "s2".to_string();
        spans[0].start_time_us = 999_000;
        store.insert_trace(Trace::from_spans("late".to_string(), spans));

        let results = store.search_traces(None, None, None, None, None, None, 100);
        assert_eq!(results[0].trace_id, "late"); // newest first
        assert_eq!(results[1].trace_id, "early");
    }

    #[test]
    fn test_trace_update_in_place() {
        let store = TelemetryStore::new(StoreConfig::default());
        store.insert_trace(make_trace("t1", "svc-v1", 1000));
        store.insert_trace(make_trace("t1", "svc-v2", 2000));

        assert_eq!(store.trace_count(), 1); // same trace_id, updated in place
        let trace = store.get_trace("t1").unwrap();
        assert_eq!(trace.duration_us, 2000);
    }

    #[test]
    fn test_log_query_with_label_filter() {
        let store = TelemetryStore::new(StoreConfig::default());

        let mut api_labels = FxHashMap::default();
        api_labels.insert("service".to_string(), "api".to_string());

        let mut worker_labels = FxHashMap::default();
        worker_labels.insert("service".to_string(), "worker".to_string());

        store.insert_logs(vec![
            LogEntry {
                timestamp_ns: 1000,
                message: "api log".to_string(),
                labels: api_labels.clone(),
                level: None,
            },
            LogEntry {
                timestamp_ns: 2000,
                message: "worker log".to_string(),
                labels: worker_labels,
                level: None,
            },
        ]);

        let results = store.query_logs(0, 5000, &api_labels, None, 100);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "api log");
    }

    #[test]
    fn test_known_log_labels_sorted() {
        let store = TelemetryStore::new(StoreConfig::default());

        let mut labels = FxHashMap::default();
        labels.insert("service".to_string(), "api".to_string());
        labels.insert("env".to_string(), "prod".to_string());
        labels.insert("app".to_string(), "web".to_string());

        store.insert_logs(vec![LogEntry {
            timestamp_ns: 1000,
            message: "msg".to_string(),
            labels,
            level: None,
        }]);

        let known = store.known_log_labels();
        assert_eq!(known, vec!["app", "env", "service"]);
    }

    #[test]
    fn test_log_query_case_insensitive_contains() {
        let store = TelemetryStore::new(StoreConfig::default());
        store.insert_logs(vec![LogEntry {
            timestamp_ns: 1000,
            message: "ERROR: Something failed".to_string(),
            labels: FxHashMap::default(),
            level: None,
        }]);

        let results = store.query_logs(0, 5000, &FxHashMap::default(), Some("error"), 100);
        assert_eq!(results.len(), 1);

        let results = store.query_logs(0, 5000, &FxHashMap::default(), Some("ERROR"), 100);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_log_store_evicts() {
        let store = TelemetryStore::new(StoreConfig {
            max_log_entries: 2,
            ..Default::default()
        });

        store.insert_logs(vec![
            LogEntry {
                timestamp_ns: 1000,
                message: "first".to_string(),
                labels: FxHashMap::default(),
                level: None,
            },
            LogEntry {
                timestamp_ns: 2000,
                message: "second".to_string(),
                labels: FxHashMap::default(),
                level: None,
            },
            LogEntry {
                timestamp_ns: 3000,
                message: "third".to_string(),
                labels: FxHashMap::default(),
                level: None,
            },
        ]);

        assert_eq!(store.log_count(), 2);
        let results = store.query_logs(0, 5000, &FxHashMap::default(), None, 100);
        assert_eq!(results.len(), 2);
        // "first" was evicted, should have "second" and "third"
        assert!(results.iter().any(|e| e.message == "second"));
        assert!(results.iter().any(|e| e.message == "third"));
    }
}
