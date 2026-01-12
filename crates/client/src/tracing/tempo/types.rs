//! Types for representing distributed traces from Grafana Tempo.

use rustc_hash::{FxHashMap, FxHashSet};

/// Unique identifier for a trace (typically 32-character hex string).
pub type TraceId = String;

/// Unique identifier for a span within a trace.
pub type SpanId = String;

/// A complete distributed trace containing multiple spans.
#[derive(Debug, Clone)]
pub struct Trace {
    /// The unique trace identifier.
    pub trace_id: TraceId,
    /// The root span ID (first span with no parent).
    pub root_span_id: Option<SpanId>,
    /// All spans in this trace.
    pub spans: Vec<Span>,
    /// Total duration in microseconds (computed from spans).
    pub duration_us: u64,
    /// Start time as Unix timestamp in microseconds.
    pub start_time_us: u64,
    /// Unique service names involved in this trace.
    pub services: Vec<String>,
}

impl Trace {
    /// Create a new trace from a list of spans, computing derived fields.
    pub fn from_spans(trace_id: TraceId, mut spans: Vec<Span>) -> Self {
        // Sort spans by start time
        spans.sort_by_key(|s| s.start_time_us);

        // Compute derived fields
        let start_time_us = spans.first().map(|s| s.start_time_us).unwrap_or(0);
        let end_time_us = spans
            .iter()
            .map(|s| s.start_time_us + s.duration_us)
            .max()
            .unwrap_or(0);
        let duration_us = end_time_us.saturating_sub(start_time_us);

        // Find root span (no parent)
        let root_span_id = spans
            .iter()
            .find(|s| s.parent_span_id.is_none())
            .map(|s| s.span_id.clone());

        // Collect unique services
        let mut services: Vec<String> = spans
            .iter()
            .map(|s| s.service_name.clone())
            .collect::<FxHashSet<_>>()
            .into_iter()
            .collect();
        services.sort();

        // Compute depth for each span
        let span_depths = compute_span_depths(&spans);
        for span in &mut spans {
            span.depth = span_depths.get(&span.span_id).copied().unwrap_or(0);
        }

        Self {
            trace_id,
            root_span_id,
            spans,
            duration_us,
            start_time_us,
            services,
        }
    }

    /// Get a span by its ID.
    pub fn get_span(&self, span_id: &str) -> Option<&Span> {
        self.spans.iter().find(|s| s.span_id == span_id)
    }

    /// Get the service index for consistent coloring.
    pub fn service_index(&self, service_name: &str) -> usize {
        self.services
            .iter()
            .position(|s| s == service_name)
            .unwrap_or(0)
    }
}

/// Compute the depth of each span in the tree.
fn compute_span_depths(spans: &[Span]) -> FxHashMap<String, usize> {
    let mut depths: FxHashMap<String, usize> = FxHashMap::default();
    let parent_map: FxHashMap<&str, &str> = spans
        .iter()
        .filter_map(|s| {
            s.parent_span_id
                .as_ref()
                .map(|p| (s.span_id.as_str(), p.as_str()))
        })
        .collect();

    for span in spans {
        let mut depth = 0;
        let mut current_id = span.span_id.as_str();
        while let Some(&parent_id) = parent_map.get(current_id) {
            depth += 1;
            current_id = parent_id;
            // Prevent infinite loops
            if depth > 100 {
                break;
            }
        }
        depths.insert(span.span_id.clone(), depth);
    }

    depths
}

/// A single span within a trace.
#[derive(Debug, Clone)]
pub struct Span {
    /// Unique span identifier.
    pub span_id: SpanId,
    /// The trace this span belongs to.
    pub trace_id: TraceId,
    /// Parent span ID (None for root span).
    pub parent_span_id: Option<SpanId>,
    /// The operation name (e.g., "HTTP GET /api/users").
    pub operation_name: String,
    /// The service that produced this span.
    pub service_name: String,
    /// Start time as Unix timestamp in microseconds.
    pub start_time_us: u64,
    /// Duration in microseconds.
    pub duration_us: u64,
    /// Span status.
    pub status: SpanStatus,
    /// Key-value tags/attributes.
    pub tags: FxHashMap<String, String>,
    /// Log events within the span.
    pub logs: Vec<SpanLog>,
    /// Computed depth in the span tree (0 = root).
    pub depth: usize,
}

impl Span {
    /// Format the duration for display.
    pub fn format_duration(&self) -> String {
        format_duration_us(self.duration_us)
    }
}

/// Format a duration in microseconds for display.
pub fn format_duration_us(us: u64) -> String {
    if us < 1_000 {
        format!("{us}us")
    } else if us < 1_000_000 {
        format!("{:.2}ms", us as f64 / 1_000.0)
    } else {
        format!("{:.2}s", us as f64 / 1_000_000.0)
    }
}

/// Status of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpanStatus {
    /// The operation completed successfully.
    #[default]
    Ok,
    /// The operation failed with an error.
    Error,
    /// Status was not set.
    Unset,
}

/// A log event within a span.
#[derive(Debug, Clone)]
pub struct SpanLog {
    /// Timestamp as Unix timestamp in microseconds.
    pub timestamp_us: u64,
    /// Key-value fields for this log entry.
    pub fields: FxHashMap<String, String>,
}

/// Summary of a trace for search results.
#[derive(Debug, Clone)]
pub struct TraceSummary {
    /// The trace identifier.
    pub trace_id: TraceId,
    /// The root service name.
    pub root_service_name: String,
    /// The root operation name.
    pub root_operation_name: String,
    /// Start time as Unix timestamp in microseconds.
    pub start_time_us: u64,
    /// Total duration in microseconds.
    pub duration_us: u64,
    /// Number of spans in the trace.
    pub span_count: usize,
    /// Number of error spans.
    pub error_count: usize,
}

/// Parameters for searching traces.
#[derive(Debug, Clone, Default)]
pub struct TraceSearchParams {
    /// Filter by service name.
    pub service_name: Option<String>,
    /// Filter by operation name.
    pub operation_name: Option<String>,
    /// Filter by tags.
    pub tags: FxHashMap<String, String>,
    /// Minimum duration in milliseconds.
    pub min_duration_ms: Option<u64>,
    /// Maximum duration in milliseconds.
    pub max_duration_ms: Option<u64>,
    /// Maximum number of traces to return.
    pub limit: Option<usize>,
    /// Start of time range (Unix timestamp in seconds).
    pub start_time_secs: Option<u64>,
    /// End of time range (Unix timestamp in seconds).
    pub end_time_secs: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration_us(500), "500us");
        assert_eq!(format_duration_us(1500), "1.50ms");
        assert_eq!(format_duration_us(1_500_000), "1.50s");
    }

    #[test]
    fn test_trace_from_spans() {
        let spans = vec![
            Span {
                span_id: "span1".to_string(),
                trace_id: "trace1".to_string(),
                parent_span_id: None,
                operation_name: "root".to_string(),
                service_name: "service-a".to_string(),
                start_time_us: 1000,
                duration_us: 500,
                status: SpanStatus::Ok,
                tags: FxHashMap::default(),
                logs: vec![],
                depth: 0,
            },
            Span {
                span_id: "span2".to_string(),
                trace_id: "trace1".to_string(),
                parent_span_id: Some("span1".to_string()),
                operation_name: "child".to_string(),
                service_name: "service-b".to_string(),
                start_time_us: 1100,
                duration_us: 200,
                status: SpanStatus::Ok,
                tags: FxHashMap::default(),
                logs: vec![],
                depth: 0,
            },
        ];

        let trace = Trace::from_spans("trace1".to_string(), spans);

        assert_eq!(trace.trace_id, "trace1");
        assert_eq!(trace.root_span_id, Some("span1".to_string()));
        assert_eq!(trace.start_time_us, 1000);
        assert_eq!(trace.duration_us, 500); // max(1000+500, 1100+200) - 1000 = 1500 - 1000 = 500
        assert_eq!(trace.services.len(), 2);

        // Check depths
        assert_eq!(trace.spans[0].depth, 0); // root
        assert_eq!(trace.spans[1].depth, 1); // child
    }
}
