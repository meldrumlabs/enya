//! Shared OpenTelemetry Protocol (OTLP) JSON serde types.
//!
//! These types are used for deserializing OTLP JSON payloads, both when
//! parsing Tempo responses (which can return OTLP format) and when receiving
//! OTLP data directly via the OTLP HTTP receiver.

use serde::Deserialize;

/// A resource batch of spans in OTLP format.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpBatch {
    pub resource: Option<OtlpResource>,
    pub scope_spans: Option<Vec<OtlpScopeSpans>>,
    /// Legacy field name
    pub instrumentation_library_spans: Option<Vec<OtlpScopeSpans>>,
}

/// OTLP resource containing attributes (e.g., service.name).
#[derive(Deserialize)]
pub struct OtlpResource {
    pub attributes: Option<Vec<OtlpAttribute>>,
}

/// A group of spans from a single instrumentation scope.
#[derive(Deserialize)]
pub struct OtlpScopeSpans {
    pub spans: Option<Vec<OtlpSpan>>,
}

/// A single OTLP span.
#[derive(Deserialize)]
pub struct OtlpSpan {
    #[serde(alias = "traceId", alias = "trace_id")]
    pub trace_id: Option<String>,
    #[serde(alias = "spanId", alias = "span_id")]
    pub span_id: Option<String>,
    #[serde(alias = "parentSpanId", alias = "parent_span_id")]
    pub parent_span_id: Option<String>,
    pub name: Option<String>,
    #[serde(alias = "startTimeUnixNano", alias = "start_time_unix_nano")]
    pub start_time_unix_nano: Option<u64>,
    #[serde(alias = "endTimeUnixNano", alias = "end_time_unix_nano")]
    pub end_time_unix_nano: Option<u64>,
    pub status: Option<OtlpStatus>,
    pub attributes: Option<Vec<OtlpAttribute>>,
    pub events: Option<Vec<OtlpEvent>>,
}

/// OTLP span status.
#[derive(Deserialize)]
pub struct OtlpStatus {
    pub code: Option<i32>,
    #[allow(dead_code)]
    pub message: Option<String>,
}

/// A key-value attribute in OTLP format.
#[derive(Deserialize)]
pub struct OtlpAttribute {
    pub key: String,
    pub value: Option<OtlpValue>,
}

/// Polymorphic OTLP attribute value.
#[derive(Deserialize)]
pub struct OtlpValue {
    #[serde(alias = "stringValue", alias = "string_value")]
    pub string_value: Option<String>,
    #[serde(alias = "intValue", alias = "int_value")]
    pub int_value: Option<i64>,
    #[serde(alias = "doubleValue", alias = "double_value")]
    pub double_value: Option<f64>,
    #[serde(alias = "boolValue", alias = "bool_value")]
    pub bool_value: Option<bool>,
}

/// An event (log) within an OTLP span.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpEvent {
    pub time_unix_nano: Option<u64>,
    pub name: Option<String>,
    pub attributes: Option<Vec<OtlpAttribute>>,
}

// ============================================================================
// OTLP Logs Types
// ============================================================================

/// Top-level OTLP logs export request body.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpLogsData {
    pub resource_logs: Vec<OtlpResourceLogs>,
}

/// Logs grouped by resource.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpResourceLogs {
    pub resource: Option<OtlpResource>,
    pub scope_logs: Option<Vec<OtlpScopeLogs>>,
}

/// Logs grouped by instrumentation scope.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpScopeLogs {
    pub log_records: Option<Vec<OtlpLogRecord>>,
}

/// A single OTLP log record.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpLogRecord {
    pub time_unix_nano: Option<u64>,
    pub observed_time_unix_nano: Option<u64>,
    pub severity_number: Option<i32>,
    pub severity_text: Option<String>,
    pub body: Option<OtlpAnyValue>,
    pub attributes: Option<Vec<OtlpAttribute>>,
    #[serde(alias = "traceId", alias = "trace_id")]
    pub trace_id: Option<String>,
    #[serde(alias = "spanId", alias = "span_id")]
    pub span_id: Option<String>,
}

/// OTLP AnyValue for log body.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpAnyValue {
    pub string_value: Option<String>,
    pub int_value: Option<i64>,
    pub double_value: Option<f64>,
    pub bool_value: Option<bool>,
}

// ============================================================================
// OTLP Traces Export Request
// ============================================================================

/// Top-level OTLP traces export request body.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OtlpTracesData {
    pub resource_spans: Vec<OtlpBatch>,
}

// ============================================================================
// Helper functions
// ============================================================================

impl OtlpResource {
    /// Extract the `service.name` attribute from this resource.
    pub fn service_name(&self) -> Option<String> {
        self.attributes.as_ref().and_then(|attrs| {
            attrs
                .iter()
                .find(|a| a.key == "service.name")
                .and_then(|a| a.value.as_ref().and_then(|v| v.string_value.clone()))
        })
    }
}

impl OtlpAttribute {
    /// Get the attribute value as a string, converting numeric/bool types.
    pub fn value_as_string(&self) -> Option<String> {
        self.value.as_ref().and_then(|v| {
            v.string_value
                .clone()
                .or(v.int_value.map(|i| i.to_string()))
                .or(v.double_value.map(|d| d.to_string()))
                .or(v.bool_value.map(|b| b.to_string()))
        })
    }
}

impl OtlpAnyValue {
    /// Convert to a string representation.
    pub fn to_string_lossy(&self) -> String {
        if let Some(s) = &self.string_value {
            s.clone()
        } else if let Some(i) = self.int_value {
            i.to_string()
        } else if let Some(d) = self.double_value {
            d.to_string()
        } else if let Some(b) = self.bool_value {
            b.to_string()
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_value_as_string_types() {
        // String value
        let attr = OtlpAttribute {
            key: "k".to_string(),
            value: Some(OtlpValue {
                string_value: Some("hello".to_string()),
                int_value: None,
                double_value: None,
                bool_value: None,
            }),
        };
        assert_eq!(attr.value_as_string().unwrap(), "hello");

        // Int value
        let attr = OtlpAttribute {
            key: "k".to_string(),
            value: Some(OtlpValue {
                string_value: None,
                int_value: Some(42),
                double_value: None,
                bool_value: None,
            }),
        };
        assert_eq!(attr.value_as_string().unwrap(), "42");

        // Bool value
        let attr = OtlpAttribute {
            key: "k".to_string(),
            value: Some(OtlpValue {
                string_value: None,
                int_value: None,
                double_value: None,
                bool_value: Some(true),
            }),
        };
        assert_eq!(attr.value_as_string().unwrap(), "true");

        // No value
        let attr = OtlpAttribute {
            key: "k".to_string(),
            value: None,
        };
        assert!(attr.value_as_string().is_none());
    }

    #[test]
    fn test_any_value_to_string_lossy() {
        let v = OtlpAnyValue {
            string_value: Some("msg".to_string()),
            int_value: None,
            double_value: None,
            bool_value: None,
        };
        assert_eq!(v.to_string_lossy(), "msg");

        let v = OtlpAnyValue {
            string_value: None,
            int_value: Some(99),
            double_value: None,
            bool_value: None,
        };
        assert_eq!(v.to_string_lossy(), "99");

        // Empty value
        let v = OtlpAnyValue {
            string_value: None,
            int_value: None,
            double_value: None,
            bool_value: None,
        };
        assert_eq!(v.to_string_lossy(), "");
    }

    #[test]
    fn test_resource_service_name() {
        let resource = OtlpResource {
            attributes: Some(vec![OtlpAttribute {
                key: "service.name".to_string(),
                value: Some(OtlpValue {
                    string_value: Some("my-api".to_string()),
                    int_value: None,
                    double_value: None,
                    bool_value: None,
                }),
            }]),
        };
        assert_eq!(resource.service_name().unwrap(), "my-api");

        // No attributes
        let resource = OtlpResource { attributes: None };
        assert!(resource.service_name().is_none());

        // No service.name key
        let resource = OtlpResource {
            attributes: Some(vec![OtlpAttribute {
                key: "other.key".to_string(),
                value: Some(OtlpValue {
                    string_value: Some("val".to_string()),
                    int_value: None,
                    double_value: None,
                    bool_value: None,
                }),
            }]),
        };
        assert!(resource.service_name().is_none());
    }

    #[test]
    fn test_deserialize_otlp_span_camel_case() {
        let json = r#"{
            "traceId": "abc123",
            "spanId": "span1",
            "parentSpanId": "parent1",
            "name": "GET /api",
            "startTimeUnixNano": 1000000000,
            "endTimeUnixNano": 2000000000,
            "status": { "code": 1 }
        }"#;
        let span: OtlpSpan = serde_json::from_str(json).unwrap();
        assert_eq!(span.trace_id.unwrap(), "abc123");
        assert_eq!(span.span_id.unwrap(), "span1");
        assert_eq!(span.parent_span_id.unwrap(), "parent1");
        assert_eq!(span.name.unwrap(), "GET /api");
        assert_eq!(span.start_time_unix_nano.unwrap(), 1_000_000_000);
        assert_eq!(span.end_time_unix_nano.unwrap(), 2_000_000_000);
        assert_eq!(span.status.unwrap().code.unwrap(), 1);
    }

    #[test]
    fn test_deserialize_otlp_span_snake_case() {
        let json = r#"{
            "trace_id": "abc123",
            "span_id": "span1",
            "start_time_unix_nano": 5000000000
        }"#;
        let span: OtlpSpan = serde_json::from_str(json).unwrap();
        assert_eq!(span.trace_id.unwrap(), "abc123");
        assert_eq!(span.span_id.unwrap(), "span1");
        assert_eq!(span.start_time_unix_nano.unwrap(), 5_000_000_000);
    }

    #[test]
    fn test_deserialize_otlp_log_record() {
        let json = r#"{
            "timeUnixNano": 1000,
            "severityNumber": 9,
            "severityText": "INFO",
            "body": { "stringValue": "hello world" },
            "attributes": [
                { "key": "env", "value": { "stringValue": "prod" } }
            ]
        }"#;
        let record: OtlpLogRecord = serde_json::from_str(json).unwrap();
        assert_eq!(record.time_unix_nano.unwrap(), 1000);
        assert_eq!(record.severity_number.unwrap(), 9);
        assert_eq!(record.severity_text.as_deref().unwrap(), "INFO");
        assert_eq!(record.body.unwrap().to_string_lossy(), "hello world");
        assert_eq!(record.attributes.unwrap().len(), 1);
    }

    #[test]
    fn test_deserialize_otlp_traces_data() {
        let json = r#"{
            "resourceSpans": [{
                "resource": {
                    "attributes": [
                        { "key": "service.name", "value": { "stringValue": "test-svc" } }
                    ]
                },
                "scopeSpans": [{
                    "spans": [{
                        "traceId": "t1",
                        "spanId": "s1",
                        "name": "root",
                        "startTimeUnixNano": 1000000,
                        "endTimeUnixNano": 2000000
                    }]
                }]
            }]
        }"#;
        let data: OtlpTracesData = serde_json::from_str(json).unwrap();
        assert_eq!(data.resource_spans.len(), 1);
        let batch = &data.resource_spans[0];
        assert_eq!(
            batch.resource.as_ref().unwrap().service_name().unwrap(),
            "test-svc"
        );
        let spans = batch.scope_spans.as_ref().unwrap();
        assert_eq!(spans[0].spans.as_ref().unwrap().len(), 1);
    }
}
