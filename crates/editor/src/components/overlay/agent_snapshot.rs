//! Snapshot extraction and restoration for the agent panel.
//!
//! Converts live `ChatMessage` / `InlineContent` types into the snapshot-friendly
//! types from `enya_config`, which can be serialized for blob storage.
//! Also handles the reverse: restoring a conversation from snapshot data.

use enya_config::{
    SnapshotConversation, SnapshotDiffFile, SnapshotDiffLine, SnapshotDiffLineKind,
    SnapshotInlineChart, SnapshotInlineContent, SnapshotInlineDiff, SnapshotInlineSearchResults,
    SnapshotInlineSource, SnapshotInlineTable, SnapshotMessage, SnapshotMessageRole,
    SnapshotSearchResultItem, SnapshotSeries, SnapshotTableColumn,
};

use super::agent_context::strip_command_blocks;
use super::agent_panel::{AgentPanel, ChatMessage};
use crate::components::pane::time_series_chart::{DataPoint, Series};
use crate::components::pane::{
    InlineChart, InlineContent, InlineDiff, InlineDiffFile, InlineDiffLine, InlineDiffLineKind,
    InlineSearchResults, InlineSource, InlineTable, InlineTableColumn, SearchResultItem,
};
use crate::components::util::{MessageRole, SyntaxHighlightData};

impl AgentPanel {
    /// Extract the current conversation as snapshot data for blob storage.
    ///
    /// Returns `None` if there are no messages. Strips `enya-command` blocks
    /// from message content and converts inline content to snapshot-friendly types.
    pub fn extract_snapshot_conversation(&self) -> Option<SnapshotConversation> {
        if self.messages.is_empty() {
            return None;
        }

        let thread_name = self
            .conversation_store
            .active_thread()
            .map(|t| t.name.clone())
            .unwrap_or_default();

        let messages = self
            .messages
            .iter()
            .filter(|m| !m.is_streaming)
            .map(|m| SnapshotMessage {
                role: convert_role(m.role),
                content: strip_command_blocks(&m.content),
                inline_blocks: m
                    .inline_blocks
                    .iter()
                    .filter_map(convert_inline_content)
                    .collect(),
            })
            .collect();

        Some(SnapshotConversation {
            name: thread_name,
            messages,
        })
    }

    /// Load a conversation from snapshot data into the agent panel.
    ///
    /// Clears existing messages and replaces them with the snapshot conversation,
    /// converting snapshot types back to the live `ChatMessage` / `InlineContent` types.
    pub fn load_snapshot_conversation(&mut self, conversation: &SnapshotConversation) {
        self.messages.clear();
        self.current_activities.clear();
        self.response_text.clear();

        for msg in &conversation.messages {
            self.messages.push(ChatMessage {
                role: restore_role(msg.role),
                content: msg.content.clone(),
                is_streaming: false,
                inline_blocks: msg
                    .inline_blocks
                    .iter()
                    .map(restore_inline_content)
                    .collect(),
            });
        }

        self.is_open = true;
        self.scroll_to_bottom = true;

        log::info!(
            "Loaded snapshot conversation '{}': {} messages",
            conversation.name,
            conversation.messages.len()
        );
    }
}

fn convert_role(role: MessageRole) -> SnapshotMessageRole {
    match role {
        MessageRole::User => SnapshotMessageRole::User,
        MessageRole::Assistant => SnapshotMessageRole::Assistant,
        MessageRole::System => SnapshotMessageRole::System,
    }
}

fn convert_inline_content(content: &InlineContent) -> Option<SnapshotInlineContent> {
    match content {
        InlineContent::Chart(chart) => Some(SnapshotInlineContent::Chart(SnapshotInlineChart {
            title: chart.title.clone(),
            series: chart
                .series
                .iter()
                .map(|s| {
                    let mut tags: Vec<(String, String)> =
                        s.tags.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    tags.sort_by(|a, b| a.0.cmp(&b.0));

                    SnapshotSeries {
                        name: s.name.clone(),
                        tags,
                        points: s.points.iter().map(|p| (p.timestamp, p.value)).collect(),
                    }
                })
                .collect(),
        })),
        InlineContent::Source(src) => Some(SnapshotInlineContent::Source(SnapshotInlineSource {
            file_path: src.file_path.clone(),
            line: src.line,
            lines: src.lines.clone(),
            start_line: src.start_line,
            language: src.language.clone(),
        })),
        InlineContent::SearchResults(sr) => Some(SnapshotInlineContent::SearchResults(
            SnapshotInlineSearchResults {
                query: sr.query.clone(),
                filter: sr.filter.clone(),
                results: sr
                    .results
                    .iter()
                    .map(|r| SnapshotSearchResultItem {
                        kind: r.kind.clone(),
                        name: r.name.clone(),
                        file_path: r.file_path.clone(),
                        line: r.line,
                        score: r.score,
                        snippet: r.snippet.clone(),
                    })
                    .collect(),
            },
        )),
        InlineContent::Diff(diff) => Some(SnapshotInlineContent::Diff(SnapshotInlineDiff {
            commit_hash: diff.commit_hash.clone(),
            commit_message: diff.commit_message.clone(),
            file_diffs: diff
                .file_diffs
                .iter()
                .map(|f| SnapshotDiffFile {
                    path: f.path.clone(),
                    lines: f
                        .lines
                        .iter()
                        .map(|l| SnapshotDiffLine {
                            content: l.content.clone(),
                            kind: match l.kind {
                                InlineDiffLineKind::Context => SnapshotDiffLineKind::Context,
                                InlineDiffLineKind::Addition => SnapshotDiffLineKind::Addition,
                                InlineDiffLineKind::Deletion => SnapshotDiffLineKind::Deletion,
                                InlineDiffLineKind::Hunk => SnapshotDiffLineKind::Hunk,
                            },
                        })
                        .collect(),
                    additions: f.additions,
                    deletions: f.deletions,
                })
                .collect(),
            additions: diff.additions,
            deletions: diff.deletions,
        })),
        InlineContent::Table(table) => Some(SnapshotInlineContent::Table(SnapshotInlineTable {
            title: table.title.clone(),
            columns: table
                .columns
                .iter()
                .map(|c| SnapshotTableColumn {
                    name: c.name.clone(),
                    data_type: c.data_type.clone(),
                })
                .collect(),
            rows: table.rows.clone(),
            total_rows: table.total_rows as u64,
            execution_time_ms: table.execution_time_ms,
        })),
    }
}

// =============================================================================
// Restore: Snapshot types → Live types
// =============================================================================

fn restore_role(role: SnapshotMessageRole) -> MessageRole {
    match role {
        SnapshotMessageRole::User => MessageRole::User,
        SnapshotMessageRole::Assistant => MessageRole::Assistant,
        SnapshotMessageRole::System => MessageRole::System,
    }
}

fn restore_inline_content(content: &SnapshotInlineContent) -> InlineContent {
    match content {
        SnapshotInlineContent::Chart(chart) => InlineContent::Chart(InlineChart {
            title: chart.title.clone(),
            series: chart
                .series
                .iter()
                .map(|s| {
                    let mut series = Series::new(s.name.clone());
                    series.tags = s.tags.iter().cloned().collect();
                    series.points = s
                        .points
                        .iter()
                        .map(|&(t, v)| DataPoint {
                            timestamp: t,
                            value: v,
                        })
                        .collect();
                    series
                })
                .collect(),
            height: None,
        }),
        SnapshotInlineContent::Source(src) => {
            let source_text = src.lines.join("\n");
            InlineContent::Source(InlineSource {
                file_path: src.file_path.clone(),
                line: src.line,
                lines: src.lines.clone(),
                start_line: src.start_line,
                language: src.language.clone(),
                highlight_data: SyntaxHighlightData::new(&source_text, &src.language),
            })
        }
        SnapshotInlineContent::SearchResults(sr) => {
            InlineContent::SearchResults(InlineSearchResults {
                query: sr.query.clone(),
                filter: sr.filter.clone(),
                results: sr
                    .results
                    .iter()
                    .map(|r| SearchResultItem {
                        kind: r.kind.clone(),
                        name: r.name.clone(),
                        file_path: r.file_path.clone(),
                        line: r.line,
                        score: r.score,
                        snippet: r.snippet.clone(),
                    })
                    .collect(),
            })
        }
        SnapshotInlineContent::Diff(diff) => InlineContent::Diff(InlineDiff {
            commit_hash: diff.commit_hash.clone(),
            commit_message: diff.commit_message.clone(),
            file_diffs: diff
                .file_diffs
                .iter()
                .map(|f| InlineDiffFile {
                    path: f.path.clone(),
                    lines: f
                        .lines
                        .iter()
                        .map(|l| InlineDiffLine {
                            content: l.content.clone(),
                            kind: match l.kind {
                                SnapshotDiffLineKind::Context => InlineDiffLineKind::Context,
                                SnapshotDiffLineKind::Addition => InlineDiffLineKind::Addition,
                                SnapshotDiffLineKind::Deletion => InlineDiffLineKind::Deletion,
                                SnapshotDiffLineKind::Hunk => InlineDiffLineKind::Hunk,
                            },
                            old_line: None,
                            new_line: None,
                        })
                        .collect(),
                    additions: f.additions,
                    deletions: f.deletions,
                })
                .collect(),
            additions: diff.additions,
            deletions: diff.deletions,
        }),
        SnapshotInlineContent::Table(table) => InlineContent::Table(InlineTable {
            title: table.title.clone(),
            columns: table
                .columns
                .iter()
                .map(|c| InlineTableColumn {
                    name: c.name.clone(),
                    data_type: c.data_type.clone(),
                })
                .collect(),
            rows: table.rows.clone(),
            total_rows: table.total_rows as usize,
            execution_time_ms: table.execution_time_ms,
        }),
    }
}
