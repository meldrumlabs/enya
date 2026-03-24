//! GitHub REST API client for Pull Request operations.
//!
//! - **WASM**: proxies through `https://api.enya.build/github` to bypass CORS.
//! - **Native**: calls `https://api.github.com` directly.

use serde::{Deserialize, Serialize};

// ── WASM-aware base URL ─────────────────────────────────────────────────

fn github_api_base() -> &'static str {
    #[cfg(target_arch = "wasm32")]
    {
        "https://api.enya.build/github"
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        "https://api.github.com"
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    #[serde(default)]
    pub draft: bool,
    pub user: PrUser,
    pub head: PrRef,
    pub base: PrRef,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub additions: u32,
    #[serde(default)]
    pub deletions: u32,
    #[serde(default)]
    pub changed_files: u32,
    /// Whether the PR can be merged cleanly. `None` when GitHub hasn't computed it yet.
    #[serde(default)]
    pub mergeable: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrUser {
    pub login: String,
    #[serde(default)]
    pub avatar_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrRef {
    #[serde(rename = "ref")]
    pub ref_name: String,
    pub sha: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrFile {
    pub filename: String,
    pub status: String,
    #[serde(default)]
    pub additions: u32,
    #[serde(default)]
    pub deletions: u32,
    #[serde(default)]
    pub changes: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrComment {
    pub id: u64,
    pub body: String,
    pub user: PrUser,
    pub created_at: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub line: Option<usize>,
    #[serde(default)]
    pub in_reply_to_id: Option<u64>,
}

/// A thread of review comments on a specific file and line.
#[derive(Debug, Clone)]
pub struct CommentThread {
    pub path: String,
    pub line: usize,
    pub comments: Vec<PrComment>,
}

/// Group review comments into threads by (path, line), chaining `in_reply_to_id` replies.
pub fn group_into_threads(comments: &[PrComment]) -> Vec<CommentThread> {
    use rustc_hash::FxHashMap;

    // Map comment id → (path, line) for reply chain resolution
    let mut id_to_location: FxHashMap<u64, (String, usize)> = FxHashMap::default();
    for c in comments {
        if let (Some(path), Some(line)) = (&c.path, c.line) {
            id_to_location.insert(c.id, (path.clone(), line));
        }
    }

    // Group comments by (path, line), resolving replies via parent location
    let mut threads: FxHashMap<(String, usize), Vec<PrComment>> = FxHashMap::default();
    for c in comments {
        let location = if let Some(parent_id) = c.in_reply_to_id {
            // Reply: use parent's location
            id_to_location.get(&parent_id).cloned()
        } else {
            None
        };
        let location =
            location.or_else(|| c.path.as_ref().zip(c.line).map(|(p, l)| (p.clone(), l)));
        if let Some((path, line)) = location {
            threads.entry((path, line)).or_default().push(c.clone());
        }
    }

    let mut result: Vec<CommentThread> = threads
        .into_iter()
        .map(|((path, line), mut comments)| {
            comments.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            CommentThread {
                path,
                line,
                comments,
            }
        })
        .collect();
    result.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    result
}

#[derive(Debug, Clone, Deserialize)]
pub struct IssueComment {
    pub id: u64,
    pub body: String,
    pub user: PrUser,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckRun {
    pub id: u64,
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub conclusion: Option<String>,
}

/// Wrapper for the check-runs API response.
#[derive(Debug, Deserialize)]
struct CheckRunsResponse {
    check_runs: Vec<CheckRun>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DraftComment {
    pub path: String,
    pub line: usize,
    pub side: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ReviewEvent {
    #[serde(rename = "APPROVE")]
    Approve,
    #[serde(rename = "REQUEST_CHANGES")]
    RequestChanges,
    #[serde(rename = "COMMENT")]
    Comment,
}

/// A review submitted on a pull request (from GET /pulls/{number}/reviews).
#[derive(Debug, Clone, Deserialize)]
pub struct PrReview {
    pub id: u64,
    pub user: PrUser,
    pub state: String,
}

/// Submission payload for the create-review API.
#[derive(Debug, Serialize)]
struct ReviewSubmission {
    event: ReviewEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    comments: Vec<DraftComment>,
}

// ── URL parsing ─────────────────────────────────────────────────────────

/// Extract `(owner, repo)` from an HTTPS or SSH git remote URL.
pub fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    // HTTPS: https://github.com/owner/repo.git
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        let rest = rest.trim_end_matches(".git").trim_end_matches('/');
        let (owner, repo) = rest.split_once('/')?;
        if owner.is_empty() || repo.is_empty() || repo.contains('/') {
            return None;
        }
        return Some((owner.to_string(), repo.to_string()));
    }

    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let rest = rest.trim_end_matches(".git").trim_end_matches('/');
        let (owner, repo) = rest.split_once('/')?;
        if owner.is_empty() || repo.is_empty() || repo.contains('/') {
            return None;
        }
        return Some((owner.to_string(), repo.to_string()));
    }

    None
}

// ── API helpers ─────────────────────────────────────────────────────────

/// Build common headers for GitHub API requests.
fn api_headers(token: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        "application/vnd.github+json".parse().expect("valid header"),
    );
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse().expect("valid header"),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        "enya-editor".parse().expect("valid header"),
    );
    headers
}

/// Perform a GET request and deserialize the JSON response.
async fn api_get<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    token: &str,
    path: &str,
) -> Result<T, String> {
    let url = format!("{}{path}", github_api_base());
    let resp = client
        .get(&url)
        .headers(api_headers(token))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {status}: {body}"));
    }

    resp.json::<T>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Public API functions ────────────────────────────────────────────────

/// List open pull requests for a repository.
pub async fn list_open_pulls(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<PullRequest>, String> {
    api_get(
        client,
        token,
        &format!("/repos/{owner}/{repo}/pulls?state=open&per_page=30"),
    )
    .await
}

/// Get a single pull request (includes additions/deletions/changed_files).
pub async fn get_pull(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    number: u32,
) -> Result<PullRequest, String> {
    api_get(
        client,
        token,
        &format!("/repos/{owner}/{repo}/pulls/{number}"),
    )
    .await
}

/// Get the raw diff for a pull request.
pub async fn get_pull_diff(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    number: u32,
) -> Result<String, String> {
    let url = format!("{}/repos/{owner}/{repo}/pulls/{number}", github_api_base());
    let mut headers = api_headers(token);
    headers.insert(
        reqwest::header::ACCEPT,
        "application/vnd.github.diff".parse().expect("valid header"),
    );

    let resp = client
        .get(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {status}: {body}"));
    }

    resp.text()
        .await
        .map_err(|e| format!("Read body failed: {e}"))
}

/// Get the list of files changed in a pull request.
pub async fn get_pull_files(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    number: u32,
) -> Result<Vec<PrFile>, String> {
    api_get(
        client,
        token,
        &format!("/repos/{owner}/{repo}/pulls/{number}/files?per_page=100"),
    )
    .await
}

/// Get review comments on a pull request.
pub async fn get_review_comments(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    number: u32,
) -> Result<Vec<PrComment>, String> {
    api_get(
        client,
        token,
        &format!("/repos/{owner}/{repo}/pulls/{number}/comments?per_page=100"),
    )
    .await
}

/// Get issue-level comments on a pull request.
pub async fn get_issue_comments(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    number: u32,
) -> Result<Vec<IssueComment>, String> {
    api_get(
        client,
        token,
        &format!("/repos/{owner}/{repo}/issues/{number}/comments?per_page=100"),
    )
    .await
}

/// Get reviews on a pull request (approval state per reviewer).
pub async fn get_reviews(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    number: u32,
) -> Result<Vec<PrReview>, String> {
    api_get(
        client,
        token,
        &format!("/repos/{owner}/{repo}/pulls/{number}/reviews?per_page=100"),
    )
    .await
}

/// Get check runs for a commit ref.
pub async fn get_check_runs(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    ref_sha: &str,
) -> Result<Vec<CheckRun>, String> {
    let response: CheckRunsResponse = api_get(
        client,
        token,
        &format!("/repos/{owner}/{repo}/commits/{ref_sha}/check-runs"),
    )
    .await?;
    Ok(response.check_runs)
}

/// Submit a pull request review.
#[allow(clippy::too_many_arguments)]
pub async fn submit_review(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    number: u32,
    event: ReviewEvent,
    body: Option<String>,
    comments: Vec<DraftComment>,
) -> Result<(), String> {
    let url = format!(
        "{}/repos/{owner}/{repo}/pulls/{number}/reviews",
        github_api_base()
    );

    let submission = ReviewSubmission {
        event,
        body,
        comments,
    };

    let resp = client
        .post(&url)
        .headers(api_headers(token))
        .json(&submission)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {status}: {body}"));
    }

    Ok(())
}

/// Post a single review comment on a pull request (immediately visible, not batched).
#[allow(clippy::too_many_arguments)]
pub async fn create_review_comment(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    number: u32,
    commit_id: &str,
    path: &str,
    line: usize,
    body: &str,
) -> Result<PrComment, String> {
    let url = format!(
        "{}/repos/{owner}/{repo}/pulls/{number}/comments",
        github_api_base()
    );

    let payload = serde_json::json!({
        "body": body,
        "commit_id": commit_id,
        "path": path,
        "line": line,
        "side": "RIGHT",
    });

    let resp = client
        .post(&url)
        .headers(api_headers(token))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error {status}: {text}"));
    }

    resp.json::<PrComment>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Relative time ───────────────────────────────────────────────────────

/// Format an ISO 8601 timestamp as a human-readable relative time string.
///
/// Returns strings like "2 hours ago", "3 days ago", "just now".
/// Falls back to the original string if parsing fails.
pub fn relative_time(timestamp: &str) -> String {
    let Some(ts_secs) = parse_iso8601_to_unix(timestamp) else {
        return timestamp.to_string();
    };

    let now = crate::util::now_unix_secs() as u64;
    let diff = now.saturating_sub(ts_secs);

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        if mins == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{mins} minutes ago")
        }
    } else if diff < 86400 {
        let hours = diff / 3600;
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{hours} hours ago")
        }
    } else if diff < 2592000 {
        let days = diff / 86400;
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        }
    } else {
        let months = diff / 2592000;
        if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{months} months ago")
        }
    }
}

/// Parse a subset of ISO 8601 timestamps to Unix seconds.
///
/// Handles formats like `2024-01-15T10:30:00Z` and `2024-01-15T10:30:00+00:00`.
fn parse_iso8601_to_unix(s: &str) -> Option<u64> {
    // Strip trailing Z or timezone offset
    let s = s.trim_end_matches('Z');
    let date_time = if let Some(pos) = s.rfind('+') {
        // Has positive timezone offset — strip it
        &s[..pos]
    } else if s.len() > 19 {
        // May have negative offset like -05:00
        if let Some(pos) = s[19..].find('-') {
            &s[..19 + pos]
        } else {
            s
        }
    } else {
        s
    };

    let (date_part, time_part) = date_time.split_once('T')?;
    let mut date_parts = date_part.split('-');
    let year: u64 = date_parts.next()?.parse().ok()?;
    let month: u64 = date_parts.next()?.parse().ok()?;
    let day: u64 = date_parts.next()?.parse().ok()?;

    let mut time_parts = time_part.split(':');
    let hour: u64 = time_parts.next()?.parse().ok()?;
    let minute: u64 = time_parts.next()?.parse().ok()?;
    let second: u64 = time_parts
        .next()
        .and_then(|s| s.split('.').next()) // strip fractional seconds
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Simplified days-since-epoch calculation (no leap second precision needed)
    let mut total_days: u64 = 0;
    for y in 1970..year {
        total_days += if is_leap_year(y) { 366 } else { 365 };
    }
    let days_in_months = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for m in 0..(month.saturating_sub(1) as usize) {
        total_days += days_in_months.get(m).copied().unwrap_or(30) as u64;
    }
    total_days += day.saturating_sub(1);

    Some(total_days * 86400 + hour * 3600 + minute * 60 + second)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_owner_repo_https() {
        let (owner, repo) = parse_owner_repo("https://github.com/conductor/enya.git").unwrap();
        assert_eq!(owner, "conductor");
        assert_eq!(repo, "enya");
    }

    #[test]
    fn test_parse_owner_repo_https_no_git_suffix() {
        let (owner, repo) = parse_owner_repo("https://github.com/conductor/enya").unwrap();
        assert_eq!(owner, "conductor");
        assert_eq!(repo, "enya");
    }

    #[test]
    fn test_parse_owner_repo_ssh() {
        let (owner, repo) = parse_owner_repo("git@github.com:conductor/enya.git").unwrap();
        assert_eq!(owner, "conductor");
        assert_eq!(repo, "enya");
    }

    #[test]
    fn test_parse_owner_repo_invalid() {
        assert!(parse_owner_repo("https://gitlab.com/owner/repo.git").is_none());
        assert!(parse_owner_repo("not a url").is_none());
        assert!(parse_owner_repo("").is_none());
    }

    #[test]
    fn test_parse_iso8601() {
        let secs = parse_iso8601_to_unix("2024-01-01T00:00:00Z").unwrap();
        // 2024-01-01 00:00:00 UTC should be 54 years * 365.25 days ≈ 1704067200
        assert!(secs > 1704000000 && secs < 1704200000);
    }

    #[test]
    fn test_parse_iso8601_with_offset() {
        let secs = parse_iso8601_to_unix("2024-01-01T00:00:00+00:00").unwrap();
        assert!(secs > 1704000000 && secs < 1704200000);
    }
}
