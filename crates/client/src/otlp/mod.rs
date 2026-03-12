//! OpenTelemetry Protocol (OTLP) support.
//!
//! This module provides:
//! - Shared OTLP JSON serde types (used by Tempo and direct OTLP ingestion)
//! - In-memory telemetry store for received OTLP data
//! - Client trait implementations that read from the store
//! - OTLP ingestion functions for parsing incoming payloads

pub mod types;

mod logs_client;
mod metrics_client;
mod store;
mod tracing_client;

// HTTP-based clients that query the agent daemon over the network
mod http_logs_client;
mod http_metrics_client;
mod http_tracing_client;

// Native-only: OTLP ingestion functions that write to the store
#[cfg(not(target_arch = "wasm32"))]
pub mod ingest;

// Native-only: prost-based protobuf message definitions for OTLP decoding
#[cfg(not(target_arch = "wasm32"))]
pub mod proto;

pub use http_logs_client::OtlpHttpLogsClient;
pub use http_metrics_client::OtlpHttpMetricsClient;
pub use http_tracing_client::OtlpHttpTracingClient;
pub use logs_client::OtlpLogsClient;
pub use metrics_client::OtlpMetricsClient;
pub use store::{MetricDataPoint, StoreConfig, TelemetryStore};
pub use tracing_client::OtlpTracingClient;

use crate::error::ClientError;
use crate::tracing::tempo::types::{Span, SpanLog, SpanStatus, Trace};
use rustc_hash::FxHashMap;
use types::{OtlpBatch, OtlpEvent, OtlpSpan};

/// Parse OTLP trace batches into a Trace.
///
/// Shared logic used by both Tempo response parsing and direct OTLP ingestion.
pub fn parse_otlp_trace(batches: Vec<OtlpBatch>) -> Result<Trace, ClientError> {
    let mut all_spans = Vec::new();
    let mut trace_id = String::new();

    for batch in batches {
        let service_name = batch
            .resource
            .as_ref()
            .and_then(|r| r.service_name())
            .unwrap_or_else(|| "unknown".to_string());

        // Get spans from either scope_spans or instrumentation_library_spans
        let scope_spans = batch
            .scope_spans
            .or(batch.instrumentation_library_spans)
            .unwrap_or_default();

        for scope in scope_spans {
            for otlp_span in scope.spans.as_ref().unwrap_or(&Vec::new()) {
                let span = convert_otlp_span(otlp_span, &service_name);
                if trace_id.is_empty() {
                    trace_id.clone_from(&span.trace_id);
                }
                all_spans.push(span);
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

/// Convert a single OTLP span into our domain `Span` type.
///
/// Shared by both `parse_otlp_trace` (Tempo responses) and `ingest::ingest_traces`
/// (direct OTLP push ingestion).
pub fn convert_otlp_span(otlp_span: &OtlpSpan, service_name: &str) -> Span {
    let start_time_ns = otlp_span.start_time_unix_nano.unwrap_or(0);
    let end_time_ns = otlp_span.end_time_unix_nano.unwrap_or(0);

    let status = match otlp_span.status.as_ref().and_then(|s| s.code) {
        Some(2) => SpanStatus::Error,
        Some(1) => SpanStatus::Ok,
        _ => SpanStatus::Unset,
    };

    let tags = otlp_span
        .attributes
        .as_ref()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|a| Some((a.key.clone(), a.value_as_string()?)))
        .collect();

    let logs = otlp_span
        .events
        .as_ref()
        .unwrap_or(&Vec::new())
        .iter()
        .map(convert_otlp_event)
        .collect();

    let parent_span_id = otlp_span.parent_span_id.clone().filter(|s| !s.is_empty());

    Span {
        span_id: otlp_span.span_id.clone().unwrap_or_default(),
        trace_id: otlp_span.trace_id.clone().unwrap_or_default(),
        parent_span_id,
        operation_name: otlp_span
            .name
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        service_name: service_name.to_string(),
        start_time_us: start_time_ns / 1000,
        duration_us: end_time_ns.saturating_sub(start_time_ns) / 1000,
        status,
        tags,
        logs,
        depth: 0,
    }
}

/// Convert an OTLP event into a SpanLog.
fn convert_otlp_event(event: &OtlpEvent) -> SpanLog {
    let mut fields: FxHashMap<String, String> = event
        .attributes
        .as_ref()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|a| Some((a.key.clone(), a.value_as_string()?)))
        .collect();
    if let Some(name) = &event.name {
        fields.insert("event".to_string(), name.clone());
    }
    SpanLog {
        timestamp_us: event.time_unix_nano.unwrap_or(0) / 1000,
        fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{OtlpAttribute, OtlpResource, OtlpScopeSpans, OtlpSpan, OtlpStatus, OtlpValue};

    fn make_batch(service: &str, spans: Vec<OtlpSpan>) -> OtlpBatch {
        OtlpBatch {
            resource: Some(OtlpResource {
                attributes: Some(vec![OtlpAttribute {
                    key: "service.name".to_string(),
                    value: Some(OtlpValue {
                        string_value: Some(service.to_string()),
                        int_value: None,
                        double_value: None,
                        bool_value: None,
                    }),
                }]),
            }),
            scope_spans: Some(vec![OtlpScopeSpans { spans: Some(spans) }]),
            instrumentation_library_spans: None,
        }
    }

    fn make_otlp_span(
        trace_id: &str,
        span_id: &str,
        parent: Option<&str>,
        name: &str,
        start_ns: u64,
        end_ns: u64,
    ) -> OtlpSpan {
        OtlpSpan {
            trace_id: Some(trace_id.to_string()),
            span_id: Some(span_id.to_string()),
            parent_span_id: parent.map(|s| s.to_string()),
            name: Some(name.to_string()),
            start_time_unix_nano: Some(start_ns),
            end_time_unix_nano: Some(end_ns),
            status: None,
            attributes: None,
            events: None,
        }
    }

    #[test]
    fn test_parse_single_span_trace() {
        let batches = vec![make_batch(
            "my-api",
            vec![make_otlp_span(
                "t1", "s1", None, "GET /", 1_000_000, 2_000_000,
            )],
        )];

        let trace = parse_otlp_trace(batches).unwrap();
        assert_eq!(trace.trace_id, "t1");
        assert_eq!(trace.spans.len(), 1);
        assert_eq!(trace.spans[0].operation_name, "GET /");
        assert_eq!(trace.spans[0].service_name, "my-api");
        assert_eq!(trace.spans[0].start_time_us, 1000);
        assert_eq!(trace.spans[0].duration_us, 1000);
    }

    #[test]
    fn test_parse_multi_service_trace() {
        let batches = vec![
            make_batch(
                "gateway",
                vec![make_otlp_span(
                    "t1", "s1", None, "ingress", 1_000_000, 5_000_000,
                )],
            ),
            make_batch(
                "backend",
                vec![make_otlp_span(
                    "t1",
                    "s2",
                    Some("s1"),
                    "handle",
                    2_000_000,
                    4_000_000,
                )],
            ),
        ];

        let trace = parse_otlp_trace(batches).unwrap();
        assert_eq!(trace.trace_id, "t1");
        assert_eq!(trace.spans.len(), 2);
        assert!(trace.services.contains(&"gateway".to_string()));
        assert!(trace.services.contains(&"backend".to_string()));
    }

    #[test]
    fn test_parse_empty_batches_returns_error() {
        let result = parse_otlp_trace(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_batch_with_no_spans_returns_error() {
        let batches = vec![OtlpBatch {
            resource: None,
            scope_spans: Some(vec![OtlpScopeSpans {
                spans: Some(vec![]),
            }]),
            instrumentation_library_spans: None,
        }];
        let result = parse_otlp_trace(batches);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_status_codes() {
        let mut span_ok = make_otlp_span("t1", "s1", None, "ok-span", 1000, 2000);
        span_ok.status = Some(OtlpStatus {
            code: Some(1),
            message: None,
        });

        let mut span_err = make_otlp_span("t1", "s2", Some("s1"), "err-span", 1000, 2000);
        span_err.status = Some(OtlpStatus {
            code: Some(2),
            message: Some("failed".to_string()),
        });

        let mut span_unset = make_otlp_span("t1", "s3", Some("s1"), "unset-span", 1000, 2000);
        span_unset.status = Some(OtlpStatus {
            code: Some(0),
            message: None,
        });

        let batches = vec![make_batch("svc", vec![span_ok, span_err, span_unset])];
        let trace = parse_otlp_trace(batches).unwrap();

        assert_eq!(trace.spans[0].status, SpanStatus::Ok);
        assert_eq!(trace.spans[1].status, SpanStatus::Error);
        assert_eq!(trace.spans[2].status, SpanStatus::Unset);
    }

    #[test]
    fn test_parse_span_with_attributes() {
        let mut span = make_otlp_span("t1", "s1", None, "op", 1000, 2000);
        span.attributes = Some(vec![
            OtlpAttribute {
                key: "http.method".to_string(),
                value: Some(OtlpValue {
                    string_value: Some("GET".to_string()),
                    int_value: None,
                    double_value: None,
                    bool_value: None,
                }),
            },
            OtlpAttribute {
                key: "http.status_code".to_string(),
                value: Some(OtlpValue {
                    string_value: None,
                    int_value: Some(200),
                    double_value: None,
                    bool_value: None,
                }),
            },
        ]);

        let batches = vec![make_batch("svc", vec![span])];
        let trace = parse_otlp_trace(batches).unwrap();

        assert_eq!(trace.spans[0].tags.get("http.method").unwrap(), "GET");
        assert_eq!(trace.spans[0].tags.get("http.status_code").unwrap(), "200");
    }

    #[test]
    fn test_parse_span_with_events() {
        let mut span = make_otlp_span("t1", "s1", None, "op", 1_000_000, 2_000_000);
        span.events = Some(vec![OtlpEvent {
            time_unix_nano: Some(1_500_000),
            name: Some("exception".to_string()),
            attributes: Some(vec![OtlpAttribute {
                key: "exception.message".to_string(),
                value: Some(OtlpValue {
                    string_value: Some("null pointer".to_string()),
                    int_value: None,
                    double_value: None,
                    bool_value: None,
                }),
            }]),
        }]);

        let batches = vec![make_batch("svc", vec![span])];
        let trace = parse_otlp_trace(batches).unwrap();

        assert_eq!(trace.spans[0].logs.len(), 1);
        let log = &trace.spans[0].logs[0];
        assert_eq!(log.timestamp_us, 1500);
        assert_eq!(log.fields.get("event").unwrap(), "exception");
        assert_eq!(log.fields.get("exception.message").unwrap(), "null pointer");
    }

    #[test]
    fn test_parse_missing_resource_uses_unknown() {
        let batches = vec![OtlpBatch {
            resource: None,
            scope_spans: Some(vec![OtlpScopeSpans {
                spans: Some(vec![make_otlp_span("t1", "s1", None, "op", 1000, 2000)]),
            }]),
            instrumentation_library_spans: None,
        }];

        let trace = parse_otlp_trace(batches).unwrap();
        assert_eq!(trace.spans[0].service_name, "unknown");
    }

    #[test]
    fn test_parse_instrumentation_library_spans_fallback() {
        let batches = vec![OtlpBatch {
            resource: None,
            scope_spans: None,
            instrumentation_library_spans: Some(vec![OtlpScopeSpans {
                spans: Some(vec![make_otlp_span("t1", "s1", None, "op", 1000, 2000)]),
            }]),
        }];

        let trace = parse_otlp_trace(batches).unwrap();
        assert_eq!(trace.spans.len(), 1);
    }
}
