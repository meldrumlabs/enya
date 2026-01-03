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

    match theme {
        AppTheme::Light => match name {
            "keyword" => Color32::from_rgb(160, 50, 160), // purple
            "string" | "escape" => Color32::from_rgb(50, 130, 50), // green
            "comment" => Color32::from_rgb(120, 120, 120), // gray
            "function" | "function.builtin" => Color32::from_rgb(50, 100, 180), // blue
            "function.macro" => Color32::from_rgb(0, 130, 150), // teal
            "type" | "type.builtin" | "constructor" => Color32::from_rgb(180, 100, 50), // orange
            "number" | "constant" | "constant.builtin" => Color32::from_rgb(180, 80, 80), // red-ish
            "attribute" => Color32::from_rgb(150, 120, 50), // yellow-brown
            "variable" | "variable.parameter" | "property" | "label" => {
                Color32::from_rgb(40, 40, 40)
            } // dark
            "variable.builtin" => Color32::from_rgb(160, 50, 160), // purple (self)
            "operator" | "punctuation" | "punctuation.bracket" | "punctuation.delimiter" => {
                Color32::from_rgb(60, 60, 60) // dark gray
            }
            _ => Color32::from_rgb(40, 40, 40), // default dark
        },
        AppTheme::Dark => match name {
            "keyword" => Color32::from_rgb(200, 120, 220), // purple
            "string" | "escape" => Color32::from_rgb(130, 200, 130), // green
            "comment" => Color32::from_rgb(128, 128, 128), // gray
            "function" | "function.builtin" => Color32::from_rgb(100, 160, 255), // blue
            "function.macro" => Color32::from_rgb(80, 200, 200), // cyan
            "type" | "type.builtin" | "constructor" => Color32::from_rgb(220, 160, 100), // orange
            "number" | "constant" | "constant.builtin" => Color32::from_rgb(220, 120, 120), // red-ish
            "attribute" => Color32::from_rgb(220, 190, 100), // yellow
            "variable" | "variable.parameter" | "property" | "label" => {
                Color32::from_rgb(220, 220, 220)
            } // light
            "variable.builtin" => Color32::from_rgb(200, 120, 220), // purple (self)
            "operator" | "punctuation" | "punctuation.bracket" | "punctuation.delimiter" => {
                Color32::from_rgb(180, 180, 180) // light gray
            }
            _ => Color32::from_rgb(220, 220, 220), // default light
        },
    }
}

/// Stub for highlight_color when tree-sitter is not available.
#[cfg(target_arch = "wasm32")]
pub fn highlight_color(_idx: usize, theme: AppTheme) -> Color32 {
    text_color(theme)
}
