use console::style;
use enya_config::{
    WorkspaceConfig, list_project_workspaces, list_projects, resolve_project_workspace_path,
};
use serde::Serialize;

use crate::Result;

// -- Result types -------------------------------------------------------------

#[derive(Serialize)]
pub struct FmtResult {
    pub workspace: String,
    pub path: String,
    pub changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// -- Core function ------------------------------------------------------------

pub fn fmt_core(name: &str, project: &str) -> FmtResult {
    let path = resolve_project_workspace_path(project, name);
    let path_str = path.display().to_string();

    let original = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            return FmtResult {
                workspace: name.to_string(),
                path: path_str,
                changed: false,
                error: Some(e.to_string()),
            };
        }
    };

    let ws = match WorkspaceConfig::load(&path) {
        Ok(ws) => ws,
        Err(e) => {
            return FmtResult {
                workspace: name.to_string(),
                path: path_str,
                changed: false,
                error: Some(e.to_string()),
            };
        }
    };

    let formatted = match ws.to_toml() {
        Ok(s) => s,
        Err(e) => {
            return FmtResult {
                workspace: ws.workspace.name,
                path: path_str,
                changed: false,
                error: Some(e.to_string()),
            };
        }
    };

    let changed = formatted != original;

    if changed {
        if let Err(e) = std::fs::write(&path, &formatted) {
            return FmtResult {
                workspace: ws.workspace.name,
                path: path_str,
                changed: false,
                error: Some(e.to_string()),
            };
        }
    }

    FmtResult {
        workspace: ws.workspace.name,
        path: path_str,
        changed,
        error: None,
    }
}

// -- CLI wrapper --------------------------------------------------------------

pub fn fmt(name: Option<&str>, json: bool) -> Result {
    let results: Vec<FmtResult> = match name {
        Some(n) => {
            let mut found = false;
            let mut results = Vec::new();
            for project in list_projects() {
                if list_project_workspaces(&project)
                    .iter()
                    .any(|(name, _)| name == n)
                {
                    results.push(fmt_core(n, &project));
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(format!("workspace not found: {n}").into());
            }
            results
        }
        None => {
            let mut all = Vec::new();
            for project in list_projects() {
                for (ws_name, _) in list_project_workspaces(&project) {
                    all.push(fmt_core(&ws_name, &project));
                }
            }
            if all.is_empty() {
                return Err("no workspaces found".into());
            }
            all
        }
    };

    if json {
        if results.len() == 1 {
            println!("{}", serde_json::to_string(&results[0])?);
        } else {
            println!("{}", serde_json::to_string(&results)?);
        }
        return Ok(());
    }

    let mut had_errors = false;
    for result in &results {
        if let Some(err) = &result.error {
            println!(
                "  {} {}  {}",
                style(&result.workspace).bold(),
                style("ERROR").red(),
                err
            );
            had_errors = true;
        } else if result.changed {
            println!(
                "  {} {}",
                style(&result.workspace).bold(),
                style("formatted").green()
            );
        } else {
            println!(
                "  {} {}",
                style(&result.workspace).bold(),
                style("unchanged").dim()
            );
        }
    }

    if had_errors {
        return Err("some workspaces could not be formatted".into());
    }

    Ok(())
}
