//! Response parsing for Grafana Tempo HTTP API.
//!
//! Tempo returns traces in a JSON format based on the Jaeger model or
//! OpenTelemetry format. OTLP parsing is shared via [`crate::otlp`].

use crate::error::ClientError;
use crate::otlp::types::OtlpBatch;
use rustc_hash::FxHashMap;
use serde::Deserialize;

use super::types::{Span, SpanLog, SpanStatus, Trace, TraceSummary};

/// Parse a trace response from Tempo's `/api/traces/{traceID}` endpoint.
pub fn parse_trace_response(bytes: &[u8]) -> Result<Trace, ClientError> {
    let response: TempoTraceResponse =
        serde_json::from_slice(bytes).map_err(|e| ClientError::ParseError(e.to_string()))?;

    // Tempo wraps the trace in a "batches" array (OpenTelemetry format)
    // or returns Jaeger format with "data" array
    if let Some(batches) = response.batches {
        crate::otlp::parse_otlp_trace(batches)
    } else if let Some(data) = response.data {
        parse_jaeger_trace(data)
    } else {
        Err(ClientError::ParseError(
            "Unknown trace response format".to_string(),
        ))
    }
}

/// Parse a search response from Tempo's `/api/search` endpoint.
pub fn parse_search_response(bytes: &[u8]) -> Result<Vec<TraceSummary>, ClientError> {
    let response: TempoSearchResponse =
        serde_json::from_slice(bytes).map_err(|e| ClientError::ParseError(e.to_string()))?;

    Ok(response
        .traces
        .into_iter()
        .map(|t| TraceSummary {
            trace_id: t.trace_id,
            root_service_name: t.root_service_name.unwrap_or_default(),
            root_operation_name: t.root_trace_name.unwrap_or_default(),
            start_time_us: t.start_time_unix_nano.unwrap_or(0) / 1000,
            duration_us: t.duration_ms.unwrap_or(0) * 1000,
            span_count: t.span_set.as_ref().map(|s| s.spans).unwrap_or(0),
            error_count: 0, // Tempo doesn't include this in search results
        })
        .collect())
}

// ============================================================================
// Tempo Response Types (JSON deserialization)
// ============================================================================

#[derive(Deserialize)]
struct TempoTraceResponse {
    /// OpenTelemetry format
    batches: Option<Vec<OtlpBatch>>,
    /// Jaeger format
    data: Option<Vec<JaegerTrace>>,
}

#[derive(Deserialize)]
struct TempoSearchResponse {
    traces: Vec<TempoSearchTrace>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TempoSearchTrace {
    #[serde(rename = "traceID")]
    trace_id: String,
    root_service_name: Option<String>,
    root_trace_name: Option<String>,
    start_time_unix_nano: Option<u64>,
    duration_ms: Option<u64>,
    span_set: Option<SpanSet>,
}

#[derive(Deserialize)]
struct SpanSet {
    spans: usize,
}

// ============================================================================
// Jaeger Format Parsing
// ============================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JaegerTrace {
    #[serde(rename = "traceID")]
    trace_id: String,
    spans: Vec<JaegerSpan>,
    processes: FxHashMap<String, JaegerProcess>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JaegerSpan {
    #[serde(rename = "spanID")]
    span_id: String,
    #[serde(rename = "traceID")]
    trace_id: String,
    operation_name: String,
    references: Option<Vec<JaegerReference>>,
    start_time: u64, // microseconds
    duration: u64,   // microseconds
    tags: Option<Vec<JaegerTag>>,
    logs: Option<Vec<JaegerLog>>,
    #[serde(rename = "processID")]
    process_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JaegerReference {
    ref_type: String,
    #[serde(rename = "spanID")]
    span_id: String,
}

#[derive(Deserialize)]
struct JaegerTag {
    key: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    tag_type: String,
    value: serde_json::Value,
}

#[derive(Deserialize)]
struct JaegerLog {
    timestamp: u64,
    fields: Vec<JaegerTag>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JaegerProcess {
    service_name: String,
    #[allow(dead_code)]
    tags: Option<Vec<JaegerTag>>,
}

fn parse_jaeger_trace(data: Vec<JaegerTrace>) -> Result<Trace, ClientError> {
    let jaeger_trace = data
        .into_iter()
        .next()
        .ok_or_else(|| ClientError::ParseError("No trace data".to_string()))?;

    let spans: Vec<Span> = jaeger_trace
        .spans
        .into_iter()
        .map(|js| {
            let service_name = jaeger_trace
                .processes
                .get(&js.process_id)
                .map(|p| p.service_name.clone())
                .unwrap_or_else(|| "unknown".to_string());

            // Find parent span ID from references
            let parent_span_id = js
                .references
                .and_then(|refs| {
                    refs.into_iter()
                        .find(|r| r.ref_type == "CHILD_OF")
                        .map(|r| r.span_id)
                })
                .filter(|s| !s.is_empty());

            // Check for error status in tags
            let status = js
                .tags
                .as_ref()
                .and_then(|tags| {
                    tags.iter().find(|t| t.key == "error").and_then(|t| {
                        if t.value.as_bool() == Some(true) {
                            Some(SpanStatus::Error)
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or(SpanStatus::Ok);

            // Parse tags
            let tags = js
                .tags
                .unwrap_or_default()
                .into_iter()
                .map(|t| (t.key, jaeger_value_to_string(&t.value)))
                .collect();

            // Parse logs
            let logs = js
                .logs
                .unwrap_or_default()
                .into_iter()
                .map(|l| SpanLog {
                    timestamp_us: l.timestamp,
                    fields: l
                        .fields
                        .into_iter()
                        .map(|f| (f.key, jaeger_value_to_string(&f.value)))
                        .collect(),
                })
                .collect();

            Span {
                span_id: js.span_id,
                trace_id: js.trace_id,
                parent_span_id,
                operation_name: js.operation_name,
                service_name,
                start_time_us: js.start_time,
                duration_us: js.duration,
                status,
                tags,
                logs,
                depth: 0,
            }
        })
        .collect();

    if spans.is_empty() {
        return Err(ClientError::ParseError(
            "No spans found in trace".to_string(),
        ));
    }

    Ok(Trace::from_spans(jaeger_trace.trace_id, spans))
}

fn jaeger_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_search_response() {
        let json = r#"{
            "traces": [
                {
                    "traceID": "abc123",
                    "rootServiceName": "frontend",
                    "rootTraceName": "HTTP GET /api",
                    "startTimeUnixNano": 1000000000000,
                    "durationMs": 150,
                    "spanSet": {"spans": 5}
                }
            ]
        }"#;

        let summaries = parse_search_response(json.as_bytes()).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].trace_id, "abc123");
        assert_eq!(summaries[0].root_service_name, "frontend");
        assert_eq!(summaries[0].duration_us, 150_000);
    }

    #[test]
    fn test_parse_jaeger_trace() {
        let json = r#"{
            "data": [{
                "traceID": "abc123",
                "spans": [{
                    "spanID": "span1",
                    "traceID": "abc123",
                    "operationName": "root",
                    "startTime": 1000000,
                    "duration": 500000,
                    "processID": "p1",
                    "tags": [],
                    "logs": []
                }],
                "processes": {
                    "p1": {"serviceName": "my-service", "tags": []}
                }
            }]
        }"#;

        let trace = parse_trace_response(json.as_bytes()).unwrap();
        assert_eq!(trace.trace_id, "abc123");
        assert_eq!(trace.spans.len(), 1);
        assert_eq!(trace.spans[0].service_name, "my-service");
    }
}
