//! OTLP ingestion: parse incoming OTLP JSON payloads and write to the store.
//!
//! This module is native-only (`cfg(not(target_arch = "wasm32"))`) since the
//! OTLP receiver runs as part of the agent server process.

use rustc_hash::FxHashMap;

use super::convert_otlp_span;
use super::store::TelemetryStore;
use super::types::{OtlpLogsData, OtlpTracesData};
use crate::logs::{LogEntry, LogLevel};

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
                for otlp_span in scope.spans.as_ref().unwrap_or(&Vec::new()) {
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
                for record in scope.log_records.as_ref().unwrap_or(&Vec::new()) {
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
