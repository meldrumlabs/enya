//! Beautiful diff viewer overlay with GitHub-style styling.
//!
//! Features:
//! - **Side panel file list** - Version control style file tree on the right
//! - **Word-level diff highlighting** - Shows exactly which characters changed
//! - **Syntax highlighting** - Tree-sitter language-aware colors layered under diffs
//! - **Split view toggle** - Switch between unified and side-by-side diff views
//! - **Dual line numbers** - Old and new line numbers in the gutter
//! - **Colored gutter stripes** - Green/red bars for add/remove
//! - **Expandable context** - Click hunk separators to reveal surrounding lines
//! - **Search** - Inline search across all files with match cycling
//! - **Commit info header** - Shows hash, message, and file stats
//!
//! # Keyboard Shortcuts
//!
//! - `s` - Toggle split/unified view
//! - `n` / `p` - Next/previous changed file
//! - `j` / `k` - Scroll down/up
//! - `h` / `l` - Scroll left/right
//! - `{` / `}` - Jump to previous/next hunk
//! - `/` or `⌘F` - Open search bar
//! - `⌘C` - Copy selected lines
//! - `o` - Open file in external app
//! - `Escape` - Close search → clear selection → close overlay

use std::path::PathBuf;

use egui::{Color32, Key, RichText};

use crate::components::OverlayColors;
use crate::components::util::file_opener::{FileOpenerAction, FileOpenerPopup, FileOpenerResult};
use crate::components::util::finder_utils::{OverlayStyle, draw_backdrop};
use crate::git::diff::{self, FileDiff};
use crate::git::diff_renderer::{DiffKeyAction, DiffRenderer};
#[cfg(not(target_arch = "wasm32"))]
use crate::ui::icons::APP_GHOSTTY;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Result of showing the diff viewer overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffViewerResult {
    /// No action taken.
    None,
    /// Overlay was closed.
    Closed,
    /// An error occurred (e.g., file not found).
    Error(String),
}

/// A modal overlay that displays a commit diff with file navigation.
pub struct DiffViewerOverlay {
    /// Whether the overlay is open.
    is_open: bool,
    /// Current theme.
    theme: AppTheme,
    /// Commit hash being viewed.
    commit_hash: String,
    /// Commit message.
    commit_message: String,
    /// Parsed file diffs.
    file_diffs: Vec<FileDiff>,
    /// Currently selected file index.
    current_file_index: usize,
    /// Disable keyboard handling (when another overlay is on top).
    keyboard_disabled: bool,
    /// Repository root path for computing full file paths.
    repo_root: Option<PathBuf>,
    /// File opener popup for opening files in external apps.
    file_opener: FileOpenerPopup,
    /// Flag to open file opener on next render (triggered by 'o' key).
    pending_open_file_opener: bool,
    /// Shared diff renderer with search, selection, and hunk navigation.
    diff_renderer: DiffRenderer,
}

impl Default for DiffViewerOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffViewerOverlay {
    /// Creates a new diff viewer overlay.
    pub fn new() -> Self {
        Self {
            is_open: false,
            theme: AppTheme::Dark,
            commit_hash: String::new(),
            commit_message: String::new(),
            file_diffs: Vec::new(),
            current_file_index: 0,
            keyboard_disabled: false,
            pending_open_file_opener: false,
            repo_root: None,
            file_opener: FileOpenerPopup::new(),
            diff_renderer: DiffRenderer::new("diff_viewer", typography::MD),
        }
    }

    /// Disable keyboard handling (call when another overlay is on top).
    pub fn set_keyboard_disabled(&mut self, disabled: bool) {
        self.keyboard_disabled = disabled;
    }

    /// Sets the UI theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.file_opener.set_theme(theme);
    }

    /// Sets the repository root path for computing full file paths.
    pub fn set_repo_root(&mut self, path: Option<PathBuf>) {
        self.repo_root = path;
    }

    /// Returns true if the overlay is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Opens the overlay with a commit diff.
    pub fn open(&mut self, hash: &str, message: &str, diff_content: &str) {
        self.commit_hash = hash.to_string();
        self.commit_message = message.to_string();
        self.file_diffs = diff::parse_diff_into_files(diff_content);
        self.current_file_index = 0;
        self.diff_renderer.reset_for_file_change();
        self.diff_renderer.close_search();
        self.is_open = true;

        // Load full file contents for context expansion (native only)
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ref repo_root) = self.repo_root {
            let commit = self.commit_hash.clone();
            for file in &mut self.file_diffs {
                let path = file.path.clone();
                // Old version: parent commit
                file.old_file_lines =
                    diff::load_file_at_commit(repo_root, &format!("{commit}^"), &path);
                // New version: this commit
                file.new_file_lines = diff::load_file_at_commit(repo_root, &commit, &path);
            }
        }

        log::debug!(
            "DiffViewerOverlay::open() - {} files in diff",
            self.file_diffs.len()
        );
    }

    /// Closes the overlay.
    pub fn close(&mut self) {
        self.is_open = false;
        self.diff_renderer.close_search();
    }

    /// Show the overlay. Returns the result of the interaction.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> DiffViewerResult {
        if !self.is_open {
            return DiffViewerResult::None;
        }

        let mut should_close = false;
        let mut clear_focus = false;

        // Handle keyboard input (unless another overlay is on top or file opener is open)
        if !self.keyboard_disabled && !self.file_opener.is_open() {
            ctx.input_mut(|i| {
                // Escape: close search -> clear selection -> close overlay
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    if self.diff_renderer.search_active() {
                        self.diff_renderer.close_search();
                        clear_focus = true;
                    } else if self.diff_renderer.selected_lines().is_some() {
                        self.diff_renderer.clear_selection();
                    } else {
                        should_close = true;
                    }
                }

                // Delegate standard diff keys to renderer
                let action = self.diff_renderer.handle_keyboard(i);
                match action {
                    DiffKeyAction::NextFile => {
                        if !self.file_diffs.is_empty() {
                            self.current_file_index =
                                (self.current_file_index + 1) % self.file_diffs.len();
                            self.diff_renderer.reset_for_file_change();
                        }
                    }
                    DiffKeyAction::PrevFile => {
                        if !self.file_diffs.is_empty() {
                            self.current_file_index = if self.current_file_index == 0 {
                                self.file_diffs.len() - 1
                            } else {
                                self.current_file_index - 1
                            };
                            self.diff_renderer.reset_for_file_change();
                        }
                    }
                    DiffKeyAction::CopySelected => {
                        if let Some(file_diff) = self.file_diffs.get(self.current_file_index) {
                            if let Some(text) = self.diff_renderer.copy_selected(file_diff) {
                                ctx.copy_text(text);
                            }
                        }
                    }
                    DiffKeyAction::OpenFile => {
                        self.pending_open_file_opener = true;
                    }
                    DiffKeyAction::CommentOnSelected => {
                        // Not supported in the overlay diff viewer
                    }
                    DiffKeyAction::None => {}
                }
            });
        }

        if clear_focus {
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
        }

        if should_close {
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
            return DiffViewerResult::Closed;
        }

        // Draw backdrop
        draw_backdrop(ctx, self.theme, "diff_viewer");

        // Calculate popup dimensions - larger to accommodate side panel
        let popup_width = crate::util::overlay_width(ctx, 0.85, 700.0, 1400.0);
        let popup_max_height = crate::util::overlay_height(ctx, 0.85, 500.0, 900.0);

        // File panel width
        let file_panel_width = 240.0;

        egui::Area::new(egui::Id::new("diff_viewer_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .constrain_to(crate::util::overlay_content_rect(ctx))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Extract colors from theme (Custom variant handles plugin colors internally)
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let colors = OverlayColors::new(self.theme);
                let separator_color = colors.separator;
                let muted_text = colors.muted_text;

                overlay_style.frame().show(ui, |ui| {
                    // Cap width/height to prevent content from stretching the overlay
                    ui.set_width(popup_width);
                    ui.set_max_width(popup_width);
                    ui.set_max_height(popup_max_height);

                    // Header section (commit info only, no file tabs)
                    self.render_header(ui, &colors, separator_color);

                    // Search bar (when active)
                    if self.diff_renderer.search_active() {
                        if let Some(new_file) = self.diff_renderer.render_search_bar(
                            ui,
                            self.theme,
                            &self.file_diffs,
                            self.current_file_index,
                        ) {
                            self.current_file_index = new_file;
                        }
                    }

                    // Main content: horizontal split with diff on left, file panel on right
                    // Calculate content height (leave room for footer)
                    let content_height = (ui.available_height() - 50.0).max(100.0);

                    ui.horizontal(|ui| {
                        // Left side: Diff content (takes remaining width)
                        let diff_width = (popup_width - file_panel_width - 24.0).max(1.0); // account for margins
                        ui.allocate_ui_with_layout(
                            egui::vec2(diff_width, content_height),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                let file_idx = self.current_file_index;
                                let theme = self.theme;
                                if let Some(file_diff) = self.file_diffs.get(file_idx) {
                                    let file_diff_clone = file_diff.clone();
                                    self.diff_renderer.render_diff(
                                        ui,
                                        &file_diff_clone,
                                        file_idx,
                                        theme,
                                        None,
                                    );
                                }
                                // Process hunk expansion
                                if let Some(hunk_idx) = self.diff_renderer.take_pending_expand() {
                                    self.expand_context(hunk_idx);
                                }
                            },
                        );

                        // Vertical separator
                        let separator_rect = ui.available_rect_before_wrap();
                        ui.painter().vline(
                            separator_rect.left(),
                            separator_rect.y_range(),
                            egui::Stroke::new(1.0, separator_color),
                        );

                        // Right side: File panel
                        ui.allocate_ui_with_layout(
                            egui::vec2(file_panel_width, content_height),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                self.render_file_panel(ui, &colors, separator_color);
                            },
                        );
                    });

                    // Footer
                    self.render_footer(ui, muted_text, separator_color);
                });
            });

        // Show file opener popup if open
        if self.file_opener.is_open() {
            match self.file_opener.show(ctx, self.theme) {
                FileOpenerResult::Selected(action) => {
                    if let Some(error) = self.handle_file_opener_action(&action, ctx) {
                        return DiffViewerResult::Error(error);
                    }
                }
                FileOpenerResult::Closed | FileOpenerResult::None => {}
            }
        }

        DiffViewerResult::None
    }

    /// Handle file opener action. Returns an error message if the action failed.
    fn handle_file_opener_action(
        &self,
        action: &FileOpenerAction,
        ctx: &egui::Context,
    ) -> Option<String> {
        match action {
            FileOpenerAction::OpenIn(app) => {
                if let Some(path) = self.file_opener.file_path() {
                    // Compute full path if we have a repo root
                    let full_path = if let Some(ref root) = self.repo_root {
                        root.join(path)
                    } else {
                        path.to_path_buf()
                    };
                    log::debug!(
                        "DiffViewer: Opening file - repo_root: {:?}, path: {:?}, full_path: {:?}",
                        self.repo_root,
                        path,
                        full_path
                    );
                    if let Err(e) = app.execute(&full_path) {
                        log::warn!("Failed to open file: {e}");
                        return Some(e);
                    }
                } else {
                    log::warn!("DiffViewer: No file path available for file opener");
                    return Some("No file path available".to_string());
                }
            }
            FileOpenerAction::CopyPath => {
                if let Some(path) = self.file_opener.file_path() {
                    let full_path = if let Some(ref root) = self.repo_root {
                        root.join(path)
                    } else {
                        path.to_path_buf()
                    };
                    ctx.copy_text(full_path.display().to_string());
                }
            }
            FileOpenerAction::CopyRelativePath => {
                if let Some(path) = self.file_opener.file_path() {
                    ctx.copy_text(path.display().to_string());
                }
            }
        }
        None
    }

    /// Renders the header with commit info (no file tabs - those are now in the side panel).
    fn render_header(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        separator_color: Color32,
    ) {
        // ===== Commit info row =====
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Git commit icon
            ui.label(
                RichText::new(egui_nerdfonts::regular::GIT_COMMIT)
                    .color(colors.accent)
                    .size(16.0),
            );
            ui.add_space(6.0);

            // Commit hash (short)
            let short_hash = &self.commit_hash[..7.min(self.commit_hash.len())];
            ui.label(
                RichText::new(short_hash)
                    .color(colors.accent)
                    .font(typography::monospace(typography::MD))
                    .strong(),
            );

            ui.add_space(12.0);

            // Commit message (truncated)
            let msg = if self.commit_message.chars().count() > 80 {
                let truncated: String = self.commit_message.chars().take(77).collect();
                format!("{truncated}...")
            } else {
                self.commit_message.clone()
            };
            ui.label(
                RichText::new(msg)
                    .color(colors.text.gamma_multiply(0.8))
                    .font(typography::proportional(typography::MD)),
            );

            // Right side: Open in button and total stats
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // Total additions/deletions across all files
                let total_adds: usize = self.file_diffs.iter().map(|f| f.additions).sum();
                let total_dels: usize = self.file_diffs.iter().map(|f| f.deletions).sum();

                if total_dels > 0 {
                    render_stat_badge(ui, total_dels, false, self.theme);
                    ui.add_space(4.0);
                }
                if total_adds > 0 {
                    render_stat_badge(ui, total_adds, true, self.theme);
                    ui.add_space(8.0);
                }

                // File count
                ui.label(
                    RichText::new(format!("{} files changed", self.file_diffs.len()))
                        .color(colors.muted_text)
                        .font(typography::proportional(typography::SM)),
                );

                ui.add_space(12.0);

                // "Open" dropdown button (native only)
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(file_diff) = self.file_diffs.get(self.current_file_index) {
                    let file_path = file_diff.path.clone();

                    // Button with Ghostty icon and "Open" text
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
                        let popup_pos = btn.rect.left_bottom();
                        self.file_opener.open_with_base(
                            popup_pos,
                            std::path::PathBuf::from(&file_path),
                            self.repo_root.clone(),
                        );
                    }
                }
            });
        });
        ui.add_space(8.0);

        // Separator below header
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator_color),
        );
    }

    /// Expand context around a hunk header by splicing in lines from the full file.
    fn expand_context(&mut self, hunk_line_idx: usize) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(file_diff) = self.file_diffs.get_mut(self.current_file_index) {
            self.diff_renderer.expand_context(file_diff, hunk_line_idx);
        }
        // On WASM, context expansion is unavailable (requires git CLI)
        #[cfg(target_arch = "wasm32")]
        let _ = hunk_line_idx;
    }

    /// Renders the file panel on the right side with version control style file list.
    fn render_file_panel(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        _separator_color: Color32,
    ) {
        // Panel header
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new("Changed Files")
                    .color(colors.text.gamma_multiply(0.9))
                    .font(typography::proportional(typography::SM))
                    .strong(),
            );
        });
        ui.add_space(8.0);

        // Scrollable file list
        egui::ScrollArea::vertical()
            .id_salt("diff_file_panel")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, file) in self.file_diffs.iter().enumerate() {
                    let is_selected = i == self.current_file_index;

                    // Extract filename and directory from path
                    let (filename, directory) = {
                        let path = std::path::Path::new(&file.path);
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(&file.path);
                        let dir = path
                            .parent()
                            .and_then(|p| p.to_str())
                            .filter(|s| !s.is_empty());
                        (name, dir)
                    };

                    // Row styling
                    let row_height = 32.0;
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );

                    let is_hovered = response.hovered();

                    // Background
                    if is_selected {
                        ui.painter()
                            .rect_filled(rect, 4.0, colors.accent.gamma_multiply(0.15));
                        // Left accent bar
                        let bar_rect =
                            egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height));
                        ui.painter().rect_filled(bar_rect, 2.0, colors.accent);
                    } else if is_hovered {
                        ui.painter()
                            .rect_filled(rect, 4.0, colors.text.gamma_multiply(0.05));
                    }

                    // File icon based on change type
                    let icon = if file.deletions > 0 && file.additions > 0 {
                        egui_nerdfonts::regular::FILE_EDIT // Modified
                    } else if file.deletions > 0 {
                        egui_nerdfonts::regular::FILE_MINUS // Deleted
                    } else {
                        egui_nerdfonts::regular::FILE_PLUS // Added
                    };

                    let icon_color = if is_selected {
                        colors.accent
                    } else {
                        colors.muted_text
                    };

                    // Layout the row content
                    let content_rect = rect.shrink2(egui::vec2(8.0, 0.0));
                    let mut cursor_x = content_rect.left() + 4.0;

                    // File icon
                    let icon_galley = ui.painter().layout_no_wrap(
                        icon.to_string(),
                        typography::proportional(typography::SM),
                        icon_color,
                    );
                    ui.painter().galley(
                        egui::pos2(
                            cursor_x,
                            content_rect.center().y - icon_galley.size().y / 2.0,
                        ),
                        icon_galley.clone(),
                        icon_color,
                    );
                    cursor_x += icon_galley.size().x + 6.0;

                    // Filename
                    let name_color = if is_selected {
                        colors.text
                    } else {
                        colors.text.gamma_multiply(0.85)
                    };

                    // Truncate filename if needed
                    let max_name_width =
                        content_rect.width() - (cursor_x - content_rect.left()) - 50.0;
                    let name_galley = ui.painter().layout(
                        filename.to_string(),
                        typography::monospace(typography::SM),
                        name_color,
                        max_name_width,
                    );
                    ui.painter().galley(
                        egui::pos2(
                            cursor_x,
                            content_rect.center().y - name_galley.size().y / 2.0,
                        ),
                        name_galley,
                        name_color,
                    );

                    // Stats on the right
                    let stats_x = content_rect.right() - 4.0;

                    // Deletions (show first, positioned from right)
                    let mut right_x = stats_x;
                    if file.deletions > 0 {
                        let del_text = format!("-{}", file.deletions);
                        let del_galley = ui.painter().layout_no_wrap(
                            del_text,
                            typography::monospace(typography::XS),
                            self.theme.diff_removed_gutter(),
                        );
                        right_x -= del_galley.size().x;
                        ui.painter().galley(
                            egui::pos2(
                                right_x,
                                content_rect.center().y - del_galley.size().y / 2.0,
                            ),
                            del_galley,
                            self.theme.diff_removed_gutter(),
                        );
                        right_x -= 4.0;
                    }

                    // Additions
                    if file.additions > 0 {
                        let add_text = format!("+{}", file.additions);
                        let add_galley = ui.painter().layout_no_wrap(
                            add_text,
                            typography::monospace(typography::XS),
                            self.theme.diff_added_gutter(),
                        );
                        right_x -= add_galley.size().x;
                        ui.painter().galley(
                            egui::pos2(
                                right_x,
                                content_rect.center().y - add_galley.size().y / 2.0,
                            ),
                            add_galley,
                            self.theme.diff_added_gutter(),
                        );
                    }

                    // Handle left click on the row - select file
                    if response.clicked() {
                        self.current_file_index = i;
                        self.diff_renderer.reset_for_file_change();
                    }

                    // Scroll selected item into view
                    if is_selected {
                        response.clone().scroll_to_me(Some(egui::Align::Center));
                    }

                    // Show directory path in tooltip on hover
                    if let Some(dir) = directory {
                        response.on_hover_text(dir);
                    }
                }
            });
    }

    /// Renders the footer with keyboard hints.
    fn render_footer(&self, ui: &mut egui::Ui, muted_text: Color32, separator_color: Color32) {
        // Separator above footer
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator_color),
        );
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Current file path (full path for context)
            if let Some(file_diff) = self.file_diffs.get(self.current_file_index) {
                ui.label(
                    RichText::new(&file_diff.path)
                        .color(muted_text)
                        .font(typography::monospace(typography::SM)),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // Keyboard hint - show current view mode and available shortcuts
                let view_mode = if self.diff_renderer.split_view() { "split" } else { "unified" };
                let copy_hint = if self.diff_renderer.selected_lines().is_some() {
                    " • ⌘C copy"
                } else {
                    ""
                };
                let hint = if self.file_diffs.len() > 1 {
                    format!(
                        "/ search • o open • s {view_mode} • n/p files • {{/}} hunks • j/k scroll{copy_hint} • Esc"
                    )
                } else {
                    format!(
                        "/ search • o open • s {view_mode} • {{/}} hunks • j/k scroll{copy_hint} • Esc"
                    )
                };
                ui.label(
                    RichText::new(hint)
                        .color(muted_text.gamma_multiply(0.7))
                        .font(typography::proportional(typography::XS)),
                );
            });
        });
        ui.add_space(8.0);
    }
}

/// Renders a +N or -N stat badge.
fn render_stat_badge(ui: &mut egui::Ui, count: usize, is_addition: bool, theme: AppTheme) {
    let (text, text_color, bg_color) = if is_addition {
        (
            format!("+{count}"),
            theme.diff_added_text(),
            theme.diff_added_bg(),
        )
    } else {
        (
            format!("-{count}"),
            theme.diff_removed_text(),
            theme.diff_removed_bg(),
        )
    };

    egui::Frame::new()
        .fill(bg_color)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .color(text_color)
                    .font(typography::monospace(typography::SM))
                    .strong(),
            );
        });
}
