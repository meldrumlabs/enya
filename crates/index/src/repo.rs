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
#[must_use]
pub fn repos_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".enya").join("repos"))
}

/// Extracts a repository name from a git URL.
///
/// # Examples
///
/// ```
/// use enya_index::repo::repo_name_from_url;
///
/// assert_eq!(repo_name_from_url("https://github.com/org/repo.git"), "repo");
/// assert_eq!(repo_name_from_url("git@github.com:org/repo.git"), "repo");
/// assert_eq!(repo_name_from_url("https://github.com/org/repo"), "repo");
/// ```
#[must_use]
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
///
/// # Errors
///
/// Returns an error if cloning fails.
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
///
/// # Errors
///
/// Returns an error if fetching fails.
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

/// Information about a git commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    /// Full git commit hash
    pub hash: String,
    /// Commit timestamp in Unix seconds
    pub timestamp: i64,
    /// Commit message (subject line)
    pub message: String,
}

/// Fetches commit history for a repository within a time range.
///
/// Returns commits between `start_secs` and `end_secs` (Unix timestamps).
/// Commits are returned in reverse chronological order (newest first).
///
/// # Errors
///
/// Returns an error if the git command fails.
pub fn fetch_commit_history(
    repo_path: &Path,
    start_secs: i64,
    end_secs: i64,
) -> Result<Vec<CommitInfo>, RepoError> {
    let output = Command::new("git")
        .args([
            "log",
            &format!("--after=@{start_secs}"),
            &format!("--before=@{end_secs}"),
            "--format=%H|%ct|%s",
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to run git log: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git log failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_git_log_output(&stdout)
}

/// Parses git log output in the format `hash|timestamp|message`.
fn parse_git_log_output(output: &str) -> Result<Vec<CommitInfo>, RepoError> {
    let mut commits = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Split on first two pipes only (message may contain pipes)
        let mut parts = line.splitn(3, '|');

        let hash = parts
            .next()
            .ok_or_else(|| RepoError("Missing hash in git log output".to_string()))?
            .to_string();

        let timestamp_str = parts
            .next()
            .ok_or_else(|| RepoError("Missing timestamp in git log output".to_string()))?;

        let timestamp = timestamp_str
            .parse::<i64>()
            .map_err(|e| RepoError(format!("Invalid timestamp '{timestamp_str}': {e}")))?;

        let message = parts.next().unwrap_or("").to_string();

        commits.push(CommitInfo {
            hash,
            timestamp,
            message,
        });
    }

    Ok(commits)
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

    #[test]
    fn test_parse_git_log_output_single_commit() {
        let output = "abc123def456|1700000000|Initial commit\n";
        let commits = parse_git_log_output(output).expect("should parse");
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].hash, "abc123def456");
        assert_eq!(commits[0].timestamp, 1_700_000_000);
        assert_eq!(commits[0].message, "Initial commit");
    }

    #[test]
    fn test_parse_git_log_output_multiple_commits() {
        let output = "\
abc123|1700000000|First commit
def456|1700001000|Second commit
ghi789|1700002000|Third commit
";
        let commits = parse_git_log_output(output).expect("should parse");
        assert_eq!(commits.len(), 3);
        assert_eq!(commits[0].hash, "abc123");
        assert_eq!(commits[1].hash, "def456");
        assert_eq!(commits[2].hash, "ghi789");
    }

    #[test]
    fn test_parse_git_log_output_message_with_pipes() {
        let output = "abc123|1700000000|Fix bug | add feature | cleanup\n";
        let commits = parse_git_log_output(output).expect("should parse");
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "Fix bug | add feature | cleanup");
    }

    #[test]
    fn test_parse_git_log_output_empty() {
        let output = "";
        let commits = parse_git_log_output(output).expect("should parse");
        assert!(commits.is_empty());
    }

    #[test]
    fn test_parse_git_log_output_whitespace_only() {
        let output = "  \n  \n  ";
        let commits = parse_git_log_output(output).expect("should parse");
        assert!(commits.is_empty());
    }

    #[test]
    fn test_parse_git_log_output_empty_message() {
        let output = "abc123|1700000000|\n";
        let commits = parse_git_log_output(output).expect("should parse");
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "");
    }

    #[test]
    fn test_commit_info_equality() {
        let c1 = CommitInfo {
            hash: "abc".to_string(),
            timestamp: 1000,
            message: "test".to_string(),
        };
        let c2 = CommitInfo {
            hash: "abc".to_string(),
            timestamp: 1000,
            message: "test".to_string(),
        };
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_commit_info_clone() {
        let c1 = CommitInfo {
            hash: "abc".to_string(),
            timestamp: 1000,
            message: "test".to_string(),
        };
        let c2 = c1.clone();
        assert_eq!(c1, c2);
    }
}
