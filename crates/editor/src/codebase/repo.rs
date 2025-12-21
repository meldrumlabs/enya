//! Git repository operations for codebase integration.
//!
//! Handles cloning repositories and fetching updates using system git commands.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Error type for repository operations.
#[derive(Debug)]
pub struct RepoError(pub String);

impl std::fmt::Display for RepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RepoError {}

impl From<std::io::Error> for RepoError {
    fn from(e: std::io::Error) -> Self {
        Self(format!("IO error: {e}"))
    }
}

/// Returns the directory where repositories are stored.
///
/// Uses `~/.enya/repos/` as the base directory.
pub fn repos_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".enya").join("repos"))
}

/// Extracts a repository name from a git URL.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(repo_name_from_url("https://github.com/org/repo.git"), "repo");
/// assert_eq!(repo_name_from_url("git@github.com:org/repo.git"), "repo");
/// assert_eq!(repo_name_from_url("https://github.com/org/repo"), "repo");
/// ```
pub fn repo_name_from_url(url: &str) -> String {
    // Remove trailing .git if present
    let url = url.strip_suffix(".git").unwrap_or(url);

    // Get the last path component
    url.rsplit('/')
        .next()
        .or_else(|| url.rsplit(':').next())
        .unwrap_or("repo")
        .to_string()
}

/// Clones a repository from the given URL.
///
/// The repository is cloned to `~/.enya/repos/<repo-name>/`.
/// Returns the path to the cloned repository.
pub fn clone_repo(url: &str) -> Result<PathBuf, RepoError> {
    let Some(base_dir) = repos_dir() else {
        return Err(RepoError("Could not determine home directory".to_string()));
    };

    // Ensure base directory exists
    std::fs::create_dir_all(&base_dir)?;

    let repo_name = repo_name_from_url(url);
    let repo_path = base_dir.join(&repo_name);

    // If repo already exists, just return the path
    if repo_path.exists() {
        return Ok(repo_path);
    }

    // Clone using git command
    let output = Command::new("git")
        .args(["clone", "--depth", "1", url])
        .arg(&repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to run git clone: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git clone failed: {stderr}")));
    }

    Ok(repo_path)
}

/// Fetches updates for an existing repository.
///
/// Returns `true` if there were remote changes.
pub fn fetch_updates(repo_path: &Path) -> Result<bool, RepoError> {
    // Get current HEAD
    let head_before = get_head_commit(repo_path)?;

    // Fetch from remote
    let output = Command::new("git")
        .args(["fetch", "origin"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to run git fetch: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git fetch failed: {stderr}")));
    }

    // Pull changes
    let output = Command::new("git")
        .args(["pull", "--ff-only"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to run git pull: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git pull failed: {stderr}")));
    }

    // Check if HEAD changed
    let head_after = get_head_commit(repo_path)?;
    Ok(head_before != head_after)
}

/// Gets the current HEAD commit hash.
fn get_head_commit(repo_path: &Path) -> Result<String, RepoError> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to run git rev-parse: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git rev-parse failed: {stderr}")));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_name_from_url() {
        assert_eq!(
            repo_name_from_url("https://github.com/org/repo.git"),
            "repo"
        );
        assert_eq!(repo_name_from_url("https://github.com/org/repo"), "repo");
        assert_eq!(
            repo_name_from_url("git@github.com:org/my-repo.git"),
            "my-repo"
        );
        assert_eq!(
            repo_name_from_url("https://gitlab.com/group/subgroup/project.git"),
            "project"
        );
    }
}
