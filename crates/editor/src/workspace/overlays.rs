//! Overlay management methods for the workspace.
//!
//! This module contains methods for managing the diagnostics overlay
//! and other modal overlays in the workspace.

use super::Workspace;
use crate::components::Diagnostic;

impl Workspace {
    // ==================== Diagnostics Methods ====================

    /// Toggle the diagnostics overlay visibility
    pub fn toggle_diagnostics(&mut self) {
        self.diagnostics_pane.toggle();
        self.diagnostics_visible = self.diagnostics_pane.is_open();
    }

    /// Show the diagnostics overlay
    pub fn show_diagnostics(&mut self) {
        self.diagnostics_pane.open();
        self.diagnostics_visible = true;
    }

    /// Hide the diagnostics overlay
    pub fn hide_diagnostics(&mut self) {
        self.diagnostics_pane.close();
        self.diagnostics_visible = false;
    }

    /// Add a diagnostic to the diagnostics pane
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics_pane.add(diagnostic);
    }

    /// Clear all diagnostics
    pub fn clear_diagnostics(&mut self) {
        self.diagnostics_pane.clear();
    }

    /// Clear diagnostics for a specific pane
    pub fn clear_diagnostics_for_pane(&mut self, pane_id: usize) {
        self.diagnostics_pane.clear_for_pane(pane_id);
    }

    /// Get diagnostics count
    pub fn diagnostics_count(&self) -> usize {
        self.diagnostics_pane.count()
    }

    /// Get diagnostics count by level (errors, warnings, infos)
    pub fn diagnostics_count_by_level(&self) -> (usize, usize, usize) {
        let (errors, warnings, infos, _) = self.diagnostics_pane.count_by_level();
        (errors, warnings, infos)
    }

    /// Check if there are any errors
    pub fn has_diagnostic_errors(&self) -> bool {
        self.diagnostics_pane.has_errors()
    }

    /// Check if the diagnostics pane is visible
    pub fn is_diagnostics_visible(&self) -> bool {
        self.diagnostics_visible
    }

    // ==================== End Diagnostics Methods ====================
}
