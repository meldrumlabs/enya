//! Response parsing for Grafana Tempo HTTP API.
//!
//! Tempo returns traces in a JSON format based on the Jaeger model.
//! This module handles parsing those responses into our trace types.

use crate::error::ClientError;
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
        parse_otlp_trace(batches)
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
// OpenTelemetry (OTLP) Format Parsing
// ============================================================================

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OtlpBatch {
    resource: Option<OtlpResource>,
    scope_spans: Option<Vec<OtlpScopeSpans>>,
    /// Legacy field name
    instrumentation_library_spans: Option<Vec<OtlpScopeSpans>>,
}

#[derive(Deserialize)]
struct OtlpResource {
    attributes: Option<Vec<OtlpAttribute>>,
}

#[derive(Deserialize)]
struct OtlpScopeSpans {
    spans: Option<Vec<OtlpSpan>>,
}

#[derive(Deserialize)]
struct OtlpSpan {
    #[serde(alias = "traceId", alias = "trace_id")]
    trace_id: Option<String>,
    #[serde(alias = "spanId", alias = "span_id")]
    span_id: Option<String>,
    #[serde(alias = "parentSpanId", alias = "parent_span_id")]
    parent_span_id: Option<String>,
    name: Option<String>,
    #[serde(alias = "startTimeUnixNano", alias = "start_time_unix_nano")]
    start_time_unix_nano: Option<u64>,
    #[serde(alias = "endTimeUnixNano", alias = "end_time_unix_nano")]
    end_time_unix_nano: Option<u64>,
    status: Option<OtlpStatus>,
    attributes: Option<Vec<OtlpAttribute>>,
    events: Option<Vec<OtlpEvent>>,
}

#[derive(Deserialize)]
struct OtlpStatus {
    code: Option<i32>,
    #[allow(dead_code)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct OtlpAttribute {
    key: String,
    value: Option<OtlpValue>,
}

#[derive(Deserialize)]
struct OtlpValue {
    #[serde(alias = "stringValue", alias = "string_value")]
    string_value: Option<String>,
    #[serde(alias = "intValue", alias = "int_value")]
    int_value: Option<i64>,
    #[serde(alias = "doubleValue", alias = "double_value")]
    double_value: Option<f64>,
    #[serde(alias = "boolValue", alias = "bool_value")]
    bool_value: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OtlpEvent {
    time_unix_nano: Option<u64>,
    name: Option<String>,
    attributes: Option<Vec<OtlpAttribute>>,
}

fn parse_otlp_trace(batches: Vec<OtlpBatch>) -> Result<Trace, ClientError> {
    let mut all_spans = Vec::new();
    let mut trace_id = String::new();

    for batch in batches {
        // Extract service name from resource attributes
        let service_name = batch
            .resource
            .as_ref()
            .and_then(|r| r.attributes.as_ref())
            .and_then(|attrs| {
                attrs
                    .iter()
                    .find(|a| a.key == "service.name")
                    .and_then(|a| a.value.as_ref().and_then(|v| v.string_value.clone()))
            })
            .unwrap_or_else(|| "unknown".to_string());

        // Get spans from either scope_spans or instrumentation_library_spans
        let scope_spans = batch
            .scope_spans
            .or(batch.instrumentation_library_spans)
            .unwrap_or_default();

        for scope in scope_spans {
            for otlp_span in scope.spans.unwrap_or_default() {
                let span_trace_id = otlp_span.trace_id.clone().unwrap_or_default();
                if trace_id.is_empty() {
                    trace_id = span_trace_id.clone();
                }

                let start_time_ns = otlp_span.start_time_unix_nano.unwrap_or(0);
                let end_time_ns = otlp_span.end_time_unix_nano.unwrap_or(0);

                let duration_us = (end_time_ns.saturating_sub(start_time_ns)) / 1000;
                let start_time_us = start_time_ns / 1000;

                // Parse status
                let status = match otlp_span.status.as_ref().and_then(|s| s.code) {
                    Some(2) => SpanStatus::Error,
                    Some(1) => SpanStatus::Ok,
                    _ => SpanStatus::Unset,
                };

                // Parse tags from attributes
                let tags = otlp_span
                    .attributes
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|a| {
                        let value = a.value.and_then(|v| {
                            v.string_value
                                .or(v.int_value.map(|i| i.to_string()))
                                .or(v.double_value.map(|d| d.to_string()))
                                .or(v.bool_value.map(|b| b.to_string()))
                        })?;
                        Some((a.key, value))
                    })
                    .collect();

                // Parse logs from events
                let logs = otlp_span
                    .events
                    .unwrap_or_default()
                    .into_iter()
                    .map(|e| {
                        let mut fields: FxHashMap<String, String> = e
                            .attributes
                            .unwrap_or_default()
                            .into_iter()
                            .filter_map(|a| {
                                let value = a.value.and_then(|v| {
                                    v.string_value.or(v.int_value.map(|i| i.to_string()))
                                })?;
                                Some((a.key, value))
                            })
                            .collect();
                        if let Some(name) = e.name {
                            fields.insert("event".to_string(), name);
                        }
                        SpanLog {
                            timestamp_us: e.time_unix_nano.unwrap_or(0) / 1000,
                            fields,
                        }
                    })
                    .collect();

                let parent_span_id = otlp_span.parent_span_id.filter(|s| !s.is_empty());

                all_spans.push(Span {
                    span_id: otlp_span.span_id.unwrap_or_default(),
                    trace_id: span_trace_id,
                    parent_span_id,
                    operation_name: otlp_span.name.unwrap_or_else(|| "unknown".to_string()),
                    service_name: service_name.clone(),
                    start_time_us,
                    duration_us,
                    status,
                    tags,
                    logs,
                    depth: 0, // Computed in Trace::from_spans
                });
            }
        }
    }

    if all_spans.is_empty() {
        return Err(ClientError::ParseError(
            "No spans found in trace".to_string(),
        ));
    }

    Ok(Trace::from_spans(trace_id, all_spans))
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
