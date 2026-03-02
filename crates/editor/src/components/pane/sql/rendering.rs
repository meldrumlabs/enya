//! Shared rendering helpers for the SQL pane.
//!
//! Used by both `native/query_card.rs` and `stub/snapshot_card.rs` to avoid
//! duplicating table layout code. All data is pre-formatted as strings at
//! the call site, so this module has no dependency on `enya_datafusion`.

use egui::Color32;

use crate::components::OverlayColors;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Format a number with comma separators (e.g. 1234567 → "1,234,567").
pub(crate) fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Render a stats bar container with right-to-left badge layout.
///
/// Provides the standard Frame + horizontal + right-to-left layout used by
/// benchmark and describe stats bars. The closure receives the `Ui` and
/// `OverlayColors` to add badges.
pub(crate) fn render_stats_bar_frame(
    ui: &mut egui::Ui,
    theme: AppTheme,
    add_badges: impl FnOnce(&mut egui::Ui, &OverlayColors),
) {
    let colors = OverlayColors::new(theme);

    egui::Frame::new()
        .fill(theme.bg_surface())
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    add_badges(ui, &colors);
                });
            });
        });
}

// ---------------------------------------------------------------------------
// Benchmark phase timing table
// ---------------------------------------------------------------------------

/// A pre-formatted phase row for the benchmark table.
pub(crate) struct PhaseRow<'a> {
    /// Phase name (e.g. "Logical Planning").
    pub name: &'a str,
    /// Pre-formatted duration values: [Min, Median, Mean, Max].
    pub values: [String; 4],
    /// Percentage of total. `None` suppresses the "%" column (for the "Total" row).
    pub percent: Option<f64>,
}

/// Render a benchmark phase timing table with header, separator, and alternating rows.
pub(crate) fn render_phase_table(ui: &mut egui::Ui, rows: &[PhaseRow<'_>], theme: AppTheme) {
    let colors = OverlayColors::new(theme);
    let col_width = 90.0;
    let phase_col_width = 130.0;
    let pct_col_width = 60.0;
    let row_height = typography::SM + 10.0;

    egui::Frame::new()
        .fill(theme.bg_base())
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;

            // Header row
            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing.x = 0.0;

                let (phase_rect, _) = ui.allocate_exact_size(
                    egui::vec2(phase_col_width, row_height),
                    egui::Sense::hover(),
                );
                ui.painter().text(
                    phase_rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    "Phase",
                    typography::monospace(typography::SM),
                    colors.accent,
                );

                for label in &["Min", "Median", "Mean", "Max"] {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(col_width, row_height),
                        egui::Sense::hover(),
                    );
                    ui.painter().text(
                        rect.right_center() + egui::vec2(-4.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        *label,
                        typography::monospace(typography::SM),
                        colors.accent,
                    );
                }

                // Percentage column header
                let (pct_rect, _) = ui.allocate_exact_size(
                    egui::vec2(pct_col_width, row_height),
                    egui::Sense::hover(),
                );
                ui.painter().text(
                    pct_rect.right_center() + egui::vec2(-4.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    "%",
                    typography::monospace(typography::SM),
                    colors.faint_text,
                );
            });

            // Separator line
            let separator_rect = ui.available_rect_before_wrap();
            ui.painter().hline(
                separator_rect.left()
                    ..=separator_rect.left() + phase_col_width + col_width * 4.0 + pct_col_width,
                ui.cursor().top(),
                egui::Stroke::new(1.0, colors.separator),
            );
            ui.add_space(2.0);

            // Phase rows
            for (idx, row) in rows.iter().enumerate() {
                let row_bg = if idx % 2 == 1 {
                    theme.bg_hover().gamma_multiply(0.3)
                } else {
                    Color32::TRANSPARENT
                };

                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 0.0;

                    // Phase name
                    let (phase_rect, _) = ui.allocate_exact_size(
                        egui::vec2(phase_col_width, row_height),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(phase_rect, 0.0, row_bg);
                    ui.painter().text(
                        phase_rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        row.name,
                        typography::monospace(typography::SM),
                        colors.text,
                    );

                    // Duration values: Min, Median, Mean, Max
                    for val in &row.values {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(col_width, row_height),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 0.0, row_bg);
                        ui.painter().text(
                            rect.right_center() + egui::vec2(-4.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            val,
                            typography::monospace(typography::SM),
                            colors.muted_text,
                        );
                    }

                    // Percentage column
                    let (pct_rect, _) = ui.allocate_exact_size(
                        egui::vec2(pct_col_width, row_height),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(pct_rect, 0.0, row_bg);
                    if let Some(pct) = row.percent {
                        ui.painter().text(
                            pct_rect.right_center() + egui::vec2(-4.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            format!("({pct:.1}%)"),
                            typography::monospace(typography::SM),
                            colors.faint_text,
                        );
                    }
                });
            }
        });
}

// ---------------------------------------------------------------------------
// Describe column stats table
// ---------------------------------------------------------------------------

/// A pre-formatted column row for the describe stats table.
pub(crate) struct ColumnRow<'a> {
    pub name: &'a str,
    pub data_type: &'a str,
    pub count: String,
    pub null_count: String,
    pub distinct_count: String,
    pub min: Option<&'a str>,
    pub max: Option<&'a str>,
    pub mean: Option<f64>,
}

/// Render a column statistics table with header, separator, and alternating rows.
pub(crate) fn render_column_stats_table(
    ui: &mut egui::Ui,
    rows: &[ColumnRow<'_>],
    theme: AppTheme,
) {
    let colors = OverlayColors::new(theme);
    let name_col = 140.0;
    let type_col = 100.0;
    let num_col = 80.0;
    let str_col = 100.0;
    let mean_col = 80.0;
    let row_height = typography::SM + 10.0;

    egui::Frame::new()
        .fill(theme.bg_base())
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;

            // Header row
            let headers: &[(&str, f32, egui::Align2)] = &[
                ("Column", name_col, egui::Align2::LEFT_CENTER),
                ("Type", type_col, egui::Align2::LEFT_CENTER),
                ("Count", num_col, egui::Align2::RIGHT_CENTER),
                ("Nulls", num_col, egui::Align2::RIGHT_CENTER),
                ("Distinct", num_col, egui::Align2::RIGHT_CENTER),
                ("Min", str_col, egui::Align2::RIGHT_CENTER),
                ("Max", str_col, egui::Align2::RIGHT_CENTER),
                ("Mean", mean_col, egui::Align2::RIGHT_CENTER),
            ];

            ui.horizontal(|ui| {
                ui.style_mut().spacing.item_spacing.x = 0.0;
                for (label, width, align) in headers {
                    let (rect, _) = ui
                        .allocate_exact_size(egui::vec2(*width, row_height), egui::Sense::hover());
                    let pos = if *align == egui::Align2::LEFT_CENTER {
                        rect.left_center()
                    } else {
                        rect.right_center() + egui::vec2(-4.0, 0.0)
                    };
                    ui.painter().text(
                        pos,
                        *align,
                        *label,
                        typography::monospace(typography::SM),
                        colors.accent,
                    );
                }
            });

            // Separator
            let sep_rect = ui.available_rect_before_wrap();
            let total_width = name_col + type_col + num_col * 3.0 + str_col * 2.0 + mean_col;
            ui.painter().hline(
                sep_rect.left()..=sep_rect.left() + total_width,
                ui.cursor().top(),
                egui::Stroke::new(1.0, colors.separator),
            );
            ui.add_space(2.0);

            // Data rows
            for (idx, row) in rows.iter().enumerate() {
                let row_bg = if idx % 2 == 1 {
                    theme.bg_hover().gamma_multiply(0.3)
                } else {
                    Color32::TRANSPARENT
                };

                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 0.0;

                    // Column name (left-aligned)
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(name_col, row_height),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 0.0, row_bg);
                    ui.painter().text(
                        rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        row.name,
                        typography::monospace(typography::SM),
                        colors.text,
                    );

                    // Type (left-aligned)
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(type_col, row_height),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 0.0, row_bg);
                    ui.painter().text(
                        rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        row.data_type,
                        typography::monospace(typography::SM),
                        colors.muted_text,
                    );

                    // Count, Nulls, Distinct (right-aligned numbers)
                    for val in [&row.count, &row.null_count, &row.distinct_count] {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(num_col, row_height),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 0.0, row_bg);
                        ui.painter().text(
                            rect.right_center() + egui::vec2(-4.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            val,
                            typography::monospace(typography::SM),
                            colors.muted_text,
                        );
                    }

                    // Min, Max (right-aligned strings, truncated)
                    for val in [row.min, row.max] {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(str_col, row_height),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 0.0, row_bg);
                        let display = val.unwrap_or("--");
                        let display = if display.len() > 12 {
                            &display[..12]
                        } else {
                            display
                        };
                        ui.painter().text(
                            rect.right_center() + egui::vec2(-4.0, 0.0),
                            egui::Align2::RIGHT_CENTER,
                            display,
                            typography::monospace(typography::SM),
                            colors.muted_text,
                        );
                    }

                    // Mean (right-aligned, "--" for non-numeric)
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(mean_col, row_height),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(rect, 0.0, row_bg);
                    let mean_str = match row.mean {
                        Some(m) => format!("{m:.2}"),
                        None => "--".to_string(),
                    };
                    ui.painter().text(
                        rect.right_center() + egui::vec2(-4.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        mean_str,
                        typography::monospace(typography::SM),
                        colors.muted_text,
                    );
                });
            }
        });
}
