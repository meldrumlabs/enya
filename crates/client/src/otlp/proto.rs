//! Minimal prost message definitions for OTLP protobuf decoding.
//!
//! These structs mirror the OpenTelemetry proto definitions just enough to
//! decode `ExportTraceServiceRequest`, `ExportLogsServiceRequest`, and
//! `ExportMetricsServiceRequest` protobuf payloads. Field numbers match the
//! upstream `.proto` files exactly.
//!
//! After decoding, values are converted to the JSON-compatible types in
//! [`super::types`] so the rest of the pipeline is shared.

use rustc_hash::FxHashMap;

// ============================================================================
// Common types
// ============================================================================

#[derive(prost::Message, Clone)]
pub struct Resource {
    #[prost(message, repeated, tag = "1")]
    pub attributes: Vec<KeyValue>,
}

#[derive(prost::Message, Clone)]
pub struct KeyValue {
    #[prost(string, tag = "1")]
    pub key: String,
    #[prost(message, optional, tag = "2")]
    pub value: Option<AnyValue>,
}

#[derive(prost::Message, Clone)]
pub struct AnyValue {
    #[prost(oneof = "any_value::Value", tags = "1, 2, 3, 4")]
    pub value: Option<any_value::Value>,
}

pub mod any_value {
    #[derive(prost::Oneof, Clone)]
    pub enum Value {
        #[prost(string, tag = "1")]
        StringValue(String),
        #[prost(bool, tag = "2")]
        BoolValue(bool),
        #[prost(int64, tag = "3")]
        IntValue(i64),
        #[prost(double, tag = "4")]
        DoubleValue(f64),
    }
}

// ============================================================================
// Traces
// ============================================================================

#[derive(prost::Message)]
pub struct ExportTraceServiceRequest {
    #[prost(message, repeated, tag = "1")]
    pub resource_spans: Vec<ResourceSpans>,
}

#[derive(prost::Message)]
pub struct ResourceSpans {
    #[prost(message, optional, tag = "1")]
    pub resource: Option<Resource>,
    #[prost(message, repeated, tag = "2")]
    pub scope_spans: Vec<ScopeSpans>,
}

#[derive(prost::Message)]
pub struct ScopeSpans {
    #[prost(message, repeated, tag = "2")]
    pub spans: Vec<ProtoSpan>,
}

#[derive(prost::Message, Clone)]
pub struct ProtoSpan {
    #[prost(bytes, tag = "1")]
    pub trace_id: Vec<u8>,
    #[prost(bytes, tag = "2")]
    pub span_id: Vec<u8>,
    #[prost(bytes, tag = "4")]
    pub parent_span_id: Vec<u8>,
    #[prost(string, tag = "5")]
    pub name: String,
    #[prost(fixed64, tag = "7")]
    pub start_time_unix_nano: u64,
    #[prost(fixed64, tag = "8")]
    pub end_time_unix_nano: u64,
    #[prost(message, repeated, tag = "9")]
    pub attributes: Vec<KeyValue>,
    #[prost(message, repeated, tag = "11")]
    pub events: Vec<ProtoEvent>,
    #[prost(message, optional, tag = "15")]
    pub status: Option<ProtoStatus>,
}

#[derive(prost::Message, Clone)]
pub struct ProtoEvent {
    #[prost(fixed64, tag = "1")]
    pub time_unix_nano: u64,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(message, repeated, tag = "3")]
    pub attributes: Vec<KeyValue>,
}

#[derive(prost::Message, Clone)]
pub struct ProtoStatus {
    #[prost(string, tag = "2")]
    pub message: String,
    #[prost(int32, tag = "3")]
    pub code: i32,
}

// ============================================================================
// Logs
// ============================================================================

#[derive(prost::Message)]
pub struct ExportLogsServiceRequest {
    #[prost(message, repeated, tag = "1")]
    pub resource_logs: Vec<ResourceLogs>,
}

#[derive(prost::Message)]
pub struct ResourceLogs {
    #[prost(message, optional, tag = "1")]
    pub resource: Option<Resource>,
    #[prost(message, repeated, tag = "2")]
    pub scope_logs: Vec<ScopeLogs>,
}

#[derive(prost::Message)]
pub struct ScopeLogs {
    #[prost(message, repeated, tag = "2")]
    pub log_records: Vec<ProtoLogRecord>,
}

#[derive(prost::Message, Clone)]
pub struct ProtoLogRecord {
    #[prost(fixed64, tag = "1")]
    pub time_unix_nano: u64,
    #[prost(int32, tag = "2")]
    pub severity_number: i32,
    #[prost(string, tag = "3")]
    pub severity_text: String,
    #[prost(message, optional, tag = "5")]
    pub body: Option<AnyValue>,
    #[prost(message, repeated, tag = "6")]
    pub attributes: Vec<KeyValue>,
    #[prost(bytes, tag = "9")]
    pub trace_id: Vec<u8>,
    #[prost(bytes, tag = "10")]
    pub span_id: Vec<u8>,
    #[prost(fixed64, tag = "11")]
    pub observed_time_unix_nano: u64,
}

// ============================================================================
// Metrics
// ============================================================================

#[derive(prost::Message)]
pub struct ExportMetricsServiceRequest {
    #[prost(message, repeated, tag = "1")]
    pub resource_metrics: Vec<ProtoResourceMetrics>,
}

#[derive(prost::Message)]
pub struct ProtoResourceMetrics {
    #[prost(message, optional, tag = "1")]
    pub resource: Option<Resource>,
    #[prost(message, repeated, tag = "2")]
    pub scope_metrics: Vec<ProtoScopeMetrics>,
}

#[derive(prost::Message)]
pub struct ProtoScopeMetrics {
    #[prost(message, repeated, tag = "2")]
    pub metrics: Vec<ProtoMetric>,
}

#[derive(prost::Message, Clone)]
pub struct ProtoMetric {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "3")]
    pub unit: String,
    #[prost(oneof = "proto_metric::Data", tags = "5, 7, 9")]
    pub data: Option<proto_metric::Data>,
}

pub mod proto_metric {
    #[derive(prost::Oneof, Clone)]
    pub enum Data {
        #[prost(message, tag = "5")]
        Gauge(super::ProtoGauge),
        #[prost(message, tag = "7")]
        Sum(super::ProtoSum),
        #[prost(message, tag = "9")]
        Histogram(super::ProtoHistogram),
    }
}

#[derive(prost::Message, Clone)]
pub struct ProtoGauge {
    #[prost(message, repeated, tag = "1")]
    pub data_points: Vec<ProtoNumberDataPoint>,
}

#[derive(prost::Message, Clone)]
pub struct ProtoSum {
    #[prost(message, repeated, tag = "1")]
    pub data_points: Vec<ProtoNumberDataPoint>,
    #[prost(bool, tag = "3")]
    pub is_monotonic: bool,
}

#[derive(prost::Message, Clone)]
pub struct ProtoNumberDataPoint {
    #[prost(fixed64, tag = "2")]
    pub start_time_unix_nano: u64,
    #[prost(fixed64, tag = "3")]
    pub time_unix_nano: u64,
    #[prost(oneof = "proto_number_value::Value", tags = "4, 6")]
    pub value: Option<proto_number_value::Value>,
    #[prost(message, repeated, tag = "7")]
    pub attributes: Vec<KeyValue>,
}

pub mod proto_number_value {
    #[derive(prost::Oneof, Clone)]
    pub enum Value {
        #[prost(double, tag = "4")]
        AsDouble(f64),
        #[prost(sfixed64, tag = "6")]
        AsInt(i64),
    }
}

#[derive(prost::Message, Clone)]
pub struct ProtoHistogram {
    #[prost(message, repeated, tag = "1")]
    pub data_points: Vec<ProtoHistogramDataPoint>,
}

#[derive(prost::Message, Clone)]
pub struct ProtoHistogramDataPoint {
    #[prost(fixed64, tag = "2")]
    pub start_time_unix_nano: u64,
    #[prost(fixed64, tag = "3")]
    pub time_unix_nano: u64,
    #[prost(uint64, tag = "4")]
    pub count: u64,
    #[prost(double, optional, tag = "5")]
    pub sum: Option<f64>,
    #[prost(uint64, repeated, tag = "6")]
    pub bucket_counts: Vec<u64>,
    #[prost(double, repeated, tag = "7")]
    pub explicit_bounds: Vec<f64>,
    #[prost(message, repeated, tag = "9")]
    pub attributes: Vec<KeyValue>,
    #[prost(double, optional, tag = "11")]
    pub min: Option<f64>,
    #[prost(double, optional, tag = "12")]
    pub max: Option<f64>,
}

// ============================================================================
// Conversion helpers: proto types → domain types
// ============================================================================

/// Encode bytes as a lowercase hex string.
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Extract the `service.name` attribute from a Resource.
pub fn resource_service_name(resource: &Option<Resource>) -> String {
    resource
        .as_ref()
        .and_then(|r| {
            r.attributes
                .iter()
                .find(|a| a.key == "service.name")
                .and_then(|a| match &a.value {
                    Some(AnyValue {
                        value: Some(any_value::Value::StringValue(s)),
                    }) => Some(s.clone()),
                    _ => None,
                })
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Convert proto KeyValue attributes to a string map.
pub fn attrs_to_map(attrs: &[KeyValue]) -> FxHashMap<String, String> {
    attrs
        .iter()
        .filter_map(|kv| {
            let value = kv.value.as_ref().and_then(any_value_to_string_inner)?;
            Some((kv.key.clone(), value))
        })
        .collect()
}

/// Get string value from an optional AnyValue.
pub fn any_value_to_string(value: &Option<AnyValue>) -> String {
    value
        .as_ref()
        .and_then(any_value_to_string_inner)
        .unwrap_or_default()
}

/// Get string value from an AnyValue reference.
pub fn any_value_ref_to_string(value: &AnyValue) -> String {
    any_value_to_string_inner(value).unwrap_or_default()
}

fn any_value_to_string_inner(av: &AnyValue) -> Option<String> {
    match &av.value {
        Some(any_value::Value::StringValue(s)) => Some(s.clone()),
        Some(any_value::Value::IntValue(i)) => Some(i.to_string()),
        Some(any_value::Value::DoubleValue(d)) => Some(d.to_string()),
        Some(any_value::Value::BoolValue(b)) => Some(b.to_string()),
        None => None,
    }
}

/// Get numeric value from a NumberDataPoint.
pub fn number_data_point_value(dp: &ProtoNumberDataPoint) -> f64 {
    match &dp.value {
        Some(proto_number_value::Value::AsDouble(d)) => *d,
        Some(proto_number_value::Value::AsInt(i)) => *i as f64,
        None => 0.0,
    }
}
