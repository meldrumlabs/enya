//! Notebook-style query card rendering.
//!
//! Contains free functions (not `&mut SqlPane` methods) to render each query
//! as a collapsed or expanded card. Returns `CardAction` enums so the caller
//! can apply mutations to `SqlPane` after the closure returns.

use egui::{Color32, RichText};
use enya_datafusion::arrow::array::RecordBatch;
use enya_datafusion::arrow::datatypes::SchemaRef;
use enya_datafusion::format_array_value;

use super::plan_view::PlanViewer;
use super::types::{Cell, CellViewState, QueryStatus};
use crate::components::OverlayColors;
use crate::components::util::{render_stat_badge, render_stat_badge_with_icon};
use crate::ui::semantic_icons::{actions, nav, status, time};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Number of rows displayed per page in table views.
const ROWS_PER_PAGE: usize = 50;

/// Actions returned by card rendering for the caller to apply.
pub(super) enum CardAction {
    /// Select this cell (highlight, enter NAV mode).
    Select,
    /// Expand this cell (collapse any other).
    Expand,
    /// Collapse this cell.
    Collapse,
    /// Copy text to clipboard.
    CopyToClipboard(String),
    /// Share result to agent panel.
    ShareToAgent,
    /// Delete this cell from history.
    Delete,
}

/// Entry point: renders a query card (collapsed or expanded).
#[allow(clippy::too_many_arguments)]
pub(super) fn render_query_card(
    ui: &mut egui::Ui,
    cell: &Cell,
    cell_idx: usize,
    view_state: &mut CellViewState,
    theme: AppTheme,
    overlay_blocks_input: bool,
    plan_viewer: &mut PlanViewer,
    is_selected: bool,
    cell_number: usize,
) -> Vec<CardAction> {
    if view_state.expanded {
        render_expanded_card(
            ui,
            cell,
            cell_idx,
            view_state,
            theme,
            overlay_blocks_input,
            plan_viewer,
            cell_number,
        )
    } else {
        render_collapsed_card(ui, cell, cell_idx, theme, is_selected, cell_number)
    }
}

/// Render a collapsed card: status icon + SQL preview + stats + expand chevron.
fn render_collapsed_card(
    ui: &mut egui::Ui,
    cell: &Cell,
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
            // Header: status icon + SQL preview + stats + chevron
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
                        match cell.status() {
                            QueryStatus::Running => {
                                ui.spinner();
                            }
                            QueryStatus::Completed => {
                                ui.label(
                                    RichText::new(status::SUCCESS)
                                        .color(theme.semantic_success())
                                        .size(11.0),
                                );
                            }
                            QueryStatus::Failed => {
                                ui.label(
                                    RichText::new(status::ERROR)
                                        .color(theme.semantic_error())
                                        .size(11.0),
                                );
                            }
                            QueryStatus::Cancelled => {
                                ui.label(
                                    RichText::new(status::ERROR)
                                        .color(text_secondary)
                                        .size(11.0),
                                );
                            }
                        }
                        ui.add_space(6.0);

                        // SQL preview (truncated to 1 line)
                        let sql_oneline = cell.sql().replace('\n', " ");
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

                        // Right side: close + stats + chevron
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Close button
                            let close_resp = ui.add(
                                egui::Label::new(
                                    RichText::new(actions::CLOSE)
                                        .color(text_secondary.gamma_multiply(0.3))
                                        .size(11.0),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if close_resp.clicked() {
                                actions.push(CardAction::Delete);
                            }
                            if close_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            ui.add_space(8.0);

                            // Expand chevron
                            ui.label(
                                RichText::new(nav::FORWARD)
                                    .color(text_secondary.gamma_multiply(0.5))
                                    .size(11.0),
                            );
                            ui.add_space(8.0);

                            // Execution time
                            if let Some(stats) = cell.stats() {
                                ui.label(
                                    RichText::new(format!("{}ms", stats.total_time.as_millis()))
                                        .color(text_secondary)
                                        .size(10.0),
                                );
                                ui.add_space(4.0);
                            }

                            // Row count
                            if cell.status() == QueryStatus::Completed {
                                let row_count: usize =
                                    cell.batches().iter().map(|b| b.num_rows()).sum();
                                ui.label(
                                    RichText::new(format!("{row_count} rows"))
                                        .color(text_secondary)
                                        .size(10.0),
                                );
                            }

                            // Running elapsed time
                            if cell.status() == QueryStatus::Running {
                                let elapsed = cell.created_at().elapsed().as_secs_f32();
                                ui.label(
                                    RichText::new(format!("{elapsed:.1}s"))
                                        .color(accent)
                                        .size(10.0),
                                );
                                ui.ctx().request_repaint();
                            }
                        });
                    });
                });

            // Error message if failed
            if let Some(error) = cell.get_error() {
                egui::Frame::new()
                    .fill(theme.semantic_error().gamma_multiply(0.1))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        let display_error = if error.len() > 120 {
                            format!("{}...", &error[..120])
                        } else {
                            error.to_string()
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
            if !cell.batches().is_empty() {
                if let Some(schema) = cell.schema() {
                    render_compact_table_preview(
                        ui,
                        schema,
                        cell.batches(),
                        max_preview_rows,
                        max_value_len,
                        text_primary,
                        text_secondary,
                    );
                }
            }

            // Bottom bar (subtle, no footer needed for collapsed)
            egui::Frame::new()
                .fill(theme.bg_surface())
                .inner_margin(egui::Margin::symmetric(12, 4))
                .corner_radius(egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 8,
                    se: 8,
                })
                .show(ui, |_ui| {
                    // Intentionally minimal - just closes the card frame
                });
        });

    // Click to select, double-click to expand
    let card_rect = card_response.response.rect;
    let click_response = ui.interact(
        card_rect,
        egui::Id::new(("card_click", cell_idx)),
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
    schema: &SchemaRef,
    batches: &[RecordBatch],
    max_rows: usize,
    max_value_len: usize,
    text_primary: Color32,
    text_secondary: Color32,
) {
    let total_cols = schema.fields().len();
    let available_width = ui.available_width() - 24.0;
    let col_spacing = 16.0;
    let char_width = 6.5;
    let overflow_indicator_width = 40.0;

    let col_widths: Vec<f32> = schema
        .fields()
        .iter()
        .map(|f| {
            let name_len = f.name().len().min(max_value_len);
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
                for (col_idx, field) in schema.fields().iter().take(show_cols).enumerate() {
                    if col_idx > 0 {
                        ui.add_space(col_spacing);
                    }
                    let name = field.name();
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
            let mut rows_shown = 0;
            'outer: for batch in batches {
                for row_idx in 0..batch.num_rows() {
                    if rows_shown >= max_rows {
                        break 'outer;
                    }
                    ui.horizontal(|ui| {
                        for col_idx in 0..batch.num_columns().min(show_cols) {
                            if col_idx > 0 {
                                ui.add_space(col_spacing);
                            }
                            let col = batch.column(col_idx);
                            let value = format_array_value(col.as_ref(), row_idx);
                            let (display_val, color) = if value == "NULL" {
                                ("null".to_string(), text_secondary.gamma_multiply(0.4))
                            } else if value.len() > max_value_len {
                                (format!("{}…", &value[..max_value_len - 1]), text_secondary)
                            } else {
                                (value, text_secondary)
                            };
                            ui.label(
                                RichText::new(display_val)
                                    .color(color)
                                    .size(10.0)
                                    .monospace(),
                            );
                        }
                    });
                    rows_shown += 1;
                }
            }

            // "More rows" indicator
            let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
            if total_rows > max_rows {
                ui.label(
                    RichText::new(format!("… {} more", total_rows - max_rows))
                        .color(text_secondary.gamma_multiply(0.5))
                        .size(10.0)
                        .italics(),
                );
            }
        });
}

/// Render an expanded card: full SQL + tab bar + inline content + footer.
#[allow(clippy::too_many_arguments)]
fn render_expanded_card(
    ui: &mut egui::Ui,
    cell: &Cell,
    cell_idx: usize,
    view_state: &mut CellViewState,
    theme: AppTheme,
    overlay_blocks_input: bool,
    plan_viewer: &mut PlanViewer,
    cell_number: usize,
) -> Vec<CardAction> {
    let mut actions = Vec::new();
    let colors = OverlayColors::new(theme);
    let text_primary = theme.text_primary();
    let text_secondary = theme.text_secondary();
    let accent = theme.accent_primary();

    // Surrender focus so vim keys don't leak into the input bar
    ui.ctx()
        .memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));

    // Handle keyboard shortcuts
    if !overlay_blocks_input {
        let mut should_collapse = false;
        let mut should_delete = false;
        let mut next_page = false;
        let mut prev_page = false;
        let mut goto_last_page = false;
        let mut goto_first_pending = false;
        let mut scroll_delta = egui::Vec2::ZERO;
        let mut should_copy = false;
        let mut should_share = false;

        ui.ctx().input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                should_collapse = true;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::X) {
                should_delete = true;
            }
            if i.consume_key(egui::Modifiers::COMMAND, egui::Key::C)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::C)
            {
                should_copy = true;
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::S) {
                should_share = true;
            }

            // Vim scrolling (query cells only)
            if matches!(cell.kind, super::types::CellKind::Query(_)) {
                let step = 50.0;
                if i.consume_key(egui::Modifiers::NONE, egui::Key::L) {
                    scroll_delta.x += step;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::H) {
                    scroll_delta.x -= step;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::J) {
                    scroll_delta.y += step;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::K) {
                    scroll_delta.y -= step;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::CloseBracket)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                {
                    next_page = true;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::OpenBracket)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
                {
                    prev_page = true;
                }
                // G (Shift+G): jump to last page
                if i.consume_key(egui::Modifiers::SHIFT, egui::Key::G) {
                    goto_last_page = true;
                }
                // g: first press of gg motion
                if i.consume_key(egui::Modifiers::NONE, egui::Key::G) {
                    goto_first_pending = true;
                }
            }
        });

        if should_collapse {
            actions.push(CardAction::Collapse);
        }
        if should_delete {
            actions.push(CardAction::Delete);
        }
        if should_copy {
            if let Some(schema) = cell.schema() {
                let tsv = format_results_as_tsv(schema, cell.batches());
                actions.push(CardAction::CopyToClipboard(tsv));
            }
        }
        if should_share {
            actions.push(CardAction::ShareToAgent);
        }

        // Apply pagination
        let rows_per_page = ROWS_PER_PAGE;
        let total_rows: usize = cell.batches().iter().map(|b| b.num_rows()).sum();
        let total_pages = total_rows.div_ceil(rows_per_page).max(1);
        if next_page && view_state.table_page < total_pages - 1 {
            view_state.table_page += 1;
        }
        if prev_page && view_state.table_page > 0 {
            view_state.table_page -= 1;
        }

        // G: jump to last page
        if goto_last_page {
            view_state.table_page = total_pages.saturating_sub(1);
        }

        // gg motion: two consecutive g presses → first page
        let pending_g_id = egui::Id::new(("table_pending_g", cell_idx));
        if goto_first_pending {
            let was_pending = ui
                .ctx()
                .memory(|m| m.data.get_temp::<bool>(pending_g_id).unwrap_or(false));
            if was_pending {
                view_state.table_page = 0;
                ui.ctx()
                    .memory_mut(|m| m.data.insert_temp(pending_g_id, false));
            } else {
                ui.ctx()
                    .memory_mut(|m| m.data.insert_temp(pending_g_id, true));
            }
        } else if goto_last_page || next_page || prev_page || scroll_delta != egui::Vec2::ZERO {
            // Any other table key cancels pending g
            ui.ctx()
                .memory_mut(|m| m.data.insert_temp(pending_g_id, false));
        }

        // Apply vim scroll delta to the scroll area
        if scroll_delta != egui::Vec2::ZERO {
            let scroll_id = egui::Id::new(("card_table_scroll", cell_idx));
            let h_sync_id = egui::Id::new(("card_table_h_sync", cell_idx));
            let current = ui
                .ctx()
                .memory(|m| m.data.get_temp::<egui::Vec2>(scroll_id))
                .unwrap_or(egui::Vec2::ZERO);
            let new_offset = (current + scroll_delta).max(egui::Vec2::ZERO);
            ui.ctx().memory_mut(|m| {
                m.data.insert_temp(scroll_id, new_offset);
                m.data.insert_temp(h_sync_id, new_offset.x);
            });
        }
    }

    egui::Frame::new()
        .fill(theme.bg_elevated())
        .stroke(egui::Stroke::new(1.5, accent.gamma_multiply(0.5)))
        .corner_radius(8.0)
        .inner_margin(0.0)
        .show(ui, |ui| {
            // === Header: full SQL + collapse chevron ===
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
                        match cell.status() {
                            QueryStatus::Completed => {
                                ui.label(
                                    RichText::new(status::SUCCESS)
                                        .color(theme.semantic_success())
                                        .size(11.0),
                                );
                            }
                            QueryStatus::Failed => {
                                ui.label(
                                    RichText::new(status::ERROR)
                                        .color(theme.semantic_error())
                                        .size(11.0),
                                );
                            }
                            QueryStatus::Running => {
                                ui.spinner();
                            }
                            QueryStatus::Cancelled => {
                                ui.label(
                                    RichText::new(status::ERROR)
                                        .color(text_secondary)
                                        .size(11.0),
                                );
                            }
                        }
                        ui.add_space(6.0);

                        // Full SQL text (multi-line)
                        ui.label(
                            RichText::new(cell.sql())
                                .color(text_primary)
                                .size(11.0)
                                .monospace(),
                        );

                        // Close + collapse on the right
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Close button
                            let close_resp = ui.add(
                                egui::Label::new(
                                    RichText::new(actions::CLOSE)
                                        .color(text_secondary.gamma_multiply(0.3))
                                        .size(11.0),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if close_resp.clicked() {
                                actions.push(CardAction::Delete);
                            }
                            if close_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            ui.add_space(8.0);

                            // Collapse chevron
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
            if let Some(error) = cell.get_error() {
                egui::Frame::new()
                    .fill(theme.semantic_error().gamma_multiply(0.1))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(error)
                                .color(theme.semantic_error())
                                .size(11.0)
                                .monospace(),
                        );
                    });
            }

            // === Tab bar + content area ===
            let is_query = matches!(cell.kind, super::types::CellKind::Query(_));
            let is_explain = matches!(cell.kind, super::types::CellKind::Explain(_));

            if is_query && cell.status() == QueryStatus::Completed {
                render_stats_bar(ui, cell, theme);
            }

            // Separator
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, colors.separator),
            );

            if is_explain {
                // Explain cells always show the plan directly
                render_inline_plan(ui, cell, plan_viewer, theme, overlay_blocks_input);
            } else if is_query {
                if !cell.batches().is_empty() && cell.schema().is_some() {
                    render_inline_table(ui, cell, cell_idx, view_state, theme);
                } else if cell.batches().is_empty() && cell.get_error().is_none() {
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

            // === Footer ===
            render_card_footer(ui, cell, view_state, theme, &colors);
        });

    actions
}

/// Render the stats bar for query cells.
fn render_stats_bar(ui: &mut egui::Ui, cell: &Cell, theme: AppTheme) {
    let colors = OverlayColors::new(theme);

    egui::Frame::new()
        .fill(theme.bg_surface())
        .inner_margin(egui::Margin::symmetric(12, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Stats on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Execution time
                    if let Some(stats) = cell.stats() {
                        render_stat_badge_with_icon(
                            ui,
                            time::TIMER,
                            &format!("{}ms", stats.total_time.as_millis()),
                            &colors,
                        );
                        ui.add_space(4.0);
                    }

                    // Row count
                    let total_rows: usize = cell.batches().iter().map(|b| b.num_rows()).sum();
                    render_stat_badge(ui, &format!("{total_rows} rows"), &colors);

                    // Column count
                    if let Some(schema) = cell.schema() {
                        ui.add_space(4.0);
                        render_stat_badge(ui, &format!("{} cols", schema.fields().len()), &colors);
                    }
                });
            });
        });
}

/// Render the inline table view (full data with sort and pagination).
fn render_inline_table(
    ui: &mut egui::Ui,
    cell: &Cell,
    cell_idx: usize,
    view_state: &mut CellViewState,
    theme: AppTheme,
) {
    let colors = OverlayColors::new(theme);
    let bg_surface = theme.bg_surface();
    let rows_per_page = ROWS_PER_PAGE;

    let schema = cell.schema().unwrap();
    let num_cols = schema.fields().len();

    // Calculate column widths
    let column_widths: Vec<f32> = schema
        .fields()
        .iter()
        .map(|field| {
            let name_width = field.name().len() as f32 * 7.0;
            let type_width = format!("{}", field.data_type()).len() as f32 * 6.0;
            name_width.max(type_width).clamp(80.0, 200.0)
        })
        .collect();

    let header_height = typography::SM + typography::XS + 8.0;
    let row_height = typography::SM + 8.0;
    let start_row = view_state.table_page * rows_per_page;
    let max_row_num = (view_state.table_page + 1) * rows_per_page;
    let row_num_width = max_row_num.to_string().len().max(3);
    let row_num_gutter_width = (row_num_width + 2) as f32 * 8.0;

    // Build sorted row indices
    let sort_col = view_state.sort_column;
    let sort_asc = view_state.sort_ascending;
    let sorted_row_indices: Vec<(usize, usize)> = {
        let mut indices: Vec<(usize, usize)> = Vec::new();
        for (batch_idx, batch) in cell.batches().iter().enumerate() {
            for row_idx in 0..batch.num_rows() {
                indices.push((batch_idx, row_idx));
            }
        }
        if let Some(sc) = sort_col {
            if sc < num_cols {
                indices.sort_by(|a, b| {
                    let val_a = format_array_value(cell.batches()[a.0].column(sc).as_ref(), a.1);
                    let val_b = format_array_value(cell.batches()[b.0].column(sc).as_ref(), b.1);
                    let ord = compare_cell_values(&val_a, &val_b);
                    if sort_asc { ord } else { ord.reverse() }
                });
            }
        }
        indices
    };

    // Table content with sticky headers + scrollable body
    let col_spacing = 16.0;

    egui::Frame::new()
        .fill(theme.bg_base())
        .inner_margin(0.0)
        .show(ui, |ui| {
            let scroll_id = egui::Id::new(("card_table_scroll", cell_idx));
            let h_sync_id = egui::Id::new(("card_table_h_sync", cell_idx));

            // Read stored offsets (body is source of truth)
            let stored_offset = ui
                .ctx()
                .memory(|m| m.data.get_temp::<egui::Vec2>(scroll_id))
                .unwrap_or(egui::Vec2::ZERO);
            let stored_h_offset = ui
                .ctx()
                .memory(|m| m.data.get_temp::<f32>(h_sync_id))
                .unwrap_or(0.0);

            // === Sticky column headers (horizontal-only, synced with body) ===
            egui::ScrollArea::horizontal()
                .id_salt(("card_table_header", cell_idx))
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

                        for (idx, field) in schema.fields().iter().enumerate() {
                            let col_width = column_widths.get(idx).copied().unwrap_or(100.0);

                            let (col_rect, col_response) = ui.allocate_exact_size(
                                egui::vec2(col_width + col_spacing, header_height),
                                egui::Sense::click(),
                            );

                            // Toggle sort on click
                            if col_response.clicked() {
                                if view_state.sort_column == Some(idx) {
                                    if view_state.sort_ascending {
                                        view_state.sort_ascending = false;
                                    } else {
                                        view_state.sort_column = None;
                                    }
                                } else {
                                    view_state.sort_column = Some(idx);
                                    view_state.sort_ascending = true;
                                }
                                view_state.table_page = 0;
                            }

                            if col_response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            let is_sort_col = view_state.sort_column == Some(idx);
                            let header_bg = if col_response.hovered() {
                                theme.bg_hover()
                            } else if is_sort_col {
                                theme.bg_hover().gamma_multiply(0.5)
                            } else {
                                bg_surface
                            };
                            ui.painter().rect_filled(col_rect, 0.0, header_bg);

                            let sort_indicator = if is_sort_col {
                                if view_state.sort_ascending {
                                    " ▲"
                                } else {
                                    " ▼"
                                }
                            } else {
                                ""
                            };

                            ui.painter().text(
                                col_rect.left_center() + egui::vec2(8.0, -6.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{}{sort_indicator}", field.name()),
                                typography::monospace(typography::SM),
                                if is_sort_col {
                                    colors.accent
                                } else {
                                    colors.text
                                },
                            );

                            ui.painter().text(
                                col_rect.left_center() + egui::vec2(8.0, 6.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{}", field.data_type()),
                                typography::monospace(typography::XS),
                                colors.faint_text,
                            );
                        }
                    });
                });

            // === Scrollable data body (both directions) ===
            let body_max_height = (400.0 - header_height).max(100.0);
            let body_scroll_output = egui::ScrollArea::both()
                .id_salt(("card_table_body", cell_idx))
                .scroll_offset(egui::vec2(stored_h_offset, stored_offset.y))
                .max_height(body_max_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.style_mut().spacing.item_spacing = egui::Vec2::ZERO;

                    // Data rows
                    let page_end = (start_row + rows_per_page).min(sorted_row_indices.len());
                    let page_start = start_row.min(sorted_row_indices.len());

                    for (display_idx, &(batch_idx, row_idx)) in
                        sorted_row_indices[page_start..page_end].iter().enumerate()
                    {
                        let absolute_row = start_row + display_idx + 1;
                        let batch = &cell.batches()[batch_idx];

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
                            for col_idx in 0..batch.num_columns() {
                                let col_width =
                                    column_widths.get(col_idx).copied().unwrap_or(100.0);

                                let (cell_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(col_width + col_spacing, row_height),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(cell_rect, 0.0, row_bg);

                                let col = batch.column(col_idx);
                                let value = format_array_value(col.as_ref(), row_idx);

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
                                        value
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

            // Store body scroll offset as source of truth for sync
            let body_offset = body_scroll_output.state.offset;
            ui.ctx().memory_mut(|m| {
                m.data.insert_temp(scroll_id, body_offset);
                m.data.insert_temp(h_sync_id, body_offset.x);
            });
        });
}

/// Render the inline plan view (PlanViewer embedded in the card).
fn render_inline_plan(
    ui: &mut egui::Ui,
    _cell: &Cell,
    plan_viewer: &mut PlanViewer,
    theme: AppTheme,
    overlay_blocks_input: bool,
) {
    plan_viewer.set_overlay_blocks_input(overlay_blocks_input);

    // The plan is loaded into plan_viewer when the explain/analyze query completes
    // (in poll_async). We just render whatever is currently loaded.
    egui::Frame::new()
        .fill(theme.bg_base())
        .inner_margin(12.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if plan_viewer.root_plan().is_some() {
                        plan_viewer.show(ui);
                    } else {
                        ui.label(
                            RichText::new("No execution plan available. Run .explain or .analyze to see the plan.")
                                .color(theme.text_secondary())
                                .size(11.0),
                        );
                    }
                });
        });
}

/// Render the card footer with pagination controls and keyboard hints.
fn render_card_footer(
    ui: &mut egui::Ui,
    cell: &Cell,
    view_state: &CellViewState,
    _theme: AppTheme,
    colors: &OverlayColors,
) {
    let rows_per_page = ROWS_PER_PAGE;
    let total_rows: usize = cell.batches().iter().map(|b| b.num_rows()).sum();
    let total_pages = total_rows.div_ceil(rows_per_page).max(1);

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

            // Keyboard hints
            let is_explain = matches!(cell.kind, super::types::CellKind::Explain(_));
            let hints = if is_explain {
                "j/k nav \u{00B7} h/l fold \u{00B7} \u{2318}C copy \u{00B7} x close \u{00B7} Esc"
            } else {
                "hjkl scroll \u{00B7} [/] page \u{00B7} gg/G first/last \u{00B7} \u{2318}C copy \u{00B7} S share \u{00B7} x close \u{00B7} Esc"
            };
            ui.label(
                RichText::new(hints)
                    .color(colors.faint_text.gamma_multiply(0.7))
                    .font(typography::proportional(typography::XS)),
            );

            // Pagination
            if !is_explain && total_pages > 1 {
                ui.add_space(12.0);

                // Page indicator
                ui.label(
                    RichText::new(format!(
                        "{} / {}",
                        view_state.table_page + 1,
                        total_pages
                    ))
                    .color(colors.muted_text)
                    .font(typography::proportional(typography::SM)),
                );
            }
        });
    });
    ui.add_space(4.0);
}

/// Compare two cell values for sorting (numeric-aware, NULL-last).
pub(super) fn compare_cell_values(a: &str, b: &str) -> std::cmp::Ordering {
    match (a, b) {
        ("NULL", "NULL") => std::cmp::Ordering::Equal,
        ("NULL", _) => std::cmp::Ordering::Greater,
        (_, "NULL") => std::cmp::Ordering::Less,
        _ => {
            if let (Ok(an), Ok(bn)) = (a.parse::<f64>(), b.parse::<f64>()) {
                an.partial_cmp(&bn).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.cmp(b)
            }
        }
    }
}

/// Format query results as tab-separated values for clipboard.
fn format_results_as_tsv(schema: &SchemaRef, batches: &[RecordBatch]) -> String {
    let mut out = String::new();
    let fields = schema.fields();
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            out.push('\t');
        }
        out.push_str(field.name());
    }
    out.push('\n');
    for batch in batches {
        for row in 0..batch.num_rows() {
            for col in 0..batch.num_columns() {
                if col > 0 {
                    out.push('\t');
                }
                out.push_str(&format_array_value(batch.column(col).as_ref(), row));
            }
            out.push('\n');
        }
    }
    out
}
