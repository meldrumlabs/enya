//! AI-powered review assistant — presents PR changes as inline insights with
//! a review summary card.
//!
//! Sends the PR diff context to an LLM which returns a structured analysis:
//! a verdict, risk level, summary, top concerns, logical file groups, and
//! line-level insights that render as floating cards in the diff gutter.

#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::Receiver;

use rustc_hash::FxHashMap;

use crate::components::util::AiProvider;
use crate::git::diff::FileDiff;

/// Verdict from the AI review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReviewVerdict {
    /// No significant issues found.
    Lgtm,
    /// Bugs or risks identified — needs fixes.
    NeedsWork,
    /// Unclear or needs discussion.
    NeedsDiscussion,
}

/// Risk level of the PR changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

impl ReviewVerdict {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Lgtm => "LGTM",
            Self::NeedsWork => "Needs Work",
            Self::NeedsDiscussion => "Needs Discussion",
        }
    }

    /// Theme color for rendering the verdict badge.
    pub fn theme_color(self, theme: &crate::ui::theme::AppTheme) -> egui::Color32 {
        match self {
            Self::Lgtm => theme.diff_added_text(),
            Self::NeedsWork => theme.diff_removed_text(),
            Self::NeedsDiscussion => theme.semantic_warning(),
        }
    }
}

impl RiskLevel {
    /// Theme color for rendering the risk badge.
    pub fn theme_color(self, theme: &crate::ui::theme::AppTheme) -> egui::Color32 {
        match self {
            Self::Low => theme.text_secondary(),
            Self::Medium => theme.semantic_warning(),
            Self::High => theme.diff_removed_text(),
        }
    }
}

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
    /// Optional suggested code change for this line.
    pub suggested_change: Option<String>,
}

/// AI-generated review walkthrough for a PR.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(super) struct ReviewWalkthrough {
    /// 2-3 sentence narrative summary of the PR (what and why).
    pub summary: String,
    /// Overall verdict.
    pub verdict: ReviewVerdict,
    /// Risk level of the changes.
    pub risk_level: RiskLevel,
    /// Top concerns (max 3) when verdict is not LGTM.
    pub top_concerns: Vec<String>,
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

/// Format a single file diff into prompt text.
fn format_diff_for_prompt(diff: &FileDiff) -> String {
    let mut out = String::new();
    out.push_str(&format!("### {}\n```diff\n", diff.path));

    const MAX_LINES: usize = 200;
    for (i, line) in diff.lines.iter().enumerate() {
        if i >= MAX_LINES {
            out.push_str(&format!("... ({} more lines)\n", diff.lines.len() - i));
            break;
        }
        if line.kind == crate::git::diff::DiffLineKind::CollapsedBlock {
            continue;
        }

        match line.kind {
            crate::git::diff::DiffLineKind::HunkHeader => {
                out.push_str("@@ ");
                out.push_str(&line.content);
                out.push('\n');
            }
            crate::git::diff::DiffLineKind::FileHeader => {
                out.push_str("## ");
                out.push_str(&line.content);
                out.push('\n');
            }
            _ => {
                // Prefix with new-file line number when available so the AI
                // can reference exact line numbers for insights.
                let prefix = match line.kind {
                    crate::git::diff::DiffLineKind::Addition => "+",
                    crate::git::diff::DiffLineKind::Deletion => "-",
                    crate::git::diff::DiffLineKind::Context => " ",
                    _ => unreachable!(),
                };
                if let Some(ln) = line.new_line_num {
                    out.push_str(&format!(
                        "{prefix} {ln:>4} |{content}\n",
                        content = &line.content
                    ));
                } else {
                    out.push_str(prefix);
                    out.push_str(&line.content);
                    out.push('\n');
                }
            }
        }
    }
    out.push_str("```\n\n");
    out
}

/// Build the prompt for the AI review request.
///
/// The prompt is optimized for Codex (OpenAI) with clear task-oriented
/// instructions, while remaining compatible with Claude.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) fn build_review_prompt(
    pr_title: &str,
    pr_body: Option<&str>,
    file_diffs: &[FileDiff],
    _provider: AiProvider,
) -> String {
    let mut prompt = String::with_capacity(8192);

    prompt.push_str("You are reviewing a pull request. Analyze the changes carefully and produce a structured review.\n\n");
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
        prompt.push_str(&format_diff_for_prompt(diff));
    }

    prompt.push_str(
        r#"Your task:
1. Identify the purpose of this PR and the approach taken.
2. Group files by logical concern in review order (foundational first, tests last).
3. For each important change, note what changed, potential issues, and suggestions.
4. Output ONLY a JSON object with this exact structure:
{
  "summary": "2-3 sentence narrative of what this PR does and why",
  "verdict": "LGTM" | "Needs Work" | "Needs Discussion",
  "risk_level": "Low" | "Medium" | "High",
  "top_concerns": ["concern 1", "concern 2"],
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
      "kind": "key_change" | "concern" | "suggestion" | "context",
      "text": "Brief explanation of what's happening at this line",
      "suggested_change": "optional replacement code, or omitted"
    }
  ]
}

Guidelines:
- Verdict: "LGTM" only if no real issues found. "Needs Work" if bugs or risks. "Needs Discussion" if unclear.
- Risk level: "Low" for typos/docs. "Medium" for behavior changes. "High" for security, auth, data loss.
- top_concerns: max 3 items. Only include when verdict is "Needs Work" or "Needs Discussion".
- Order groups by logical dependency (foundational changes first, then dependent layers, tests last).
- Order files within each group by review priority.
- Every changed file must appear in exactly one group.
- Keep annotations concise (under 100 chars).
- For insights: target 2-5 per file (more for complex files, fewer for trivial ones).
- Insight line numbers MUST match the line numbers shown in the left margin of the diff (e.g. "  42 |").
- Use "key_change" for the most important modifications worth understanding.
- Use "concern" for potential bugs, edge cases, or issues worth flagging.
- Use "suggestion" for possible improvements.
- Use "context" for explaining rationale or non-obvious design decisions.
- suggested_change: only include when you can propose a concrete, single-line fix. Omit otherwise.
- Keep insight text under 120 characters.
- Respond with ONLY the JSON object — no markdown fences, no preamble.
"#,
    );

    prompt
}

/// Parse verdict from JSON string.
fn parse_verdict(value: &serde_json::Value) -> ReviewVerdict {
    match value.as_str().unwrap_or("Needs Discussion") {
        "LGTM" | "lgtm" => ReviewVerdict::Lgtm,
        "Needs Work" | "needs_work" | "needs work" => ReviewVerdict::NeedsWork,
        _ => ReviewVerdict::NeedsDiscussion,
    }
}

/// Parse risk level from JSON string.
fn parse_risk_level(value: &serde_json::Value) -> RiskLevel {
    match value.as_str().unwrap_or("Medium") {
        "Low" | "low" => RiskLevel::Low,
        "High" | "high" => RiskLevel::High,
        _ => RiskLevel::Medium,
    }
}

/// Parse string array from JSON value.
fn parse_string_array(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse file groups from JSON array.
fn parse_groups(value: &serde_json::Value) -> Vec<WalkthroughGroup> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|g| WalkthroughGroup {
                    label: g["label"].as_str().unwrap_or("Other").to_string(),
                    files: g["files"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse annotations map from JSON object.
fn parse_annotations(value: &serde_json::Value) -> FxHashMap<String, String> {
    let mut map = FxHashMap::default();
    if let Some(obj) = value.as_object() {
        for (path, desc) in obj {
            if let Some(s) = desc.as_str() {
                map.insert(path.clone(), s.to_string());
            }
        }
    }
    map
}

/// Parse insight kind from JSON string.
fn parse_insight_kind(value: &serde_json::Value) -> InsightKind {
    match value.as_str().unwrap_or("context") {
        "key_change" => InsightKind::KeyChange,
        "concern" => InsightKind::Concern,
        "suggestion" => InsightKind::Suggestion,
        _ => InsightKind::Context,
    }
}

/// Parse insights array from JSON.
fn parse_insights(value: &serde_json::Value) -> Vec<LineInsight> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let file = item["file"].as_str().unwrap_or_default().to_string();
                    let line = item["line"].as_u64().unwrap_or(0) as usize;
                    let text = item["text"].as_str().unwrap_or_default().to_string();
                    if file.is_empty() || line == 0 || text.is_empty() {
                        return None;
                    }
                    Some(LineInsight {
                        file,
                        line,
                        kind: parse_insight_kind(&item["kind"]),
                        text,
                        suggested_change: item["suggested_change"].as_str().map(String::from),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parse the AI response JSON into a `ReviewWalkthrough`.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) fn parse_walkthrough_response(response: &str) -> Result<ReviewWalkthrough, String> {
    let json_str = extract_json(response);

    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let groups = parse_groups(&value["groups"]);
    if groups.is_empty() {
        return Err("No file groups found in response".to_string());
    }

    Ok(ReviewWalkthrough {
        summary: value["summary"]
            .as_str()
            .unwrap_or("No summary provided")
            .to_string(),
        verdict: parse_verdict(&value["verdict"]),
        risk_level: parse_risk_level(&value["risk_level"]),
        top_concerns: parse_string_array(&value["top_concerns"]),
        groups,
        annotations: parse_annotations(&value["annotations"]),
        insights: parse_insights(&value["insights"]),
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
    provider: AiProvider,
) -> Receiver<enya_ai::AgentEvent> {
    let client = match provider {
        AiProvider::Claude => {
            enya_ai::AcpClient::claude_code_with_runtime(async_runtime.handle().clone())
        }
        AiProvider::Codex => enya_ai::AcpClient::codex_with_runtime(async_runtime.handle().clone()),
    };
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
            "verdict": "Needs Work",
            "risk_level": "High",
            "top_concerns": ["No rate limiting", "Missing tests"],
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
        assert_eq!(result.verdict, ReviewVerdict::NeedsWork);
        assert_eq!(result.risk_level, RiskLevel::High);
        assert_eq!(
            result.top_concerns,
            vec!["No rate limiting", "Missing tests"]
        );
    }

    #[test]
    fn test_parse_walkthrough_without_insights() {
        let json = r#"{
            "summary": "Simple fix",
            "verdict": "LGTM",
            "risk_level": "Low",
            "groups": [{"label": "Fix", "files": ["src/lib.rs"]}],
            "annotations": {"src/lib.rs": "Bug fix"}
        }"#;

        let result = parse_walkthrough_response(json).unwrap();
        assert!(result.insights.is_empty());
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.verdict, ReviewVerdict::Lgtm);
        assert_eq!(result.risk_level, RiskLevel::Low);
        assert!(result.top_concerns.is_empty());
    }

    #[test]
    fn test_parse_insight_kinds() {
        let json = r#"{
            "summary": "Test",
            "verdict": "Needs Discussion",
            "risk_level": "Medium",
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
        assert_eq!(result.insights[4].kind, InsightKind::Context);
    }

    #[test]
    fn test_parse_insights_filters_invalid() {
        let json = r#"{
            "summary": "Test",
            "verdict": "LGTM",
            "risk_level": "Low",
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

    #[test]
    fn test_parse_suggested_change() {
        let json = r#"{
            "summary": "Test",
            "verdict": "Needs Work",
            "risk_level": "Medium",
            "groups": [{"label": "All", "files": ["a.rs"]}],
            "annotations": {},
            "insights": [
                {
                    "file": "a.rs",
                    "line": 5,
                    "kind": "suggestion",
                    "text": "Use constant",
                    "suggested_change": "const FOO = 42;"
                },
                {
                    "file": "a.rs",
                    "line": 6,
                    "kind": "context",
                    "text": "No suggestion here"
                }
            ]
        }"#;

        let result = parse_walkthrough_response(json).unwrap();
        assert_eq!(result.insights.len(), 2);
        assert_eq!(
            result.insights[0].suggested_change,
            Some("const FOO = 42;".to_string())
        );
        assert_eq!(result.insights[1].suggested_change, None);
    }

    #[test]
    fn test_verdict_and_risk_fallbacks() {
        let json = r#"{
            "summary": "Test",
            "groups": [{"label": "All", "files": ["a.rs"]}],
            "annotations": {}
        }"#;

        let result = parse_walkthrough_response(json).unwrap();
        assert_eq!(result.verdict, ReviewVerdict::NeedsDiscussion);
        assert_eq!(result.risk_level, RiskLevel::Medium);
        assert!(result.top_concerns.is_empty());
    }
}
