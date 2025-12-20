//! WorkspaceFinder - A telescope/fzf-style finder for saved workspaces.
//!
//! This module provides a finder modal for quickly searching and loading
//! saved workspaces. It uses the generic [`Finder<T>`] abstraction for
//! consistent behavior with other finder modals.
//!
//! # Usage
//!
//! ```ignore
//! let mut finder = WorkspaceFinder::new();
//! finder.set_workspaces(vec![
//!     WorkspaceItem { name: "dashboard".into(), description: None },
//!     WorkspaceItem { name: "api-metrics".into(), description: Some("API monitoring".into()) },
//! ]);
//! finder.open();
//!
//! // In render loop:
//! if let Some(workspace_name) = finder.show(ctx) {
//!     load_workspace(&workspace_name);
//! }
//! ```

use crate::theme::AppTheme;
use crate::ui::semantic_icons;

use crate::components::util::finder::{Finder, FinderConfig, FinderItem};

/// A workspace item for the workspace finder.
///
/// Represents a saved workspace that can be searched and loaded.
#[derive(Debug, Clone)]
pub struct WorkspaceItem {
    /// Workspace name (filename without extension).
    pub name: String,
    /// Optional description of the workspace.
    pub description: Option<String>,
}

impl FinderItem for WorkspaceItem {
    fn search_text(&self) -> &str {
        &self.name
    }

    fn icon(&self) -> &'static str {
        semantic_icons::file::FOLDER
    }

    fn secondary_text(&self) -> Option<String> {
        self.description.clone()
    }
}

/// A telescope/fzf-style finder for saved workspaces.
///
/// This is a thin wrapper around [`Finder<WorkspaceItem>`] that provides
/// workspace-specific configuration and a convenience API.
pub struct WorkspaceFinder {
    /// The underlying generic finder.
    finder: Finder<WorkspaceItem>,
}

impl Default for WorkspaceFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceFinder {
    /// Creates a new workspace finder.
    pub fn new() -> Self {
        let config = FinderConfig {
            placeholder: "Search workspaces...",
            icon: semantic_icons::file::FOLDER_OPEN,
            show_preview: false,
            empty_message: "No results found",
            no_items_message: "No saved workspaces",
        };

        Self {
            finder: Finder::new(config),
        }
    }

    /// Sets the UI theme for styling.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.finder.set_theme(theme);
    }

    /// Returns `true` if the finder is currently visible.
    pub fn is_open(&self) -> bool {
        self.finder.is_open()
    }

    /// Opens the workspace finder modal.
    pub fn open(&mut self) {
        self.finder.open();
    }

    /// Closes the workspace finder modal.
    pub fn close(&mut self) {
        self.finder.close();
    }

    /// Sets the workspaces to search through.
    pub fn set_workspaces(&mut self, workspaces: Vec<WorkspaceItem>) {
        self.finder.set_items(workspaces);
    }

    /// Shows the workspace finder modal.
    ///
    /// Returns `Some(name)` if the user selected a workspace this frame,
    /// where `name` is the workspace name (filename without extension).
    pub fn show(&mut self, ctx: &egui::Context) -> Option<String> {
        self.finder.show(ctx).map(|item| item.name)
    }
}
