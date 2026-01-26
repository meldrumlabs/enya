//! Finder modal methods for the workspace.
//!
//! This module handles the workspace finder, unified finder, and codebase
//! finder overlays.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{FinderMode, Workspace, WorkspaceAction};
use crate::app::AppState;
#[cfg(not(target_arch = "wasm32"))]
use crate::codebase::search::{SearchFilter, SearchResult, SearchResultKind};
use crate::components::WorkspaceItem;
use crate::components::overlay::UnifiedFinderAction;

impl Workspace {
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
        _app_state: &AppState,
    ) -> Option<WorkspaceAction> {
        if !self.unified_finder.is_open() {
            return None;
        }

        self.unified_finder.set_theme(self.theme());

        // Update codebase search results when in a codebase mode (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Set repo path for source preview to construct full paths
            let repo_path = self
                .codebase_manager
                .index()
                .map(|idx| idx.repo_path.clone());
            self.unified_finder.set_repo_path(repo_path);

            // Search codebase for modes that need it
            // Metrics mode gets codebase metrics APPENDED to live Prometheus metrics
            // Other modes (All, Alerts, Commits) get codebase results SET (replacing any existing)
            let query = self.unified_finder.query_text().to_string();
            let mode = self.unified_finder.mode();
            let needs_codebase = matches!(
                mode,
                FinderMode::All | FinderMode::Alerts | FinderMode::Commits | FinderMode::Metrics
            );
            if needs_codebase && !query.is_empty() && self.unified_finder.needs_codebase_search() {
                let filter = match mode {
                    FinderMode::Metrics => SearchFilter::Metrics,
                    FinderMode::Alerts => SearchFilter::Alerts,
                    FinderMode::Commits => SearchFilter::Commits,
                    FinderMode::All => SearchFilter::All,
                };
                let results = self.codebase_manager.search_ranked(&query, filter, 50);
                if mode == FinderMode::Metrics {
                    // Append to existing live metrics
                    self.unified_finder.append_codebase_results(results);
                } else {
                    // Replace results for other modes
                    self.unified_finder.set_codebase_results(results);
                }
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
            UnifiedFinderAction::OpenDiffViewer {
                hash,
                message,
                diff,
            } => {
                log::info!("Opening diff viewer for commit: {hash}");
                self.diff_viewer.open(&hash, &message, 0, &diff);
                None
            }
            UnifiedFinderAction::Error(msg) => Some(WorkspaceAction::Notify {
                level: "error".to_string(),
                message: msg,
            }),
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
