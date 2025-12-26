//! Pane management methods for the workspace.
//!
//! This module handles adding, removing, splitting, and navigating panes
//! in the tile tree. It includes methods for managing the viewport layout
//! and tracking open charts.

use egui_tiles::{Tile, TileId};

use super::{AgentCommand, NavDirection, Workspace, WorkspaceAction};
use crate::components::{AgentPane, Buffer, Component, QueryPane};

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

    /// Add a query pane with a PromQL query and optional title.
    ///
    /// This is used by the agent to create panes programmatically.
    pub(super) fn add_query_pane(&mut self, query: &str, title: Option<&str>) {
        let query_number = self.next_query_number;
        self.next_query_number += 1;

        // Create the pane with the given query
        let name = title.unwrap_or(query);
        let pane: Box<dyn Component> = if self.query_executor.is_connected() {
            Box::new(QueryPane::with_query_named(query, name, query_number))
        } else {
            Box::new(QueryPane::with_demo_query_named(query, name, query_number))
        };
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.open_charts.insert(query.to_string());
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::info!("Agent created query pane: {}", title.unwrap_or(query));
        }
    }

    /// Add an agent pane to the viewport.
    ///
    /// Creates a new AI chat pane that can run in parallel with query panes.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn add_agent_pane(&mut self) -> Option<TileId> {
        let runtime_handle = self.query_executor.runtime_handle();
        let mut agent_pane = AgentPane::new(runtime_handle);

        // Set editor context for the agent
        self.update_agent_context();
        if let Some(context) = self.build_editor_context() {
            agent_pane.set_context(context);
        }

        let pane: Box<dyn Component> = Box::new(agent_pane);
        let pane_tile = self.viewport_tree.tiles.insert_pane(pane);

        if self.add_tile_to_viewport(pane_tile) {
            self.behavior.set_focused_tile(Some(pane_tile));
            self.show_landing = false;
            log::info!("Added agent pane");
            Some(pane_tile)
        } else {
            None
        }
    }

    /// Add an agent pane (WASM stub - agents not supported in browser).
    #[cfg(target_arch = "wasm32")]
    pub(super) fn add_agent_pane(&mut self) -> Option<TileId> {
        log::warn!("Agent panes are not available in the browser");
        None
    }

    /// Find or create an agent pane. Returns the tile ID.
    ///
    /// If an agent pane already exists, focuses it. Otherwise creates a new one.
    pub(super) fn focus_or_create_agent_pane(&mut self) -> Option<TileId> {
        // Check if we already have an agent pane
        for tile_id in self.get_pane_tile_ids() {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get(tile_id) {
                if component.as_any().downcast_ref::<AgentPane>().is_some() {
                    // Found an existing agent pane, focus it
                    self.behavior.set_focused_tile(Some(tile_id));
                    return Some(tile_id);
                }
            }
        }

        // No agent pane found, create a new one
        self.add_agent_pane()
    }

    /// Handle commands from the AI agent.
    ///
    /// These commands are parsed from the agent's response and executed
    /// to manipulate the workspace (create panes, change time range, etc.)
    #[allow(unused_variables)] // ctx is used conditionally
    pub(super) fn handle_agent_commands(
        &mut self,
        commands: Vec<AgentCommand>,
        ctx: &egui::Context,
    ) {
        for command in commands {
            match command {
                AgentCommand::CreatePane { query, title } => {
                    self.add_query_pane(&query, title.as_deref());
                }
                AgentCommand::SetTimeRange { preset } => {
                    // Parse preset string into a TimeRangePreset
                    if let Some(preset_enum) = Self::parse_time_preset(&preset) {
                        self.time_range_toolbar.set_preset(preset_enum);
                        log::info!("Agent set time range to: {preset}");
                    } else {
                        log::warn!("Agent requested unknown time preset: {preset}");
                    }
                }
                AgentCommand::SearchMetrics { pattern } => {
                    // Open the metrics finder with the pattern
                    self.metrics_finder.open();
                    self.metrics_finder.set_query(&pattern);
                    log::info!("Agent opened metrics search: {pattern}");
                }
                AgentCommand::ShowMetricSource { metric } => {
                    // Open the source preview for the metric definition
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        self.open_metric_definition(&metric);
                        log::info!("Agent opened metric source: {metric}");
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        log::warn!("ShowMetricSource not available on WASM: {metric}");
                    }
                }
                AgentCommand::ShowAlertSource { alert } => {
                    // Open the source preview for the alert rule
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        self.open_alert_definition(&alert);
                        log::info!("Agent opened alert source: {alert}");
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        log::warn!("ShowAlertSource not available on WASM: {alert}");
                    }
                }
            }
        }
    }

    /// Poll all agent panes for pending commands and execute them.
    ///
    /// This should be called during the workspace's show() method to ensure
    /// commands from agent panes are processed.
    pub(super) fn poll_agent_pane_commands(&mut self, ctx: &egui::Context) {
        // Build context once before iterating (avoids borrow issues)
        let context = self.build_editor_context();

        let mut all_commands = Vec::new();
        let pane_ids = self.get_pane_tile_ids();

        // Iterate through all panes and collect commands from agent panes
        for tile_id in pane_ids {
            if let Some(Tile::Pane(component)) = self.viewport_tree.tiles.get_mut(tile_id) {
                if let Some(agent_pane) = component.as_any_mut().downcast_mut::<AgentPane>() {
                    // Update context for the agent
                    if let Some(ref ctx) = context {
                        agent_pane.set_context(ctx.clone());
                    }

                    // Poll for pending commands from this agent pane
                    let commands = agent_pane.poll_pending_commands();
                    if !commands.is_empty() {
                        log::info!(
                            "Agent pane {} produced {} commands",
                            agent_pane.id(),
                            commands.len()
                        );
                    }
                    all_commands.extend(commands);
                }
            }
        }

        // Execute all collected commands
        if !all_commands.is_empty() {
            log::info!("Executing {} agent commands", all_commands.len());
            self.handle_agent_commands(all_commands, ctx);
        }
    }

    /// Parse a time range preset string into the enum.
    fn parse_time_preset(
        preset: &str,
    ) -> Option<crate::components::widget::time_range::TimeRangePreset> {
        use crate::components::widget::time_range::TimeRangePreset;
        match preset.to_lowercase().as_str() {
            "5m" | "5min" | "5 minutes" => Some(TimeRangePreset::Last5Minutes),
            "15m" | "15min" | "15 minutes" => Some(TimeRangePreset::Last15Minutes),
            "30m" | "30min" | "30 minutes" => Some(TimeRangePreset::Last30Minutes),
            "1h" | "1hour" | "1 hour" => Some(TimeRangePreset::Last1Hour),
            "6h" | "6hour" | "6 hours" => Some(TimeRangePreset::Last6Hours),
            "24h" | "1d" | "1day" | "1 day" => Some(TimeRangePreset::Last24Hours),
            "7d" | "7day" | "7 days" | "1 week" | "1w" => Some(TimeRangePreset::Last7Days),
            _ => None,
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
