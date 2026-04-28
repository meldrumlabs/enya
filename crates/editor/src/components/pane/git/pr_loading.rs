//! Premium loading skeleton for PR review panes.
//!
//! Draws placeholder bars that mimic the shape of PR content
//! with a single sweeping accent highlight across the entire area.

use egui::{Color32, Pos2, Rect, Vec2};

/// Render a premium skeleton for the PR list loading state.
///
/// Draws placeholder rows with a single horizontal shimmer bar
/// that sweeps left-to-right across the entire content area.
pub fn render_pr_list_skeleton(ui: &mut egui::Ui, theme: crate::ui::theme::AppTheme) {
    let time = ui.ctx().input(|i| i.time) as f32;
    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter();

    let row_height = 48.0;
    let gap = 8.0;
    let avatar_size = 28.0;
    let max_rows = ((rect.height() - 60.0) / (row_height + gap)).max(3.0).min(6.0) as usize;

    let bg = theme.bg_elevated().gamma_multiply(0.45);

    // ── Draw placeholder shapes ──
    for i in 0..max_rows {
        let y = rect.min.y + 20.0 + i as f32 * (row_height + gap);
        let row_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 12.0, y),
            Vec2::new(rect.width() - 24.0, row_height),
        );

        // Row background
        painter.rect_filled(row_rect, 6.0, bg);

        // Avatar circle
        let avatar_center = Pos2::new(row_rect.min.x + 20.0, row_rect.center().y);
        painter.circle_filled(avatar_center, avatar_size / 2.0, bg);

        // Title bar (varying widths)
        let title_width = rect.width() * (0.45 + (i as f32 * 0.08).sin() * 0.15);
        let title_rect = Rect::from_min_size(
            Pos2::new(row_rect.min.x + 44.0, row_rect.min.y + 10.0),
            Vec2::new(title_width, 12.0),
        );
        painter.rect_filled(title_rect, 4.0, bg);

        // Subtitle bar
        let subtitle_width = rect.width() * (0.25 + (i as f32 * 0.12).cos() * 0.1);
        let subtitle_rect = Rect::from_min_size(
            Pos2::new(row_rect.min.x + 44.0, row_rect.min.y + 28.0),
            Vec2::new(subtitle_width, 8.0),
        );
        painter.rect_filled(subtitle_rect, 4.0, bg);
    }

    // Bottom label placeholder
    let label_y = rect.min.y + 20.0 + max_rows as f32 * (row_height + gap) + 16.0;
    let label_rect = Rect::from_min_size(
        Pos2::new(rect.center().x - 80.0, label_y),
        Vec2::new(160.0, 16.0),
    );
    painter.rect_filled(label_rect, 4.0, theme.bg_elevated().gamma_multiply(0.35));

    // ── Single shimmer sweep across the whole area ──
    let content_top = rect.min.y + 20.0;
    let content_bottom = label_y + 16.0;
    let content_height = content_bottom - content_top;
    let content_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 12.0, content_top),
        Vec2::new(rect.width() - 24.0, content_height),
    );
    draw_shimmer_bar(painter, content_rect, time, theme);

    ui.ctx().request_repaint();
}

/// Render a premium skeleton for the PR detail loading state.
pub fn render_pr_detail_skeleton(ui: &mut egui::Ui, theme: crate::ui::theme::AppTheme) {
    let time = ui.ctx().input(|i| i.time) as f32;
    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter();

    let bg = theme.bg_elevated().gamma_multiply(0.45);

    // ── Title area ──
    let title_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 12.0, rect.min.y + 16.0),
        Vec2::new(rect.width() * 0.6, 18.0),
    );
    painter.rect_filled(title_rect, 4.0, bg);

    let subtitle_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 12.0, rect.min.y + 42.0),
        Vec2::new(rect.width() * 0.35, 10.0),
    );
    painter.rect_filled(subtitle_rect, 4.0, bg);

    // ── Description blocks ──
    let mut y = rect.min.y + 72.0;
    for i in 0..3 {
        let line_width = rect.width() * (0.9 - (i as f32 * 0.15));
        let line_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 12.0, y),
            Vec2::new(line_width, 10.0),
        );
        painter.rect_filled(line_rect, 4.0, bg);
        y += 18.0;
    }

    // ── File diff rows ──
    y += 16.0;
    for i in 0..4 {
        let row_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 12.0, y),
            Vec2::new(rect.width() - 24.0, 32.0),
        );
        painter.rect_filled(row_rect, 4.0, bg);

        // File icon
        let icon_rect = Rect::from_min_size(
            Pos2::new(row_rect.min.x + 10.0, row_rect.center().y - 6.0),
            Vec2::new(12.0, 12.0),
        );
        painter.rect_filled(icon_rect, 2.0, bg);

        // Filename
        let name_width = rect.width() * (0.4 + (i as f32 * 0.1).sin() * 0.1);
        let name_rect = Rect::from_min_size(
            Pos2::new(row_rect.min.x + 32.0, row_rect.center().y - 5.0),
            Vec2::new(name_width, 10.0),
        );
        painter.rect_filled(name_rect, 4.0, bg);

        // Diff stat
        let stat_rect = Rect::from_min_size(
            Pos2::new(row_rect.max.x - 80.0, row_rect.center().y - 4.0),
            Vec2::new(60.0, 8.0),
        );
        painter.rect_filled(stat_rect, 4.0, bg);

        y += 40.0;
    }

    // Bottom label
    let label_rect = Rect::from_min_size(
        Pos2::new(rect.center().x - 70.0, y + 12.0),
        Vec2::new(140.0, 14.0),
    );
    painter.rect_filled(label_rect, 4.0, theme.bg_elevated().gamma_multiply(0.35));

    // ── Single shimmer sweep across the whole area ──
    let content_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 12.0, rect.min.y + 16.0),
        Vec2::new(rect.width() - 24.0, (y + 26.0) - (rect.min.y + 16.0)),
    );
    draw_shimmer_bar(painter, content_rect, time, theme);

    ui.ctx().request_repaint();
}

/// Draw a single shimmer bar that sweeps horizontally across `rect`.
///
/// The bar is a soft accent-colored rectangle that fades in from the left,
/// travels across, and fades out to the right. Only one bar is drawn,
/// preventing the overlapping-lines artifact.
fn draw_shimmer_bar(painter: &egui::Painter, rect: Rect, time: f32, theme: crate::ui::theme::AppTheme) {
    // Cycle: 2 seconds total (1.5s visible sweep + 0.5s gap)
    let cycle = 2.0;
    let t = time % cycle;

    // Only show shimmer during the first 1.5s of the cycle
    if t > 1.5 {
        return;
    }

    let progress = t / 1.5; // 0..1

    let bar_width = rect.width() * 0.4;
    let center_x = rect.min.x + progress * (rect.width() + bar_width) - bar_width * 0.5;

    // Clamp to visible area
    let left = (center_x - bar_width * 0.5).max(rect.min.x);
    let right = (center_x + bar_width * 0.5).min(rect.max.x);

    if right <= left {
        return;
    }

    let accent = theme.accent_primary();

    // Use a simple filled rect with low opacity — no mesh, no overlapping.
    // The smooth movement creates the shimmer illusion.
    let bar_rect = Rect::from_min_size(
        Pos2::new(left, rect.min.y),
        Vec2::new(right - left, rect.height()),
    );

    // Fade the bar at the edges for a softer look
    let edge_fade = 12.0f32;
    let fade_left = (center_x - bar_width * 0.5 - rect.min.x).clamp(0.0, edge_fade) / edge_fade;
    let fade_right = (rect.max.x - (center_x + bar_width * 0.5)).clamp(0.0, edge_fade) / edge_fade;
    let fade = fade_left.min(fade_right);

    let alpha = (0.06 + fade * 0.04).min(0.10);
    let shimmer_color = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), (alpha * 255.0) as u8);

    painter.rect_filled(bar_rect, 0.0, shimmer_color);
}
