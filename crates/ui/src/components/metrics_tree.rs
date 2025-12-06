use std::collections::{HashMap, HashSet};

use egui::{Color32, Key, RichText, collapsing_header::CollapsingState};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;

/// Represents a navigable item in the tree for vim-style navigation
#[derive(Debug, Clone, PartialEq)]
pub enum TreeItem {
    /// A category header
    Category(MetricCategory),
    /// A metric group (path)
    Group {
        category: MetricCategory,
        path: String,
    },
    /// A metric leaf node
    Metric { name: String },
}

/// Vim-style keybinding mode
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum VimMode {
    #[default]
    Normal,
    /// Waiting for second key (e.g., 'g' pressed, waiting for 'g' or other)
    PendingG,
}

/// Represents a metric category/section in the tree
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetricCategory {
    /// Tokio runtime metrics (park count, queue depth, etc.)
    Tokio,
    /// Task-level metrics from @monitor macro
    Tasks,
    /// DataFusion query engine metrics
    DataFusion,
    /// System-level metrics (CPU, memory, disk)
    System,
    /// Application-specific custom metrics
    Application,
    /// Unknown/other metrics
    Other,
}

impl MetricCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tokio => "Tokio Runtime",
            Self::Tasks => "Tasks",
            Self::DataFusion => "DataFusion",
            Self::System => "System",
            Self::Application => "Application",
            Self::Other => "Other",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Tokio => egui_phosphor::regular::LIGHTNING,
            Self::Tasks => egui_phosphor::regular::LIST_CHECKS,
            Self::DataFusion => egui_phosphor::regular::DATABASE,
            Self::System => egui_phosphor::regular::CPU,
            Self::Application => egui_phosphor::regular::CUBE,
            Self::Other => egui_phosphor::regular::DOTS_THREE,
        }
    }

    /// Determine category from metric name
    pub fn from_metric_name(name: &str) -> Self {
        if name.starts_with("tokio.") {
            Self::Tokio
        } else if name.starts_with("task.") {
            Self::Tasks
        } else if name.starts_with("datafusion.") {
            Self::DataFusion
        } else if name.starts_with("system.")
            || name.starts_with("cpu.")
            || name.starts_with("memory.")
            || name.starts_with("disk.")
        {
            Self::System
        } else if name.starts_with("app.")
            || name.starts_with("http.")
            || name.starts_with("request.")
        {
            Self::Application
        } else {
            Self::Other
        }
    }

    /// All categories in display order
    pub fn all() -> &'static [MetricCategory] {
        &[
            Self::Tokio,
            Self::Tasks,
            Self::DataFusion,
            Self::System,
            Self::Application,
            Self::Other,
        ]
    }
}

/// A metric with its metadata
#[derive(Debug, Clone)]
pub struct MetricInfo {
    /// Full metric name (e.g., "tokio.runtime.total_park_count")
    pub name: String,
    /// Human-readable description
    pub description: Option<String>,
    /// Unit of measurement (e.g., "ms", "bytes", "count")
    pub unit: Option<String>,
    /// Tags associated with this metric (key -> set of values)
    pub tags: HashMap<String, HashSet<String>>,
    /// Category for grouping
    pub category: MetricCategory,
    /// Number of active series for this metric
    pub series_count: usize,
}

impl MetricInfo {
    pub fn new(name: String) -> Self {
        let category = MetricCategory::from_metric_name(&name);
        Self {
            name,
            description: None,
            unit: None,
            tags: HashMap::new(),
            category,
            series_count: 0,
        }
    }

    /// Short display name (last segment of dotted name)
    pub fn display_name(&self) -> &str {
        self.name.rsplit('.').next().unwrap_or(&self.name)
    }

    /// Parent path for grouping (all but last segment)
    pub fn parent_path(&self) -> Option<&str> {
        self.name.rsplit_once('.').map(|(parent, _)| parent)
    }
}

/// Selection state for a metric
#[derive(Debug, Clone, Default)]
pub struct MetricSelection {
    /// Selected metric name
    pub metric: Option<String>,
    /// Selected tag filters (key -> value)
    pub tag_filters: HashMap<String, String>,
}

/// The metrics tree panel component
pub struct MetricsTree {
    /// All known metrics
    metrics: Vec<MetricInfo>,
    /// Search/filter text
    filter: String,
    /// Expanded category sections
    expanded_categories: HashSet<MetricCategory>,
    /// Expanded metric groups within categories
    expanded_groups: HashSet<String>,
    /// Current selection
    selection: MetricSelection,
    /// Current theme
    theme: AppTheme,
    /// Pending chart to add (set on double-click)
    pending_chart: Option<String>,
    /// Focused item index for vim navigation (into the flat item list)
    focus_index: Option<usize>,
    /// Vim mode for multi-key commands
    vim_mode: VimMode,
    /// Whether the tree has keyboard focus (reserved for future use)
    #[allow(dead_code)]
    has_focus: bool,
}

impl Default for MetricsTree {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsTree {
    pub fn new() -> Self {
        // Start with some categories expanded
        let mut expanded_categories = HashSet::new();
        expanded_categories.insert(MetricCategory::Tokio);
        expanded_categories.insert(MetricCategory::Tasks);

        Self {
            metrics: Vec::new(),
            filter: String::new(),
            expanded_categories,
            expanded_groups: HashSet::new(),
            selection: MetricSelection::default(),
            theme: AppTheme::default(),
            pending_chart: None,
            focus_index: None,
            vim_mode: VimMode::Normal,
            has_focus: false,
        }
    }

    /// Take the pending chart request (returns None if no pending chart)
    pub fn take_pending_chart(&mut self) -> Option<String> {
        self.pending_chart.take()
    }

    /// Request adding a chart for the given metric
    pub fn request_chart(&mut self, metric_name: impl Into<String>) {
        self.pending_chart = Some(metric_name.into());
    }

    /// Create with example/demo metrics for testing
    pub fn with_demo_metrics() -> Self {
        let mut tree = Self::new();

        // Tokio metrics
        tree.add_metric(MetricInfo::new("tokio.runtime.total_park_count".into()));
        tree.add_metric(MetricInfo::new("tokio.runtime.blocking_queue_depth".into()));
        tree.add_metric(MetricInfo::new("tokio.runtime.num_remote_schedules".into()));
        tree.add_metric(MetricInfo::new(
            "tokio.runtime.budget_forced_yield_count".into(),
        ));
        tree.add_metric(MetricInfo::new(
            "tokio.runtime.io_driver_ready_count".into(),
        ));
        tree.add_metric(MetricInfo::new(
            "tokio.runtime.mean_poll_duration_ns".into(),
        ));

        // Task metrics
        let mut task_poll = MetricInfo::new("task.poll.count".into());
        task_poll.tags.insert(
            "task".into(),
            ["ingestor", "query_handler", "compactor"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        task_poll.series_count = 3;
        tree.add_metric(task_poll);

        let mut task_duration = MetricInfo::new("task.poll.duration_ns".into());
        task_duration.tags.insert(
            "task".into(),
            ["ingestor", "query_handler", "compactor"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        task_duration.series_count = 3;
        tree.add_metric(task_duration);

        tree.add_metric(MetricInfo::new("task.poll.slow_count".into()));
        tree.add_metric(MetricInfo::new("task.idle.duration_ns".into()));
        tree.add_metric(MetricInfo::new("task.scheduled.duration_ns".into()));

        // DataFusion metrics
        tree.add_metric(MetricInfo::new("datafusion.query.execution_time_ns".into()));
        tree.add_metric(MetricInfo::new("datafusion.query.rows_produced".into()));
        tree.add_metric(MetricInfo::new("datafusion.memory.pool_size".into()));

        // System metrics
        let mut cpu = MetricInfo::new("cpu.usage".into());
        cpu.tags.insert(
            "host".into(),
            ["server1", "server2", "server3"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        cpu.series_count = 3;
        tree.add_metric(cpu);

        tree.add_metric(MetricInfo::new("memory.used".into()));
        tree.add_metric(MetricInfo::new("memory.available".into()));

        // Application metrics
        let mut http_req = MetricInfo::new("http.requests".into());
        http_req.tags.insert(
            "env".into(),
            ["prod", "staging"].iter().map(|s| s.to_string()).collect(),
        );
        http_req.tags.insert(
            "service".into(),
            ["api", "web"].iter().map(|s| s.to_string()).collect(),
        );
        http_req.series_count = 4;
        tree.add_metric(http_req);

        tree.add_metric(MetricInfo::new("request.count".into()));
        tree.add_metric(MetricInfo::new("request.latency".into()));

        tree
    }

    /// Add a metric to the tree
    pub fn add_metric(&mut self, metric: MetricInfo) {
        self.metrics.push(metric);
    }

    /// Update metrics list (e.g., from backend refresh)
    pub fn set_metrics(&mut self, metrics: Vec<MetricInfo>) {
        self.metrics = metrics;
    }

    /// Get the current selection
    pub fn selection(&self) -> &MetricSelection {
        &self.selection
    }

    /// Get a metric by name
    pub fn get_metric(&self, name: &str) -> Option<&MetricInfo> {
        self.metrics.iter().find(|m| m.name == name)
    }

    /// Get all metrics
    pub fn metrics(&self) -> &[MetricInfo] {
        &self.metrics
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the filter text (for external filtering from Dashboard)
    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
    }

    /// Get the current filter text
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Check if there are any metrics matching the current filter
    pub fn has_matching_metrics(&self) -> bool {
        if self.filter.is_empty() {
            return !self.metrics.is_empty();
        }
        self.filtered_metrics().next().is_some()
    }

    /// Filter metrics by search text
    fn filtered_metrics(&self) -> impl Iterator<Item = &MetricInfo> {
        let filter_lower = self.filter.to_lowercase();
        self.metrics.iter().filter(move |m| {
            if filter_lower.is_empty() {
                return true;
            }
            m.name.to_lowercase().contains(&filter_lower)
        })
    }

    /// Group metrics by category
    fn metrics_by_category(&self) -> HashMap<MetricCategory, Vec<&MetricInfo>> {
        let mut groups: HashMap<MetricCategory, Vec<&MetricInfo>> = HashMap::new();
        for metric in self.filtered_metrics() {
            groups
                .entry(metric.category.clone())
                .or_default()
                .push(metric);
        }
        groups
    }

    /// Build a flat list of navigable items based on current expansion state
    fn build_flat_items(&self) -> Vec<TreeItem> {
        // NOTE: This uses our internal tracking which is synced from egui's state
        // in the show methods
        let mut items = Vec::new();
        let grouped = self.metrics_by_category();

        for category in MetricCategory::all() {
            let Some(metrics) = grouped.get(category) else {
                continue;
            };

            // Add category header
            items.push(TreeItem::Category(category.clone()));

            // Only add children if expanded
            if !self.expanded_categories.contains(category) {
                continue;
            }

            // Group metrics by parent path within category
            let mut by_group: HashMap<Option<&str>, Vec<&MetricInfo>> = HashMap::new();
            for metric in metrics {
                by_group
                    .entry(metric.parent_path())
                    .or_default()
                    .push(metric);
            }

            // Sort groups for consistent display
            let mut groups: Vec<_> = by_group.into_iter().collect();
            groups.sort_by_key(|(path, _)| path.unwrap_or(""));

            for (group_path, group_metrics) in groups {
                if let Some(path) = group_path {
                    if group_metrics.len() > 1 {
                        // This is a group
                        items.push(TreeItem::Group {
                            category: category.clone(),
                            path: path.to_string(),
                        });

                        // Add metrics if group is expanded
                        if self.expanded_groups.contains(path) {
                            for metric in group_metrics {
                                items.push(TreeItem::Metric {
                                    name: metric.name.clone(),
                                });
                            }
                        }
                    } else {
                        // Single metric, add directly
                        for metric in group_metrics {
                            items.push(TreeItem::Metric {
                                name: metric.name.clone(),
                            });
                        }
                    }
                } else {
                    // No parent path, add directly
                    for metric in group_metrics {
                        items.push(TreeItem::Metric {
                            name: metric.name.clone(),
                        });
                    }
                }
            }
        }

        items
    }

    /// Build a flat list of navigable items, querying egui's state for expansion
    fn build_flat_items_with_ctx(&self, ctx: &egui::Context) -> Vec<TreeItem> {
        let mut items = Vec::new();
        let grouped = self.metrics_by_category();

        for category in MetricCategory::all() {
            let Some(metrics) = grouped.get(category) else {
                continue;
            };

            // Add category header
            items.push(TreeItem::Category(category.clone()));

            // Check if category is expanded using egui's state
            let cat_id = egui::Id::new(format!("cat_{category:?}"));
            let cat_expanded =
                CollapsingState::load_with_default_open(ctx, cat_id, false).is_open();
            if !cat_expanded {
                continue;
            }

            // Group metrics by parent path within category
            let mut by_group: HashMap<Option<&str>, Vec<&MetricInfo>> = HashMap::new();
            for metric in metrics {
                by_group
                    .entry(metric.parent_path())
                    .or_default()
                    .push(metric);
            }

            // Sort groups for consistent display
            let mut groups: Vec<_> = by_group.into_iter().collect();
            groups.sort_by_key(|(path, _)| path.unwrap_or(""));

            for (group_path, group_metrics) in groups {
                if let Some(path) = group_path {
                    if group_metrics.len() > 1 {
                        // This is a group
                        items.push(TreeItem::Group {
                            category: category.clone(),
                            path: path.to_string(),
                        });

                        // Check if group is expanded using egui's state
                        let group_id = egui::Id::new(format!("group_{path}"));
                        let group_expanded =
                            CollapsingState::load_with_default_open(ctx, group_id, false).is_open();
                        if group_expanded {
                            for metric in group_metrics {
                                items.push(TreeItem::Metric {
                                    name: metric.name.clone(),
                                });
                            }
                        }
                    } else {
                        // Single metric, add directly
                        for metric in group_metrics {
                            items.push(TreeItem::Metric {
                                name: metric.name.clone(),
                            });
                        }
                    }
                } else {
                    // No parent path, add directly
                    for metric in group_metrics {
                        items.push(TreeItem::Metric {
                            name: metric.name.clone(),
                        });
                    }
                }
            }
        }

        items
    }

    /// Handle vim-style keyboard navigation
    /// Returns true if a key was consumed
    pub fn handle_keyboard(&mut self, ctx: &egui::Context) -> bool {
        // Don't handle keys if something else has focus (e.g., text input)
        if ctx.memory(|mem| mem.focused().is_some()) {
            return false;
        }

        // Use the ctx-aware version to get accurate expansion state
        let items = self.build_flat_items_with_ctx(ctx);
        if items.is_empty() {
            return false;
        }

        // Initialize focus if not set
        if self.focus_index.is_none() {
            self.focus_index = Some(0);
        }

        // Clamp focus index to valid range (items may have changed)
        if let Some(idx) = self.focus_index {
            if idx >= items.len() {
                self.focus_index = Some(items.len().saturating_sub(1));
            }
        }

        let mut consumed = false;
        let mut toggle_category: Option<MetricCategory> = None;
        let mut toggle_group: Option<String> = None;

        ctx.input_mut(|input| {
            // Handle pending 'g' mode
            if self.vim_mode == VimMode::PendingG {
                if input.consume_key(egui::Modifiers::NONE, Key::G) {
                    // gg - go to top
                    self.focus_index = Some(0);
                    self.select_focused_item(&items);
                    consumed = true;
                }
                // Any key press (including 'g') exits pending mode
                self.vim_mode = VimMode::Normal;
                if consumed {
                    return;
                }
            }

            // j - move down
            if input.consume_key(egui::Modifiers::NONE, Key::J) {
                let current = self.focus_index.unwrap_or(0);
                if current + 1 < items.len() {
                    self.focus_index = Some(current + 1);
                    self.select_focused_item(&items);
                }
                consumed = true;
                return;
            }

            // k - move up
            if input.consume_key(egui::Modifiers::NONE, Key::K) {
                let current = self.focus_index.unwrap_or(0);
                if current > 0 {
                    self.focus_index = Some(current - 1);
                    self.select_focused_item(&items);
                }
                consumed = true;
                return;
            }

            // l - expand / move right into children
            if input.consume_key(egui::Modifiers::NONE, Key::L) {
                if let Some(idx) = self.focus_index {
                    if let Some(item) = items.get(idx) {
                        match item {
                            TreeItem::Category(cat) => {
                                toggle_category = Some(cat.clone());
                            }
                            TreeItem::Group { path, .. } => {
                                toggle_group = Some(path.clone());
                            }
                            TreeItem::Metric { name } => {
                                // Select metric and add chart
                                self.selection.metric = Some(name.clone());
                                self.pending_chart = Some(name.clone());
                            }
                        }
                    }
                }
                consumed = true;
                return;
            }

            // h - collapse / move left to parent
            if input.consume_key(egui::Modifiers::NONE, Key::H) {
                if let Some(idx) = self.focus_index {
                    if let Some(item) = items.get(idx) {
                        match item {
                            TreeItem::Category(cat) => {
                                toggle_category = Some(cat.clone());
                            }
                            TreeItem::Group { path, .. } => {
                                toggle_group = Some(path.clone());
                            }
                            TreeItem::Metric { .. } => {
                                // Move focus to parent category/group
                                self.move_to_parent(&items, idx);
                            }
                        }
                    }
                }
                consumed = true;
                return;
            }

            // g - start pending 'g' mode for gg
            if input.consume_key(egui::Modifiers::NONE, Key::G) {
                self.vim_mode = VimMode::PendingG;
                consumed = true;
                return;
            }

            // G (Shift+g) - go to bottom
            if input.consume_key(egui::Modifiers::SHIFT, Key::G) {
                if !items.is_empty() {
                    self.focus_index = Some(items.len() - 1);
                    self.select_focused_item(&items);
                }
                consumed = true;
                return;
            }

            // Enter - select/toggle current item
            if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                if let Some(idx) = self.focus_index {
                    if let Some(item) = items.get(idx) {
                        match item {
                            TreeItem::Category(cat) => {
                                toggle_category = Some(cat.clone());
                            }
                            TreeItem::Group { path, .. } => {
                                toggle_group = Some(path.clone());
                            }
                            TreeItem::Metric { name } => {
                                self.selection.metric = Some(name.clone());
                                self.pending_chart = Some(name.clone());
                            }
                        }
                    }
                }
                consumed = true;
            }
        });

        // Toggle category/group state using egui's internal state
        if let Some(cat) = toggle_category {
            let id = egui::Id::new(format!("cat_{cat:?}"));
            let mut state = CollapsingState::load_with_default_open(ctx, id, false);
            state.set_open(!state.is_open());
            state.store(ctx);
        }
        if let Some(path) = toggle_group {
            let id = egui::Id::new(format!("group_{path}"));
            let mut state = CollapsingState::load_with_default_open(ctx, id, false);
            state.set_open(!state.is_open());
            state.store(ctx);
        }

        // Request repaint if we consumed a key (to update visuals)
        if consumed {
            ctx.request_repaint();
        }

        consumed
    }

    /// Select the metric at the focused index (if it's a metric)
    fn select_focused_item(&mut self, items: &[TreeItem]) {
        if let Some(idx) = self.focus_index {
            if let Some(TreeItem::Metric { name }) = items.get(idx) {
                self.selection.metric = Some(name.clone());
            }
        }
    }

    /// Move focus to the parent category or group
    fn move_to_parent(&mut self, items: &[TreeItem], current_idx: usize) {
        // Search backwards for a Category or Group
        for i in (0..current_idx).rev() {
            if let Some(item) = items.get(i) {
                match item {
                    TreeItem::Category(_) | TreeItem::Group { .. } => {
                        self.focus_index = Some(i);
                        return;
                    }
                    TreeItem::Metric { .. } => continue,
                }
            }
        }
    }

    /// Get the currently focused item
    pub fn focused_item(&self) -> Option<TreeItem> {
        let items = self.build_flat_items();
        self.focus_index.and_then(|idx| items.get(idx).cloned())
    }

    /// Show the metrics tree panel (search box is now handled by Dashboard)
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_color = text_color(self.theme);

        // Build flat items for focus tracking
        let flat_items = self.build_flat_items();

        // Collect metrics into owned data to avoid borrow issues
        let metrics_data: Vec<(MetricCategory, Vec<MetricInfo>)> = {
            let grouped = self.metrics_by_category();
            MetricCategory::all()
                .iter()
                .filter_map(|cat| {
                    grouped.get(cat).map(|metrics| {
                        (cat.clone(), metrics.iter().map(|m| (*m).clone()).collect())
                    })
                })
                .collect()
        };

        // Track item index for focus matching
        let mut item_idx = 0;

        // Render categories directly (parent handles scrolling)
        for (category, metrics) in &metrics_data {
            self.show_category_with_focus(
                ui,
                category,
                metrics,
                text_color,
                &flat_items,
                &mut item_idx,
            );
        }
    }

    /// Show a category with focus tracking for vim navigation
    fn show_category_with_focus(
        &mut self,
        ui: &mut egui::Ui,
        category: &MetricCategory,
        metrics: &[MetricInfo],
        text_color: Color32,
        flat_items: &[TreeItem],
        item_idx: &mut usize,
    ) {
        let _is_expanded = self.expanded_categories.contains(category);
        let is_focused = self.focus_index == Some(*item_idx);

        // Increment for category itself
        *item_idx += 1;

        // Build header text with icon and focus indicator
        let header_text = format!(
            "{}{} {} ({})",
            if is_focused { "▶ " } else { "" },
            category.icon(),
            category.label(),
            metrics.len()
        );

        let header_color = if is_focused {
            Color32::from_rgb(255, 215, 0) // Accent yellow for focus
        } else {
            text_color
        };

        let response =
            egui::CollapsingHeader::new(RichText::new(header_text).color(header_color).strong())
                .id_salt(format!("cat_{category:?}"))
                .show(ui, |ui| {
                    // Group metrics by parent path within category
                    let mut by_group: HashMap<Option<&str>, Vec<&MetricInfo>> = HashMap::new();
                    for metric in metrics {
                        by_group
                            .entry(metric.parent_path())
                            .or_default()
                            .push(metric);
                    }

                    // Sort groups for consistent display
                    let mut groups: Vec<_> = by_group.into_iter().collect();
                    groups.sort_by_key(|(path, _)| path.unwrap_or(""));

                    for (group_path, group_metrics) in groups {
                        if let Some(path) = group_path {
                            // Show as sub-group if there are multiple metrics
                            if group_metrics.len() > 1 {
                                self.show_metric_group_with_focus(
                                    ui,
                                    path,
                                    &group_metrics,
                                    text_color,
                                    flat_items,
                                    item_idx,
                                );
                            } else {
                                // Single metric, show directly
                                for metric in group_metrics {
                                    self.show_metric_item_with_focus(
                                        ui, metric, text_color, item_idx,
                                    );
                                }
                            }
                        } else {
                            // No parent path, show directly
                            for metric in group_metrics {
                                self.show_metric_item_with_focus(ui, metric, text_color, item_idx);
                            }
                        }
                    }
                });

        // Sync our expanded state with the header's actual state
        // This allows both mouse clicks and keyboard to control expansion
        if response.fully_open() {
            self.expanded_categories.insert(category.clone());
        } else if !response.fully_open() && response.openness < 0.1 {
            self.expanded_categories.remove(category);
        }

        // Handle click to set focus
        if response.header_response.clicked() {
            self.focus_index = Some(*item_idx - 1);
        }
    }

    /// Show a metric group with focus tracking
    fn show_metric_group_with_focus(
        &mut self,
        ui: &mut egui::Ui,
        group_path: &str,
        metrics: &[&MetricInfo],
        text_color: Color32,
        _flat_items: &[TreeItem],
        item_idx: &mut usize,
    ) {
        let _is_expanded = self.expanded_groups.contains(group_path);
        let is_focused = self.focus_index == Some(*item_idx);

        // Increment for group itself
        *item_idx += 1;

        // Build header text with folder icon and focus indicator
        let header_text = format!(
            "{}{} {} ({})",
            if is_focused { "▶ " } else { "" },
            egui_phosphor::regular::FOLDER,
            group_path,
            metrics.len()
        );

        let header_color = if is_focused {
            Color32::from_rgb(255, 215, 0) // Accent yellow for focus
        } else {
            text_color.gamma_multiply(0.9)
        };

        ui.indent(group_path, |ui| {
            let response =
                egui::CollapsingHeader::new(RichText::new(header_text).color(header_color))
                    .id_salt(format!("group_{group_path}"))
                    .show(ui, |ui| {
                        for metric in metrics {
                            self.show_metric_item_with_focus(ui, metric, text_color, item_idx);
                        }
                    });

            // Sync our expanded state with the header's actual state
            if response.fully_open() {
                self.expanded_groups.insert(group_path.to_string());
            } else if !response.fully_open() && response.openness < 0.1 {
                self.expanded_groups.remove(group_path);
            }

            // Handle click to set focus
            if response.header_response.clicked() {
                self.focus_index = Some(*item_idx - 1);
            }
        });
    }

    /// Show a metric item with focus tracking
    fn show_metric_item_with_focus(
        &mut self,
        ui: &mut egui::Ui,
        metric: &MetricInfo,
        text_color: Color32,
        item_idx: &mut usize,
    ) {
        let is_selected = self.selection.metric.as_deref() == Some(&metric.name);
        let is_focused = self.focus_index == Some(*item_idx);

        // Increment for this metric
        *item_idx += 1;

        let mut add_chart_requested = false;

        // Determine the display color based on focus/selection
        let display_color = if is_focused {
            Color32::from_rgb(255, 215, 0) // Accent yellow for focus
        } else {
            text_color
        };

        // Focus indicator prefix
        let focus_prefix = if is_focused { "▶ " } else { "  " };

        ui.horizontal(|ui| {
            ui.add_space(32.0); // Indent for leaf items

            // Selection highlight with focus indicator
            let label_text = format!("{}{}", focus_prefix, metric.display_name());
            let response = ui.selectable_label(
                is_selected || is_focused,
                RichText::new(label_text).color(display_color),
            );

            if response.clicked() {
                self.selection.metric = Some(metric.name.clone());
                self.selection.tag_filters.clear();
                self.focus_index = Some(*item_idx - 1);
            }

            // Double-click to add chart
            if response.double_clicked() {
                add_chart_requested = true;
            }

            // Show series count if > 1
            if metric.series_count > 1 {
                ui.label(
                    RichText::new(format!("[{}]", metric.series_count))
                        .color(text_color.gamma_multiply(0.4))
                        .small(),
                );
            }

            // Show tag indicator if has tags
            if !metric.tags.is_empty() {
                ui.label(
                    RichText::new(egui_phosphor::regular::TAG)
                        .color(text_color.gamma_multiply(0.4))
                        .small(),
                );
            }

            // Add chart button (visible on hover or when selected)
            if (is_selected || response.hovered())
                && ui
                    .small_button(egui_phosphor::regular::CHART_LINE)
                    .on_hover_text("Add chart")
                    .clicked()
            {
                add_chart_requested = true;
            }
        });

        // Handle chart request (outside the closure to avoid borrow issues)
        if add_chart_requested {
            self.pending_chart = Some(metric.name.clone());
        }

        // Show tags as sub-items if selected and has tags
        if is_selected && !metric.tags.is_empty() {
            ui.indent("metric_tags", |ui| {
                for (key, values) in &metric.tags {
                    ui.horizontal(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new(format!("{key}:"))
                                .color(text_color.gamma_multiply(0.6))
                                .small(),
                        );

                        // Show tag values as selectable chips
                        for value in values {
                            let is_tag_selected =
                                self.selection.tag_filters.get(key) == Some(value);
                            let chip_text = RichText::new(value)
                                .color(if is_tag_selected {
                                    Color32::from_rgb(255, 215, 0) // Accent yellow
                                } else {
                                    text_color.gamma_multiply(0.7)
                                })
                                .small();

                            if ui.selectable_label(is_tag_selected, chip_text).clicked() {
                                if is_tag_selected {
                                    self.selection.tag_filters.remove(key);
                                } else {
                                    self.selection
                                        .tag_filters
                                        .insert(key.clone(), value.clone());
                                }
                            }
                        }
                    });
                }
            });
        }
    }
}

/// Implement Component trait so MetricsTree can be used in the dashboard
impl super::Component for MetricsTree {
    fn show(&mut self, ui: &mut egui::Ui) {
        MetricsTree::show(self, ui);
    }

    fn id(&self) -> usize {
        // Use a fixed ID for the metrics tree panel
        1
    }

    fn name(&self) -> String {
        "Metrics".to_string()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    fn set_api_key(&mut self, _key: &str) {
        // Not needed for metrics tree
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed for metrics tree
    }

    fn label(&self) -> egui::RichText {
        egui::RichText::new(format!(
            "{} Metrics",
            egui_phosphor::regular::TREE_STRUCTURE
        ))
    }
}
