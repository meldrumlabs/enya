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
use crate::types::{MetricsBucket, MetricsGroup, QueryResponse, ResultType, Timestamp};

/// Configuration for the telemetry store's bounded capacity.
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Maximum number of traces to retain (default: 1000).
    pub max_traces: usize,
    /// Maximum number of log entries to retain (default: 50_000).
    pub max_log_entries: usize,
    /// Maximum number of data points per metric time series (default: 10_000).
    pub max_metric_data_points: usize,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            max_traces: 1000,
            max_log_entries: 50_000,
            max_metric_data_points: 10_000,
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
    metrics: RwLock<MetricsStore>,
}

impl TelemetryStore {
    /// Create a new telemetry store with the given configuration.
    pub fn new(config: StoreConfig) -> Arc<Self> {
        Arc::new(Self {
            traces: RwLock::new(TraceStore::new(config.max_traces)),
            logs: RwLock::new(LogStore::new(config.max_log_entries)),
            metrics: RwLock::new(MetricsStore::new(config.max_metric_data_points)),
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

    // === Metrics operations ===

    /// Insert a metric data point into the store.
    pub fn insert_metric_point(&self, point: MetricDataPoint) {
        self.metrics.write().insert(point);
    }

    /// Insert a batch of metric data points.
    pub fn insert_metric_points(&self, points: Vec<MetricDataPoint>) {
        let mut store = self.metrics.write();
        for point in points {
            store.insert(point);
        }
    }

    /// Get all known metric names.
    pub fn metric_names(&self) -> Vec<String> {
        self.metrics.read().metric_names()
    }

    /// Get all known label names for a specific metric.
    pub fn metric_label_names(&self, metric: &str) -> Vec<String> {
        self.metrics.read().label_names(metric)
    }

    /// Get all known label values for a specific metric and label name.
    pub fn metric_label_values(&self, metric: &str, label: &str) -> Vec<String> {
        self.metrics.read().label_values(metric, label)
    }

    /// Query a metric and return a QueryResponse compatible with the Prometheus format.
    pub fn query_metric(
        &self,
        metric: &str,
        labels: &FxHashMap<String, String>,
        start_ns: u64,
        end_ns: u64,
        step_ns: u64,
    ) -> QueryResponse {
        self.metrics
            .read()
            .query(metric, labels, start_ns, end_ns, step_ns)
    }

    /// Get the number of stored metric time series.
    pub fn metric_series_count(&self) -> usize {
        self.metrics.read().series_count()
    }

    /// Get the total number of stored metric data points.
    pub fn metric_point_count(&self) -> usize {
        self.metrics.read().point_count()
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
// MetricsStore: bounded time-series storage for OTLP metrics
// ============================================================================

/// A single metric data point to be inserted into the store.
#[derive(Debug, Clone)]
pub struct MetricDataPoint {
    /// Metric name (e.g., "http_requests_total").
    pub name: String,
    /// Labels/attributes (e.g., {"method": "GET", "service": "api"}).
    pub labels: FxHashMap<String, String>,
    /// Timestamp in nanoseconds since epoch.
    pub timestamp_ns: u64,
    /// The metric value.
    pub value: f64,
}

/// A unique time series identified by metric name + sorted label set.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SeriesKey {
    name: String,
    /// Labels as a sorted string: "k1=v1,k2=v2"
    labels_key: String,
}

/// A time-ordered data point within a series.
#[derive(Debug, Clone)]
struct StoredPoint {
    timestamp_ns: u64,
    value: f64,
}

struct MetricsStore {
    /// Time series indexed by their unique key.
    series: FxHashMap<SeriesKey, TimeSeries>,
    /// Maximum data points per series.
    max_points_per_series: usize,
}

struct TimeSeries {
    /// The original labels for this series.
    labels: FxHashMap<String, String>,
    /// Metric name.
    name: String,
    /// Data points sorted by timestamp.
    points: VecDeque<StoredPoint>,
}

impl MetricsStore {
    fn new(max_points_per_series: usize) -> Self {
        Self {
            series: FxHashMap::default(),
            max_points_per_series,
        }
    }

    fn make_key(name: &str, labels: &FxHashMap<String, String>) -> SeriesKey {
        let mut pairs: Vec<_> = labels.iter().collect();
        pairs.sort_by_key(|(k, _)| *k);
        let labels_key = pairs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",");
        SeriesKey {
            name: name.to_string(),
            labels_key,
        }
    }

    fn insert(&mut self, point: MetricDataPoint) {
        let key = Self::make_key(&point.name, &point.labels);
        let series = self.series.entry(key).or_insert_with(|| TimeSeries {
            labels: point.labels.clone(),
            name: point.name.clone(),
            points: VecDeque::new(),
        });

        // Insert in timestamp order (usually appending)
        let stored = StoredPoint {
            timestamp_ns: point.timestamp_ns,
            value: point.value,
        };

        if series
            .points
            .back()
            .is_none_or(|last| last.timestamp_ns <= stored.timestamp_ns)
        {
            series.points.push_back(stored);
        } else {
            // Out-of-order: find insertion point
            let pos = series
                .points
                .iter()
                .position(|p| p.timestamp_ns > stored.timestamp_ns)
                .unwrap_or(series.points.len());
            series.points.insert(pos, stored);
        }

        // Evict oldest if over capacity
        while series.points.len() > self.max_points_per_series {
            series.points.pop_front();
        }
    }

    fn metric_names(&self) -> Vec<String> {
        let mut names: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();
        for series in self.series.values() {
            names.insert(series.name.clone());
        }
        let mut sorted: Vec<String> = names.into_iter().collect();
        sorted.sort();
        sorted
    }

    fn label_names(&self, metric: &str) -> Vec<String> {
        let mut names: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();
        for series in self.series.values() {
            if series.name == metric {
                for key in series.labels.keys() {
                    names.insert(key.clone());
                }
            }
        }
        let mut sorted: Vec<String> = names.into_iter().collect();
        sorted.sort();
        sorted
    }

    fn label_values(&self, metric: &str, label: &str) -> Vec<String> {
        let mut values: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();
        for series in self.series.values() {
            if series.name == metric {
                if let Some(v) = series.labels.get(label) {
                    values.insert(v.clone());
                }
            }
        }
        let mut sorted: Vec<String> = values.into_iter().collect();
        sorted.sort();
        sorted
    }

    fn query(
        &self,
        metric: &str,
        label_filter: &FxHashMap<String, String>,
        start_ns: u64,
        end_ns: u64,
        step_ns: u64,
    ) -> QueryResponse {
        let matching_series: Vec<&TimeSeries> = self
            .series
            .values()
            .filter(|s| {
                if s.name != metric {
                    return false;
                }
                for (k, v) in label_filter {
                    match s.labels.get(k) {
                        Some(sv) if sv == v => {}
                        _ => return false,
                    }
                }
                true
            })
            .collect();

        let step_ns = if step_ns == 0 {
            // Auto-calculate: aim for ~200 data points
            let range = end_ns.saturating_sub(start_ns);
            (range / 200).max(1_000_000_000) // at least 1 second
        } else {
            step_ns
        };

        let groups: Vec<MetricsGroup> = matching_series
            .iter()
            .map(|series| {
                // Build group label string
                let mut pairs: Vec<_> = series.labels.iter().collect();
                pairs.sort_by_key(|(k, _)| *k);
                let group = pairs
                    .iter()
                    .map(|(k, v)| format!("{k}:{v}"))
                    .collect::<Vec<_>>()
                    .join(", ");

                // Bucket the data points
                let buckets = bucket_points(&series.points, start_ns, end_ns, step_ns);

                MetricsGroup {
                    group: if group.is_empty() {
                        metric.to_string()
                    } else {
                        group
                    },
                    buckets,
                }
            })
            .collect();

        QueryResponse {
            metric: metric.to_string(),
            query: metric.to_string(),
            parsed_agg: None,
            parsed_filter: String::new(),
            parsed_grouping: None,
            parsed_time_range: None,
            start: Some(start_ns as Timestamp),
            end: Some(end_ns as Timestamp),
            granularity_ns: step_ns as u128,
            groups,
            result_type: ResultType::Matrix,
        }
    }

    fn series_count(&self) -> usize {
        self.series.len()
    }

    fn point_count(&self) -> usize {
        self.series.values().map(|s| s.points.len()).sum()
    }
}

/// Bucket data points into fixed-width time windows, taking the last value per bucket.
///
/// Single-pass O(N) algorithm: since points are sorted by timestamp, we advance
/// through points once, assigning each to its bucket.
fn bucket_points(
    points: &VecDeque<StoredPoint>,
    start_ns: u64,
    end_ns: u64,
    step_ns: u64,
) -> Vec<MetricsBucket> {
    if points.is_empty() || start_ns >= end_ns || step_ns == 0 {
        return Vec::new();
    }

    let num_buckets = (end_ns - start_ns).div_ceil(step_ns);
    let mut bucket_data: Vec<Option<(f64, usize)>> = vec![None; num_buckets as usize];

    for p in points.iter() {
        if p.timestamp_ns < start_ns || p.timestamp_ns >= end_ns {
            continue;
        }
        let idx = ((p.timestamp_ns - start_ns) / step_ns) as usize;
        if idx < bucket_data.len() {
            match &mut bucket_data[idx] {
                Some((val, count)) => {
                    *val = p.value;
                    *count += 1;
                }
                slot @ None => {
                    *slot = Some((p.value, 1));
                }
            }
        }
    }

    bucket_data
        .into_iter()
        .enumerate()
        .filter_map(|(i, data)| {
            let (value, count) = data?;
            let bucket_start = start_ns + (i as u64) * step_ns;
            let bucket_end = (bucket_start + step_ns).min(end_ns);
            Some(MetricsBucket {
                start: bucket_start as Timestamp,
                end: bucket_end as Timestamp,
                value,
                count,
            })
        })
        .collect()
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
        let contains_lower = contains.map(|t| t.to_lowercase());
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
                if let Some(ref pattern) = contains_lower {
                    if !entry.message.to_lowercase().contains(pattern) {
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
