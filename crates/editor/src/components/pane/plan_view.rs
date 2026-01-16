//! Query Plan Visualization Components.
//!
//! Provides three views for visualizing SQL query execution plans:
//! - **TreeView**: Vim-navigable hierarchical tree with bottleneck highlighting
//! - **TimelineView**: Horizontal bar chart showing operator execution times
//! - **DiffView**: Side-by-side comparison of two plans (optimized vs unoptimized)

#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use egui::{Color32, Key, RichText, Stroke, StrokeKind};
use egui_plot::{Bar, BarChart, Plot};
use enya_datafusion::{OperatorMetrics, PlanNode};
use rustc_hash::FxHashSet;

use crate::ui::semantic_icons::{action, diff, nav, status};
use crate::ui::theme::AppTheme;

/// Format a duration as a human-readable string.
fn format_duration(d: Duration) -> String {
    let micros = d.as_micros();
    if micros < 1000 {
        format!("{micros}µs")
    } else if micros < 1_000_000 {
        format!("{:.2}ms", micros as f64 / 1000.0)
    } else {
        format!("{:.2}s", micros as f64 / 1_000_000.0)
    }
}

/// Format bytes as a human-readable string.
fn format_bytes(bytes: usize) -> String {
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

/// Get color for an operator type.
fn operator_color(operator: &str, theme: &AppTheme) -> Color32 {
    // Color-code operators by category
    if operator.contains("Scan") || operator.contains("Read") {
        theme.chart_color(0) // Blue - I/O operations
    } else if operator.contains("Filter") || operator.contains("Limit") {
        theme.chart_color(1) // Green - filtering
    } else if operator.contains("Join") {
        theme.chart_color(2) // Orange - joins (often expensive)
    } else if operator.contains("Aggregate") || operator.contains("Group") {
        theme.chart_color(3) // Purple - aggregations
    } else if operator.contains("Sort") || operator.contains("Order") {
        theme.chart_color(4) // Red - sorting
    } else if operator.contains("Project") {
        theme.chart_color(5) // Teal - projections
    } else if operator.contains("Hash") {
        theme.chart_color(6) // Yellow - hash operations
    } else {
        theme.text_secondary() // Default
    }
}

/// A flattened node for navigation in the tree view.
#[derive(Debug, Clone)]
struct FlatNode {
    /// Index into the flat list.
    #[allow(dead_code)] // Used for debugging
    index: usize,
    /// Depth in the tree (0 = root).
    depth: usize,
    /// The operator name.
    operator: String,
    /// Short description.
    #[allow(dead_code)] // Reserved for tooltip display
    description: String,
    /// Execution metrics (if available).
    metrics: Option<OperatorMetrics>,
    /// Whether this node has children.
    has_children: bool,
    /// Whether this node is expanded.
    expanded: bool,
    /// Parent index (None for root).
    parent: Option<usize>,
    /// Child indices.
    children: Vec<usize>,
    /// Whether this is a bottleneck.
    is_bottleneck: bool,
}

/// Which plan view mode is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanViewMode {
    /// Hierarchical tree view (default).
    #[default]
    Tree,
    /// Timeline/Gantt view.
    Timeline,
    /// Side-by-side diff view.
    Diff,
}

/// State for the plan tree view with vim navigation.
pub struct PlanTreeView {
    /// Flattened nodes for navigation.
    nodes: Vec<FlatNode>,
    /// Currently selected node index.
    selected: usize,
    /// Set of expanded node indices.
    expanded: FxHashSet<usize>,
    /// Current theme.
    theme: AppTheme,
    /// Total execution time for percentage calculations.
    total_time: Duration,
    /// Index of the bottleneck node.
    bottleneck_index: Option<usize>,
}

impl PlanTreeView {
    /// Create a new tree view from a plan node.
    pub fn new(root: &PlanNode, theme: AppTheme) -> Self {
        let mut nodes = Vec::new();
        let mut expanded = FxHashSet::default();
        let total_time = Self::calculate_total_time(root);
        let bottleneck_time = Self::find_bottleneck_time(root);

        Self::flatten_node(root, 0, None, &mut nodes, &mut expanded, bottleneck_time);

        // Find bottleneck index
        let bottleneck_index = nodes.iter().position(|n| n.is_bottleneck);

        Self {
            nodes,
            selected: 0,
            expanded,
            theme,
            total_time,
            bottleneck_index,
        }
    }

    /// Calculate total execution time from a plan tree.
    fn calculate_total_time(node: &PlanNode) -> Duration {
        let self_time = node
            .metrics
            .as_ref()
            .map_or(Duration::ZERO, |m| m.elapsed_time);
        let child_time: Duration = node.children.iter().map(Self::calculate_total_time).sum();
        self_time.max(child_time) // Use max since children run in parallel
    }

    /// Find the maximum elapsed time (bottleneck) in the tree.
    fn find_bottleneck_time(node: &PlanNode) -> Duration {
        let self_time = node
            .metrics
            .as_ref()
            .map_or(Duration::ZERO, |m| m.elapsed_time);
        let max_child = node
            .children
            .iter()
            .map(Self::find_bottleneck_time)
            .max()
            .unwrap_or(Duration::ZERO);
        self_time.max(max_child)
    }

    /// Flatten a plan node tree into a navigable list.
    fn flatten_node(
        node: &PlanNode,
        depth: usize,
        parent: Option<usize>,
        nodes: &mut Vec<FlatNode>,
        expanded: &mut FxHashSet<usize>,
        bottleneck_time: Duration,
    ) {
        let index = nodes.len();
        let has_children = !node.children.is_empty();

        // Expand all nodes by default
        if has_children {
            expanded.insert(index);
        }

        // Check if this is the bottleneck
        let is_bottleneck = node
            .metrics
            .as_ref()
            .is_some_and(|m| m.elapsed_time == bottleneck_time && !bottleneck_time.is_zero());

        let flat = FlatNode {
            index,
            depth,
            operator: node.operator.clone(),
            description: node.description.clone(),
            metrics: node.metrics.clone(),
            has_children,
            expanded: has_children,
            parent,
            children: Vec::new(),
            is_bottleneck,
        };
        nodes.push(flat);

        // Recursively flatten children
        let child_indices: Vec<usize> = node
            .children
            .iter()
            .map(|child| {
                let child_index = nodes.len();
                Self::flatten_node(
                    child,
                    depth + 1,
                    Some(index),
                    nodes,
                    expanded,
                    bottleneck_time,
                );
                child_index
            })
            .collect();

        // Update parent with child indices
        nodes[index].children = child_indices;
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Handle keyboard input for vim navigation.
    /// Returns true if the input was handled.
    pub fn handle_input(&mut self, ui: &egui::Ui) -> bool {
        let mut handled = false;

        ui.input(|input| {
            // j - move down
            if input.key_pressed(Key::J) {
                self.move_down();
                handled = true;
            }
            // k - move up
            if input.key_pressed(Key::K) {
                self.move_up();
                handled = true;
            }
            // h - collapse / go to parent
            if input.key_pressed(Key::H) {
                self.collapse_or_parent();
                handled = true;
            }
            // l - expand / go to first child
            if input.key_pressed(Key::L) {
                self.expand_or_child();
                handled = true;
            }
            // gg - go to top
            if input.key_pressed(Key::G) && !input.modifiers.shift {
                // Simple single-g handling (would need state for gg)
                self.selected = 0;
                handled = true;
            }
            // G - go to bottom
            if input.key_pressed(Key::G) && input.modifiers.shift {
                self.go_to_bottom();
                handled = true;
            }
            // b - jump to bottleneck
            if input.key_pressed(Key::B) && !input.modifiers.any() {
                if let Some(idx) = self.bottleneck_index {
                    self.selected = idx;
                    handled = true;
                }
            }
            // Space - toggle expand/collapse
            if input.key_pressed(Key::Space) {
                self.toggle_expand();
                handled = true;
            }
        });

        handled
    }

    /// Move selection down to next visible node.
    fn move_down(&mut self) {
        let visible = self.visible_indices();
        if let Some(pos) = visible.iter().position(|&i| i == self.selected) {
            if pos + 1 < visible.len() {
                self.selected = visible[pos + 1];
            }
        }
    }

    /// Move selection up to previous visible node.
    fn move_up(&mut self) {
        let visible = self.visible_indices();
        if let Some(pos) = visible.iter().position(|&i| i == self.selected) {
            if pos > 0 {
                self.selected = visible[pos - 1];
            }
        }
    }

    /// Collapse current node or go to parent.
    fn collapse_or_parent(&mut self) {
        if self.expanded.contains(&self.selected) {
            self.expanded.remove(&self.selected);
            if let Some(node) = self.nodes.get_mut(self.selected) {
                node.expanded = false;
            }
        } else if let Some(parent) = self.nodes.get(self.selected).and_then(|n| n.parent) {
            self.selected = parent;
        }
    }

    /// Expand current node or go to first child.
    fn expand_or_child(&mut self) {
        if let Some(node) = self.nodes.get(self.selected) {
            if node.has_children {
                if !self.expanded.contains(&self.selected) {
                    self.expanded.insert(self.selected);
                    if let Some(n) = self.nodes.get_mut(self.selected) {
                        n.expanded = true;
                    }
                } else if let Some(&first_child) = node.children.first() {
                    self.selected = first_child;
                }
            }
        }
    }

    /// Go to the last visible node.
    fn go_to_bottom(&mut self) {
        let visible = self.visible_indices();
        if let Some(&last) = visible.last() {
            self.selected = last;
        }
    }

    /// Toggle expand/collapse of current node.
    fn toggle_expand(&mut self) {
        if self
            .nodes
            .get(self.selected)
            .is_some_and(|n| n.has_children)
        {
            if self.expanded.contains(&self.selected) {
                self.expanded.remove(&self.selected);
                if let Some(n) = self.nodes.get_mut(self.selected) {
                    n.expanded = false;
                }
            } else {
                self.expanded.insert(self.selected);
                if let Some(n) = self.nodes.get_mut(self.selected) {
                    n.expanded = true;
                }
            }
        }
    }

    /// Get indices of visible nodes (respecting collapsed state).
    fn visible_indices(&self) -> Vec<usize> {
        let mut visible = Vec::new();
        self.collect_visible(0, &mut visible);
        visible
    }

    fn collect_visible(&self, index: usize, visible: &mut Vec<usize>) {
        if index >= self.nodes.len() {
            return;
        }
        visible.push(index);

        if let Some(node) = self.nodes.get(index) {
            if self.expanded.contains(&index) {
                for &child in &node.children {
                    self.collect_visible(child, visible);
                }
            }
        }
    }

    /// Render the tree view.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let bg_selected = self.theme.bg_elevated();
        let accent = self.theme.accent_primary();

        // Handle keyboard input
        self.handle_input(ui);

        // Header with keybindings hint
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} Query Plan", nav::TREE))
                    .color(text_primary)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("j/k:nav  h/l:fold  b:bottleneck  Space:toggle")
                        .color(text_secondary)
                        .small(),
                );
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Scrollable tree
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let visible = self.visible_indices();

                for &index in &visible {
                    if let Some(node) = self.nodes.get(index) {
                        let is_selected = index == self.selected;
                        let indent = node.depth as f32 * 20.0;

                        // Row background for selected item
                        let response = ui.horizontal(|ui| {
                            ui.add_space(indent);

                            // Expand/collapse indicator
                            if node.has_children {
                                let icon = if self.expanded.contains(&index) {
                                    nav::COLLAPSE
                                } else {
                                    nav::EXPAND
                                };
                                ui.label(RichText::new(icon).color(text_secondary));
                            } else {
                                ui.add_space(16.0);
                            }

                            // Bottleneck indicator
                            if node.is_bottleneck {
                                ui.label(
                                    RichText::new(status::WARNING)
                                        .color(self.theme.semantic_warning()),
                                );
                            }

                            // Operator name with color
                            let op_color = operator_color(&node.operator, &self.theme);
                            ui.label(RichText::new(&node.operator).color(op_color).strong());

                            // Metrics inline
                            if let Some(metrics) = &node.metrics {
                                ui.add_space(8.0);
                                // Time
                                let time_str = format_duration(metrics.elapsed_time);
                                let pct = if !self.total_time.is_zero() {
                                    (metrics.elapsed_time.as_nanos() as f64
                                        / self.total_time.as_nanos() as f64
                                        * 100.0) as u32
                                } else {
                                    0
                                };
                                ui.label(
                                    RichText::new(format!("{time_str} ({pct}%)"))
                                        .color(text_secondary)
                                        .small(),
                                );

                                // Rows
                                if metrics.output_rows > 0 {
                                    ui.label(
                                        RichText::new(format!("{}r", metrics.output_rows))
                                            .color(text_secondary)
                                            .small(),
                                    );
                                }

                                // Memory
                                if metrics.memory_bytes > 0 {
                                    ui.label(
                                        RichText::new(format_bytes(metrics.memory_bytes))
                                            .color(text_secondary)
                                            .small(),
                                    );
                                }
                            }
                        });

                        // Draw selection background
                        if is_selected {
                            let rect = response.response.rect;
                            ui.painter().rect_filled(rect, 2.0, bg_selected);
                            ui.painter().rect_stroke(
                                rect,
                                2.0,
                                Stroke::new(1.0, accent),
                                StrokeKind::Outside,
                            );
                        }
                    }
                }
            });
    }
}

/// Timeline view showing operator execution times as horizontal bars.
pub struct TimelineView {
    /// Plan nodes with timing data.
    nodes: Vec<TimelineNode>,
    /// Current theme.
    theme: AppTheme,
    /// Total execution time.
    total_time: Duration,
}

#[derive(Debug, Clone)]
struct TimelineNode {
    /// Operator name.
    operator: String,
    /// Execution time.
    elapsed_time: Duration,
    /// Depth in tree (for ordering).
    #[allow(dead_code)] // Reserved for future hierarchical display
    depth: usize,
}

impl TimelineView {
    /// Create a new timeline view from a plan node.
    pub fn new(root: &PlanNode, theme: AppTheme) -> Self {
        let mut nodes = Vec::new();
        Self::collect_nodes(root, 0, &mut nodes);

        // Sort by time descending for Gantt-style visualization
        nodes.sort_by(|a, b| b.elapsed_time.cmp(&a.elapsed_time));

        let total_time = nodes
            .iter()
            .map(|n| n.elapsed_time)
            .max()
            .unwrap_or(Duration::ZERO);

        Self {
            nodes,
            theme,
            total_time,
        }
    }

    fn collect_nodes(node: &PlanNode, depth: usize, nodes: &mut Vec<TimelineNode>) {
        if let Some(metrics) = &node.metrics {
            if !metrics.elapsed_time.is_zero() {
                nodes.push(TimelineNode {
                    operator: node.operator.clone(),
                    elapsed_time: metrics.elapsed_time,
                    depth,
                });
            }
        }

        for child in &node.children {
            Self::collect_nodes(child, depth + 1, nodes);
        }
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Render the timeline view.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();

        // Header
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} Execution Timeline", action::CHART))
                    .color(text_primary)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("Total: {}", format_duration(self.total_time)))
                        .color(text_secondary),
                );
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        if self.nodes.is_empty() {
            ui.label(
                RichText::new("No timing data available. Use EXPLAIN ANALYZE.")
                    .color(text_secondary)
                    .italics(),
            );
            return;
        }

        // Create bars for the chart
        let bars: Vec<Bar> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let time_ms = node.elapsed_time.as_secs_f64() * 1000.0;
                let color = operator_color(&node.operator, &self.theme);
                Bar::new(i as f64, time_ms)
                    .name(&node.operator)
                    .fill(color)
                    .stroke(Stroke::new(1.0, color))
                    .horizontal()
            })
            .collect();

        let chart = BarChart::new("timeline", bars)
            .element_formatter(Box::new(|bar, _chart| format!("{:.2}ms", bar.value)));

        // Calculate height based on number of operators
        let chart_height = (self.nodes.len() as f32 * 25.0).clamp(100.0, 400.0);

        Plot::new("timeline_plot")
            .height(chart_height)
            .show_axes([true, true])
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .y_axis_label("Operator")
            .x_axis_label("Time (ms)")
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(chart);
            });

        // Legend below the chart
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for node in &self.nodes {
                let color = operator_color(&node.operator, &self.theme);
                ui.horizontal(|ui| {
                    // Colored dot
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 5.0, color);

                    ui.label(
                        RichText::new(format!(
                            "{}: {}",
                            node.operator,
                            format_duration(node.elapsed_time)
                        ))
                        .color(text_secondary)
                        .small(),
                    );
                });
                ui.add_space(8.0);
            }
        });
    }
}

/// Diff view for comparing two query plans side by side.
pub struct DiffView {
    /// Left plan (typically unoptimized).
    left: Option<PlanTreeView>,
    /// Right plan (typically optimized).
    right: Option<PlanTreeView>,
    /// Left plan label.
    left_label: String,
    /// Right plan label.
    right_label: String,
    /// Current theme.
    theme: AppTheme,
}

impl DiffView {
    /// Create a new diff view.
    pub fn new(theme: AppTheme) -> Self {
        Self {
            left: None,
            right: None,
            left_label: "Logical Plan".to_string(),
            right_label: "Physical Plan".to_string(),
            theme,
        }
    }

    /// Set the left (unoptimized) plan.
    pub fn set_left(&mut self, plan: &PlanNode, label: &str) {
        self.left = Some(PlanTreeView::new(plan, self.theme));
        self.left_label = label.to_string();
    }

    /// Set the right (optimized) plan.
    pub fn set_right(&mut self, plan: &PlanNode, label: &str) {
        self.right = Some(PlanTreeView::new(plan, self.theme));
        self.right_label = label.to_string();
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        if let Some(left) = &mut self.left {
            left.set_theme(theme);
        }
        if let Some(right) = &mut self.right {
            right.set_theme(theme);
        }
    }

    /// Render the diff view.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();

        // Header
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} Plan Comparison", diff::DIFF))
                    .color(text_primary)
                    .strong(),
            );
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        let available_width = ui.available_width();
        let half_width = (available_width - 16.0) / 2.0;

        ui.horizontal(|ui| {
            // Left pane
            egui::Frame::new()
                .fill(self.theme.bg_surface())
                .stroke(Stroke::new(1.0, self.theme.border_default()))
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(half_width);
                    ui.set_min_height(200.0);

                    ui.label(RichText::new(&self.left_label).color(text_primary).strong());
                    ui.separator();

                    if let Some(left) = &mut self.left {
                        left.show(ui);
                    } else {
                        ui.label(
                            RichText::new("No plan loaded")
                                .color(text_secondary)
                                .italics(),
                        );
                    }
                });

            ui.add_space(8.0);

            // Right pane
            egui::Frame::new()
                .fill(self.theme.bg_surface())
                .stroke(Stroke::new(1.0, self.theme.border_default()))
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(half_width);
                    ui.set_min_height(200.0);

                    ui.label(
                        RichText::new(&self.right_label)
                            .color(text_primary)
                            .strong(),
                    );
                    ui.separator();

                    if let Some(right) = &mut self.right {
                        right.show(ui);
                    } else {
                        ui.label(
                            RichText::new("No plan loaded")
                                .color(text_secondary)
                                .italics(),
                        );
                    }
                });
        });
    }
}

/// Container that manages all plan view modes.
pub struct PlanViewer {
    /// Current view mode.
    pub mode: PlanViewMode,
    /// Tree view.
    tree_view: Option<PlanTreeView>,
    /// Timeline view.
    timeline_view: Option<TimelineView>,
    /// Diff view.
    diff_view: DiffView,
    /// Current theme.
    theme: AppTheme,
}

impl PlanViewer {
    /// Create a new plan viewer.
    pub fn new(theme: AppTheme) -> Self {
        Self {
            mode: PlanViewMode::Tree,
            tree_view: None,
            timeline_view: None,
            diff_view: DiffView::new(theme),
            theme,
        }
    }

    /// Load a plan for visualization.
    pub fn load_plan(&mut self, plan: &PlanNode) {
        self.tree_view = Some(PlanTreeView::new(plan, self.theme));
        self.timeline_view = Some(TimelineView::new(plan, self.theme));
    }

    /// Load two plans for diff comparison.
    pub fn load_diff(
        &mut self,
        left: &PlanNode,
        right: &PlanNode,
        left_label: &str,
        right_label: &str,
    ) {
        self.diff_view.set_left(left, left_label);
        self.diff_view.set_right(right, right_label);
        self.mode = PlanViewMode::Diff;
    }

    /// Check if a plan is loaded.
    pub fn has_plan(&self) -> bool {
        self.tree_view.is_some()
    }

    /// Clear the loaded plan.
    pub fn clear(&mut self) {
        self.tree_view = None;
        self.timeline_view = None;
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        if let Some(tree) = &mut self.tree_view {
            tree.set_theme(theme);
        }
        if let Some(timeline) = &mut self.timeline_view {
            timeline.set_theme(theme);
        }
        self.diff_view.set_theme(theme);
    }

    /// Render the plan viewer.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Mode selector tabs
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            if ui
                .selectable_label(
                    self.mode == PlanViewMode::Tree,
                    format!("{} Tree", nav::TREE),
                )
                .clicked()
            {
                self.mode = PlanViewMode::Tree;
            }

            if ui
                .selectable_label(
                    self.mode == PlanViewMode::Timeline,
                    format!("{} Timeline", action::CHART),
                )
                .clicked()
            {
                self.mode = PlanViewMode::Timeline;
            }

            if ui
                .selectable_label(
                    self.mode == PlanViewMode::Diff,
                    format!("{} Diff", diff::DIFF),
                )
                .clicked()
            {
                self.mode = PlanViewMode::Diff;
            }
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Render the active view
        match self.mode {
            PlanViewMode::Tree => {
                if let Some(tree) = &mut self.tree_view {
                    tree.show(ui);
                } else {
                    self.show_empty_state(ui);
                }
            }
            PlanViewMode::Timeline => {
                if let Some(timeline) = &mut self.timeline_view {
                    timeline.show(ui);
                } else {
                    self.show_empty_state(ui);
                }
            }
            PlanViewMode::Diff => {
                self.diff_view.show(ui);
            }
        }
    }

    fn show_empty_state(&self, ui: &mut egui::Ui) {
        let text_secondary = self.theme.text_secondary();

        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                RichText::new("No query plan loaded")
                    .color(text_secondary)
                    .size(16.0),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Use :explain or :analyze to view a query plan")
                    .color(text_secondary)
                    .small(),
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::FxHashMap;

    fn create_test_plan() -> PlanNode {
        PlanNode {
            operator: "ProjectionExec".to_string(),
            description: "a, b, c".to_string(),
            properties: FxHashMap::default(),
            metrics: Some(OperatorMetrics {
                output_rows: 100,
                elapsed_time: Duration::from_millis(10),
                memory_bytes: 1024,
                spill_count: 0,
                spill_bytes: 0,
            }),
            children: vec![PlanNode {
                operator: "FilterExec".to_string(),
                description: "x > 10".to_string(),
                properties: FxHashMap::default(),
                metrics: Some(OperatorMetrics {
                    output_rows: 100,
                    elapsed_time: Duration::from_millis(5),
                    memory_bytes: 512,
                    spill_count: 0,
                    spill_bytes: 0,
                }),
                children: vec![PlanNode {
                    operator: "ParquetExec".to_string(),
                    description: "file.parquet".to_string(),
                    properties: FxHashMap::default(),
                    metrics: Some(OperatorMetrics {
                        output_rows: 1000,
                        elapsed_time: Duration::from_millis(50),
                        memory_bytes: 4096,
                        spill_count: 0,
                        spill_bytes: 0,
                    }),
                    children: vec![],
                }],
            }],
        }
    }

    #[test]
    fn test_tree_view_creation() {
        let plan = create_test_plan();
        let theme = AppTheme::default();
        let tree = PlanTreeView::new(&plan, theme);

        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.nodes[0].operator, "ProjectionExec");
        assert_eq!(tree.nodes[1].operator, "FilterExec");
        assert_eq!(tree.nodes[2].operator, "ParquetExec");
    }

    #[test]
    fn test_bottleneck_detection() {
        let plan = create_test_plan();
        let theme = AppTheme::default();
        let tree = PlanTreeView::new(&plan, theme);

        // ParquetExec should be the bottleneck (50ms vs 10ms and 5ms)
        assert!(tree.bottleneck_index.is_some());
        assert_eq!(
            tree.nodes[tree.bottleneck_index.unwrap()].operator,
            "ParquetExec"
        );
    }

    #[test]
    fn test_timeline_view_creation() {
        let plan = create_test_plan();
        let theme = AppTheme::default();
        let timeline = TimelineView::new(&plan, theme);

        assert_eq!(timeline.nodes.len(), 3);
        // Should be sorted by time descending
        assert_eq!(timeline.nodes[0].operator, "ParquetExec");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_micros(500)), "500µs");
        assert_eq!(format_duration(Duration::from_micros(1500)), "1.50ms");
        assert_eq!(format_duration(Duration::from_millis(1500)), "1.50s");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1572864), "1.5 MB");
    }
}
