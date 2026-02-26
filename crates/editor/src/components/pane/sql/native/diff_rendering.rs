//! Diff view rendering utilities.
//!
//! This module contains rendering functions for the various diff view types:
//! - Plan diff: side-by-side query plan trees
//! - Schema diff: table column comparison
//! - Profile diff: execution timing comparison with delta highlighting
//! - Data diff: side-by-side result tables

use egui::{Color32, RichText};
use enya_datafusion::PlanNode;

use super::diff::{DiffRow, DiffRowPair, RowDiffStatus, compute_detailed_diff};
use super::types::{ColumnDiffStatus, DiffQueryResult, ProfileRow};
use crate::components::util::{OverlayColors, render_split_header, render_split_panels};
use crate::ui::semantic_icons::status;
use crate::ui::theme::AppTheme;

// =============================================================================
// Plan Diff Rendering
// =============================================================================

/// Render plan diff content (side-by-side plan trees).
pub fn render_plan_diff_content(ui: &mut egui::Ui, theme: AppTheme, diff_result: &DiffQueryResult) {
    let text_primary = theme.text_primary();
    let text_secondary = theme.text_secondary();
    let available_height = ui.available_height().max(300.0);
    let colors = OverlayColors::new(theme);

    // Header with connection names
    render_split_header(
        ui,
        &diff_result.left_name,
        &diff_result.right_name,
        text_primary,
        text_primary,
        colors.separator,
    );

    // Side-by-side plan trees
    let content_height = (available_height - 40.0).max(200.0);
    let left_plan = diff_result.left_plan.clone();
    let right_plan = diff_result.right_plan.clone();

    render_split_panels(
        ui,
        content_height,
        colors.separator,
        "sql_diff_plan",
        |ui| {
            if let Some(plan) = &left_plan {
                render_plan_tree(ui, theme, plan, 0);
            } else {
                ui.label(
                    RichText::new("No plan data")
                        .color(text_secondary)
                        .italics(),
                );
            }
        },
        |ui| {
            if let Some(plan) = &right_plan {
                render_plan_tree(ui, theme, plan, 0);
            } else {
                ui.label(
                    RichText::new("No plan data")
                        .color(text_secondary)
                        .italics(),
                );
            }
        },
    );
}

/// Render a simple plan tree (non-interactive, for diff view).
fn render_plan_tree(ui: &mut egui::Ui, theme: AppTheme, node: &PlanNode, depth: usize) {
    let text_secondary = theme.text_secondary();
    let indent = depth as f32 * 16.0;

    ui.horizontal(|ui| {
        ui.add_space(indent);

        // Operator name with color based on category
        let category = enya_datafusion::OperatorCategory::from_operator(&node.operator);
        let color = theme.plan_color(category.color_index());

        ui.label(
            RichText::new(&node.operator)
                .color(color)
                .strong()
                .size(12.0),
        );

        // Metrics if available
        if let Some(metrics) = &node.metrics {
            ui.label(
                RichText::new(format!(
                    " ({}, {}r)",
                    enya_datafusion::format_duration(metrics.elapsed_time),
                    metrics.output_rows
                ))
                .color(text_secondary)
                .size(10.0),
            );
        }
    });

    // Description
    if !node.description.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(indent + 16.0);
            ui.label(
                RichText::new(&node.description)
                    .color(text_secondary)
                    .size(10.0),
            );
        });
    }

    // Recursively render children
    for child in &node.children {
        render_plan_tree(ui, theme, child, depth + 1);
    }
}

// =============================================================================
// Schema Diff Rendering
// =============================================================================

/// Render schema diff content (unified table showing column differences).
pub fn render_schema_diff_content(
    ui: &mut egui::Ui,
    theme: AppTheme,
    diff_result: &DiffQueryResult,
) {
    let text_primary = theme.text_primary();
    let text_secondary = theme.text_secondary();
    let available_width = ui.available_width();
    let available_height = ui.available_height().max(300.0);
    let colors = OverlayColors::new(theme);

    let Some(schema_diff) = &diff_result.schema_diff else {
        ui.label(
            RichText::new("No schema diff data available")
                .color(text_secondary)
                .italics(),
        );
        return;
    };

    // Stats summary
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "{} matching  {} changed  {} removed  {} added",
                schema_diff.matching,
                schema_diff.changed,
                schema_diff.left_only,
                schema_diff.right_only
            ))
            .color(text_secondary)
            .size(10.0),
        );
    });
    ui.add_space(4.0);

    // Column widths - proportional to available width (25%, 30%, 30%, 15%)
    let usable_width = (available_width - 24.0).max(400.0);
    let col_widths = [
        usable_width * 0.25, // Column name
        usable_width * 0.30, // Left type
        usable_width * 0.30, // Right type
        usable_width * 0.15, // Status
    ];
    let row_height = 22.0;

    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.add_sized(
            [col_widths[0], row_height],
            egui::Label::new(
                RichText::new("Column")
                    .color(text_primary)
                    .strong()
                    .size(11.0),
            ),
        );
        ui.add_sized(
            [col_widths[1], row_height],
            egui::Label::new(
                RichText::new(&diff_result.left_name)
                    .color(theme.diff_removed_text())
                    .strong()
                    .size(11.0),
            ),
        );
        ui.add_sized(
            [col_widths[2], row_height],
            egui::Label::new(
                RichText::new(&diff_result.right_name)
                    .color(theme.diff_added_text())
                    .strong()
                    .size(11.0),
            ),
        );
        ui.add_sized(
            [col_widths[3], row_height],
            egui::Label::new(
                RichText::new("Status")
                    .color(text_primary)
                    .strong()
                    .size(11.0),
            ),
        );
    });

    // Separator
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.cursor().top(),
        egui::Stroke::new(1.0, colors.separator),
    );
    ui.add_space(2.0);

    // Scrollable column rows
    let content_height = (available_height - 80.0).max(200.0);
    egui::ScrollArea::vertical()
        .id_salt("sql_schema_diff_rows")
        .max_height(content_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().spacing.item_spacing.y = 0.0;

            for col in &schema_diff.columns {
                // Determine row background and status display based on column status
                let (bg_color, status_text, status_color) = match &col.status {
                    ColumnDiffStatus::Matching => {
                        (Color32::TRANSPARENT, "✓", theme.semantic_success())
                    }
                    ColumnDiffStatus::Changed => (
                        theme.semantic_warning().gamma_multiply(0.1),
                        "changed",
                        theme.semantic_warning(),
                    ),
                    ColumnDiffStatus::LeftOnly => (
                        theme.diff_removed_bg(),
                        "removed",
                        theme.diff_removed_text(),
                    ),
                    ColumnDiffStatus::RightOnly => {
                        (theme.diff_added_bg(), "added", theme.diff_added_text())
                    }
                };

                // Row frame
                egui::Frame::new()
                    .fill(bg_color)
                    .inner_margin(egui::Margin::symmetric(0, 2))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);

                            // Column name
                            ui.add_sized(
                                [col_widths[0], row_height],
                                egui::Label::new(
                                    RichText::new(&col.name).color(text_primary).size(11.0),
                                ),
                            );

                            // Left type
                            let left_type = col
                                .left_type
                                .as_ref()
                                .map(|t| {
                                    let nullable = col
                                        .left_nullable
                                        .map(|n| if n { " NULL" } else { " NOT NULL" })
                                        .unwrap_or("");
                                    format!("{t}{nullable}")
                                })
                                .unwrap_or_else(|| "—".to_string());
                            ui.add_sized(
                                [col_widths[1], row_height],
                                egui::Label::new(
                                    RichText::new(&left_type)
                                        .color(if col.left_type.is_some() {
                                            text_secondary
                                        } else {
                                            text_secondary.gamma_multiply(0.5)
                                        })
                                        .size(10.0),
                                ),
                            );

                            // Right type
                            let right_type = col
                                .right_type
                                .as_ref()
                                .map(|t| {
                                    let nullable = col
                                        .right_nullable
                                        .map(|n| if n { " NULL" } else { " NOT NULL" })
                                        .unwrap_or("");
                                    format!("{t}{nullable}")
                                })
                                .unwrap_or_else(|| "—".to_string());
                            ui.add_sized(
                                [col_widths[2], row_height],
                                egui::Label::new(
                                    RichText::new(&right_type)
                                        .color(if col.right_type.is_some() {
                                            text_secondary
                                        } else {
                                            text_secondary.gamma_multiply(0.5)
                                        })
                                        .size(10.0),
                                ),
                            );

                            // Status
                            ui.add_sized(
                                [col_widths[3], row_height],
                                egui::Label::new(
                                    RichText::new(status_text).color(status_color).size(10.0),
                                ),
                            );
                        });
                    });
            }
        });
}

// =============================================================================
// Profile Diff Rendering
// =============================================================================

/// Render profile diff content (side-by-side trees with timing deltas).
pub fn render_profile_diff_content(
    ui: &mut egui::Ui,
    theme: AppTheme,
    diff_result: &DiffQueryResult,
) {
    let text_secondary = theme.text_secondary();
    let available_height = ui.available_height().max(300.0);
    let available_width = ui.available_width();

    // Side-by-side layout
    let separator_width = 2.0;
    let side_width = ((available_width - separator_width) / 2.0).max(1.0);

    // Side-by-side scrolling content
    let content_height = (available_height - 80.0).max(200.0);

    egui::ScrollArea::vertical()
        .id_salt("sql_profile_diff_split")
        .max_height(content_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(available_width);
            ui.set_max_width(available_width);

            if let Some(left_plan) = &diff_result.left_plan {
                render_split_profile_tree(
                    ui,
                    theme,
                    left_plan,
                    diff_result.right_plan.as_ref(),
                    side_width,
                );
            } else if let Some(right_plan) = &diff_result.right_plan {
                render_split_profile_tree(ui, theme, right_plan, None, side_width);
            } else {
                ui.label(
                    RichText::new("No plan data available")
                        .color(text_secondary)
                        .italics(),
                );
            }
        });
}

/// Render side-by-side profile trees like git diff.
fn render_split_profile_tree(
    ui: &mut egui::Ui,
    theme: AppTheme,
    left_node: &PlanNode,
    right_root: Option<&PlanNode>,
    side_width: f32,
) {
    let mut paired_rows: Vec<(Option<ProfileRow>, Option<ProfileRow>)> = Vec::new();
    build_paired_profile_rows(left_node, right_root, 0, &mut paired_rows);

    let text_secondary = theme.text_secondary();
    let row_height = 28.0;
    let separator_width = 2.0;
    let total_width = side_width * 2.0 + separator_width;

    for (left_row, right_row) in &paired_rows {
        // Main operator row
        ui.horizontal(|ui| {
            ui.set_min_width(total_width);
            ui.set_max_width(total_width);

            // Left side panel
            egui::Frame::new()
                .fill(Color32::TRANSPARENT)
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(side_width, row_height));
                    ui.set_max_width(side_width);
                    ui.horizontal(|ui| {
                        render_profile_row(ui, theme, left_row.as_ref(), true, side_width);
                    });
                });

            // Center separator line
            let rect = ui.available_rect_before_wrap();
            ui.painter().vline(
                rect.left() + 1.0,
                rect.y_range(),
                egui::Stroke::new(1.0, theme.border_default()),
            );
            ui.add_space(separator_width);

            // Right side panel
            egui::Frame::new()
                .fill(Color32::TRANSPARENT)
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(side_width, row_height));
                    ui.set_max_width(side_width);
                    ui.horizontal(|ui| {
                        render_profile_row(ui, theme, right_row.as_ref(), false, side_width);
                    });
                });
        });

        // Description row (if present) - show on both sides
        let left_desc = left_row
            .as_ref()
            .filter(|r| !r.description.is_empty())
            .map(|r| (r.description.as_str(), r.depth));
        let right_desc = right_row
            .as_ref()
            .filter(|r| !r.description.is_empty())
            .map(|r| r.description.as_str());

        if left_desc.is_some() || right_desc.is_some() {
            let depth = left_desc.map(|(_, d)| d).unwrap_or(0);
            let indent = 16.0 + depth as f32 * 16.0;

            ui.horizontal(|ui| {
                ui.set_min_width(total_width);
                ui.set_max_width(total_width);

                // Left description
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(side_width, 16.0));
                        ui.set_max_width(side_width);
                        ui.add_space(indent);
                        if let Some((desc_text, _)) = left_desc {
                            ui.label(
                                RichText::new(desc_text)
                                    .color(text_secondary.gamma_multiply(0.6))
                                    .size(10.0),
                            );
                        }
                    });

                ui.add_space(separator_width);

                // Right description
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(side_width, 16.0));
                        ui.set_max_width(side_width);
                        ui.add_space(indent);
                        if let Some(desc_text) = right_desc {
                            ui.label(
                                RichText::new(desc_text)
                                    .color(text_secondary.gamma_multiply(0.6))
                                    .size(10.0),
                            );
                        }
                    });
            });
        }
    }
}

/// Render a single row in the split profile view.
fn render_profile_row(
    ui: &mut egui::Ui,
    theme: AppTheme,
    row: Option<&ProfileRow>,
    is_left: bool,
    side_width: f32,
) {
    let text_secondary = theme.text_secondary();

    let Some(row) = row else {
        // Empty row placeholder
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(side_width - 8.0, 24.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 0.0, theme.bg_base().gamma_multiply(0.3));
        return;
    };

    let indent = row.depth as f32 * 16.0;

    // Delta calculation: how much slower/faster is THIS side compared to OTHER side
    let delta_ms = row
        .other_time_ms
        .map(|other| other as i64 - row.time_ms as i64);

    let is_this_side_slower = delta_ms.map(|d| d < -5).unwrap_or(false);
    let is_this_side_faster = delta_ms.map(|d| d > 5).unwrap_or(false);

    // Determine highlighting based on which side and whether it's significant
    let (should_highlight_red, should_highlight_green) = if is_left {
        (is_this_side_slower, false)
    } else {
        (false, is_this_side_faster)
    };

    // Background color
    let bg_color = if should_highlight_red {
        theme.diff_removed_bg().gamma_multiply(0.4)
    } else if should_highlight_green {
        theme.diff_added_bg().gamma_multiply(0.4)
    } else {
        Color32::TRANSPARENT
    };

    // Gutter stripe color
    let gutter_color = if should_highlight_red {
        theme.diff_removed_text()
    } else if should_highlight_green {
        theme.diff_added_text()
    } else if is_left {
        theme.diff_removed_bg().gamma_multiply(0.3)
    } else {
        theme.diff_added_bg().gamma_multiply(0.3)
    };

    // Draw row background
    let row_rect = ui.available_rect_before_wrap();
    let bg_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(side_width - 4.0, 24.0));
    ui.painter().rect_filled(bg_rect, 2.0, bg_color);

    // Draw gutter stripe
    let gutter_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(3.0, 24.0));
    ui.painter().rect_filled(gutter_rect, 0.0, gutter_color);

    ui.add_space(8.0 + indent);

    // Tree connector
    if row.depth > 0 {
        ui.label(
            RichText::new("└")
                .color(text_secondary.gamma_multiply(0.3))
                .size(10.0),
        );
        ui.add_space(2.0);
    }

    // Operator name with category color
    let category = enya_datafusion::OperatorCategory::from_operator(&row.operator);
    let op_color = theme.plan_color(category.color_index());
    ui.label(
        RichText::new(&row.operator)
            .color(op_color)
            .strong()
            .size(11.0),
    );

    ui.add_space(8.0);

    // Timing display
    ui.label(
        RichText::new(format!("{}ms", row.time_ms))
            .color(text_secondary)
            .size(11.0),
    );

    // Delta badge - only show on the SLOWER side
    if let Some(delta) = delta_ms {
        if is_this_side_slower && delta.abs() > 5 {
            ui.add_space(6.0);
            let diff = delta.abs();
            ui.label(
                RichText::new(format!("+{diff}ms"))
                    .color(theme.semantic_error())
                    .size(10.0)
                    .strong(),
            );
        }
    }

    // Row count (compact)
    if row.rows > 0 {
        ui.add_space(6.0);
        ui.label(
            RichText::new(format!("{}rows", enya_datafusion::format_rows(row.rows)))
                .color(text_secondary.gamma_multiply(0.5))
                .size(9.0),
        );
    }
}

/// Build paired rows from two plan trees for side-by-side rendering.
fn build_paired_profile_rows(
    left_node: &PlanNode,
    right_root: Option<&PlanNode>,
    depth: usize,
    rows: &mut Vec<(Option<ProfileRow>, Option<ProfileRow>)>,
) {
    let right_node = find_matching_node(left_node, right_root);

    let left_time_ms = left_node
        .metrics
        .as_ref()
        .map(|m| m.elapsed_time.as_millis() as u64)
        .unwrap_or(0);
    let right_time_ms = right_node
        .and_then(|n| n.metrics.as_ref())
        .map(|m| m.elapsed_time.as_millis() as u64);

    let left_rows = left_node
        .metrics
        .as_ref()
        .map(|m| m.output_rows)
        .unwrap_or(0);
    let right_rows = right_node
        .and_then(|n| n.metrics.as_ref())
        .map(|m| m.output_rows)
        .unwrap_or(0);

    let left_row = ProfileRow {
        operator: left_node.operator.clone(),
        description: left_node.description.clone(),
        depth,
        time_ms: left_time_ms,
        other_time_ms: right_time_ms,
        rows: left_rows,
    };

    let right_row = right_node.map(|rn| ProfileRow {
        operator: rn.operator.clone(),
        description: rn.description.clone(),
        depth,
        time_ms: right_time_ms.unwrap_or(0),
        other_time_ms: Some(left_time_ms),
        rows: right_rows,
    });

    rows.push((Some(left_row), right_row));

    for child in &left_node.children {
        let right_child =
            right_node.and_then(|rn| rn.children.iter().find(|rc| rc.operator == child.operator));
        build_paired_profile_rows(child, right_child, depth + 1, rows);
    }
}

/// Find matching node in the other plan tree by operator name.
fn find_matching_node<'a>(node: &PlanNode, other: Option<&'a PlanNode>) -> Option<&'a PlanNode> {
    let other = other?;
    if other.operator == node.operator {
        Some(other)
    } else {
        None
    }
}

// =============================================================================
// Data Diff Rendering
// =============================================================================

/// Render data diff content (side-by-side tables with row highlighting).
pub fn render_data_diff_content(ui: &mut egui::Ui, theme: AppTheme, diff_result: &DiffQueryResult) {
    let text_primary = theme.text_primary();
    let text_secondary = theme.text_secondary();
    let available_width = ui.available_width();
    let available_height = ui.available_height().max(300.0);
    let side_width = ((available_width - 12.0) / 2.0).max(1.0);
    let colors = OverlayColors::new(theme);

    // Compute detailed diff with paired rows
    let table_diff = compute_detailed_diff(
        diff_result.left_schema.as_ref(),
        &diff_result.left_batches,
        diff_result.right_schema.as_ref(),
        &diff_result.right_batches,
    );

    // Schema mismatch warning
    if !diff_result.schemas_match
        && diff_result.left_schema.is_some()
        && diff_result.right_schema.is_some()
    {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("{} Schemas don't match", status::WARNING))
                    .color(theme.semantic_warning())
                    .size(11.0),
            );
        });
        ui.add_space(4.0);
    }

    // Row counts for headers
    let left_rows: usize = diff_result.left_batches.iter().map(|b| b.num_rows()).sum();
    let right_rows: usize = diff_result.right_batches.iter().map(|b| b.num_rows()).sum();

    // Diff stats summary
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "{} matching  {} left only  {} right only",
                table_diff.stats.matching, table_diff.stats.left_only, table_diff.stats.right_only
            ))
            .color(text_secondary)
            .size(10.0),
        );
    });
    ui.add_space(4.0);

    // Column headers with connection names
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(side_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&diff_result.left_name)
                        .color(theme.diff_removed_text())
                        .strong(),
                );
                ui.label(
                    RichText::new(format!("({left_rows} rows)"))
                        .color(text_secondary)
                        .size(10.0),
                );
            },
        );
        ui.add_space(4.0);
        ui.allocate_ui_with_layout(
            egui::vec2(side_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&diff_result.right_name)
                        .color(theme.diff_added_text())
                        .strong(),
                );
                ui.label(
                    RichText::new(format!("({right_rows} rows)"))
                        .color(text_secondary)
                        .size(10.0),
                );
            },
        );
    });

    // Separator below headers
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.cursor().top(),
        egui::Stroke::new(1.0, colors.separator),
    );
    ui.add_space(2.0);

    // Calculate dimensions
    let content_height = (available_height - 80.0).max(200.0);
    let num_cols = table_diff.columns.len().max(1);
    let col_width = ((side_width - 16.0) / num_cols as f32).clamp(60.0, 120.0);
    let row_height = 18.0;

    // Render column headers
    ui.horizontal(|ui| {
        // Left header
        ui.allocate_ui_with_layout(
            egui::vec2(side_width, row_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(4.0);
                for col in &table_diff.columns {
                    let display_name = if col.len() > 12 {
                        format!("{}…", &col[..11])
                    } else {
                        col.clone()
                    };
                    ui.add_sized(
                        [col_width, row_height],
                        egui::Label::new(
                            RichText::new(display_name)
                                .color(text_primary)
                                .strong()
                                .size(10.0),
                        ),
                    );
                }
            },
        );
        // Right header
        ui.allocate_ui_with_layout(
            egui::vec2(side_width, row_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(4.0);
                for col in &table_diff.columns {
                    let display_name = if col.len() > 12 {
                        format!("{}…", &col[..11])
                    } else {
                        col.clone()
                    };
                    ui.add_sized(
                        [col_width, row_height],
                        egui::Label::new(
                            RichText::new(display_name)
                                .color(text_primary)
                                .strong()
                                .size(10.0),
                        ),
                    );
                }
            },
        );
    });

    // Separator below column headers
    ui.painter().hline(
        ui.available_rect_before_wrap().x_range(),
        ui.cursor().top(),
        egui::Stroke::new(1.0, theme.border_default()),
    );

    // Scrollable paired rows
    egui::ScrollArea::vertical()
        .id_salt("sql_diff_paired_rows")
        .max_height(content_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().spacing.item_spacing.y = 0.0;

            let max_rows = 100;
            for pair in table_diff.rows.iter().take(max_rows) {
                render_diff_row_pair(ui, theme, pair, side_width, col_width, row_height);
            }

            if table_diff.rows.len() > max_rows {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("… {}+ rows", table_diff.rows.len()))
                        .color(text_secondary)
                        .italics()
                        .size(9.0),
                );
            }
        });
}

/// Render a single paired row in the diff view.
fn render_diff_row_pair(
    ui: &mut egui::Ui,
    theme: AppTheme,
    pair: &DiffRowPair,
    side_width: f32,
    col_width: f32,
    row_height: f32,
) {
    let text_secondary = theme.text_secondary();
    let empty_bg = theme.bg_base().gamma_multiply(0.7);

    // Determine colors based on row status
    let (left_bg, left_text, right_bg, right_text) = match (&pair.left, &pair.right) {
        (Some(left), Some(_right)) => {
            if left.status == RowDiffStatus::Matching {
                (None, text_secondary, None, text_secondary)
            } else {
                (
                    Some(theme.diff_removed_bg()),
                    theme.diff_removed_text(),
                    Some(theme.diff_added_bg()),
                    theme.diff_added_text(),
                )
            }
        }
        (Some(_), None) => (
            Some(theme.diff_removed_bg()),
            theme.diff_removed_text(),
            Some(empty_bg),
            text_secondary,
        ),
        (None, Some(_)) => (
            Some(empty_bg),
            text_secondary,
            Some(theme.diff_added_bg()),
            theme.diff_added_text(),
        ),
        (None, None) => return,
    };

    // Allocate the full row
    let (row_rect, _) = ui.allocate_exact_size(
        egui::vec2(side_width * 2.0 + 8.0, row_height),
        egui::Sense::hover(),
    );

    // Draw backgrounds
    let left_rect = egui::Rect::from_min_size(row_rect.min, egui::vec2(side_width, row_height));
    let right_rect = egui::Rect::from_min_size(
        egui::pos2(row_rect.min.x + side_width + 8.0, row_rect.min.y),
        egui::vec2(side_width, row_height),
    );

    if let Some(bg) = left_bg {
        ui.painter().rect_filled(left_rect, 0.0, bg);
    }
    if let Some(bg) = right_bg {
        ui.painter().rect_filled(right_rect, 0.0, bg);
    }

    // Draw vertical separator
    ui.painter().vline(
        row_rect.min.x + side_width + 4.0,
        egui::Rangef::new(row_rect.top(), row_rect.bottom()),
        egui::Stroke::new(1.0, theme.border_default().gamma_multiply(0.5)),
    );

    // Render left side content
    if let Some(row) = &pair.left {
        render_diff_row_cells_at(ui, row, left_rect, col_width, left_text);
    }

    // Render right side content
    if let Some(row) = &pair.right {
        render_diff_row_cells_at(ui, row, right_rect, col_width, right_text);
    }
}

/// Render cells for a single diff row at a specific position.
fn render_diff_row_cells_at(
    ui: &mut egui::Ui,
    row: &DiffRow,
    rect: egui::Rect,
    col_width: f32,
    text_color: egui::Color32,
) {
    let mut x = rect.left() + 4.0;
    let y_center = rect.center().y;

    for value in &row.values {
        let max_chars = (col_width / 7.0) as usize;
        let display_value = if value.chars().count() > max_chars && max_chars > 3 {
            let truncated: String = value.chars().take(max_chars - 1).collect();
            format!("{truncated}…")
        } else {
            value.clone()
        };

        ui.painter().text(
            egui::pos2(x, y_center),
            egui::Align2::LEFT_CENTER,
            display_value,
            egui::FontId::monospace(9.0),
            text_color,
        );

        x += col_width;
    }
}
