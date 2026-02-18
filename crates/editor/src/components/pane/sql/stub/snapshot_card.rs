//! Snapshot card rendering for the WASM SQL stub.
//!
//! Mirrors the native `query_card.rs` but operates on `SnapshotQueryCell`
//! string data instead of Arrow `RecordBatch`es. Values are already
//! pre-stringified, so no `format_array_value` calls are needed.

use egui::{Color32, RichText};
use enya_config::SnapshotQueryCell;

use super::{CellTab, CellViewState, format_ms, snapshot_plan};
use crate::components::OverlayColors;
use crate::components::util::{render_stat_badge, render_stat_badge_with_icon};
use crate::ui::semantic_icons::{nav, status, time};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Actions returned by card rendering.
pub(super) enum CardAction {
    Select,
    Expand,
    Collapse,
    SetTab(CellTab),
    NextPage,
    PrevPage,
}

const ROWS_PER_PAGE: usize = 50;

/// Entry point: renders a snapshot query card (collapsed or expanded).
pub(super) fn render_snapshot_card(
    ui: &mut egui::Ui,
    cell: &SnapshotQueryCell,
    cell_idx: usize,
    view_state: &mut CellViewState,
    theme: AppTheme,
    is_selected: bool,
    cell_number: usize,
) -> Vec<CardAction> {
    if view_state.expanded {
        render_expanded_card(ui, cell, cell_idx, view_state, theme, cell_number)
    } else {
        render_collapsed_card(ui, cell, cell_idx, theme, is_selected, cell_number)
    }
}

/// Render a collapsed card: status icon + SQL preview + stats.
fn render_collapsed_card(
    ui: &mut egui::Ui,
    cell: &SnapshotQueryCell,
    cell_idx: usize,
    theme: AppTheme,
    is_selected: bool,
    cell_number: usize,
) -> Vec<CardAction> {
    let mut actions = Vec::new();
    let text_primary = theme.text_primary();
    let text_secondary = theme.text_secondary();
    let accent = theme.accent_primary();
    let max_preview_rows = 3;
    let max_value_len = 16;

    let border_color = if is_selected {
        accent
    } else {
        theme.border_default()
    };
    let border_width = if is_selected { 2.0 } else { 1.0 };

    let card_response = egui::Frame::new()
        .fill(theme.bg_elevated())
        .stroke(egui::Stroke::new(border_width, border_color))
        .corner_radius(8.0)
        .inner_margin(0.0)
        .show(ui, |ui| {
            // Header
            egui::Frame::new()
                .fill(theme.bg_surface())
                .inner_margin(egui::Margin::symmetric(12, 8))
                .corner_radius(egui::CornerRadius {
                    nw: 8,
                    ne: 8,
                    sw: 0,
                    se: 0,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Cell number
                        ui.label(
                            RichText::new(format!("[{cell_number}]"))
                                .color(text_secondary.gamma_multiply(0.4))
                                .size(10.0)
                                .monospace(),
                        );
                        ui.add_space(4.0);

                        // Status icon
                        if cell.error.is_some() {
                            ui.label(
                                RichText::new(status::ERROR)
                                    .color(theme.semantic_error())
                                    .size(11.0),
                            );
                        } else {
                            ui.label(
                                RichText::new(status::SUCCESS)
                                    .color(theme.semantic_success())
                                    .size(11.0),
                            );
                        }
                        ui.add_space(6.0);

                        // SQL preview (truncated to 1 line)
                        let sql_oneline = cell.sql.replace('\n', " ");
                        let sql_display = if sql_oneline.len() > 60 {
                            format!("{}…", &sql_oneline[..59])
                        } else {
                            sql_oneline
                        };
                        ui.label(
                            RichText::new(sql_display)
                                .color(text_primary)
                                .size(11.0)
                                .monospace(),
                        );

                        // Right side: stats + chevron
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(nav::FORWARD)
                                    .color(text_secondary.gamma_multiply(0.5))
                                    .size(11.0),
                            );
                            ui.add_space(8.0);

                            // Execution time
                            if let Some(stats) = &cell.stats {
                                ui.label(
                                    RichText::new(format!("{}ms", stats.total_time_ms))
                                        .color(text_secondary)
                                        .size(10.0),
                                );
                                ui.add_space(4.0);
                            }

                            // Row count
                            if cell.error.is_none() {
                                ui.label(
                                    RichText::new(format!("{} rows", cell.total_rows))
                                        .color(text_secondary)
                                        .size(10.0),
                                );
                            }
                        });
                    });
                });

            // Error message if failed
            if let Some(error) = &cell.error {
                egui::Frame::new()
                    .fill(theme.semantic_error().gamma_multiply(0.1))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        let display_error = if error.len() > 120 {
                            format!("{}...", &error[..120])
                        } else {
                            error.clone()
                        };
                        ui.label(
                            RichText::new(display_error)
                                .color(theme.semantic_error())
                                .size(11.0)
                                .monospace(),
                        );
                    });
            }

            // Compact table preview (3 rows)
            if !cell.rows.is_empty() && !cell.columns.is_empty() {
                render_compact_table_preview(
                    ui,
                    cell,
                    max_preview_rows,
                    max_value_len,
                    text_primary,
                    text_secondary,
                );
            }

            // Bottom bar
            egui::Frame::new()
                .fill(theme.bg_surface())
                .inner_margin(egui::Margin::symmetric(12, 4))
                .corner_radius(egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 8,
                    se: 8,
                })
                .show(ui, |_ui| {});
        });

    // Click to select, double-click to expand
    let card_rect = card_response.response.rect;
    let click_response = ui.interact(
        card_rect,
        egui::Id::new(("snapshot_card_click", cell_idx)),
        egui::Sense::click(),
    );
    if click_response.double_clicked() {
        actions.push(CardAction::Expand);
    } else if click_response.clicked() {
        actions.push(CardAction::Select);
    }
    if click_response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    actions
}

/// Render a compact table preview (used in collapsed cards).
fn render_compact_table_preview(
    ui: &mut egui::Ui,
    cell: &SnapshotQueryCell,
    max_rows: usize,
    max_value_len: usize,
    text_primary: Color32,
    text_secondary: Color32,
) {
    let total_cols = cell.columns.len();
    let available_width = ui.available_width() - 24.0;
    let col_spacing = 16.0;
    let char_width = 6.5;
    let overflow_indicator_width = 40.0;

    let col_widths: Vec<f32> = cell
        .columns
        .iter()
        .map(|c| {
            let name_len = c.name.len().min(max_value_len);
            (name_len as f32 * char_width).max(40.0)
        })
        .collect();

    let mut total_width = 0.0;
    let mut show_cols = 0;
    for (i, &width) in col_widths.iter().enumerate() {
        let needed = if i == 0 { width } else { col_spacing + width };
        let reserve = if i + 1 < total_cols {
            overflow_indicator_width
        } else {
            0.0
        };
        if total_width + needed + reserve <= available_width {
            total_width += needed;
            show_cols = i + 1;
        } else {
            break;
        }
    }
    show_cols = show_cols.max(1);

    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            // Column headers
            ui.horizontal(|ui| {
                for (col_idx, col) in cell.columns.iter().take(show_cols).enumerate() {
                    if col_idx > 0 {
                        ui.add_space(col_spacing);
                    }
                    let name = &col.name;
                    let display_name = if name.len() > max_value_len {
                        format!("{}…", &name[..max_value_len - 1])
                    } else {
                        name.to_string()
                    };
                    ui.label(
                        RichText::new(display_name)
                            .color(text_primary)
                            .size(10.0)
                            .strong()
                            .monospace(),
                    );
                }
                if total_cols > show_cols {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!("+{}", total_cols - show_cols))
                            .color(text_secondary.gamma_multiply(0.5))
                            .size(10.0),
                    );
                }
            });

            ui.add_space(2.0);

            // Data rows
            for row in cell.rows.iter().take(max_rows) {
                ui.horizontal(|ui| {
                    for (col_idx, value) in row.iter().enumerate().take(show_cols) {
                        if col_idx > 0 {
                            ui.add_space(col_spacing);
                        }
                        let (display_val, color) = if value == "NULL" {
                            ("null".to_string(), text_secondary.gamma_multiply(0.4))
                        } else if value.len() > max_value_len {
                            (format!("{}…", &value[..max_value_len - 1]), text_secondary)
                        } else {
                            (value.clone(), text_secondary)
                        };
                        ui.label(
                            RichText::new(display_val)
                                .color(color)
                                .size(10.0)
                                .monospace(),
                        );
                    }
                });
            }

            // "More rows" indicator
            if cell.rows.len() > max_rows {
                ui.label(
                    RichText::new(format!("… {} more", cell.rows.len() - max_rows))
                        .color(text_secondary.gamma_multiply(0.5))
                        .size(10.0)
                        .italics(),
                );
            }
        });
}

/// Render an expanded card: full SQL + tab bar + inline content + footer.
fn render_expanded_card(
    ui: &mut egui::Ui,
    cell: &SnapshotQueryCell,
    cell_idx: usize,
    view_state: &mut CellViewState,
    theme: AppTheme,
    cell_number: usize,
) -> Vec<CardAction> {
    let mut actions = Vec::new();
    let colors = OverlayColors::new(theme);
    let text_primary = theme.text_primary();
    let text_secondary = theme.text_secondary();
    let accent = theme.accent_primary();

    // Clamp page
    let total_pages = (cell.rows.len().div_ceil(ROWS_PER_PAGE)).max(1);
    if view_state.table_page >= total_pages {
        view_state.table_page = total_pages - 1;
    }

    egui::Frame::new()
        .fill(theme.bg_elevated())
        .stroke(egui::Stroke::new(1.5, accent.gamma_multiply(0.5)))
        .corner_radius(8.0)
        .inner_margin(0.0)
        .show(ui, |ui| {
            // Header: full SQL + collapse chevron
            egui::Frame::new()
                .fill(theme.bg_surface())
                .inner_margin(egui::Margin::symmetric(12, 8))
                .corner_radius(egui::CornerRadius {
                    nw: 8,
                    ne: 8,
                    sw: 0,
                    se: 0,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Cell number
                        ui.label(
                            RichText::new(format!("[{cell_number}]"))
                                .color(text_secondary.gamma_multiply(0.4))
                                .size(10.0)
                                .monospace(),
                        );
                        ui.add_space(4.0);

                        // Status icon
                        if cell.error.is_some() {
                            ui.label(
                                RichText::new(status::ERROR)
                                    .color(theme.semantic_error())
                                    .size(11.0),
                            );
                        } else {
                            ui.label(
                                RichText::new(status::SUCCESS)
                                    .color(theme.semantic_success())
                                    .size(11.0),
                            );
                        }
                        ui.add_space(6.0);

                        // Full SQL text
                        ui.label(
                            RichText::new(&cell.sql)
                                .color(text_primary)
                                .size(11.0)
                                .monospace(),
                        );

                        // Collapse chevron
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let chevron_resp = ui.add(
                                egui::Label::new(
                                    RichText::new(nav::COLLAPSE)
                                        .color(text_secondary.gamma_multiply(0.6))
                                        .size(11.0),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if chevron_resp.clicked() {
                                actions.push(CardAction::Collapse);
                            }
                            if chevron_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        });
                    });
                });

            // Error display
            if let Some(error) = &cell.error {
                egui::Frame::new()
                    .fill(theme.semantic_error().gamma_multiply(0.1))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(error.as_str())
                                .color(theme.semantic_error())
                                .size(11.0)
                                .monospace(),
                        );
                    });
            }

            // Tab bar + stats
            if cell.error.is_none() {
                render_tab_bar(ui, cell, view_state, theme, &mut actions);
            }

            // Separator
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, colors.separator),
            );

            // Content area
            match view_state.active_tab {
                CellTab::Table => {
                    if !cell.rows.is_empty() && !cell.columns.is_empty() {
                        render_inline_table(ui, cell, cell_idx, view_state, theme);
                    } else if cell.rows.is_empty() && cell.error.is_none() {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("Query returned no rows")
                                    .color(text_secondary)
                                    .size(11.0),
                            );
                        });
                        ui.add_space(16.0);
                    }
                }
                CellTab::Plan => {
                    render_inline_plan(ui, cell, theme);
                }
            }

            // Footer
            render_card_footer(ui, cell, view_state, &colors, &mut actions);
        });

    actions
}

/// Render the [Table] [Plan] tab bar with stat badges.
fn render_tab_bar(
    ui: &mut egui::Ui,
    cell: &SnapshotQueryCell,
    view_state: &mut CellViewState,
    theme: AppTheme,
    actions: &mut Vec<CardAction>,
) {
    let colors = OverlayColors::new(theme);
    let accent = theme.accent_primary();
    let text_secondary = theme.text_secondary();

    egui::Frame::new()
        .fill(theme.bg_surface())
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Table tab
                let table_active = view_state.active_tab == CellTab::Table;
                let table_color = if table_active { accent } else { text_secondary };
                let table_resp = ui.add(
                    egui::Label::new(
                        RichText::new("Table")
                            .color(table_color)
                            .size(11.0)
                            .strong(),
                    )
                    .sense(egui::Sense::click()),
                );
                if table_resp.clicked() && !table_active {
                    actions.push(CardAction::SetTab(CellTab::Table));
                }
                if table_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if table_active {
                    let rect = table_resp.rect;
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom() + 1.0,
                        egui::Stroke::new(2.0, accent),
                    );
                }

                ui.add_space(12.0);

                // Plan tab (only if plan data exists)
                if cell.plan.is_some() {
                    let plan_active = view_state.active_tab == CellTab::Plan;
                    let plan_color = if plan_active { accent } else { text_secondary };
                    let plan_resp = ui.add(
                        egui::Label::new(
                            RichText::new("Plan").color(plan_color).size(11.0).strong(),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if plan_resp.clicked() && !plan_active {
                        actions.push(CardAction::SetTab(CellTab::Plan));
                    }
                    if plan_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if plan_active {
                        let rect = plan_resp.rect;
                        ui.painter().hline(
                            rect.x_range(),
                            rect.bottom() + 1.0,
                            egui::Stroke::new(2.0, accent),
                        );
                    }
                }

                // Stats on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(stats) = &cell.stats {
                        render_stat_badge_with_icon(
                            ui,
                            time::TIMER,
                            &format_ms(stats.total_time_ms),
                            &colors,
                        );
                        ui.add_space(4.0);
                    }

                    render_stat_badge(ui, &format!("{} rows", cell.total_rows), &colors);

                    ui.add_space(4.0);
                    render_stat_badge(ui, &format!("{} cols", cell.columns.len()), &colors);
                });
            });
        });
}

/// Render the inline table view with pagination.
fn render_inline_table(
    ui: &mut egui::Ui,
    cell: &SnapshotQueryCell,
    cell_idx: usize,
    view_state: &CellViewState,
    theme: AppTheme,
) {
    let colors = OverlayColors::new(theme);
    let bg_surface = theme.bg_surface();

    let num_cols = cell.columns.len();

    // Column widths
    let column_widths: Vec<f32> = cell
        .columns
        .iter()
        .map(|col| {
            let name_width = col.name.len() as f32 * 7.0;
            let type_width = col.data_type.len() as f32 * 6.0;
            name_width.max(type_width).clamp(80.0, 200.0)
        })
        .collect();

    let header_height = typography::SM + typography::XS + 8.0;
    let row_height = typography::SM + 8.0;
    let start_row = view_state.table_page * ROWS_PER_PAGE;
    let max_row_num = (view_state.table_page + 1) * ROWS_PER_PAGE;
    let row_num_width = max_row_num.to_string().len().max(3);
    let row_num_gutter_width = (row_num_width + 2) as f32 * 8.0;

    let col_spacing = 16.0;

    egui::Frame::new()
        .fill(theme.bg_base())
        .inner_margin(0.0)
        .show(ui, |ui| {
            let h_sync_id = egui::Id::new(("snapshot_table_h_sync", cell_idx));

            let stored_h_offset = ui
                .ctx()
                .memory(|m| m.data.get_temp::<f32>(h_sync_id))
                .unwrap_or(0.0);

            // Sticky column headers
            egui::ScrollArea::horizontal()
                .id_salt(("snapshot_table_header", cell_idx))
                .scroll_offset(egui::vec2(stored_h_offset, 0.0))
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;

                    ui.horizontal(|ui| {
                        ui.style_mut().spacing.item_spacing.x = 0.0;

                        // Row number gutter
                        let (gutter_rect, _) = ui.allocate_exact_size(
                            egui::vec2(row_num_gutter_width, header_height),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(gutter_rect, 0.0, theme.bg_base());
                        ui.painter().text(
                            gutter_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "#",
                            typography::monospace(typography::XS),
                            colors.faint_text,
                        );

                        for (idx, col) in cell.columns.iter().enumerate() {
                            let col_width = column_widths.get(idx).copied().unwrap_or(100.0);

                            let (col_rect, _) = ui.allocate_exact_size(
                                egui::vec2(col_width + col_spacing, header_height),
                                egui::Sense::hover(),
                            );

                            ui.painter().rect_filled(col_rect, 0.0, bg_surface);

                            ui.painter().text(
                                col_rect.left_center() + egui::vec2(8.0, -6.0),
                                egui::Align2::LEFT_CENTER,
                                &col.name,
                                typography::monospace(typography::SM),
                                colors.text,
                            );

                            ui.painter().text(
                                col_rect.left_center() + egui::vec2(8.0, 6.0),
                                egui::Align2::LEFT_CENTER,
                                &col.data_type,
                                typography::monospace(typography::XS),
                                colors.faint_text,
                            );
                        }
                    });
                });

            // Scrollable data body
            let body_max_height = (400.0 - header_height).max(100.0);
            let body_scroll_output = egui::ScrollArea::both()
                .id_salt(("snapshot_table_body", cell_idx))
                .scroll_offset(egui::vec2(stored_h_offset, 0.0))
                .max_height(body_max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;

                    let end_row = (start_row + ROWS_PER_PAGE).min(cell.rows.len());

                    for (display_idx, row) in cell.rows[start_row..end_row].iter().enumerate() {
                        let absolute_row = start_row + display_idx + 1;

                        let row_bg = if display_idx % 2 == 0 {
                            Color32::TRANSPARENT
                        } else {
                            theme.bg_hover().gamma_multiply(0.3)
                        };

                        ui.horizontal(|ui| {
                            ui.style_mut().spacing.item_spacing.x = 0.0;

                            // Row number gutter
                            let (gutter_rect, _) = ui.allocate_exact_size(
                                egui::vec2(row_num_gutter_width, row_height),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(gutter_rect, 0.0, theme.bg_base());
                            let row_num_str = format!("{absolute_row:>row_num_width$}");
                            ui.painter().text(
                                gutter_rect.left_center() + egui::vec2(8.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                row_num_str,
                                typography::monospace(typography::XS),
                                colors.faint_text,
                            );

                            // Cell values
                            for (col_idx, value) in row.iter().enumerate().take(num_cols) {
                                let col_width =
                                    column_widths.get(col_idx).copied().unwrap_or(100.0);

                                let (cell_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(col_width + col_spacing, row_height),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(cell_rect, 0.0, row_bg);

                                if value == "NULL" {
                                    let null_bg = colors.faint_text.gamma_multiply(0.06);
                                    ui.painter().rect_filled(cell_rect, 0.0, null_bg);
                                    let job = egui::text::LayoutJob::single_section(
                                        "null".to_string(),
                                        egui::TextFormat {
                                            font_id: typography::monospace(typography::SM),
                                            color: colors.faint_text,
                                            italics: true,
                                            ..Default::default()
                                        },
                                    );
                                    let galley = ui.fonts_mut(|f| f.layout_job(job));
                                    ui.painter().galley(
                                        cell_rect.left_center()
                                            + egui::vec2(8.0, -galley.size().y / 2.0),
                                        galley,
                                        colors.faint_text,
                                    );
                                } else {
                                    let max_chars = ((col_width - 8.0) / 7.0) as usize;
                                    let display_val = if value.len() > max_chars && max_chars > 3 {
                                        format!("{}…", &value[..max_chars.saturating_sub(1)])
                                    } else {
                                        value.clone()
                                    };

                                    ui.painter().text(
                                        cell_rect.left_center() + egui::vec2(8.0, 0.0),
                                        egui::Align2::LEFT_CENTER,
                                        display_val,
                                        typography::monospace(typography::SM),
                                        colors.muted_text,
                                    );
                                }
                            }
                        });
                    }
                });

            // Sync horizontal scroll
            let body_offset = body_scroll_output.state.offset;
            ui.ctx()
                .memory_mut(|m| m.data.insert_temp(h_sync_id, body_offset.x));
        });
}

/// Render the inline plan view from snapshot data.
fn render_inline_plan(ui: &mut egui::Ui, cell: &SnapshotQueryCell, theme: AppTheme) {
    egui::Frame::new()
        .fill(theme.bg_base())
        .inner_margin(12.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(plan) = &cell.plan {
                        snapshot_plan::render_plan_tree(ui, plan, theme);
                    } else {
                        ui.label(
                            RichText::new("No execution plan available.")
                                .color(theme.text_secondary())
                                .size(11.0),
                        );
                    }
                });
        });
}

/// Render the card footer with pagination controls.
fn render_card_footer(
    ui: &mut egui::Ui,
    cell: &SnapshotQueryCell,
    view_state: &CellViewState,
    colors: &OverlayColors,
    actions: &mut Vec<CardAction>,
) {
    let total_pages = (cell.rows.len().div_ceil(ROWS_PER_PAGE)).max(1);

    // Separator
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.cursor().top(),
        egui::Stroke::new(1.0, colors.separator),
    );
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.add_space(12.0);

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(12.0);

            // Read-only hint
            ui.label(
                RichText::new("read-only snapshot")
                    .color(colors.faint_text.gamma_multiply(0.7))
                    .font(typography::proportional(typography::XS)),
            );

            // Pagination (table tab only)
            if view_state.active_tab == CellTab::Table && total_pages > 1 {
                ui.add_space(12.0);

                // Next page button
                if view_state.table_page < total_pages - 1 {
                    let next_resp = ui.add(
                        egui::Label::new(
                            RichText::new(nav::FORWARD)
                                .color(colors.muted_text)
                                .size(11.0),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if next_resp.clicked() {
                        actions.push(CardAction::NextPage);
                    }
                    if next_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }

                // Page indicator
                ui.label(
                    RichText::new(format!("{} / {}", view_state.table_page + 1, total_pages))
                        .color(colors.muted_text)
                        .font(typography::proportional(typography::SM)),
                );

                // Prev page button
                if view_state.table_page > 0 {
                    let prev_resp = ui.add(
                        egui::Label::new(
                            RichText::new(nav::LEFT).color(colors.muted_text).size(11.0),
                        )
                        .sense(egui::Sense::click()),
                    );
                    if prev_resp.clicked() {
                        actions.push(CardAction::PrevPage);
                    }
                    if prev_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            }
        });
    });
    ui.add_space(4.0);
}
