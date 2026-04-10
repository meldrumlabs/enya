//! AI-powered review walkthrough — presents PR changes as inline insights.
//!
//! Sends the PR diff context to an LLM which returns a structured analysis:
//! a summary of the PR, logical file groups, and line-level insights that
//! render as floating cards in the diff gutter alongside human comments.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Receiver;

use rustc_hash::FxHashMap;

use crate::git::diff::FileDiff;

/// A logical group of files in the walkthrough (e.g. "Data model", "API layer").
#[derive(Debug, Clone)]
pub(super) struct WalkthroughGroup {
    /// Human-readable group label (e.g. "Core data model changes").
    pub label: String,
    /// File paths belonging to this group, in suggested review order.
    pub files: Vec<String>,
}

/// Kind of AI insight for a specific diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InsightKind {
    /// Key change worth understanding.
    KeyChange,
    /// Potential issue or concern.
    Concern,
    /// Suggestion for improvement.
    Suggestion,
    /// Contextual note (why something was done).
    Context,
}

/// A single line-level AI insight anchored to a diff line.
#[derive(Debug, Clone)]
pub(super) struct LineInsight {
    /// File path this insight applies to.
    pub file: String,
    /// New-file line number (matches `DiffLine::new_line_num`).
    pub line: usize,
    /// Classification of the insight.
    pub kind: InsightKind,
    /// Short descriptive text (1-2 sentences).
    pub text: String,
}

/// AI-generated review walkthrough for a PR.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(super) struct ReviewWalkthrough {
    /// 2-3 sentence narrative summary of the PR (what and why).
    pub summary: String,
    /// Files grouped by logical concern, in suggested review order.
    pub groups: Vec<WalkthroughGroup>,
    /// Per-file one-liner annotations keyed by file path.
    pub annotations: FxHashMap<String, String>,
    /// Line-level insights for the floating gutter.
    pub insights: Vec<LineInsight>,
}

/// State of the walkthrough request.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) enum WalkthroughState {
    /// No walkthrough requested.
    Idle,
    /// Walkthrough is being generated.
    Loading,
    /// Walkthrough is ready.
    Ready(ReviewWalkthrough),
    /// Walkthrough generation failed.
    Error(String),
}

/// Build the prompt for the walkthrough AI request.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) fn build_walkthrough_prompt(
    pr_title: &str,
    pr_body: Option<&str>,
    file_diffs: &[FileDiff],
) -> String {
    let mut prompt = String::with_capacity(8192);

    prompt.push_str("Analyze this pull request and produce a structured review walkthrough.\n\n");
    prompt.push_str(&format!("**PR Title:** {pr_title}\n\n"));

    if let Some(body) = pr_body {
        if !body.is_empty() {
            let truncated = if body.len() > 2000 {
                &body[..2000]
            } else {
                body
            };
            prompt.push_str(&format!("**PR Description:**\n{truncated}\n\n"));
        }
    }

    prompt.push_str("**Changed files and diffs:**\n\n");

    for diff in file_diffs {
        prompt.push_str(&format!("### {}\n```diff\n", diff.path));
        // Include up to ~200 lines per file to stay within context limits
        let max_lines = 200;
        for (i, line) in diff.lines.iter().enumerate() {
            if i >= max_lines {
                prompt.push_str(&format!("... ({} more lines)\n", diff.lines.len() - i));
                break;
            }
            let prefix = match line.kind {
                crate::git::diff::DiffLineKind::Addition => "+",
                crate::git::diff::DiffLineKind::Deletion => "-",
                crate::git::diff::DiffLineKind::Context => " ",
                crate::git::diff::DiffLineKind::HunkHeader => "@@",
                crate::git::diff::DiffLineKind::FileHeader => "##",
            };
            prompt.push_str(prefix);
            prompt.push_str(&line.content);
            prompt.push('\n');
        }
        prompt.push_str("```\n\n");
    }

    prompt.push_str(
        r#"Respond with ONLY a JSON object (no markdown fences, no extra text) in this exact format:
{
  "summary": "2-3 sentence narrative summary of what this PR does and why",
  "groups": [
    {
      "label": "Group name (e.g. 'Core data model', 'API endpoints', 'Tests')",
      "files": ["path/to/file1.rs", "path/to/file2.rs"]
    }
  ],
  "annotations": {
    "path/to/file1.rs": "One-liner explaining what changed in this file and why"
  },
  "insights": [
    {
      "file": "path/to/file1.rs",
      "line": 42,
      "kind": "key_change",
      "text": "Brief explanation of what's happening at this line or what to look for"
    }
  ]
}

Guidelines:
- Order groups by logical dependency (foundational changes first, then dependent layers, tests last)
- Order files within each group by review priority
- Every changed file must appear in exactly one group
- Keep annotations concise (under 100 chars)
- The summary should explain the "story" of the PR — what problem it solves and the approach taken
- Use 2-5 groups depending on PR complexity (don't over-segment small PRs)
- For insights: target 2-5 per file (more for complex files, fewer for trivial ones)
- Insight line numbers MUST be new-file line numbers from the @@ +N,count @@ hunk headers
- Use "key_change" for the most important modifications worth understanding
- Use "concern" for potential bugs, edge cases, or issues worth flagging
- Use "suggestion" for possible improvements
- Use "context" for explaining rationale or non-obvious design decisions
- Keep insight text under 120 characters
"#,
    );

    prompt
}

/// Parse the AI response JSON into a `ReviewWalkthrough`.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) fn parse_walkthrough_response(response: &str) -> Result<ReviewWalkthrough, String> {
    // Try to find JSON in the response (model may wrap in markdown fences)
    let json_str = extract_json(response);

    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let summary = value["summary"]
        .as_str()
        .unwrap_or("No summary provided")
        .to_string();

    let mut groups = Vec::new();
    if let Some(groups_arr) = value["groups"].as_array() {
        for g in groups_arr {
            let label = g["label"].as_str().unwrap_or("Other").to_string();
            let files = g["files"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            groups.push(WalkthroughGroup { label, files });
        }
    }

    let mut annotations = FxHashMap::default();
    if let Some(ann_obj) = value["annotations"].as_object() {
        for (path, desc) in ann_obj {
            if let Some(s) = desc.as_str() {
                annotations.insert(path.clone(), s.to_string());
            }
        }
    }

    let mut insights = Vec::new();
    if let Some(insights_arr) = value["insights"].as_array() {
        for item in insights_arr {
            let file = item["file"].as_str().unwrap_or_default().to_string();
            let line = item["line"].as_u64().unwrap_or(0) as usize;
            let kind = match item["kind"].as_str().unwrap_or("context") {
                "key_change" => InsightKind::KeyChange,
                "concern" => InsightKind::Concern,
                "suggestion" => InsightKind::Suggestion,
                _ => InsightKind::Context,
            };
            let text = item["text"].as_str().unwrap_or_default().to_string();
            // Filter out invalid insights
            if !file.is_empty() && line > 0 && !text.is_empty() {
                insights.push(LineInsight {
                    file,
                    line,
                    kind,
                    text,
                });
            }
        }
    }

    if groups.is_empty() {
        return Err("No file groups found in response".to_string());
    }

    Ok(ReviewWalkthrough {
        summary,
        groups,
        annotations,
        insights,
    })
}

/// Extract JSON from a response that may contain markdown fences or preamble.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn extract_json(s: &str) -> &str {
    let s = s.trim();

    // Strip ```json ... ``` fences
    if let Some(start) = s.find("```json") {
        let start = start + 7;
        if let Some(end) = s[start..].find("```") {
            return s[start..start + end].trim();
        }
    }
    if let Some(start) = s.find("```") {
        let start = start + 3;
        // Skip optional language tag on same line
        let start = s[start..]
            .find('\n')
            .map(|i| start + i + 1)
            .unwrap_or(start);
        if let Some(end) = s[start..].find("```") {
            return s[start..start + end].trim();
        }
    }

    // Try to find raw JSON object
    if let Some(start) = s.find('{') {
        if let Some(end) = s.rfind('}') {
            return &s[start..=end];
        }
    }

    s
}

/// Spawn the walkthrough AI request. Returns a receiver for streaming events.
///
/// The caller should poll the receiver each frame with `try_recv()` to collect
/// `TextDelta` events, then parse the accumulated text on `Done`.
///
/// `model` should be the user's selected AI model from settings. If `None`,
/// the ACP client falls back to its default.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn spawn_walkthrough_request(
    async_runtime: &crate::AsyncRuntime,
    prompt: String,
    model: Option<&str>,
) -> Receiver<enya_ai::AgentEvent> {
    let client = enya_ai::AcpClient::claude_code_with_runtime(async_runtime.handle().clone());
    client.prompt_with_context(
        prompt,
        None,
        model,
        Some("You are a code review assistant. Respond only with the requested JSON format."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_raw() {
        let input = r#"{"summary": "test", "groups": [], "annotations": {}}"#;
        assert_eq!(extract_json(input), input);
    }

    #[test]
    fn test_extract_json_fenced() {
        let input = "Here's the analysis:\n```json\n{\"summary\": \"test\"}\n```\nDone.";
        assert_eq!(extract_json(input), r#"{"summary": "test"}"#);
    }

    #[test]
    fn test_extract_json_with_preamble() {
        let input = "Sure! Here you go:\n{\"summary\": \"hello\"}";
        assert_eq!(extract_json(input), r#"{"summary": "hello"}"#);
    }

    #[test]
    fn test_parse_walkthrough_response() {
        let json = r#"{
            "summary": "Adds user authentication",
            "groups": [
                {"label": "Data model", "files": ["src/models/user.rs"]},
                {"label": "API", "files": ["src/routes/auth.rs"]}
            ],
            "annotations": {
                "src/models/user.rs": "New User struct with password hashing",
                "src/routes/auth.rs": "Login and signup endpoints"
            },
            "insights": [
                {
                    "file": "src/models/user.rs",
                    "line": 15,
                    "kind": "key_change",
                    "text": "New User struct with bcrypt password hashing"
                },
                {
                    "file": "src/routes/auth.rs",
                    "line": 42,
                    "kind": "concern",
                    "text": "No rate limiting on login endpoint"
                }
            ]
        }"#;

        let result = parse_walkthrough_response(json).unwrap();
        assert_eq!(result.groups.len(), 2);
        assert_eq!(result.groups[0].label, "Data model");
        assert_eq!(result.annotations.len(), 2);
        assert_eq!(result.insights.len(), 2);
        assert_eq!(result.insights[0].line, 15);
        assert_eq!(result.insights[0].kind, InsightKind::KeyChange);
        assert_eq!(result.insights[1].kind, InsightKind::Concern);
    }

    #[test]
    fn test_parse_walkthrough_without_insights() {
        // Backward compat: JSON without insights field should parse fine
        let json = r#"{
            "summary": "Simple fix",
            "groups": [{"label": "Fix", "files": ["src/lib.rs"]}],
            "annotations": {"src/lib.rs": "Bug fix"}
        }"#;

        let result = parse_walkthrough_response(json).unwrap();
        assert!(result.insights.is_empty());
        assert_eq!(result.groups.len(), 1);
    }

    #[test]
    fn test_parse_insight_kinds() {
        let json = r#"{
            "summary": "Test",
            "groups": [{"label": "All", "files": ["a.rs"]}],
            "annotations": {},
            "insights": [
                {"file": "a.rs", "line": 1, "kind": "key_change", "text": "a"},
                {"file": "a.rs", "line": 2, "kind": "concern", "text": "b"},
                {"file": "a.rs", "line": 3, "kind": "suggestion", "text": "c"},
                {"file": "a.rs", "line": 4, "kind": "context", "text": "d"},
                {"file": "a.rs", "line": 5, "kind": "unknown_kind", "text": "e"}
            ]
        }"#;

        let result = parse_walkthrough_response(json).unwrap();
        assert_eq!(result.insights.len(), 5);
        assert_eq!(result.insights[0].kind, InsightKind::KeyChange);
        assert_eq!(result.insights[1].kind, InsightKind::Concern);
        assert_eq!(result.insights[2].kind, InsightKind::Suggestion);
        assert_eq!(result.insights[3].kind, InsightKind::Context);
        assert_eq!(result.insights[4].kind, InsightKind::Context); // unknown falls back
    }

    #[test]
    fn test_parse_insights_filters_invalid() {
        let json = r#"{
            "summary": "Test",
            "groups": [{"label": "All", "files": ["a.rs"]}],
            "annotations": {},
            "insights": [
                {"file": "", "line": 1, "kind": "context", "text": "empty file"},
                {"file": "a.rs", "line": 0, "kind": "context", "text": "zero line"},
                {"file": "a.rs", "line": 1, "kind": "context", "text": ""},
                {"file": "a.rs", "line": 10, "kind": "key_change", "text": "valid"}
            ]
        }"#;

        let result = parse_walkthrough_response(json).unwrap();
        assert_eq!(result.insights.len(), 1);
        assert_eq!(result.insights[0].line, 10);
    }
}
