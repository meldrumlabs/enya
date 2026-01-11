//! Workspace serialization and deserialization.
//!
//! This module handles converting the workspace state to/from `WorkspaceConfig`
//! for persistence (saving/loading workspaces), including layout tree building
//! and extraction.

use rustc_hash::FxHashMap;

use egui_tiles::{Tile, TileId, Tiles};

use super::{
    ConnectionConfig, GitConfig, LayoutConfig, LayoutContainer, LayoutNode, LayoutType, LogsConfig,
    MetricsConfig, PaneConfig, RefreshInterval, TimeConfig, ViewConfig, WORKSPACE_VERSION,
    Workspace, WorkspaceConfig, WorkspaceMeta,
};
use crate::components::{Component, QueryPane};

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

        // Collect all QueryPane data from the viewport tree
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                    let state = query_pane.query_state();
                    panes.push(PaneConfig::from_query_state(
                        query_pane.saved_query(),
                        query_pane.name(),
                        query_pane.tag(),
                        query_pane.description(),
                        state,
                    ));
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
            git: GitConfig::default(),
            view: ViewConfig {
                // Theme is NOT included - it's a user preference, not workspace setting
                zen_mode: self.zen_mode,
                ..Default::default()
            },
            time: TimeConfig::from_preset_with_refresh(
                self.time_range_toolbar.time_range().preset,
                self.refresh_interval.unwrap_or_default(),
            ),
            panes,
            layout: self.extract_layout_from_tree(),
        }
    }

    /// Load a workspace config, replacing current state
    /// Returns the connection config if specified in the workspace
    ///
    /// Note: Theme is NOT loaded from workspace config - it's a user preference
    /// stored in AppSettings, not a per-workspace setting.
    pub fn load_workspace_config(&mut self, config: &WorkspaceConfig) -> Option<ConnectionConfig> {
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

        // Phase 1: Insert all panes and collect their TileIds
        let mut pane_tile_ids: Vec<TileId> = Vec::with_capacity(config.panes.len());

        for pane_config in &config.panes {
            let query_number = self.next_query_number;
            self.next_query_number += 1;

            let mut query_pane = QueryPane::from_config_numbered(
                &pane_config.query,
                &pane_config.name,
                query_number,
            );
            if !pane_config.tag.is_empty() {
                query_pane.set_tag(&pane_config.tag);
            }
            if !pane_config.description.is_empty() {
                query_pane.set_description(&pane_config.description);
            }
            if !pane_config.unit.is_empty() {
                query_pane.set_unit(&pane_config.unit);
            }

            // Apply query state
            let state = pane_config.to_query_state(&config.time.preset);
            query_pane.set_query_state(state);

            // Apply visualization type from config
            query_pane.set_visualization_type(pane_config.visualization_type());

            // Track the chart
            self.open_charts.insert(pane_config.query.clone());

            // Insert pane and record its TileId (don't add to viewport yet)
            let tile_id = self.viewport_tree.tiles.insert_pane(Box::new(query_pane));
            pane_tile_ids.push(tile_id);
        }

        // Phase 2: Build the layout tree
        let root_id = if let Some(layout) = &config.layout {
            // Validate layout references before building
            if let Err(e) = layout.validate(config.panes.len()) {
                log::warn!("Invalid layout config: {e}. Falling back to tabs.");
                self.viewport_tree
                    .tiles
                    .insert_tab_tile(pane_tile_ids.clone())
            } else {
                // Use explicit layout configuration
                self.build_layout_tree(layout, &pane_tile_ids)
            }
        } else {
            // Backward compatibility: no layout = tabs container
            self.viewport_tree
                .tiles
                .insert_tab_tile(pane_tile_ids.clone())
        };

        // Set the root
        self.viewport_tree.root = Some(root_id);

        // Hide landing page if we have panes
        if !config.panes.is_empty() {
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

        // Return connection config if present (for logging/tracking in caller)
        if effective_conn.is_empty() {
            None
        } else {
            Some(effective_conn)
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

    /// Extract layout configuration from the current tile tree
    fn extract_layout_from_tree(&self) -> Option<LayoutConfig> {
        let root_id = self.viewport_tree.root()?;

        // Build a mapping from TileId to pane index
        let pane_ids = self.get_pane_tile_ids();
        let pane_index_map: FxHashMap<TileId, usize> = pane_ids
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

                // Extract shares
                let shares: Vec<f32> = linear
                    .children
                    .iter()
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

    /// Convert a tile to a layout node
    fn tile_to_layout_node(
        &self,
        tile_id: TileId,
        pane_index_map: &FxHashMap<TileId, usize>,
    ) -> Option<LayoutNode> {
        match self.viewport_tree.tiles.get(tile_id)? {
            Tile::Pane(_) => {
                let index = pane_index_map.get(&tile_id)?;
                Some(LayoutNode::Pane(*index))
            }
            Tile::Container(container) => {
                let (layout_type, children, shares) =
                    self.extract_container(container, pane_index_map);
                Some(LayoutNode::Container(LayoutContainer {
                    layout_type,
                    children,
                    shares,
                }))
            }
        }
    }
}
