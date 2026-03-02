//! Workspace directory discovery and listing (native only).

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

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

/// Get the user plugins directory (`~/.enya/plugins/`).
///
/// Creates the directory if it doesn't exist.
pub fn plugins_dir() -> PathBuf {
    let dir = enya_dir().join("plugins");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Get the search index directory for a repository (`~/.enya/indexes/{name}-{hash}/`).
///
/// Derives a stable, readable subdirectory name from the repo path using
/// the last path component plus a short hash of the full path.
///
/// Unlike other `*_dir()` helpers, this does **not** create the directory —
/// callers (e.g. Tantivy `create()`) are responsible for creating it so that
/// `open_or_create` can distinguish "no index yet" from "index exists".
pub fn index_dir(repo_path: &Path) -> PathBuf {
    let repo_name = repo_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");

    let mut hasher = DefaultHasher::new();
    repo_path.hash(&mut hasher);
    let hash = hasher.finish();

    let key = format!("{repo_name}-{hash:016x}");
    enya_dir().join("indexes").join(key)
}

/// Get the conversations directory for a workspace (`~/.enya/conversations/{key}/`).
///
/// Uses the workspace name as subdirectory key, falling back to `"default"`.
/// Creates the directory if it doesn't exist.
pub fn conversations_dir(workspace_name: Option<&str>) -> PathBuf {
    let key = workspace_name.unwrap_or("default");
    let dir = enya_dir().join("conversations").join(key);
    let _ = std::fs::create_dir_all(&dir);
    dir
}
