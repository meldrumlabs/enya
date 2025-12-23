//! Workspace Tab Bar - barbar.nvim style workspace switching.
//!
//! Provides a top-level tab bar for managing multiple workspaces,
//! similar to neovim's barbar.nvim plugin.

use std::path::PathBuf;

use egui::{Color32, Sense, Ui};

use crate::AsyncRuntime;
use crate::theme::AppTheme;
use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::workspace::Workspace;

/// Actions that can be triggered from the workspace tab bar
#[derive(Debug, Clone, PartialEq)]
pub enum TabBarAction {
    /// No action needed
    None,
    /// Switch to a specific tab by index
    SwitchToTab(usize),
    /// Close a specific tab by index
    CloseTab(usize),
    /// Create a new workspace tab
    NewTab,
}

/// Represents an open workspace tab
pub struct WorkspaceTab {
    /// Unique identifier for this tab
    pub id: usize,
    /// Display name for the tab
    pub name: String,
    /// Optional file path for saved workspaces
    pub file_path: Option<PathBuf>,
    /// Whether this workspace has unsaved changes
    pub is_modified: bool,
    /// The workspace state for this workspace (pane layout, modals, etc.)
    pub workspace: Workspace,
}

impl WorkspaceTab {
    /// Create a new workspace tab with a default workspace (shows landing page)
    pub fn new(id: usize, name: String, async_runtime: AsyncRuntime) -> Self {
        Self {
            id,
            name,
            file_path: None,
            is_modified: false,
            workspace: Workspace::new(async_runtime),
        }
    }

    /// Create a new workspace tab with an empty workspace (no landing page)
    pub fn new_empty(id: usize, name: String, async_runtime: AsyncRuntime) -> Self {
        Self {
            id,
            name,
            file_path: None,
            is_modified: false,
            workspace: Workspace::new_empty(async_runtime),
        }
    }

    /// Create a workspace tab from an existing workspace
    pub fn from_workspace(id: usize, name: String, workspace: Workspace) -> Self {
        Self {
            id,
            name,
            file_path: None,
            is_modified: false,
            workspace,
        }
    }
}

/// Manages multiple workspace tabs with barbar.nvim style
pub struct WorkspaceTabBar {
    /// All open workspace tabs
    tabs: Vec<WorkspaceTab>,
    /// Index of the currently active tab
    active_tab_index: usize,
    /// Counter for generating unique tab IDs
    next_tab_id: usize,
    /// Current theme
    theme: AppTheme,
    /// Async runtime for creating new workspaces
    async_runtime: AsyncRuntime,
}

impl WorkspaceTabBar {
    /// Create a new tab bar with an initial default workspace
    pub fn new(async_runtime: AsyncRuntime) -> Self {
        let mut bar = Self {
            tabs: Vec::new(),
            active_tab_index: 0,
            next_tab_id: 0,
            theme: AppTheme::Dark,
            async_runtime,
        };
        // Create initial default workspace (shows landing page)
        bar.new_initial_tab("default".to_string());
        bar
    }

    /// Set the current theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if a workspace name already exists
    pub fn name_exists(&self, name: &str) -> bool {
        self.tabs.iter().any(|tab| tab.name == name)
    }

    /// Generate a unique workspace name based on a base name
    /// If "workspace" exists, tries "workspace-1", "workspace-2", etc.
    pub fn unique_name(&self, base: &str) -> String {
        if !self.name_exists(base) {
            return base.to_string();
        }

        let mut counter = 1;
        loop {
            let candidate = format!("{base}-{counter}");
            if !self.name_exists(&candidate) {
                return candidate;
            }
            counter += 1;
        }
    }

    /// Get the currently active tab
    pub fn active_tab(&self) -> Option<&WorkspaceTab> {
        self.tabs.get(self.active_tab_index)
    }

    /// Get the currently active tab mutably
    pub fn active_tab_mut(&mut self) -> Option<&mut WorkspaceTab> {
        self.tabs.get_mut(self.active_tab_index)
    }

    /// Get the active tab index
    pub fn active_index(&self) -> usize {
        self.active_tab_index
    }

    /// Get the number of open tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Create a new workspace tab with a generated unique name (empty workspace, no landing page)
    pub fn new_tab(&mut self) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let name = self.unique_name("workspace");

        let tab = WorkspaceTab::new_empty(id, name, self.async_runtime.clone());
        self.tabs.push(tab);
        let new_idx = self.tabs.len() - 1;
        self.active_tab_index = new_idx;
        new_idx
    }

    /// Create a new workspace tab with a specific name (empty workspace, no landing page)
    /// The name will be made unique if it already exists.
    pub fn new_tab_with_name(&mut self, name: String) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let unique_name = self.unique_name(&name);

        let tab = WorkspaceTab::new_empty(id, unique_name, self.async_runtime.clone());
        self.tabs.push(tab);
        let new_idx = self.tabs.len() - 1;
        self.active_tab_index = new_idx;
        new_idx
    }

    /// Create the initial default workspace tab (shows landing page)
    fn new_initial_tab(&mut self, name: String) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let unique_name = self.unique_name(&name);

        let tab = WorkspaceTab::new(id, unique_name, self.async_runtime.clone());
        self.tabs.push(tab);
        let new_idx = self.tabs.len() - 1;
        self.active_tab_index = new_idx;
        new_idx
    }

    /// Add an existing workspace as a new tab
    /// The name will be made unique if it already exists.
    pub fn add_workspace_as_tab(&mut self, name: String, workspace: Workspace) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let unique_name = self.unique_name(&name);

        let tab = WorkspaceTab::from_workspace(id, unique_name, workspace);
        self.tabs.push(tab);
        let new_idx = self.tabs.len() - 1;
        self.active_tab_index = new_idx;
        new_idx
    }

    /// Switch to a specific tab by index
    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab_index = index;
        }
    }

    /// Switch to the next tab (wraps around)
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
        }
    }

    /// Switch to the previous tab (wraps around)
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            if self.active_tab_index == 0 {
                self.active_tab_index = self.tabs.len() - 1;
            } else {
                self.active_tab_index -= 1;
            }
        }
    }

    /// Close a tab by index, returns true if successful
    pub fn close_tab(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }

        self.tabs.remove(index);

        // Ensure we always have at least one tab
        if self.tabs.is_empty() {
            // Create a new default tab with landing page
            self.new_initial_tab("default".to_string());
            self.active_tab_index = 0;
        } else if self.active_tab_index >= self.tabs.len() {
            // Adjust active index if we closed the last tab
            self.active_tab_index = self.tabs.len() - 1;
        } else if index < self.active_tab_index {
            // Adjust active index if we closed a tab before it
            self.active_tab_index -= 1;
        }

        true
    }

    /// Mark the active tab as modified
    pub fn mark_modified(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.is_modified = true;
        }
    }

    /// Mark the active tab as saved (not modified)
    pub fn mark_saved(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab_index) {
            tab.is_modified = false;
        }
    }

    /// Render the workspace tab bar and return any triggered action
    pub fn show(&mut self, ctx: &egui::Context) -> TabBarAction {
        let mut action = TabBarAction::None;

        egui::TopBottomPanel::top("workspace_tabs")
            .exact_height(32.0)
            .frame(
                egui::Frame::NONE
                    .fill(self.bg_color())
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    // Render each tab
                    for idx in 0..self.tabs.len() {
                        let is_active = idx == self.active_tab_index;
                        if let Some(tab_action) = self.render_tab(ui, idx, is_active) {
                            action = tab_action;
                        }
                    }

                    // Add new tab button
                    ui.add_space(4.0);
                    if self.render_add_button(ui) {
                        action = TabBarAction::NewTab;
                    }
                });
            });

        action
    }

    /// Render a single tab, returns action if triggered
    fn render_tab(&self, ui: &mut Ui, idx: usize, is_active: bool) -> Option<TabBarAction> {
        let tab = self.tabs.get(idx)?;

        let mut action = None;

        // Tab styling
        let tab_height = 28.0;
        let tab_padding_h = 12.0;
        let tab_padding_v = 2.0;

        // Build tab text with optional modified indicator
        let icon = semantic_icons::file::FOLDER;
        let tab_text = if tab.is_modified {
            format!("{} {} ", icon, tab.name)
        } else {
            format!("{} {}", icon, tab.name)
        };

        // Calculate tab width
        let font_id = egui::FontId::proportional(12.0);
        let text_galley = ui.painter().layout_no_wrap(
            tab_text.clone(),
            font_id.clone(),
            self.text_color(is_active),
        );
        let close_btn_width = if is_active { 20.0 } else { 0.0 };
        let modified_dot_width = if tab.is_modified && !is_active {
            12.0
        } else {
            0.0
        };
        let tab_width =
            text_galley.size().x + tab_padding_h * 2.0 + close_btn_width + modified_dot_width;

        // Allocate tab space
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(tab_width, tab_height), Sense::click());

        if response.clicked() {
            action = Some(TabBarAction::SwitchToTab(idx));
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Background
            let bg = if is_active {
                self.active_bg_color()
            } else if response.hovered() {
                self.hover_bg_color()
            } else {
                self.bg_color()
            };
            painter.rect_filled(rect, 0.0, bg);

            // Active indicator bar at bottom
            if is_active {
                let bar_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x, rect.max.y - 2.0),
                    egui::vec2(rect.width(), 2.0),
                );
                painter.rect_filled(bar_rect, 0.0, palette::accent::PRIMARY);
            }

            // Tab text
            let text_pos = egui::pos2(
                rect.min.x + tab_padding_h,
                rect.center().y - text_galley.size().y / 2.0,
            );
            painter.galley(
                text_pos,
                ui.painter()
                    .layout_no_wrap(tab_text, font_id.clone(), self.text_color(is_active)),
                Color32::TRANSPARENT,
            );

            // Modified dot (for inactive tabs)
            if tab.is_modified && !is_active {
                let dot_pos = egui::pos2(
                    rect.min.x + tab_padding_h + text_galley.size().x + 4.0,
                    rect.center().y,
                );
                painter.circle_filled(dot_pos, 3.0, palette::accent::PRIMARY);
            }

            // Close button (for active tab or on hover)
            if is_active || response.hovered() {
                let close_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        rect.max.x - tab_padding_h - 14.0,
                        rect.center().y - 7.0 + tab_padding_v,
                    ),
                    egui::vec2(14.0, 14.0),
                );

                let close_response =
                    ui.interact(close_rect, ui.id().with(("close", idx)), Sense::click());

                let close_color = if close_response.hovered() {
                    palette::semantic::ERROR
                } else {
                    self.close_btn_color()
                };

                painter.text(
                    close_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "×",
                    egui::FontId::proportional(14.0),
                    close_color,
                );

                if close_response.clicked() {
                    action = Some(TabBarAction::CloseTab(idx));
                }
            }

            // Separator between tabs (except for last tab)
            if idx < self.tabs.len() - 1 {
                let sep_x = rect.max.x;
                painter.vline(
                    sep_x,
                    egui::Rangef::new(rect.min.y + 8.0, rect.max.y - 8.0),
                    egui::Stroke::new(1.0, self.separator_color()),
                );
            }
        }

        action
    }

    /// Render the add new tab button, returns true if clicked
    fn render_add_button(&self, ui: &mut Ui) -> bool {
        let btn_size = 24.0;
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(btn_size, btn_size), Sense::click());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Background on hover
            if response.hovered() {
                painter.rect_filled(rect, 4.0, self.hover_bg_color());
            }

            // Plus icon
            let icon_color = if response.hovered() {
                self.text_color(true)
            } else {
                self.text_color(false)
            };

            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                semantic_icons::action::ADD,
                egui::FontId::proportional(14.0),
                icon_color,
            );
        }

        response.on_hover_text("New workspace (tabnew)").clicked()
    }

    // Color helpers

    fn bg_color(&self) -> Color32 {
        match self.theme {
            AppTheme::Dark => palette::bg::SURFACE,
            AppTheme::Light => palette::light_bg::SURFACE,
        }
    }

    fn active_bg_color(&self) -> Color32 {
        match self.theme {
            AppTheme::Dark => palette::bg::ELEVATED,
            AppTheme::Light => palette::light_bg::ELEVATED,
        }
    }

    fn hover_bg_color(&self) -> Color32 {
        match self.theme {
            AppTheme::Dark => palette::bg::HOVER,
            AppTheme::Light => palette::light_bg::HOVER,
        }
    }

    fn text_color(&self, is_active: bool) -> Color32 {
        match (self.theme, is_active) {
            (AppTheme::Dark, true) => palette::text::PRIMARY,
            (AppTheme::Dark, false) => palette::text::SECONDARY,
            (AppTheme::Light, true) => palette::light_text::PRIMARY,
            (AppTheme::Light, false) => palette::light_text::SECONDARY,
        }
    }

    fn close_btn_color(&self) -> Color32 {
        match self.theme {
            AppTheme::Dark => palette::text::TERTIARY,
            AppTheme::Light => palette::light_text::TERTIARY,
        }
    }

    fn separator_color(&self) -> Color32 {
        match self.theme {
            AppTheme::Dark => palette::border::SUBTLE,
            AppTheme::Light => palette::light_border::SUBTLE,
        }
    }
}
