//! Workspace serialization and deserialization.
//!
//! This module handles converting the workspace state to/from `WorkspaceConfig`
//! for persistence (saving/loading workspaces), including layout tree building
//! and extraction.

use rustc_hash::FxHashMap;

use egui_tiles::{Tile, TileId, Tiles};

use enya_config::SnapshotPaneData;

use super::{
    ConnectionConfig, GitConfig, LayoutConfig, LayoutContainer, LayoutNode, LayoutType, LogsConfig,
    MetricsConfig, PaneConfigExt, PluginsConfig, RefreshInterval, TimeConfigExt, TracingConfig,
    ViewConfig, WORKSPACE_VERSION, Workspace, WorkspaceConfig, WorkspaceMeta,
    pane_from_query_state, time_config_from_preset_with_refresh,
};
use crate::components::{Component, LogsBackend, LogsPane, QueryPane, TracingPane};

impl Workspace {
    // =========================================================================
    // Workspace serialization/deserialization
    // =========================================================================

    /// Serialize the current workspace state to a WorkspaceConfig
    ///
    /// Note: Theme is NOT saved to workspace config - it's a user preference
    /// stored in AppSettings, not a per-workspace setting.
    pub fn to_workspace_config(&self, name: &str, endpoint: Option<&str>) -> WorkspaceConfig {
        let mut panes = Vec::new();
        let mut query_pane_tile_ids = Vec::new();

        // Collect QueryPane data and their TileIds together so pane indices
        // in the layout exactly match the panes array. Non-QueryPane components
        // (LogsPane, PluginPanes, etc.) are excluded from both.
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    let state = query_pane.query_state();
                    let mut pane_config = pane_from_query_state(
                        query_pane.saved_query(),
                        query_pane.name(),
                        query_pane.tag(),
                        query_pane.description(),
                        state,
                    );
                    pane_config.unit = query_pane.unit().to_string();
                    panes.push(pane_config);
                    query_pane_tile_ids.push(tile_id);
                }
            }
        }

        WorkspaceConfig {
            workspace: WorkspaceMeta {
                name: name.to_string(),
                description: String::new(),
                version: WORKSPACE_VERSION,
                endpoint: endpoint.map(|e| e.to_string()).unwrap_or_default(),
            },
            metrics: MetricsConfig::default(),
            logs: LogsConfig::default(),
            tracing: TracingConfig::default(),
            git: GitConfig::default(),
            view: ViewConfig {
                // Theme is NOT included - it's a user preference, not workspace setting
                zen_mode: self.zen_mode,
                ..Default::default()
            },
            time: time_config_from_preset_with_refresh(
                self.time_range_toolbar.time_range().preset,
                self.refresh_interval.unwrap_or_default(),
            ),
            plugins: PluginsConfig::default(),
            panes,
            layout: self.extract_layout_from_tile_ids(&query_pane_tile_ids),
            snapshot: None,
        }
    }

    /// Load a workspace config, replacing current state
    /// Returns the connection config if specified in the workspace
    ///
    /// Note: Theme is NOT loaded from workspace config - it's a user preference
    /// stored in AppSettings, not a per-workspace setting.
    pub fn load_workspace_config(&mut self, config: &WorkspaceConfig) -> Option<ConnectionConfig> {
        // Track whether this workspace is an immutable snapshot
        self.is_snapshot = config.snapshot.is_some();
        self.snapshot_title =
            Some(config.workspace.name.clone()).filter(|n| !n.is_empty() && n != "snapshot");

        // Apply view settings (theme is intentionally NOT loaded - it's a user preference)
        self.zen_mode = config.view.zen_mode;

        // Apply time range
        self.time_range_toolbar.set_preset(config.time.to_preset());

        // Apply refresh interval
        self.set_refresh_interval(RefreshInterval::parse(&config.time.refresh));

        // Clear existing panes and reset the tree
        self.clear_all_panes();

        // Reset query counter for new workspace
        self.next_query_number = 1;

        let all_panes = config.all_panes();
        let pane_count = all_panes.len();

        // Phase 1: Insert all panes and collect their TileIds
        let mut pane_tile_ids: Vec<TileId> = Vec::with_capacity(pane_count);

        for (i, pane_config) in all_panes.iter().enumerate() {
            let query_number = self.next_query_number;
            self.next_query_number += 1;

            // Handle special pane types that aren't standard query visualizations
            if pane_config.visualization == "logs" {
                let now_secs = crate::util::now_unix_secs();
                let end_ns = now_secs * 1_000_000_000;
                let start_ns = end_ns - 3600 * 1_000_000_000; // 1 hour ago
                let pane: Box<dyn Component> =
                    Box::new(LogsPane::with_backend(start_ns, end_ns, LogsBackend::Demo));
                let tile_id = self.viewport_tree.tiles.insert_pane(pane);
                pane_tile_ids.push(tile_id);
                continue;
            }
            if pane_config.visualization == "tracing" {
                let pane: Box<dyn Component> = Box::new(TracingPane::with_demo());
                let tile_id = self.viewport_tree.tiles.insert_pane(pane);
                pane_tile_ids.push(tile_id);
                continue;
            }
            // Use snapshot constructor if this workspace has embedded data
            let snapshot_data = config.snapshot.as_ref().and_then(|s| s.pane_data.get(i));

            let mut query_pane = if let Some(data) = snapshot_data {
                QueryPane::from_snapshot(
                    &pane_config.query,
                    &pane_config.name,
                    query_number,
                    pane_config.visualization_type(),
                    data,
                )
            } else {
                let mut pane = QueryPane::from_config_numbered(
                    &pane_config.query,
                    &pane_config.name,
                    query_number,
                );
                // Apply query state and visualization type for non-snapshot panes
                let state = pane_config.to_query_state(&config.time.preset);
                pane.set_query_state(state);
                pane.set_visualization_type(pane_config.visualization_type());
                // Populate demo data when no backend is configured (e.g. tutorials)
                if config.effective_connection().is_empty() {
                    pane.enable_demo_data();
                }
                pane
            };

            if !pane_config.tag.is_empty() {
                query_pane.set_tag(&pane_config.tag);
            }
            if !pane_config.description.is_empty() {
                query_pane.set_description(&pane_config.description);
            }
            if !pane_config.unit.is_empty() {
                query_pane.set_unit(&pane_config.unit);
            }

            // Track the chart
            self.open_charts.insert(pane_config.query.clone());

            // Insert pane and record its TileId (don't add to viewport yet)
            let tile_id = self.viewport_tree.tiles.insert_pane(Box::new(query_pane));
            pane_tile_ids.push(tile_id);
        }

        // Phase 2: Build the layout tree
        let root_id = if let Some(layout) = &config.layout {
            log::info!(
                "Loading workspace layout: {:?} with {} children",
                layout.layout_type,
                layout.children.len()
            );
            // Validate layout references before building
            if let Err(e) = layout.validate(pane_count) {
                log::warn!("Invalid layout config: {e}. Falling back to tabs.");
                self.viewport_tree
                    .tiles
                    .insert_tab_tile(pane_tile_ids.clone())
            } else {
                // Use explicit layout configuration
                self.build_layout_tree(layout, &pane_tile_ids)
            }
        } else {
            log::debug!("No layout in workspace config, using tabs");
            // Backward compatibility: no layout = tabs container
            self.viewport_tree
                .tiles
                .insert_tab_tile(pane_tile_ids.clone())
        };

        // Set the root
        self.viewport_tree.root = Some(root_id);

        // Hide landing page if we have panes
        if !all_panes.is_empty() {
            self.show_landing = false;
        }

        // Store git URL for deferred initialization (native only with codebase feature)
        // The actual clone/index happens in show() when ctx is available
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pending_git_config = if config.git.is_empty() {
                None
            } else {
                // Set language filter if configured
                self.codebase_manager.set_language(&config.git.language);
                Some(config.git.url.clone())
            };
        }

        // Store connection endpoint for deferred initialization
        // The actual connection happens in show() when ctx is available
        // Use effective_connection() to support both workspace.endpoint and [connection]
        let effective_conn = config.effective_connection();
        self.pending_connection_endpoint = if effective_conn.is_empty() {
            None
        } else {
            Some(effective_conn.endpoint.clone())
        };

        // Focus the first pane (top-left)
        if !pane_tile_ids.is_empty() {
            self.behavior.set_focused_tile(Some(pane_tile_ids[0]));
        }

        // Load snapshot conversation into the agent panel if present
        if let Some(conversation) = config
            .snapshot
            .as_ref()
            .and_then(|s| s.conversation.as_ref())
        {
            self.agent_panel.load_snapshot_conversation(conversation);
        }

        // Load snapshot SQL pane data if present
        if let Some(sql_data) = config.snapshot.as_ref().and_then(|s| s.sql_pane.as_ref()) {
            self.load_sql_snapshot_data(sql_data);
        }

        // Return connection config if present (for logging/tracking in caller)
        if effective_conn.is_empty() {
            None
        } else {
            Some(effective_conn)
        }
    }

    /// Returns true if any pane has loaded visualization data (non-empty).
    pub fn has_pane_data(&self) -> bool {
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    if !query_pane
                        .visualization()
                        .extract_snapshot_data()
                        .is_empty()
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Extract snapshot data from all panes for snapshot sharing.
    pub fn extract_all_snapshot_data(&self) -> Vec<SnapshotPaneData> {
        let mut data = Vec::new();
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    data.push(query_pane.visualization().extract_snapshot_data());
                }
            }
        }
        data
    }

    /// Serialize a subset of panes to a WorkspaceConfig (for multi-pane sharing).
    ///
    /// Only includes panes at the specified indices (0-based, matching `get_pane_tile_ids()` order).
    /// Layout is omitted since a subset doesn't map to the original tree layout.
    pub fn to_workspace_config_for_panes(
        &self,
        name: &str,
        pane_indices: &[usize],
    ) -> WorkspaceConfig {
        let pane_tile_ids = self.get_pane_tile_ids();
        let mut panes = Vec::new();

        for &idx in pane_indices {
            if let Some(&tile_id) = pane_tile_ids.get(idx) {
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        let state = query_pane.query_state();
                        let mut pane_config = pane_from_query_state(
                            query_pane.saved_query(),
                            query_pane.name(),
                            query_pane.tag(),
                            query_pane.description(),
                            state,
                        );
                        pane_config.unit = query_pane.unit().to_string();
                        panes.push(pane_config);
                    }
                }
            }
        }

        WorkspaceConfig {
            workspace: WorkspaceMeta {
                name: name.to_string(),
                description: String::new(),
                version: WORKSPACE_VERSION,
                endpoint: String::new(),
            },
            metrics: MetricsConfig::default(),
            logs: LogsConfig::default(),
            tracing: TracingConfig::default(),
            git: GitConfig::default(),
            view: ViewConfig {
                zen_mode: self.zen_mode,
                ..Default::default()
            },
            time: time_config_from_preset_with_refresh(
                self.time_range_toolbar.time_range().preset,
                self.refresh_interval.unwrap_or_default(),
            ),
            plugins: PluginsConfig::default(),
            // Stack subset panes vertically
            layout: if panes.len() > 1 {
                Some(LayoutConfig {
                    layout_type: LayoutType::Vertical,
                    children: (0..panes.len()).map(LayoutNode::Pane).collect(),
                    shares: Vec::new(), // Equal shares
                })
            } else {
                None
            },
            panes,
            snapshot: None,
        }
    }

    /// Extract snapshot data for a subset of panes (for multi-pane sharing).
    ///
    /// Returns data in the same order as `pane_indices`.
    pub fn extract_snapshot_data_for_panes(&self, pane_indices: &[usize]) -> Vec<SnapshotPaneData> {
        let pane_tile_ids = self.get_pane_tile_ids();
        let mut data = Vec::new();

        for &idx in pane_indices {
            if let Some(&tile_id) = pane_tile_ids.get(idx) {
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        data.push(query_pane.visualization().extract_snapshot_data());
                    }
                }
            }
        }

        data
    }

    /// Returns true if any of the specified panes have loaded visualization data.
    pub fn has_pane_data_for_indices(&self, pane_indices: &[usize]) -> bool {
        let pane_tile_ids = self.get_pane_tile_ids();
        for &idx in pane_indices {
            if let Some(&tile_id) = pane_tile_ids.get(idx) {
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        if !query_pane
                            .visualization()
                            .extract_snapshot_data()
                            .is_empty()
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Returns a reference to the agent panel.
    pub fn agent_panel(&self) -> &crate::components::overlay::AgentPanel {
        &self.agent_panel
    }

    /// Extract snapshot data from the SQL pane (if any).
    pub fn extract_sql_snapshot_data(&self) -> Option<enya_config::SnapshotSqlPane> {
        use crate::components::SqlPane;
        for (_tile_id, tile) in self.viewport_tree.tiles.iter() {
            if let egui_tiles::Tile::Pane(component) = tile {
                if let Some(sql_pane) = component.as_any().downcast_ref::<SqlPane>() {
                    return sql_pane.extract_snapshot_data();
                }
            }
        }
        None
    }

    /// Load snapshot data into the SQL pane (if any).
    fn load_sql_snapshot_data(&mut self, data: &enya_config::SnapshotSqlPane) {
        use crate::components::SqlPane;
        for (_tile_id, tile) in self.viewport_tree.tiles.iter_mut() {
            if let egui_tiles::Tile::Pane(component) = tile {
                if let Some(sql_pane) = component.as_any_mut().downcast_mut::<SqlPane>() {
                    sql_pane.load_snapshot_data(data);
                    return;
                }
            }
        }
    }

    /// Clear all panes from the viewport
    pub(super) fn clear_all_panes(&mut self) {
        // Get all pane IDs
        let pane_ids = self.get_pane_tile_ids();

        // Remove each pane
        for tile_id in pane_ids {
            self.viewport_tree.tiles.remove(tile_id);
        }

        // Reset the tree with an empty tabs container
        let mut tiles: Tiles<Box<dyn Component>> = egui_tiles::Tiles::default();
        let tabs = Vec::new();
        let root = tiles.insert_tab_tile(tabs);
        self.viewport_tree = egui_tiles::Tree::new("viewport_tree", root, tiles);

        // Clear tracking
        self.open_charts.clear();
        self.behavior.set_focused_tile(None);
        self.show_landing = true;
    }

    // ==================== Layout Tree Building ====================

    /// Build the tile tree from a layout configuration
    fn build_layout_tree(&mut self, layout: &LayoutConfig, pane_tile_ids: &[TileId]) -> TileId {
        let container = LayoutContainer {
            layout_type: layout.layout_type,
            children: layout.children.clone(),
            shares: layout.shares.clone(),
        };
        self.build_container(&container, pane_tile_ids)
    }

    /// Recursively build a container and its children
    fn build_container(&mut self, container: &LayoutContainer, pane_tile_ids: &[TileId]) -> TileId {
        // First, resolve all children to TileIds
        let child_ids: Vec<TileId> = container
            .children
            .iter()
            .filter_map(|node| self.resolve_layout_node(node, pane_tile_ids))
            .collect();

        if child_ids.is_empty() {
            // Fallback: create empty tabs container
            return self.viewport_tree.tiles.insert_tab_tile(vec![]);
        }

        match container.layout_type {
            LayoutType::Tabs => self.viewport_tree.tiles.insert_tab_tile(child_ids),
            LayoutType::Horizontal => {
                let container_id = self
                    .viewport_tree
                    .tiles
                    .insert_horizontal_tile(child_ids.clone());

                // Apply shares - use specified shares or default to equal (1.0) for all
                let shares = if container.shares.is_empty() {
                    vec![1.0; child_ids.len()]
                } else {
                    container.shares.clone()
                };
                self.apply_shares(container_id, &child_ids, &shares);

                container_id
            }
            LayoutType::Vertical => {
                let container_id = self
                    .viewport_tree
                    .tiles
                    .insert_vertical_tile(child_ids.clone());

                // Apply shares - use specified shares or default to equal (1.0) for all
                let shares = if container.shares.is_empty() {
                    vec![1.0; child_ids.len()]
                } else {
                    container.shares.clone()
                };
                self.apply_shares(container_id, &child_ids, &shares);

                container_id
            }
        }
    }

    /// Resolve a layout node to a TileId
    fn resolve_layout_node(
        &mut self,
        node: &LayoutNode,
        pane_tile_ids: &[TileId],
    ) -> Option<TileId> {
        match node {
            LayoutNode::Pane(index) => {
                // Get the pre-inserted pane's TileId
                pane_tile_ids.get(*index).copied()
            }
            LayoutNode::Container(container) => {
                // Recursively build nested container
                Some(self.build_container(container, pane_tile_ids))
            }
        }
    }

    /// Apply shares to a linear container
    fn apply_shares(&mut self, container_id: TileId, child_ids: &[TileId], shares: &[f32]) {
        if let Some(Tile::Container(egui_tiles::Container::Linear(linear))) =
            self.viewport_tree.tiles.get_mut(container_id)
        {
            for (i, &child_id) in child_ids.iter().enumerate() {
                let share = shares.get(i).copied().unwrap_or(1.0);
                linear.shares.set_share(child_id, share);
            }
        }
    }

    // ==================== Layout Tree Extraction ====================

    /// Extract layout configuration from the current tile tree, using only the
    /// given pane tile IDs for the pane index mapping. This ensures the layout
    /// indices match the panes array exactly (important when non-QueryPane
    /// components like LogsPane or PluginPanes are present in the tree).
    fn extract_layout_from_tile_ids(&self, pane_tile_ids: &[TileId]) -> Option<LayoutConfig> {
        let root_id = self.viewport_tree.root()?;

        // Build a mapping from TileId to pane index using only the provided IDs
        let pane_index_map: FxHashMap<TileId, usize> = pane_tile_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        // Extract the root container
        match self.viewport_tree.tiles.get(root_id)? {
            Tile::Container(container) => {
                let (layout_type, children, shares) =
                    self.extract_container(container, &pane_index_map);
                Some(LayoutConfig {
                    layout_type,
                    children,
                    shares,
                })
            }
            Tile::Pane(_) => {
                // Single pane - wrap in tabs
                let index = pane_index_map.get(&root_id)?;
                Some(LayoutConfig {
                    layout_type: LayoutType::Tabs,
                    children: vec![LayoutNode::Pane(*index)],
                    shares: Vec::new(),
                })
            }
        }
    }

    /// Extract a container's layout configuration
    fn extract_container(
        &self,
        container: &egui_tiles::Container,
        pane_index_map: &FxHashMap<TileId, usize>,
    ) -> (LayoutType, Vec<LayoutNode>, Vec<f32>) {
        match container {
            egui_tiles::Container::Tabs(tabs) => {
                let children: Vec<LayoutNode> = tabs
                    .children
                    .iter()
                    .filter_map(|&id| self.tile_to_layout_node(id, pane_index_map))
                    .collect();
                (LayoutType::Tabs, children, Vec::new())
            }
            egui_tiles::Container::Linear(linear) => {
                let layout_type = match linear.dir {
                    egui_tiles::LinearDir::Horizontal => LayoutType::Horizontal,
                    egui_tiles::LinearDir::Vertical => LayoutType::Vertical,
                };

                let children: Vec<LayoutNode> = linear
                    .children
                    .iter()
                    .filter_map(|&id| self.tile_to_layout_node(id, pane_index_map))
                    .collect();

                // Extract shares only for children that were included (filter_map may skip some)
                let shares: Vec<f32> = linear
                    .children
                    .iter()
                    .filter(|&&id| {
                        // Include share only if this child produced a layout node
                        self.tile_produces_layout_node(id, pane_index_map)
                    })
                    .map(|&id| linear.shares[id])
                    .collect();

                // Only include shares if they differ from default (all 1.0)
                let all_default = shares.iter().all(|&s| (s - 1.0).abs() < 0.01);
                let shares = if all_default { Vec::new() } else { shares };

                (layout_type, children, shares)
            }
            egui_tiles::Container::Grid(_) => {
                // Grid not supported in this schema - convert to tabs
                let children: Vec<LayoutNode> = container
                    .children()
                    .filter_map(|&id| self.tile_to_layout_node(id, pane_index_map))
                    .collect();
                (LayoutType::Tabs, children, Vec::new())
            }
        }
    }

    /// Check if a tile would produce a layout node (used for share alignment)
    fn tile_produces_layout_node(
        &self,
        tile_id: TileId,
        pane_index_map: &FxHashMap<TileId, usize>,
    ) -> bool {
        self.tile_to_layout_node(tile_id, pane_index_map).is_some()
    }

    /// Convert a tile to a layout node.
    ///
    /// Normalizes single-pane Tabs wrappers (added by `all_panes_must_have_tabs`)
    /// back to bare Pane nodes for a cleaner, more compact layout representation.
    /// Tiles not in the pane_index_map (non-QueryPane components) are skipped.
    fn tile_to_layout_node(
        &self,
        tile_id: TileId,
        pane_index_map: &FxHashMap<TileId, usize>,
    ) -> Option<LayoutNode> {
        match self.viewport_tree.tiles.get(tile_id)? {
            Tile::Pane(_) => {
                // Only include panes that are in our index map (QueryPanes)
                let index = pane_index_map.get(&tile_id)?;
                Some(LayoutNode::Pane(*index))
            }
            Tile::Container(container) => {
                let (layout_type, children, shares) =
                    self.extract_container(container, pane_index_map);

                // Skip empty containers (all children were non-QueryPane)
                if children.is_empty() {
                    return None;
                }

                // Unwrap single-pane Tabs wrappers: these are added by egui_tiles'
                // `all_panes_must_have_tabs` simplification and are a rendering detail,
                // not part of the user's intended layout.
                if layout_type == LayoutType::Tabs
                    && children.len() == 1
                    && matches!(children.first(), Some(LayoutNode::Pane(_)))
                {
                    return children.into_iter().next();
                }

                Some(LayoutNode::Container(LayoutContainer {
                    layout_type,
                    children,
                    shares,
                }))
            }
        }
    }
}
