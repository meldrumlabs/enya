use std::collections::{HashMap, HashSet};

use egui::{Color32, RichText};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;

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
        }
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

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
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

    /// Show the metrics tree panel
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_color = text_color(self.theme);

        // Header
        ui.horizontal(|ui| {
            ui.label(RichText::new("Metrics").color(text_color).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(
                        RichText::new(egui_phosphor::regular::ARROWS_CLOCKWISE).color(text_color),
                    )
                    .clicked()
                {
                    // TODO: Trigger refresh from backend
                }
            });
        });

        ui.add_space(4.0);

        // Search box
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                    .color(text_color.gamma_multiply(0.6)),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("Filter metrics...")
                    .desired_width(ui.available_width() - 8.0),
            );
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

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

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (category, metrics) in &metrics_data {
                    self.show_category(ui, category, metrics, text_color);
                }
            });
    }

    fn show_category(
        &mut self,
        ui: &mut egui::Ui,
        category: &MetricCategory,
        metrics: &[MetricInfo],
        text_color: Color32,
    ) {
        let is_expanded = self.expanded_categories.contains(category);

        // Build header text with icon
        let header_text = format!(
            "{} {} ({})",
            category.icon(),
            category.label(),
            metrics.len()
        );

        let response =
            egui::CollapsingHeader::new(RichText::new(header_text).color(text_color).strong())
                .id_salt(format!("cat_{category:?}"))
                .default_open(is_expanded)
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
                                self.show_metric_group(ui, path, &group_metrics, text_color);
                            } else {
                                // Single metric, show directly
                                for metric in group_metrics {
                                    self.show_metric_item(ui, metric, text_color);
                                }
                            }
                        } else {
                            // No parent path, show directly
                            for metric in group_metrics {
                                self.show_metric_item(ui, metric, text_color);
                            }
                        }
                    }
                });

        // Update expanded state based on response
        if response.fully_open() {
            self.expanded_categories.insert(category.clone());
        } else if response.openness < 0.5 {
            self.expanded_categories.remove(category);
        }
    }

    fn show_metric_group(
        &mut self,
        ui: &mut egui::Ui,
        group_path: &str,
        metrics: &[&MetricInfo],
        text_color: Color32,
    ) {
        let is_expanded = self.expanded_groups.contains(group_path);

        // Build header text with folder icon
        let header_text = format!(
            "{} {} ({})",
            egui_phosphor::regular::FOLDER,
            group_path,
            metrics.len()
        );

        ui.indent(group_path, |ui| {
            let response = egui::CollapsingHeader::new(
                RichText::new(header_text).color(text_color.gamma_multiply(0.9)),
            )
            .id_salt(format!("group_{group_path}"))
            .default_open(is_expanded)
            .show(ui, |ui| {
                for metric in metrics {
                    self.show_metric_item(ui, metric, text_color);
                }
            });

            // Update expanded state based on response
            if response.fully_open() {
                self.expanded_groups.insert(group_path.to_string());
            } else if response.openness < 0.5 {
                self.expanded_groups.remove(group_path);
            }
        });
    }

    fn show_metric_item(&mut self, ui: &mut egui::Ui, metric: &MetricInfo, text_color: Color32) {
        let is_selected = self.selection.metric.as_deref() == Some(&metric.name);

        ui.horizontal(|ui| {
            ui.add_space(32.0); // Indent for leaf items

            // Selection highlight
            let response = ui.selectable_label(
                is_selected,
                RichText::new(metric.display_name()).color(text_color),
            );

            if response.clicked() {
                self.selection.metric = Some(metric.name.clone());
                self.selection.tag_filters.clear();
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
        });

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
