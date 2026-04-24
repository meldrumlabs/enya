//! Project and workspace directory discovery and listing (native only).

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::WorkspaceConfig;

/// Get the Enya data directory (`~/.enya/`).
///
/// Creates the directory if it doesn't exist.
pub fn enya_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(&home).join(".enya");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Get the projects directory (`~/.enya/projects/`).
///
/// Creates the directory if it doesn't exist.
pub fn projects_dir() -> PathBuf {
    let dir = enya_dir().join("projects");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Get the workspace directory for a project (`~/.enya/projects/{project}/workspaces/`).
///
/// Creates the directory if it doesn't exist.
pub fn project_workspace_dir(project: &str) -> PathBuf {
    let dir = projects_dir().join(project).join("workspaces");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Get the conversations directory for a workspace within a project.
///
/// Returns `~/.enya/projects/{project}/conversations/{workspace}/`.
/// Falls back to a `"default"` workspace key when none is provided.
/// Creates the directory if it doesn't exist.
pub fn project_conversations_dir(project: &str, workspace: Option<&str>) -> PathBuf {
    let key = workspace.unwrap_or("default");
    let dir = projects_dir().join(project).join("conversations").join(key);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Resolve a workspace name to a file path within a project.
///
/// Returns `~/.enya/projects/{project}/workspaces/{name}.toml`.
pub fn resolve_project_workspace_path(project: &str, name: &str) -> PathBuf {
    project_workspace_dir(project).join(format!("{name}.toml"))
}

/// List all projects (subdirectories of `~/.enya/projects/`).
///
/// Returns a sorted list of project names.
pub fn list_projects() -> Vec<String> {
    let dir = projects_dir();
    let mut projects = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    projects.push(name.to_string());
                }
            }
        }
    }

    projects.sort();
    projects
}

/// List available workspaces for a project.
///
/// Returns a sorted list of `(name, description)` tuples for each `.toml`
/// file found in the project's workspace directory.
pub fn list_project_workspaces(project: &str) -> Vec<(String, Option<String>)> {
    let dir = project_workspace_dir(project);
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

/// Create a project directory structure.
///
/// Creates `~/.enya/projects/{project}/workspaces/` (and conversations dir).
pub fn create_project_dir(project: &str) {
    let _ = std::fs::create_dir_all(project_workspace_dir(project));
}

/// Delete a project directory and all its contents.
pub fn delete_project_dir(project: &str) {
    let dir = projects_dir().join(project);
    let _ = std::fs::remove_dir_all(dir);
}

/// Get the Enya daemon config file path (`~/.enya/config.toml`).
pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(&home).join(".enya").join("config.toml")
}

/// Get the user plugins directory (`~/.enya/plugins/`).
///
/// Creates the directory if it doesn't exist.
pub fn plugins_dir() -> PathBuf {
    let dir = enya_dir().join("plugins");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Get the PR review sessions directory (`~/.enya/pr_sessions/`).
///
/// Creates the directory if it doesn't exist.
pub fn pr_sessions_dir() -> PathBuf {
    let dir = enya_dir().join("pr_sessions");
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
