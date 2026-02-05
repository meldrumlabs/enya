//! Workspace directory discovery and listing (native only).

use std::path::PathBuf;

use crate::WorkspaceConfig;

/// Get the workspace directory path (`~/.enya/workspaces/`).
///
/// Creates the directory if it doesn't exist.
pub fn workspace_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(&home).join(".enya").join("workspaces");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Resolve a workspace name to a file path.
///
/// If the input is already a path to an existing file, returns it directly.
/// Otherwise, resolves the name in the workspace directory as `{name}.toml`.
pub fn resolve_workspace_path(name_or_path: &str) -> PathBuf {
    let path = PathBuf::from(name_or_path);
    if path.exists() {
        return path;
    }
    workspace_dir().join(format!("{name_or_path}.toml"))
}

/// List available workspaces from the workspace directory.
///
/// Returns a sorted list of `(name, description)` tuples for each `.toml`
/// file found in the workspace directory.
pub fn list_workspaces() -> Vec<(String, Option<String>)> {
    let dir = workspace_dir();
    let mut workspaces = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    let description = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|content| WorkspaceConfig::from_toml(&content).ok())
                        .and_then(|ws| {
                            if ws.workspace.description.is_empty() {
                                None
                            } else {
                                Some(ws.workspace.description)
                            }
                        });
                    workspaces.push((name.to_string(), description));
                }
            }
        }
    }

    workspaces.sort_by(|a, b| a.0.cmp(&b.0));
    workspaces
}

/// Get the Enya daemon config file path (`~/.enya/config.toml`).
pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(&home).join(".enya").join("config.toml")
}

/// Get the Enya data directory (`~/.enya/`).
///
/// Creates the directory if it doesn't exist.
pub fn enya_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(&home).join(".enya");
    let _ = std::fs::create_dir_all(&dir);
    dir
}
