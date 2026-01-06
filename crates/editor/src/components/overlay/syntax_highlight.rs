//! Syntax highlighting utilities using tree-sitter.
//!
//! This module provides syntax highlighting for source code previews,
//! used by the unified finder and source preview overlays.

use std::ops::Range;
use std::path::PathBuf;

use egui::Color32;
use egui::text::LayoutJob;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::ui::colors::text_color;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Highlight names recognized by our syntax highlighter.
/// These map to tree-sitter highlight capture names.
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "escape",
    "function",
    "function.builtin",
    "function.macro",
    "keyword",
    "label",
    "number",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
];

/// A cached highlight span: byte range and the highlight type index.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    /// Byte range in the source content.
    pub range: Range<usize>,
    /// Index into `HIGHLIGHT_NAMES`.
    pub highlight_idx: usize,
}

/// Cache for syntax-highlighted source files.
#[derive(Debug, Clone)]
pub struct HighlightCache {
    /// The file path this cache is for.
    pub file_path: PathBuf,
    /// Cached source content (avoids re-reading from disk).
    pub source_content: String,
    /// Cached highlight spans.
    pub spans: Vec<HighlightSpan>,
    /// Cached line offsets for byte position mapping.
    pub line_offsets: Vec<usize>,
}

impl HighlightCache {
    /// Creates a new highlight cache for the given file.
    ///
    /// Returns `None` if the file cannot be read or is empty.
    #[must_use]
    pub fn new(file_path: PathBuf) -> Option<Self> {
        let source_content = std::fs::read_to_string(&file_path).ok()?;

        if source_content.is_empty() {
            return None;
        }

        // Compute line offsets
        let mut line_offsets: Vec<usize> = vec![0];
        for (i, ch) in source_content.char_indices() {
            if ch == '\n' {
                line_offsets.push(i + 1);
            }
        }

        // Compute syntax highlights
        let spans = compute_syntax_highlights(&source_content);

        Some(Self {
            file_path,
            source_content,
            spans,
            line_offsets,
        })
    }
}

/// Compute syntax highlights for source content using tree-sitter.
#[must_use]
pub fn compute_syntax_highlights(source_content: &str) -> Vec<HighlightSpan> {
    let mut highlight_spans = Vec::new();

    if source_content.is_empty() {
        return highlight_spans;
    }

    // Create highlight configuration
    let mut config = match HighlightConfiguration::new(
        tree_sitter_rust::LANGUAGE.into(),
        "rust",
        tree_sitter_rust::HIGHLIGHTS_QUERY,
        "", // injections query
        "", // locals query
    ) {
        Ok(config) => config,
        Err(e) => {
            log::warn!("Failed to create highlight config: {e}");
            return highlight_spans;
        }
    };

    // Configure recognized highlight names
    config.configure(HIGHLIGHT_NAMES);

    // Create highlighter and process the source
    let mut highlighter = Highlighter::new();
    let source_bytes = source_content.as_bytes();

    let highlights = match highlighter.highlight(&config, source_bytes, None, |_| None) {
        Ok(h) => h,
        Err(e) => {
            log::warn!("Failed to highlight: {e}");
            return highlight_spans;
        }
    };

    // Track highlight stack for nested highlights
    let mut highlight_stack: Vec<usize> = Vec::new();

    for event in highlights {
        let event = match event {
            Ok(e) => e,
            Err(e) => {
                log::warn!("Highlight event error: {e}");
                continue;
            }
        };

        match event {
            HighlightEvent::Source { start, end } => {
                // If we have an active highlight, record this span
                if let Some(&highlight_idx) = highlight_stack.last() {
                    highlight_spans.push(HighlightSpan {
                        range: start..end,
                        highlight_idx,
                    });
                }
            }
            HighlightEvent::HighlightStart(highlight) => {
                highlight_stack.push(highlight.0);
            }
            HighlightEvent::HighlightEnd => {
                highlight_stack.pop();
            }
        }
    }

    highlight_spans
}

/// Highlight a line of code using cached tree-sitter spans.
///
/// `line_num` is 1-indexed.
#[must_use]
pub fn highlight_line_with_spans(
    line_num: usize,
    line: &str,
    highlight_spans: &[HighlightSpan],
    line_offsets: &[usize],
    source_len: usize,
    theme: AppTheme,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font_id = typography::monospace(typography::SM);
    let default_color = text_color(theme);

    // If we have no highlight spans or line offsets, fall back to plain text
    if highlight_spans.is_empty() || line_offsets.is_empty() {
        job.append(line, 0.0, egui::TextFormat::simple(font_id, default_color));
        return job;
    }

    // Get the byte range for this line (0-indexed)
    let line_idx = line_num.saturating_sub(1);
    let line_start = line_offsets.get(line_idx).copied().unwrap_or(0);
    let line_end = line_offsets
        .get(line_idx + 1)
        .copied()
        .unwrap_or(source_len);

    // Find spans that overlap with this line
    let mut current_pos = 0usize; // Position within the line string

    // Collect spans for this line, adjusting to line-relative positions
    let mut line_spans: Vec<(usize, usize, Color32)> = Vec::new();

    for span in highlight_spans {
        // Check if span overlaps with this line
        if span.range.end <= line_start || span.range.start >= line_end {
            continue;
        }

        // Clamp span to line boundaries and convert to line-relative positions
        let span_start_in_line = span.range.start.saturating_sub(line_start);
        let span_end_in_line = span.range.end.saturating_sub(line_start).min(line.len());

        if span_start_in_line >= span_end_in_line {
            continue;
        }

        let color = highlight_color(span.highlight_idx, theme);
        line_spans.push((span_start_in_line, span_end_in_line, color));
    }

    // Sort spans by start position
    line_spans.sort_by_key(|(start, _, _)| *start);

    // Build the LayoutJob by iterating through spans
    for (span_start, span_end, color) in line_spans {
        // Add any unhighlighted text before this span
        if span_start > current_pos {
            if let Some(text) = line.get(current_pos..span_start) {
                job.append(
                    text,
                    0.0,
                    egui::TextFormat::simple(font_id.clone(), default_color),
                );
            }
        }

        // Add the highlighted span
        if let Some(text) = line.get(span_start..span_end) {
            job.append(text, 0.0, egui::TextFormat::simple(font_id.clone(), color));
        }

        current_pos = span_end;
    }

    // Add any remaining unhighlighted text
    if current_pos < line.len() {
        if let Some(text) = line.get(current_pos..) {
            job.append(
                text,
                0.0,
                egui::TextFormat::simple(font_id.clone(), default_color),
            );
        }
    }

    // Handle empty line
    if job.is_empty() && line.is_empty() {
        job.append("", 0.0, egui::TextFormat::simple(font_id, default_color));
    }

    job
}

/// Get the color for a highlight index based on theme.
#[must_use]
pub fn highlight_color(idx: usize, theme: AppTheme) -> Color32 {
    let name = HIGHLIGHT_NAMES.get(idx).copied().unwrap_or("");

    match name {
        "keyword" => theme.syntax_keyword(),
        "string" | "escape" => theme.syntax_value(),
        "comment" => theme.syntax_comment(),
        "function" | "function.builtin" | "function.macro" => theme.syntax_function(),
        "type" | "type.builtin" | "constructor" => theme.syntax_type(),
        "number" | "constant" | "constant.builtin" => theme.syntax_number(),
        "attribute" => theme.syntax_type(), // Use type color for attributes
        "variable" | "variable.parameter" | "property" | "label" => theme.syntax_variable(),
        "variable.builtin" => theme.syntax_keyword(), // Use keyword color for builtin vars (self)
        "operator" | "punctuation" | "punctuation.bracket" | "punctuation.delimiter" => {
            theme.syntax_punctuation()
        }
        _ => theme.syntax_variable(), // Default to variable color
    }
}
