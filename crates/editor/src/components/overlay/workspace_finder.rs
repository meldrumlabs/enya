//! WorkspaceFinder - A LazyVim-style overlay for browsing and loading workspaces.
//!
//! This module provides a modal overlay for navigating saved workspaces with
//! vim-style keybindings (j/k navigation, gg/G for jump, / for search).
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
//! match finder.show(ctx) {
//!     WorkspaceFinderResult::Selected(name) => load_workspace(&name),
//!     WorkspaceFinderResult::Closed => {},
//!     WorkspaceFinderResult::None => {},
//! }
//! ```

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use egui::{Color32, Key, RichText, ScrollArea};

#[cfg(not(target_arch = "wasm32"))]
use crate::ui::icons::APP_GHOSTTY;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::{OverlayStyle, render_keyboard_hint_pill};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::{FileOpenerAction, FileOpenerPopup, FileOpenerResult};

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

/// Result from showing the workspace finder.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceFinderResult {
    /// No action
    None,
    /// User selected a workspace to load
    Selected(String),
    /// Overlay was closed
    Closed,
}

/// A LazyVim-style overlay for browsing and loading workspaces.
pub struct WorkspaceFinder {
    /// Whether the overlay is open
    is_open: bool,
    /// Skip first frame of input (to avoid closing on same key that opened it)
    just_opened: bool,
    /// Current theme
    theme: AppTheme,
    /// All workspaces
    workspaces: Vec<WorkspaceItem>,
    /// Currently selected index
    selected_index: usize,
    /// Target selection index for smooth animation
    target_index: usize,
    /// Animation progress (0.0 to 1.0)
    selection_anim_progress: f32,
    /// Search filter text
    search_filter: String,
    /// Whether search input is focused
    search_focused: bool,
    /// Whether 'g' was pressed (for gg navigation)
    g_pressed: bool,
    /// Directory containing workspace TOML files (native only).
    #[cfg(not(target_arch = "wasm32"))]
    workspace_dir: Option<PathBuf>,
    /// File opener popup for opening workspace configs in external apps.
    #[cfg(not(target_arch = "wasm32"))]
    file_opener: FileOpenerPopup,
    /// Flag to open file opener on next render (triggered by 'o' key).
    #[cfg(not(target_arch = "wasm32"))]
    pending_open_file_opener: bool,
}

impl Default for WorkspaceFinder {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceFinder {
    /// Creates a new workspace finder.
    pub fn new() -> Self {
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            workspaces: Vec::new(),
            selected_index: 0,
            target_index: 0,
            selection_anim_progress: 1.0,
            search_filter: String::new(),
            search_focused: false,
            g_pressed: false,
            #[cfg(not(target_arch = "wasm32"))]
            workspace_dir: None,
            #[cfg(not(target_arch = "wasm32"))]
            file_opener: FileOpenerPopup::new(),
            #[cfg(not(target_arch = "wasm32"))]
            pending_open_file_opener: false,
        }
    }

    /// Sets the workspace directory path (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_workspace_dir(&mut self, dir: Option<PathBuf>) {
        self.workspace_dir = dir;
    }

    /// Sets the UI theme for styling.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Returns `true` if the finder is currently visible.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Opens the workspace finder modal.
    pub fn open(&mut self) {
        self.is_open = true;
        self.just_opened = true;
        self.selected_index = 0;
        self.target_index = 0;
        self.search_filter.clear();
        self.search_focused = false;
        self.g_pressed = false;
    }

    /// Closes the workspace finder modal.
    pub fn close(&mut self) {
        self.is_open = false;
        self.search_filter.clear();
        self.search_focused = false;
    }

    /// Sets the workspaces to search through.
    pub fn set_workspaces(&mut self, workspaces: Vec<WorkspaceItem>) {
        self.workspaces = workspaces;
        if self.selected_index >= self.workspaces.len() {
            self.selected_index = 0;
            self.target_index = 0;
        }
    }

    /// Get filtered workspaces based on search filter.
    fn filtered_workspaces(&self) -> impl Iterator<Item = &WorkspaceItem> {
        let filter = self.search_filter.to_lowercase();
        self.workspaces.iter().filter(move |w| {
            filter.is_empty()
                || w.name.to_lowercase().contains(&filter)
                || w.description
                    .as_ref()
                    .is_some_and(|d| d.to_lowercase().contains(&filter))
        })
    }

    /// Get count of filtered workspaces.
    fn filtered_count(&self) -> usize {
        self.filtered_workspaces().count()
    }

    /// Get the currently selected workspace.
    fn selected_workspace(&self) -> Option<&WorkspaceItem> {
        self.filtered_workspaces().nth(self.target_index)
    }

    /// Shows the workspace finder modal.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> WorkspaceFinderResult {
        if !self.is_open {
            return WorkspaceFinderResult::None;
        }

        // Skip input on first frame
        if self.just_opened {
            self.just_opened = false;
            ctx.request_repaint();
            return WorkspaceFinderResult::None;
        }

        let mut result = WorkspaceFinderResult::None;

        // Update selection animation
        if self.selected_index != self.target_index {
            self.selection_anim_progress += ctx.input(|i| i.stable_dt) * 8.0;
            if self.selection_anim_progress >= 1.0 {
                self.selection_anim_progress = 1.0;
                self.selected_index = self.target_index;
            }
            ctx.request_repaint();
        }

        // Handle file opener popup first (consumes keys when open)
        #[cfg(not(target_arch = "wasm32"))]
        let file_opener_open = self.file_opener.is_open();
        #[cfg(target_arch = "wasm32")]
        let file_opener_open = false;

        // Handle keyboard input
        let search_focused = self.search_focused;

        if !file_opener_open {
            ctx.input_mut(|input| {
                // Handle search input when focused
                if search_focused {
                    if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                        self.search_focused = false;
                        self.search_filter.clear();
                        self.selected_index = 0;
                        self.target_index = 0;
                        return;
                    }
                    if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                        self.search_focused = false;
                        return;
                    }
                    if input.consume_key(egui::Modifiers::NONE, Key::Backspace) {
                        self.search_filter.pop();
                        self.selected_index = 0;
                        self.target_index = 0;
                        return;
                    }
                    // Capture typed characters
                    for event in &input.events {
                        if let egui::Event::Text(text) = event {
                            self.search_filter.push_str(text);
                            self.selected_index = 0;
                            self.target_index = 0;
                        }
                    }
                    return;
                }

                // Normal keyboard handling
                if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    result = WorkspaceFinderResult::Closed;
                    return;
                }

                // / - Focus search
                if input.consume_key(egui::Modifiers::NONE, Key::Slash) {
                    self.search_focused = true;
                    return;
                }

                // G - Jump to last item (Shift+g)
                if input.consume_key(egui::Modifiers::SHIFT, Key::G) {
                    let visible_count = self.filtered_count();
                    if visible_count > 0 {
                        self.target_index = visible_count - 1;
                        self.selection_anim_progress = 0.0;
                    }
                    self.g_pressed = false;
                    return;
                }

                // g - First press of gg sequence
                if input.consume_key(egui::Modifiers::NONE, Key::G) {
                    if self.g_pressed {
                        // Second g - jump to first
                        self.target_index = 0;
                        self.selection_anim_progress = 0.0;
                        self.g_pressed = false;
                    } else {
                        self.g_pressed = true;
                    }
                    return;
                } else {
                    // Reset g_pressed if any other key is pressed
                    self.g_pressed = false;
                }

                // j/Down - Move down
                if input.consume_key(egui::Modifiers::NONE, Key::J)
                    || input.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                {
                    let visible_count = self.filtered_count();
                    if visible_count > 0 {
                        self.target_index = (self.target_index + 1) % visible_count;
                        self.selection_anim_progress = 0.0;
                    }
                    return;
                }

                // k/Up - Move up
                if input.consume_key(egui::Modifiers::NONE, Key::K)
                    || input.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                {
                    let visible_count = self.filtered_count();
                    if visible_count > 0 {
                        self.target_index = if self.target_index == 0 {
                            visible_count - 1
                        } else {
                            self.target_index - 1
                        };
                        self.selection_anim_progress = 0.0;
                    }
                    return;
                }

                // o - Open workspace config file
                #[cfg(not(target_arch = "wasm32"))]
                if input.consume_key(egui::Modifiers::NONE, Key::O) {
                    self.pending_open_file_opener = true;
                    return;
                }

                // Enter - Select workspace
                if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    if let Some(workspace) = self.selected_workspace() {
                        result = WorkspaceFinderResult::Selected(workspace.name.clone());
                    }
                }
            });
        }

        if result == WorkspaceFinderResult::Closed {
            self.close();
            return result;
        }

        if matches!(result, WorkspaceFinderResult::Selected(_)) {
            self.close();
            return result;
        }

        // Calculate popup dimensions (match PluginsOverlay)
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.6).clamp(500.0, 800.0);
        let popup_max_height = (screen_rect.height() * 0.7).min(600.0);

        // Pre-calculate colors
        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let separator_color = self.theme.border_subtle();
        let muted_text = self.theme.text_primary().gamma_multiply(0.6);
        let accent_color = self.theme.accent_hover();
        let text_col = self.theme.text_primary();

        egui::Area::new(egui::Id::new("workspace_finder_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);
                    ui.set_max_height(popup_max_height);

                    // Header section
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(semantic_icons::file::FOLDER_OPEN)
                                .color(accent_color)
                                .size(20.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Workspaces")
                                .color(accent_color)
                                .size(18.0)
                                .strong(),
                        );

                        // Right side: Open button and count
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(16.0);

                            // Count
                            let count = self.workspaces.len();
                            ui.label(
                                RichText::new(format!("{count} saved"))
                                    .color(muted_text)
                                    .font(typography::proportional(typography::MD)),
                            );

                            ui.add_space(12.0);

                            // "Open" dropdown button (native only)
                            #[cfg(not(target_arch = "wasm32"))]
                            if self.selected_workspace().is_some() {
                                let btn = ui.add(
                                    egui::Button::image_and_text(
                                        egui::Image::new(APP_GHOSTTY.as_image_source())
                                            .fit_to_exact_size(egui::vec2(14.0, 14.0)),
                                        RichText::new(format!(
                                            "Open {}",
                                            egui_nerdfonts::regular::CHEVRON_DOWN
                                        ))
                                        .size(typography::SM)
                                        .color(self.theme.text_secondary()),
                                    )
                                    .fill(self.theme.bg_elevated())
                                    .stroke(egui::Stroke::new(1.0, self.theme.border_subtle()))
                                    .corner_radius(4.0),
                                );

                                // Open popup on button click or 'o' key press
                                if btn.clicked() || self.pending_open_file_opener {
                                    self.pending_open_file_opener = false;
                                    if let Some(selected) = self.selected_workspace() {
                                        if let Some(ref dir) = self.workspace_dir {
                                            let config_path =
                                                dir.join(format!("{}.toml", selected.name));
                                            let popup_pos = btn.rect.left_bottom();
                                            self.file_opener.open(popup_pos, config_path);
                                        }
                                    }
                                }
                            }
                        });
                    });
                    ui.add_space(12.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(8.0);

                    // Search input or filter indicator
                    if self.search_focused || !self.search_filter.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(semantic_icons::action::SEARCH)
                                    .color(accent_color)
                                    .size(14.0),
                            );
                            ui.add_space(4.0);
                            let search_text = if self.search_filter.is_empty() {
                                "Type to search...".to_string()
                            } else {
                                self.search_filter.clone()
                            };
                            let search_color = if self.search_filter.is_empty() {
                                muted_text.gamma_multiply(0.7)
                            } else {
                                text_col
                            };
                            ui.label(
                                RichText::new(&search_text)
                                    .color(search_color)
                                    .font(typography::proportional(typography::MD)),
                            );
                            if self.search_focused {
                                // Show blinking cursor
                                let blink = (ui.input(|i| i.time) * 2.0) as i32 % 2 == 0;
                                if blink {
                                    ui.label(
                                        RichText::new("|")
                                            .color(accent_color)
                                            .font(typography::proportional(typography::MD)),
                                    );
                                }
                                ui.ctx().request_repaint();
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(16.0);
                                    ui.label(
                                        RichText::new("Esc to clear")
                                            .color(muted_text.gamma_multiply(0.6))
                                            .font(typography::proportional(typography::XS)),
                                    );
                                },
                            );
                        });
                        ui.add_space(4.0);
                    }

                    // Workspace list
                    let list_height = popup_max_height - 140.0;
                    ScrollArea::vertical()
                        .max_height(list_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let filtered: Vec<_> = self.filtered_workspaces().cloned().collect();

                            if filtered.is_empty() {
                                ui.add_space(20.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new(semantic_icons::empty::NO_ITEMS)
                                            .color(muted_text)
                                            .size(32.0),
                                    );
                                    ui.add_space(8.0);
                                    let message = if !self.search_filter.is_empty() {
                                        "No matching workspaces"
                                    } else {
                                        "No saved workspaces"
                                    };
                                    ui.label(
                                        RichText::new(message)
                                            .color(muted_text)
                                            .font(typography::proportional(typography::LG)),
                                    );
                                });
                            } else {
                                for (idx, workspace) in filtered.iter().enumerate() {
                                    let is_selected = idx == self.target_index;
                                    let response = Self::show_workspace_row(
                                        ui,
                                        workspace,
                                        is_selected,
                                        text_col,
                                        accent_color,
                                        muted_text,
                                    );

                                    // Scroll to selected item
                                    if is_selected {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                }
                            }
                        });

                    ui.add_space(8.0);

                    // Separator above footer
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(8.0);

                    // Footer with keyboard hints
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        render_keyboard_hint_pill(ui, "j/k", "nav", muted_text, text_col);
                        ui.add_space(8.0);
                        render_keyboard_hint_pill(ui, "/", "search", muted_text, text_col);
                        ui.add_space(8.0);
                        render_keyboard_hint_pill(ui, "Enter", "load", muted_text, text_col);
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            ui.add_space(8.0);
                            render_keyboard_hint_pill(ui, "o", "open", muted_text, text_col);
                        }
                        ui.add_space(8.0);
                        render_keyboard_hint_pill(ui, "Esc", "close", muted_text, text_col);
                    });
                    ui.add_space(12.0);
                });
            });

        // Show file opener popup and handle result (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.file_opener.show(ctx, self.theme) {
                FileOpenerResult::Selected(action) => {
                    if let Some(path) = self.file_opener.file_path() {
                        match action {
                            FileOpenerAction::OpenIn(app) => {
                                if let Err(e) = app.execute(path) {
                                    log::warn!(
                                        "Failed to open workspace config in {}: {e}",
                                        app.name()
                                    );
                                }
                            }
                            FileOpenerAction::CopyPath => {
                                ctx.copy_text(path.display().to_string());
                            }
                            FileOpenerAction::CopyRelativePath => {
                                // Just use filename for relative path
                                if let Some(filename) = path.file_name() {
                                    ctx.copy_text(filename.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                }
                FileOpenerResult::Closed | FileOpenerResult::None => {}
            }
        }

        result
    }

    /// Show a single workspace row with premium styling.
    fn show_workspace_row(
        ui: &mut egui::Ui,
        workspace: &WorkspaceItem,
        is_selected: bool,
        text_col: Color32,
        accent_color: Color32,
        muted_text: Color32,
    ) -> egui::Response {
        let row_height = 48.0;
        let width = ui.available_width();
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::new(width, row_height), egui::Sense::click());

        let is_hovered = response.hovered();

        // Premium row styling
        if is_selected {
            // Selected: accent-tinted background, full width
            let bg_color = accent_color.gamma_multiply(0.15);
            ui.painter().rect_filled(rect, 0.0, bg_color);

            // Left accent bar
            let bar_rect = egui::Rect::from_min_size(rect.min, egui::Vec2::new(3.0, row_height));
            ui.painter().rect_filled(bar_rect, 0.0, accent_color);
        } else if is_hovered {
            // Hovered: subtle highlight
            let bg_color = text_col.gamma_multiply(0.06);
            ui.painter().rect_filled(rect, 0.0, bg_color);
        }

        // Content area with padding
        let content_left = rect.min.x + 16.0;

        // Folder icon
        let icon_color = if is_selected || is_hovered {
            accent_color
        } else {
            muted_text
        };
        ui.painter().text(
            egui::pos2(content_left + 4.0, rect.center().y - 4.0),
            egui::Align2::LEFT_CENTER,
            semantic_icons::file::FOLDER,
            egui::FontId::proportional(16.0),
            icon_color,
        );

        // Workspace name
        let name_color = if is_selected { accent_color } else { text_col };
        ui.painter().text(
            egui::pos2(content_left + 32.0, rect.center().y - 4.0),
            egui::Align2::LEFT_CENTER,
            &workspace.name,
            typography::proportional(typography::LG),
            name_color,
        );

        // Description (if any)
        if let Some(desc) = &workspace.description {
            let font = typography::proportional(typography::SM);
            let available_width = width - content_left - 32.0 - 16.0; // content start + icon + right margin
            let desc_truncated = truncate_to_width(desc, available_width, font.clone(), ui);
            ui.painter().text(
                egui::pos2(content_left + 32.0, rect.center().y + 14.0),
                egui::Align2::LEFT_CENTER,
                &desc_truncated,
                font,
                muted_text,
            );
        }

        response
    }
}

/// Truncates a string to fit within a given pixel width, adding "..." if truncated.
fn truncate_to_width(text: &str, max_width: f32, font: egui::FontId, ui: &egui::Ui) -> String {
    // Quick check - if the text is short, it probably fits
    if text.len() < 20 {
        return text.to_string();
    }

    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        font.clone(),
        Color32::WHITE, // Color doesn't matter for width calculation
    );

    if galley.size().x <= max_width {
        return text.to_string();
    }

    // Binary search for the right length
    let mut low = 0;
    let mut high = text.chars().count();
    let chars: Vec<char> = text.chars().collect();

    while low < high {
        let mid = (low + high).div_ceil(2);
        let truncated: String = chars[..mid].iter().collect();
        let test_str = format!("{truncated}...");

        let test_galley = ui
            .painter()
            .layout_no_wrap(test_str, font.clone(), Color32::WHITE);

        if test_galley.size().x <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    if low == 0 {
        "...".to_string()
    } else {
        let truncated: String = chars[..low].iter().collect();
        format!("{truncated}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_workspaces() -> Vec<WorkspaceItem> {
        vec![
            WorkspaceItem {
                name: "dashboard".into(),
                description: Some("Main dashboard workspace".into()),
            },
            WorkspaceItem {
                name: "api-metrics".into(),
                description: Some("API monitoring".into()),
            },
            WorkspaceItem {
                name: "infrastructure".into(),
                description: None,
            },
            WorkspaceItem {
                name: "testing".into(),
                description: Some("Test environment".into()),
            },
        ]
    }

    #[test]
    fn test_workspace_finder_default_closed() {
        let finder = WorkspaceFinder::new();
        assert!(!finder.is_open());
    }

    #[test]
    fn test_workspace_finder_open_close() {
        let mut finder = WorkspaceFinder::new();
        finder.open();
        assert!(finder.is_open());

        finder.close();
        assert!(!finder.is_open());
    }

    #[test]
    fn test_open_resets_state() {
        let mut finder = WorkspaceFinder::new();
        finder.set_workspaces(sample_workspaces());
        finder.search_filter = "test".into();
        finder.target_index = 2;
        finder.search_focused = true;
        finder.g_pressed = true;

        finder.open();

        assert_eq!(finder.selected_index, 0);
        assert_eq!(finder.target_index, 0);
        assert!(finder.search_filter.is_empty());
        assert!(!finder.search_focused);
        assert!(!finder.g_pressed);
    }

    #[test]
    fn test_filtered_workspaces_no_filter() {
        let mut finder = WorkspaceFinder::new();
        finder.set_workspaces(sample_workspaces());

        // Without filter, all workspaces should be returned
        assert_eq!(finder.filtered_count(), 4);
    }

    #[test]
    fn test_filtered_workspaces_by_name() {
        let mut finder = WorkspaceFinder::new();
        finder.set_workspaces(sample_workspaces());
        finder.search_filter = "api".into();

        // Only "api-metrics" matches
        assert_eq!(finder.filtered_count(), 1);
        let filtered: Vec<_> = finder.filtered_workspaces().collect();
        assert_eq!(filtered[0].name, "api-metrics");
    }

    #[test]
    fn test_filtered_workspaces_by_description() {
        let mut finder = WorkspaceFinder::new();
        finder.set_workspaces(sample_workspaces());
        finder.search_filter = "monitoring".into();

        // "api-metrics" has "API monitoring" in description
        assert_eq!(finder.filtered_count(), 1);
        let filtered: Vec<_> = finder.filtered_workspaces().collect();
        assert_eq!(filtered[0].name, "api-metrics");
    }

    #[test]
    fn test_filtered_workspaces_case_insensitive() {
        let mut finder = WorkspaceFinder::new();
        finder.set_workspaces(sample_workspaces());
        finder.search_filter = "DASHBOARD".into();

        // Should match "dashboard" case-insensitively
        assert_eq!(finder.filtered_count(), 1);
        let filtered: Vec<_> = finder.filtered_workspaces().collect();
        assert_eq!(filtered[0].name, "dashboard");
    }

    #[test]
    fn test_filtered_workspaces_no_match() {
        let mut finder = WorkspaceFinder::new();
        finder.set_workspaces(sample_workspaces());
        finder.search_filter = "nonexistent".into();

        assert_eq!(finder.filtered_count(), 0);
    }

    #[test]
    fn test_selected_workspace() {
        let mut finder = WorkspaceFinder::new();
        finder.set_workspaces(sample_workspaces());
        finder.target_index = 1;

        let selected = finder.selected_workspace();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name, "api-metrics");
    }

    #[test]
    fn test_selected_workspace_with_filter() {
        let mut finder = WorkspaceFinder::new();
        finder.set_workspaces(sample_workspaces());
        finder.search_filter = "test".into();
        finder.target_index = 0;

        // Should select from filtered list only
        let selected = finder.selected_workspace();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name, "testing");
    }

    #[test]
    fn test_selected_workspace_empty() {
        let finder = WorkspaceFinder::new();
        assert!(finder.selected_workspace().is_none());
    }

    #[test]
    fn test_set_workspaces_resets_index_if_out_of_bounds() {
        let mut finder = WorkspaceFinder::new();
        finder.set_workspaces(sample_workspaces());
        finder.selected_index = 3;
        finder.target_index = 3;

        // Set fewer workspaces
        finder.set_workspaces(vec![WorkspaceItem {
            name: "single".into(),
            description: None,
        }]);

        assert_eq!(finder.selected_index, 0);
        assert_eq!(finder.target_index, 0);
    }

    #[test]
    fn test_workspace_finder_result_variants() {
        // Test that all variants are distinct
        assert_ne!(WorkspaceFinderResult::None, WorkspaceFinderResult::Closed);
        assert_ne!(
            WorkspaceFinderResult::Selected("test".into()),
            WorkspaceFinderResult::None
        );
        assert_eq!(
            WorkspaceFinderResult::Selected("a".into()),
            WorkspaceFinderResult::Selected("a".into())
        );
    }

    #[test]
    fn test_workspace_item_debug() {
        let item = WorkspaceItem {
            name: "test".into(),
            description: Some("desc".into()),
        };
        // Just verify debug impl works
        let _ = format!("{item:?}");
    }
}
