//! Shared diff types and parsing — used by both DiffViewerOverlay and PrReviewPane.

use similar::{ChangeTag, TextDiff};

/// A single file's diff content.
#[derive(Debug, Clone, Default)]
pub struct FileDiff {
    /// The file path (relative to repo root).
    pub path: String,
    /// Lines of the diff for this file.
    pub lines: Vec<DiffLine>,
    /// Number of additions.
    pub additions: usize,
    /// Number of deletions.
    pub deletions: usize,
    /// Syntax highlight data for the old (deleted) side of the file.
    pub old_highlight: Option<crate::components::util::syntax_highlight::SyntaxHighlightData>,
    /// Syntax highlight data for the new (added) side of the file.
    pub new_highlight: Option<crate::components::util::syntax_highlight::SyntaxHighlightData>,
    /// Full old file content lines (for context expansion, loaded lazily).
    pub old_file_lines: Option<Vec<String>>,
    /// Full new file content lines (for context expansion, loaded lazily).
    pub new_file_lines: Option<Vec<String>>,
}

impl FileDiff {
    /// Compute syntax highlighting for old and new sides by reconstructing file content
    /// from the diff lines. Also assigns `old_recon_num` / `new_recon_num` for each line.
    pub fn compute_syntax_highlights(&mut self) {
        let lang = language_from_path(&self.path);

        // Reconstruct old and new file content, tracking reconstruction line numbers
        let mut old_lines: Vec<&str> = Vec::new();
        let mut new_lines: Vec<&str> = Vec::new();
        let mut old_counter: usize = 0;
        let mut new_counter: usize = 0;

        for line in &mut self.lines {
            match line.kind {
                DiffLineKind::Context => {
                    old_counter += 1;
                    new_counter += 1;
                    line.old_recon_num = Some(old_counter);
                    line.new_recon_num = Some(new_counter);
                    old_lines.push(&line.content);
                    new_lines.push(&line.content);
                }
                DiffLineKind::Deletion => {
                    old_counter += 1;
                    line.old_recon_num = Some(old_counter);
                    old_lines.push(&line.content);
                }
                DiffLineKind::Addition => {
                    new_counter += 1;
                    line.new_recon_num = Some(new_counter);
                    new_lines.push(&line.content);
                }
                DiffLineKind::HunkHeader | DiffLineKind::FileHeader => {}
            }
        }

        let old_content = old_lines.join("\n");
        let new_content = new_lines.join("\n");

        if !old_content.is_empty() {
            self.old_highlight = Some(
                crate::components::util::syntax_highlight::SyntaxHighlightData::new(
                    &old_content,
                    lang,
                ),
            );
        }
        if !new_content.is_empty() {
            self.new_highlight = Some(
                crate::components::util::syntax_highlight::SyntaxHighlightData::new(
                    &new_content,
                    lang,
                ),
            );
        }
    }
}

/// Map file path extension to a tree-sitter language name.
pub fn language_from_path(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "rs" => "rust",
        "go" => "go",
        "py" => "python",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "toml" => "toml",
        _ => "rust", // fallback — tree-sitter will gracefully handle mismatched grammar
    }
}

/// A single line in a diff with word-level change information.
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// The line content (without the +/- prefix).
    pub content: String,
    /// The line type.
    pub kind: DiffLineKind,
    /// Old line number (for context and deletions).
    pub old_line_num: Option<usize>,
    /// New line number (for context and additions).
    pub new_line_num: Option<usize>,
    /// Word-level changes within this line (start, end byte indices).
    pub word_highlights: Vec<(usize, usize)>,
    /// Line number in the reconstructed old file (1-indexed, for syntax highlighting).
    pub old_recon_num: Option<usize>,
    /// Line number in the reconstructed new file (1-indexed, for syntax highlighting).
    pub new_recon_num: Option<usize>,
    /// Number of hidden lines (for hunk headers only).
    pub hidden_lines: Option<usize>,
    /// Function/method context from hunk header (e.g. "fn foo()").
    pub hunk_context: Option<String>,
    /// Old-side start line from hunk header (for context expansion).
    pub hunk_old_start: Option<usize>,
    /// New-side start line from hunk header (for context expansion).
    pub hunk_new_start: Option<usize>,
}

/// The type of diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// Context line (unchanged).
    Context,
    /// Added line.
    Addition,
    /// Removed line.
    Deletion,
    /// Hunk header (@@ ... @@).
    HunkHeader,
    /// File header (diff --git, ---, +++).
    FileHeader,
}

/// Parses a unified diff into per-file sections with word-level highlighting.
pub fn parse_diff_into_files(diff: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileDiff> = None;
    let mut old_line_num: usize = 0;
    let mut new_line_num: usize = 0;
    let mut pending_deletions: Vec<(String, usize)> = Vec::new();
    let mut pending_additions: Vec<(String, usize)> = Vec::new();

    for raw_line in diff.lines() {
        if raw_line.starts_with("diff --git") {
            if let Some(ref mut file) = current_file {
                compute_word_highlights(file, &pending_deletions, &pending_additions);
                pending_deletions.clear();
                pending_additions.clear();
            }
            if let Some(file) = current_file.take() {
                files.push(file);
            }
            let path = raw_line
                .strip_prefix("diff --git a/")
                .and_then(|s| s.split(" b/").next())
                .unwrap_or("")
                .to_string();
            current_file = Some(FileDiff {
                path,
                old_highlight: None,
                new_highlight: None,
                old_file_lines: None,
                new_file_lines: None,
                lines: vec![DiffLine {
                    content: raw_line.to_string(),
                    kind: DiffLineKind::FileHeader,
                    old_line_num: None,
                    new_line_num: None,
                    word_highlights: Vec::new(),
                    old_recon_num: None,
                    new_recon_num: None,
                    hidden_lines: None,
                    hunk_context: None,
                    hunk_old_start: None,
                    hunk_new_start: None,
                }],
                additions: 0,
                deletions: 0,
            });
            continue;
        }

        if let Some(ref mut file) = current_file {
            if raw_line.starts_with("@@") {
                compute_word_highlights(file, &pending_deletions, &pending_additions);
                pending_deletions.clear();
                pending_additions.clear();
                if let Some((old_start, new_start)) = parse_hunk_header(raw_line) {
                    old_line_num = old_start;
                    new_line_num = new_start;
                }
                // Extract function context from hunk header (text after @@ ... @@)
                let hunk_context = raw_line
                    .find("@@ ")
                    .and_then(|start| {
                        let after_first = &raw_line[start + 3..];
                        after_first.find("@@").map(|end| {
                            let ctx = after_first[end + 2..].trim();
                            if ctx.is_empty() {
                                None
                            } else {
                                Some(ctx.to_string())
                            }
                        })
                    })
                    .flatten();

                let (hunk_old_start, hunk_new_start) = parse_hunk_header(raw_line).unzip();
                file.lines.push(DiffLine {
                    content: raw_line.to_string(),
                    kind: DiffLineKind::HunkHeader,
                    old_line_num: None,
                    new_line_num: None,
                    word_highlights: Vec::new(),
                    old_recon_num: None,
                    new_recon_num: None,
                    hidden_lines: None,
                    hunk_context,
                    hunk_old_start,
                    hunk_new_start,
                });
                continue;
            }

            let kind = classify_diff_line(raw_line);
            let content = match kind {
                DiffLineKind::Addition | DiffLineKind::Deletion => {
                    raw_line.get(1..).unwrap_or("").to_string()
                }
                DiffLineKind::Context => raw_line.get(1..).unwrap_or(raw_line).to_string(),
                _ => raw_line.to_string(),
            };

            let (old_num, new_num) = match kind {
                DiffLineKind::Addition => {
                    let n = new_line_num;
                    new_line_num += 1;
                    file.additions += 1;
                    (None, Some(n))
                }
                DiffLineKind::Deletion => {
                    let n = old_line_num;
                    old_line_num += 1;
                    file.deletions += 1;
                    (Some(n), None)
                }
                DiffLineKind::Context => {
                    let old = old_line_num;
                    let new = new_line_num;
                    old_line_num += 1;
                    new_line_num += 1;
                    (Some(old), Some(new))
                }
                _ => (None, None),
            };

            let line_index = file.lines.len();
            file.lines.push(DiffLine {
                content: content.clone(),
                kind,
                old_line_num: old_num,
                new_line_num: new_num,
                word_highlights: Vec::new(),
                old_recon_num: None,
                new_recon_num: None,
                hidden_lines: None,
                hunk_context: None,
                hunk_old_start: None,
                hunk_new_start: None,
            });

            match kind {
                DiffLineKind::Deletion => {
                    if !pending_additions.is_empty() && pending_deletions.is_empty() {
                        pending_additions.clear();
                    }
                    pending_deletions.push((content, line_index));
                }
                DiffLineKind::Addition => {
                    pending_additions.push((content, line_index));
                }
                DiffLineKind::Context | DiffLineKind::HunkHeader | DiffLineKind::FileHeader => {
                    compute_word_highlights(file, &pending_deletions, &pending_additions);
                    pending_deletions.clear();
                    pending_additions.clear();
                }
            }
        }
    }

    if let Some(mut file) = current_file {
        compute_word_highlights(&mut file, &pending_deletions, &pending_additions);
        files.push(file);
    }

    // Compute hidden line counts for hunk headers and syntax highlighting
    for file in &mut files {
        compute_hidden_lines(file);
        file.compute_syntax_highlights();
    }

    files
}

/// Computes word-level highlights for paired addition/deletion lines.
fn compute_word_highlights(
    file: &mut FileDiff,
    deletions: &[(String, usize)],
    additions: &[(String, usize)],
) {
    let pairs = deletions.len().min(additions.len());
    for i in 0..pairs {
        let (del_content, del_idx) = &deletions[i];
        let (add_content, add_idx) = &additions[i];
        let diff = TextDiff::from_words(del_content, add_content);

        let mut del_highlights: Vec<(usize, usize)> = Vec::new();
        let mut del_pos = 0;
        for change in diff.iter_all_changes() {
            let len = change.value().len();
            match change.tag() {
                ChangeTag::Delete => {
                    del_highlights.push((del_pos, del_pos + len));
                    del_pos += len;
                }
                ChangeTag::Equal => del_pos += len,
                ChangeTag::Insert => {}
            }
        }

        let mut add_highlights: Vec<(usize, usize)> = Vec::new();
        let mut add_pos = 0;
        for change in diff.iter_all_changes() {
            let len = change.value().len();
            match change.tag() {
                ChangeTag::Insert => {
                    add_highlights.push((add_pos, add_pos + len));
                    add_pos += len;
                }
                ChangeTag::Equal => add_pos += len,
                ChangeTag::Delete => {}
            }
        }

        let del_highlights = merge_adjacent_highlights(del_highlights);
        let add_highlights = merge_adjacent_highlights(add_highlights);

        if let Some(line) = file.lines.get_mut(*del_idx) {
            line.word_highlights = del_highlights;
        }
        if let Some(line) = file.lines.get_mut(*add_idx) {
            line.word_highlights = add_highlights;
        }
    }
}

/// Merges adjacent or overlapping highlight ranges.
fn merge_adjacent_highlights(mut highlights: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if highlights.is_empty() {
        return highlights;
    }
    highlights.sort_by_key(|&(start, _)| start);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    let mut current = highlights[0];
    for &(start, end) in &highlights[1..] {
        if start <= current.1 {
            current.1 = current.1.max(end);
        } else {
            merged.push(current);
            current = (start, end);
        }
    }
    merged.push(current);
    merged
}

/// Parses a hunk header to extract starting line numbers.
pub fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let content = line.strip_prefix("@@")?.trim_start();
    let content = content.split("@@").next()?.trim();
    let mut parts = content.split_whitespace();
    let old_part = parts.next()?.strip_prefix('-')?;
    let old_start: usize = old_part.split(',').next()?.parse().ok()?;
    let new_part = parts.next()?.strip_prefix('+')?;
    let new_start: usize = new_part.split(',').next()?.parse().ok()?;
    Some((old_start, new_start))
}

/// Classifies a diff line by its type.
fn classify_diff_line(line: &str) -> DiffLineKind {
    if line.starts_with("@@") {
        DiffLineKind::HunkHeader
    } else if line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("new file mode")
        || line.starts_with("deleted file mode")
    {
        DiffLineKind::FileHeader
    } else if line.starts_with('+') {
        DiffLineKind::Addition
    } else if line.starts_with('-') {
        DiffLineKind::Deletion
    } else {
        DiffLineKind::Context
    }
}

/// Compute the number of hidden lines for each hunk header.
///
/// The hidden count is the gap between the end of the previous hunk
/// and the start of this hunk (lines not shown in the diff).
fn compute_hidden_lines(file: &mut FileDiff) {
    let mut prev_old_end: Option<usize> = None;

    for line in &mut file.lines {
        if line.kind == DiffLineKind::HunkHeader {
            if let Some((old_start, _)) = parse_hunk_header(&line.content) {
                if let Some(prev_end) = prev_old_end {
                    let hidden = old_start.saturating_sub(prev_end + 1);
                    if hidden > 0 {
                        line.hidden_lines = Some(hidden);
                    }
                }
            }
        }
        // Track the last old line number we've seen
        if let Some(n) = line.old_line_num {
            prev_old_end = Some(n);
        }
    }
}

/// Builds paired lines for split (side-by-side) view.
pub fn build_split_view_lines(lines: &[DiffLine]) -> Vec<(Option<DiffLine>, Option<DiffLine>)> {
    let mut result: Vec<(Option<DiffLine>, Option<DiffLine>)> = Vec::new();
    let mut pending_deletions: Vec<DiffLine> = Vec::new();
    let mut pending_additions: Vec<DiffLine> = Vec::new();

    for line in lines {
        match line.kind {
            DiffLineKind::Context => {
                flush_pending_changes(&mut result, &mut pending_deletions, &mut pending_additions);
                result.push((Some(line.clone()), Some(line.clone())));
            }
            DiffLineKind::Deletion => {
                pending_deletions.push(line.clone());
            }
            DiffLineKind::Addition => {
                pending_additions.push(line.clone());
            }
            DiffLineKind::HunkHeader | DiffLineKind::FileHeader => {
                flush_pending_changes(&mut result, &mut pending_deletions, &mut pending_additions);
                result.push((Some(line.clone()), Some(line.clone())));
            }
        }
    }
    flush_pending_changes(&mut result, &mut pending_deletions, &mut pending_additions);
    result
}

/// Flushes pending deletions and additions into paired rows.
fn flush_pending_changes(
    result: &mut Vec<(Option<DiffLine>, Option<DiffLine>)>,
    deletions: &mut Vec<DiffLine>,
    additions: &mut Vec<DiffLine>,
) {
    let pairs = deletions.len().min(additions.len());
    for i in 0..pairs {
        result.push((Some(deletions[i].clone()), Some(additions[i].clone())));
    }
    for deletion in deletions.iter().skip(pairs) {
        result.push((Some(deletion.clone()), None));
    }
    for addition in additions.iter().skip(pairs) {
        result.push((None, Some(addition.clone())));
    }
    deletions.clear();
    additions.clear();
}

/// Builds paired lines for split view using references (zero-copy).
pub fn build_split_view_lines_ref(
    lines: &[DiffLine],
) -> Vec<(Option<&DiffLine>, Option<&DiffLine>)> {
    let mut result: Vec<(Option<&DiffLine>, Option<&DiffLine>)> = Vec::new();
    let mut pending_deletions: Vec<&DiffLine> = Vec::new();
    let mut pending_additions: Vec<&DiffLine> = Vec::new();

    for line in lines {
        match line.kind {
            DiffLineKind::Context => {
                flush_pending_refs(&mut result, &mut pending_deletions, &mut pending_additions);
                result.push((Some(line), Some(line)));
            }
            DiffLineKind::Deletion => {
                pending_deletions.push(line);
            }
            DiffLineKind::Addition => {
                pending_additions.push(line);
            }
            DiffLineKind::HunkHeader | DiffLineKind::FileHeader => {
                flush_pending_refs(&mut result, &mut pending_deletions, &mut pending_additions);
                result.push((Some(line), Some(line)));
            }
        }
    }
    flush_pending_refs(&mut result, &mut pending_deletions, &mut pending_additions);
    result
}

/// Flushes pending deletions and additions into paired rows (reference version).
fn flush_pending_refs<'a>(
    result: &mut Vec<(Option<&'a DiffLine>, Option<&'a DiffLine>)>,
    deletions: &mut Vec<&'a DiffLine>,
    additions: &mut Vec<&'a DiffLine>,
) {
    let pairs = deletions.len().min(additions.len());
    for i in 0..pairs {
        result.push((Some(deletions[i]), Some(additions[i])));
    }
    for deletion in deletions.iter().skip(pairs) {
        result.push((Some(deletion), None));
    }
    for addition in additions.iter().skip(pairs) {
        result.push((None, Some(addition)));
    }
    deletions.clear();
    additions.clear();
}

/// Parse the old-side line count from a hunk header (e.g. `@@ -10,5 +20,7 @@` → `5`).
pub fn parse_hunk_old_count(line: &str) -> Option<usize> {
    let content = line.strip_prefix("@@")?.trim_start();
    let content = content.split("@@").next()?.trim();
    let old_part = content.split_whitespace().next()?.strip_prefix('-')?;
    old_part.split(',').nth(1)?.parse().ok()
}

/// Load a file at a specific git commit using `git show`.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_file_at_commit(
    repo_root: &std::path::Path,
    commit: &str,
    file_path: &str,
) -> Option<Vec<String>> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("{commit}:{file_path}")])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if output.status.success() {
        Some(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(String::from)
                .collect(),
        )
    } else {
        None
    }
}

/// Get the word-level diff background color for a line kind.
pub fn diff_word_bg(
    kind: DiffLineKind,
    theme: crate::ui::theme::AppTheme,
) -> Option<egui::Color32> {
    match kind {
        DiffLineKind::Addition => Some(theme.diff_added_word_bg()),
        DiffLineKind::Deletion => Some(theme.diff_removed_word_bg()),
        _ => None,
    }
}
