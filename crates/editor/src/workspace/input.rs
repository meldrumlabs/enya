//! Keyboard input handling for workspace navigation.
//!
//! This module provides vim-style navigation state and direction handling
//! for navigating between panes in the workspace.

use std::collections::HashSet;

use egui_tiles::TileId;

/// Direction for vim-style navigation between panes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavDirection {
    Left,
    Right,
    Up,
    Down,
}

/// State for visual multi-select mode.
///
/// Allows selecting multiple panes for batch operations
/// (e.g., find & replace across queries, close multiple panes).
#[derive(Debug, Clone, Default)]
pub struct VisualMultiState {
    /// The panes that are currently selected
    pub selected_tile_ids: HashSet<TileId>,
    /// The pane that currently has the cursor (for j/k navigation)
    pub cursor_tile_id: Option<TileId>,
}

impl VisualMultiState {
    /// Create a new visual multi state with the given starting pane
    pub fn new(starting_tile_id: TileId) -> Self {
        let mut selected = HashSet::new();
        selected.insert(starting_tile_id);
        Self {
            selected_tile_ids: selected,
            cursor_tile_id: Some(starting_tile_id),
        }
    }

    /// Toggle selection of a pane
    pub fn toggle_selection(&mut self, tile_id: TileId) {
        if self.selected_tile_ids.contains(&tile_id) {
            self.selected_tile_ids.remove(&tile_id);
        } else {
            self.selected_tile_ids.insert(tile_id);
        }
    }

    /// Check if a pane is selected
    pub fn is_selected(&self, tile_id: TileId) -> bool {
        self.selected_tile_ids.contains(&tile_id)
    }

    /// Get the number of selected panes
    pub fn selection_count(&self) -> usize {
        self.selected_tile_ids.len()
    }

    /// Move cursor to a new pane
    pub fn set_cursor(&mut self, tile_id: TileId) {
        self.cursor_tile_id = Some(tile_id);
    }

    /// Select all given panes
    pub fn select_all(&mut self, tile_ids: &[TileId]) {
        for &tile_id in tile_ids {
            self.selected_tile_ids.insert(tile_id);
        }
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        self.selected_tile_ids.clear();
    }
}
