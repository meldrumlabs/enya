//! Query Plan Visualization Components.
//!
//! Provides three views for visualizing SQL query execution plans:
//! - **TreeView**: Vim-navigable hierarchical tree with bottleneck highlighting
//! - **StatsView**: Aggregate dashboard with key metrics at a glance
//! - **WaterfallView**: Gantt-style visualization showing parallel execution

use std::time::Duration;

use egui::{Color32, Key, RichText, Stroke, StrokeKind};
use enya_datafusion::{OperatorCategory, OperatorMetrics, PlanNode};
use rustc_hash::FxHashSet;

use crate::components::util::render_key_badge;
use crate::ui::semantic_icons::{action, diff, nav, status, time};
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

/// Get color for an operator type using the theme's plan palette.
fn operator_color(operator: &str, theme: &AppTheme) -> Color32 {
    let category = OperatorCategory::from_operator(operator);
    if category == OperatorCategory::Other {
        theme.text_secondary() // Default for non-Exec nodes
    } else {
        theme.plan_color(category.color_index())
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
    /// Aggregate stats dashboard.
    Stats,
    /// Gantt-style waterfall showing parallel execution.
    Waterfall,
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
    /// Whether a workspace overlay is blocking keyboard input.
    overlay_blocks_input: bool,
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
            overlay_blocks_input: false,
        }
    }

    /// Set whether a workspace overlay is blocking keyboard input.
    pub fn set_overlay_blocks_input(&mut self, blocks: bool) {
        self.overlay_blocks_input = blocks;
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
        // Skip input handling if a workspace overlay is blocking
        if self.overlay_blocks_input {
            return false;
        }

        let mut handled = false;

        // Use input_mut to consume key events and prevent propagation
        ui.ctx().input_mut(|input| {
            // j - move down
            if input.consume_key(egui::Modifiers::NONE, Key::J) {
                self.move_down();
                handled = true;
            }
            // k - move up
            if input.consume_key(egui::Modifiers::NONE, Key::K) {
                self.move_up();
                handled = true;
            }
            // h - collapse / go to parent
            if input.consume_key(egui::Modifiers::NONE, Key::H) {
                self.collapse_or_parent();
                handled = true;
            }
            // l - expand / go to first child
            if input.consume_key(egui::Modifiers::NONE, Key::L) {
                self.expand_or_child();
                handled = true;
            }
            // g - go to top
            if input.consume_key(egui::Modifiers::NONE, Key::G) {
                self.selected = 0;
                handled = true;
            }
            // G - go to bottom
            if input.consume_key(egui::Modifiers::SHIFT, Key::G) {
                self.go_to_bottom();
                handled = true;
            }
            // b - jump to bottleneck
            if input.consume_key(egui::Modifiers::NONE, Key::B) {
                if let Some(idx) = self.bottleneck_index {
                    self.selected = idx;
                    handled = true;
                }
            }
            // Space - toggle expand/collapse
            if input.consume_key(egui::Modifiers::NONE, Key::Space) {
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
        let text_secondary = self.theme.text_secondary();
        let bg_selected = self.theme.bg_elevated();
        let accent = self.theme.accent_primary();

        // Handle keyboard input
        self.handle_input(ui);

        // Scrollable tree (vertical with horizontal scroll for long content)
        let guide_color = self.theme.border_default().gamma_multiply(0.5);
        let row_height = 28.0;

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let visible = self.visible_indices();
                let available_width = ui.available_width().max(600.0);

                for (vis_idx, &index) in visible.iter().enumerate() {
                    if let Some(node) = self.nodes.get(index) {
                        let is_selected = index == self.selected;
                        let indent = node.depth as f32 * 20.0;

                        // Build the content using egui's layout system
                        let row_response = ui.horizontal(|ui| {
                            // Draw tree connection lines
                            let start_x = ui.cursor().left();
                            let row_top = ui.cursor().top();

                            // Draw vertical guides for each depth level
                            for d in 1..=node.depth {
                                let x = start_x + (d as f32 - 0.5) * 20.0;
                                // Check if we need to continue the line (has siblings below)
                                let has_sibling_below =
                                    visible.iter().skip(vis_idx + 1).any(|&idx| {
                                        self.nodes
                                            .get(idx)
                                            .is_some_and(|n| n.depth == d && n.depth <= node.depth)
                                    });
                                if has_sibling_below || d == node.depth {
                                    ui.painter().line_segment(
                                        [
                                            egui::pos2(x, row_top),
                                            egui::pos2(x, row_top + row_height),
                                        ],
                                        Stroke::new(1.0, guide_color),
                                    );
                                }
                            }

                            // Draw horizontal connector to this node
                            if node.depth > 0 {
                                let x_start = start_x + (node.depth as f32 - 0.5) * 20.0;
                                let x_end = start_x + indent - 4.0;
                                let y = row_top + row_height / 2.0;
                                ui.painter().line_segment(
                                    [egui::pos2(x_start, y), egui::pos2(x_end, y)],
                                    Stroke::new(1.0, guide_color),
                                );
                            }

                            // Indent spacer
                            ui.add_space(indent);

                            // Content frame with selection highlight
                            let response = egui::Frame::new()
                                .fill(if is_selected {
                                    bg_selected
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .stroke(if is_selected {
                                    Stroke::new(1.0, accent)
                                } else {
                                    Stroke::NONE
                                })
                                .corner_radius(2.0)
                                .inner_margin(4.0)
                                .show(ui, |ui| {
                                    ui.set_min_width(available_width - indent - 40.0);

                                    // First line: icon, operator, metrics
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;

                                        // Expand/collapse indicator
                                        if node.has_children {
                                            let icon = if self.expanded.contains(&index) {
                                                nav::COLLAPSE
                                            } else {
                                                nav::EXPAND
                                            };
                                            ui.label(
                                                RichText::new(icon)
                                                    .color(text_secondary)
                                                    .size(12.0),
                                            );
                                        } else {
                                            ui.allocate_space(egui::vec2(14.0, 1.0));
                                        }

                                        // Bottleneck indicator
                                        if node.is_bottleneck {
                                            ui.label(
                                                RichText::new(status::WARNING)
                                                    .color(self.theme.semantic_warning())
                                                    .size(12.0),
                                            );
                                        }

                                        // Operator name
                                        let op_color = operator_color(&node.operator, &self.theme);
                                        ui.label(
                                            RichText::new(&node.operator)
                                                .color(op_color)
                                                .strong()
                                                .size(13.0),
                                        );

                                        // Metrics on same line
                                        if let Some(metrics) = &node.metrics {
                                            ui.add_space(4.0);

                                            // Time with percentage
                                            let time_str = format_duration(metrics.elapsed_time);
                                            let pct = if !self.total_time.is_zero() {
                                                (metrics.elapsed_time.as_nanos() as f64
                                                    / self.total_time.as_nanos() as f64
                                                    * 100.0)
                                                    as u32
                                            } else {
                                                0
                                            };
                                            let time_color = if pct > 50 {
                                                self.theme.semantic_warning()
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
                                            // Background
                                            ui.painter().rect_filled(
                                                bar_rect,
                                                2.0,
                                                self.theme.bg_base(),
                                            );
                                            // Fill
                                            let fill_width =
                                                (pct as f32 / 100.0).min(1.0) * bar_width;
                                            if fill_width > 0.0 {
                                                let fill_rect = egui::Rect::from_min_size(
                                                    bar_rect.min,
                                                    egui::vec2(fill_width, bar_height),
                                                );
                                                ui.painter()
                                                    .rect_filled(fill_rect, 2.0, time_color);
                                            }

                                            // Rows
                                            if metrics.output_rows > 0 {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{}r",
                                                        metrics.output_rows
                                                    ))
                                                    .color(text_secondary)
                                                    .size(11.0),
                                                );
                                            }

                                            // Memory
                                            if metrics.memory_bytes > 0 {
                                                ui.label(
                                                    RichText::new(format_bytes(
                                                        metrics.memory_bytes,
                                                    ))
                                                    .color(text_secondary)
                                                    .size(11.0),
                                                );
                                            }
                                        }
                                    });

                                    // Description on next line(s) if present, with indent
                                    if !node.description.is_empty() {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.allocate_space(egui::vec2(14.0, 1.0)); // Align with operator
                                            ui.label(
                                                RichText::new(&node.description)
                                                    .color(text_secondary)
                                                    .size(11.0),
                                            );
                                        });
                                    }
                                });

                            let _ = response;
                        });
                        let _ = row_response;
                    }
                }
            });
    }
}

/// Stats entry for a single operator.
#[derive(Debug, Clone)]
struct StatsEntry {
    /// Operator name.
    operator: String,
    /// Execution time.
    elapsed_time: Duration,
    /// Output rows.
    output_rows: usize,
    /// Memory bytes.
    memory_bytes: usize,
    /// Whether this is the bottleneck.
    is_bottleneck: bool,
}

/// Category stats for grouping operators.
#[derive(Debug, Clone, Default)]
struct CategoryStats {
    count: usize,
    total_time: Duration,
    total_rows: usize,
}

/// Stats view showing aggregate metrics dashboard.
pub struct StatsView {
    /// All operator entries sorted by time.
    entries: Vec<StatsEntry>,
    /// Stats by category.
    category_stats: Vec<(String, CategoryStats)>,
    /// Current theme.
    theme: AppTheme,
    /// Total execution time.
    total_time: Duration,
    /// Total operator count.
    operator_count: usize,
    /// Total rows processed.
    total_rows: usize,
    /// Peak memory usage.
    peak_memory: usize,
    /// Bottleneck operator name.
    bottleneck: Option<String>,
}

impl StatsView {
    /// Create a new stats view from a plan node.
    pub fn new(root: &PlanNode, theme: AppTheme) -> Self {
        let mut entries = Vec::new();
        let bottleneck_time = Self::find_bottleneck_time(root);
        Self::collect_entries(root, &mut entries, bottleneck_time);

        // Sort by time descending
        entries.sort_by(|a, b| b.elapsed_time.cmp(&a.elapsed_time));

        // Calculate totals
        let total_time = entries
            .iter()
            .map(|e| e.elapsed_time)
            .max()
            .unwrap_or(Duration::ZERO);
        let operator_count = entries.len();
        let total_rows = entries.iter().map(|e| e.output_rows).max().unwrap_or(0);
        let peak_memory = entries.iter().map(|e| e.memory_bytes).max().unwrap_or(0);
        let bottleneck = entries
            .iter()
            .find(|e| e.is_bottleneck)
            .map(|e| e.operator.clone());

        // Build category stats
        let mut categories: rustc_hash::FxHashMap<String, CategoryStats> =
            rustc_hash::FxHashMap::default();
        for entry in &entries {
            let category = Self::categorize_operator(&entry.operator);
            let stats = categories.entry(category).or_default();
            stats.count += 1;
            stats.total_time += entry.elapsed_time;
            stats.total_rows += entry.output_rows;
        }

        let mut category_stats: Vec<_> = categories.into_iter().collect();
        category_stats.sort_by(|a, b| b.1.total_time.cmp(&a.1.total_time));

        Self {
            entries,
            category_stats,
            theme,
            total_time,
            operator_count,
            total_rows,
            peak_memory,
            bottleneck,
        }
    }

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

    fn collect_entries(node: &PlanNode, entries: &mut Vec<StatsEntry>, bottleneck_time: Duration) {
        let metrics = node.metrics.as_ref();
        let elapsed_time = metrics.map_or(Duration::ZERO, |m| m.elapsed_time);
        let is_bottleneck = !bottleneck_time.is_zero() && elapsed_time == bottleneck_time;

        entries.push(StatsEntry {
            operator: node.operator.clone(),
            elapsed_time,
            output_rows: metrics.map_or(0, |m| m.output_rows),
            memory_bytes: metrics.map_or(0, |m| m.memory_bytes),
            is_bottleneck,
        });

        for child in &node.children {
            Self::collect_entries(child, entries, bottleneck_time);
        }
    }

    fn categorize_operator(operator: &str) -> String {
        OperatorCategory::from_operator(operator)
            .display_name()
            .to_string()
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Render the stats view.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();

        if self.entries.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("No plan data available")
                        .color(text_secondary)
                        .size(14.0),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Use /analyze to see execution stats")
                        .color(text_secondary)
                        .small(),
                );
            });
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Summary cards row
                ui.horizontal(|ui| {
                    self.render_stat_card(
                        ui,
                        "Total Time",
                        &format_duration(self.total_time),
                        time::TIMER,
                    );
                    self.render_stat_card(
                        ui,
                        "Operators",
                        &self.operator_count.to_string(),
                        nav::TREE,
                    );
                    self.render_stat_card(
                        ui,
                        "Rows Out",
                        &format_rows(self.total_rows),
                        action::CHART,
                    );
                    self.render_stat_card(
                        ui,
                        "Peak Memory",
                        &format_bytes(self.peak_memory),
                        status::INFO,
                    );
                });

                ui.add_space(12.0);

                // Bottleneck warning
                if let Some(bottleneck) = &self.bottleneck {
                    egui::Frame::new()
                        .fill(self.theme.semantic_warning().gamma_multiply(0.15))
                        .stroke(Stroke::new(1.0, self.theme.semantic_warning()))
                        .corner_radius(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(status::WARNING)
                                        .color(self.theme.semantic_warning())
                                        .size(14.0),
                                );
                                ui.label(
                                    RichText::new(format!("Bottleneck: {bottleneck}"))
                                        .color(text_primary)
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "({})",
                                        format_duration(self.total_time)
                                    ))
                                    .color(text_secondary),
                                );
                            });
                        });
                    ui.add_space(12.0);
                }

                // Two-column layout
                ui.columns(2, |columns| {
                    // Left column: By Category
                    columns[0].vertical(|ui| {
                        ui.label(
                            RichText::new("By Category")
                                .color(text_primary)
                                .strong()
                                .size(13.0),
                        );
                        ui.add_space(4.0);

                        for (category, stats) in &self.category_stats {
                            let pct = if !self.total_time.is_zero() {
                                (stats.total_time.as_nanos() as f64
                                    / self.total_time.as_nanos() as f64
                                    * 100.0) as u32
                            } else {
                                0
                            };

                            ui.horizontal(|ui| {
                                let color = operator_color(category, &self.theme);
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(10.0, 10.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 2.0, color);

                                ui.label(
                                    RichText::new(format!("{}: {}", category, stats.count))
                                        .color(text_primary)
                                        .size(12.0),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "({}, {}%)",
                                        format_duration(stats.total_time),
                                        pct
                                    ))
                                    .color(text_secondary)
                                    .size(11.0),
                                );
                            });
                        }
                    });

                    // Right column: Top Slowest
                    columns[1].vertical(|ui| {
                        ui.label(
                            RichText::new("Top Slowest")
                                .color(text_primary)
                                .strong()
                                .size(13.0),
                        );
                        ui.add_space(4.0);

                        for (i, entry) in self.entries.iter().take(5).enumerate() {
                            let pct = if !self.total_time.is_zero() {
                                (entry.elapsed_time.as_nanos() as f64
                                    / self.total_time.as_nanos() as f64
                                    * 100.0) as u32
                            } else {
                                0
                            };

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{}.", i + 1))
                                        .color(text_secondary)
                                        .size(11.0),
                                );

                                if entry.is_bottleneck {
                                    ui.label(
                                        RichText::new(status::WARNING)
                                            .color(self.theme.semantic_warning())
                                            .size(11.0),
                                    );
                                }

                                let color = operator_color(&entry.operator, &self.theme);
                                ui.label(RichText::new(&entry.operator).color(color).size(12.0));
                                ui.label(
                                    RichText::new(format!(
                                        "{} ({}%)",
                                        format_duration(entry.elapsed_time),
                                        pct
                                    ))
                                    .color(text_secondary)
                                    .size(11.0),
                                );
                            });
                        }
                    });
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                // Full operator table
                ui.label(
                    RichText::new("All Operators")
                        .color(text_primary)
                        .strong()
                        .size(13.0),
                );
                ui.add_space(4.0);

                // Table header
                ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Operator")
                            .color(text_secondary)
                            .size(11.0)
                            .strong(),
                    );
                    ui.add_space(100.0);
                    ui.label(
                        RichText::new("Time")
                            .color(text_secondary)
                            .size(11.0)
                            .strong(),
                    );
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("Rows")
                            .color(text_secondary)
                            .size(11.0)
                            .strong(),
                    );
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("Memory")
                            .color(text_secondary)
                            .size(11.0)
                            .strong(),
                    );
                });

                for entry in &self.entries {
                    let pct = if !self.total_time.is_zero() {
                        (entry.elapsed_time.as_nanos() as f64 / self.total_time.as_nanos() as f64
                            * 100.0) as u32
                    } else {
                        0
                    };

                    ui.horizontal(|ui| {
                        ui.add_space(4.0);

                        if entry.is_bottleneck {
                            ui.label(
                                RichText::new(status::WARNING)
                                    .color(self.theme.semantic_warning())
                                    .size(11.0),
                            );
                        }

                        let color = operator_color(&entry.operator, &self.theme);
                        ui.label(RichText::new(&entry.operator).color(color).size(12.0));

                        // Pad to align columns (rough)
                        let op_len = entry.operator.len();
                        if op_len < 20 {
                            ui.add_space((20 - op_len) as f32 * 6.0);
                        }

                        ui.label(
                            RichText::new(format!(
                                "{} ({}%)",
                                format_duration(entry.elapsed_time),
                                pct
                            ))
                            .color(if pct > 50 {
                                self.theme.semantic_warning()
                            } else {
                                text_secondary
                            })
                            .size(11.0),
                        );

                        ui.add_space(20.0);
                        ui.label(
                            RichText::new(format_rows(entry.output_rows))
                                .color(text_secondary)
                                .size(11.0),
                        );

                        ui.add_space(20.0);
                        ui.label(
                            RichText::new(format_bytes(entry.memory_bytes))
                                .color(text_secondary)
                                .size(11.0),
                        );
                    });
                }
            });
    }

    fn render_stat_card(&self, ui: &mut egui::Ui, label: &str, value: &str, icon: &str) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let accent = self.theme.accent_primary();

        let response = egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .stroke(Stroke::new(1.0, self.theme.border_default()))
            .corner_radius(6.0)
            .inner_margin(egui::Margin {
                left: 12,
                right: 10,
                top: 8,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.set_min_width(90.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon).color(accent).size(12.0));
                        ui.label(RichText::new(label).color(text_secondary).size(11.0));
                    });
                    ui.label(RichText::new(value).color(text_primary).strong().size(16.0));
                });
            });

        // Draw accent stripe on left edge
        let rect = response.response.rect;
        let stripe_rect =
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height()));
        ui.painter().rect_filled(stripe_rect, 3.0, accent);
    }
}

/// Format row count with K/M suffixes.
fn format_rows(rows: usize) -> String {
    if rows < 1000 {
        rows.to_string()
    } else if rows < 1_000_000 {
        format!("{:.1}K", rows as f64 / 1000.0)
    } else {
        format!("{:.1}M", rows as f64 / 1_000_000.0)
    }
}

/// A node in the waterfall view with timing information.
#[derive(Debug, Clone)]
struct WaterfallNode {
    /// Operator name.
    operator: String,
    /// Depth in tree (for indentation).
    depth: usize,
    /// Start time relative to query start (estimated).
    start_time: Duration,
    /// End time relative to query start.
    end_time: Duration,
    /// Whether this is a bottleneck.
    is_bottleneck: bool,
    /// Output rows from this operator.
    #[allow(dead_code)] // Reserved for future tooltip/detail display
    output_rows: usize,
}

/// Waterfall view showing Gantt-style execution timeline.
pub struct WaterfallView {
    /// Nodes with timing data.
    nodes: Vec<WaterfallNode>,
    /// Current theme.
    theme: AppTheme,
    /// Total execution time.
    total_time: Duration,
    /// Selected node index.
    selected: usize,
    /// Whether a workspace overlay is blocking keyboard input.
    overlay_blocks_input: bool,
}

impl WaterfallView {
    /// Create a new waterfall view from a plan node.
    pub fn new(root: &PlanNode, theme: AppTheme) -> Self {
        let mut nodes = Vec::new();
        let total_time = Self::calculate_total_time(root);
        let bottleneck_time = Self::find_bottleneck_time(root);

        // Collect nodes with timing estimation
        Self::collect_nodes(root, 0, Duration::ZERO, &mut nodes, bottleneck_time);

        Self {
            nodes,
            theme,
            total_time,
            selected: 0,
            overlay_blocks_input: false,
        }
    }

    /// Set whether a workspace overlay is blocking keyboard input.
    pub fn set_overlay_blocks_input(&mut self, blocks: bool) {
        self.overlay_blocks_input = blocks;
    }

    /// Calculate total execution time from a plan tree.
    fn calculate_total_time(node: &PlanNode) -> Duration {
        let self_time = node
            .metrics
            .as_ref()
            .map_or(Duration::ZERO, |m| m.elapsed_time);
        let child_time: Duration = node.children.iter().map(Self::calculate_total_time).sum();
        self_time.max(child_time)
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

    /// Collect nodes in execution order (children first, then parent).
    fn collect_nodes(
        node: &PlanNode,
        depth: usize,
        parent_end: Duration,
        nodes: &mut Vec<WaterfallNode>,
        bottleneck_time: Duration,
    ) {
        let elapsed = node
            .metrics
            .as_ref()
            .map_or(Duration::ZERO, |m| m.elapsed_time);

        // For a pull-based execution model:
        // - Children execute first (they produce data)
        // - Parent consumes the output
        // So parent's end time = parent_end + elapsed
        // And children start relative to when data is needed

        // Estimate child execution times first
        let mut child_end = parent_end;
        for child in &node.children {
            Self::collect_nodes(child, depth + 1, parent_end, nodes, bottleneck_time);
            // Track when this subtree finishes
            let child_elapsed = child
                .metrics
                .as_ref()
                .map_or(Duration::ZERO, |m| m.elapsed_time);
            if child_elapsed > child_end.saturating_sub(parent_end) {
                child_end = parent_end + child_elapsed;
            }
        }

        // This operator starts after children are done (simplified model)
        let start_time = child_end;
        let end_time = start_time + elapsed;

        let is_bottleneck = node
            .metrics
            .as_ref()
            .is_some_and(|m| m.elapsed_time == bottleneck_time && !bottleneck_time.is_zero());

        let output_rows = node.metrics.as_ref().map_or(0, |m| m.output_rows);

        nodes.push(WaterfallNode {
            operator: node.operator.clone(),
            depth,
            start_time,
            end_time,
            is_bottleneck,
            output_rows,
        });
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Handle keyboard input.
    /// Returns true if the input was handled.
    pub fn handle_input(&mut self, ui: &egui::Ui) -> bool {
        // Skip input handling if a workspace overlay is blocking
        if self.overlay_blocks_input {
            return false;
        }

        let mut handled = false;

        // Use input_mut to consume key events and prevent propagation
        ui.ctx().input_mut(|input| {
            // j - move down
            if input.consume_key(egui::Modifiers::NONE, Key::J) {
                if self.selected < self.nodes.len().saturating_sub(1) {
                    self.selected += 1;
                }
                handled = true;
            }
            // k - move up
            if input.consume_key(egui::Modifiers::NONE, Key::K) {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                handled = true;
            }
            // g - go to top
            if input.consume_key(egui::Modifiers::NONE, Key::G) {
                self.selected = 0;
                handled = true;
            }
            // G - go to bottom
            if input.consume_key(egui::Modifiers::SHIFT, Key::G) {
                if !self.nodes.is_empty() {
                    self.selected = self.nodes.len() - 1;
                }
                handled = true;
            }
            // b - jump to bottleneck
            if input.consume_key(egui::Modifiers::NONE, Key::B) {
                if let Some(idx) = self.nodes.iter().position(|n| n.is_bottleneck) {
                    self.selected = idx;
                    handled = true;
                }
            }
        });

        handled
    }

    /// Render the waterfall view.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let bg_selected = self.theme.bg_elevated();
        let accent = self.theme.accent_primary();

        // Handle keyboard input
        self.handle_input(ui);

        if self.nodes.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("No timing data available")
                        .color(text_secondary)
                        .size(14.0),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Use /analyze to see execution waterfall")
                        .color(text_secondary)
                        .small(),
                );
            });
            return;
        }

        // Calculate layout dimensions
        let available_width = ui.available_width();
        let label_width = 200.0_f32.min(available_width * 0.35);
        let time_label_width = 70.0;
        let bar_area_width = (available_width - label_width - time_label_width - 24.0).max(100.0);
        let row_height = 28.0;

        // Time scale
        let total_micros = self.total_time.as_micros().max(1) as f64;

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Time axis header with proper spacing
                ui.horizontal(|ui| {
                    ui.add_space(label_width);

                    // Draw time markers at fixed positions
                    let time_fractions = [0.0, 0.25, 0.5, 0.75, 1.0];
                    let segment_width = bar_area_width / 4.0;

                    for (i, &frac) in time_fractions.iter().enumerate() {
                        let time_at = Duration::from_micros((total_micros * frac) as u64);
                        let label_text = format_duration(time_at);

                        if i == 0 {
                            // First label aligned left
                            ui.label(RichText::new(label_text).color(text_secondary).small());
                            ui.add_space(segment_width - 40.0);
                        } else if i == time_fractions.len() - 1 {
                            // Last label
                            ui.label(RichText::new(label_text).color(text_secondary).small());
                        } else {
                            // Middle labels centered in their segment
                            ui.label(RichText::new(label_text).color(text_secondary).small());
                            ui.add_space(segment_width - 40.0);
                        }
                    }
                });

                ui.add_space(4.0);

                // Draw vertical grid lines for time markers
                let grid_color = self.theme.border_default().gamma_multiply(0.3);
                let grid_top = ui.cursor().top();
                let grid_height = self.nodes.len() as f32 * row_height + 20.0;
                let grid_left = ui.cursor().left() + label_width;

                for i in 0..=4 {
                    let x = grid_left + (i as f32 * bar_area_width / 4.0);
                    ui.painter().line_segment(
                        [
                            egui::pos2(x, grid_top),
                            egui::pos2(x, grid_top + grid_height),
                        ],
                        Stroke::new(1.0, grid_color),
                    );
                }

                // Draw each operator row
                for (idx, node) in self.nodes.iter().enumerate() {
                    let is_selected = idx == self.selected;
                    let indent = node.depth as f32 * 16.0;

                    // Calculate bar positions
                    let bar_start =
                        (node.start_time.as_micros() as f64 / total_micros) as f32 * bar_area_width;
                    let bar_end =
                        (node.end_time.as_micros() as f64 / total_micros) as f32 * bar_area_width;
                    let bar_width = (bar_end - bar_start).max(6.0); // Minimum visible width

                    let op_color = operator_color(&node.operator, &self.theme);
                    let bar_color = if node.is_bottleneck {
                        self.theme.semantic_warning()
                    } else {
                        op_color
                    };

                    // Use a frame to draw selection background first
                    egui::Frame::new()
                        .fill(if is_selected {
                            bg_selected
                        } else {
                            Color32::TRANSPARENT
                        })
                        .stroke(if is_selected {
                            Stroke::new(1.0, accent)
                        } else {
                            Stroke::NONE
                        })
                        .corner_radius(2.0)
                        .inner_margin(egui::Margin::symmetric(4, 2))
                        .show(ui, |ui| {
                            ui.set_min_height(row_height);

                            ui.horizontal(|ui| {
                                // Indent
                                ui.add_space(indent);

                                // Fixed-width label area
                                ui.allocate_ui_with_layout(
                                    egui::vec2(label_width - indent - 8.0, row_height),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        // Bottleneck indicator
                                        if node.is_bottleneck {
                                            ui.label(
                                                RichText::new(status::WARNING)
                                                    .color(self.theme.semantic_warning())
                                                    .size(12.0),
                                            );
                                        }

                                        // Operator label with wrapping
                                        ui.label(
                                            RichText::new(&node.operator)
                                                .color(if is_selected {
                                                    op_color
                                                } else {
                                                    text_primary
                                                })
                                                .strong()
                                                .size(12.0),
                                        );
                                    },
                                );

                                // Bar area
                                ui.add_space(bar_start);

                                let (bar_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(bar_width, row_height - 8.0),
                                    egui::Sense::hover(),
                                );

                                // Draw bar with rounded corners
                                ui.painter().rect_filled(bar_rect, 4.0, bar_color);

                                // Selection ring around bar
                                if is_selected {
                                    ui.painter().rect_stroke(
                                        bar_rect.expand(2.0),
                                        5.0,
                                        Stroke::new(2.0, accent),
                                        StrokeKind::Outside,
                                    );
                                }

                                // Duration label after bar
                                let duration = node.end_time.saturating_sub(node.start_time);
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(format_duration(duration))
                                        .color(if is_selected {
                                            text_primary
                                        } else {
                                            text_secondary
                                        })
                                        .size(11.0),
                                );
                            });
                        });
                }

                // Legend
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal_wrapped(|ui| {
                    let categories = [
                        ("Scan", "Scan"),
                        ("Filter", "Filter"),
                        ("Join", "Join"),
                        ("Aggregate", "Aggregate"),
                        ("Sort", "Sort"),
                        ("Project", "Project"),
                    ];

                    for (label, pattern) in categories {
                        let color = operator_color(pattern, &self.theme);
                        ui.horizontal(|ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 2.0, color);
                            ui.label(RichText::new(label).color(text_secondary).small());
                        });
                        ui.add_space(12.0);
                    }
                });
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
    /// Stats view.
    stats_view: Option<StatsView>,
    /// Waterfall view.
    waterfall_view: Option<WaterfallView>,
    /// Current theme.
    theme: AppTheme,
    /// Total execution time (for header stats).
    total_time: Duration,
    /// Number of operators.
    operator_count: usize,
    /// Number of bottlenecks.
    bottleneck_count: usize,
    /// Whether a workspace overlay is blocking keyboard input.
    overlay_blocks_input: bool,
}

impl PlanViewer {
    /// Create a new plan viewer.
    pub fn new(theme: AppTheme) -> Self {
        Self {
            mode: PlanViewMode::Tree,
            tree_view: None,
            stats_view: None,
            waterfall_view: None,
            theme,
            total_time: Duration::ZERO,
            operator_count: 0,
            bottleneck_count: 0,
            overlay_blocks_input: false,
        }
    }

    /// Set whether a workspace overlay is blocking keyboard input.
    pub fn set_overlay_blocks_input(&mut self, blocks: bool) {
        self.overlay_blocks_input = blocks;
    }

    /// Load a plan for visualization.
    pub fn load_plan(&mut self, plan: &PlanNode) {
        self.tree_view = Some(PlanTreeView::new(plan, self.theme));
        self.stats_view = Some(StatsView::new(plan, self.theme));
        self.waterfall_view = Some(WaterfallView::new(plan, self.theme));

        // Calculate stats
        self.total_time = Self::calculate_total_time(plan);
        self.operator_count = Self::count_operators(plan);
        self.bottleneck_count = if self
            .tree_view
            .as_ref()
            .is_some_and(|t| t.bottleneck_index.is_some())
        {
            1
        } else {
            0
        };
    }

    /// Calculate total execution time from a plan tree.
    fn calculate_total_time(node: &PlanNode) -> Duration {
        let self_time = node
            .metrics
            .as_ref()
            .map_or(Duration::ZERO, |m| m.elapsed_time);
        let child_time: Duration = node.children.iter().map(Self::calculate_total_time).sum();
        self_time.max(child_time)
    }

    /// Count operators in the plan tree.
    fn count_operators(node: &PlanNode) -> usize {
        1 + node
            .children
            .iter()
            .map(Self::count_operators)
            .sum::<usize>()
    }

    /// Check if a plan is loaded.
    pub fn has_plan(&self) -> bool {
        self.tree_view.is_some()
    }

    /// Clear the loaded plan.
    pub fn clear(&mut self) {
        self.tree_view = None;
        self.stats_view = None;
        self.waterfall_view = None;
        self.total_time = Duration::ZERO;
        self.operator_count = 0;
        self.bottleneck_count = 0;
    }

    /// Get stats for display in overlay header.
    pub fn stats(&self) -> (Duration, usize, usize) {
        (self.total_time, self.operator_count, self.bottleneck_count)
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        if let Some(tree) = &mut self.tree_view {
            tree.set_theme(theme);
        }
        if let Some(stats) = &mut self.stats_view {
            stats.set_theme(theme);
        }
        if let Some(waterfall) = &mut self.waterfall_view {
            waterfall.set_theme(theme);
        }
    }

    /// Handle keyboard input for mode switching.
    /// Returns true if input was handled.
    fn handle_input(&mut self, ui: &egui::Ui) -> bool {
        // Skip input handling if a workspace overlay is blocking
        if self.overlay_blocks_input {
            return false;
        }

        let mut handled = false;

        ui.ctx().input_mut(|input| {
            // Tab - cycle to next mode
            if input.consume_key(egui::Modifiers::NONE, Key::Tab) {
                self.mode = match self.mode {
                    PlanViewMode::Tree => PlanViewMode::Stats,
                    PlanViewMode::Stats => PlanViewMode::Waterfall,
                    PlanViewMode::Waterfall => PlanViewMode::Tree,
                };
                handled = true;
            }
            // Shift+Tab - cycle to previous mode
            if input.consume_key(egui::Modifiers::SHIFT, Key::Tab) {
                self.mode = match self.mode {
                    PlanViewMode::Tree => PlanViewMode::Waterfall,
                    PlanViewMode::Stats => PlanViewMode::Tree,
                    PlanViewMode::Waterfall => PlanViewMode::Stats,
                };
                handled = true;
            }
        });

        handled
    }

    /// Render the plan viewer.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Handle Tab key for mode switching
        self.handle_input(ui);

        let text_secondary = self.theme.text_secondary();
        let key_bg = self.theme.bg_elevated();

        // Premium pill-style tab bar
        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // Tab container with subtle background
            egui::Frame::new()
                .fill(self.theme.bg_base().gamma_multiply(0.5))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(3, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;

                        self.render_tab_pill(ui, PlanViewMode::Tree, nav::TREE, "Tree");
                        self.render_tab_pill(ui, PlanViewMode::Stats, action::CHART, "Stats");
                        self.render_tab_pill(ui, PlanViewMode::Waterfall, time::TIMER, "Waterfall");
                    });
                });

            ui.add_space(8.0);

            // Mode-specific keybindings with key badges
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                match self.mode {
                    PlanViewMode::Tree => {
                        render_key_badge(ui, "Tab", key_bg, text_secondary);
                        ui.label(RichText::new("view").color(text_secondary).small());
                        ui.add_space(8.0);
                        render_key_badge(ui, "b", key_bg, text_secondary);
                        ui.label(RichText::new("bottleneck").color(text_secondary).small());
                        ui.add_space(8.0);
                        render_key_badge(ui, "h/l", key_bg, text_secondary);
                        ui.label(RichText::new("fold").color(text_secondary).small());
                        ui.add_space(8.0);
                        render_key_badge(ui, "j/k", key_bg, text_secondary);
                        ui.label(RichText::new("nav").color(text_secondary).small());
                    }
                    PlanViewMode::Stats => {
                        render_key_badge(ui, "Tab", key_bg, text_secondary);
                        ui.label(RichText::new("view").color(text_secondary).small());
                    }
                    PlanViewMode::Waterfall => {
                        render_key_badge(ui, "Tab", key_bg, text_secondary);
                        ui.label(RichText::new("view").color(text_secondary).small());
                        ui.add_space(8.0);
                        render_key_badge(ui, "b", key_bg, text_secondary);
                        ui.label(RichText::new("bottleneck").color(text_secondary).small());
                        ui.add_space(8.0);
                        render_key_badge(ui, "g/G", key_bg, text_secondary);
                        ui.label(RichText::new("jump").color(text_secondary).small());
                        ui.add_space(8.0);
                        render_key_badge(ui, "j/k", key_bg, text_secondary);
                        ui.label(RichText::new("nav").color(text_secondary).small());
                    }
                }
            });
        });

        ui.add_space(8.0);

        // Propagate overlay_blocks_input to child views
        let blocks = self.overlay_blocks_input;
        if let Some(tree) = &mut self.tree_view {
            tree.set_overlay_blocks_input(blocks);
        }
        if let Some(waterfall) = &mut self.waterfall_view {
            waterfall.set_overlay_blocks_input(blocks);
        }

        // Render the active view
        match self.mode {
            PlanViewMode::Tree => {
                if let Some(tree) = &mut self.tree_view {
                    tree.show(ui);
                } else {
                    self.show_empty_state(ui);
                }
            }
            PlanViewMode::Stats => {
                if let Some(stats) = &mut self.stats_view {
                    stats.show(ui);
                } else {
                    self.show_empty_state(ui);
                }
            }
            PlanViewMode::Waterfall => {
                if let Some(waterfall) = &mut self.waterfall_view {
                    waterfall.show(ui);
                } else {
                    self.show_empty_state(ui);
                }
            }
        }
    }

    /// Render a premium pill-style tab button.
    fn render_tab_pill(&mut self, ui: &mut egui::Ui, mode: PlanViewMode, icon: &str, label: &str) {
        let is_active = self.mode == mode;
        let text_color = if is_active {
            self.theme.text_primary()
        } else {
            self.theme.text_secondary()
        };
        let bg_color = if is_active {
            self.theme.bg_elevated()
        } else {
            Color32::TRANSPARENT
        };

        let response = egui::Frame::new()
            .fill(bg_color)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .stroke(if is_active {
                Stroke::new(1.0, self.theme.border_default())
            } else {
                Stroke::NONE
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(RichText::new(icon).color(text_color).size(12.0));
                    ui.label(RichText::new(label).color(text_color).size(12.0));
                });
            });

        if response.response.clicked() {
            self.mode = mode;
        }
        response
            .response
            .on_hover_cursor(egui::CursorIcon::PointingHand);
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
    fn test_stats_view_creation() {
        let plan = create_test_plan();
        let theme = AppTheme::default();
        let stats = StatsView::new(&plan, theme);

        assert_eq!(stats.entries.len(), 3);
        assert_eq!(stats.operator_count, 3);
        // Should be sorted by time descending, ParquetExec is the bottleneck (50ms)
        assert_eq!(stats.entries[0].operator, "ParquetExec");
        assert!(stats.entries[0].is_bottleneck);
        assert_eq!(stats.bottleneck, Some("ParquetExec".to_string()));
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
