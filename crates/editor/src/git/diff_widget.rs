//! Shared diff rendering widgets — stateless helpers for drawing diff lines.

use egui::RichText;
use egui::text::LayoutJob;

use super::diff::{DiffLine, DiffLineKind, FileDiff};
use crate::components::util::syntax_highlight::SyntaxHighlightData;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Compute the maximum line number width for formatting.
pub fn max_line_num_width(file_diff: &FileDiff) -> usize {
    let max_line_num = file_diff
        .lines
        .iter()
        .filter_map(|l| l.old_line_num.max(l.new_line_num))
        .max()
        .unwrap_or(1);
    max_line_num.to_string().len().max(3)
}

/// Render a +N or -N stat badge.
pub fn render_stat_badge(ui: &mut egui::Ui, count: usize, is_addition: bool, theme: AppTheme) {
    let (text, color) = if is_addition {
        (format!("+{count}"), theme.diff_added_gutter())
    } else {
        (format!("-{count}"), theme.diff_removed_gutter())
    };
    ui.label(
        RichText::new(text)
            .color(color)
            .font(typography::monospace(typography::XS)),
    );
}

/// Get colors for a diff line kind: (text_color, bg_color, gutter_color).
pub fn diff_line_colors(
    kind: DiffLineKind,
    theme: AppTheme,
) -> (egui::Color32, Option<egui::Color32>, Option<egui::Color32>) {
    match kind {
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
    }
}

/// Build a `LayoutJob` for diff line content with syntax highlighting, word highlights,
/// and optional search highlights.
///
/// Uses a sweep-line algorithm to composite three layers:
/// 1. **Syntax colors** (from tree-sitter) → text color
/// 2. **Word highlights** (diff-specific changes) → background color
/// 3. **Search highlights** (query matches) → background color (takes priority over word highlights)
///
/// `search_highlights` is a slice of `(start, end, is_current_match)`.
/// Pass `&[]` when search is not active.
pub fn build_diff_line_layout_job(
    content: &str,
    word_highlights: &[(usize, usize)],
    base_text_color: egui::Color32,
    word_bg: Option<egui::Color32>,
    syntax_spans: &[(usize, usize, egui::Color32)],
    search_highlights: &[(usize, usize, bool)],
    font_size: f32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font_id = typography::monospace(font_size);

    if content.is_empty() {
        job.append(" ", 0.0, egui::TextFormat::simple(font_id, base_text_color));
        return job;
    }

    let len = content.len();

    // Collect all boundary points where formatting changes
    let mut boundaries: Vec<usize> = Vec::with_capacity(
        2 + syntax_spans.len() * 2 + word_highlights.len() * 2 + search_highlights.len() * 2,
    );
    boundaries.push(0);
    boundaries.push(len);
    for &(start, end, _) in syntax_spans {
        if start < len {
            boundaries.push(start);
        }
        if end <= len {
            boundaries.push(end);
        }
    }
    for &(start, end) in word_highlights {
        if start < len {
            boundaries.push(start);
        }
        if end <= len {
            boundaries.push(end);
        }
    }
    for &(start, end, _) in search_highlights {
        if start < len {
            boundaries.push(start);
        }
        if end <= len {
            boundaries.push(end);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    // Snap boundaries to valid UTF-8 char boundaries
    for b in &mut boundaries {
        while *b < len && !content.is_char_boundary(*b) {
            *b += 1;
        }
    }
    boundaries.dedup();

    // Iterate through segments between boundaries
    for pair in boundaries.windows(2) {
        let seg_start = pair[0];
        let seg_end = pair[1];
        if seg_start >= seg_end || seg_start >= len {
            continue;
        }
        let seg_end = seg_end.min(len);

        let Some(text) = content.get(seg_start..seg_end) else {
            continue;
        };

        // Determine syntax color at this segment (first matching span)
        let text_color = syntax_spans
            .iter()
            .find(|&&(s, e, _)| seg_start >= s && seg_start < e)
            .map(|&(_, _, c)| c)
            .unwrap_or(base_text_color);

        // Determine if this segment is inside a word highlight
        let in_word_highlight = word_highlights
            .iter()
            .any(|&(s, e)| seg_start >= s && seg_start < e);

        // Determine if this segment is inside a search highlight
        let search_match = search_highlights
            .iter()
            .find(|&&(s, e, _)| seg_start >= s && seg_start < e);

        // Search highlights take priority over word highlights for background
        let bg = if let Some(&(_, _, is_current)) = search_match {
            if is_current {
                Some(egui::Color32::from_rgba_premultiplied(230, 160, 0, 180))
            } else {
                Some(egui::Color32::from_rgba_premultiplied(180, 140, 0, 100))
            }
        } else if in_word_highlight {
            word_bg
        } else {
            None
        };

        // For search highlights, use dark text for contrast
        let final_text_color = if search_match.is_some() {
            egui::Color32::from_rgb(30, 30, 30)
        } else {
            text_color
        };

        let mut format = egui::TextFormat::simple(font_id.clone(), final_text_color);
        if let Some(bg_color) = bg {
            format.background = bg_color;
        }
        // Add underline to word-highlighted segments (not search matches) for extra visual cue
        if in_word_highlight && search_match.is_none() {
            if let Some(bg_color) = word_bg {
                format.underline = egui::Stroke::new(1.0, bg_color.gamma_multiply(1.8));
            }
        }
        job.append(text, 0.0, format);
    }

    if job.is_empty() {
        job.append(" ", 0.0, egui::TextFormat::simple(font_id, base_text_color));
    }

    job
}

/// Get the syntax color spans for a diff line, using its reconstruction line number.
///
/// Returns `(start, end, color)` tuples relative to the line content.
pub fn get_syntax_spans_for_line(
    line: &DiffLine,
    old_highlight: Option<&SyntaxHighlightData>,
    new_highlight: Option<&SyntaxHighlightData>,
    theme: AppTheme,
) -> Vec<(usize, usize, egui::Color32)> {
    let (syntax_data, recon_num) = match line.kind {
        DiffLineKind::Deletion => (old_highlight, line.old_recon_num),
        DiffLineKind::Addition => (new_highlight, line.new_recon_num),
        DiffLineKind::Context => (new_highlight, line.new_recon_num),
        _ => (None, None),
    };

    syntax_data
        .and_then(|data| recon_num.map(|n| data.get_line_spans(n, theme)))
        .unwrap_or_default()
}

/// Render a single diff line with gutter, line numbers, and content (unified view).
///
/// Uses direct painter calls for consistent rendering (same approach as split view).
pub fn render_diff_line(
    ui: &mut egui::Ui,
    line: &DiffLine,
    line_num_width: usize,
    theme: AppTheme,
    old_highlight: Option<&SyntaxHighlightData>,
    new_highlight: Option<&SyntaxHighlightData>,
) {
    let (text_color, bg_color, gutter_color) = diff_line_colors(line.kind, theme);
    let available_width = ui.available_width();
    let line_height = typography::SM + 6.0;

    let gutter_width = 4.0;
    let line_num_area_width = (line_num_width * 2 + 3) as f32 * 8.0;

    // Allocate the full line rect
    let (line_rect, _) = ui.allocate_exact_size(
        egui::vec2(available_width, line_height),
        egui::Sense::hover(),
    );

    // Background fill
    if let Some(bg) = bg_color {
        ui.painter().rect_filled(line_rect, 0.0, bg);
    }

    let mut cursor_x = line_rect.left();

    // Gutter stripe
    if let Some(gc) = gutter_color {
        let gutter_rect = egui::Rect::from_min_size(
            egui::pos2(cursor_x, line_rect.top()),
            egui::vec2(gutter_width, line_height),
        );
        ui.painter().rect_filled(gutter_rect, 0.0, gc);
    }
    cursor_x += gutter_width + 4.0;

    // Line numbers background
    let line_num_rect = egui::Rect::from_min_size(
        egui::pos2(cursor_x, line_rect.top()),
        egui::vec2(line_num_area_width, line_height),
    );
    ui.painter()
        .rect_filled(line_num_rect, 0.0, theme.diff_line_number_bg());

    // Line numbers text
    let old_num_str = line
        .old_line_num
        .map(|n| format!("{n:>line_num_width$}"))
        .unwrap_or_else(|| " ".repeat(line_num_width));
    let new_num_str = line
        .new_line_num
        .map(|n| format!("{n:>line_num_width$}"))
        .unwrap_or_else(|| " ".repeat(line_num_width));

    ui.painter().text(
        line_num_rect.left_center() + egui::vec2(4.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{old_num_str} {new_num_str}"),
        typography::monospace(typography::SM),
        theme.diff_line_number(),
    );
    cursor_x += line_num_area_width + 8.0;

    // Content with syntax highlighting
    let content = if line.content.is_empty() {
        " "
    } else {
        &line.content
    };

    let syntax_spans = get_syntax_spans_for_line(line, old_highlight, new_highlight, theme);
    let word_bg = match line.kind {
        DiffLineKind::Addition => Some(theme.diff_added_word_bg()),
        DiffLineKind::Deletion => Some(theme.diff_removed_word_bg()),
        _ => None,
    };

    let job = build_diff_line_layout_job(
        content,
        &line.word_highlights,
        text_color,
        word_bg,
        &syntax_spans,
        &[],
        typography::SM,
    );

    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        egui::pos2(cursor_x, line_rect.center().y - galley.size().y / 2.0),
        galley,
        text_color,
    );
}

/// Render a styled hunk separator with hidden line count and function context.
///
/// Displays: `··· N lines hidden ··· fn foo()` with a subtle background.
pub fn render_hunk_separator(
    ui: &mut egui::Ui,
    line: &DiffLine,
    available_width: f32,
    theme: AppTheme,
) {
    if line.kind == DiffLineKind::FileHeader {
        // File headers are not shown separately — the file path header handles this.
        return;
    }
    if line.kind != DiffLineKind::HunkHeader {
        return;
    }

    let line_height = typography::SM + 12.0;
    let (line_rect, response) = ui.allocate_exact_size(
        egui::vec2(available_width, line_height),
        egui::Sense::hover(),
    );

    // Background — slightly brighter on hover
    let bg = if response.hovered() {
        theme.diff_hunk_bg().gamma_multiply(1.3)
    } else {
        theme.diff_hunk_bg()
    };
    ui.painter().rect_filled(line_rect, 0.0, bg);

    // Top/bottom separator lines
    let sep_color = theme.border_subtle().gamma_multiply(0.6);
    ui.painter().hline(
        line_rect.x_range(),
        line_rect.top(),
        egui::Stroke::new(1.0, sep_color),
    );
    ui.painter().hline(
        line_rect.x_range(),
        line_rect.bottom(),
        egui::Stroke::new(1.0, sep_color),
    );

    // Build display text
    let hidden = line.hidden_lines.unwrap_or(0);
    let dots = "\u{00B7}\u{00B7}\u{00B7}"; // ···
    let label = if hidden > 0 {
        format!("{dots} {hidden} lines hidden {dots}")
    } else {
        format!("{dots}{dots}{dots}")
    };

    let text_alpha = if response.hovered() { 1.0 } else { 0.7 };
    let text_color = theme.diff_hunk_text().gamma_multiply(text_alpha);

    ui.painter().text(
        line_rect.center(),
        egui::Align2::CENTER_CENTER,
        &label,
        typography::proportional(typography::XS),
        text_color,
    );

    // Function context on the right
    if let Some(ref ctx) = line.hunk_context {
        let ctx_color = theme.syntax_function().gamma_multiply(text_alpha * 0.8);
        ui.painter().text(
            egui::pos2(line_rect.right() - 16.0, line_rect.center().y),
            egui::Align2::RIGHT_CENTER,
            ctx,
            typography::proportional(typography::XS),
            ctx_color,
        );
    }
}

/// Render a header line spanning full width in split view.
pub fn render_split_header_line(
    ui: &mut egui::Ui,
    line: &DiffLine,
    available_width: f32,
    theme: AppTheme,
) {
    if line.kind == DiffLineKind::FileHeader {
        return;
    }
    // Delegate to the styled hunk separator
    render_hunk_separator(ui, line, available_width, theme);
}

/// Render a single line in the split view.
pub fn render_split_line(
    ui: &mut egui::Ui,
    line: Option<&DiffLine>,
    line_num_width: usize,
    is_left: bool,
    side_width: f32,
    theme: AppTheme,
    syntax_data: Option<&SyntaxHighlightData>,
) {
    ui.set_max_width(side_width);
    let line_height = typography::SM + 6.0;

    let Some(line) = line else {
        // Empty placeholder line
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(side_width, line_height), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 0.0, theme.diff_line_number_bg().gamma_multiply(0.5));
        return;
    };

    let (text_color, bg_color, gutter_color) = diff_line_colors(line.kind, theme);

    let gutter_width = 3.0;
    let line_num_area_width = (line_num_width + 1) as f32 * 8.0;
    let content_max_width = (side_width - gutter_width - line_num_area_width - 12.0).max(50.0);

    let (line_rect, _) =
        ui.allocate_exact_size(egui::vec2(side_width, line_height), egui::Sense::hover());

    if let Some(bg) = bg_color {
        ui.painter().rect_filled(line_rect, 0.0, bg);
    }

    let mut cursor_x = line_rect.left();

    // Gutter stripe
    if let Some(gc) = gutter_color {
        let gutter_rect = egui::Rect::from_min_size(
            egui::pos2(cursor_x, line_rect.top()),
            egui::vec2(gutter_width, line_height),
        );
        ui.painter().rect_filled(gutter_rect, 0.0, gc);
    }
    cursor_x += gutter_width + 2.0;

    // Line number
    let line_num_rect = egui::Rect::from_min_size(
        egui::pos2(cursor_x, line_rect.top()),
        egui::vec2(line_num_area_width, line_height),
    );
    ui.painter()
        .rect_filled(line_num_rect, 0.0, theme.diff_line_number_bg());

    let line_num = if is_left {
        line.old_line_num
    } else {
        line.new_line_num
    };
    let line_num_str = line_num
        .map(|n| format!("{n:>line_num_width$}"))
        .unwrap_or_else(|| " ".repeat(line_num_width));

    ui.painter().text(
        line_num_rect.left_center() + egui::vec2(2.0, 0.0),
        egui::Align2::LEFT_CENTER,
        line_num_str,
        typography::monospace(typography::SM),
        theme.diff_line_number(),
    );
    cursor_x += line_num_area_width + 4.0;

    // Content — truncate to fit, with syntax highlighting
    let content = if line.content.is_empty() {
        " ".to_string()
    } else {
        let char_width = 7.0;
        let max_chars = (content_max_width / char_width) as usize;
        let char_count = line.content.chars().count();
        if char_count > max_chars && max_chars > 3 {
            let truncate_at = max_chars.saturating_sub(1);
            let truncated: String = line.content.chars().take(truncate_at).collect();
            format!("{truncated}\u{2026}")
        } else {
            line.content.clone()
        }
    };

    let syntax_spans = if let Some(data) = syntax_data {
        let recon_num = if is_left {
            line.old_recon_num
        } else {
            line.new_recon_num
        };
        recon_num
            .map(|n| data.get_line_spans(n, theme))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let word_bg = match line.kind {
        DiffLineKind::Addition => Some(theme.diff_added_word_bg()),
        DiffLineKind::Deletion => Some(theme.diff_removed_word_bg()),
        _ => None,
    };

    let job = build_diff_line_layout_job(
        &content,
        &line.word_highlights,
        text_color,
        word_bg,
        &syntax_spans,
        &[],
        typography::SM,
    );

    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        egui::pos2(cursor_x, line_rect.center().y - galley.size().y / 2.0),
        galley,
        text_color,
    );
}

/// Render text with word-level highlights.
pub fn render_highlighted_text(
    ui: &mut egui::Ui,
    content: &str,
    word_highlights: &[(usize, usize)],
    base_color: egui::Color32,
    kind: DiffLineKind,
    theme: AppTheme,
) {
    let word_bg = match kind {
        DiffLineKind::Addition => Some(theme.diff_added_word_bg()),
        DiffLineKind::Deletion => Some(theme.diff_removed_word_bg()),
        _ => None,
    };

    if word_highlights.is_empty() {
        ui.label(
            RichText::new(content)
                .color(base_color)
                .font(typography::monospace(typography::MD)),
        );
        return;
    }

    let mut segments: Vec<(&str, bool)> = Vec::new();
    let mut pos = 0;
    for &(start, end) in word_highlights {
        if start > pos {
            if let Some(text) = content.get(pos..start) {
                segments.push((text, false));
            }
        }
        if let Some(text) = content.get(start..end) {
            segments.push((text, true));
        }
        pos = end;
    }
    if pos < content.len() {
        if let Some(text) = content.get(pos..) {
            segments.push((text, false));
        }
    }

    for (text, is_highlighted) in segments {
        if is_highlighted {
            if let Some(bg) = word_bg {
                egui::Frame::new()
                    .fill(bg)
                    .corner_radius(2.0)
                    .inner_margin(egui::Margin::symmetric(0, 0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(text)
                                .color(base_color)
                                .font(typography::monospace(typography::MD)),
                        );
                    });
            } else {
                ui.label(
                    RichText::new(text)
                        .color(base_color)
                        .font(typography::monospace(typography::MD)),
                );
            }
        } else {
            ui.label(
                RichText::new(text)
                    .color(base_color)
                    .font(typography::monospace(typography::MD)),
            );
        }
    }
}
