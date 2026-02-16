//! Full snapshot format for R2 blob storage.
//!
//! Encodes a complete workspace snapshot (config + pane data + optional conversation)
//! as compressed binary (postcard + LZ4). Designed for blob storage, not URL encoding,
//! so full fidelity is preserved without truncation.

use serde::{Deserialize, Serialize};

use super::compact::CompactSnapshotWorkspace;
use super::{SnapshotPaneData, SnapshotSeries, WorkspaceConfig, WorkspaceError};

// =============================================================================
// Public Domain Types
// =============================================================================

/// Role of a message in a snapshot conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotMessageRole {
    User,
    Assistant,
    System,
}

/// Inline chart in a snapshot message.
#[derive(Debug, Clone)]
pub struct SnapshotInlineChart {
    pub title: String,
    /// Reuses the existing SnapshotSeries type (sorted tags, (f64,f64) points).
    pub series: Vec<SnapshotSeries>,
}

/// Inline source code preview in a snapshot message (no tree-sitter data).
#[derive(Debug, Clone)]
pub struct SnapshotInlineSource {
    pub file_path: String,
    pub line: usize,
    pub lines: Vec<String>,
    pub start_line: usize,
    pub language: String,
}

/// A single search result item.
#[derive(Debug, Clone)]
pub struct SnapshotSearchResultItem {
    pub kind: String,
    pub name: String,
    pub file_path: String,
    pub line: usize,
    pub score: f32,
    pub snippet: Option<String>,
}

/// Inline search results in a snapshot message.
#[derive(Debug, Clone)]
pub struct SnapshotInlineSearchResults {
    pub query: String,
    pub filter: String,
    pub results: Vec<SnapshotSearchResultItem>,
}

/// Diff line kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotDiffLineKind {
    Context,
    Addition,
    Deletion,
    Hunk,
}

/// A single line in a diff.
#[derive(Debug, Clone)]
pub struct SnapshotDiffLine {
    pub content: String,
    pub kind: SnapshotDiffLineKind,
}

/// A file in a diff.
#[derive(Debug, Clone)]
pub struct SnapshotDiffFile {
    pub path: String,
    pub lines: Vec<SnapshotDiffLine>,
    pub additions: usize,
    pub deletions: usize,
}

/// Inline diff in a snapshot message.
#[derive(Debug, Clone)]
pub struct SnapshotInlineDiff {
    pub commit_hash: String,
    pub commit_message: String,
    pub file_diffs: Vec<SnapshotDiffFile>,
    pub additions: usize,
    pub deletions: usize,
}

/// Inline content in a snapshot message.
#[derive(Debug, Clone)]
pub enum SnapshotInlineContent {
    Chart(SnapshotInlineChart),
    Source(SnapshotInlineSource),
    SearchResults(SnapshotInlineSearchResults),
    Diff(SnapshotInlineDiff),
}

/// A message in a snapshot conversation.
#[derive(Debug, Clone)]
pub struct SnapshotMessage {
    pub role: SnapshotMessageRole,
    pub content: String,
    pub inline_blocks: Vec<SnapshotInlineContent>,
}

/// Conversation data in a snapshot.
#[derive(Debug, Clone)]
pub struct SnapshotConversation {
    pub name: String,
    pub messages: Vec<SnapshotMessage>,
}

/// A decoded full snapshot.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub workspace: WorkspaceConfig,
    pub captured_at: u64,
    pub conversation: Option<SnapshotConversation>,
}

// =============================================================================
// Compact Encoding Types (postcard-serializable)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CompactMessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CompactDiffLineKind {
    Context,
    Addition,
    Deletion,
    Hunk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactInlineChart {
    pub title: String,
    pub series: Vec<CompactChartSeries>,
}

/// Simplified chart series for inline charts. Uses f64 for full fidelity
/// (blob storage has no size pressure — clarity over compactness).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactChartSeries {
    pub name: String,
    pub tags: Vec<(String, String)>,
    pub points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactInlineSource {
    pub file_path: String,
    pub line: u32,
    pub start_line: u32,
    pub language: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactSearchResult {
    pub kind: String,
    pub name: String,
    pub file_path: String,
    pub line: u32,
    pub score: f32,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactInlineSearchResults {
    pub query: String,
    pub filter: String,
    pub results: Vec<CompactSearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactDiffLine {
    pub content: String,
    pub kind: CompactDiffLineKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactDiffFile {
    pub path: String,
    pub lines: Vec<CompactDiffLine>,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactInlineDiff {
    pub commit_hash: String,
    pub commit_message: String,
    pub file_diffs: Vec<CompactDiffFile>,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CompactInlineContent {
    Chart(CompactInlineChart),
    Source(CompactInlineSource),
    SearchResults(CompactInlineSearchResults),
    Diff(CompactInlineDiff),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactSnapshotMessage {
    pub role: CompactMessageRole,
    pub content: String,
    pub inline_blocks: Vec<CompactInlineContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactSnapshotConversation {
    pub name: String,
    pub messages: Vec<CompactSnapshotMessage>,
}

/// Top-level blob snapshot: workspace config + pane data + optional conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactFullSnapshot {
    pub workspace: CompactSnapshotWorkspace,
    pub conversation: Option<CompactSnapshotConversation>,
}

// =============================================================================
// Encode / Decode
// =============================================================================

/// Encode a full snapshot to compressed binary (postcard + LZ4).
///
/// The resulting bytes are meant for blob storage (R2), not URL encoding.
pub fn encode_snapshot(
    ws: &WorkspaceConfig,
    pane_data: &[SnapshotPaneData],
    captured_at: u64,
    conversation: Option<&SnapshotConversation>,
) -> Result<Vec<u8>, WorkspaceError> {
    let mut compact_ws = CompactSnapshotWorkspace::from_workspace(ws, pane_data);
    compact_ws.captured_at = captured_at;

    let compact_convo = conversation.map(|c| CompactSnapshotConversation {
        name: c.name.clone(),
        messages: c
            .messages
            .iter()
            .map(|m| CompactSnapshotMessage {
                role: match m.role {
                    SnapshotMessageRole::User => CompactMessageRole::User,
                    SnapshotMessageRole::Assistant => CompactMessageRole::Assistant,
                    SnapshotMessageRole::System => CompactMessageRole::System,
                },
                content: m.content.clone(),
                inline_blocks: m.inline_blocks.iter().map(encode_inline_content).collect(),
            })
            .collect(),
    });

    let full = CompactFullSnapshot {
        workspace: compact_ws,
        conversation: compact_convo,
    };

    let bytes = postcard::to_allocvec(&full).map_err(|e| WorkspaceError::Encode(e.to_string()))?;
    Ok(lz4_flex::compress_prepend_size(&bytes))
}

/// Decode a full snapshot from compressed binary.
pub fn decode_snapshot(bytes: &[u8]) -> Result<Snapshot, WorkspaceError> {
    let decompressed = lz4_flex::decompress_size_prepended(bytes)
        .map_err(|e| WorkspaceError::Decode(e.to_string()))?;
    let full: CompactFullSnapshot =
        postcard::from_bytes(&decompressed).map_err(|e| WorkspaceError::Decode(e.to_string()))?;

    let mut ws = full.workspace.into_workspace();

    let conversation = full.conversation.map(|c| SnapshotConversation {
        name: c.name,
        messages: c
            .messages
            .into_iter()
            .map(|m| SnapshotMessage {
                role: match m.role {
                    CompactMessageRole::User => SnapshotMessageRole::User,
                    CompactMessageRole::Assistant => SnapshotMessageRole::Assistant,
                    CompactMessageRole::System => SnapshotMessageRole::System,
                },
                content: m.content,
                inline_blocks: m
                    .inline_blocks
                    .into_iter()
                    .map(decode_inline_content)
                    .collect(),
            })
            .collect(),
    });

    let captured_at = ws.snapshot.as_ref().map_or(0, |s| s.captured_at);

    // Attach conversation to the snapshot meta
    if let Some(ref mut snapshot) = ws.snapshot {
        snapshot.conversation = conversation.clone();
    }

    Ok(Snapshot {
        workspace: ws,
        captured_at,
        conversation,
    })
}

// =============================================================================
// Inline Content Conversion Helpers
// =============================================================================

fn encode_inline_content(content: &SnapshotInlineContent) -> CompactInlineContent {
    match content {
        SnapshotInlineContent::Chart(chart) => CompactInlineContent::Chart(CompactInlineChart {
            title: chart.title.clone(),
            series: chart
                .series
                .iter()
                .map(|s| CompactChartSeries {
                    name: s.name.clone(),
                    tags: s.tags.clone(),
                    points: s.points.clone(),
                })
                .collect(),
        }),
        SnapshotInlineContent::Source(src) => CompactInlineContent::Source(CompactInlineSource {
            file_path: src.file_path.clone(),
            line: src.line as u32,
            start_line: src.start_line as u32,
            language: src.language.clone(),
            lines: src.lines.clone(),
        }),
        SnapshotInlineContent::SearchResults(sr) => {
            CompactInlineContent::SearchResults(CompactInlineSearchResults {
                query: sr.query.clone(),
                filter: sr.filter.clone(),
                results: sr
                    .results
                    .iter()
                    .map(|r| CompactSearchResult {
                        kind: r.kind.clone(),
                        name: r.name.clone(),
                        file_path: r.file_path.clone(),
                        line: r.line as u32,
                        score: r.score,
                        snippet: r.snippet.clone(),
                    })
                    .collect(),
            })
        }
        SnapshotInlineContent::Diff(diff) => CompactInlineContent::Diff(CompactInlineDiff {
            commit_hash: diff.commit_hash.clone(),
            commit_message: diff.commit_message.clone(),
            file_diffs: diff
                .file_diffs
                .iter()
                .map(|f| CompactDiffFile {
                    path: f.path.clone(),
                    lines: f
                        .lines
                        .iter()
                        .map(|l| CompactDiffLine {
                            content: l.content.clone(),
                            kind: match l.kind {
                                SnapshotDiffLineKind::Context => CompactDiffLineKind::Context,
                                SnapshotDiffLineKind::Addition => CompactDiffLineKind::Addition,
                                SnapshotDiffLineKind::Deletion => CompactDiffLineKind::Deletion,
                                SnapshotDiffLineKind::Hunk => CompactDiffLineKind::Hunk,
                            },
                        })
                        .collect(),
                    additions: f.additions as u32,
                    deletions: f.deletions as u32,
                })
                .collect(),
            additions: diff.additions as u32,
            deletions: diff.deletions as u32,
        }),
    }
}

fn decode_inline_content(content: CompactInlineContent) -> SnapshotInlineContent {
    match content {
        CompactInlineContent::Chart(chart) => SnapshotInlineContent::Chart(SnapshotInlineChart {
            title: chart.title,
            series: chart
                .series
                .into_iter()
                .map(|s| SnapshotSeries {
                    name: s.name,
                    tags: s.tags,
                    points: s.points,
                })
                .collect(),
        }),
        CompactInlineContent::Source(src) => SnapshotInlineContent::Source(SnapshotInlineSource {
            file_path: src.file_path,
            line: src.line as usize,
            start_line: src.start_line as usize,
            language: src.language,
            lines: src.lines,
        }),
        CompactInlineContent::SearchResults(sr) => {
            SnapshotInlineContent::SearchResults(SnapshotInlineSearchResults {
                query: sr.query,
                filter: sr.filter,
                results: sr
                    .results
                    .into_iter()
                    .map(|r| SnapshotSearchResultItem {
                        kind: r.kind,
                        name: r.name,
                        file_path: r.file_path,
                        line: r.line as usize,
                        score: r.score,
                        snippet: r.snippet,
                    })
                    .collect(),
            })
        }
        CompactInlineContent::Diff(diff) => SnapshotInlineContent::Diff(SnapshotInlineDiff {
            commit_hash: diff.commit_hash,
            commit_message: diff.commit_message,
            file_diffs: diff
                .file_diffs
                .into_iter()
                .map(|f| SnapshotDiffFile {
                    path: f.path,
                    lines: f
                        .lines
                        .into_iter()
                        .map(|l| SnapshotDiffLine {
                            content: l.content,
                            kind: match l.kind {
                                CompactDiffLineKind::Context => SnapshotDiffLineKind::Context,
                                CompactDiffLineKind::Addition => SnapshotDiffLineKind::Addition,
                                CompactDiffLineKind::Deletion => SnapshotDiffLineKind::Deletion,
                                CompactDiffLineKind::Hunk => SnapshotDiffLineKind::Hunk,
                            },
                        })
                        .collect(),
                    additions: f.additions as usize,
                    deletions: f.deletions as usize,
                })
                .collect(),
            additions: diff.additions as usize,
            deletions: diff.deletions as usize,
        }),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{SnapshotPaneData, SnapshotSeries, WorkspaceConfig};

    fn make_test_workspace() -> (WorkspaceConfig, Vec<SnapshotPaneData>) {
        let mut ws = WorkspaceConfig::new("test-snapshot".to_string());
        ws.time.preset = "1h".to_string();
        ws.view.theme = "dark".to_string();

        use crate::workspace::PaneConfig;
        let mut pane = PaneConfig::new("rate(http_requests_total[5m])");
        pane.name = "Request Rate".to_string();
        ws.panes = vec![pane];

        let pane_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![SnapshotSeries {
                name: "http_requests_total".to_string(),
                tags: vec![("instance".to_string(), "server1".to_string())],
                points: (0..50)
                    .map(|i| (1000.0 + i as f64 * 60.0, i as f64 * 1.5))
                    .collect(),
            }],
        }];

        (ws, pane_data)
    }

    fn make_test_conversation() -> SnapshotConversation {
        SnapshotConversation {
            name: "Debug latency spike".to_string(),
            messages: vec![
                SnapshotMessage {
                    role: SnapshotMessageRole::User,
                    content: "What's causing the latency spike?".to_string(),
                    inline_blocks: vec![],
                },
                SnapshotMessage {
                    role: SnapshotMessageRole::Assistant,
                    content: "I can see a correlation with increased error rates.".to_string(),
                    inline_blocks: vec![
                        SnapshotInlineContent::Chart(SnapshotInlineChart {
                            title: "Error Rate".to_string(),
                            series: vec![SnapshotSeries {
                                name: "errors".to_string(),
                                tags: vec![("host".to_string(), "web-1".to_string())],
                                points: (0..20)
                                    .map(|i| (1000.0 + i as f64 * 60.0, i as f64 * 0.1))
                                    .collect(),
                            }],
                        }),
                        SnapshotInlineContent::Source(SnapshotInlineSource {
                            file_path: "src/handlers/api.rs".to_string(),
                            line: 42,
                            lines: vec![
                                "fn handle_request(req: Request) -> Response {".to_string(),
                                "    let start = Instant::now();".to_string(),
                                "    let result = process(req);".to_string(),
                                "    metrics::record_latency(start.elapsed());".to_string(),
                                "    result".to_string(),
                                "}".to_string(),
                            ],
                            start_line: 40,
                            language: "rust".to_string(),
                        }),
                    ],
                },
            ],
        }
    }

    #[test]
    fn round_trip_snapshot_without_conversation() {
        let (ws, pane_data) = make_test_workspace();

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, None).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        assert_eq!(decoded.captured_at, 1700000000);
        assert!(decoded.conversation.is_none());
        assert_eq!(decoded.workspace.workspace.name, "test-snapshot");
        assert_eq!(decoded.workspace.panes.len(), 1);
        assert_eq!(
            decoded.workspace.panes[0].query,
            "rate(http_requests_total[5m])"
        );

        // Verify snapshot pane data survived
        let snapshot = decoded.workspace.snapshot.as_ref().unwrap();
        assert_eq!(snapshot.pane_data.len(), 1);
        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                assert_eq!(series.len(), 1);
                assert_eq!(series[0].name, "http_requests_total");
                assert_eq!(series[0].points.len(), 50);
            }
            _ => panic!("expected TimeSeries"),
        }
    }

    #[test]
    fn round_trip_snapshot_with_conversation() {
        let (ws, pane_data) = make_test_workspace();
        let convo = make_test_conversation();

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo)).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        let convo_out = decoded.conversation.as_ref().unwrap();
        assert_eq!(convo_out.name, "Debug latency spike");
        assert_eq!(convo_out.messages.len(), 2);

        // First message: user, no inline blocks
        assert_eq!(convo_out.messages[0].role, SnapshotMessageRole::User);
        assert_eq!(
            convo_out.messages[0].content,
            "What's causing the latency spike?"
        );
        assert!(convo_out.messages[0].inline_blocks.is_empty());

        // Second message: assistant, with chart + source
        assert_eq!(convo_out.messages[1].role, SnapshotMessageRole::Assistant);
        assert_eq!(convo_out.messages[1].inline_blocks.len(), 2);

        // Verify inline chart
        match &convo_out.messages[1].inline_blocks[0] {
            SnapshotInlineContent::Chart(chart) => {
                assert_eq!(chart.title, "Error Rate");
                assert_eq!(chart.series.len(), 1);
                assert_eq!(chart.series[0].name, "errors");
                assert_eq!(chart.series[0].tags.len(), 1);
                assert_eq!(chart.series[0].points.len(), 20);
            }
            _ => panic!("expected Chart"),
        }

        // Verify inline source
        match &convo_out.messages[1].inline_blocks[1] {
            SnapshotInlineContent::Source(src) => {
                assert_eq!(src.file_path, "src/handlers/api.rs");
                assert_eq!(src.line, 42);
                assert_eq!(src.start_line, 40);
                assert_eq!(src.language, "rust");
                assert_eq!(src.lines.len(), 6);
            }
            _ => panic!("expected Source"),
        }
    }

    #[test]
    fn round_trip_inline_search_results() {
        let (ws, pane_data) = make_test_workspace();
        let convo = SnapshotConversation {
            name: "search test".to_string(),
            messages: vec![SnapshotMessage {
                role: SnapshotMessageRole::Assistant,
                content: "Found these results.".to_string(),
                inline_blocks: vec![SnapshotInlineContent::SearchResults(
                    SnapshotInlineSearchResults {
                        query: "http_requests".to_string(),
                        filter: "metrics".to_string(),
                        results: vec![
                            SnapshotSearchResultItem {
                                kind: "metric".to_string(),
                                name: "http_requests_total".to_string(),
                                file_path: "src/metrics.rs".to_string(),
                                line: 15,
                                score: 0.95,
                                snippet: Some("counter!(http_requests_total)".to_string()),
                            },
                            SnapshotSearchResultItem {
                                kind: "metric".to_string(),
                                name: "http_requests_duration".to_string(),
                                file_path: "src/metrics.rs".to_string(),
                                line: 20,
                                score: 0.8,
                                snippet: None,
                            },
                        ],
                    },
                )],
            }],
        };

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo)).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        let msg = &decoded.conversation.as_ref().unwrap().messages[0];
        match &msg.inline_blocks[0] {
            SnapshotInlineContent::SearchResults(sr) => {
                assert_eq!(sr.query, "http_requests");
                assert_eq!(sr.filter, "metrics");
                assert_eq!(sr.results.len(), 2);
                assert_eq!(sr.results[0].name, "http_requests_total");
                assert!((sr.results[0].score - 0.95).abs() < 0.001);
                assert_eq!(
                    sr.results[0].snippet.as_deref(),
                    Some("counter!(http_requests_total)")
                );
                assert!(sr.results[1].snippet.is_none());
            }
            _ => panic!("expected SearchResults"),
        }
    }

    #[test]
    fn round_trip_inline_diff() {
        let (ws, pane_data) = make_test_workspace();
        let convo = SnapshotConversation {
            name: "diff test".to_string(),
            messages: vec![SnapshotMessage {
                role: SnapshotMessageRole::Assistant,
                content: "Here's the recent commit.".to_string(),
                inline_blocks: vec![SnapshotInlineContent::Diff(SnapshotInlineDiff {
                    commit_hash: "abc123".to_string(),
                    commit_message: "Fix timeout handling".to_string(),
                    file_diffs: vec![SnapshotDiffFile {
                        path: "src/server.rs".to_string(),
                        lines: vec![
                            SnapshotDiffLine {
                                content: "@@ -10,3 +10,5 @@".to_string(),
                                kind: SnapshotDiffLineKind::Hunk,
                            },
                            SnapshotDiffLine {
                                content: "    let timeout = Duration::from_secs(30);".to_string(),
                                kind: SnapshotDiffLineKind::Context,
                            },
                            SnapshotDiffLine {
                                content: "    let resp = client.get(url).await?;".to_string(),
                                kind: SnapshotDiffLineKind::Deletion,
                            },
                            SnapshotDiffLine {
                                content: "    let resp = client.get(url).timeout(timeout).await?;"
                                    .to_string(),
                                kind: SnapshotDiffLineKind::Addition,
                            },
                        ],
                        additions: 1,
                        deletions: 1,
                    }],
                    additions: 1,
                    deletions: 1,
                })],
            }],
        };

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo)).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        let msg = &decoded.conversation.as_ref().unwrap().messages[0];
        match &msg.inline_blocks[0] {
            SnapshotInlineContent::Diff(diff) => {
                assert_eq!(diff.commit_hash, "abc123");
                assert_eq!(diff.commit_message, "Fix timeout handling");
                assert_eq!(diff.file_diffs.len(), 1);
                assert_eq!(diff.file_diffs[0].path, "src/server.rs");
                assert_eq!(diff.file_diffs[0].lines.len(), 4);
                assert_eq!(diff.file_diffs[0].lines[0].kind, SnapshotDiffLineKind::Hunk);
                assert_eq!(
                    diff.file_diffs[0].lines[2].kind,
                    SnapshotDiffLineKind::Deletion
                );
                assert_eq!(
                    diff.file_diffs[0].lines[3].kind,
                    SnapshotDiffLineKind::Addition
                );
                assert_eq!(diff.additions, 1);
                assert_eq!(diff.deletions, 1);
            }
            _ => panic!("expected Diff"),
        }
    }

    #[test]
    fn chart_data_preserved_in_blob() {
        let (ws, pane_data) = make_test_workspace();
        let convo = SnapshotConversation {
            name: "preserve test".to_string(),
            messages: vec![SnapshotMessage {
                role: SnapshotMessageRole::Assistant,
                content: "Large chart.".to_string(),
                inline_blocks: vec![SnapshotInlineContent::Chart(SnapshotInlineChart {
                    title: "Big Series".to_string(),
                    series: vec![SnapshotSeries {
                        name: "big".to_string(),
                        tags: vec![],
                        points: (0..1000).map(|i| (i as f64, (i as f64).sin())).collect(),
                    }],
                })],
            }],
        };

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo)).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        let msg = &decoded.conversation.as_ref().unwrap().messages[0];
        match &msg.inline_blocks[0] {
            SnapshotInlineContent::Chart(chart) => {
                assert_eq!(
                    chart.series[0].points.len(),
                    1000,
                    "All points should be preserved in blob snapshots"
                );
            }
            _ => panic!("expected Chart"),
        }
    }

    #[test]
    fn empty_conversation_messages() {
        let (ws, pane_data) = make_test_workspace();
        let convo = SnapshotConversation {
            name: "empty".to_string(),
            messages: vec![],
        };

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo)).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        let convo_out = decoded.conversation.as_ref().unwrap();
        assert_eq!(convo_out.name, "empty");
        assert!(convo_out.messages.is_empty());
    }

    #[test]
    fn snapshot_pane_data_preserved_with_conversation() {
        let (ws, _) = make_test_workspace();
        let pane_data = vec![SnapshotPaneData::Stat {
            value: 42.5,
            sparkline: vec![1.0, 2.0, 3.0, 4.0, 5.0],
        }];
        let convo = make_test_conversation();

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo)).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        let snapshot = decoded.workspace.snapshot.as_ref().unwrap();
        match &snapshot.pane_data[0] {
            SnapshotPaneData::Stat { value, sparkline } => {
                assert!((value - 42.5).abs() < 0.1);
                assert_eq!(sparkline.len(), 5);
            }
            _ => panic!("expected Stat"),
        }

        assert!(decoded.conversation.is_some());
    }
}
