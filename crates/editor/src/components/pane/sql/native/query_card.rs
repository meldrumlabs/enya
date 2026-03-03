//! Query card rendering for the single-cell SQL pane.
//!
//! Contains free functions (not `&mut SqlPane` methods) to render the result
//! card. Returns `CardAction` enums so the caller can apply mutations to
//! `SqlPane` after the closure returns.

use egui::{Color32, RichText};
use enya_datafusion::arrow::array::RecordBatch;
use enya_datafusion::arrow::datatypes::SchemaRef;
use enya_datafusion::format_array_value;

use super::super::rendering::{self, ColumnRow, PhaseRow};
use super::plan_view::PlanViewer;
use super::types::{Cell, CellViewState, QueryStatus};
use crate::components::OverlayColors;
use crate::components::util::{render_stat_badge, render_stat_badge_with_icon};
use crate::ui::semantic_icons::{action, nav, status, time};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Number of rows displayed per page in table views.
pub(super) const ROWS_PER_PAGE: usize = 50;

// Consistent styling constants (matching LogsPane pattern).
pub(super) const ROW_HEIGHT: f32 = 26.0;
const PADDING: f32 = 12.0;
const CORNER_RADIUS: f32 = 8.0;
pub(super) const COL_SPACING: f32 = 16.0;

/// Actions returned by card rendering for the caller to apply.
pub(super) enum CardAction {
    /// Collapse/dismiss the result card (refocus input).
    Collapse,
    /// Copy text to clipboard.
    CopyToClipboard(String),
    /// Share result to agent panel.
    ShareToAgent,
    /// Delete the result cell.
    Delete,
    /// Open the fullscreen table overlay.
    ExpandTable,
    /// Cancel the currently running query.
    Cancel,
    /// Move to the next result page.
    NextPage,
    /// Move to the previous result page.
    PrevPage,
}

/// Render the result card (always expanded).
///
/// `input_has_focus`: when `true`, the SQL input bar has focus — the card
/// skips global keyboard shortcuts (vim scroll, x/s/Esc) so they don't
/// conflict with typing.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_query_card(
    ui: &mut egui::Ui,
    cell: &Cell,
    cell_idx: usize,
    view_state: &mut CellViewState,
    theme: AppTheme,
    overlay_blocks_input: bool,
    plan_viewer: &mut PlanViewer,
    input_has_focus: bool,
) -> Vec<CardAction> {
    render_expanded_card(
        ui,
        cell,
        cell_idx,
        view_state,
        theme,
        overlay_blocks_input,
        plan_viewer,
        input_has_focus,
    )
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
    input_has_focus: bool,
) -> Vec<CardAction> {
    let mut actions = Vec::new();
    let colors = OverlayColors::new(theme);
    let text_primary = theme.text_primary();
    let text_secondary = theme.text_secondary();
    let accent = theme.accent_primary();

    // Handle keyboard shortcuts only when input bar doesn't have focus
    if !overlay_blocks_input && !input_has_focus {
        let mut should_collapse = false;
        let mut should_cancel = false;
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
                if cell.status() == QueryStatus::Running {
                    should_cancel = true;
                } else {
                    should_collapse = true;
                }
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

        if should_cancel {
            actions.push(CardAction::Cancel);
        }
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
        .corner_radius(CORNER_RADIUS)
        .inner_margin(0.0)
        .show(ui, |ui| {
            // === Header: full SQL + collapse chevron ===
            egui::Frame::new()
                .fill(theme.bg_surface())
                .inner_margin(egui::Margin::symmetric(PADDING as i8, 8))
                .corner_radius(egui::CornerRadius {
                    nw: CORNER_RADIUS as u8,
                    ne: CORNER_RADIUS as u8,
                    sw: 0,
                    se: 0,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
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
                                ui.add(egui::Spinner::new().color(accent).size(14.0));
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
                                    RichText::new(action::CLOSE)
                                        .color(text_secondary.gamma_multiply(0.5))
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
                                        .color(text_secondary.gamma_multiply(0.7))
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

                            // Cancel button (only when running)
                            if cell.status() == QueryStatus::Running {
                                ui.add_space(8.0);
                                let cancel_resp = ui.add(
                                    egui::Label::new(
                                        RichText::new(action::CANCEL)
                                            .color(theme.semantic_error().gamma_multiply(0.7))
                                            .size(11.0),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if cancel_resp.clicked() {
                                    actions.push(CardAction::Cancel);
                                }
                                if cancel_resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    cancel_resp.on_hover_text("Cancel query (Esc)");
                                }
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
                render_stats_bar(ui, cell, theme, &mut actions);
            }

            // Separator
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, colors.separator),
            );

            let is_benchmark = matches!(cell.kind, super::types::CellKind::Benchmark(_));
            let is_describe = matches!(cell.kind, super::types::CellKind::Describe(_));

            if is_benchmark {
                render_benchmark_card(ui, cell, theme);
            } else if is_describe {
                render_describe_card(ui, cell, theme);
            } else if is_explain {
                // Explain cells always show the plan directly
                render_inline_plan(ui, cell, plan_viewer, theme, overlay_blocks_input);
            } else if is_query {
                if cell.status() == QueryStatus::Running && cell.batches().is_empty() {
                    // Show shimmer skeleton while query is executing
                    render_loading_skeleton(ui, theme);
                } else if !cell.batches().is_empty() && cell.schema().is_some() {
                    render_inline_table(ui, cell, cell_idx, view_state, theme);
                } else if cell.status() == QueryStatus::Completed
                    && cell.batches().is_empty()
                    && cell.get_error().is_none()
                {
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
            render_card_footer(ui, cell, view_state, theme, &colors, &mut actions);
        });

    actions
}

/// Render the stats bar for query cells.
fn render_stats_bar(
    ui: &mut egui::Ui,
    cell: &Cell,
    theme: AppTheme,
    actions: &mut Vec<CardAction>,
) {
    let colors = OverlayColors::new(theme);
    let text_secondary = theme.text_secondary();

    egui::Frame::new()
        .fill(theme.bg_surface())
        .inner_margin(egui::Margin::symmetric(PADDING as i8, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Stats on the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Fullscreen button
                    if !cell.batches().is_empty() {
                        let expand_resp = ui.add(
                            egui::Label::new(
                                RichText::new(nav::FULLSCREEN)
                                    .color(text_secondary.gamma_multiply(0.5))
                                    .size(13.0),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if expand_resp.clicked() {
                            actions.push(CardAction::ExpandTable);
                        }
                        if expand_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            expand_resp.on_hover_text("Expand results");
                        }
                        ui.add_space(8.0);
                    }

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
    let accent = theme.accent_primary();
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
    let row_height = ROW_HEIGHT;
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
    let col_spacing = COL_SPACING;

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

                            // Column name (without inline sort indicator)
                            ui.painter().text(
                                col_rect.left_center() + egui::vec2(8.0, -6.0),
                                egui::Align2::LEFT_CENTER,
                                field.name(),
                                typography::monospace(typography::SM),
                                if is_sort_col {
                                    colors.accent
                                } else {
                                    colors.text
                                },
                            );

                            // Sort indicator as separate icon at right edge
                            if is_sort_col {
                                let icon = if view_state.sort_ascending {
                                    "▲"
                                } else {
                                    "▼"
                                };
                                ui.painter().text(
                                    col_rect.right_center() + egui::vec2(-8.0, -6.0),
                                    egui::Align2::RIGHT_CENTER,
                                    icon,
                                    typography::monospace(typography::XS),
                                    colors.accent,
                                );
                            } else if col_response.hovered() {
                                // Ghost arrow hint on hover for unsorted columns
                                ui.painter().text(
                                    col_rect.right_center() + egui::vec2(-8.0, -6.0),
                                    egui::Align2::RIGHT_CENTER,
                                    "▲",
                                    typography::monospace(typography::XS),
                                    colors.faint_text.gamma_multiply(0.3),
                                );
                            }

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
            let avail = ui.available_height();
            let body_max_height = if avail.is_finite() && avail > 0.0 {
                (avail - header_height - 40.0).clamp(100.0, 600.0)
            } else {
                (400.0 - header_height).max(100.0)
            };
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

                        let row_resp = ui.horizontal(|ui| {
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

                            // Gutter right border
                            ui.painter().line_segment(
                                [
                                    egui::pos2(gutter_rect.right(), gutter_rect.top()),
                                    egui::pos2(gutter_rect.right(), gutter_rect.bottom()),
                                ],
                                egui::Stroke::new(1.0, theme.border_subtle()),
                            );

                            // Alternating row background
                            let row_bg = if display_idx % 2 == 0 {
                                Color32::TRANSPARENT
                            } else {
                                theme.bg_hover().gamma_multiply(0.3)
                            };

                            // Cell values
                            for col_idx in 0..batch.num_columns() {
                                let col_width =
                                    column_widths.get(col_idx).copied().unwrap_or(100.0);

                                let (cell_rect, cell_response) = ui.allocate_exact_size(
                                    egui::vec2(col_width + col_spacing, row_height),
                                    egui::Sense::click(),
                                );
                                ui.painter().rect_filled(cell_rect, 0.0, row_bg);

                                let col = batch.column(col_idx);
                                let value = format_array_value(col.as_ref(), row_idx);

                                let mut is_truncated = false;

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
                                    // Right-align numeric columns
                                    let is_numeric =
                                        is_numeric_type(schema.field(col_idx).data_type());
                                    let max_chars = ((col_width - 8.0) / 7.0).max(0.0) as usize;
                                    is_truncated = value.len() > max_chars && max_chars > 3;
                                    let display_val = if is_truncated {
                                        let truncated: String = value
                                            .chars()
                                            .take(max_chars.saturating_sub(1))
                                            .collect();
                                        format!("{truncated}…")
                                    } else {
                                        value.clone()
                                    };

                                    let (align, pos) = if is_numeric {
                                        (
                                            egui::Align2::RIGHT_CENTER,
                                            cell_rect.right_center() + egui::vec2(-8.0, 0.0),
                                        )
                                    } else {
                                        (
                                            egui::Align2::LEFT_CENTER,
                                            cell_rect.left_center() + egui::vec2(8.0, 0.0),
                                        )
                                    };

                                    ui.painter().text(
                                        pos,
                                        align,
                                        display_val,
                                        typography::monospace(typography::SM),
                                        colors.muted_text,
                                    );
                                }

                                // Click-to-copy and cursor (extract before tooltip move)
                                let clicked = cell_response.clicked();
                                let hovered = cell_response.hovered();
                                if clicked {
                                    ui.ctx().copy_text(value.clone());
                                }
                                if hovered {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::Cell);
                                }

                                // Tooltip for truncated values (consumes response)
                                if is_truncated {
                                    cell_response.on_hover_text_at_pointer(
                                        RichText::new(&value).monospace().size(11.0),
                                    );
                                }
                            }
                        });

                        // Hover row highlighting (painted over the row)
                        if row_resp.response.hovered() {
                            let hover_bg = accent.gamma_multiply(0.06);
                            ui.painter()
                                .rect_filled(row_resp.response.rect, 0.0, hover_bg);
                        }
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

/// Render a benchmark results card with progress or stats.
fn render_benchmark_card(ui: &mut egui::Ui, cell: &Cell, theme: AppTheme) {
    let text_secondary = theme.text_secondary();
    let accent = theme.accent_primary();

    if let super::types::CellKind::Benchmark(bench) = &cell.kind {
        match bench.status {
            QueryStatus::Running => {
                egui::Frame::new()
                    .fill(theme.bg_base())
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        // Show progress
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().color(accent).size(14.0));
                            ui.add_space(8.0);
                            if let Some((current, total)) = bench.progress {
                                ui.label(
                                    RichText::new(format!("Iteration {current}/{total}"))
                                        .color(theme.text_primary())
                                        .size(12.0)
                                        .monospace(),
                                );
                                if let Some(last) = bench.last_duration {
                                    ui.add_space(12.0);
                                    let colors = OverlayColors::new(theme);
                                    render_stat_badge_with_icon(
                                        ui,
                                        time::TIMER,
                                        &format!(
                                            "last: {}",
                                            enya_datafusion::format_duration(last)
                                        ),
                                        &colors,
                                    );
                                }
                            } else {
                                ui.label(
                                    RichText::new("Starting benchmark...")
                                        .color(text_secondary)
                                        .size(11.0),
                                );
                            }
                        });

                        // Progress bar
                        if let Some((current, total)) = bench.progress {
                            ui.add_space(8.0);
                            let progress = current as f32 / total as f32;
                            let bar = egui::ProgressBar::new(progress)
                                .desired_width(ui.available_width().max(1.0));
                            ui.add(bar);
                        }
                    });
            }
            QueryStatus::Completed => {
                if let Some(stats) = &bench.stats {
                    render_benchmark_stats_bar(ui, stats, theme);

                    // Separator
                    let colors = OverlayColors::new(theme);
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, colors.separator),
                    );

                    render_benchmark_phase_table(ui, stats, theme);
                }
            }
            QueryStatus::Failed | QueryStatus::Cancelled => {
                // Error already shown by the outer card
            }
        }
    }
}

/// Render the stats bar for completed benchmarks.
fn render_benchmark_stats_bar(
    ui: &mut egui::Ui,
    stats: &enya_datafusion::BenchmarkStats,
    theme: AppTheme,
) {
    rendering::render_stats_bar_frame(ui, theme, |ui, colors| {
        if stats.rows_per_iteration > 0 {
            render_stat_badge(
                ui,
                &format!(
                    "{} rows/iter",
                    rendering::format_number(stats.rows_per_iteration as u64)
                ),
                colors,
            );
            ui.add_space(4.0);
        }

        render_stat_badge_with_icon(
            ui,
            time::TIMER,
            &format!(
                "median: {}",
                enya_datafusion::format_duration(stats.total.median)
            ),
            colors,
        );
        ui.add_space(4.0);

        render_stat_badge(ui, &format!("{} iterations", stats.iterations), colors);
    });
}

/// Render the phase timing table for completed benchmarks.
fn render_benchmark_phase_table(
    ui: &mut egui::Ui,
    stats: &enya_datafusion::BenchmarkStats,
    theme: AppTheme,
) {
    let fmt = |t: &enya_datafusion::PhaseTiming| -> PhaseRow<'_> {
        PhaseRow {
            name: "", // set below
            values: [t.min, t.median, t.mean, t.max].map(enya_datafusion::format_duration),
            percent: Some(t.percent_of_total),
        }
    };

    let mut rows = [
        fmt(&stats.logical_planning),
        fmt(&stats.physical_planning),
        fmt(&stats.execution),
        fmt(&stats.total),
    ];
    let default_names = [
        "Logical Planning",
        "Physical Planning",
        "Execution",
        "Total",
    ];
    for (i, row) in rows.iter_mut().enumerate() {
        row.name = match &stats.phase_names {
            Some(names) => names[i].as_str(),
            None => default_names[i],
        };
    }
    rows[3].percent = None; // suppress % for Total

    rendering::render_phase_table(ui, &rows, theme);
}

/// Render a describe results card with stats or spinner.
fn render_describe_card(ui: &mut egui::Ui, cell: &Cell, theme: AppTheme) {
    let text_secondary = theme.text_secondary();
    let accent = theme.accent_primary();

    if let super::types::CellKind::Describe(desc) = &cell.kind {
        match desc.status {
            QueryStatus::Running => {
                egui::Frame::new()
                    .fill(theme.bg_base())
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().color(accent).size(14.0));
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Computing column statistics...")
                                    .color(text_secondary)
                                    .size(11.0),
                            );
                        });
                    });
            }
            QueryStatus::Completed => {
                if let Some(stats) = &desc.stats {
                    render_describe_stats_bar(ui, stats, theme);

                    let colors = OverlayColors::new(theme);
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, colors.separator),
                    );

                    render_describe_table(ui, stats, theme);
                }
            }
            QueryStatus::Failed | QueryStatus::Cancelled => {}
        }
    }
}

/// Render the stats bar for completed describe.
fn render_describe_stats_bar(
    ui: &mut egui::Ui,
    stats: &enya_datafusion::DescribeStats,
    theme: AppTheme,
) {
    rendering::render_stats_bar_frame(ui, theme, |ui, colors| {
        render_stat_badge_with_icon(
            ui,
            time::TIMER,
            &enya_datafusion::format_duration(stats.elapsed),
            colors,
        );
        ui.add_space(4.0);

        render_stat_badge(ui, &format!("{} columns", stats.columns.len()), colors);
        ui.add_space(4.0);

        render_stat_badge(
            ui,
            &format!("{} rows", rendering::format_number(stats.total_rows as u64)),
            colors,
        );
    });
}

/// Render the column statistics table.
fn render_describe_table(
    ui: &mut egui::Ui,
    stats: &enya_datafusion::DescribeStats,
    theme: AppTheme,
) {
    let rows: Vec<ColumnRow<'_>> = stats
        .columns
        .iter()
        .map(|col| ColumnRow {
            name: &col.name,
            data_type: &col.data_type,
            count: rendering::format_number(col.count as u64),
            null_count: rendering::format_number(col.null_count as u64),
            distinct_count: rendering::format_number(col.distinct_count as u64),
            min: col.min.as_deref(),
            max: col.max.as_deref(),
            mean: col.mean,
        })
        .collect();

    rendering::render_column_stats_table(ui, &rows, theme);
}

/// Render the card footer with pagination controls and keyboard hints.
fn render_card_footer(
    ui: &mut egui::Ui,
    cell: &Cell,
    view_state: &CellViewState,
    _theme: AppTheme,
    colors: &OverlayColors,
    actions: &mut Vec<CardAction>,
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

        // Left side: row range indicator for query cells
        let is_query = matches!(cell.kind, super::types::CellKind::Query(_));
        if is_query && total_rows > 0 {
            let start = view_state.table_page * rows_per_page + 1;
            let end = ((view_state.table_page + 1) * rows_per_page).min(total_rows);
            let total_fmt = rendering::format_number(total_rows as u64);
            ui.label(
                RichText::new(format!("Rows {start}\u{2013}{end} of {total_fmt}"))
                    .color(colors.muted_text)
                    .font(typography::proportional(typography::XS)),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(12.0);

            // Keyboard hints
            let is_explain = matches!(cell.kind, super::types::CellKind::Explain(_));
            let is_benchmark = matches!(cell.kind, super::types::CellKind::Benchmark(_));
            let is_describe = matches!(cell.kind, super::types::CellKind::Describe(_));
            let hints = if is_benchmark || is_describe {
                "\u{2318}C copy \u{00B7} x close \u{00B7} Esc"
            } else if is_explain {
                "j/k nav \u{00B7} h/l fold \u{00B7} \u{2318}C copy \u{00B7} x close \u{00B7} Esc"
            } else {
                "hjkl scroll \u{00B7} [/] page \u{00B7} gg/G first/last \u{00B7} \u{2318}C copy \u{00B7} S share \u{00B7} x close \u{00B7} Esc"
            };
            ui.label(
                RichText::new(hints)
                    .color(colors.faint_text.gamma_multiply(0.7))
                    .font(typography::proportional(typography::XS)),
            );

            // Pagination with clickable buttons
            if !is_explain && total_pages > 1 {
                ui.add_space(12.0);

                // Next page button (right-to-left: appears rightmost)
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
                    RichText::new(format!(
                        "{} / {}",
                        view_state.table_page + 1,
                        total_pages
                    ))
                    .color(colors.muted_text)
                    .font(typography::proportional(typography::SM)),
                );

                // Prev page button
                if view_state.table_page > 0 {
                    let prev_resp = ui.add(
                        egui::Label::new(
                            RichText::new(nav::BACK)
                                .color(colors.muted_text)
                                .size(11.0),
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

/// Render a table-shaped shimmer loading skeleton.
fn render_loading_skeleton(ui: &mut egui::Ui, theme: AppTheme) {
    let time = ui.ctx().input(|i| i.time);
    let available = ui.available_size();
    let accent = theme.accent_primary();
    let base = theme.bg_elevated();

    // Theme-aware skeleton colors with subtle accent tint
    let skeleton_base = if theme.is_dark() {
        Color32::from_rgb(
            base.r().saturating_add((accent.r() as u16 * 3 / 100) as u8),
            base.g().saturating_add((accent.g() as u16 * 3 / 100) as u8),
            base.b().saturating_add((accent.b() as u16 * 3 / 100) as u8),
        )
    } else {
        Color32::from_rgb(
            base.r().saturating_sub(8),
            base.g().saturating_sub(8),
            base.b().saturating_sub(6),
        )
    };

    let shimmer_color = if theme.is_dark() {
        accent.gamma_multiply(0.35)
    } else {
        accent.gamma_multiply(0.20)
    };

    // Shimmer sweeps left to right
    let shimmer_progress = ((time * 0.8) % 2.0) as f32;
    let shimmer_width = available.x * 0.4;
    let shimmer_x = (shimmer_progress - 0.5) * (available.x + shimmer_width);

    // Constrain skeleton height
    let skeleton_height = available.y.clamp(100.0, 300.0);
    let (full_rect, _) = ui.allocate_exact_size(
        egui::vec2(available.x, skeleton_height),
        egui::Sense::hover(),
    );
    let painter = ui.painter();

    // Row gutter skeleton
    let gutter_width = 40.0;
    painter.rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(full_rect.left() + PADDING, full_rect.top() + PADDING),
            egui::vec2(20.0, 12.0),
        ),
        3.0,
        skeleton_base.gamma_multiply(0.5),
    );

    // Column header skeletons
    let col_widths = [120.0, 80.0, 100.0, 90.0, 110.0];
    let mut x = full_rect.left() + PADDING + gutter_width;
    for &w in &col_widths {
        if x + w > full_rect.right() - PADDING {
            break;
        }
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(x, full_rect.top() + PADDING),
                egui::vec2(w * 0.6, 12.0),
            ),
            3.0,
            skeleton_base.gamma_multiply(0.7),
        );
        x += w + COL_SPACING;
    }

    // Row skeletons
    let num_rows = ((skeleton_height - 50.0) / ROW_HEIGHT) as usize;
    for i in 0..num_rows.min(10) {
        let y = full_rect.top() + 42.0 + i as f32 * ROW_HEIGHT;

        // Alternating row background
        if i % 2 == 1 {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(full_rect.left(), y),
                    egui::vec2(available.x, ROW_HEIGHT),
                ),
                0.0,
                skeleton_base.gamma_multiply(0.3),
            );
        }

        // Gutter number skeleton
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(full_rect.left() + PADDING + 4.0, y + 7.0),
                egui::vec2(24.0, 12.0),
            ),
            3.0,
            skeleton_base.gamma_multiply(0.4),
        );

        // Cell value skeletons at varying widths
        let mut cx = full_rect.left() + PADDING + gutter_width;
        for (j, &w) in col_widths.iter().enumerate() {
            if cx + w > full_rect.right() - PADDING {
                break;
            }
            let cell_width = w * (0.4 + ((i * 47 + j * 31) % 50) as f32 / 100.0);
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(cx + 8.0, y + 7.0),
                    egui::vec2(cell_width, 12.0),
                ),
                3.0,
                skeleton_base,
            );
            cx += w + COL_SPACING;
        }
    }

    // Shimmer overlay
    let shimmer_rect = egui::Rect::from_min_size(
        egui::pos2(full_rect.left() + shimmer_x, full_rect.top()),
        egui::vec2(shimmer_width, skeleton_height),
    );
    let clipped = shimmer_rect.intersect(full_rect);
    if clipped.width() > 0.0 {
        let segments = 10;
        let segment_width = clipped.width() / segments as f32;
        for i in 0..segments {
            let alpha = {
                let t = i as f32 / segments as f32;
                (-(t - 0.5).powi(2) * 8.0).exp()
            };
            let seg_rect = egui::Rect::from_min_size(
                egui::pos2(clipped.left() + i as f32 * segment_width, clipped.top()),
                egui::vec2(segment_width, clipped.height()),
            );
            painter.rect_filled(seg_rect, 0.0, shimmer_color.gamma_multiply(alpha));
        }
    }

    ui.ctx().request_repaint();
}

/// Check whether an Arrow data type is numeric (for right-alignment).
pub(super) fn is_numeric_type(dt: &enya_datafusion::arrow::datatypes::DataType) -> bool {
    use enya_datafusion::arrow::datatypes::DataType;
    matches!(
        dt,
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float16
            | DataType::Float32
            | DataType::Float64
            | DataType::Decimal128(_, _)
            | DataType::Decimal256(_, _)
    )
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
