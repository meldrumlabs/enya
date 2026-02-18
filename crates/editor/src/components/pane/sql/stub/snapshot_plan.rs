//! Simplified plan tree renderer for `SnapshotPlanNode`.
//!
//! Operates on the pure-Rust `SnapshotPlanNode` type (no DataFusion dependency).
//! All nodes are expanded — no interactive fold/unfold in read-only mode.

use egui::{Color32, RichText, Stroke};
use enya_config::{SnapshotOperatorMetrics, SnapshotPlanNode};

use super::format_ms;
use crate::ui::semantic_icons::{nav, status};
use crate::ui::theme::AppTheme;

/// A flattened plan node for rendering.
struct FlatNode {
    depth: usize,
    operator: String,
    description: String,
    metrics: Option<SnapshotOperatorMetrics>,
    has_children: bool,
    is_bottleneck: bool,
}

/// Render a plan tree from a `SnapshotPlanNode`.
pub(super) fn render_plan_tree(ui: &mut egui::Ui, root: &SnapshotPlanNode, theme: AppTheme) {
    // Flatten tree
    let mut nodes = Vec::new();
    let total_time_ms = find_total_time(root);
    let bottleneck_ms = find_bottleneck_time(root);
    flatten_node(root, 0, &mut nodes, bottleneck_ms);

    if nodes.is_empty() {
        ui.label(
            RichText::new("Empty plan")
                .color(theme.text_secondary())
                .size(11.0),
        );
        return;
    }

    let text_secondary = theme.text_secondary();
    let guide_color = theme.border_default().gamma_multiply(0.5);
    let row_height = 28.0;

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let available_width = ui.available_width().max(600.0);

            for (vis_idx, node) in nodes.iter().enumerate() {
                let indent = node.depth as f32 * 20.0;

                ui.horizontal(|ui| {
                    let start_x = ui.cursor().left();
                    let row_top = ui.cursor().top();

                    // Vertical guides
                    for d in 1..=node.depth {
                        let x = start_x + (d as f32 - 0.5) * 20.0;
                        let has_sibling_below = nodes
                            .iter()
                            .skip(vis_idx + 1)
                            .any(|n| n.depth == d && n.depth <= node.depth);
                        if has_sibling_below || d == node.depth {
                            ui.painter().line_segment(
                                [egui::pos2(x, row_top), egui::pos2(x, row_top + row_height)],
                                Stroke::new(1.0, guide_color),
                            );
                        }
                    }

                    // Horizontal connector
                    if node.depth > 0 {
                        let x_start = start_x + (node.depth as f32 - 0.5) * 20.0;
                        let x_end = start_x + indent - 4.0;
                        let y = row_top + row_height / 2.0;
                        ui.painter().line_segment(
                            [egui::pos2(x_start, y), egui::pos2(x_end, y)],
                            Stroke::new(1.0, guide_color),
                        );
                    }

                    // Indent
                    ui.add_space(indent);

                    // Content
                    egui::Frame::new()
                        .corner_radius(2.0)
                        .inner_margin(4.0)
                        .show(ui, |ui| {
                            ui.set_min_width(available_width - indent - 40.0);

                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;

                                // Expand indicator
                                if node.has_children {
                                    ui.label(
                                        RichText::new(nav::EXPAND).color(text_secondary).size(12.0),
                                    );
                                } else {
                                    ui.allocate_space(egui::vec2(14.0, 1.0));
                                }

                                // Bottleneck indicator
                                if node.is_bottleneck {
                                    ui.label(
                                        RichText::new(status::WARNING)
                                            .color(theme.semantic_warning())
                                            .size(12.0),
                                    );
                                }

                                // Operator name
                                let op_color = operator_color(&node.operator, &theme);
                                ui.label(
                                    RichText::new(&node.operator)
                                        .color(op_color)
                                        .strong()
                                        .size(13.0),
                                );

                                // Metrics
                                if let Some(metrics) = &node.metrics {
                                    ui.add_space(4.0);

                                    // Time with percentage
                                    let time_str = format_ms(metrics.elapsed_time_ms);
                                    let pct = if total_time_ms > 0 {
                                        (metrics.elapsed_time_ms as f64 / total_time_ms as f64
                                            * 100.0) as u32
                                    } else {
                                        0
                                    };
                                    let time_color = if pct > 50 {
                                        theme.semantic_warning()
                                    } else {
                                        text_secondary
                                    };
                                    ui.label(
                                        RichText::new(format!("{time_str} ({pct}%)"))
                                            .color(time_color)
                                            .size(11.0),
                                    );

                                    // Mini progress bar
                                    let bar_width = 40.0;
                                    let bar_height = 4.0;
                                    let (bar_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(bar_width, bar_height),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(bar_rect, 2.0, theme.bg_base());
                                    let fill_width = (pct as f32 / 100.0).min(1.0) * bar_width;
                                    if fill_width > 0.0 {
                                        let fill_rect = egui::Rect::from_min_size(
                                            bar_rect.min,
                                            egui::vec2(fill_width, bar_height),
                                        );
                                        ui.painter().rect_filled(fill_rect, 2.0, time_color);
                                    }

                                    // Rows
                                    if metrics.output_rows > 0 {
                                        ui.label(
                                            RichText::new(format!(
                                                "{} rows",
                                                format_rows(metrics.output_rows)
                                            ))
                                            .color(text_secondary)
                                            .size(11.0),
                                        );
                                    }

                                    // Memory
                                    if metrics.memory_bytes > 0 {
                                        ui.label(
                                            RichText::new(format_bytes(metrics.memory_bytes))
                                                .color(text_secondary)
                                                .size(11.0),
                                        );
                                    }
                                }
                            });

                            // Description on next line
                            if !node.description.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.allocate_space(egui::vec2(14.0, 1.0));
                                    ui.label(
                                        RichText::new(&node.description)
                                            .color(text_secondary)
                                            .size(11.0),
                                    );
                                });
                            }
                        });
                });
            }
        });
}

/// Flatten a plan node tree into a list for rendering.
fn flatten_node(
    node: &SnapshotPlanNode,
    depth: usize,
    nodes: &mut Vec<FlatNode>,
    bottleneck_ms: u64,
) {
    let has_children = !node.children.is_empty();
    let is_bottleneck = node
        .metrics
        .as_ref()
        .is_some_and(|m| m.elapsed_time_ms == bottleneck_ms && bottleneck_ms > 0);

    nodes.push(FlatNode {
        depth,
        operator: node.operator.clone(),
        description: node.description.clone(),
        metrics: node.metrics.clone(),
        has_children,
        is_bottleneck,
    });

    for child in &node.children {
        flatten_node(child, depth + 1, nodes, bottleneck_ms);
    }
}

/// Find the total (max) execution time in the plan tree.
fn find_total_time(node: &SnapshotPlanNode) -> u64 {
    let own = node.metrics.as_ref().map_or(0, |m| m.elapsed_time_ms);
    let child_max = node.children.iter().map(find_total_time).max().unwrap_or(0);
    own.max(child_max)
}

/// Find the bottleneck (max elapsed_time_ms) in the plan tree.
fn find_bottleneck_time(node: &SnapshotPlanNode) -> u64 {
    let own = node.metrics.as_ref().map_or(0, |m| m.elapsed_time_ms);
    let child_max = node
        .children
        .iter()
        .map(find_bottleneck_time)
        .max()
        .unwrap_or(0);
    own.max(child_max)
}

/// Get color for an operator based on simple string matching.
fn operator_color(operator: &str, theme: &AppTheme) -> Color32 {
    let index = operator_color_index(operator);
    if index == 11 {
        theme.text_secondary()
    } else {
        theme.plan_color(index)
    }
}

/// Map operator name to a color index (matching enya_datafusion::OperatorCategory).
fn operator_color_index(operator: &str) -> usize {
    let op = operator.to_lowercase();
    if op.contains("scan") || op.contains("parquet") || op.contains("csv") || op.contains("json") {
        0 // Scan
    } else if op.contains("filter") {
        1 // Filter
    } else if op.contains("join") {
        2 // Join
    } else if op.contains("aggregate") || op.contains("group") {
        3 // Aggregate
    } else if op.contains("sort") || op.contains("topk") {
        4 // Sort
    } else if op.contains("projection") {
        5 // Project
    } else if op.contains("repartition") || op.contains("coalesce") {
        6 // Exchange
    } else if op.contains("union") {
        8 // Union
    } else if op.ends_with("exec") {
        10 // Other Exec
    } else {
        11 // Not an exec node
    }
}

/// Format bytes for display.
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Format row counts for display.
fn format_rows(rows: u64) -> String {
    if rows < 1_000 {
        format!("{rows}")
    } else if rows < 1_000_000 {
        format!("{:.1}K", rows as f64 / 1_000.0)
    } else {
        format!("{:.1}M", rows as f64 / 1_000_000.0)
    }
}
