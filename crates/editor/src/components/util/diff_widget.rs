//! Shared diff rendering widgets — stateless helpers for drawing diff lines.

use egui::RichText;

use super::diff_rendering::{DiffLine, DiffLineKind, FileDiff};
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

/// Render a single diff line with gutter, line numbers, and content (unified view).
///
/// Uses direct painter calls for consistent rendering (same approach as split view).
pub fn render_diff_line(
    ui: &mut egui::Ui,
    line: &DiffLine,
    line_num_width: usize,
    theme: AppTheme,
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

    // Content
    let content = if line.content.is_empty() {
        " "
    } else {
        &line.content
    };

    // Paint word highlights as background rects, then paint text on top
    if !line.word_highlights.is_empty() {
        let word_bg = match line.kind {
            DiffLineKind::Addition => Some(theme.diff_added_word_bg()),
            DiffLineKind::Deletion => Some(theme.diff_removed_word_bg()),
            _ => None,
        };
        if let Some(bg) = word_bg {
            let font = typography::monospace(typography::SM);
            let char_width = ui
                .painter()
                .layout_no_wrap("m".to_string(), font.clone(), text_color)
                .size()
                .x;
            for &(start, end) in &line.word_highlights {
                // Convert byte offsets to character offsets for positioning
                let char_start = content[..start.min(content.len())].chars().count();
                let char_end = content[..end.min(content.len())].chars().count();
                let hl_x = cursor_x + char_start as f32 * char_width;
                let hl_w = (char_end - char_start) as f32 * char_width;
                let hl_rect = egui::Rect::from_min_size(
                    egui::pos2(hl_x, line_rect.top()),
                    egui::vec2(hl_w, line_height),
                );
                ui.painter().rect_filled(hl_rect, 0.0, bg);
            }
        }
    }

    ui.painter().text(
        egui::pos2(cursor_x, line_rect.center().y),
        egui::Align2::LEFT_CENTER,
        content,
        typography::monospace(typography::SM),
        text_color,
    );
}

/// Render a header line spanning full width in split view.
pub fn render_split_header_line(
    ui: &mut egui::Ui,
    line: &DiffLine,
    available_width: f32,
    theme: AppTheme,
) {
    let (text_color, bg_color) = match line.kind {
        DiffLineKind::HunkHeader => (theme.diff_hunk_text(), theme.diff_hunk_bg()),
        DiffLineKind::FileHeader => (theme.diff_file_header(), theme.diff_file_header_bg()),
        _ => return,
    };

    let line_height = typography::SM + 6.0;
    let (line_rect, _) = ui.allocate_exact_size(
        egui::vec2(available_width, line_height),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(line_rect, 0.0, bg_color);
    ui.painter().text(
        line_rect.left_center() + egui::vec2(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        &line.content,
        typography::monospace(typography::SM),
        text_color,
    );
}

/// Render a single line in the split view.
pub fn render_split_line(
    ui: &mut egui::Ui,
    line: Option<&DiffLine>,
    line_num_width: usize,
    is_left: bool,
    side_width: f32,
    theme: AppTheme,
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

    // Content — truncate to fit
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

    ui.painter().text(
        egui::pos2(cursor_x, line_rect.center().y),
        egui::Align2::LEFT_CENTER,
        content,
        typography::monospace(typography::SM),
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
