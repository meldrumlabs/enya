use console::style;
use enya_config::{WorkspaceConfig, list_workspaces, resolve_workspace_path};
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

pub fn fmt_core(name: &str) -> FmtResult {
    let path = resolve_workspace_path(name);
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
        Some(n) => vec![fmt_core(n)],
        None => {
            let workspaces = list_workspaces();
            if workspaces.is_empty() {
                return Err("no workspaces found".into());
            }
            workspaces.iter().map(|(n, _)| fmt_core(n)).collect()
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
