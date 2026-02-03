//! Prometheus metrics discovery — list metrics, labels, metadata, and series.
//!
//! Uses `std::collections::HashMap` for serde compatibility with Prometheus API
//! responses (series and metadata endpoints return maps). The module-level allow
//! suppresses the disallowed_types lint for these cases.

#![allow(clippy::disallowed_types)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::format;
use super::promql::resolve_endpoint;
use crate::Result;

// -- Prometheus response types ------------------------------------------------

/// Response where `data` is a string array (labels, label values, metric names).
#[derive(Deserialize)]
struct StringListResponse {
    status: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    data: Vec<String>,
}

/// Response where `data` is an array of label-set maps (`/api/v1/series`).
#[derive(Deserialize)]
struct SeriesResponse {
    status: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    data: Vec<HashMap<String, String>>,
}

/// Response where `data` is a map of metric name → metadata entries (`/api/v1/metadata`).
#[derive(Deserialize)]
struct MetadataResponse {
    status: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    data: HashMap<String, Vec<MetadataEntry>>,
}

#[derive(Deserialize)]
struct MetadataEntry {
    #[serde(rename = "type")]
    metric_type: String,
    help: String,
    unit: String,
}

// -- Public return types ------------------------------------------------------

/// A single metric's metadata (type + help text).
#[derive(Debug, Clone, Serialize)]
pub struct MetricInfo {
    pub metric: String,
    pub metric_type: String,
    pub help: String,
    pub unit: String,
}

/// A series label-set (all labels for one matching series).
#[derive(Debug, Clone, Serialize)]
pub struct SeriesEntry {
    pub labels: HashMap<String, String>,
}

// -- HTTP helpers -------------------------------------------------------------

fn get_body(url: &str) -> Result<String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    Ok(response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("failed to read response: {e}"))?)
}

fn get_body_with_query(url: &str, params: &[(&str, &str)]) -> Result<String> {
    let mut req = ureq::get(url);
    for (k, v) in params {
        req = req.query(k, v);
    }
    let response = req
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    Ok(response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("failed to read response: {e}"))?)
}

fn check_string_list(body: &str) -> Result<Vec<String>> {
    let resp: StringListResponse =
        serde_json::from_str(body).map_err(|e| format!("failed to parse response: {e}"))?;
    if resp.status != "success" {
        return Err(resp.error.unwrap_or_else(|| "unknown error".into()).into());
    }
    Ok(resp.data)
}

// -- Core functions -----------------------------------------------------------

/// Fetch all metric names from Prometheus (`/api/v1/label/__name__/values`).
pub fn list_metrics(base_url: &str, match_selector: Option<&str>) -> Result<Vec<String>> {
    let url = format!("{base_url}/api/v1/label/__name__/values");
    let body = match match_selector {
        Some(sel) => get_body_with_query(&url, &[("match[]", sel)])?,
        None => get_body(&url)?,
    };
    check_string_list(&body)
}

/// Fetch all label names from Prometheus (`/api/v1/labels`).
pub fn list_labels(base_url: &str, match_selector: Option<&str>) -> Result<Vec<String>> {
    let url = format!("{base_url}/api/v1/labels");
    let body = match match_selector {
        Some(sel) => get_body_with_query(&url, &[("match[]", sel)])?,
        None => get_body(&url)?,
    };
    check_string_list(&body)
}

/// Fetch all values for a specific label (`/api/v1/label/{label}/values`).
pub fn label_values(base_url: &str, label: &str) -> Result<Vec<String>> {
    let url = format!("{base_url}/api/v1/label/{label}/values");
    let body = get_body(&url)?;
    check_string_list(&body)
}

/// Fetch metric metadata (`/api/v1/metadata`).
///
/// If `metric` is provided, filters to that single metric via the `metric=` query param.
pub fn metric_info(base_url: &str, metric: Option<&str>) -> Result<Vec<MetricInfo>> {
    let url = format!("{base_url}/api/v1/metadata");
    let body = match metric {
        Some(m) => get_body_with_query(&url, &[("metric", m)])?,
        None => get_body(&url)?,
    };

    let resp: MetadataResponse =
        serde_json::from_str(&body).map_err(|e| format!("failed to parse response: {e}"))?;

    if resp.status != "success" {
        return Err(resp.error.unwrap_or_else(|| "unknown error".into()).into());
    }

    let mut results: Vec<MetricInfo> = resp
        .data
        .into_iter()
        .filter_map(|(name, entries)| {
            entries.into_iter().next().map(|entry| MetricInfo {
                metric: name,
                metric_type: entry.metric_type,
                help: entry.help,
                unit: entry.unit,
            })
        })
        .collect();

    results.sort_by(|a, b| a.metric.cmp(&b.metric));
    Ok(results)
}

/// Fetch matching series from Prometheus (`/api/v1/series?match[]={selector}`).
pub fn query_series(base_url: &str, selector: &str) -> Result<Vec<SeriesEntry>> {
    let url = format!("{base_url}/api/v1/series");
    let body = get_body_with_query(&url, &[("match[]", selector)])?;

    let resp: SeriesResponse =
        serde_json::from_str(&body).map_err(|e| format!("failed to parse response: {e}"))?;

    if resp.status != "success" {
        return Err(resp.error.unwrap_or_else(|| "unknown error".into()).into());
    }

    Ok(resp
        .data
        .into_iter()
        .map(|labels| SeriesEntry { labels })
        .collect())
}

// -- CLI entry points ---------------------------------------------------------

/// CLI entry point for `enya metrics list`.
pub fn metrics_list(
    endpoint: Option<&str>,
    workspace: Option<&str>,
    match_selector: Option<&str>,
    json: bool,
) -> Result {
    let base_url = resolve_endpoint(endpoint, workspace)?;
    let names = list_metrics(&base_url, match_selector)?;
    if json {
        format::print_string_list_json("metrics", &names)?;
    } else {
        format::print_string_list("METRIC", &names)?;
    }
    Ok(())
}

/// CLI entry point for `enya metrics labels`.
pub fn metrics_labels(
    endpoint: Option<&str>,
    workspace: Option<&str>,
    match_selector: Option<&str>,
    json: bool,
) -> Result {
    let base_url = resolve_endpoint(endpoint, workspace)?;
    let labels = list_labels(&base_url, match_selector)?;
    if json {
        format::print_string_list_json("labels", &labels)?;
    } else {
        format::print_string_list("LABEL", &labels)?;
    }
    Ok(())
}

/// CLI entry point for `enya metrics label-values`.
pub fn metrics_label_values(
    endpoint: Option<&str>,
    workspace: Option<&str>,
    label: &str,
    json: bool,
) -> Result {
    let base_url = resolve_endpoint(endpoint, workspace)?;
    let values = label_values(&base_url, label)?;
    if json {
        format::print_string_list_json("values", &values)?;
    } else {
        format::print_string_list("VALUE", &values)?;
    }
    Ok(())
}

/// CLI entry point for `enya metrics info`.
pub fn metrics_info(
    endpoint: Option<&str>,
    workspace: Option<&str>,
    metric: Option<&str>,
    json: bool,
) -> Result {
    let base_url = resolve_endpoint(endpoint, workspace)?;
    let infos = metric_info(&base_url, metric)?;
    if json {
        format::print_metric_info_json(&infos)?;
    } else {
        format::print_metric_info_table(&infos)?;
    }
    Ok(())
}

/// CLI entry point for `enya metrics series`.
pub fn metrics_series(
    endpoint: Option<&str>,
    workspace: Option<&str>,
    selector: &str,
    json: bool,
) -> Result {
    let base_url = resolve_endpoint(endpoint, workspace)?;
    let entries = query_series(&base_url, selector)?;
    if json {
        format::print_series_json(&entries)?;
    } else {
        format::print_series_table(&entries)?;
    }
    Ok(())
}

// -- Tests --------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_string_list_success() {
        let json =
            r#"{"status":"success","data":["cpu_usage","memory_usage","http_requests_total"]}"#;
        let resp: StringListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "success");
        assert_eq!(resp.data.len(), 3);
        assert_eq!(resp.data[0], "cpu_usage");
    }

    #[test]
    fn test_parse_string_list_error() {
        let json = r#"{"status":"error","error":"some error","data":[]}"#;
        let resp: StringListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "error");
        assert_eq!(resp.error.as_deref(), Some("some error"));
    }

    #[test]
    fn test_parse_string_list_empty() {
        let json = r#"{"status":"success","data":[]}"#;
        let resp: StringListResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
    }

    #[test]
    fn test_check_string_list_success() {
        let json = r#"{"status":"success","data":["a","b","c"]}"#;
        let result = check_string_list(json).unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_check_string_list_error() {
        let json = r#"{"status":"error","error":"bad request"}"#;
        let err = check_string_list(json).unwrap_err();
        assert!(err.to_string().contains("bad request"));
    }

    #[test]
    fn test_parse_series_response() {
        let json = r#"{
            "status":"success",
            "data":[
                {"__name__":"cpu_usage","env":"prod","host":"server1"},
                {"__name__":"cpu_usage","env":"staging","host":"server2"}
            ]
        }"#;
        let resp: SeriesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "success");
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].get("env").unwrap(), "prod");
    }

    #[test]
    fn test_parse_series_empty() {
        let json = r#"{"status":"success","data":[]}"#;
        let resp: SeriesResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
    }

    #[test]
    fn test_parse_metadata_response() {
        let json = r#"{
            "status":"success",
            "data":{
                "http_requests_total":[{"type":"counter","help":"Total HTTP requests","unit":""}],
                "cpu_usage":[{"type":"gauge","help":"CPU usage percentage","unit":""}]
            }
        }"#;
        let resp: MetadataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status, "success");
        assert_eq!(resp.data.len(), 2);
        let http_meta = &resp.data["http_requests_total"][0];
        assert_eq!(http_meta.metric_type, "counter");
        assert_eq!(http_meta.help, "Total HTTP requests");
    }

    #[test]
    fn test_parse_metadata_empty() {
        let json = r#"{"status":"success","data":{}}"#;
        let resp: MetadataResponse = serde_json::from_str(json).unwrap();
        assert!(resp.data.is_empty());
    }

    #[test]
    fn test_parse_metadata_with_unit() {
        let json = r#"{
            "status":"success",
            "data":{
                "request_duration_seconds":[{"type":"histogram","help":"Request duration","unit":"seconds"}]
            }
        }"#;
        let resp: MetadataResponse = serde_json::from_str(json).unwrap();
        let entry = &resp.data["request_duration_seconds"][0];
        assert_eq!(entry.unit, "seconds");
    }
}
