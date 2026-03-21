//! Syntax highlighting utilities using tree-sitter.
//!
//! Provides shared syntax highlighting functionality for source code display,
//! used by both `SourcePreviewOverlay` and inline source previews in agent panes.
//!
//! Requires the "codebase" feature on native builds.

#[cfg(not(target_arch = "wasm32"))]
use std::ops::Range;

use egui::{Color32, text::LayoutJob};

use crate::ui::colors::text_color;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

#[cfg(not(target_arch = "wasm32"))]
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Highlight names recognized by our syntax highlighter.
/// These map to tree-sitter highlight capture names.
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub range: Range<usize>,
    pub highlight_idx: usize,
}

/// Precomputed syntax highlighting data for source content.
///
/// This struct caches the tree-sitter analysis results so that
/// individual lines can be efficiently highlighted during rendering.
#[derive(Debug, Clone, Default)]
pub struct SyntaxHighlightData {
    /// The raw source content that was analyzed.
    #[cfg(not(target_arch = "wasm32"))]
    pub source_content: String,
    /// Byte offsets for the start of each line.
    #[cfg(not(target_arch = "wasm32"))]
    pub line_offsets: Vec<usize>,
    /// Cached highlight spans from tree-sitter.
    #[cfg(not(target_arch = "wasm32"))]
    pub spans: Vec<HighlightSpan>,
}

impl SyntaxHighlightData {
    /// Create new syntax highlight data from source content and language.
    ///
    /// Computes tree-sitter highlights for the content. On WASM, this returns
    /// an empty struct since tree-sitter is not available.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(content: &str, language: &str) -> Self {
        let mut data = Self {
            source_content: content.to_string(),
            line_offsets: Vec::new(),
            spans: Vec::new(),
        };

        // Compute line offsets
        data.line_offsets.push(0);
        for (i, ch) in content.char_indices() {
            if ch == '\n' {
                data.line_offsets.push(i + 1);
            }
        }

        // Compute syntax highlights
        data.compute_highlights(language);
        data
    }

    /// Stub when tree-sitter is not available (WASM or codebase feature disabled).
    #[cfg(target_arch = "wasm32")]
    pub fn new(_content: &str, _language: &str) -> Self {
        Self::default()
    }

    /// Compute syntax highlights using tree-sitter.
    #[cfg(not(target_arch = "wasm32"))]
    fn compute_highlights(&mut self, language: &str) {
        self.spans.clear();

        if self.source_content.is_empty() {
            return;
        }

        // Select the appropriate language grammar
        // Rust is always available, other languages require "all-languages" feature
        let config_result = match language {
            "rust" => HighlightConfiguration::new(
                tree_sitter_rust::LANGUAGE.into(),
                "rust",
                tree_sitter_rust::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            #[cfg(feature = "all-languages")]
            "go" => HighlightConfiguration::new(
                tree_sitter_go::LANGUAGE.into(),
                "go",
                tree_sitter_go::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            #[cfg(feature = "all-languages")]
            "python" => HighlightConfiguration::new(
                tree_sitter_python::LANGUAGE.into(),
                "python",
                tree_sitter_python::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            #[cfg(feature = "all-languages")]
            "javascript" | "typescript" | "js" | "ts" => HighlightConfiguration::new(
                tree_sitter_javascript::LANGUAGE.into(),
                "javascript",
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::INJECTIONS_QUERY,
                tree_sitter_javascript::LOCALS_QUERY,
            ),
            // Default to Rust for unknown languages (better than no highlighting)
            _ => HighlightConfiguration::new(
                tree_sitter_rust::LANGUAGE.into(),
                "rust",
                tree_sitter_rust::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
        };

        let mut config = match config_result {
            Ok(config) => config,
            Err(e) => {
                log::warn!("Failed to create highlight config for {language}: {e}");
                return;
            }
        };

        // Configure recognized highlight names
        config.configure(HIGHLIGHT_NAMES);

        // Create highlighter and process the source
        let mut highlighter = Highlighter::new();
        let source_bytes = self.source_content.as_bytes();

        let highlights = match highlighter.highlight(&config, source_bytes, None, |_| None) {
            Ok(h) => h,
            Err(e) => {
                log::warn!("Failed to highlight: {e}");
                return;
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
                        self.spans.push(HighlightSpan {
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
    }

    /// Highlight a single line of source code.
    ///
    /// `line_num` is 1-indexed. Returns a `LayoutJob` ready for egui rendering.
    #[cfg(not(target_arch = "wasm32"))]
    #[profiling::function]
    pub fn highlight_line(&self, line_num: usize, line: &str, theme: AppTheme) -> LayoutJob {
        let mut job = LayoutJob::default();
        let font_id = typography::monospace(typography::SM);
        let default_color = text_color(theme);

        // If we have no highlight spans or line offsets, fall back to plain text
        if self.spans.is_empty() || self.line_offsets.is_empty() {
            job.append(line, 0.0, egui::TextFormat::simple(font_id, default_color));
            return job;
        }

        // Get the byte range for this line (0-indexed internally)
        let line_idx = line_num.saturating_sub(1);
        let line_start = self.line_offsets.get(line_idx).copied().unwrap_or(0);
        let line_end = self
            .line_offsets
            .get(line_idx + 1)
            .copied()
            .unwrap_or(self.source_content.len());

        // Find spans that overlap with this line
        let mut current_pos = 0usize; // Position within the line string

        // Collect spans for this line, adjusting to line-relative positions
        let mut line_spans: Vec<(usize, usize, Color32)> = Vec::new();

        for span in &self.spans {
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

    /// Get the syntax color spans for a single line (line_num is 1-indexed).
    ///
    /// Returns `(start, end, color)` tuples relative to the line content.
    /// Useful for compositing syntax colors with other overlays (e.g. diff highlights).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_line_spans(&self, line_num: usize, theme: AppTheme) -> Vec<(usize, usize, Color32)> {
        if self.spans.is_empty() || self.line_offsets.is_empty() {
            return Vec::new();
        }

        let line_idx = line_num.saturating_sub(1);
        let line_start = self.line_offsets.get(line_idx).copied().unwrap_or(0);
        let line_end = self
            .line_offsets
            .get(line_idx + 1)
            .copied()
            .unwrap_or(self.source_content.len());

        let mut line_spans = Vec::new();
        for span in &self.spans {
            if span.range.end <= line_start || span.range.start >= line_end {
                continue;
            }
            let span_start = span.range.start.saturating_sub(line_start);
            let span_end = span.range.end.saturating_sub(line_start);
            if span_start < span_end {
                line_spans.push((
                    span_start,
                    span_end,
                    highlight_color(span.highlight_idx, theme),
                ));
            }
        }
        line_spans.sort_by_key(|(start, _, _)| *start);
        line_spans
    }

    /// WASM stub - no syntax data available.
    #[cfg(target_arch = "wasm32")]
    pub fn get_line_spans(
        &self,
        _line_num: usize,
        _theme: AppTheme,
    ) -> Vec<(usize, usize, Color32)> {
        Vec::new()
    }

    /// Fallback when tree-sitter is not available - no syntax highlighting, just plain text.
    #[cfg(target_arch = "wasm32")]
    #[profiling::function]
    pub fn highlight_line(&self, _line_num: usize, line: &str, theme: AppTheme) -> LayoutJob {
        let mut job = LayoutJob::default();
        let font_id = typography::monospace(typography::SM);
        let default_color = text_color(theme);
        job.append(line, 0.0, egui::TextFormat::simple(font_id, default_color));
        job
    }
}

/// Get the color for a highlight index.
#[cfg(not(target_arch = "wasm32"))]
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

/// Stub for highlight_color when tree-sitter is not available.
#[cfg(target_arch = "wasm32")]
pub fn highlight_color(_idx: usize, theme: AppTheme) -> Color32 {
    text_color(theme)
}

/// Cache for syntax-highlighted source files read from disk.
///
/// Wraps [`SyntaxHighlightData`] with a file path and disk I/O.
/// Used by the unified finder and source preview overlays.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct HighlightCache {
    /// The file path this cache is for.
    pub file_path: std::path::PathBuf,
    /// Inner highlight data (source content, line offsets, spans).
    pub data: SyntaxHighlightData,
}

#[cfg(not(target_arch = "wasm32"))]
impl HighlightCache {
    /// Creates a new highlight cache for the given file.
    ///
    /// Returns `None` if the file cannot be read or is empty.
    #[must_use]
    pub fn new(file_path: std::path::PathBuf) -> Option<Self> {
        let source_content = std::fs::read_to_string(&file_path).ok()?;
        if source_content.is_empty() {
            return None;
        }
        let lang = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let data = SyntaxHighlightData::new(&source_content, lang);
        Some(Self { file_path, data })
    }

    /// Access the cached source content.
    pub fn source_content(&self) -> &str {
        &self.data.source_content
    }

    /// Access the cached highlight spans.
    pub fn spans(&self) -> &[HighlightSpan] {
        &self.data.spans
    }

    /// Access the cached line offsets.
    pub fn line_offsets(&self) -> &[usize] {
        &self.data.line_offsets
    }
}

/// Highlight a line of code using pre-computed tree-sitter spans.
///
/// Standalone function for callers that hold spans/offsets separately.
/// `line_num` is 1-indexed.
#[cfg(not(target_arch = "wasm32"))]
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

    if highlight_spans.is_empty() || line_offsets.is_empty() {
        job.append(line, 0.0, egui::TextFormat::simple(font_id, default_color));
        return job;
    }

    let line_idx = line_num.saturating_sub(1);
    let line_start = line_offsets.get(line_idx).copied().unwrap_or(0);
    let line_end = line_offsets
        .get(line_idx + 1)
        .copied()
        .unwrap_or(source_len);

    let mut current_pos = 0usize;
    let mut line_spans: Vec<(usize, usize, Color32)> = Vec::new();

    for span in highlight_spans {
        if span.range.end <= line_start || span.range.start >= line_end {
            continue;
        }
        let span_start_in_line = span.range.start.saturating_sub(line_start);
        let span_end_in_line = span.range.end.saturating_sub(line_start).min(line.len());
        if span_start_in_line >= span_end_in_line {
            continue;
        }
        let color = highlight_color(span.highlight_idx, theme);
        line_spans.push((span_start_in_line, span_end_in_line, color));
    }

    line_spans.sort_by_key(|(start, _, _)| *start);

    for (span_start, span_end, color) in line_spans {
        if span_start > current_pos {
            if let Some(text) = line.get(current_pos..span_start) {
                job.append(
                    text,
                    0.0,
                    egui::TextFormat::simple(font_id.clone(), default_color),
                );
            }
        }
        if let Some(text) = line.get(span_start..span_end) {
            job.append(text, 0.0, egui::TextFormat::simple(font_id.clone(), color));
        }
        current_pos = span_end;
    }

    if current_pos < line.len() {
        if let Some(text) = line.get(current_pos..) {
            job.append(
                text,
                0.0,
                egui::TextFormat::simple(font_id.clone(), default_color),
            );
        }
    }

    if job.is_empty() && line.is_empty() {
        job.append("", 0.0, egui::TextFormat::simple(font_id, default_color));
    }

    job
}
