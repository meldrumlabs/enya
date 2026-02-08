//! Loki HTTP API response parsing.

use rustc_hash::FxHashMap;
use serde::Deserialize;

use crate::error::ClientError;
use crate::logs::{LogEntry, LogLevel, LogsResponse};

/// Top-level Loki API response wrapper.
#[derive(Debug, Deserialize)]
pub struct LokiResponse {
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(rename = "errorType")]
    #[serde(default)]
    pub error_type: Option<String>,
    pub data: Option<LokiData>,
}

/// The data payload from a Loki query.
#[derive(Debug, Deserialize)]
pub struct LokiData {
    #[serde(rename = "resultType")]
    pub result_type: String,
    pub result: Vec<LokiStream>,
}

/// A single log stream from Loki.
#[derive(Debug, Deserialize)]
pub struct LokiStream {
    /// Labels identifying this stream.
    pub stream: FxHashMap<String, String>,
    /// Log entries as (timestamp_ns_string, log_line) pairs.
    pub values: Vec<(String, String)>,
}

/// Loki labels endpoint response.
#[derive(Debug, Deserialize)]
pub struct LokiLabelsResponse {
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    pub data: Option<Vec<String>>,
}

/// Loki build info response.
#[derive(Debug, Deserialize)]
pub struct LokiBuildInfo {
    pub version: String,
    #[serde(default)]
    pub revision: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
}

/// Loki build info endpoint response wrapper.
#[derive(Debug, Deserialize)]
pub struct LokiBuildInfoResponse {
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    pub data: Option<LokiBuildInfo>,
}

/// Parse a Loki query_range response into a LogsResponse.
pub fn parse_logs_response(json: &[u8]) -> Result<LogsResponse, ClientError> {
    let response: LokiResponse =
        serde_json::from_slice(json).map_err(|e| ClientError::ParseError(e.to_string()))?;

    if response.status != "success" {
        let message = response
            .error
            .or(response.error_type)
            .unwrap_or_else(|| "unknown error".to_string());
        return Err(ClientError::BackendError {
            status: 400,
            message,
        });
    }

    let data = response
        .data
        .ok_or_else(|| ClientError::ParseError("missing data field".to_string()))?;

    let streams_count = data.result.len();
    let mut entries: Vec<LogEntry> = Vec::new();

    for stream in data.result {
        let labels = stream.stream;

        for (ts_str, message) in stream.values {
            // Parse timestamp - Loki returns nanoseconds as a string
            let timestamp_ns: i64 = ts_str
                .parse()
                .map_err(|e| ClientError::ParseError(format!("invalid timestamp: {e}")))?;

            // Try to detect log level from labels or message
            let level = labels
                .get("level")
                .and_then(|l| LogLevel::parse(l))
                .or_else(|| labels.get("severity").and_then(|l| LogLevel::parse(l)))
                .or_else(|| LogLevel::detect_from_message(&message));

            entries.push(LogEntry {
                timestamp_ns,
                message,
                labels: labels.clone(),
                level,
            });
        }
    }

    // Sort by timestamp (most backends return sorted, but let's be safe)
    entries.sort_by_key(|e| e.timestamp_ns);

    Ok(LogsResponse {
        entries,
        streams_count,
    })
}

/// Parse a Loki labels response.
pub fn parse_labels_response(json: &[u8]) -> Result<Vec<String>, ClientError> {
    let response: LokiLabelsResponse =
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

    Ok(response.data.unwrap_or_default())
}

/// Parse a Loki build info response.
pub fn parse_buildinfo_response(json: &[u8]) -> Result<LokiBuildInfo, ClientError> {
    let response: LokiBuildInfoResponse =
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

    response
        .data
        .ok_or_else(|| ClientError::ParseError("missing data field".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_logs_response_success() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "streams",
                "result": [
                    {
                        "stream": {"app": "myservice", "level": "info"},
                        "values": [
                            ["1609459200000000000", "Starting server on port 8080"],
                            ["1609459201000000000", "Connected to database"]
                        ]
                    },
                    {
                        "stream": {"app": "myservice", "level": "error"},
                        "values": [
                            ["1609459202000000000", "Connection timeout"]
                        ]
                    }
                ]
            }
        }"#;

        let result = parse_logs_response(json.as_bytes()).unwrap();
        assert_eq!(result.streams_count, 2);
        assert_eq!(result.entries.len(), 3);

        // Check first entry
        assert_eq!(result.entries[0].timestamp_ns, 1609459200000000000);
        assert_eq!(result.entries[0].message, "Starting server on port 8080");
        assert_eq!(result.entries[0].level, Some(LogLevel::Info));
        assert_eq!(
            result.entries[0].labels.get("app"),
            Some(&"myservice".to_string())
        );

        // Check last entry (error level)
        assert_eq!(result.entries[2].level, Some(LogLevel::Error));
    }

    #[test]
    fn test_parse_logs_response_empty() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "streams",
                "result": []
            }
        }"#;

        let result = parse_logs_response(json.as_bytes()).unwrap();
        assert_eq!(result.streams_count, 0);
        assert_eq!(result.entries.len(), 0);
    }

    #[test]
    fn test_parse_logs_response_error() {
        let json = r#"{
            "status": "error",
            "error": "parse error at line 1"
        }"#;

        let result = parse_logs_response(json.as_bytes());
        assert!(result.is_err());
        match result.unwrap_err() {
            ClientError::BackendError {
                status: 400,
                message,
            } => {
                assert!(message.contains("parse error"));
            }
            e => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn test_parse_labels_response() {
        let json = r#"{
            "status": "success",
            "data": ["app", "env", "host", "level"]
        }"#;

        let result = parse_labels_response(json.as_bytes()).unwrap();
        assert_eq!(result, vec!["app", "env", "host", "level"]);
    }

    #[test]
    fn test_parse_buildinfo_response() {
        let json = r#"{
            "status": "success",
            "data": {
                "version": "2.9.0",
                "revision": "abc123",
                "branch": "main"
            }
        }"#;

        let result = parse_buildinfo_response(json.as_bytes()).unwrap();
        assert_eq!(result.version, "2.9.0");
        assert_eq!(result.revision, Some("abc123".to_string()));
    }

    #[test]
    fn test_level_detection_from_labels() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "streams",
                "result": [
                    {
                        "stream": {"severity": "warning"},
                        "values": [["1000000000", "This is a warning"]]
                    }
                ]
            }
        }"#;

        let result = parse_logs_response(json.as_bytes()).unwrap();
        assert_eq!(result.entries[0].level, Some(LogLevel::Warn));
    }

    #[test]
    fn test_level_detection_from_message() {
        let json = r#"{
            "status": "success",
            "data": {
                "resultType": "streams",
                "result": [
                    {
                        "stream": {"app": "test"},
                        "values": [["1000000000", "[ERROR] Something went wrong"]]
                    }
                ]
            }
        }"#;

        let result = parse_logs_response(json.as_bytes()).unwrap();
        assert_eq!(result.entries[0].level, Some(LogLevel::Error));
    }
}
