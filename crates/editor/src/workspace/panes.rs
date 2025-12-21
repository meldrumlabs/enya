//! Pane management methods for the workspace.
//!
//! This module handles adding, removing, splitting, and navigating panes
//! in the tile tree. It includes methods for managing the viewport layout
//! and tracking open charts.

use egui_tiles::{Tile, TileId};

use super::{NavDirection, Workspace, WorkspaceAction};
use crate::components::{Buffer, Component, QueryPane};

impl Workspace {
    // ==================== Pane Adding ====================

    /// Add a chart for a metric and return a tracking action
    pub(super) fn add_chart_for_metric_with_tracking(
        &mut self,
        metric_name: &str,
    ) -> WorkspaceAction {
        // Don't add duplicate charts
        if self.open_charts.contains(metric_name) {
            log::debug!("Chart for {metric_name} already open");
            return WorkspaceAction::None;
        }

        // Create a QueryPane (buffer + chart) for the metric
        // Use real query pane when connected to a backend, demo pane otherwise
        let query_number = self.next_query_number;
        self.next_query_number += 1;
        let pane: Box<dyn Component> = if self.query_executor.is_connected() {
            Box::new(QueryPane::for_metric_with_number(metric_name, query_number))
        } else {
            Box::new(QueryPane::with_demo_metric_numbered(
                metric_name,
                query_number,
            ))
        };
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.open_charts.insert(metric_name.to_string());
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::debug!("Added query pane for {metric_name}");

            // Return action to track this in recent queries
            // Use "Query N" as the display name, metric_name for lookup
            return WorkspaceAction::TrackRecentPlot {
                name: format!("Query {query_number}"),
                metric_name: metric_name.to_string(),
                is_query: false,
            };
        }

        WorkspaceAction::None
    }

    /// Add a demo query pane with a full PromQL query, custom name, and unit (for tutorial)
    pub(super) fn add_demo_query_pane(&mut self, query: &str, name: &str, unit: &str) {
        let pane: Box<dyn Component> =
            Box::new(QueryPane::with_demo_query_named_unit(query, name, unit));
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.open_charts.insert(query.to_string());
            self.behavior.set_focused_tile(Some(pane_tile));
            log::debug!("Added demo query pane: {name}");
        }
    }

    /// Add a tile to the viewport, handling different container types
    /// Returns true if the tile was successfully added
    pub(super) fn add_tile_to_viewport(&mut self, tile_id: TileId) -> bool {
        let Some(root_id) = self.viewport_tree.root() else {
            // No root exists (all panes were closed), create a new tabs container
            let new_root = self.viewport_tree.tiles.insert_tab_tile(vec![tile_id]);
            self.viewport_tree.root = Some(new_root);
            return true;
        };

        match self.viewport_tree.tiles.get_mut(root_id) {
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) => {
                tabs.add_child(tile_id);
                tabs.set_active(tile_id);
                true
            }
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) => {
                linear.add_child(tile_id);
                true
            }
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Grid(grid))) => {
                grid.add_child(tile_id);
                true
            }
            _ => false,
        }
    }

    // ==================== Pane Closing ====================

    /// Close a tile and remove it from the viewport
    pub(super) fn close_tile(&mut self, tile_id: TileId) {
        // Get the pane's label before removing it (for open_charts tracking)
        let label = if let Some(egui_tiles::Tile::Pane(component)) =
            self.viewport_tree.tiles.get(tile_id)
        {
            Some(component.label().text().to_string())
        } else {
            None
        };

        // Find the next tile to focus before removing
        let pane_ids = self.get_pane_tile_ids();
        let next_focus = if pane_ids.len() > 1 {
            // Try to find a sibling to focus
            self.find_sibling_in_direction(tile_id, NavDirection::Right)
                .or_else(|| self.find_sibling_in_direction(tile_id, NavDirection::Left))
                .or_else(|| self.find_sibling_in_direction(tile_id, NavDirection::Down))
                .or_else(|| self.find_sibling_in_direction(tile_id, NavDirection::Up))
                .or_else(|| pane_ids.iter().find(|&&id| id != tile_id).copied())
        } else {
            None
        };

        // Remove the tile from the tree
        self.viewport_tree.tiles.remove(tile_id);

        // Remove from open_charts tracking
        if let Some(label) = label {
            self.open_charts.remove(&label);
            // Also try removing with query: prefix
            self.open_charts.remove(&format!("query:{label}"));
            log::debug!("Closed tile: {label}");
        }

        // Update focus to next tile
        self.behavior.set_focused_tile(next_focus);
    }

    /// Close all charts and reset the viewport to show landing page
    pub(super) fn close_all_charts(&mut self) {
        // Get all pane tile IDs and close them
        let pane_ids = self.get_pane_tile_ids();
        for tile_id in pane_ids {
            self.viewport_tree.tiles.remove(tile_id);
        }

        // Clear tracking
        self.open_charts.clear();
        self.behavior.set_focused_tile(None);
        self.fullscreen_tile = None;
        self.zen_mode = false;

        log::debug!("Closed all charts, showing landing page");
    }

    // ==================== Pane Splitting ====================

    /// Split panes horizontally (`:split` - panes stacked vertically, one above another)
    pub(super) fn split_panes_horizontal(&mut self) {
        let pane_ids = self.get_pane_tile_ids();
        if pane_ids.len() < 2 {
            log::debug!("Need at least 2 panes to split");
            return;
        }

        // Preserve focus on the currently focused pane, or first pane
        let focus_pane = self
            .behavior
            .focused_tile()
            .filter(|id| pane_ids.contains(id))
            .or_else(|| pane_ids.first().copied());

        // Create a new vertical container (panes stacked on top of each other)
        let new_root = self.viewport_tree.tiles.insert_vertical_tile(pane_ids);
        self.viewport_tree.root = Some(new_root);

        // Restore focus
        self.behavior.set_focused_tile(focus_pane);
        log::debug!("Split panes horizontally (vertical layout)");
    }

    /// Split panes vertically (`:vsplit` - panes side by side)
    pub(super) fn split_panes_vertical(&mut self) {
        let pane_ids = self.get_pane_tile_ids();
        if pane_ids.len() < 2 {
            log::debug!("Need at least 2 panes to split");
            return;
        }

        // Preserve focus on the currently focused pane, or first pane
        let focus_pane = self
            .behavior
            .focused_tile()
            .filter(|id| pane_ids.contains(id))
            .or_else(|| pane_ids.first().copied());

        // Create a new horizontal container (panes side by side)
        let new_root = self.viewport_tree.tiles.insert_horizontal_tile(pane_ids);
        self.viewport_tree.root = Some(new_root);

        // Restore focus
        self.behavior.set_focused_tile(focus_pane);
        log::debug!("Split panes vertically (horizontal layout)");
    }

    // ==================== Pane Queries ====================

    /// Get all pane tile IDs in the viewport (for navigation)
    pub(super) fn get_pane_tile_ids(&self) -> Vec<TileId> {
        let mut pane_ids = Vec::new();

        if let Some(root_id) = self.viewport_tree.root() {
            self.collect_pane_ids(root_id, &mut pane_ids);
        }

        pane_ids
    }

    /// Recursively collect all pane tile IDs
    fn collect_pane_ids(&self, tile_id: TileId, pane_ids: &mut Vec<TileId>) {
        if let Some(tile) = self.viewport_tree.tiles.get(tile_id) {
            match tile {
                Tile::Pane(_) => {
                    pane_ids.push(tile_id);
                }
                Tile::Container(container) => {
                    for child_id in container.children() {
                        self.collect_pane_ids(*child_id, pane_ids);
                    }
                }
            }
        }
    }

    /// Count how many panes match the current filter and total panes
    pub(super) fn count_filtered_panes(&self) -> (usize, usize) {
        let pane_ids = self.get_pane_tile_ids();
        let total = pane_ids.len();

        if !self.viewport_filter.is_active() {
            return (total, total);
        }

        let matching = pane_ids
            .iter()
            .filter(|&&tile_id| {
                if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                    // Check QueryPane - match on query content OR tag
                    if let Some(query_pane) = component.as_any().downcast_ref::<QueryPane>() {
                        return self.viewport_filter.matches(query_pane.saved_query())
                            || self.viewport_filter.matches(query_pane.tag());
                    }
                    // Check Buffer
                    if let Some(buffer) = component.as_any().downcast_ref::<Buffer>() {
                        return self.viewport_filter.matches(buffer.saved_content());
                    }
                }
                true // Unknown component types are always shown
            })
            .count();

        (matching, total)
    }

    /// Find a tile by the pane's component ID
    pub(super) fn find_tile_by_pane_id(&self, pane_id: usize) -> Option<TileId> {
        for tile_id in self.get_pane_tile_ids() {
            if let Some(egui_tiles::Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                if component.id() == pane_id {
                    return Some(tile_id);
                }
            }
        }
        None
    }

    // ==================== Tile Activation ====================

    /// Activate a tile (make it the active tab in its parent container)
    pub(super) fn activate_tile(&mut self, tile_id: TileId) {
        // Find the parent tabs container and set this tile as active
        if let Some(root_id) = self.viewport_tree.root() {
            self.activate_tile_in_container(root_id, tile_id);
        }
    }

    /// Recursively find and activate a tile in its parent tabs container
    fn activate_tile_in_container(&mut self, container_id: TileId, target_id: TileId) -> bool {
        if let Some(Tile::Container(container)) = self.viewport_tree.tiles.get(container_id) {
            let children: Vec<TileId> = container.children().copied().collect();

            // Check if target is a direct child
            if children.contains(&target_id) {
                // Set this tile as active in the tabs container
                if let Some(Tile::Container(egui_tiles::Container::Tabs(tabs))) =
                    self.viewport_tree.tiles.get_mut(container_id)
                {
                    tabs.set_active(target_id);
                    return true;
                }
            }

            // Recursively search children
            for child_id in children {
                if self.activate_tile_in_container(child_id, target_id) {
                    return true;
                }
            }
        }
        false
    }
}
