use console::style;
use enya_workspace::{WorkspaceConfig, list_workspaces, resolve_workspace_path};
use serde::Serialize;

use crate::Result;

// -- Result types -------------------------------------------------------------

#[derive(Serialize)]
pub struct CheckResult {
    pub workspace: String,
    pub config: CheckStatus,
    pub endpoint: Option<EndpointStatus>,
    pub queries: Vec<QueryStatus>,
}

#[derive(Serialize)]
pub struct CheckStatus {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct EndpointStatus {
    pub ok: bool,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct QueryStatus {
    pub query: String,
    pub section: String,
    pub pane: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CheckResult {
    /// Returns true if any hard errors were found (config or query failures).
    /// Endpoint unreachable is a warning, not an error.
    pub fn has_errors(&self) -> bool {
        !self.config.ok || self.queries.iter().any(|q| !q.ok)
    }
}

// -- Core function ------------------------------------------------------------

pub fn check_core(name: &str) -> CheckResult {
    let path = resolve_workspace_path(name);

    let ws = match WorkspaceConfig::load(&path) {
        Ok(ws) => ws,
        Err(e) => {
            return CheckResult {
                workspace: name.to_string(),
                config: CheckStatus {
                    ok: false,
                    error: Some(e.to_string()),
                },
                endpoint: None,
                queries: Vec::new(),
            };
        }
    };

    // Structural validation
    let config = match ws.validate() {
        Ok(()) => CheckStatus {
            ok: true,
            error: None,
        },
        Err(e) => CheckStatus {
            ok: false,
            error: Some(e.to_string()),
        },
    };

    // Query validation
    let mut queries = Vec::new();
    for section in &ws.sections {
        for pane in &section.panes {
            let result = enya_promql::validate(&pane.query);
            queries.push(QueryStatus {
                query: pane.query.clone(),
                section: section.name.clone(),
                pane: if pane.name.is_empty() {
                    pane.query.clone()
                } else {
                    pane.name.clone()
                },
                ok: result.is_valid,
                error: result.error,
            });
        }
    }

    // Endpoint check
    let endpoint = ws.effective_endpoint().map(check_endpoint);

    CheckResult {
        workspace: ws.workspace.name,
        config,
        endpoint,
        queries,
    }
}

fn check_endpoint(url: &str) -> EndpointStatus {
    let build_info_url = format!("{url}/api/v1/status/buildinfo");

    let response = match ureq::get(&build_info_url).call() {
        Ok(resp) => resp,
        Err(e) => {
            return EndpointStatus {
                ok: false,
                url: url.to_string(),
                version: None,
                error: Some(format!("{e}")),
            };
        }
    };

    let body = match response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("{e}"))
    {
        Ok(b) => b,
        Err(e) => {
            return EndpointStatus {
                ok: false,
                url: url.to_string(),
                version: None,
                error: Some(e),
            };
        }
    };

    let version = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["data"]["version"].as_str().map(String::from));

    EndpointStatus {
        ok: true,
        url: url.to_string(),
        version,
        error: None,
    }
}

// -- CLI wrapper --------------------------------------------------------------

pub fn check(name: Option<&str>, json: bool) -> Result<bool> {
    let results: Vec<CheckResult> = match name {
        Some(n) => vec![check_core(n)],
        None => {
            let workspaces = list_workspaces();
            if workspaces.is_empty() {
                return Err("no workspaces found".into());
            }
            workspaces.iter().map(|(n, _)| check_core(n)).collect()
        }
    };

    let has_errors = results.iter().any(|r| r.has_errors());

    if json {
        if results.len() == 1 {
            println!("{}", serde_json::to_string(&results[0])?);
        } else {
            println!("{}", serde_json::to_string(&results)?);
        }
        return Ok(has_errors);
    }

    for result in &results {
        print_check_result(result);
    }

    Ok(has_errors)
}

fn print_check_result(result: &CheckResult) {
    println!(
        "{}",
        style(format!("Checking {}...", result.workspace)).bold()
    );

    // Config
    if result.config.ok {
        println!("  {} {}", style("Config:").bold(), style("OK").green());
    } else {
        println!(
            "  {} {}  {}",
            style("Config:").bold(),
            style("FAIL").red().bold(),
            result.config.error.as_deref().unwrap_or("unknown error")
        );
    }

    // Endpoint
    match &result.endpoint {
        Some(ep) if ep.ok => {
            let version_str = ep
                .version
                .as_ref()
                .map(|v| format!(", {v}"))
                .unwrap_or_default();
            println!(
                "  {} {}  ({}{})",
                style("Endpoint:").bold(),
                style("OK").green(),
                ep.url,
                version_str
            );
        }
        Some(ep) => {
            println!(
                "  {} {}  {}",
                style("Endpoint:").bold(),
                style("WARN").yellow(),
                ep.error.as_deref().unwrap_or("unreachable")
            );
        }
        None => {
            println!(
                "  {} {}",
                style("Endpoint:").bold(),
                style("(not configured)").dim()
            );
        }
    }

    // Queries
    let total = result.queries.len();
    let invalid: Vec<&QueryStatus> = result.queries.iter().filter(|q| !q.ok).collect();

    if total == 0 {
        println!(
            "  {} {}",
            style("Queries:").bold(),
            style("(no panes)").dim()
        );
    } else if invalid.is_empty() {
        println!(
            "  {} {}  {}/{} valid",
            style("Queries:").bold(),
            style("OK").green(),
            total,
            total
        );
    } else {
        println!(
            "  {} {}  {}/{} invalid",
            style("Queries:").bold(),
            style("FAIL").red().bold(),
            invalid.len(),
            total
        );
        for q in &invalid {
            println!(
                "    - {} {} — {}",
                style(format!("[{}]", q.section)).dim(),
                q.query,
                q.error.as_deref().unwrap_or("invalid")
            );
        }
    }
}
