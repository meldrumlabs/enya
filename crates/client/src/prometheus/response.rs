//! Prometheus response parsing.
//!
//! Converts Prometheus HTTP API JSON responses to `QueryResponse`.

use serde::Deserialize;
use std::collections::HashMap;

use crate::error::ClientError;
use enya_common::{MetricsBucket, MetricsGroup, QueryResponse};

/// Prometheus API response wrapper for query endpoints.
#[derive(Debug, Deserialize)]
pub struct PrometheusResponse {
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_type: Option<String>,
    pub data: Option<PrometheusData>,
}

/// Prometheus API response wrapper for label/metadata endpoints.
#[derive(Debug, Deserialize)]
pub struct PrometheusLabelsResponse {
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_type: Option<String>,
    #[serde(default)]
    pub data: Vec<String>,
}

/// Prometheus query result data.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrometheusData {
    pub result_type: String,
    pub result: Vec<PrometheusResult>,
}

/// A single result entry from Prometheus.
#[derive(Debug, Deserialize)]
pub struct PrometheusResult {
    /// Label set for this series (metric name + labels).
    pub metric: HashMap<String, String>,
    /// Time series values as [timestamp, value] pairs.
    /// Timestamps are Unix seconds (float), values are strings.
    pub values: Vec<(f64, String)>,
}

/// Parse a Prometheus JSON response into a `QueryResponse`.
///
/// # Arguments
///
/// * `json` - Raw JSON bytes from Prometheus HTTP API
/// * `metric` - The original metric name (for the response)
/// * `query` - The original query string (for the response)
/// * `granularity_ns` - The query step in nanoseconds
///
/// # Errors
///
/// Returns `ClientError::ParseError` if the JSON is invalid or has unexpected structure.
/// Returns `ClientError::BackendError` if Prometheus returned an error status.
pub fn parse_response(
    json: &[u8],
    metric: &str,
    query: &str,
    granularity_ns: u128,
) -> Result<QueryResponse, ClientError> {
    let response: PrometheusResponse =
        serde_json::from_slice(json).map_err(|e| ClientError::ParseError(e.to_string()))?;

    // Check for error status
    if response.status != "success" {
        let message = response
            .error
            .unwrap_or_else(|| "unknown error".to_string());
        return Err(ClientError::BackendError {
            status: 400,
            message,
        });
    }

    let data = response
        .data
        .ok_or_else(|| ClientError::ParseError("missing data field".to_string()))?;

    // Convert Prometheus results to MetricsGroups
    let groups = data.result.into_iter().map(convert_result).collect();

    Ok(QueryResponse {
        metric: metric.to_string(),
        query: query.to_string(),
        parsed_agg: None,
        parsed_filter: String::new(),
        parsed_grouping: None,
        parsed_time_range: None,
        start: None,
        end: None,
        granularity_ns,
        groups,
    })
}

/// Parse a Prometheus labels/values response into a Vec<String>.
///
/// Used for `/api/v1/labels`, `/api/v1/label/{name}/values`, etc.
///
/// # Errors
///
/// Returns `ClientError::ParseError` if the JSON is invalid.
/// Returns `ClientError::BackendError` if Prometheus returned an error status.
pub fn parse_labels_response(json: &[u8]) -> Result<Vec<String>, ClientError> {
    let response: PrometheusLabelsResponse =
        serde_json::from_slice(json).map_err(|e| ClientError::ParseError(e.to_string()))?;

    if response.status != "success" {
        let message = response
            .error
            .unwrap_or_else(|| "unknown error".to_string());
        return Err(ClientError::BackendError {
            status: 400,
            message,
        });
    }

    Ok(response.data)
}

/// Convert a Prometheus result entry to a MetricsGroup.
fn convert_result(result: PrometheusResult) -> MetricsGroup {
    // Build group identifier from labels (excluding __name__)
    let group = result
        .metric
        .iter()
        .filter(|(k, _)| *k != "__name__")
        .map(|(k, v)| format!("{k}:{v}"))
        .collect::<Vec<_>>()
        .join(",");

    // Convert [timestamp, value] pairs to MetricsBuckets
    let buckets = result
        .values
        .iter()
        .filter_map(|(ts, val)| {
            let value: f64 = val.parse().ok()?;
            let ts_ns = (*ts * 1_000_000_000.0) as u128;
            Some(MetricsBucket {
                start: ts_ns,
                end: ts_ns, // Point-in-time for Prometheus
                value,
                count: 1,
            })
        })
        .collect();

    MetricsGroup { group, buckets }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_success_response() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": [
                    {
                        "metric": {"__name__": "cpu_usage", "env": "prod", "host": "server1"},
                        "values": [[1700000000, "0.5"], [1700000060, "0.6"]]
                    }
                ]
            }
        }"#;

        let response = parse_response(json.as_bytes(), "cpu_usage", "env:prod", 60_000_000_000)
            .expect("should parse");

        assert_eq!(response.metric, "cpu_usage");
        assert_eq!(response.groups.len(), 1);

        let group = &response.groups[0];
        // Group should contain labels but not __name__
        assert!(group.group.contains("env:prod"));
        assert!(group.group.contains("host:server1"));
        assert!(!group.group.contains("__name__"));

        assert_eq!(group.buckets.len(), 2);
        assert!((group.buckets[0].value - 0.5).abs() < f64::EPSILON);
        assert!((group.buckets[1].value - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_multiple_series() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": [
                    {
                        "metric": {"env": "prod"},
                        "values": [[1700000000, "10"]]
                    },
                    {
                        "metric": {"env": "staging"},
                        "values": [[1700000000, "5"]]
                    }
                ]
            }
        }"#;

        let response =
            parse_response(json.as_bytes(), "metric", "*", 60_000_000_000).expect("should parse");

        assert_eq!(response.groups.len(), 2);
    }

    #[test]
    fn test_parse_error_response() {
        let json = r#"{
            "status": "error",
            "errorType": "bad_data",
            "error": "invalid query syntax"
        }"#;

        let result = parse_response(json.as_bytes(), "metric", "bad", 60_000_000_000);
        assert!(result.is_err());

        match result.unwrap_err() {
            ClientError::BackendError { message, .. } => {
                assert!(message.contains("invalid query syntax"));
            }
            _ => panic!("expected BackendError"),
        }
    }

    #[test]
    fn test_parse_empty_result() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": []
            }
        }"#;

        let response =
            parse_response(json.as_bytes(), "metric", "*", 60_000_000_000).expect("should parse");

        assert!(response.groups.is_empty());
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = b"not json";
        let result = parse_response(json, "metric", "*", 60_000_000_000);
        assert!(matches!(result, Err(ClientError::ParseError(_))));
    }

    // === Labels response tests ===

    #[test]
    fn test_parse_labels_response_success() {
        let json = r#"{
            "status": "success",
            "data": ["env", "host", "service", "region"]
        }"#;

        let labels = parse_labels_response(json.as_bytes()).expect("should parse");
        assert_eq!(labels, vec!["env", "host", "service", "region"]);
    }

    #[test]
    fn test_parse_labels_response_empty() {
        let json = r#"{
            "status": "success",
            "data": []
        }"#;

        let labels = parse_labels_response(json.as_bytes()).expect("should parse");
        assert!(labels.is_empty());
    }

    #[test]
    fn test_parse_labels_response_error() {
        let json = r#"{
            "status": "error",
            "errorType": "bad_data",
            "error": "invalid label name"
        }"#;

        let result = parse_labels_response(json.as_bytes());
        assert!(result.is_err());

        match result.unwrap_err() {
            ClientError::BackendError { message, .. } => {
                assert!(message.contains("invalid label name"));
            }
            _ => panic!("expected BackendError"),
        }
    }

    #[test]
    fn test_parse_labels_response_invalid_json() {
        let json = b"not json";
        let result = parse_labels_response(json);
        assert!(matches!(result, Err(ClientError::ParseError(_))));
    }

    #[test]
    fn test_parse_label_values_response() {
        // This is the same format as labels, just with different data
        let json = r#"{
            "status": "success",
            "data": ["prod", "staging", "dev"]
        }"#;

        let values = parse_labels_response(json.as_bytes()).expect("should parse");
        assert_eq!(values, vec!["prod", "staging", "dev"]);
    }

    #[test]
    fn test_parse_metric_names_response() {
        // Metric names come from __name__ label values
        let json = r#"{
            "status": "success",
            "data": ["cpu_usage", "memory_usage", "http_requests_total"]
        }"#;

        let metrics = parse_labels_response(json.as_bytes()).expect("should parse");
        assert_eq!(
            metrics,
            vec!["cpu_usage", "memory_usage", "http_requests_total"]
        );
    }
}
