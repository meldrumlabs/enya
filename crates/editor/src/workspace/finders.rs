//! Finder modal methods for the workspace.
//!
//! This module handles the metrics finder, workspace finder, unified finder, and codebase
//! finder overlays, including generating metric items from Prometheus or demo data.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{FinderMode, Workspace, WorkspaceAction};
use crate::app::AppState;
#[cfg(not(target_arch = "wasm32"))]
use crate::codebase::search::{SearchFilter, SearchResult, SearchResultKind};
use crate::components::overlay::UnifiedFinderAction;
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

    // ==================== Codebase Finder ====================

    /// Handle codebase finder selection - navigate to source
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn handle_codebase_finder_result(&mut self, result: SearchResult) {
        use crate::codebase::CodebaseStatus;

        log::info!("Codebase finder selected: {result:?}");

        // Get the index if codebase is ready
        let Some(index) = self.codebase_manager.index() else {
            log::warn!("Codebase not ready, cannot open source preview");
            return;
        };

        // Check if codebase manager is ready
        if !matches!(self.codebase_manager.status(), CodebaseStatus::Ready { .. }) {
            log::warn!("Codebase not in ready state");
            return;
        }

        match &result.kind {
            SearchResultKind::Metric(_metric_kind) => {
                // Look up the metric in the index to get full instrumentation data
                let locations: Vec<_> = index
                    .metrics
                    .iter()
                    .filter(|m| m.name == result.name)
                    .cloned()
                    .collect();

                if locations.is_empty() {
                    // Fallback: just add a chart for this metric
                    log::info!("No source location for metric, adding chart");
                    self.show_landing = false;
                    let _ = self.add_chart_for_metric_with_tracking(&result.name);
                } else {
                    log::info!(
                        "Opening source preview for '{}' with {} location(s)",
                        result.name,
                        locations.len()
                    );
                    self.source_preview
                        .open_metric_with_locations(locations, &index.repo_path);
                }
            }
            SearchResultKind::Alert { .. } => {
                // Look up the alert in the index
                let alert = index.alerts.iter().find(|a| a.name == result.name);

                if let Some(alert) = alert {
                    log::info!(
                        "Opening alert preview for '{}' at {}:{}",
                        alert.name,
                        alert.file.display(),
                        alert.line
                    );
                    self.source_preview.open_alert(alert, &index.repo_path);
                } else {
                    log::warn!("Alert '{}' not found in index", result.name);
                }
            }
            SearchResultKind::Commit {
                hash,
                timestamp,
                diff,
            } => {
                log::info!("Opening diff viewer for commit: {} - {}", hash, result.name);
                // Open the diff viewer overlay with the commit's full diff
                self.diff_viewer.open(hash, &result.name, *timestamp, diff);
            }
        }
    }

    // ==================== Unified Finder ====================

    /// Open the unified finder modal
    pub fn open_unified_finder(&mut self) {
        self.open_unified_finder_with_mode(FinderMode::default());
    }

    /// Open the unified finder with a specific mode
    pub fn open_unified_finder_with_mode(&mut self, mode: FinderMode) {
        // Populate live metrics
        let live_metrics = self.prometheus_metric_items_for_unified();

        // Populate workspaces
        // Note: we don't have access to app_state here, so we use cached workspaces
        // The workspace list should be set before opening

        self.unified_finder.set_live_metrics(live_metrics);
        self.unified_finder.open_with_mode(mode);
    }

    /// Show the unified finder and handle its actions
    pub(super) fn show_unified_finder(
        &mut self,
        ctx: &egui::Context,
        app_state: &AppState,
    ) -> Option<WorkspaceAction> {
        if !self.unified_finder.is_open() {
            return None;
        }

        self.unified_finder.set_theme(app_state.theme);

        // Update codebase search results when in a codebase mode (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Set repo path for source preview to construct full paths
            let repo_path = self
                .codebase_manager
                .index()
                .map(|idx| idx.repo_path.clone());
            self.unified_finder.set_repo_path(repo_path);

            // Only search if query or mode changed (avoid redundant searches every frame)
            let query = self.unified_finder.query_text().to_string();
            if !query.is_empty() && self.unified_finder.needs_codebase_search() {
                let mode = self.unified_finder.mode();
                let filter = match mode {
                    FinderMode::Metrics => SearchFilter::Metrics,
                    FinderMode::Alerts => SearchFilter::Alerts,
                    FinderMode::Commits => SearchFilter::Commits,
                    FinderMode::All => SearchFilter::All,
                };
                let results = self.codebase_manager.search_ranked(&query, filter, 50);
                self.unified_finder.set_codebase_results(results);
            }
        }

        // Show the finder and handle the result
        if let Some(action) = self.unified_finder.show(ctx) {
            return self.handle_unified_finder_action(action);
        }

        None
    }

    /// Handle an action from the unified finder
    fn handle_unified_finder_action(
        &mut self,
        action: UnifiedFinderAction,
    ) -> Option<WorkspaceAction> {
        match action {
            UnifiedFinderAction::CreateMetricPane(metric_name) => {
                self.show_landing = false;
                self.add_chart_for_metric_with_tracking(&metric_name);
                None
            }
            UnifiedFinderAction::NavigateToSource { file, line } => {
                log::info!("Navigate to source: {}:{}", file.display(), line);
                // Look up metric by file/line to open in source preview
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(index) = self.codebase_manager.index() {
                    // Find metric at this location
                    let locations: Vec<_> = index
                        .metrics
                        .iter()
                        .filter(|m| m.file == file && m.line == line)
                        .cloned()
                        .collect();

                    if !locations.is_empty() {
                        self.source_preview
                            .open_metric_with_locations(locations, &index.repo_path);
                    } else {
                        // Try alerts
                        if let Some(alert) = index
                            .alerts
                            .iter()
                            .find(|a| a.file == file && a.line == line)
                        {
                            self.source_preview.open_alert(alert, &index.repo_path);
                        }
                    }
                }
                None
            }
            #[cfg(not(target_arch = "wasm32"))]
            UnifiedFinderAction::OpenDiffViewer { hash, diff } => {
                log::info!("Opening diff viewer for commit: {hash}");
                // We need the commit message for the title - just use the hash for now
                self.diff_viewer.open(
                    &hash,
                    &format!("Commit {}", &hash[..7.min(hash.len())]),
                    0,
                    &diff,
                );
                None
            }
        }
    }

    /// Generate metric items for the unified finder (name, category, tags)
    fn prometheus_metric_items_for_unified(
        &self,
    ) -> Vec<(String, String, FxHashMap<String, FxHashSet<String>>)> {
        self.query_executor
            .metric_names()
            .iter()
            .map(|name| {
                let category = Self::infer_prometheus_category(name);
                // Get cached labels for this metric if available
                let tags: FxHashMap<String, FxHashSet<String>> =
                    if let Some(labels) = self.query_executor.get_metric_labels(name) {
                        labels
                            .labels
                            .iter()
                            .map(|(k, v)| (k.clone(), v.iter().cloned().collect()))
                            .collect()
                    } else {
                        FxHashMap::default()
                    };
                (name.clone(), category, tags)
            })
            .collect()
    }
}
