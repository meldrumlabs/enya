//! Git repository operations for codebase integration.
//!
//! Handles cloning repositories and fetching updates using system git commands.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;

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

/// Counts the total number of commits in the repository.
///
/// This is a fast operation that doesn't fetch commit data.
///
/// # Errors
///
/// Returns an error if the git command fails.
pub fn count_commits(repo_path: &Path) -> Result<usize, RepoError> {
    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to run git rev-list --count: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git rev-list --count failed: {stderr}")));
    }

    let count_str = String::from_utf8_lossy(&output.stdout);
    count_str
        .trim()
        .parse::<usize>()
        .map_err(|e| RepoError(format!("Failed to parse commit count: {e}")))
}

/// Fetches all commits for indexing purposes.
///
/// Returns all commits in reverse chronological order (newest first).
/// This is used for building the complete search index.
/// Includes the list of files changed in each commit.
///
/// # Errors
///
/// Returns an error if the git command fails.
pub fn fetch_all_commits(repo_path: &Path) -> Result<Vec<CommitInfo>, RepoError> {
    // Use --name-only to get files changed, with a record separator to parse
    // Format: hash|timestamp|message\n\nfile1\nfile2\n\n (commits separated by empty line)
    let output = Command::new("git")
        .args(["log", "--format=%H|%ct|%s", "--name-only"])
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

/// Progress callback for commit fetching.
///
/// Called periodically during diff fetching with:
/// - `current`: Number of commits processed so far
/// - `total`: Total number of commits to process
/// - `current_item`: Description of a recently processed commit
pub type ProgressCallback = Box<dyn Fn(usize, usize, Option<&str>) + Send + Sync>;

/// Unique delimiter used to separate commits in batch git log output.
const COMMIT_DELIMITER: &str = "\n__ENYA_COMMIT_BOUNDARY__\n";

/// Fetches all commits with their diffs in a single git command (batch mode).
///
/// This is significantly faster than fetching diffs individually because it
/// avoids the overhead of spawning many git processes. Uses a single `git log -p`
/// command to get all commits and diffs, then parses the output.
///
/// **Note:** This function skips merge commits (`--no-merges`) and only follows
/// the main branch history (`--first-parent`), excluding feature branch commits.
/// This reduces redundant diff processing and focuses on the mainline history.
///
/// Semantic extraction is still parallelized with rayon for CPU efficiency.
///
/// # Arguments
///
/// * `repo_path` - Path to the git repository
/// * `since_commit` - If provided, only fetch commits after this SHA (for incremental indexing)
/// * `progress` - Optional progress callback for UI updates
///
/// # Returns
///
/// A vector of `CommitInfo` with diffs and semantics populated.
///
/// # Errors
///
/// Returns an error if the git command fails.
pub fn fetch_all_commits_with_diffs_batch(
    repo_path: &Path,
    since_commit: Option<&str>,
    progress: Option<&ProgressCallback>,
) -> Result<Vec<CommitInfo>, RepoError> {
    // Build the git log command with patches
    // Format: delimiter + hash|timestamp|subject + newline + diff
    let format_arg = format!("{}%H|%ct|%s", COMMIT_DELIMITER.trim_start());

    let mut args = vec![
        "log".to_string(),
        format!("--format={format_arg}"),
        "-p".to_string(),             // Include patches (diffs)
        "--unified=3".to_string(),    // 3 lines of context
        "--no-merges".to_string(),    // Skip merge commits (large, redundant diffs)
        "--first-parent".to_string(), // Follow only main branch, skip feature branch commits
    ];
    // NOTE: Do NOT use --name-only with -p, it overrides patch output!

    // For incremental indexing: only get commits since the last indexed one
    if let Some(since) = since_commit {
        args.push(format!("{since}..HEAD"));
    }

    log::info!(
        "Fetching commits with diffs in batch mode from: {}{}",
        repo_path.display(),
        since_commit.map_or(String::new(), |s| format!(
            " (since {})",
            &s[..7.min(s.len())]
        ))
    );

    let output = Command::new("git")
        .args(&args)
        .current_dir(repo_path)
        .output()
        .map_err(|e| RepoError(format!("Failed to run git log -p: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoError(format!("git log -p failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse the batch output into commits with diffs
    let mut commits = parse_batch_log_output(&stdout);
    let total = commits.len();

    if total == 0 {
        log::info!("No commits to process");
        return Ok(commits);
    }

    log::info!("Parsing diffs and extracting semantics for {total} commits");

    // Phase 2: Extract semantics in parallel (CPU-bound, benefits from parallelization)
    let processed = Arc::new(AtomicUsize::new(0));

    commits.par_iter_mut().for_each(|commit| {
        // Extract semantic information from the diff
        commit.semantics = crate::diff::extract_semantics(&commit.diff);

        // Truncate large diffs after semantic extraction
        if commit.diff.len() > MAX_DIFF_SIZE {
            let truncated_diff = format!(
                "{}\n\n[... diff truncated, {} bytes total ...]",
                &commit.diff[..MAX_DIFF_SIZE],
                commit.diff.len()
            );
            commit.diff = truncated_diff;
        }

        // Update progress atomically
        let count = processed.fetch_add(1, Ordering::Relaxed) + 1;

        // Report progress periodically
        if let Some(ref callback) = progress {
            if count % 50 == 0 || count == total {
                let short_hash = &commit.hash[..7.min(commit.hash.len())];
                let first_line = commit.message.lines().next().unwrap_or("");
                let truncated = if first_line.len() > 35 {
                    format!("{}...", &first_line[..32])
                } else {
                    first_line.to_string()
                };
                let item_desc = format!("{short_hash} {truncated}");
                callback(count, total, Some(&item_desc));
            }
        }
    });

    log::info!(
        "Completed batch diff fetching for {} commits",
        commits.len()
    );

    Ok(commits)
}

/// Parses the output of `git log -p` with our custom format.
///
/// The format uses `COMMIT_DELIMITER` to separate commits, with each commit
/// having: hash|timestamp|subject followed by the diff.
fn parse_batch_log_output(output: &str) -> Vec<CommitInfo> {
    let mut commits = Vec::new();

    // Split by our delimiter
    for section in output.split(COMMIT_DELIMITER.trim()) {
        let section = section.trim();
        if section.is_empty() {
            continue;
        }

        // First line is: hash|timestamp|subject
        let mut lines = section.lines();
        let Some(header) = lines.next() else {
            continue;
        };

        // Parse header: hash|timestamp|subject
        let mut parts = header.splitn(3, '|');
        let Some(hash) = parts.next() else { continue };
        let Some(timestamp_str) = parts.next() else {
            continue;
        };
        let message = parts.next().unwrap_or("");

        // Validate hash (should be 40 hex chars)
        let hash = hash.trim();
        if hash.len() < 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }

        let timestamp = timestamp_str.trim().parse::<i64>().unwrap_or(0);

        // Rest is the diff (and file list from --name-only)
        let diff: String = lines.collect::<Vec<_>>().join("\n");

        // Extract files changed from the diff header lines
        let files_changed = extract_files_from_diff(&diff);

        commits.push(CommitInfo {
            hash: hash.to_string(),
            timestamp,
            message: message.to_string(),
            files_changed,
            diff,
            semantics: DiffSemantics::default(),
        });
    }

    commits
}

/// Extracts file paths from a diff's header lines.
fn extract_files_from_diff(diff: &str) -> Vec<String> {
    let mut files = Vec::new();

    for line in diff.lines() {
        // Look for "diff --git a/path b/path" lines
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            // Format: "path/to/file b/path/to/file"
            if let Some(space_idx) = rest.find(" b/") {
                let file_path = &rest[..space_idx];
                if !files.contains(&file_path.to_string()) {
                    files.push(file_path.to_string());
                }
            }
        }
    }

    files
}

/// Fetches all commits with their diffs in parallel for full indexing.
///
/// This function fetches the complete git history and processes diffs
/// in parallel using rayon for significant performance improvements.
///
/// **Note:** Consider using `fetch_all_commits_with_diffs_batch` instead,
/// which is faster for large repositories as it uses a single git command.
///
/// # Arguments
///
/// * `repo_path` - Path to the git repository
/// * `progress` - Optional progress callback for UI updates
///
/// # Returns
///
/// A vector of `CommitInfo` with diffs and semantics populated.
///
/// # Errors
///
/// Returns an error if fetching commit metadata fails.
/// Individual diff fetch failures are logged but don't fail the operation.
#[deprecated(
    since = "0.1.0",
    note = "Use fetch_all_commits_with_diffs_batch for better performance"
)]
pub fn fetch_all_commits_with_diffs_parallel(
    repo_path: &Path,
    progress: Option<&ProgressCallback>,
) -> Result<Vec<CommitInfo>, RepoError> {
    // Phase 1: Fetch commit metadata (fast - single git command)
    let mut commits = fetch_all_commits(repo_path)?;

    if commits.is_empty() {
        return Ok(commits);
    }

    let total = commits.len();
    log::info!(
        "Fetching diffs for {} commits in parallel from: {}",
        total,
        repo_path.display()
    );

    // Atomic counter for progress tracking across threads
    let processed = Arc::new(AtomicUsize::new(0));

    // Phase 2: Fetch diffs and extract semantics in parallel
    let repo_path = repo_path.to_path_buf();
    commits.par_iter_mut().for_each(|commit| {
        // Fetch diff for this commit
        match fetch_commit_diff(&repo_path, &commit.hash) {
            Ok(diff) => {
                commit.semantics = crate::diff::extract_semantics(&diff);
                commit.diff = diff;
            }
            Err(e) => {
                log::warn!("Failed to fetch diff for {}: {e}", &commit.hash[..8]);
            }
        }

        // Update progress atomically
        let count = processed.fetch_add(1, Ordering::Relaxed) + 1;

        // Report progress periodically (every 50 commits) to reduce callback overhead
        if let Some(ref callback) = progress {
            if count % 50 == 0 || count == total {
                let short_hash = &commit.hash[..7.min(commit.hash.len())];
                let first_line = commit.message.lines().next().unwrap_or("");
                let truncated = if first_line.len() > 35 {
                    format!("{}...", &first_line[..32])
                } else {
                    first_line.to_string()
                };
                let item_desc = format!("{short_hash} {truncated}");
                callback(count, total, Some(&item_desc));
            }
        }
    });

    log::info!(
        "Completed parallel diff fetching for {} commits",
        commits.len()
    );

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

    #[test]
    fn test_parse_batch_log_output_with_diff_content() {
        // Regression test: ensure batch parsing preserves actual diff content,
        // not just file names. The --name-only flag conflicts with -p and must
        // NOT be used together, otherwise diffs are empty.
        let output = "__ENYA_COMMIT_BOUNDARY__
abc123def456789012345678901234567890abcd|1700000000|Add new feature

diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdefg 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
 fn main() {
+    println!(\"Hello, world!\");
+    do_something();
 }
diff --git a/src/lib.rs b/src/lib.rs
index 2345678..bcdefgh 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1,3 @@
+pub fn do_something() {
+}
";
        let commits = parse_batch_log_output(output);

        assert_eq!(commits.len(), 1);
        let commit = &commits[0];

        // Verify metadata
        assert_eq!(commit.hash, "abc123def456789012345678901234567890abcd");
        assert_eq!(commit.timestamp, 1_700_000_000);
        assert_eq!(commit.message, "Add new feature");

        // CRITICAL: Verify diff content is present, not just file names
        // This is the regression test - if --name-only is used with -p,
        // the diff would only contain file names without actual changes
        assert!(
            commit.diff.contains(r#"println!("Hello, world!");"#),
            "Diff should contain actual code changes, not just file names. Got: {}",
            &commit.diff[..200.min(commit.diff.len())]
        );
        assert!(
            commit.diff.contains("pub fn do_something()"),
            "Diff should contain function definition"
        );
        assert!(
            commit.diff.contains("@@ -1,3 +1,5 @@"),
            "Diff should contain hunk headers"
        );

        // Verify files are extracted from diff headers
        assert_eq!(commit.files_changed.len(), 2);
        assert!(commit.files_changed.contains(&"src/main.rs".to_string()));
        assert!(commit.files_changed.contains(&"src/lib.rs".to_string()));
    }

    #[test]
    fn test_parse_batch_log_output_multiple_commits() {
        let output = "__ENYA_COMMIT_BOUNDARY__
abc123def456789012345678901234567890abcd|1700000000|First commit

diff --git a/file1.rs b/file1.rs
--- a/file1.rs
+++ b/file1.rs
@@ -1 +1,2 @@
+// added line
__ENYA_COMMIT_BOUNDARY__
def456789012345678901234567890abcdef1234|1700001000|Second commit

diff --git a/file2.rs b/file2.rs
--- a/file2.rs
+++ b/file2.rs
@@ -1 +1,2 @@
+// another line
";
        let commits = parse_batch_log_output(output);

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].hash, "abc123def456789012345678901234567890abcd");
        assert_eq!(commits[0].message, "First commit");
        assert!(commits[0].diff.contains("// added line"));

        assert_eq!(commits[1].hash, "def456789012345678901234567890abcdef1234");
        assert_eq!(commits[1].message, "Second commit");
        assert!(commits[1].diff.contains("// another line"));
    }

    #[test]
    fn test_parse_batch_log_output_empty() {
        let commits = parse_batch_log_output("");
        assert!(commits.is_empty());
    }

    #[test]
    fn test_extract_files_from_diff() {
        let diff = "diff --git a/src/main.rs b/src/main.rs
index 1234567..abcdefg 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
 fn main() {}
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1,2 @@
+pub fn foo() {}
";
        let files = extract_files_from_diff(diff);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0], "src/main.rs");
        assert_eq!(files[1], "src/lib.rs");
    }

    #[test]
    fn test_extract_files_from_diff_no_duplicates() {
        // Same file modified multiple times in one diff should only appear once
        let diff = "diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
 fn main() {}
";
        let files = extract_files_from_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], "src/main.rs");
    }
}
