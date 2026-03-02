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

/// Column definition for a snapshot table.
#[derive(Debug, Clone)]
pub struct SnapshotTableColumn {
    pub name: String,
    pub data_type: String,
}

/// Inline SQL result table in a snapshot message.
#[derive(Debug, Clone)]
pub struct SnapshotInlineTable {
    pub title: String,
    pub columns: Vec<SnapshotTableColumn>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: u64,
    pub execution_time_ms: Option<u64>,
}

/// Execution statistics for a snapshot query cell.
#[derive(Debug, Clone)]
pub struct SnapshotQueryStats {
    pub total_time_ms: u64,
    pub planning_time_ms: u64,
    pub execution_time_ms: u64,
    pub rows_returned: u64,
    pub bytes_scanned: u64,
    pub partitions_scanned: u32,
}

/// Operator metrics in a snapshot plan node.
#[derive(Debug, Clone)]
pub struct SnapshotOperatorMetrics {
    pub output_rows: u64,
    pub elapsed_time_ms: u64,
    pub memory_bytes: u64,
    pub spill_count: u32,
    pub spill_bytes: u64,
}

/// A node in a query execution plan tree (snapshot-friendly).
#[derive(Debug, Clone)]
pub struct SnapshotPlanNode {
    pub operator: String,
    pub description: String,
    pub properties: Vec<(String, String)>,
    pub children: Vec<SnapshotPlanNode>,
    pub metrics: Option<SnapshotOperatorMetrics>,
}

/// Phase timing data for a snapshot benchmark.
#[derive(Debug, Clone)]
pub struct SnapshotPhaseTiming {
    /// Minimum duration in microseconds.
    pub min_us: u64,
    /// Maximum duration in microseconds.
    pub max_us: u64,
    /// Mean duration in microseconds.
    pub mean_us: u64,
    /// Median duration in microseconds.
    pub median_us: u64,
    /// Percentage of total execution time (0.0–100.0).
    pub percent_of_total: f64,
}

/// Benchmark statistics for a snapshot cell.
#[derive(Debug, Clone)]
pub struct SnapshotBenchmarkData {
    /// Number of iterations run.
    pub iterations: u64,
    /// Rows returned per iteration.
    pub rows_per_iteration: u64,
    /// Logical planning phase timings.
    pub logical_planning: SnapshotPhaseTiming,
    /// Physical planning phase timings.
    pub physical_planning: SnapshotPhaseTiming,
    /// Execution phase timings.
    pub execution: SnapshotPhaseTiming,
    /// Total (end-to-end) timings.
    pub total: SnapshotPhaseTiming,
}

/// Per-column statistics for a snapshot describe cell.
#[derive(Debug, Clone)]
pub struct SnapshotColumnStats {
    /// Column name.
    pub name: String,
    /// Data type as string.
    pub data_type: String,
    /// Total non-null count.
    pub count: u64,
    /// Number of null values.
    pub null_count: u64,
    /// Number of distinct values.
    pub distinct_count: u64,
    /// Minimum value as string.
    pub min: Option<String>,
    /// Maximum value as string.
    pub max: Option<String>,
    /// Mean value (numeric columns only).
    pub mean: Option<f64>,
}

/// Describe statistics for a snapshot cell.
#[derive(Debug, Clone)]
pub struct SnapshotDescribeData {
    /// Table name that was described.
    pub table_name: String,
    /// Total row count.
    pub total_rows: u64,
    /// Per-column statistics.
    pub columns: Vec<SnapshotColumnStats>,
    /// Time taken in milliseconds.
    pub elapsed_ms: u64,
}

/// Cell kind discriminant for snapshot SQL cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotCellKind {
    /// Standard query with tabular results.
    #[default]
    Query,
    /// Info or system message.
    Info,
    /// Diff comparison between two connections.
    Diff,
    /// Explain/analyze execution plan.
    Explain,
    /// Benchmark results with per-phase timing.
    Benchmark,
    /// Describe table statistics.
    Describe,
}

/// Type of diff comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotDiffType {
    /// Query result comparison.
    #[default]
    Data,
    /// Execution plan comparison.
    Plan,
    /// Table schema comparison.
    Schema,
    /// EXPLAIN ANALYZE profile comparison.
    Profile,
}

/// Row-level diff statistics.
#[derive(Debug, Clone)]
pub struct SnapshotDiffStats {
    pub left_only: u64,
    pub right_only: u64,
    pub different: u64,
    pub matching: u64,
}

/// Column diff status in a schema comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotColumnDiffStatus {
    Matching,
    LeftOnly,
    RightOnly,
    Changed,
}

/// A column in a schema diff.
#[derive(Debug, Clone)]
pub struct SnapshotSchemaDiffColumn {
    pub name: String,
    pub left_type: Option<String>,
    pub left_nullable: Option<bool>,
    pub right_type: Option<String>,
    pub right_nullable: Option<bool>,
    pub status: SnapshotColumnDiffStatus,
}

/// Schema diff result for a snapshot.
#[derive(Debug, Clone)]
pub struct SnapshotSchemaDiff {
    pub table_name: String,
    pub columns: Vec<SnapshotSchemaDiffColumn>,
    pub matching: u64,
    pub left_only: u64,
    pub right_only: u64,
    pub changed: u64,
}

/// Diff comparison data for a snapshot cell.
#[derive(Debug, Clone)]
pub struct SnapshotDiffData {
    pub left_name: String,
    pub right_name: String,
    pub left_columns: Vec<SnapshotTableColumn>,
    pub left_rows: Vec<Vec<String>>,
    pub left_total_rows: u64,
    pub left_error: Option<String>,
    pub right_columns: Vec<SnapshotTableColumn>,
    pub right_rows: Vec<Vec<String>>,
    pub right_total_rows: u64,
    pub right_error: Option<String>,
    pub schemas_match: bool,
    pub diff_stats: Option<SnapshotDiffStats>,
    pub left_plan: Option<SnapshotPlanNode>,
    pub right_plan: Option<SnapshotPlanNode>,
    pub diff_type: SnapshotDiffType,
    pub schema_diff: Option<SnapshotSchemaDiff>,
}

/// A single cell in a SQL pane snapshot (query, info, diff, explain, or benchmark).
#[derive(Debug, Clone)]
pub struct SnapshotQueryCell {
    /// Cell kind discriminant.
    pub kind: SnapshotCellKind,
    pub sql: String,
    pub columns: Vec<SnapshotTableColumn>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: u64,
    pub stats: Option<SnapshotQueryStats>,
    pub error: Option<String>,
    pub plan: Option<SnapshotPlanNode>,
    /// Diff comparison data (populated only for Diff cells).
    pub diff: Option<SnapshotDiffData>,
    /// Benchmark data (populated only for Benchmark cells).
    pub benchmark: Option<SnapshotBenchmarkData>,
    /// Describe data (populated only for Describe cells).
    pub describe: Option<SnapshotDescribeData>,
}

/// Snapshot data for a SQL pane (all query cells).
#[derive(Debug, Clone)]
pub struct SnapshotSqlPane {
    pub cells: Vec<SnapshotQueryCell>,
}

/// Inline content in a snapshot message.
#[derive(Debug, Clone)]
pub enum SnapshotInlineContent {
    Chart(SnapshotInlineChart),
    Source(SnapshotInlineSource),
    SearchResults(SnapshotInlineSearchResults),
    Diff(SnapshotInlineDiff),
    Table(SnapshotInlineTable),
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
struct CompactTableColumn {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactInlineTable {
    pub title: String,
    pub columns: Vec<CompactTableColumn>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: u64,
    pub execution_time_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactQueryStats {
    pub total_time_ms: u64,
    pub planning_time_ms: u64,
    pub execution_time_ms: u64,
    pub rows_returned: u64,
    pub bytes_scanned: u64,
    pub partitions_scanned: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactOperatorMetrics {
    pub output_rows: u64,
    pub elapsed_time_ms: u64,
    pub memory_bytes: u64,
    pub spill_count: u32,
    pub spill_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactPlanNode {
    pub operator: String,
    pub description: String,
    pub properties: Vec<(String, String)>,
    pub children: Vec<CompactPlanNode>,
    pub metrics: Option<CompactOperatorMetrics>,
}

// --- Compact types for kind-aware SQL cells ---

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum CompactCellKind {
    Query,
    Info,
    Diff,
    Explain,
    Benchmark,
    Describe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactPhaseTiming {
    pub min_us: u64,
    pub max_us: u64,
    pub mean_us: u64,
    pub median_us: u64,
    pub pct: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactBenchmarkData {
    pub iterations: u32,
    pub rows_per_iteration: u64,
    pub logical: CompactPhaseTiming,
    pub physical: CompactPhaseTiming,
    pub execution: CompactPhaseTiming,
    pub total: CompactPhaseTiming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactColumnStats {
    pub name: String,
    pub data_type: String,
    pub count: u64,
    pub null_count: u64,
    pub distinct_count: u64,
    pub min: Option<String>,
    pub max: Option<String>,
    pub mean: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactDescribeData {
    pub table_name: String,
    pub total_rows: u64,
    pub columns: Vec<CompactColumnStats>,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum CompactDiffType {
    Data,
    Plan,
    Schema,
    Profile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactDiffStats {
    pub left_only: u64,
    pub right_only: u64,
    pub different: u64,
    pub matching: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum CompactColumnDiffStatus {
    Matching,
    LeftOnly,
    RightOnly,
    Changed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactSchemaDiffColumn {
    pub name: String,
    pub left_type: Option<String>,
    pub left_nullable: Option<bool>,
    pub right_type: Option<String>,
    pub right_nullable: Option<bool>,
    pub status: CompactColumnDiffStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactSchemaDiff {
    pub table_name: String,
    pub columns: Vec<CompactSchemaDiffColumn>,
    pub matching: u64,
    pub left_only: u64,
    pub right_only: u64,
    pub changed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactDiffData {
    pub left_name: String,
    pub right_name: String,
    pub left_columns: Vec<CompactTableColumn>,
    pub left_rows: Vec<Vec<String>>,
    pub left_total_rows: u64,
    pub left_error: Option<String>,
    pub right_columns: Vec<CompactTableColumn>,
    pub right_rows: Vec<Vec<String>>,
    pub right_total_rows: u64,
    pub right_error: Option<String>,
    pub schemas_match: bool,
    pub diff_stats: Option<CompactDiffStats>,
    pub left_plan: Option<CompactPlanNode>,
    pub right_plan: Option<CompactPlanNode>,
    pub diff_type: CompactDiffType,
    pub schema_diff: Option<CompactSchemaDiff>,
}

/// V2 cell: kind-aware, with optional diff data.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactSqlCell {
    pub kind: CompactCellKind,
    pub sql: String,
    pub columns: Vec<CompactTableColumn>,
    pub rows: Vec<Vec<String>>,
    pub total_rows: u64,
    pub stats: Option<CompactQueryStats>,
    pub error: Option<String>,
    pub plan: Option<CompactPlanNode>,
    pub diff: Option<CompactDiffData>,
    #[serde(default)]
    pub benchmark: Option<CompactBenchmarkData>,
    #[serde(default)]
    pub describe: Option<CompactDescribeData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactSqlPane {
    pub cells: Vec<CompactSqlCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum CompactInlineContent {
    Chart(CompactInlineChart),
    Source(CompactInlineSource),
    SearchResults(CompactInlineSearchResults),
    Diff(CompactInlineDiff),
    Table(CompactInlineTable),
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
    pub sql_pane: Option<CompactSqlPane>,
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
    sql_pane: Option<&SnapshotSqlPane>,
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

    let compact_sql = sql_pane.map(encode_sql_pane);

    let full = CompactFullSnapshot {
        workspace: compact_ws,
        conversation: compact_convo,
        sql_pane: compact_sql,
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
    let conversation = full.conversation.map(decode_conversation);
    let sql_pane = full.sql_pane.map(decode_sql_pane);
    let captured_at = ws.snapshot.as_ref().map_or(0, |s| s.captured_at);
    if let Some(ref mut snapshot) = ws.snapshot {
        snapshot.conversation = conversation.clone();
        snapshot.sql_pane = sql_pane.clone();
    }

    Ok(Snapshot {
        workspace: ws,
        captured_at,
        conversation,
    })
}

fn decode_conversation(c: CompactSnapshotConversation) -> SnapshotConversation {
    SnapshotConversation {
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
    }
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
        SnapshotInlineContent::Table(table) => CompactInlineContent::Table(CompactInlineTable {
            title: table.title.clone(),
            columns: table
                .columns
                .iter()
                .map(|c| CompactTableColumn {
                    name: c.name.clone(),
                    data_type: c.data_type.clone(),
                })
                .collect(),
            rows: table.rows.clone(),
            total_rows: table.total_rows,
            execution_time_ms: table.execution_time_ms,
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
        CompactInlineContent::Table(table) => SnapshotInlineContent::Table(SnapshotInlineTable {
            title: table.title,
            columns: table
                .columns
                .into_iter()
                .map(|c| SnapshotTableColumn {
                    name: c.name,
                    data_type: c.data_type,
                })
                .collect(),
            rows: table.rows,
            total_rows: table.total_rows,
            execution_time_ms: table.execution_time_ms,
        }),
    }
}

// =============================================================================
// SQL Pane Conversion Helpers
// =============================================================================

fn encode_sql_pane(pane: &SnapshotSqlPane) -> CompactSqlPane {
    CompactSqlPane {
        cells: pane.cells.iter().map(encode_sql_cell).collect(),
    }
}

fn decode_sql_pane(pane: CompactSqlPane) -> SnapshotSqlPane {
    SnapshotSqlPane {
        cells: pane.cells.into_iter().map(decode_sql_cell).collect(),
    }
}

fn encode_phase_timing(p: &SnapshotPhaseTiming) -> CompactPhaseTiming {
    CompactPhaseTiming {
        min_us: p.min_us,
        max_us: p.max_us,
        mean_us: p.mean_us,
        median_us: p.median_us,
        pct: p.percent_of_total as f32,
    }
}

fn decode_phase_timing(p: CompactPhaseTiming) -> SnapshotPhaseTiming {
    SnapshotPhaseTiming {
        min_us: p.min_us,
        max_us: p.max_us,
        mean_us: p.mean_us,
        median_us: p.median_us,
        percent_of_total: p.pct as f64,
    }
}

fn encode_benchmark_data(b: &SnapshotBenchmarkData) -> CompactBenchmarkData {
    CompactBenchmarkData {
        iterations: b.iterations as u32,
        rows_per_iteration: b.rows_per_iteration,
        logical: encode_phase_timing(&b.logical_planning),
        physical: encode_phase_timing(&b.physical_planning),
        execution: encode_phase_timing(&b.execution),
        total: encode_phase_timing(&b.total),
    }
}

fn decode_benchmark_data(b: CompactBenchmarkData) -> SnapshotBenchmarkData {
    SnapshotBenchmarkData {
        iterations: b.iterations as u64,
        rows_per_iteration: b.rows_per_iteration,
        logical_planning: decode_phase_timing(b.logical),
        physical_planning: decode_phase_timing(b.physical),
        execution: decode_phase_timing(b.execution),
        total: decode_phase_timing(b.total),
    }
}

fn encode_describe_data(d: &SnapshotDescribeData) -> CompactDescribeData {
    CompactDescribeData {
        table_name: d.table_name.clone(),
        total_rows: d.total_rows,
        columns: d
            .columns
            .iter()
            .map(|c| CompactColumnStats {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
                count: c.count,
                null_count: c.null_count,
                distinct_count: c.distinct_count,
                min: c.min.clone(),
                max: c.max.clone(),
                mean: c.mean,
            })
            .collect(),
        elapsed_ms: d.elapsed_ms,
    }
}

fn decode_describe_data(d: CompactDescribeData) -> SnapshotDescribeData {
    SnapshotDescribeData {
        table_name: d.table_name,
        total_rows: d.total_rows,
        columns: d
            .columns
            .into_iter()
            .map(|c| SnapshotColumnStats {
                name: c.name,
                data_type: c.data_type,
                count: c.count,
                null_count: c.null_count,
                distinct_count: c.distinct_count,
                min: c.min,
                max: c.max,
                mean: c.mean,
            })
            .collect(),
        elapsed_ms: d.elapsed_ms,
    }
}

fn encode_sql_cell(cell: &SnapshotQueryCell) -> CompactSqlCell {
    CompactSqlCell {
        kind: match cell.kind {
            SnapshotCellKind::Query => CompactCellKind::Query,
            SnapshotCellKind::Info => CompactCellKind::Info,
            SnapshotCellKind::Diff => CompactCellKind::Diff,
            SnapshotCellKind::Explain => CompactCellKind::Explain,
            SnapshotCellKind::Benchmark => CompactCellKind::Benchmark,
            SnapshotCellKind::Describe => CompactCellKind::Describe,
        },
        sql: cell.sql.clone(),
        columns: encode_table_columns(&cell.columns),
        rows: cell.rows.clone(),
        total_rows: cell.total_rows,
        stats: cell.stats.as_ref().map(encode_query_stats),
        error: cell.error.clone(),
        plan: cell.plan.as_ref().map(encode_plan_node),
        diff: cell.diff.as_ref().map(encode_diff_data),
        benchmark: cell.benchmark.as_ref().map(encode_benchmark_data),
        describe: cell.describe.as_ref().map(encode_describe_data),
    }
}

fn decode_sql_cell(cell: CompactSqlCell) -> SnapshotQueryCell {
    SnapshotQueryCell {
        kind: match cell.kind {
            CompactCellKind::Query => SnapshotCellKind::Query,
            CompactCellKind::Info => SnapshotCellKind::Info,
            CompactCellKind::Diff => SnapshotCellKind::Diff,
            CompactCellKind::Explain => SnapshotCellKind::Explain,
            CompactCellKind::Benchmark => SnapshotCellKind::Benchmark,
            CompactCellKind::Describe => SnapshotCellKind::Describe,
        },
        sql: cell.sql,
        columns: decode_table_columns(cell.columns),
        rows: cell.rows,
        total_rows: cell.total_rows,
        stats: cell.stats.map(decode_query_stats),
        error: cell.error,
        plan: cell.plan.map(decode_plan_node),
        diff: cell.diff.map(decode_diff_data),
        benchmark: cell.benchmark.map(decode_benchmark_data),
        describe: cell.describe.map(decode_describe_data),
    }
}

fn encode_diff_data(d: &SnapshotDiffData) -> CompactDiffData {
    CompactDiffData {
        left_name: d.left_name.clone(),
        right_name: d.right_name.clone(),
        left_columns: encode_table_columns(&d.left_columns),
        left_rows: d.left_rows.clone(),
        left_total_rows: d.left_total_rows,
        left_error: d.left_error.clone(),
        right_columns: encode_table_columns(&d.right_columns),
        right_rows: d.right_rows.clone(),
        right_total_rows: d.right_total_rows,
        right_error: d.right_error.clone(),
        schemas_match: d.schemas_match,
        diff_stats: d.diff_stats.as_ref().map(|s| CompactDiffStats {
            left_only: s.left_only,
            right_only: s.right_only,
            different: s.different,
            matching: s.matching,
        }),
        left_plan: d.left_plan.as_ref().map(encode_plan_node),
        right_plan: d.right_plan.as_ref().map(encode_plan_node),
        diff_type: match d.diff_type {
            SnapshotDiffType::Data => CompactDiffType::Data,
            SnapshotDiffType::Plan => CompactDiffType::Plan,
            SnapshotDiffType::Schema => CompactDiffType::Schema,
            SnapshotDiffType::Profile => CompactDiffType::Profile,
        },
        schema_diff: d.schema_diff.as_ref().map(encode_schema_diff),
    }
}

fn decode_diff_data(d: CompactDiffData) -> SnapshotDiffData {
    SnapshotDiffData {
        left_name: d.left_name,
        right_name: d.right_name,
        left_columns: decode_table_columns(d.left_columns),
        left_rows: d.left_rows,
        left_total_rows: d.left_total_rows,
        left_error: d.left_error,
        right_columns: decode_table_columns(d.right_columns),
        right_rows: d.right_rows,
        right_total_rows: d.right_total_rows,
        right_error: d.right_error,
        schemas_match: d.schemas_match,
        diff_stats: d.diff_stats.map(|s| SnapshotDiffStats {
            left_only: s.left_only,
            right_only: s.right_only,
            different: s.different,
            matching: s.matching,
        }),
        left_plan: d.left_plan.map(decode_plan_node),
        right_plan: d.right_plan.map(decode_plan_node),
        diff_type: match d.diff_type {
            CompactDiffType::Data => SnapshotDiffType::Data,
            CompactDiffType::Plan => SnapshotDiffType::Plan,
            CompactDiffType::Schema => SnapshotDiffType::Schema,
            CompactDiffType::Profile => SnapshotDiffType::Profile,
        },
        schema_diff: d.schema_diff.map(decode_schema_diff),
    }
}

fn encode_schema_diff(s: &SnapshotSchemaDiff) -> CompactSchemaDiff {
    CompactSchemaDiff {
        table_name: s.table_name.clone(),
        columns: s
            .columns
            .iter()
            .map(|c| CompactSchemaDiffColumn {
                name: c.name.clone(),
                left_type: c.left_type.clone(),
                left_nullable: c.left_nullable,
                right_type: c.right_type.clone(),
                right_nullable: c.right_nullable,
                status: match c.status {
                    SnapshotColumnDiffStatus::Matching => CompactColumnDiffStatus::Matching,
                    SnapshotColumnDiffStatus::LeftOnly => CompactColumnDiffStatus::LeftOnly,
                    SnapshotColumnDiffStatus::RightOnly => CompactColumnDiffStatus::RightOnly,
                    SnapshotColumnDiffStatus::Changed => CompactColumnDiffStatus::Changed,
                },
            })
            .collect(),
        matching: s.matching,
        left_only: s.left_only,
        right_only: s.right_only,
        changed: s.changed,
    }
}

fn decode_schema_diff(s: CompactSchemaDiff) -> SnapshotSchemaDiff {
    SnapshotSchemaDiff {
        table_name: s.table_name,
        columns: s
            .columns
            .into_iter()
            .map(|c| SnapshotSchemaDiffColumn {
                name: c.name,
                left_type: c.left_type,
                left_nullable: c.left_nullable,
                right_type: c.right_type,
                right_nullable: c.right_nullable,
                status: match c.status {
                    CompactColumnDiffStatus::Matching => SnapshotColumnDiffStatus::Matching,
                    CompactColumnDiffStatus::LeftOnly => SnapshotColumnDiffStatus::LeftOnly,
                    CompactColumnDiffStatus::RightOnly => SnapshotColumnDiffStatus::RightOnly,
                    CompactColumnDiffStatus::Changed => SnapshotColumnDiffStatus::Changed,
                },
            })
            .collect(),
        matching: s.matching,
        left_only: s.left_only,
        right_only: s.right_only,
        changed: s.changed,
    }
}

// --- Shared helpers ---

fn encode_table_columns(cols: &[SnapshotTableColumn]) -> Vec<CompactTableColumn> {
    cols.iter()
        .map(|c| CompactTableColumn {
            name: c.name.clone(),
            data_type: c.data_type.clone(),
        })
        .collect()
}

fn decode_table_columns(cols: Vec<CompactTableColumn>) -> Vec<SnapshotTableColumn> {
    cols.into_iter()
        .map(|c| SnapshotTableColumn {
            name: c.name,
            data_type: c.data_type,
        })
        .collect()
}

fn encode_query_stats(s: &SnapshotQueryStats) -> CompactQueryStats {
    CompactQueryStats {
        total_time_ms: s.total_time_ms,
        planning_time_ms: s.planning_time_ms,
        execution_time_ms: s.execution_time_ms,
        rows_returned: s.rows_returned,
        bytes_scanned: s.bytes_scanned,
        partitions_scanned: s.partitions_scanned,
    }
}

fn decode_query_stats(s: CompactQueryStats) -> SnapshotQueryStats {
    SnapshotQueryStats {
        total_time_ms: s.total_time_ms,
        planning_time_ms: s.planning_time_ms,
        execution_time_ms: s.execution_time_ms,
        rows_returned: s.rows_returned,
        bytes_scanned: s.bytes_scanned,
        partitions_scanned: s.partitions_scanned,
    }
}

fn encode_plan_node(node: &SnapshotPlanNode) -> CompactPlanNode {
    CompactPlanNode {
        operator: node.operator.clone(),
        description: node.description.clone(),
        properties: node.properties.clone(),
        children: node.children.iter().map(encode_plan_node).collect(),
        metrics: node.metrics.as_ref().map(|m| CompactOperatorMetrics {
            output_rows: m.output_rows,
            elapsed_time_ms: m.elapsed_time_ms,
            memory_bytes: m.memory_bytes,
            spill_count: m.spill_count,
            spill_bytes: m.spill_bytes,
        }),
    }
}

fn decode_plan_node(node: CompactPlanNode) -> SnapshotPlanNode {
    SnapshotPlanNode {
        operator: node.operator,
        description: node.description,
        properties: node.properties,
        children: node.children.into_iter().map(decode_plan_node).collect(),
        metrics: node.metrics.map(|m| SnapshotOperatorMetrics {
            output_rows: m.output_rows,
            elapsed_time_ms: m.elapsed_time_ms,
            memory_bytes: m.memory_bytes,
            spill_count: m.spill_count,
            spill_bytes: m.spill_bytes,
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

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, None, None).unwrap();
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

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo), None).unwrap();
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

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo), None).unwrap();
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

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo), None).unwrap();
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

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo), None).unwrap();
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

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo), None).unwrap();
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

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, Some(&convo), None).unwrap();
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

    #[test]
    fn blob_snapshot_round_trip_preserves_layout() {
        use crate::workspace::{LayoutConfig, LayoutNode, LayoutType, PaneConfig};

        let mut ws = WorkspaceConfig::new("layout-blob");
        let mut p1 = PaneConfig::new("left_query");
        p1.name = "Left".to_string();
        ws.panes.push(p1);
        let mut p2 = PaneConfig::new("right_query");
        p2.name = "Right".to_string();
        ws.panes.push(p2);
        // Horizontal split layout
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
            shares: vec![],
        });

        let pane_data = vec![
            SnapshotPaneData::TimeSeries {
                series: vec![SnapshotSeries {
                    name: "s1".to_string(),
                    tags: vec![],
                    points: vec![(1.0, 2.0)],
                }],
            },
            SnapshotPaneData::TimeSeries {
                series: vec![SnapshotSeries {
                    name: "s2".to_string(),
                    tags: vec![],
                    points: vec![(3.0, 4.0)],
                }],
            },
        ];

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, None, None).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        // Layout must survive the blob round-trip
        let layout = decoded
            .workspace
            .layout
            .expect("layout should survive blob snapshot round-trip");
        assert_eq!(layout.layout_type, LayoutType::Horizontal);
        assert_eq!(layout.children.len(), 2);
        assert!(matches!(layout.children[0], LayoutNode::Pane(0)));
        assert!(matches!(layout.children[1], LayoutNode::Pane(1)));
    }

    #[test]
    fn blob_snapshot_round_trip_preserves_nested_layout() {
        use crate::workspace::{LayoutConfig, LayoutContainer, LayoutNode, LayoutType, PaneConfig};

        let mut ws = WorkspaceConfig::new("nested-blob");
        ws.panes.push(PaneConfig::new("a"));
        ws.panes.push(PaneConfig::new("b"));
        ws.panes.push(PaneConfig::new("c"));
        // Nested: Horizontal [ Vertical [0, 1], 2 ]
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![
                LayoutNode::Container(LayoutContainer {
                    layout_type: LayoutType::Vertical,
                    children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
                    shares: vec![],
                }),
                LayoutNode::Pane(2),
            ],
            shares: vec![],
        });

        let pane_data = vec![
            SnapshotPaneData::Stat {
                value: 1.0,
                sparkline: vec![],
            },
            SnapshotPaneData::Stat {
                value: 2.0,
                sparkline: vec![],
            },
            SnapshotPaneData::Stat {
                value: 3.0,
                sparkline: vec![],
            },
        ];

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, None, None).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        let layout = decoded
            .workspace
            .layout
            .expect("nested layout should survive blob snapshot round-trip");
        assert_eq!(layout.layout_type, LayoutType::Horizontal);
        assert_eq!(layout.children.len(), 2);

        match &layout.children[0] {
            LayoutNode::Container(c) => {
                assert_eq!(c.layout_type, LayoutType::Vertical);
                assert_eq!(c.children.len(), 2);
            }
            _ => panic!("Expected nested vertical container"),
        }
        assert!(matches!(layout.children[1], LayoutNode::Pane(2)));
    }

    #[test]
    fn snapshot_workspace_name_preserved() {
        let (ws, pane_data) = make_test_workspace();

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, None, None).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        // The workspace name survives the blob round-trip
        assert_eq!(decoded.workspace.workspace.name, "test-snapshot");
    }

    fn make_test_sql_pane() -> SnapshotSqlPane {
        SnapshotSqlPane {
            cells: vec![
                // Query cell
                SnapshotQueryCell {
                    kind: SnapshotCellKind::Query,
                    sql: "SELECT * FROM users".to_string(),
                    columns: vec![
                        SnapshotTableColumn {
                            name: "id".to_string(),
                            data_type: "Int64".to_string(),
                        },
                        SnapshotTableColumn {
                            name: "name".to_string(),
                            data_type: "Utf8".to_string(),
                        },
                    ],
                    rows: vec![
                        vec!["1".to_string(), "Alice".to_string()],
                        vec!["2".to_string(), "Bob".to_string()],
                    ],
                    total_rows: 2,
                    stats: Some(SnapshotQueryStats {
                        total_time_ms: 42,
                        planning_time_ms: 5,
                        execution_time_ms: 37,
                        rows_returned: 2,
                        bytes_scanned: 1024,
                        partitions_scanned: 1,
                    }),
                    error: None,
                    plan: None,
                    diff: None,
                    benchmark: None,
                    describe: None,
                },
                // Info cell
                SnapshotQueryCell {
                    kind: SnapshotCellKind::Info,
                    sql: "Connected to production".to_string(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    stats: None,
                    error: None,
                    plan: None,
                    diff: None,
                    benchmark: None,
                    describe: None,
                },
                // Diff cell
                SnapshotQueryCell {
                    kind: SnapshotCellKind::Diff,
                    sql: "SELECT count(*) FROM orders".to_string(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    stats: None,
                    error: None,
                    plan: None,
                    benchmark: None,
                    describe: None,
                    diff: Some(SnapshotDiffData {
                        left_name: "staging".to_string(),
                        right_name: "production".to_string(),
                        left_columns: vec![SnapshotTableColumn {
                            name: "count".to_string(),
                            data_type: "Int64".to_string(),
                        }],
                        left_rows: vec![vec!["100".to_string()]],
                        left_total_rows: 1,
                        left_error: None,
                        right_columns: vec![SnapshotTableColumn {
                            name: "count".to_string(),
                            data_type: "Int64".to_string(),
                        }],
                        right_rows: vec![vec!["105".to_string()]],
                        right_total_rows: 1,
                        right_error: None,
                        schemas_match: true,
                        diff_stats: Some(SnapshotDiffStats {
                            left_only: 0,
                            right_only: 0,
                            different: 1,
                            matching: 0,
                        }),
                        left_plan: None,
                        right_plan: None,
                        diff_type: SnapshotDiffType::Data,
                        schema_diff: None,
                    }),
                },
                // Explain cell
                SnapshotQueryCell {
                    kind: SnapshotCellKind::Explain,
                    sql: "EXPLAIN SELECT * FROM users".to_string(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    stats: None,
                    error: None,
                    plan: Some(SnapshotPlanNode {
                        operator: "TableScan".to_string(),
                        description: "users".to_string(),
                        properties: vec![("rows".to_string(), "1000".to_string())],
                        children: Vec::new(),
                        metrics: Some(SnapshotOperatorMetrics {
                            output_rows: 1000,
                            elapsed_time_ms: 5,
                            memory_bytes: 4096,
                            spill_count: 0,
                            spill_bytes: 0,
                        }),
                    }),
                    diff: None,
                    benchmark: None,
                    describe: None,
                },
                // Benchmark cell
                SnapshotQueryCell {
                    kind: SnapshotCellKind::Benchmark,
                    sql: "/bench 10 SELECT 1 + 1".to_string(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    stats: None,
                    error: None,
                    plan: None,
                    diff: None,
                    benchmark: Some(SnapshotBenchmarkData {
                        iterations: 10,
                        rows_per_iteration: 1,
                        logical_planning: SnapshotPhaseTiming {
                            min_us: 50,
                            max_us: 120,
                            mean_us: 75,
                            median_us: 70,
                            percent_of_total: 2.5,
                        },
                        physical_planning: SnapshotPhaseTiming {
                            min_us: 100,
                            max_us: 200,
                            mean_us: 140,
                            median_us: 130,
                            percent_of_total: 4.7,
                        },
                        execution: SnapshotPhaseTiming {
                            min_us: 2000,
                            max_us: 3500,
                            mean_us: 2800,
                            median_us: 2700,
                            percent_of_total: 92.8,
                        },
                        total: SnapshotPhaseTiming {
                            min_us: 2200,
                            max_us: 3800,
                            mean_us: 3015,
                            median_us: 2900,
                            percent_of_total: 100.0,
                        },
                    }),
                    describe: None,
                },
                // Describe cell
                SnapshotQueryCell {
                    kind: SnapshotCellKind::Describe,
                    sql: "/describe test_table".to_string(),
                    columns: Vec::new(),
                    rows: Vec::new(),
                    total_rows: 0,
                    stats: None,
                    error: None,
                    plan: None,
                    diff: None,
                    benchmark: None,
                    describe: Some(SnapshotDescribeData {
                        table_name: "test_table".to_string(),
                        total_rows: 1000,
                        columns: vec![
                            SnapshotColumnStats {
                                name: "id".to_string(),
                                data_type: "Int32".to_string(),
                                count: 1000,
                                null_count: 0,
                                distinct_count: 1000,
                                min: Some("1".to_string()),
                                max: Some("1000".to_string()),
                                mean: Some(500.5),
                            },
                            SnapshotColumnStats {
                                name: "name".to_string(),
                                data_type: "Utf8".to_string(),
                                count: 990,
                                null_count: 10,
                                distinct_count: 850,
                                min: Some("Aaron".to_string()),
                                max: Some("Zoe".to_string()),
                                mean: None,
                            },
                        ],
                        elapsed_ms: 42,
                    }),
                },
            ],
        }
    }

    #[test]
    fn round_trip_sql_pane_with_cell_kinds() {
        let (ws, pane_data) = make_test_workspace();
        let sql_pane = make_test_sql_pane();

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, None, Some(&sql_pane)).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        let sql = decoded
            .workspace
            .snapshot
            .as_ref()
            .unwrap()
            .sql_pane
            .as_ref()
            .unwrap();
        assert_eq!(sql.cells.len(), 6);

        // Query cell
        assert_eq!(sql.cells[0].kind, SnapshotCellKind::Query);
        assert_eq!(sql.cells[0].sql, "SELECT * FROM users");
        assert_eq!(sql.cells[0].columns.len(), 2);
        assert_eq!(sql.cells[0].rows.len(), 2);
        assert_eq!(sql.cells[0].total_rows, 2);
        assert!(sql.cells[0].stats.is_some());
        assert!(sql.cells[0].diff.is_none());
        assert!(sql.cells[0].benchmark.is_none());

        // Info cell
        assert_eq!(sql.cells[1].kind, SnapshotCellKind::Info);
        assert_eq!(sql.cells[1].sql, "Connected to production");
        assert!(sql.cells[1].columns.is_empty());
        assert!(sql.cells[1].diff.is_none());

        // Diff cell
        assert_eq!(sql.cells[2].kind, SnapshotCellKind::Diff);
        let diff = sql.cells[2].diff.as_ref().unwrap();
        assert_eq!(diff.left_name, "staging");
        assert_eq!(diff.right_name, "production");
        assert_eq!(diff.left_rows, vec![vec!["100".to_string()]]);
        assert_eq!(diff.right_rows, vec![vec!["105".to_string()]]);
        assert!(diff.schemas_match);
        assert_eq!(diff.diff_type, SnapshotDiffType::Data);
        let stats = diff.diff_stats.as_ref().unwrap();
        assert_eq!(stats.different, 1);
        assert_eq!(stats.matching, 0);

        // Explain cell
        assert_eq!(sql.cells[3].kind, SnapshotCellKind::Explain);
        let plan = sql.cells[3].plan.as_ref().unwrap();
        assert_eq!(plan.operator, "TableScan");
        assert!(plan.metrics.is_some());

        // Benchmark cell
        assert_eq!(sql.cells[4].kind, SnapshotCellKind::Benchmark);
        assert_eq!(sql.cells[4].sql, "/bench 10 SELECT 1 + 1");
        let bench = sql.cells[4].benchmark.as_ref().unwrap();
        assert_eq!(bench.iterations, 10);
        assert_eq!(bench.rows_per_iteration, 1);
        assert_eq!(bench.logical_planning.min_us, 50);
        assert_eq!(bench.execution.median_us, 2700);
        assert!((bench.execution.percent_of_total - 92.8).abs() < 0.01);

        // Describe cell
        assert_eq!(sql.cells[5].kind, SnapshotCellKind::Describe);
        assert_eq!(sql.cells[5].sql, "/describe test_table");
        let desc = sql.cells[5].describe.as_ref().unwrap();
        assert_eq!(desc.table_name, "test_table");
        assert_eq!(desc.total_rows, 1000);
        assert_eq!(desc.columns.len(), 2);
        assert_eq!(desc.columns[0].name, "id");
        assert_eq!(desc.columns[0].count, 1000);
        assert_eq!(desc.columns[0].null_count, 0);
        assert_eq!(desc.columns[0].mean, Some(500.5));
        assert_eq!(desc.columns[1].name, "name");
        assert_eq!(desc.columns[1].null_count, 10);
        assert!(desc.columns[1].mean.is_none());
        assert_eq!(desc.elapsed_ms, 42);
    }

    #[test]
    fn round_trip_diff_cell_with_schema_diff() {
        let (ws, pane_data) = make_test_workspace();
        let sql_pane = SnapshotSqlPane {
            cells: vec![SnapshotQueryCell {
                kind: SnapshotCellKind::Diff,
                sql: "DESCRIBE users".to_string(),
                columns: Vec::new(),
                rows: Vec::new(),
                total_rows: 0,
                stats: None,
                error: None,
                plan: None,
                diff: Some(SnapshotDiffData {
                    left_name: "dev".to_string(),
                    right_name: "prod".to_string(),
                    left_columns: Vec::new(),
                    left_rows: Vec::new(),
                    left_total_rows: 0,
                    left_error: None,
                    right_columns: Vec::new(),
                    right_rows: Vec::new(),
                    right_total_rows: 0,
                    right_error: None,
                    schemas_match: false,
                    diff_stats: None,
                    left_plan: None,
                    right_plan: None,
                    diff_type: SnapshotDiffType::Schema,
                    schema_diff: Some(SnapshotSchemaDiff {
                        table_name: "users".to_string(),
                        columns: vec![
                            SnapshotSchemaDiffColumn {
                                name: "id".to_string(),
                                left_type: Some("Int64".to_string()),
                                left_nullable: Some(false),
                                right_type: Some("Int64".to_string()),
                                right_nullable: Some(false),
                                status: SnapshotColumnDiffStatus::Matching,
                            },
                            SnapshotSchemaDiffColumn {
                                name: "email".to_string(),
                                left_type: None,
                                left_nullable: None,
                                right_type: Some("Utf8".to_string()),
                                right_nullable: Some(true),
                                status: SnapshotColumnDiffStatus::RightOnly,
                            },
                        ],
                        matching: 1,
                        left_only: 0,
                        right_only: 1,
                        changed: 0,
                    }),
                }),
                benchmark: None,
                describe: None,
            }],
        };

        let bytes = encode_snapshot(&ws, &pane_data, 1700000000, None, Some(&sql_pane)).unwrap();
        let decoded = decode_snapshot(&bytes).unwrap();

        let sql = decoded
            .workspace
            .snapshot
            .as_ref()
            .unwrap()
            .sql_pane
            .as_ref()
            .unwrap();
        let diff = sql.cells[0].diff.as_ref().unwrap();
        assert_eq!(diff.diff_type, SnapshotDiffType::Schema);
        let sd = diff.schema_diff.as_ref().unwrap();
        assert_eq!(sd.table_name, "users");
        assert_eq!(sd.columns.len(), 2);
        assert_eq!(sd.columns[0].status, SnapshotColumnDiffStatus::Matching);
        assert_eq!(sd.columns[1].status, SnapshotColumnDiffStatus::RightOnly);
        assert_eq!(sd.matching, 1);
        assert_eq!(sd.right_only, 1);
    }
}
