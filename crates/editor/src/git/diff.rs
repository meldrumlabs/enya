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
                lines: vec![DiffLine {
                    content: raw_line.to_string(),
                    kind: DiffLineKind::FileHeader,
                    old_line_num: None,
                    new_line_num: None,
                    word_highlights: Vec::new(),
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
                file.lines.push(DiffLine {
                    content: raw_line.to_string(),
                    kind: DiffLineKind::HunkHeader,
                    old_line_num: None,
                    new_line_num: None,
                    word_highlights: Vec::new(),
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
fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
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
