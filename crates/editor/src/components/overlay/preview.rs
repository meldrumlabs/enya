//! Preview rendering utilities for the unified finder.
//!
//! This module provides rendering functions for source code previews
//! and diff previews used in the unified finder's preview pane.
//!
//! The diff preview uses the same beautiful GitHub-style colors as the
//! full diff viewer, with colored gutters and proper line backgrounds.

use egui::{Color32, RichText};

#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::finder_utils::FinderColors;
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::{HighlightCache, highlight_line_with_spans};
#[cfg(not(target_arch = "wasm32"))]
use crate::ui::palette;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Minimum number of context lines to show before and after the target line.
#[cfg(not(target_arch = "wasm32"))]
const MIN_CONTEXT_LINES: usize = 5;
/// Approximate height of each line in pixels (typography::SM ~13px + spacing).
#[cfg(not(target_arch = "wasm32"))]
const LINE_HEIGHT_PX: f32 = 18.0;

/// Renders a source code preview for metrics/alerts with tree-sitter syntax highlighting.
///
/// Matches the styling of `SourcePreviewOverlay` - no extra background frame.
/// Uses cached content and highlights for better performance (no disk I/O per frame).
#[cfg(not(target_arch = "wasm32"))]
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
                c.source_content(),
                c.spans(),
                c.line_offsets(),
                c.source_content().len(),
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
    let total_lines = source_lines.len();

    // Calculate how many lines we can fit in the available height
    let max_visible_lines = ((max_height / LINE_HEIGHT_PX) as usize).max(1);

    // Calculate context lines - use more if we have space, but at least MIN_CONTEXT_LINES
    // We want (context_before + 1 + context_after) <= max_visible_lines
    let context_lines = ((max_visible_lines.saturating_sub(1)) / 2).max(MIN_CONTEXT_LINES);

    // Calculate line range (1-indexed target_line)
    let mut start_line = target_line.saturating_sub(context_lines).max(1);
    let mut end_line = (target_line + context_lines).min(total_lines);

    // If we have room for more lines, expand the range to fill available space
    let current_range = end_line - start_line + 1;
    if current_range < max_visible_lines {
        let extra_lines = max_visible_lines - current_range;
        // Try to expand downward first, then upward
        let expand_down = (total_lines - end_line).min(extra_lines);
        end_line += expand_down;
        let remaining = extra_lines - expand_down;
        let expand_up = (start_line - 1).min(remaining);
        start_line -= expand_up;
    }

    // Get theme colors matching source_preview.rs
    let line_num_color = theme.text_tertiary();
    let highlight_bg = theme.highlight_line();

    // Capture available width before rendering to prevent expansion
    let available_width = ui.available_width();

    // Render directly in the overlay without extra background frame (matching source_preview.rs)
    // Use vertical-only scroll to avoid horizontal scroll bar - code will be clipped if too wide
    egui::ScrollArea::vertical()
        .id_salt("unified_finder_source_preview")
        .auto_shrink([false, false])
        .max_height(max_height)
        .show(ui, |ui| {
            // Hard lock width to prevent ANY expansion
            ui.set_width(available_width);

            // Set clip rect to prevent content from visually overflowing
            let clip_rect = ui.available_rect_before_wrap();
            ui.set_clip_rect(clip_rect);

            ui.vertical(|ui| {
                // Also lock the inner vertical layout
                ui.set_width(available_width);
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

/// Renders a single diff line with beautiful GitHub-style styling in the preview pane.
///
/// Features:
/// - Colored gutter stripe on the left (green for additions, red for deletions)
/// - Subtle background colors for changed lines
/// - Proper text colors matching the full diff viewer
pub fn render_diff_line_preview(
    ui: &mut egui::Ui,
    line: &str,
    _base_text_color: Color32,
    theme: AppTheme,
) {
    let kind = classify_diff_line(line);

    let (text_color, bg_color, gutter_color) = match kind {
        DiffLineKind::Addition => (
            theme.diff_added_text(),
            Some(theme.diff_added_bg()),
            Some(theme.diff_added_gutter()),
        ),
        DiffLineKind::Deletion => (
            theme.diff_removed_text(),
            Some(theme.diff_removed_bg()),
            Some(theme.diff_removed_gutter()),
        ),
        DiffLineKind::HunkHeader => (theme.diff_hunk_text(), Some(theme.diff_hunk_bg()), None),
        DiffLineKind::FileHeader => (
            theme.diff_file_header(),
            Some(theme.diff_file_header_bg()),
            None,
        ),
        DiffLineKind::Context => (theme.diff_context_text(), None, None),
    };

    // Strip the +/- prefix for cleaner display (but keep for headers)
    let content = match kind {
        DiffLineKind::Addition | DiffLineKind::Deletion => line.get(1..).unwrap_or(line),
        DiffLineKind::Context => line.get(1..).unwrap_or(line),
        _ => line,
    };
    let content = if content.is_empty() { " " } else { content };

    // Get available width for full-line background
    let available_width = ui.available_width();

    // Render the line with gutter and background
    let response = ui.horizontal(|ui| {
        // Gutter stripe (3px wide colored bar on the left)
        let gutter_width = 3.0;
        let line_height = typography::SM + 4.0;
        let (gutter_rect, _) =
            ui.allocate_exact_size(egui::vec2(gutter_width, line_height), egui::Sense::hover());

        if let Some(gc) = gutter_color {
            ui.painter().rect_filled(gutter_rect, 0.0, gc);
        }

        ui.add_space(6.0);

        // Content
        ui.label(
            RichText::new(content)
                .color(text_color)
                .font(typography::monospace(typography::SM)),
        );
    });

    // Draw full-width background behind the line
    if let Some(bg) = bg_color {
        let rect = egui::Rect::from_min_size(
            response.response.rect.min,
            egui::vec2(available_width, response.response.rect.height()),
        );
        let bg_painter = ui.painter().clone().with_layer_id(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("diff_preview_bg"),
        ));
        bg_painter.rect_filled(rect, 0.0, bg);
    }
}
