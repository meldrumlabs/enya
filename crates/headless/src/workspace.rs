use console::style;
use enya_config::{
    PaneConfig, WorkspaceConfig, list_project_workspaces, list_projects,
    resolve_project_workspace_path,
};

use serde::Serialize;

use crate::Result;
use crate::query::{promql, time};

/// Resolve a workspace path: if `name` is an absolute path (or ends with
/// `.toml`), treat it as a direct file path; otherwise look it up inside the
/// project directory.
fn resolve_workspace_path(name: &str, project: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(name);
    if p.is_absolute() || name.ends_with(".toml") {
        p.to_path_buf()
    } else {
        resolve_project_workspace_path(project, name)
    }
}

// -- Result types -------------------------------------------------------------

#[derive(Serialize)]
pub struct InitResult {
    pub name: String,
    pub project: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct ListResult {
    pub projects: Vec<ProjectListEntry>,
}

#[derive(Serialize)]
pub struct ProjectListEntry {
    pub name: String,
    pub workspaces: Vec<WorkspaceEntry>,
}

#[derive(Serialize)]
pub struct WorkspaceEntry {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct RmResult {
    pub removed: String,
}

#[derive(Serialize)]
pub struct GetResult {
    pub workspace: String,
    pub key: String,
    pub value: String,
}

#[derive(Serialize)]
pub struct SetResult {
    pub workspace: String,
    pub key: String,
    pub value: String,
}

#[derive(Serialize)]
pub struct AddPaneResult {
    pub workspace: String,
    pub pane: String,
    pub query: String,
}

#[derive(Serialize)]
pub struct RemovePaneResult {
    pub workspace: String,
    pub removed_pane: String,
}

/// Parameters for adding a pane to a workspace.
pub struct AddPaneParams<'a> {
    pub name: &'a str,
    pub query: &'a str,
    pub pane_name: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub unit: Option<&'a str>,
    pub granularity: Option<&'a str>,
    pub visualization: Option<&'a str>,
    pub description: Option<&'a str>,
}

// -- Template resolution ------------------------------------------------------
/// Resolve a template name to its TOML content.
pub fn resolve_template(template: &str) -> Result<&'static str> {
    match template {
        "golden-signals" | "default" => Ok(enya_config::GOLDEN_SIGNALS_TOML),
        "infrastructure" => Ok(enya_config::INFRASTRUCTURE_TOML),
        "multi-service" => Ok(enya_config::MULTI_SERVICE_TOML),
        _ => Err(format!(
            "unknown template: {template} (available: golden-signals, infrastructure, multi-service)"
        )
        .into()),
    }
}

// -- Core functions (return data, no printing) --------------------------------

pub fn init_core(
    name: Option<String>,
    endpoint: Option<&str>,
    template: Option<&str>,
    output: Option<&str>,
    project: Option<&str>,
) -> Result<InitResult> {
    let name = name.unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "workspace".to_string())
    });

    let path = match output {
        Some(o) => std::path::PathBuf::from(o),
        None => {
            let project = project.ok_or("--project is required when not using --output")?;
            resolve_project_workspace_path(project, &name)
        }
    };

    if path.exists() {
        return Err(format!("{} already exists", path.display()).into());
    }

    let ws = match template {
        Some(t) => {
            let toml_str = resolve_template(t)?;
            let mut ws = WorkspaceConfig::from_toml(toml_str)?;
            ws.workspace.name = name.clone();
            if let Some(ep) = endpoint {
                ws.workspace.endpoint = ep.to_string();
            }
            ws
        }
        None => match endpoint {
            Some(ep) => WorkspaceConfig::with_endpoint(&name, ep),
            None => WorkspaceConfig::new(&name),
        },
    };

    ws.save(&path)?;
    Ok(InitResult {
        name,
        project: project.unwrap_or("").to_string(),
        path: path.display().to_string(),
    })
}

pub fn list_core() -> ListResult {
    let projects = list_projects();
    ListResult {
        projects: projects
            .into_iter()
            .map(|project_name| {
                let workspaces = list_project_workspaces(&project_name);
                ProjectListEntry {
                    name: project_name,
                    workspaces: workspaces
                        .into_iter()
                        .map(|(name, description)| WorkspaceEntry { name, description })
                        .collect(),
                }
            })
            .collect(),
    }
}

pub fn show_core(name: &str, project: &str) -> Result<WorkspaceConfig> {
    let path = resolve_workspace_path(name, project);
    Ok(WorkspaceConfig::load(&path)?)
}

pub fn rm_core(name: &str, project: &str) -> Result<RmResult> {
    let path = resolve_workspace_path(name, project);
    if !path.exists() {
        return Err(format!("workspace not found: {}", path.display()).into());
    }
    std::fs::remove_file(&path)?;
    Ok(RmResult {
        removed: path.display().to_string(),
    })
}

pub fn get_core(name: &str, key: &str, project: &str) -> Result<GetResult> {
    let path = resolve_workspace_path(name, project);
    let ws = WorkspaceConfig::load(&path)?;
    let value = ws.get_value(key)?;
    Ok(GetResult {
        workspace: ws.workspace.name,
        key: key.to_string(),
        value,
    })
}

pub fn set_core(name: &str, key: &str, value: &str, project: &str) -> Result<SetResult> {
    let path = resolve_workspace_path(name, project);
    let mut ws = WorkspaceConfig::load(&path)?;
    ws.set_value(key, value)?;
    ws.save(&path)?;
    Ok(SetResult {
        workspace: ws.workspace.name,
        key: key.to_string(),
        value: value.to_string(),
    })
}

pub fn add_pane_core(params: &AddPaneParams<'_>, project: &str) -> Result<AddPaneResult> {
    let path = resolve_workspace_path(params.name, project);
    let mut ws = WorkspaceConfig::load(&path)?;

    let mut pane = PaneConfig::new(params.query);
    if let Some(n) = params.pane_name {
        pane.name = n.to_string();
    }
    if let Some(t) = params.tag {
        pane.tag = t.to_string();
    }
    if let Some(u) = params.unit {
        pane.unit = u.to_string();
    }
    if let Some(g) = params.granularity {
        pane.granularity = g.to_string();
    }
    if let Some(v) = params.visualization {
        pane.visualization = v.to_string();
    }
    if let Some(d) = params.description {
        pane.description = d.to_string();
    }

    ws.panes.push(pane);
    ws.save(&path)?;

    Ok(AddPaneResult {
        workspace: ws.workspace.name,
        pane: params.pane_name.unwrap_or("").to_string(),
        query: params.query.to_string(),
    })
}

pub fn remove_pane_core(name: &str, pane: &str, project: &str) -> Result<RemovePaneResult> {
    let path = resolve_workspace_path(name, project);
    let mut ws = WorkspaceConfig::load(&path)?;

    let idx = ws
        .panes
        .iter()
        .position(|p| p.name == pane)
        .ok_or_else(|| format!("pane not found: {pane}"))?;

    ws.panes.remove(idx);
    ws.save(&path)?;

    Ok(RemovePaneResult {
        workspace: ws.workspace.name,
        removed_pane: pane.to_string(),
    })
}

// -- CLI wrappers (call core + format output) ---------------------------------

pub fn init(
    name: Option<String>,
    endpoint: Option<&str>,
    template: Option<&str>,
    output: Option<&str>,
    project: Option<&str>,
    json: bool,
) -> Result {
    let result = init_core(name, endpoint, template, output, project)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!(
            "{} {} (project: {})",
            style("Created").green(),
            result.path,
            result.project
        );
    }
    Ok(())
}

pub fn list(json: bool) -> Result {
    let result = list_core();

    if json {
        println!("{}", serde_json::to_string(&result)?);
        return Ok(());
    }

    if result.projects.is_empty() {
        println!("No projects found");
        return Ok(());
    }

    for project in &result.projects {
        println!("{}", style(format!("{}:", project.name)).bold());
        if project.workspaces.is_empty() {
            println!("  (no workspaces)");
        } else {
            for entry in &project.workspaces {
                match &entry.description {
                    Some(desc) => {
                        println!("  {:20} {}", style(&entry.name).bold(), style(desc).dim())
                    }
                    None => println!("  {}", style(&entry.name).bold()),
                }
            }
        }
        println!();
    }
    Ok(())
}

pub fn show(name: &str, project: &str, json: bool) -> Result {
    let ws = show_core(name, project)?;

    if json {
        println!("{}", serde_json::to_string(&ws)?);
        return Ok(());
    }

    println!("{} {}", style("Name:").bold(), ws.workspace.name);
    if !ws.workspace.description.is_empty() {
        println!(
            "{} {}",
            style("Description:").bold(),
            ws.workspace.description
        );
    }
    if let Some(ep) = ws.effective_endpoint() {
        println!("{} {ep}", style("Endpoint:").bold());
    }
    if !ws.logs.is_empty() {
        println!("{} {}", style("Logs:").bold(), ws.logs.endpoint);
    }
    if !ws.git.is_empty() {
        println!("{} {}", style("Git:").bold(), ws.git.url);
    }

    println!("{} {}", style("Theme:").bold(), ws.view.theme);
    println!("{} {}", style("Time:").bold(), ws.time.preset);

    if !ws.panes.is_empty() {
        println!("{} {}", style("Panes:").bold(), ws.panes.len());
        for (i, pane) in ws.panes.iter().enumerate() {
            let label = if pane.name.is_empty() {
                format!("pane {i}")
            } else {
                pane.name.clone()
            };
            println!(
                "  {} {label}: {}",
                style(format!("[{i}]")).dim(),
                pane.query
            );
        }
    }

    Ok(())
}

pub fn rm(name: &str, project: &str, json: bool) -> Result {
    let result = rm_core(name, project)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{} {}", style("Removed").green(), result.removed);
    }
    Ok(())
}

pub fn get(name: &str, key: &str, project: &str, json: bool) -> Result {
    let result = get_core(name, key, project)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{}", result.value);
    }
    Ok(())
}

pub fn set(name: &str, key: &str, value: &str, project: &str, json: bool) -> Result {
    let result = set_core(name, key, value, project)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("{} = {}", style(&result.key).bold(), result.value);
    }
    Ok(())
}

pub fn add_pane(params: &AddPaneParams<'_>, project: &str, json: bool) -> Result {
    let result = add_pane_core(params, project)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        let label = params.pane_name.unwrap_or(params.query);
        println!("{} pane \"{label}\"", style("Added").green(),);
    }
    Ok(())
}

pub fn remove_pane(name: &str, pane: &str, project: &str, json: bool) -> Result {
    let result = remove_pane_core(name, pane, project)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!(
            "{} pane \"{}\"",
            style("Removed").green(),
            result.removed_pane
        );
    }
    Ok(())
}

// -- Snapshot -----------------------------------------------------------------

/// Choose a reasonable query step/resolution based on the time range.
fn compute_step(range_secs: u64) -> u64 {
    match range_secs {
        0..=900 => 15,        // ≤15m → 15s step
        901..=3600 => 60,     // ≤1h  → 1m step
        3601..=21600 => 300,  // ≤6h  → 5m step
        21601..=86400 => 900, // ≤1d  → 15m step
        _ => 3600,            // >1d  → 1h step
    }
}

/// Capture a point-in-time snapshot of all pane query results in a workspace.
///
/// For each pane, executes the query as a range query over the workspace's
/// configured time preset and returns the results as a self-contained JSON blob.
pub fn snapshot(base_url: &str, ws: &WorkspaceConfig) -> Result<serde_json::Value> {
    let now = time::now_secs();
    let preset = &ws.time.preset;
    let range_secs = time::parse_duration_secs(if preset.is_empty() { "1h" } else { preset })?;
    let start_secs = now.saturating_sub(range_secs);
    let step_secs = compute_step(range_secs);

    let mut pane_results: Vec<serde_json::Value> = Vec::new();

    for pane in &ws.panes {
        let result = match promql::query_range(base_url, &pane.query, start_secs, now, step_secs) {
            Ok(data) => serde_json::to_value(&data)
                .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})),
            Err(e) => serde_json::json!({"error": e.to_string()}),
        };

        pane_results.push(serde_json::json!({
            "name": pane.name,
            "query": pane.query,
            "result": result,
        }));
    }

    Ok(serde_json::json!({
        "version": 1,
        "captured_at": now,
        "captured_at_human": time::format_timestamp(now as f64),
        "time_range": {
            "start": start_secs,
            "end": now,
            "step": step_secs,
            "preset": preset,
        },
        "workspace": serde_json::to_value(ws)?,
        "panes": pane_results,
    }))
}

/// CLI entry point for `enya snapshot`.
pub fn snapshot_cmd(
    name: &str,
    endpoint: Option<&str>,
    output: Option<&str>,
    project: &str,
    json: bool,
) -> Result {
    let path = resolve_workspace_path(name, project);
    let ws = WorkspaceConfig::load(&path)?;

    let base_url = promql::resolve_endpoint(endpoint, Some(name))?;

    let snap = snapshot(&base_url, &ws)?;

    if let Some(out_path) = output {
        let contents = serde_json::to_string_pretty(&snap)?;
        std::fs::write(out_path, contents)?;
        if json {
            println!("{}", serde_json::json!({"path": out_path}));
        } else {
            let pane_count = snap["panes"].as_array().map(|a| a.len()).unwrap_or(0);
            println!("Snapshot written to {out_path} ({pane_count} panes)");
        }
    } else if json {
        println!("{}", serde_json::to_string(&snap)?);
    } else {
        println!("{}", serde_json::to_string_pretty(&snap)?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_step() {
        assert_eq!(compute_step(300), 15); // 5m range → 15s step
        assert_eq!(compute_step(900), 15); // 15m range → 15s step
        assert_eq!(compute_step(3600), 60); // 1h range → 1m step
        assert_eq!(compute_step(21600), 300); // 6h range → 5m step
        assert_eq!(compute_step(86400), 900); // 1d range → 15m step
        assert_eq!(compute_step(604800), 3600); // 7d range → 1h step
    }

    #[test]
    fn test_resolve_template() {
        assert!(resolve_template("default").is_ok());
        assert!(resolve_template("golden-signals").is_ok());
        assert!(resolve_template("infrastructure").is_ok());
        assert!(resolve_template("multi-service").is_ok());
        assert!(resolve_template("nonexistent").is_err());
    }
}
