//! OTLP ingestion: parse incoming OTLP JSON payloads and write to the store.
//!
//! This module is native-only (`cfg(not(target_arch = "wasm32"))`) since the
//! OTLP receiver runs as part of the agent server process.

use rustc_hash::FxHashMap;

use super::convert_otlp_span;
use super::store::{MetricDataPoint, TelemetryStore};
use super::types::{OtlpLogsData, OtlpMetricsData, OtlpTracesData};
use crate::logs::{LogEntry, LogLevel};

/// Internal label key for storing metric units (filtered from user-facing label lists).
pub const UNIT_LABEL: &str = "__unit__";

/// Errors during OTLP ingestion.
#[derive(Debug)]
pub enum IngestError {
    /// JSON parse error.
    Parse(String),
    /// Empty payload (no data to ingest).
    Empty,
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "OTLP parse error: {e}"),
            Self::Empty => write!(f, "empty OTLP payload"),
        }
    }
}

impl std::error::Error for IngestError {}

/// Parse an OTLP traces export request and insert traces into the store.
///
/// Returns the number of spans ingested.
pub fn ingest_traces(store: &TelemetryStore, body: &[u8]) -> Result<usize, IngestError> {
    let data: OtlpTracesData =
        serde_json::from_slice(body).map_err(|e| IngestError::Parse(e.to_string()))?;

    if data.resource_spans.is_empty() {
        return Err(IngestError::Empty);
    }

    // Group spans by trace_id so we can assemble complete traces
    let mut spans_by_trace: FxHashMap<String, Vec<crate::tracing::tempo::types::Span>> =
        FxHashMap::default();
    let mut total_spans = 0;

    for batch in &data.resource_spans {
        let service_name = batch
            .resource
            .as_ref()
            .and_then(|r| r.service_name())
            .unwrap_or_else(|| "unknown".to_string());

        let scope_spans = batch
            .scope_spans
            .as_ref()
            .or(batch.instrumentation_library_spans.as_ref());

        if let Some(scopes) = scope_spans {
            for scope in scopes {
                for otlp_span in scope.spans.as_deref().unwrap_or_default() {
                    let trace_id = otlp_span.trace_id.clone().unwrap_or_default();
                    if trace_id.is_empty() {
                        continue;
                    }

                    let span = convert_otlp_span(otlp_span, &service_name);
                    spans_by_trace.entry(trace_id).or_default().push(span);
                    total_spans += 1;
                }
            }
        }
    }

    // Assemble and insert traces
    for (trace_id, spans) in spans_by_trace {
        let trace = crate::tracing::tempo::types::Trace::from_spans(trace_id, spans);
        store.insert_trace(trace);
    }

    Ok(total_spans)
}

/// Parse an OTLP logs export request and insert log entries into the store.
///
/// Returns the number of log entries ingested.
pub fn ingest_logs(store: &TelemetryStore, body: &[u8]) -> Result<usize, IngestError> {
    let data: OtlpLogsData =
        serde_json::from_slice(body).map_err(|e| IngestError::Parse(e.to_string()))?;

    if data.resource_logs.is_empty() {
        return Err(IngestError::Empty);
    }

    let mut entries = Vec::new();

    for resource_log in &data.resource_logs {
        let service_name = resource_log
            .resource
            .as_ref()
            .and_then(|r| r.service_name())
            .unwrap_or_else(|| "unknown".to_string());

        // Collect resource-level labels
        let mut base_labels: FxHashMap<String, String> = FxHashMap::default();
        base_labels.insert("service".to_string(), service_name);

        if let Some(scopes) = &resource_log.scope_logs {
            for scope in scopes {
                for record in scope.log_records.as_deref().unwrap_or_default() {
                    let timestamp_ns = record
                        .time_unix_nano
                        .or(record.observed_time_unix_nano)
                        .unwrap_or(0) as i64;

                    // Build message from body
                    let message = record
                        .body
                        .as_ref()
                        .map(|b| b.to_string_lossy())
                        .unwrap_or_default();

                    // Parse severity
                    let level = severity_to_log_level(record.severity_number)
                        .or_else(|| record.severity_text.as_deref().and_then(LogLevel::parse));

                    // Build labels from resource + record attributes
                    let mut labels = base_labels.clone();
                    if let Some(attrs) = &record.attributes {
                        for attr in attrs {
                            if let Some(value) = attr.value_as_string() {
                                labels.insert(attr.key.clone(), value);
                            }
                        }
                    }
                    if let Some(trace_id) = &record.trace_id {
                        if !trace_id.is_empty() {
                            labels.insert("trace_id".to_string(), trace_id.clone());
                        }
                    }
                    if let Some(span_id) = &record.span_id {
                        if !span_id.is_empty() {
                            labels.insert("span_id".to_string(), span_id.clone());
                        }
                    }

                    entries.push(LogEntry {
                        timestamp_ns,
                        message,
                        labels,
                        level,
                    });
                }
            }
        }
    }

    let count = entries.len();
    if count == 0 {
        return Err(IngestError::Empty);
    }

    store.insert_logs(entries);
    Ok(count)
}

/// Parse an OTLP metrics export request and insert data points into the store.
///
/// Supports Gauge, Sum, and Histogram (as sum/count values) metrics.
/// Returns the number of data points ingested.
pub fn ingest_metrics(store: &TelemetryStore, body: &[u8]) -> Result<usize, IngestError> {
    let data: OtlpMetricsData =
        serde_json::from_slice(body).map_err(|e| IngestError::Parse(e.to_string()))?;

    if data.resource_metrics.is_empty() {
        return Err(IngestError::Empty);
    }

    let mut points = Vec::new();

    for resource_metrics in &data.resource_metrics {
        let service_name = resource_metrics
            .resource
            .as_ref()
            .and_then(|r| r.service_name())
            .unwrap_or_else(|| "unknown".to_string());

        let scopes = resource_metrics
            .scope_metrics
            .as_deref()
            .unwrap_or_default();

        for scope in scopes {
            let metrics = scope.metrics.as_deref().unwrap_or_default();

            for metric in metrics {
                let name = metric.name.clone().unwrap_or_else(|| "unknown".to_string());

                // Gauge data points
                if let Some(ref gauge) = metric.gauge {
                    for dp in gauge.data_points.as_deref().unwrap_or_default() {
                        let mut labels = extract_labels(dp.attributes.as_ref());
                        labels.insert("service".to_string(), service_name.clone());
                        if let Some(ref unit) = metric.unit {
                            if !unit.is_empty() {
                                labels.insert(UNIT_LABEL.to_string(), unit.clone());
                            }
                        }
                        points.push(MetricDataPoint {
                            name: name.clone(),
                            labels,
                            timestamp_ns: dp.time_unix_nano.unwrap_or(0),
                            value: dp.value(),
                        });
                    }
                }

                // Sum (counter) data points
                if let Some(ref sum) = metric.sum {
                    for dp in sum.data_points.as_deref().unwrap_or_default() {
                        let mut labels = extract_labels(dp.attributes.as_ref());
                        labels.insert("service".to_string(), service_name.clone());
                        if let Some(ref unit) = metric.unit {
                            if !unit.is_empty() {
                                labels.insert(UNIT_LABEL.to_string(), unit.clone());
                            }
                        }
                        points.push(MetricDataPoint {
                            name: name.clone(),
                            labels,
                            timestamp_ns: dp.time_unix_nano.unwrap_or(0),
                            value: dp.value(),
                        });
                    }
                }

                // Histogram: expose _sum and _count as separate metrics
                if let Some(ref histogram) = metric.histogram {
                    for dp in histogram.data_points.as_deref().unwrap_or_default() {
                        let mut base_labels = extract_labels(dp.attributes.as_ref());
                        base_labels.insert("service".to_string(), service_name.clone());
                        if let Some(ref unit) = metric.unit {
                            if !unit.is_empty() {
                                base_labels.insert(UNIT_LABEL.to_string(), unit.clone());
                            }
                        }
                        let ts = dp.time_unix_nano.unwrap_or(0);

                        if let Some(sum) = dp.sum {
                            points.push(MetricDataPoint {
                                name: format!("{name}_sum"),
                                labels: base_labels.clone(),
                                timestamp_ns: ts,
                                value: sum,
                            });
                        }
                        if let Some(count) = dp.count {
                            points.push(MetricDataPoint {
                                name: format!("{name}_count"),
                                labels: base_labels,
                                timestamp_ns: ts,
                                value: count as f64,
                            });
                        }
                    }
                }
            }
        }
    }

    let count = points.len();
    if count == 0 {
        return Err(IngestError::Empty);
    }

    store.insert_metric_points(points);
    Ok(count)
}

// ============================================================================
// Protobuf ingestion (prost)
// ============================================================================

/// Parse an OTLP protobuf traces export request and insert traces into the store.
///
/// Returns the number of spans ingested.
pub fn ingest_traces_proto(store: &TelemetryStore, body: &[u8]) -> Result<usize, IngestError> {
    use super::proto;
    use prost::Message;

    let data = proto::ExportTraceServiceRequest::decode(body)
        .map_err(|e| IngestError::Parse(e.to_string()))?;

    if data.resource_spans.is_empty() {
        return Err(IngestError::Empty);
    }

    let mut spans_by_trace: FxHashMap<String, Vec<crate::tracing::tempo::types::Span>> =
        FxHashMap::default();
    let mut total_spans = 0;

    for rs in &data.resource_spans {
        let service_name = proto::resource_service_name(&rs.resource);

        for scope in &rs.scope_spans {
            for span in &scope.spans {
                let trace_id = proto::bytes_to_hex(&span.trace_id);
                if trace_id.is_empty() || span.trace_id.is_empty() {
                    continue;
                }

                let span_id = proto::bytes_to_hex(&span.span_id);
                let parent_span_id = if span.parent_span_id.is_empty() {
                    None
                } else {
                    Some(proto::bytes_to_hex(&span.parent_span_id))
                };

                let status = match span.status.as_ref().map(|s| s.code) {
                    Some(2) => crate::tracing::tempo::types::SpanStatus::Error,
                    Some(1) => crate::tracing::tempo::types::SpanStatus::Ok,
                    _ => crate::tracing::tempo::types::SpanStatus::Unset,
                };

                let tags = proto::attrs_to_map(&span.attributes);

                let logs: Vec<crate::tracing::tempo::types::SpanLog> = span
                    .events
                    .iter()
                    .map(|event| {
                        let mut fields = proto::attrs_to_map(&event.attributes);
                        if !event.name.is_empty() {
                            fields.insert("event".to_string(), event.name.clone());
                        }
                        crate::tracing::tempo::types::SpanLog {
                            timestamp_us: event.time_unix_nano / 1000,
                            fields,
                        }
                    })
                    .collect();

                let domain_span = crate::tracing::tempo::types::Span {
                    span_id,
                    trace_id: trace_id.clone(),
                    parent_span_id,
                    operation_name: if span.name.is_empty() {
                        "unknown".to_string()
                    } else {
                        span.name.clone()
                    },
                    service_name: service_name.clone(),
                    start_time_us: span.start_time_unix_nano / 1000,
                    duration_us: span
                        .end_time_unix_nano
                        .saturating_sub(span.start_time_unix_nano)
                        / 1000,
                    status,
                    tags,
                    logs,
                    depth: 0,
                };

                spans_by_trace
                    .entry(trace_id)
                    .or_default()
                    .push(domain_span);
                total_spans += 1;
            }
        }
    }

    for (trace_id, spans) in spans_by_trace {
        let trace = crate::tracing::tempo::types::Trace::from_spans(trace_id, spans);
        store.insert_trace(trace);
    }

    Ok(total_spans)
}

/// Parse an OTLP protobuf logs export request and insert log entries into the store.
///
/// Returns the number of log entries ingested.
pub fn ingest_logs_proto(store: &TelemetryStore, body: &[u8]) -> Result<usize, IngestError> {
    use super::proto;
    use prost::Message;

    let data = proto::ExportLogsServiceRequest::decode(body)
        .map_err(|e| IngestError::Parse(e.to_string()))?;

    if data.resource_logs.is_empty() {
        return Err(IngestError::Empty);
    }

    let mut entries = Vec::new();

    for rl in &data.resource_logs {
        let service_name = proto::resource_service_name(&rl.resource);

        let mut base_labels: FxHashMap<String, String> = FxHashMap::default();
        base_labels.insert("service".to_string(), service_name);

        for scope in &rl.scope_logs {
            for record in &scope.log_records {
                let timestamp_ns = if record.time_unix_nano != 0 {
                    record.time_unix_nano as i64
                } else {
                    record.observed_time_unix_nano as i64
                };

                let message = proto::any_value_to_string(&record.body);

                let level = severity_to_log_level(Some(record.severity_number)).or_else(|| {
                    if record.severity_text.is_empty() {
                        None
                    } else {
                        LogLevel::parse(&record.severity_text)
                    }
                });

                let mut labels = base_labels.clone();
                for kv in &record.attributes {
                    if let Some(v) = &kv.value {
                        let val = proto::any_value_ref_to_string(v);
                        if !val.is_empty() {
                            labels.insert(kv.key.clone(), val);
                        }
                    }
                }
                if !record.trace_id.is_empty() {
                    labels.insert(
                        "trace_id".to_string(),
                        proto::bytes_to_hex(&record.trace_id),
                    );
                }
                if !record.span_id.is_empty() {
                    labels.insert("span_id".to_string(), proto::bytes_to_hex(&record.span_id));
                }

                entries.push(LogEntry {
                    timestamp_ns,
                    message,
                    labels,
                    level,
                });
            }
        }
    }

    let count = entries.len();
    if count == 0 {
        return Err(IngestError::Empty);
    }

    store.insert_logs(entries);
    Ok(count)
}

/// Parse an OTLP protobuf metrics export request and insert data points into the store.
///
/// Returns the number of data points ingested.
pub fn ingest_metrics_proto(store: &TelemetryStore, body: &[u8]) -> Result<usize, IngestError> {
    use super::proto;
    use prost::Message;

    let data = proto::ExportMetricsServiceRequest::decode(body)
        .map_err(|e| IngestError::Parse(e.to_string()))?;

    if data.resource_metrics.is_empty() {
        return Err(IngestError::Empty);
    }

    let mut points = Vec::new();

    for rm in &data.resource_metrics {
        let service_name = proto::resource_service_name(&rm.resource);

        for scope in &rm.scope_metrics {
            for metric in &scope.metrics {
                let name = if metric.name.is_empty() {
                    "unknown".to_string()
                } else {
                    metric.name.clone()
                };

                match &metric.data {
                    Some(proto::proto_metric::Data::Gauge(gauge)) => {
                        for dp in &gauge.data_points {
                            let mut labels = proto::attrs_to_map(&dp.attributes);
                            labels.insert("service".to_string(), service_name.clone());
                            if !metric.unit.is_empty() {
                                labels.insert(UNIT_LABEL.to_string(), metric.unit.clone());
                            }
                            points.push(MetricDataPoint {
                                name: name.clone(),
                                labels,
                                timestamp_ns: dp.time_unix_nano,
                                value: proto::number_data_point_value(dp),
                            });
                        }
                    }
                    Some(proto::proto_metric::Data::Sum(sum)) => {
                        for dp in &sum.data_points {
                            let mut labels = proto::attrs_to_map(&dp.attributes);
                            labels.insert("service".to_string(), service_name.clone());
                            if !metric.unit.is_empty() {
                                labels.insert(UNIT_LABEL.to_string(), metric.unit.clone());
                            }
                            points.push(MetricDataPoint {
                                name: name.clone(),
                                labels,
                                timestamp_ns: dp.time_unix_nano,
                                value: proto::number_data_point_value(dp),
                            });
                        }
                    }
                    Some(proto::proto_metric::Data::Histogram(histogram)) => {
                        for dp in &histogram.data_points {
                            let mut base_labels = proto::attrs_to_map(&dp.attributes);
                            base_labels.insert("service".to_string(), service_name.clone());
                            if !metric.unit.is_empty() {
                                base_labels.insert(UNIT_LABEL.to_string(), metric.unit.clone());
                            }
                            let ts = dp.time_unix_nano;

                            if let Some(sum) = dp.sum {
                                points.push(MetricDataPoint {
                                    name: format!("{name}_sum"),
                                    labels: base_labels.clone(),
                                    timestamp_ns: ts,
                                    value: sum,
                                });
                            }
                            points.push(MetricDataPoint {
                                name: format!("{name}_count"),
                                labels: base_labels,
                                timestamp_ns: ts,
                                value: dp.count as f64,
                            });
                        }
                    }
                    None => {}
                }
            }
        }
    }

    let count = points.len();
    if count == 0 {
        return Err(IngestError::Empty);
    }

    store.insert_metric_points(points);
    Ok(count)
}

/// Extract labels from OTLP attributes.
fn extract_labels(
    attributes: Option<&Vec<super::types::OtlpAttribute>>,
) -> FxHashMap<String, String> {
    attributes
        .map(|v| v.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|a| Some((a.key.clone(), a.value_as_string()?)))
        .collect()
}

/// Map OTLP severity_number to LogLevel.
///
/// See <https://opentelemetry.io/docs/specs/otel/logs/data-model/#severity-fields>
fn severity_to_log_level(severity_number: Option<i32>) -> Option<LogLevel> {
    match severity_number? {
        1..=4 => Some(LogLevel::Trace),
        5..=8 => Some(LogLevel::Debug),
        9..=12 => Some(LogLevel::Info),
        13..=16 => Some(LogLevel::Warn),
        17..=24 => Some(LogLevel::Error),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::otlp::store::StoreConfig;

    #[test]
    fn test_ingest_metrics_gauge() {
        let store = TelemetryStore::new(StoreConfig::default());

        let json = r#"{
            "resourceMetrics": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "my-app"}}
                    ]
                },
                "scopeMetrics": [{
                    "metrics": [{
                        "name": "cpu_usage",
                        "unit": "percent",
                        "gauge": {
                            "dataPoints": [{
                                "timeUnixNano": 1000000000,
                                "asDouble": 42.5,
                                "attributes": [
                                    {"key": "host", "value": {"stringValue": "server-1"}}
                                ]
                            }]
                        }
                    }]
                }]
            }]
        }"#;

        let count = ingest_metrics(&store, json.as_bytes()).unwrap();
        assert_eq!(count, 1);
        assert_eq!(store.metric_series_count(), 1);

        let names = store.metric_names();
        assert_eq!(names, vec!["cpu_usage"]);

        let label_names = store.metric_label_names("cpu_usage");
        assert!(label_names.contains(&"host".to_string()));
        assert!(label_names.contains(&"service".to_string()));
    }

    #[test]
    fn test_ingest_metrics_sum() {
        let store = TelemetryStore::new(StoreConfig::default());

        let json = r#"{
            "resourceMetrics": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "api"}}
                    ]
                },
                "scopeMetrics": [{
                    "metrics": [{
                        "name": "http_requests_total",
                        "sum": {
                            "isMonotonic": true,
                            "dataPoints": [{
                                "timeUnixNano": 1000000000,
                                "asInt": 100,
                                "attributes": [
                                    {"key": "method", "value": {"stringValue": "GET"}}
                                ]
                            }, {
                                "timeUnixNano": 2000000000,
                                "asInt": 150,
                                "attributes": [
                                    {"key": "method", "value": {"stringValue": "GET"}}
                                ]
                            }]
                        }
                    }]
                }]
            }]
        }"#;

        let count = ingest_metrics(&store, json.as_bytes()).unwrap();
        assert_eq!(count, 2);
        assert_eq!(store.metric_point_count(), 2);
    }

    #[test]
    fn test_ingest_metrics_histogram() {
        let store = TelemetryStore::new(StoreConfig::default());

        let json = r#"{
            "resourceMetrics": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "api"}}
                    ]
                },
                "scopeMetrics": [{
                    "metrics": [{
                        "name": "http_request_duration",
                        "histogram": {
                            "dataPoints": [{
                                "timeUnixNano": 1000000000,
                                "count": 42,
                                "sum": 123.456,
                                "attributes": []
                            }]
                        }
                    }]
                }]
            }]
        }"#;

        let count = ingest_metrics(&store, json.as_bytes()).unwrap();
        assert_eq!(count, 2); // _sum and _count

        let names = store.metric_names();
        assert!(names.contains(&"http_request_duration_sum".to_string()));
        assert!(names.contains(&"http_request_duration_count".to_string()));
    }

    #[test]
    fn test_ingest_metrics_empty() {
        let store = TelemetryStore::new(StoreConfig::default());
        let json = r#"{"resourceMetrics": []}"#;
        let result = ingest_metrics(&store, json.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn test_ingest_traces() {
        let store = TelemetryStore::new(StoreConfig::default());

        let json = r#"{
            "resourceSpans": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "my-service"}}
                    ]
                },
                "scopeSpans": [{
                    "spans": [{
                        "traceId": "abc123",
                        "spanId": "span1",
                        "name": "HTTP GET /api",
                        "startTimeUnixNano": 1000000000,
                        "endTimeUnixNano": 1500000000,
                        "status": {"code": 1}
                    }]
                }]
            }]
        }"#;

        let count = ingest_traces(&store, json.as_bytes()).unwrap();
        assert_eq!(count, 1);
        assert_eq!(store.trace_count(), 1);

        let trace = store.get_trace("abc123").unwrap();
        assert_eq!(trace.spans.len(), 1);
        assert_eq!(trace.spans[0].service_name, "my-service");
        assert_eq!(trace.spans[0].operation_name, "HTTP GET /api");
    }

    #[test]
    fn test_ingest_logs() {
        let store = TelemetryStore::new(StoreConfig::default());

        let json = r#"{
            "resourceLogs": [{
                "resource": {
                    "attributes": [
                        {"key": "service.name", "value": {"stringValue": "api-server"}}
                    ]
                },
                "scopeLogs": [{
                    "logRecords": [{
                        "timeUnixNano": 1000000000,
                        "severityNumber": 9,
                        "severityText": "INFO",
                        "body": {"stringValue": "Request handled successfully"},
                        "attributes": [
                            {"key": "http.method", "value": {"stringValue": "GET"}}
                        ]
                    }]
                }]
            }]
        }"#;

        let count = ingest_logs(&store, json.as_bytes()).unwrap();
        assert_eq!(count, 1);
        assert_eq!(store.log_count(), 1);

        let logs = store.query_logs(0, i64::MAX, &FxHashMap::default(), None, 100);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "Request handled successfully");
        assert_eq!(logs[0].level, Some(LogLevel::Info));
        assert_eq!(logs[0].labels.get("service").unwrap(), "api-server");
        assert_eq!(logs[0].labels.get("http.method").unwrap(), "GET");
    }
}
