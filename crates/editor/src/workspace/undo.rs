//! Undo system for workspace operations.
//!
//! This module implements a vim-style undo system using the Command Pattern
//! with inverse operations. Each undoable action stores the minimal data
//! needed to reverse it.

use egui::{Pos2, Vec2};
use egui_tiles::{ContainerKind, TileId};

use super::FloatingPaneId;
use crate::components::Component;

/// Information needed to restore a closed pane.
pub struct ClosedPaneInfo {
    /// The component that was closed
    pub component: Box<dyn Component>,
    /// The parent container's TileId (if it still exists)
    pub parent_id: Option<TileId>,
    /// The index within the parent container where the pane was located
    pub child_index: usize,
    /// The kind of container the pane was in
    pub container_kind: ContainerKind,
    /// Whether this pane was focused when closed
    pub was_focused: bool,
}

/// Information needed to undo a float operation (restore pane to tile tree).
pub struct FloatedPaneInfo {
    /// The floating pane ID that was created
    pub floating_pane_id: FloatingPaneId,
    /// The parent container's TileId before floating (if it existed)
    pub parent_id: Option<TileId>,
    /// The index within the parent container where the pane was located
    pub child_index: usize,
    /// The kind of container the pane was in
    pub container_kind: ContainerKind,
    /// Whether the tile pane was focused before floating
    pub was_tile_focused: bool,
}

/// Information needed to undo a dock operation (restore pane to floating).
pub struct DockedPaneInfo {
    /// The name of the component (used to find it since TileIds can change)
    pub component_name: String,
    /// The floating pane's position before docking
    pub position: Pos2,
    /// The floating pane's size before docking
    pub size: Vec2,
    /// Whether the floating pane was pinned
    pub pinned: bool,
}

/// An action that can be undone.
pub enum UndoAction {
    /// Restore a closed pane to its previous position
    RestorePane(ClosedPaneInfo),
    /// Undo a float operation: remove from floating panes, restore to tile tree
    UnfloatPane(FloatedPaneInfo),
    /// Undo a dock operation: remove from tile tree, restore to floating
    UndockPane(DockedPaneInfo),
}

/// Stack of undo actions with a configurable size limit.
pub struct UndoStack {
    actions: Vec<UndoAction>,
    max_size: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    /// Create a new undo stack with the default max size (50).
    pub fn new() -> Self {
        Self::with_max_size(50)
    }

    /// Create a new undo stack with a custom max size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            actions: Vec::new(),
            max_size,
        }
    }

    /// Push an action onto the stack.
    ///
    /// If the stack exceeds the max size, the oldest action is removed.
    pub fn push(&mut self, action: UndoAction) {
        self.actions.push(action);
        if self.actions.len() > self.max_size {
            self.actions.remove(0);
        }
    }

    /// Pop the most recent action from the stack.
    pub fn pop(&mut self) -> Option<UndoAction> {
        self.actions.pop()
    }

    /// Check if there are any actions to undo.
    pub fn can_undo(&self) -> bool {
        !self.actions.is_empty()
    }

    /// Get the number of actions in the stack.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Check if the stack is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Clear all actions from the stack.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.actions.clear();
    }
}
