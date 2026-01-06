//! Preview rendering utilities for the unified finder.
//!
//! This module provides rendering functions for source code previews
//! and diff previews used in the unified finder's preview pane.

use egui::{Color32, RichText};

use super::syntax_highlight::{HighlightCache, highlight_line_with_spans};
use crate::components::util::finder_utils::FinderColors;
use crate::ui::palette;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Number of context lines to show before and after the target line.
const CONTEXT_LINES: usize = 5;

/// Renders a source code preview for metrics/alerts with tree-sitter syntax highlighting.
///
/// Matches the styling of `SourcePreviewOverlay` - no extra background frame.
/// Uses cached content and highlights for better performance (no disk I/O per frame).
#[allow(clippy::too_many_arguments)]
pub fn render_source_preview(
    ui: &mut egui::Ui,
    file_path: &std::path::Path,
    target_line: usize,
    max_height: f32,
    text_col: Color32,
    _colors: &FinderColors,
    theme: AppTheme,
    cache: Option<&HighlightCache>,
) {
    // Use cache if available (preferred - no disk I/O)
    let (source_content, highlight_spans, line_offsets, source_len) =
        if let Some(c) = cache.filter(|c| c.file_path == file_path) {
            // Use cached content and highlights directly
            (
                c.source_content.as_str(),
                &c.spans,
                &c.line_offsets,
                c.source_content.len(),
            )
        } else {
            // No cache available - show placeholder
            // (This shouldn't happen in normal operation since update_highlight_cache runs first)
            ui.label(
                RichText::new("Loading source...")
                    .color(text_col.gamma_multiply(0.5))
                    .italics()
                    .size(typography::SM),
            );
            return;
        };

    if source_content.is_empty() {
        ui.label(
            RichText::new("Empty file")
                .color(text_col.gamma_multiply(0.5))
                .italics()
                .size(typography::SM),
        );
        return;
    }

    let source_lines: Vec<&str> = source_content.lines().collect();

    // Calculate line range (1-indexed target_line)
    let start_line = target_line.saturating_sub(CONTEXT_LINES).max(1);
    let end_line = (target_line + CONTEXT_LINES).min(source_lines.len());

    // Get theme colors matching source_preview.rs
    let line_num_color = theme.text_tertiary();
    let highlight_bg = theme.highlight_line();

    // Render directly in the overlay without extra background frame (matching source_preview.rs)
    egui::ScrollArea::horizontal()
        .id_salt("unified_finder_source_preview")
        .auto_shrink([false, false])
        .max_height(max_height - 20.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    // Calculate the width needed for line numbers
                    let line_num_width = format!("{}", source_lines.len()).len();

                    for line_num in start_line..=end_line {
                        let line_idx = line_num - 1;
                        let line_content = source_lines.get(line_idx).copied().unwrap_or("");
                        let is_target = line_num == target_line;

                        // Syntax highlight the code using tree-sitter spans
                        let highlighted = highlight_line_with_spans(
                            line_num,
                            line_content,
                            highlight_spans,
                            line_offsets,
                            source_len,
                            theme,
                        );

                        // Draw highlight background for target line
                        if is_target {
                            let response = ui.horizontal(|ui| {
                                // Line number with arrow
                                ui.label(
                                    RichText::new(format!("{line_num:>line_num_width$} →"))
                                        .color(palette::semantic::WARNING)
                                        .font(typography::monospace(typography::SM)),
                                );
                                ui.add_space(4.0);
                                // Code content with syntax highlighting
                                ui.label(highlighted);
                            });

                            // Draw background behind the row
                            let rect = response.response.rect.expand2(egui::vec2(4.0, 1.0));
                            ui.painter().rect_filled(rect, 2.0, highlight_bg);
                        } else {
                            ui.horizontal(|ui| {
                                // Line number
                                ui.label(
                                    RichText::new(format!("{line_num:>line_num_width$}  "))
                                        .color(line_num_color)
                                        .font(typography::monospace(typography::SM)),
                                );
                                ui.add_space(4.0);
                                // Code content with syntax highlighting
                                ui.label(highlighted);
                            });
                        }
                    }
                });
            });
        });
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

/// Classifies a diff line by its prefix.
#[must_use]
pub fn classify_diff_line(line: &str) -> DiffLineKind {
    if line.starts_with("@@") {
        DiffLineKind::HunkHeader
    } else if line.starts_with('+') && !line.starts_with("+++") {
        DiffLineKind::Addition
    } else if line.starts_with('-') && !line.starts_with("---") {
        DiffLineKind::Deletion
    } else if line.starts_with("diff --git")
        || line.starts_with("---")
        || line.starts_with("+++")
        || line.starts_with("index ")
    {
        DiffLineKind::FileHeader
    } else {
        DiffLineKind::Context
    }
}

/// Renders a single diff line with appropriate styling in the preview pane.
pub fn render_diff_line_preview(ui: &mut egui::Ui, line: &str, base_text_color: Color32) {
    let kind = classify_diff_line(line);

    let (text_color, bg_color) = match kind {
        DiffLineKind::Addition => (palette::diff::ADDED_TEXT, Some(palette::diff::ADDED_BG)),
        DiffLineKind::Deletion => (palette::diff::REMOVED_TEXT, Some(palette::diff::REMOVED_BG)),
        DiffLineKind::HunkHeader => (palette::diff::HUNK_TEXT, Some(palette::diff::HUNK_BG)),
        DiffLineKind::FileHeader => (palette::diff::FILE_HEADER, None),
        DiffLineKind::Context => (base_text_color.gamma_multiply(0.7), None),
    };

    // Ensure the line content has at least some content to display properly
    let content = if line.is_empty() { " " } else { line };

    // For lines with background, use a Frame to properly layer bg behind text
    if let Some(bg) = bg_color {
        egui::Frame::new()
            .fill(bg)
            .inner_margin(egui::Margin::symmetric(4, 1))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(content)
                        .color(text_color)
                        .size(typography::SM)
                        .monospace(),
                );
            });
    } else {
        ui.label(
            RichText::new(content)
                .color(text_color)
                .size(typography::SM)
                .monospace(),
        );
    }
}
