//! Premium loading skeleton for PR review panes.
//!
//! Draws shimmer placeholder bars that mimic the shape of PR content
//! (list rows or detail sections) with a sweeping gradient highlight.

use egui::{Color32, Mesh, Pos2, Rect, Vec2};
use egui::epaint::Vertex;

/// Render a premium shimmer skeleton for the PR list loading state.
///
/// Draws 5-6 placeholder rows with a sweeping horizontal gradient that
/// travels left-to-right, simulating content being "scanned".
pub fn render_pr_list_skeleton(ui: &mut egui::Ui, theme: crate::ui::theme::AppTheme) {
    let time = ui.ctx().input(|i| i.time) as f32;
    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter();

    let row_height = 48.0;
    let gap = 8.0;
    let avatar_size = 28.0;
    let max_rows = ((rect.height() - 60.0) / (row_height + gap)).max(3.0).min(6.0) as usize;

    // Shimmer phase: sweeps across every 1.5 seconds
    let shimmer_x = (time % 1.5) / 1.5;

    for i in 0..max_rows {
        let y = rect.min.y + 20.0 + i as f32 * (row_height + gap);
        let row_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 12.0, y),
            Vec2::new(rect.width() - 24.0, row_height),
        );

        // Background pill
        let bg = theme.bg_elevated().gamma_multiply(0.5);
        painter.rect_filled(row_rect, 6.0, bg);

        // Avatar circle placeholder
        let avatar_center = Pos2::new(row_rect.min.x + 20.0, row_rect.center().y);
        painter.circle_filled(avatar_center, avatar_size / 2.0, bg);

        // Title bar placeholder (varying widths for realism)
        let title_width = rect.width() * (0.45 + (i as f32 * 0.08).sin() * 0.15);
        let title_rect = Rect::from_min_size(
            Pos2::new(row_rect.min.x + 44.0, row_rect.min.y + 10.0),
            Vec2::new(title_width, 12.0),
        );
        painter.rect_filled(title_rect, 4.0, bg);

        // Subtitle bar placeholder
        let subtitle_width = rect.width() * (0.25 + (i as f32 * 0.12).cos() * 0.1);
        let subtitle_rect = Rect::from_min_size(
            Pos2::new(row_rect.min.x + 44.0, row_rect.min.y + 28.0),
            Vec2::new(subtitle_width, 8.0),
        );
        painter.rect_filled(subtitle_rect, 4.0, bg);

        // Shimmer gradient sweep across the row
        draw_shimmer_band(painter, row_rect, shimmer_x, theme);
    }

    // Bottom label
    let label_rect = Rect::from_min_size(
        Pos2::new(rect.center().x - 80.0, rect.min.y + 20.0 + max_rows as f32 * (row_height + gap) + 16.0),
        Vec2::new(160.0, 16.0),
    );
    let label_bg = theme.bg_elevated().gamma_multiply(0.4);
    painter.rect_filled(label_rect, 4.0, label_bg);
    draw_shimmer_band(painter, label_rect, shimmer_x, theme);
}

/// Render a premium shimmer skeleton for the PR detail loading state.
///
/// Mimics the detail view layout: title area, description blocks,
/// file diff list, and sidebar stats.
pub fn render_pr_detail_skeleton(ui: &mut egui::Ui, theme: crate::ui::theme::AppTheme) {
    let time = ui.ctx().input(|i| i.time) as f32;
    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter();

    let shimmer_x = (time % 1.5) / 1.5;

    // ── Title area ──
    let title_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 12.0, rect.min.y + 16.0),
        Vec2::new(rect.width() * 0.6, 18.0),
    );
    let bg = theme.bg_elevated().gamma_multiply(0.5);
    painter.rect_filled(title_rect, 4.0, bg);
    draw_shimmer_band(painter, title_rect, shimmer_x, theme);

    // Subtitle line
    let subtitle_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 12.0, rect.min.y + 42.0),
        Vec2::new(rect.width() * 0.35, 10.0),
    );
    painter.rect_filled(subtitle_rect, 4.0, bg);
    draw_shimmer_band(painter, subtitle_rect, shimmer_x, theme);

    // ── Description blocks ──
    let mut y = rect.min.y + 72.0;
    for i in 0..3 {
        let line_width = rect.width() * (0.9 - (i as f32 * 0.15));
        let line_rect = Rect::from_min_size(
            Pos2::new(rect.min.x + 12.0, y),
            Vec2::new(line_width, 10.0),
        );
        painter.rect_filled(line_rect, 4.0, bg);
        draw_shimmer_band(painter, line_rect, shimmer_x, theme);
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

        // File icon placeholder
        let icon_rect = Rect::from_min_size(
            Pos2::new(row_rect.min.x + 10.0, row_rect.center().y - 6.0),
            Vec2::new(12.0, 12.0),
        );
        painter.rect_filled(icon_rect, 2.0, bg);

        // Filename placeholder
        let name_width = rect.width() * (0.4 + (i as f32 * 0.1).sin() * 0.1);
        let name_rect = Rect::from_min_size(
            Pos2::new(row_rect.min.x + 32.0, row_rect.center().y - 5.0),
            Vec2::new(name_width, 10.0),
        );
        painter.rect_filled(name_rect, 4.0, bg);

        // Diff stat placeholder
        let stat_rect = Rect::from_min_size(
            Pos2::new(row_rect.max.x - 80.0, row_rect.center().y - 4.0),
            Vec2::new(60.0, 8.0),
        );
        painter.rect_filled(stat_rect, 4.0, bg);

        draw_shimmer_band(painter, row_rect, shimmer_x, theme);
        y += 40.0;
    }

    // ── Bottom label ──
    let label_rect = Rect::from_min_size(
        Pos2::new(rect.center().x - 70.0, y + 12.0),
        Vec2::new(140.0, 14.0),
    );
    let label_bg = theme.bg_elevated().gamma_multiply(0.4);
    painter.rect_filled(label_rect, 4.0, label_bg);
    draw_shimmer_band(painter, label_rect, shimmer_x, theme);
}

/// Draw a single horizontal shimmer band that sweeps across `rect`.
///
/// `phase` is 0..1 representing the shimmer's center position across the rect.
fn draw_shimmer_band(painter: &egui::Painter, rect: Rect, phase: f32, theme: crate::ui::theme::AppTheme) {
    let band_width = rect.width() * 0.35;
    let center_x = rect.min.x + phase * (rect.width() + band_width) - band_width * 0.5;

    // Only draw if the band overlaps the rect
    if center_x + band_width * 0.5 < rect.min.x || center_x - band_width * 0.5 > rect.max.x {
        return;
    }

    let accent = theme.accent_primary();
    let base = theme.bg_elevated().gamma_multiply(0.5);

    // Build a gradient mesh: transparent -> accent tint -> transparent
    let mut mesh = Mesh::default();
    let left = (center_x - band_width * 0.5).max(rect.min.x);
    let right = (center_x + band_width * 0.5).min(rect.max.x);

    let tl = mesh.vertices.len() as u32;

    // Left edge (transparent)
    mesh.vertices.push(Vertex {
        pos: Pos2::new(left, rect.min.y),
        uv: egui::epaint::WHITE_UV,
        color: Color32::TRANSPARENT,
    });
    mesh.vertices.push(Vertex {
        pos: Pos2::new(left, rect.max.y),
        uv: egui::epaint::WHITE_UV,
        color: Color32::TRANSPARENT,
    });

    // Center (accent tint at ~15% opacity)
    let center_color = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 35);
    mesh.vertices.push(Vertex {
        pos: Pos2::new((left + right) / 2.0, rect.min.y),
        uv: egui::epaint::WHITE_UV,
        color: center_color,
    });
    mesh.vertices.push(Vertex {
        pos: Pos2::new((left + right) / 2.0, rect.max.y),
        uv: egui::epaint::WHITE_UV,
        color: center_color,
    });

    // Right edge (transparent)
    mesh.vertices.push(Vertex {
        pos: Pos2::new(right, rect.min.y),
        uv: egui::epaint::WHITE_UV,
        color: Color32::TRANSPARENT,
    });
    mesh.vertices.push(Vertex {
        pos: Pos2::new(right, rect.max.y),
        uv: egui::epaint::WHITE_UV,
        color: Color32::TRANSPARENT,
    });

    // Two quads: left half and right half of the gradient
    mesh.indices.extend_from_slice(&[tl, tl + 2, tl + 3, tl, tl + 3, tl + 1]);
    mesh.indices.extend_from_slice(&[tl + 2, tl + 4, tl + 5, tl + 2, tl + 5, tl + 3]);

    painter.add(egui::Shape::mesh(mesh));
}
