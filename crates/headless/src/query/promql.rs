use enya_config::{WorkspaceConfig, resolve_workspace_path};
use serde::{Deserialize, Serialize};

use super::format;
use super::time;
use crate::Result;

// -- Prometheus response types ------------------------------------------------

#[derive(Deserialize)]
struct PromResponse {
    status: String,
    #[serde(default)]
    error: Option<String>,
    data: Option<PromData>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromData {
    pub result_type: String,
    pub result: Vec<PromResult>,
}

#[derive(Deserialize, Serialize)]
pub struct PromResult {
    pub metric: serde_json::Map<String, serde_json::Value>,
    /// Time-series values from range queries (matrix).
    #[serde(default)]
    pub values: Vec<(f64, String)>,
    /// Single value from instant queries (vector).
    pub value: Option<(f64, String)>,
}

impl PromData {
    /// Convert to a JSON value suitable for machine consumption.
    ///
    /// Handles both instant queries (single `value` per series) and
    /// range queries (array of `values` per series).
    pub fn to_json(&self) -> serde_json::Value {
        let series: Vec<serde_json::Value> = self
            .result
            .iter()
            .map(|r| {
                if let Some(ref val) = r.value {
                    serde_json::json!({
                        "metric": r.metric,
                        "value": {"timestamp": val.0, "value": val.1},
                    })
                } else {
                    let values: Vec<serde_json::Value> = r
                        .values
                        .iter()
                        .map(|(ts, v)| serde_json::json!({"timestamp": ts, "value": v}))
                        .collect();
                    serde_json::json!({
                        "metric": r.metric,
                        "values": values,
                    })
                }
            })
            .collect();
        serde_json::json!({
            "result_type": self.result_type,
            "series": series,
            "series_count": self.result.len(),
        })
    }
}

// -- Endpoint resolution ------------------------------------------------------

fn normalize_url(url: &str) -> String {
    let mut url = url.to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        url = format!("http://{url}");
    }
    if url.ends_with('/') {
        url.pop();
    }
    url
}

pub fn resolve_endpoint(endpoint: Option<&str>, workspace_name: Option<&str>) -> Result<String> {
    if let Some(ep) = endpoint {
        return Ok(normalize_url(ep));
    }
    if let Some(ws_name) = workspace_name {
        let path = resolve_workspace_path(ws_name);
        let ws = WorkspaceConfig::load(&path)?;
        if let Some(ep) = ws.effective_endpoint() {
            return Ok(normalize_url(ep));
        }
        return Err("workspace has no metrics endpoint configured".into());
    }
    if let Ok(ep) = std::env::var("ENYA_PROMETHEUS_URL") {
        return Ok(normalize_url(&ep));
    }
    Err("no endpoint specified (use --endpoint, --workspace, or set ENYA_PROMETHEUS_URL)".into())
}

// -- Query execution ----------------------------------------------------------

fn parse_prom_response(body: &str) -> Result<PromData> {
    let prom: PromResponse =
        serde_json::from_str(body).map_err(|e| format!("failed to parse response: {e}"))?;

    if prom.status != "success" {
        return Err(prom.error.unwrap_or_else(|| "unknown error".into()).into());
    }

    prom.data.ok_or_else(|| "no data in response".into())
}

/// Execute a range PromQL query and return raw data.
pub fn query_range(
    base_url: &str,
    expression: &str,
    start_secs: u64,
    end_secs: u64,
    step_secs: u64,
) -> Result<PromData> {
    let url = format!("{base_url}/api/v1/query_range");

    let response = ureq::get(&url)
        .query("query", expression)
        .query("start", start_secs.to_string())
        .query("end", end_secs.to_string())
        .query("step", step_secs.to_string())
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let body: String = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("failed to read response: {e}"))?;

    parse_prom_response(&body)
}

#[allow(clippy::too_many_arguments)]
pub fn query(
    expression: &str,
    endpoint: Option<&str>,
    workspace_name: Option<&str>,
    start: &str,
    end: &str,
    step: &str,
    limit: Option<usize>,
    json: bool,
) -> Result {
    let base_url = resolve_endpoint(endpoint, workspace_name)?;
    let now = time::now_secs();
    let start_secs = time::parse_time(start, now)?;
    let end_secs = time::parse_time(end, now)?;
    let step_secs = time::parse_duration_secs(step)?;

    let data = query_range(&base_url, expression, start_secs, end_secs, step_secs)?;

    if json {
        format::print_promql_json(&data, limit)?;
    } else {
        format::print_promql_table(&data, limit)?;
    }

    Ok(())
}

/// Execute an instant PromQL query and return raw data.
///
/// Uses `/api/v1/query` (not `query_range`) to get the current value.
pub fn query_instant(base_url: &str, expression: &str) -> Result<PromData> {
    let url = format!("{base_url}/api/v1/query");
    let now = time::now_secs();

    let response = ureq::get(&url)
        .query("query", expression)
        .query("time", now.to_string())
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let body: String = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("failed to read response: {e}"))?;

    parse_prom_response(&body)
}
