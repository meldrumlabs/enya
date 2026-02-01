use enya_workspace::{WorkspaceConfig, resolve_workspace_path};
use serde::Deserialize;

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PromData {
    pub result_type: String,
    pub result: Vec<PromResult>,
}

#[derive(Deserialize)]
pub(super) struct PromResult {
    pub metric: serde_json::Map<String, serde_json::Value>,
    pub values: Vec<(f64, String)>,
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

fn resolve_endpoint(endpoint: Option<&str>, workspace_name: Option<&str>) -> Result<String> {
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

    let prom: PromResponse =
        serde_json::from_str(&body).map_err(|e| format!("failed to parse response: {e}"))?;

    if prom.status != "success" {
        return Err(prom.error.unwrap_or_else(|| "unknown error".into()).into());
    }

    let data = prom.data.ok_or("no data in response")?;

    if json {
        format::print_promql_json(&data, limit)?;
    } else {
        format::print_promql_table(&data, limit)?;
    }

    Ok(())
}
