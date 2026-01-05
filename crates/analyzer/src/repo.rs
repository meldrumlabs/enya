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
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".enya").join("repos"))
}

/// Extracts a repository name from a git URL.
///
/// # Examples
///
/// ```
/// use enya_analyzer::repo::repo_name_from_url;
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

    // If repo already exists, ensure it has full history then return
    if repo_path.exists() {
        unshallow_if_needed(&repo_path)?;
        return Ok(repo_path);
    }

    // Clone using git command (full history for commit indexing)
    let output = Command::new("git")
        .args(["clone", url])
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

/// Converts a shallow clone to full history if needed.
///
/// This enables commit indexing on repositories that were previously cloned
/// with `--depth 1`.
fn unshallow_if_needed(repo_path: &Path) -> Result<(), RepoError> {
    // Check if this is a shallow clone
    let output = Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to check if shallow: {e}")))?;

    let is_shallow = String::from_utf8_lossy(&output.stdout)
        .trim()
        .eq_ignore_ascii_case("true");

    if !is_shallow {
        return Ok(());
    }

    log::info!(
        "Converting shallow clone to full history: {}",
        repo_path.display()
    );

    // Fetch full history
    let output = Command::new("git")
        .args(["fetch", "--unshallow"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to unshallow repository: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git fetch --unshallow failed: {stderr}")));
    }

    log::info!("Successfully unshallowed repository");
    Ok(())
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommitInfo {
    /// Full git commit hash
    pub hash: String,
    /// Commit timestamp in Unix seconds
    pub timestamp: i64,
    /// Commit message (subject line)
    pub message: String,
    /// Files changed in this commit (relative paths)
    pub files_changed: Vec<String>,
    /// Raw diff content (truncated if too large)
    pub diff: String,
    /// Semantic information extracted from the diff
    pub semantics: DiffSemantics,
}

/// Semantic information extracted from a diff using Tree-sitter.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiffSemantics {
    /// Function names that were added
    pub functions_added: Vec<String>,
    /// Function names that were removed
    pub functions_removed: Vec<String>,
    /// Function names that were modified (had changes in their body)
    pub functions_modified: Vec<String>,
    /// Metric names that were added or modified (e.g., `counter.inc()`, `histogram.observe()`)
    pub metrics_added: Vec<String>,
    /// Metric names that were removed
    pub metrics_removed: Vec<String>,
    /// Import statements added
    pub imports_added: Vec<String>,
    /// Import statements removed
    pub imports_removed: Vec<String>,
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

/// Fetches recent commits for indexing purposes.
///
/// Returns up to `limit` commits in reverse chronological order (newest first).
/// This is used for building the search index, not for time-range queries.
/// Includes the list of files changed in each commit.
///
/// # Errors
///
/// Returns an error if the git command fails.
pub fn fetch_recent_commits(repo_path: &Path, limit: usize) -> Result<Vec<CommitInfo>, RepoError> {
    // Use --name-only to get files changed, with a record separator to parse
    // Format: hash|timestamp|message\n\nfile1\nfile2\n\n (commits separated by empty line)
    let output = Command::new("git")
        .args([
            "log",
            &format!("-{limit}"),
            "--format=%H|%ct|%s",
            "--name-only",
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to run git log: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git log failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_git_log_with_files(&stdout))
}

/// Maximum diff size to store per commit (64KB).
/// Larger diffs are truncated to avoid bloating the index.
const MAX_DIFF_SIZE: usize = 64 * 1024;

/// Fetches the diff for a single commit.
///
/// Returns the unified diff output for the commit. Large diffs are truncated
/// to `MAX_DIFF_SIZE` bytes.
///
/// # Errors
///
/// Returns an error if the git command fails.
pub fn fetch_commit_diff(repo_path: &Path, commit_hash: &str) -> Result<String, RepoError> {
    let output = Command::new("git")
        .args([
            "show",
            commit_hash,
            "--format=",   // Skip the commit message header
            "--unified=3", // 3 lines of context
            "-p",          // Show patch
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to run git show: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git show failed: {stderr}")));
    }

    let diff = String::from_utf8_lossy(&output.stdout);

    // Truncate if too large
    if diff.len() > MAX_DIFF_SIZE {
        Ok(format!(
            "{}\n\n[... diff truncated, {} bytes total ...]",
            &diff[..MAX_DIFF_SIZE],
            diff.len()
        ))
    } else {
        Ok(diff.into_owned())
    }
}

/// Fetches recent commits with their diffs for full indexing.
///
/// This is more expensive than `fetch_recent_commits` as it fetches the
/// full diff for each commit. Use sparingly for indexing purposes.
///
/// # Errors
///
/// Returns an error if the git command fails.
pub fn fetch_recent_commits_with_diffs(
    repo_path: &Path,
    limit: usize,
) -> Result<Vec<CommitInfo>, RepoError> {
    // First get the basic commit info
    let mut commits = fetch_recent_commits(repo_path, limit)?;

    // Then fetch diffs for each commit and extract semantics
    for commit in &mut commits {
        match fetch_commit_diff(repo_path, &commit.hash) {
            Ok(diff) => {
                // Extract semantic information from the diff
                commit.semantics = crate::diff::extract_semantics(&diff);
                commit.diff = diff;
            }
            Err(e) => {
                log::warn!("Failed to fetch diff for {}: {e}", &commit.hash[..8]);
                // Continue without diff - better to have partial data
            }
        }
    }

    Ok(commits)
}

/// Parses git log output in the format `hash|timestamp|message` (no files).
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
            ..Default::default()
        });
    }

    Ok(commits)
}

/// Parses git log output with `--name-only` format.
///
/// Format: Each commit starts with `hash|timestamp|message` followed by
/// a blank line, then the list of files (one per line), then another blank line.
fn parse_git_log_with_files(output: &str) -> Vec<CommitInfo> {
    let mut commits = Vec::new();
    let mut current_commit: Option<CommitInfo> = None;

    for line in output.lines() {
        let line = line.trim();

        // Check if this line is a commit header (contains hash|timestamp|message)
        if line.contains('|') && line.len() >= 40 {
            // This looks like a commit header line
            let mut parts = line.splitn(3, '|');

            let hash = parts.next().unwrap_or("");
            let timestamp_str = parts.next().unwrap_or("");
            let message = parts.next().unwrap_or("");

            // Validate it looks like a hash (40 hex chars)
            if hash.len() >= 40 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                // Save previous commit if any
                if let Some(commit) = current_commit.take() {
                    commits.push(commit);
                }

                let timestamp = timestamp_str.parse::<i64>().unwrap_or(0);

                current_commit = Some(CommitInfo {
                    hash: hash.to_string(),
                    timestamp,
                    message: message.to_string(),
                    ..Default::default()
                });
                continue;
            }
        }

        // Empty line or non-header line
        if line.is_empty() {
            continue;
        }

        // This is a file path - add to current commit
        if let Some(ref mut commit) = current_commit {
            commit.files_changed.push(line.to_string());
        }
    }

    // Don't forget the last commit
    if let Some(commit) = current_commit {
        commits.push(commit);
    }

    commits
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
            files_changed: vec!["file.rs".to_string()],
            ..Default::default()
        };
        let c2 = CommitInfo {
            hash: "abc".to_string(),
            timestamp: 1000,
            message: "test".to_string(),
            files_changed: vec!["file.rs".to_string()],
            ..Default::default()
        };
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_commit_info_clone() {
        let c1 = CommitInfo {
            hash: "abc".to_string(),
            timestamp: 1000,
            message: "test".to_string(),
            files_changed: vec!["file.rs".to_string()],
            ..Default::default()
        };
        let c2 = c1.clone();
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_parse_git_log_with_files() {
        // Git hashes are always 40 hex chars
        let output = "\
abc123def456789012345678901234567890abcd|1700000000|Add executor

src/executor.rs
src/lib.rs

def456789012345678901234567890abcdef12ab|1700001000|Fix bug

src/main.rs
";
        let commits = parse_git_log_with_files(output);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].hash, "abc123def456789012345678901234567890abcd");
        assert_eq!(commits[0].message, "Add executor");
        assert_eq!(
            commits[0].files_changed,
            vec!["src/executor.rs", "src/lib.rs"]
        );
        assert_eq!(commits[1].hash, "def456789012345678901234567890abcdef12ab");
        assert_eq!(commits[1].message, "Fix bug");
        assert_eq!(commits[1].files_changed, vec!["src/main.rs"]);
    }

    #[test]
    fn test_parse_git_log_with_files_empty() {
        let output = "";
        let commits = parse_git_log_with_files(output);
        assert!(commits.is_empty());
    }

    #[test]
    fn test_parse_git_log_with_files_no_files() {
        // A commit with no file changes
        let output = "abc123def456789012345678901234567890abcd|1700000000|Empty commit\n";
        let commits = parse_git_log_with_files(output);
        assert_eq!(commits.len(), 1);
        assert!(commits[0].files_changed.is_empty());
    }
}
