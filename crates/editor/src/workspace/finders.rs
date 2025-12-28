//! Finder modal methods for the workspace.
//!
//! This module handles the metrics finder and workspace finder overlays,
//! including generating metric items from Prometheus or demo data.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{Workspace, WorkspaceAction};
use crate::app::AppState;
use crate::components::{MetricItem, WorkspaceItem};

impl Workspace {
    // ==================== Metrics Finder ====================

    /// Open the metrics finder modal (for metrics only)
    pub fn open_metrics_finder(&mut self) {
        let items = if self.query_executor.is_connected() {
            // Use real metrics from Prometheus
            self.prometheus_metric_items()
        } else {
            // Fall back to demo metrics
            Self::demo_metric_items()
        };
        self.metrics_finder.set_items(items);
        self.metrics_finder.open();
    }

    /// Handle fuzzy selection (metrics only) and return tracking action
    pub(super) fn handle_metric_selection_with_tracking(
        &mut self,
        item: MetricItem,
    ) -> WorkspaceAction {
        self.show_landing = false;
        self.add_chart_for_metric_with_tracking(&item.name)
    }

    /// Generate metric items from Prometheus metric names
    fn prometheus_metric_items(&self) -> Vec<MetricItem> {
        self.query_executor
            .metric_names()
            .iter()
            .map(|name| {
                // Infer category from metric name prefix (common Prometheus conventions)
                let category = Self::infer_prometheus_category(name);

                // Check if we have cached labels for this metric
                let tags: FxHashMap<String, FxHashSet<String>> =
                    if let Some(labels) = self.query_executor.get_metric_labels(name) {
                        // Use actual per-metric labels
                        labels
                            .labels
                            .iter()
                            .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
                            .collect()
                    } else {
                        // No labels cached yet - show empty (will be fetched on selection)
                        FxHashMap::default()
                    };

                MetricItem {
                    name: name.clone(),
                    category,
                    description: None,
                    unit: None,
                    tags,
                    series_count: 0,
                }
            })
            .collect()
    }

    /// Infer category from Prometheus metric name conventions
    fn infer_prometheus_category(name: &str) -> String {
        // Common Prometheus metric prefixes
        if name.starts_with("node_") {
            "Node Exporter".to_string()
        } else if name.starts_with("go_") {
            "Go Runtime".to_string()
        } else if name.starts_with("process_") {
            "Process".to_string()
        } else if name.starts_with("promhttp_") || name.starts_with("prometheus_") {
            "Prometheus".to_string()
        } else if name.starts_with("http_") {
            "HTTP".to_string()
        } else if name.starts_with("grpc_") {
            "gRPC".to_string()
        } else if name.starts_with("scrape_") {
            "Scrape".to_string()
        } else if name.starts_with("up") || name == "up" {
            "Target".to_string()
        } else {
            // Default: extract first part before underscore
            name.split('_')
                .next()
                .map(|s| {
                    let mut chars = s.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().chain(chars).collect(),
                    }
                })
                .unwrap_or_else(|| "Metrics".to_string())
        }
    }

    /// Generate demo metric items for the fuzzy finder
    fn demo_metric_items() -> Vec<MetricItem> {
        let mut items = Vec::new();

        // Tokio metrics
        for name in [
            "tokio.runtime.total_park_count",
            "tokio.runtime.blocking_queue_depth",
            "tokio.runtime.num_remote_schedules",
            "tokio.runtime.budget_forced_yield_count",
            "tokio.runtime.io_driver_ready_count",
            "tokio.runtime.mean_poll_duration_ns",
        ] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "Tokio Runtime".to_string(),
                description: None,
                unit: None,
                tags: FxHashMap::default(),
                series_count: 0,
            });
        }

        // Task metrics with tags
        let task_tags: FxHashMap<String, FxHashSet<String>> = [(
            "task".to_string(),
            ["ingestor", "query_handler", "compactor"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )]
        .into_iter()
        .collect();

        for name in [
            "task.poll.count",
            "task.poll.duration_ns",
            "task.poll.slow_count",
            "task.idle.duration_ns",
            "task.scheduled.duration_ns",
        ] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "Tasks".to_string(),
                description: None,
                unit: None,
                tags: task_tags.clone(),
                series_count: 3,
            });
        }

        // DataFusion metrics
        for name in [
            "datafusion.query.execution_time_ns",
            "datafusion.query.rows_produced",
            "datafusion.memory.pool_size",
        ] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "DataFusion".to_string(),
                description: None,
                unit: None,
                tags: FxHashMap::default(),
                series_count: 0,
            });
        }

        // System metrics
        let host_tags: FxHashMap<String, FxHashSet<String>> = [(
            "host".to_string(),
            ["server1", "server2", "server3"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )]
        .into_iter()
        .collect();

        items.push(MetricItem {
            name: "cpu.usage".to_string(),
            category: "System".to_string(),
            description: None,
            unit: None,
            tags: host_tags,
            series_count: 3,
        });

        for name in ["memory.used", "memory.available"] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "System".to_string(),
                description: None,
                unit: None,
                tags: FxHashMap::default(),
                series_count: 0,
            });
        }

        // Application metrics
        let app_tags: FxHashMap<String, FxHashSet<String>> = [
            (
                "env".to_string(),
                ["prod", "staging"].iter().map(|s| s.to_string()).collect(),
            ),
            (
                "service".to_string(),
                ["api", "web"].iter().map(|s| s.to_string()).collect(),
            ),
        ]
        .into_iter()
        .collect();

        items.push(MetricItem {
            name: "http.requests".to_string(),
            category: "Application".to_string(),
            description: None,
            unit: None,
            tags: app_tags,
            series_count: 4,
        });

        for name in ["request.count", "request.latency"] {
            items.push(MetricItem {
                name: name.to_string(),
                category: "Application".to_string(),
                description: None,
                unit: None,
                tags: FxHashMap::default(),
                series_count: 0,
            });
        }

        items
    }

    // ==================== Workspace Finder ====================

    /// Open the workspace finder modal (for loading saved workspaces)
    pub fn open_workspace_finder(
        &mut self,
        app_state: &AppState,
        available_workspaces: Vec<(String, Option<String>)>,
    ) {
        // Start with recent workspaces
        let mut workspaces: Vec<WorkspaceItem> = app_state
            .settings
            .recent_workspaces
            .iter()
            .map(|entry| WorkspaceItem {
                name: entry.name.clone(),
                description: if entry.description.is_empty() {
                    None
                } else {
                    Some(entry.description.clone())
                },
            })
            .collect();

        // Track names already in the list
        let existing_names: FxHashSet<String> = workspaces.iter().map(|w| w.name.clone()).collect();

        // Add available workspaces from filesystem that aren't already in recent
        for (name, description) in available_workspaces {
            if !existing_names.contains(&name) {
                workspaces.push(WorkspaceItem { name, description });
            }
        }

        self.workspace_finder.set_workspaces(workspaces);
        self.workspace_finder.open();
    }

    // ==================== Workspace Creator ====================

    /// Open the workspace creator overlay
    pub fn open_workspace_creator(&mut self) {
        self.workspace_creator.open();
    }
}
