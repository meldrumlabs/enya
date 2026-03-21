//! Beautiful diff viewer overlay with GitHub-style styling.
//!
//! Features:
//! - **Side panel file list** - Version control style file tree on the right
//! - **Word-level diff highlighting** - Shows exactly which characters changed
//! - **Split view toggle** - Switch between unified and side-by-side diff views
//! - **Dual line numbers** - Old and new line numbers in the gutter
//! - **Colored gutter stripes** - Green/red bars for add/remove
//! - **GitHub dark color palette** - Professional, high-contrast styling
//! - **Commit info header** - Shows hash, message, and file stats
//!
//! # Keyboard Shortcuts
//!
//! - `s` - Toggle split/unified view
//! - `n` / `p` - Next/previous changed file
//! - `j` / `k` - Scroll down/up
//! - `h` / `l` - Scroll left/right
//! - `Escape` - Close overlay

use std::path::PathBuf;

use egui::text::LayoutJob;
use egui::{Color32, Key, RichText, TextFormat};
use similar::{ChangeTag, TextDiff};

use crate::components::OverlayColors;
use crate::components::util::file_opener::{FileOpenerAction, FileOpenerPopup, FileOpenerResult};
use crate::components::util::finder_utils::{OverlayStyle, draw_backdrop};
use crate::components::util::syntax_highlight::SyntaxHighlightData;
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

/// A single file's diff content.
#[derive(Debug, Clone, Default)]
pub struct FileDiff {
    /// The file path (relative to repo root).
    pub path: String,
    /// Lines of the diff for this file (including +/- prefixes).
    pub lines: Vec<DiffLine>,
    /// Number of additions.
    pub additions: usize,
    /// Number of deletions.
    pub deletions: usize,
    /// Syntax highlight data for the old version of the file.
    pub old_highlight: Option<SyntaxHighlightData>,
    /// Syntax highlight data for the new version of the file.
    pub new_highlight: Option<SyntaxHighlightData>,
}

/// A single line in a diff with word-level change information.
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// The line content (without the +/- prefix).
    pub content: String,
    /// The line type.
    pub kind: DiffLineKind,
    /// Old line number (for context and deletions).
    pub old_line_num: Option<usize>,
    /// New line number (for context and additions).
    pub new_line_num: Option<usize>,
    /// Word-level changes within this line (start, end byte indices of highlighted portions).
    /// These are the portions that differ from the corresponding line in the other version.
    pub word_highlights: Vec<(usize, usize)>,
    /// For HunkHeader lines: number of hidden lines between previous hunk and this one.
    pub hidden_lines: Option<usize>,
    /// For HunkHeader lines: the function/method context text (after the closing @@).
    pub hunk_context: Option<String>,
    /// Line number in the reconstructed old file (1-indexed, for syntax highlighting lookup).
    pub old_recon_num: Option<usize>,
    /// Line number in the reconstructed new file (1-indexed, for syntax highlighting lookup).
    pub new_recon_num: Option<usize>,
}

/// The type of diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    /// Context line (unchanged).
    Context,
    /// Added line.
    Addition,
    /// Removed line.
    Deletion,
    /// Hunk header (@@ ... @@).
    HunkHeader,
    /// File header (diff --git, ---, +++).
    FileHeader,
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
    /// Horizontal scroll offset.
    scroll_offset_x: f32,
    /// Vertical scroll offset.
    scroll_offset_y: f32,
    /// Whether to show split (side-by-side) view instead of unified view.
    split_view: bool,
    /// Disable keyboard handling (when another overlay is on top).
    keyboard_disabled: bool,
    /// Repository root path for computing full file paths.
    repo_root: Option<PathBuf>,
    /// File opener popup for opening files in external apps.
    file_opener: FileOpenerPopup,
    /// Flag to open file opener on next render (triggered by 'o' key).
    pending_open_file_opener: bool,
    /// Pre-computed vertical pixel offsets of each hunk header for jump navigation.
    hunk_offsets: Vec<f32>,
    /// Index of the current hunk (for {/} navigation).
    current_hunk_index: usize,
    /// Selected line range (start_index, end_index) into the current file's lines vec.
    selected_lines: Option<(usize, usize)>,
    /// The line index where selection started (for shift+click extension).
    selection_anchor: Option<usize>,
    /// Whether the search bar is active.
    search_active: bool,
    /// Current search query text.
    search_query: String,
    /// Cached search matches: (file_index, line_index, byte_start, byte_end).
    search_matches: Vec<(usize, usize, usize, usize)>,
    /// Index of the currently focused match in `search_matches`.
    current_match_index: usize,
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
            scroll_offset_x: 0.0,
            scroll_offset_y: 0.0,
            split_view: false,
            keyboard_disabled: false,
            pending_open_file_opener: false,
            repo_root: None,
            file_opener: FileOpenerPopup::new(),
            hunk_offsets: Vec::new(),
            current_hunk_index: 0,
            selected_lines: None,
            selection_anchor: None,
            search_active: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_index: 0,
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
    pub fn open(&mut self, hash: &str, message: &str, _timestamp: i64, diff_content: &str) {
        self.commit_hash = hash.to_string();
        self.commit_message = message.to_string();
        self.file_diffs = parse_diff_into_files(diff_content);
        self.current_file_index = 0;
        self.scroll_offset_x = 0.0;
        self.scroll_offset_y = 0.0;
        self.hunk_offsets = Vec::new();
        self.current_hunk_index = 0;
        self.selected_lines = None;
        self.selection_anchor = None;
        self.search_active = false;
        self.search_query.clear();
        self.search_matches.clear();
        self.current_match_index = 0;
        self.is_open = true;

        // Compute syntax highlighting for each file (native only)
        for file in &mut self.file_diffs {
            let lang = language_from_path(&file.path).to_string();
            let (old_content, new_content) = reconstruct_file_contents(file);
            if !old_content.is_empty() {
                file.old_highlight = Some(SyntaxHighlightData::new(&old_content, &lang));
            }
            if !new_content.is_empty() {
                file.new_highlight = Some(SyntaxHighlightData::new(&new_content, &lang));
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
        self.search_active = false;
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
        // Use consume_key to prevent multiple processing
        if !self.keyboard_disabled && !self.file_opener.is_open() {
            ctx.input_mut(|i| {
                // Escape: close search first, then clear selection, then close overlay
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    if self.search_active {
                        self.search_active = false;
                        // Must clear focus so the now-hidden TextEdit doesn't
                        // block vim-style keyboard handling in the workspace.
                        clear_focus = true;
                    } else if self.selected_lines.is_some() {
                        self.selected_lines = None;
                        self.selection_anchor = None;
                    } else {
                        should_close = true;
                    }
                }

                // Cmd+F or / — open search (when search is not active)
                if !self.search_active
                    && (i.consume_key(egui::Modifiers::COMMAND, Key::F)
                        || i.consume_key(egui::Modifiers::NONE, Key::Slash))
                {
                    self.search_active = true;
                }

                // When search is active, Enter/Shift+Enter navigate matches
                if self.search_active && !self.search_matches.is_empty() {
                    if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                        self.current_match_index =
                            (self.current_match_index + 1) % self.search_matches.len();
                        self.scroll_to_current_match();
                    }
                    if i.consume_key(egui::Modifiers::SHIFT, Key::Enter) {
                        self.current_match_index = if self.current_match_index == 0 {
                            self.search_matches.len() - 1
                        } else {
                            self.current_match_index - 1
                        };
                        self.scroll_to_current_match();
                    }
                }

                // The rest of keyboard shortcuts only apply when search is NOT active
                // (otherwise typing in the search box would trigger them)
                if !self.search_active {
                    // Cmd+C - copy selected lines
                    if i.consume_key(egui::Modifiers::COMMAND, Key::C) {
                        if let Some((start, end)) = self.selected_lines {
                            if let Some(file_diff) = self.file_diffs.get(self.current_file_index) {
                                let min = start.min(end);
                                let max = start.max(end);
                                let text: String = file_diff
                                    .lines
                                    .get(min..=max)
                                    .unwrap_or_default()
                                    .iter()
                                    .filter(|l| {
                                        !matches!(
                                            l.kind,
                                            DiffLineKind::HunkHeader | DiffLineKind::FileHeader
                                        )
                                    })
                                    .map(|l| l.content.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                ctx.copy_text(text);
                            }
                        }
                    }

                    // O - open file opener popup
                    if i.consume_key(egui::Modifiers::NONE, Key::O) {
                        self.pending_open_file_opener = true;
                    }

                    // File navigation: n/p
                    if !self.file_diffs.is_empty() {
                        if i.consume_key(egui::Modifiers::NONE, Key::N) {
                            self.current_file_index =
                                (self.current_file_index + 1) % self.file_diffs.len();
                            self.scroll_offset_x = 0.0;
                            self.scroll_offset_y = 0.0;
                            self.hunk_offsets.clear();
                            self.current_hunk_index = 0;
                            self.selected_lines = None;
                            self.selection_anchor = None;
                        }
                        if i.consume_key(egui::Modifiers::NONE, Key::P)
                            || i.consume_key(egui::Modifiers::SHIFT, Key::N)
                        {
                            self.current_file_index = if self.current_file_index == 0 {
                                self.file_diffs.len() - 1
                            } else {
                                self.current_file_index - 1
                            };
                            self.scroll_offset_x = 0.0;
                            self.scroll_offset_y = 0.0;
                            self.hunk_offsets.clear();
                            self.current_hunk_index = 0;
                            self.selected_lines = None;
                            self.selection_anchor = None;
                        }
                    }

                    // S - toggle split/unified view
                    if i.consume_key(egui::Modifiers::NONE, Key::S) {
                        self.split_view = !self.split_view;
                    }

                    // { / } - jump between hunks
                    if i.consume_key(egui::Modifiers::SHIFT, Key::OpenBracket)
                        && self.current_hunk_index > 0
                    {
                        self.current_hunk_index -= 1;
                        self.scroll_offset_y = self
                            .hunk_offsets
                            .get(self.current_hunk_index)
                            .copied()
                            .unwrap_or(0.0);
                    }
                    if i.consume_key(egui::Modifiers::SHIFT, Key::CloseBracket)
                        && self.current_hunk_index + 1 < self.hunk_offsets.len()
                    {
                        self.current_hunk_index += 1;
                        self.scroll_offset_y = self
                            .hunk_offsets
                            .get(self.current_hunk_index)
                            .copied()
                            .unwrap_or(0.0);
                    }

                    // Vim-style scrolling
                    let scroll_step = 40.0;
                    let h_scroll_step = 50.0;
                    if i.consume_key(egui::Modifiers::NONE, Key::J) {
                        self.scroll_offset_y += scroll_step;
                    }
                    if i.consume_key(egui::Modifiers::NONE, Key::K) {
                        self.scroll_offset_y = (self.scroll_offset_y - scroll_step).max(0.0);
                    }
                    if i.consume_key(egui::Modifiers::NONE, Key::H) {
                        self.scroll_offset_x = (self.scroll_offset_x - h_scroll_step).max(0.0);
                    }
                    if i.consume_key(egui::Modifiers::NONE, Key::L) {
                        self.scroll_offset_x += h_scroll_step;
                    }
                } // end if !self.search_active
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
                    if self.search_active {
                        self.render_search_bar(ui, &colors, separator_color);
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
                                if self.split_view {
                                    self.render_split_diff_content(ui, &colors, diff_width);
                                } else {
                                    self.render_diff_content(ui, &colors);
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

    /// Recomputes search matches across all files for the current query.
    fn recompute_search_matches(&mut self) {
        self.search_matches.clear();
        self.current_match_index = 0;
        let query = self.search_query.to_lowercase();
        if query.is_empty() {
            return;
        }
        for (file_idx, file) in self.file_diffs.iter().enumerate() {
            for (line_idx, line) in file.lines.iter().enumerate() {
                if matches!(
                    line.kind,
                    DiffLineKind::HunkHeader | DiffLineKind::FileHeader
                ) {
                    continue;
                }
                let lower = line.content.to_lowercase();
                let mut start = 0;
                while let Some(pos) = lower[start..].find(&query) {
                    let byte_start = start + pos;
                    let byte_end = byte_start + query.len();
                    self.search_matches
                        .push((file_idx, line_idx, byte_start, byte_end));
                    start = byte_end;
                }
            }
        }
    }

    /// Scrolls the view to the current search match, switching files if needed.
    fn scroll_to_current_match(&mut self) {
        let Some(&(file_idx, line_idx, _, _)) = self.search_matches.get(self.current_match_index)
        else {
            return;
        };

        // Switch file if match is in a different file
        if file_idx != self.current_file_index {
            self.current_file_index = file_idx;
            self.scroll_offset_x = 0.0;
            self.hunk_offsets.clear();
            self.current_hunk_index = 0;
            self.selected_lines = None;
            self.selection_anchor = None;
        }

        // Estimate scroll position from line index
        let line_height = typography::MD + 4.0;
        let hunk_header_height = typography::SM + 12.0;
        if let Some(file_diff) = self.file_diffs.get(file_idx) {
            let mut y = 4.0;
            for (i, line) in file_diff.lines.iter().enumerate() {
                if i == line_idx {
                    // Center the match in the viewport (rough estimate)
                    self.scroll_offset_y = (y - 100.0_f32).max(0.0);
                    break;
                }
                y += if line.kind == DiffLineKind::HunkHeader {
                    hunk_header_height
                } else {
                    line_height
                };
            }
        }
    }

    /// Renders the search bar.
    fn render_search_bar(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        separator_color: Color32,
    ) {
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Search icon
            ui.label(
                RichText::new(egui_nerdfonts::regular::MAGNIFY)
                    .color(colors.accent)
                    .size(14.0),
            );
            ui.add_space(4.0);

            // Text input
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .desired_width(250.0)
                    .font(typography::monospace(typography::SM))
                    .hint_text("Search in diff...")
                    .text_color(colors.text),
            );

            // Keep the text input focused while search is active
            response.request_focus();

            // Recompute matches when query changes
            if response.changed() {
                self.recompute_search_matches();
                // Jump to first match in current file, or first match overall
                if !self.search_matches.is_empty() {
                    // Try to find first match in current file
                    let first_in_file = self
                        .search_matches
                        .iter()
                        .position(|m| m.0 == self.current_file_index);
                    self.current_match_index = first_in_file.unwrap_or(0);
                    self.scroll_to_current_match();
                }
            }

            ui.add_space(8.0);

            // Match count indicator
            if !self.search_query.is_empty() {
                let match_text = if self.search_matches.is_empty() {
                    "No matches".to_string()
                } else {
                    format!(
                        "{}/{}",
                        self.current_match_index + 1,
                        self.search_matches.len()
                    )
                };
                ui.label(
                    RichText::new(match_text)
                        .color(if self.search_matches.is_empty() {
                            self.theme.diff_removed_text()
                        } else {
                            colors.muted_text
                        })
                        .font(typography::proportional(typography::SM)),
                );
            }

            // Hint
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("Enter next • Shift+Enter prev • Esc close")
                        .color(colors.muted_text.gamma_multiply(0.6))
                        .font(typography::proportional(typography::XS)),
                );
            });
        });

        // Separator below search bar
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator_color),
        );
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

    /// Renders the diff content with line numbers, gutter, and word highlighting.
    fn render_diff_content(&mut self, ui: &mut egui::Ui, colors: &OverlayColors) {
        if self.file_diffs.is_empty() {
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("No changes in this commit")
                        .color(colors.faint_text)
                        .font(typography::proportional(typography::MD)),
                );
            });
            ui.add_space(24.0);
            return;
        }

        // Destructure self to allow independent field borrows (avoids cloning)
        let file_diffs = &self.file_diffs;
        let hunk_offsets = &mut self.hunk_offsets;
        let scroll_offset_x = self.scroll_offset_x;
        let scroll_offset_y = self.scroll_offset_y;
        let theme = self.theme;
        let selected_lines = self.selected_lines;
        let accent = colors.accent;
        let search_matches = &self.search_matches;
        let search_query = &self.search_query;
        let current_match_index = self.current_match_index;
        let current_file_index = self.current_file_index;

        let Some(file_diff) = file_diffs.get(self.current_file_index) else {
            return;
        };

        // Calculate max line number width for alignment
        let max_line_num = file_diff
            .lines
            .iter()
            .filter_map(|l| l.old_line_num.max(l.new_line_num))
            .max()
            .unwrap_or(1);
        let line_num_width = max_line_num.to_string().len().max(3);

        // Pre-compute hunk offsets for {/} navigation
        let line_height = typography::MD + 4.0;
        let hunk_header_height = typography::SM + 12.0;
        if hunk_offsets.is_empty() {
            let mut y = 4.0; // initial add_space
            for line in &file_diff.lines {
                if line.kind == DiffLineKind::HunkHeader {
                    hunk_offsets.push(y);
                }
                y += if line.kind == DiffLineKind::HunkHeader {
                    hunk_header_height
                } else {
                    line_height
                };
            }
        }

        // Track clicks for line selection
        let mut clicked_line: Option<(usize, bool)> = None;

        // Scrollable diff content — no cloning needed thanks to field splitting
        egui::ScrollArea::both()
            .id_salt("diff_viewer_scroll")
            .scroll_offset(egui::vec2(scroll_offset_x, scroll_offset_y))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.style_mut().spacing.item_spacing.y = 0.0;

                for (line_idx, line) in file_diff.lines.iter().enumerate() {
                    // Compute search highlights for this line
                    let line_search_highlights: Vec<(usize, usize, bool)> =
                        if search_query.is_empty() {
                            Vec::new()
                        } else {
                            search_matches
                                .iter()
                                .enumerate()
                                .filter(|(_, m)| m.0 == current_file_index && m.1 == line_idx)
                                .map(|(i, m)| (m.2, m.3, i == current_match_index))
                                .collect()
                        };

                    let line_ctx = UnifiedLineCtx {
                        line,
                        line_idx,
                        line_num_width,
                        theme,
                        old_highlight: file_diff.old_highlight.as_ref(),
                        new_highlight: file_diff.new_highlight.as_ref(),
                        selected_lines,
                        accent,
                        search_highlights: line_search_highlights,
                    };
                    if let Some(shift) = render_diff_line_unified(ui, &line_ctx) {
                        clicked_line = Some((line_idx, shift));
                    }
                }

                ui.add_space(8.0);
            });

        // Process line selection clicks
        if let Some((line_idx, shift_held)) = clicked_line {
            if shift_held {
                if let Some(anchor) = self.selection_anchor {
                    self.selected_lines = Some((anchor, line_idx));
                }
            } else {
                self.selection_anchor = Some(line_idx);
                self.selected_lines = Some((line_idx, line_idx));
            }
        }
    }

    /// Renders diff content in side-by-side split view.
    ///
    /// Shows old version on the left, new version on the right, with aligned lines.
    fn render_split_diff_content(
        &mut self,
        ui: &mut egui::Ui,
        colors: &OverlayColors,
        available_width: f32,
    ) {
        if self.file_diffs.is_empty() {
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("No changes in this commit")
                        .color(colors.faint_text)
                        .font(typography::proportional(typography::MD)),
                );
            });
            ui.add_space(24.0);
            return;
        }

        let Some(file_diff) = self.file_diffs.get(self.current_file_index) else {
            return;
        };

        // Build paired lines for side-by-side view
        let paired_lines = build_split_view_lines(&file_diff.lines);

        // Calculate line number width
        let max_line_num = file_diff
            .lines
            .iter()
            .filter_map(|l| l.old_line_num.max(l.new_line_num))
            .max()
            .unwrap_or(1);
        let line_num_width = max_line_num.to_string().len().max(3);

        // Pre-compute hunk offsets for {/} navigation (split view)
        let split_line_height = typography::SM + 6.0;
        let hunk_header_height = typography::SM + 12.0;
        let hunk_offsets = &mut self.hunk_offsets;
        if hunk_offsets.is_empty() {
            let header_row_height = typography::SM + 4.0;
            let mut y = header_row_height + 4.0; // column headers + spacing
            for (left, _right) in &paired_lines {
                let is_header = left
                    .as_ref()
                    .map(|l| matches!(l.kind, DiffLineKind::HunkHeader | DiffLineKind::FileHeader))
                    .unwrap_or(false);
                if is_header
                    && left
                        .as_ref()
                        .is_some_and(|l| l.kind == DiffLineKind::HunkHeader)
                {
                    hunk_offsets.push(y);
                }
                y += if is_header {
                    hunk_header_height
                } else {
                    split_line_height
                };
            }
        }

        // Each side gets half the width minus some padding
        let side_width = ((available_width - 8.0) / 2.0).max(1.0);

        let theme = self.theme;

        // Column headers
        ui.horizontal(|ui| {
            // Left header
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, typography::SM + 4.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Old")
                            .color(theme.diff_removed_text().gamma_multiply(0.7))
                            .font(typography::proportional(typography::XS))
                            .strong(),
                    );
                },
            );

            // Separator
            ui.add_space(4.0);

            // Right header
            ui.allocate_ui_with_layout(
                egui::vec2(side_width, typography::SM + 4.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("New")
                            .color(theme.diff_added_text().gamma_multiply(0.7))
                            .font(typography::proportional(typography::XS))
                            .strong(),
                    );
                },
            );
        });

        // Separator line below headers
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, colors.separator),
        );

        // Scrollable diff content - only vertical scroll to prevent horizontal expansion
        egui::ScrollArea::vertical()
            .id_salt("diff_viewer_split_scroll")
            .scroll_offset(egui::vec2(0.0, self.scroll_offset_y))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Constrain the layout width to prevent overflow into Changed Files panel
                ui.set_max_width(available_width);
                ui.set_width(available_width);

                ui.add_space(4.0);
                ui.style_mut().spacing.item_spacing.y = 0.0;

                for (left, right) in &paired_lines {
                    // Check if this is a header line (spans full width)
                    let is_header = left
                        .as_ref()
                        .map(|l| {
                            matches!(l.kind, DiffLineKind::HunkHeader | DiffLineKind::FileHeader)
                        })
                        .unwrap_or(false);

                    if is_header {
                        // Render header spanning full width
                        if let Some(line) = left.as_ref() {
                            render_split_header_line_styled(ui, line, available_width, theme);
                        }
                    } else {
                        // Render side-by-side content with constrained width
                        ui.horizontal(|ui| {
                            // Constrain horizontal layout
                            ui.set_max_width(available_width);

                            // Left side (old/deleted)
                            ui.allocate_ui_with_layout(
                                egui::vec2(side_width, typography::MD + 4.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(side_width);
                                    render_split_line_with_syntax(
                                        ui,
                                        *left,
                                        line_num_width,
                                        true,
                                        side_width,
                                        theme,
                                        file_diff.old_highlight.as_ref(),
                                    );
                                },
                            );

                            // Center separator
                            let separator_rect = ui.available_rect_before_wrap();
                            ui.painter().vline(
                                separator_rect.left(),
                                separator_rect.y_range(),
                                egui::Stroke::new(1.0, colors.separator),
                            );
                            ui.add_space(4.0);

                            // Right side (new/added)
                            ui.allocate_ui_with_layout(
                                egui::vec2(side_width, typography::MD + 4.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_max_width(side_width);
                                    render_split_line_with_syntax(
                                        ui,
                                        *right,
                                        line_num_width,
                                        false,
                                        side_width,
                                        theme,
                                        file_diff.new_highlight.as_ref(),
                                    );
                                },
                            );
                        });
                    }
                }

                ui.add_space(8.0);
            });
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
                        self.scroll_offset_x = 0.0;
                        self.scroll_offset_y = 0.0;
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
                let view_mode = if self.split_view { "split" } else { "unified" };
                let copy_hint = if self.selected_lines.is_some() {
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

/// Context for rendering a unified diff line.
struct UnifiedLineCtx<'a> {
    line: &'a DiffLine,
    line_idx: usize,
    line_num_width: usize,
    theme: AppTheme,
    old_highlight: Option<&'a SyntaxHighlightData>,
    new_highlight: Option<&'a SyntaxHighlightData>,
    selected_lines: Option<(usize, usize)>,
    accent: Color32,
    /// Search highlight ranges: (byte_start, byte_end, is_current_match).
    search_highlights: Vec<(usize, usize, bool)>,
}

/// Renders a single diff line in unified view with syntax highlighting, hunk separators, and line selection.
///
/// Returns `Some(shift_held)` if the line number area was clicked (for line selection).
fn render_diff_line_unified(ui: &mut egui::Ui, ctx: &UnifiedLineCtx<'_>) -> Option<bool> {
    let UnifiedLineCtx {
        line,
        line_idx,
        line_num_width,
        theme,
        old_highlight,
        new_highlight,
        selected_lines,
        accent,
        search_highlights,
    } = ctx;
    let theme = *theme;
    let line_idx = *line_idx;
    let line_num_width = *line_num_width;
    let mut clicked_shift: Option<bool> = None;

    // Special rendering for hunk headers - styled separator
    if line.kind == DiffLineKind::HunkHeader {
        let available_width = ui.available_width();
        let header_height = typography::SM + 12.0;

        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, header_height),
            egui::Sense::hover(),
        );

        // Background
        ui.painter().rect_filled(rect, 0.0, theme.diff_hunk_bg());

        // Subtle top/bottom separator lines
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            egui::Stroke::new(1.0, theme.diff_hunk_text().gamma_multiply(0.2)),
        );
        ui.painter().hline(
            rect.x_range(),
            rect.bottom(),
            egui::Stroke::new(1.0, theme.diff_hunk_text().gamma_multiply(0.2)),
        );

        // Build display text: "··· N lines hidden ··· fn foo()"
        let hidden_text = line
            .hidden_lines
            .map(|n| format!("··· {n} lines hidden ···"))
            .unwrap_or_else(|| "···".to_string());
        let context_text = line.hunk_context.as_deref().unwrap_or("");

        let center_y = rect.center().y;

        // Draw hidden lines text (centered-ish)
        let hidden_galley = ui.painter().layout_no_wrap(
            hidden_text,
            typography::proportional(typography::XS),
            theme.diff_hunk_text().gamma_multiply(0.7),
        );
        let text_x = rect.left() + 16.0;
        ui.painter().galley(
            egui::pos2(text_x, center_y - hidden_galley.size().y / 2.0),
            hidden_galley.clone(),
            theme.diff_hunk_text().gamma_multiply(0.7),
        );

        // Draw function context in syntax function color
        if !context_text.is_empty() {
            let ctx_x = text_x + hidden_galley.size().x + 12.0;
            ui.painter().text(
                egui::pos2(ctx_x, center_y),
                egui::Align2::LEFT_CENTER,
                context_text,
                typography::monospace(typography::XS),
                theme.syntax_function().gamma_multiply(0.8),
            );
        }

        return None;
    }

    // File headers - keep existing style
    if line.kind == DiffLineKind::FileHeader {
        let available_width = ui.available_width();
        let response = ui.horizontal(|ui| {
            let gutter_width = 4.0;
            let (gutter_rect, _) = ui.allocate_exact_size(
                egui::vec2(gutter_width, typography::MD + 4.0),
                egui::Sense::hover(),
            );
            // No gutter for file headers
            let _ = gutter_rect;
            ui.add_space(4.0);
            ui.label(
                RichText::new(&line.content)
                    .color(theme.diff_file_header())
                    .font(typography::monospace(typography::MD)),
            );
        });
        // Background
        let rect = egui::Rect::from_min_size(
            response.response.rect.min,
            egui::vec2(available_width, response.response.rect.height()),
        );
        let bg_painter = ui.painter().clone().with_layer_id(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("diff_line_bg"),
        ));
        bg_painter.rect_filled(rect, 0.0, theme.diff_file_header_bg());
        return None;
    }

    // Regular content lines (Context, Addition, Deletion)
    let (base_text_color, bg_color, gutter_color) = match line.kind {
        DiffLineKind::Addition => (
            theme.diff_added_text(),
            Some(theme.diff_added_bg()),
            Some(theme.diff_added_gutter()),
        ),
        DiffLineKind::Deletion => (
            theme.diff_removed_text(),
            Some(theme.diff_removed_bg()),
            Some(theme.diff_removed_gutter()),
        ),
        DiffLineKind::Context => (theme.diff_context_text(), None, None),
        _ => unreachable!(),
    };

    let available_width = ui.available_width();

    // Check if this line is selected
    let is_selected = selected_lines.is_some_and(|(start, end)| {
        let min = start.min(end);
        let max = start.max(end);
        line_idx >= min && line_idx <= max
    });

    // Get syntax spans for this line
    let syntax_spans = get_syntax_spans_for_line(line, *old_highlight, *new_highlight, theme);

    // Build the layout job for content
    let content = if line.content.is_empty() {
        " "
    } else {
        &line.content
    };

    let word_bg = match line.kind {
        DiffLineKind::Addition => Some(theme.diff_added_word_bg()),
        DiffLineKind::Deletion => Some(theme.diff_removed_word_bg()),
        _ => None,
    };

    let layout_job = build_diff_line_layout_job(
        content,
        &line.word_highlights,
        base_text_color,
        word_bg,
        &syntax_spans,
        search_highlights,
    );

    // Create a horizontal layout for the line
    let response = ui.horizontal(|ui| {
        // Gutter stripe (4px wide colored bar on the left)
        let gutter_width = 4.0;
        let (gutter_rect, _) = ui.allocate_exact_size(
            egui::vec2(gutter_width, typography::MD + 4.0),
            egui::Sense::hover(),
        );
        if let Some(gc) = gutter_color {
            ui.painter().rect_filled(gutter_rect, 0.0, gc);
        }

        ui.add_space(4.0);

        // Line numbers area with darker background - CLICKABLE for selection
        let line_num_area_width = (line_num_width * 2 + 3) as f32 * 8.0;
        let (line_num_rect, line_num_response) = ui.allocate_exact_size(
            egui::vec2(line_num_area_width, typography::MD + 4.0),
            egui::Sense::click(),
        );

        // Draw line number background
        ui.painter()
            .rect_filled(line_num_rect, 0.0, theme.diff_line_number_bg());

        // Draw line numbers
        let old_num_str = line
            .old_line_num
            .map(|n| format!("{n:>line_num_width$}"))
            .unwrap_or_else(|| " ".repeat(line_num_width));
        let new_num_str = line
            .new_line_num
            .map(|n| format!("{n:>line_num_width$}"))
            .unwrap_or_else(|| " ".repeat(line_num_width));

        let line_nums_text = format!("{old_num_str} {new_num_str}");

        ui.painter().text(
            line_num_rect.left_center() + egui::vec2(4.0, 0.0),
            egui::Align2::LEFT_CENTER,
            line_nums_text,
            typography::monospace(typography::SM),
            theme.diff_line_number(),
        );

        // Check for click on line number area
        if line_num_response.clicked() {
            let shift = ui.input(|i| i.modifiers.shift);
            clicked_shift = Some(shift);
        }

        ui.add_space(8.0);

        // Content area - render with LayoutJob for syntax highlighting
        ui.label(layout_job);
    });

    // Draw full-width background behind the line
    let rect = egui::Rect::from_min_size(
        response.response.rect.min,
        egui::vec2(available_width, response.response.rect.height()),
    );
    let bg_painter = ui.painter().clone().with_layer_id(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("diff_line_bg"),
    ));
    if let Some(bg) = bg_color {
        bg_painter.rect_filled(rect, 0.0, bg);
    }

    // Draw selection overlay on top of diff background
    if is_selected {
        bg_painter.rect_filled(rect, 0.0, accent.gamma_multiply(0.12));
    }

    clicked_shift
}

/// Renders a header line (file header or hunk header) spanning full width in split view.
fn render_split_header_line_styled(
    ui: &mut egui::Ui,
    line: &DiffLine,
    available_width: f32,
    theme: AppTheme,
) {
    if line.kind == DiffLineKind::HunkHeader {
        let header_height = typography::SM + 12.0;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, header_height),
            egui::Sense::hover(),
        );

        // Background
        ui.painter().rect_filled(rect, 0.0, theme.diff_hunk_bg());

        // Top/bottom separator lines
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            egui::Stroke::new(1.0, theme.diff_hunk_text().gamma_multiply(0.2)),
        );
        ui.painter().hline(
            rect.x_range(),
            rect.bottom(),
            egui::Stroke::new(1.0, theme.diff_hunk_text().gamma_multiply(0.2)),
        );

        // Display text
        let hidden_text = line
            .hidden_lines
            .map(|n| format!("··· {n} lines hidden ···"))
            .unwrap_or_else(|| "···".to_string());
        let context_text = line.hunk_context.as_deref().unwrap_or("");

        let center_y = rect.center().y;
        let hidden_galley = ui.painter().layout_no_wrap(
            hidden_text,
            typography::proportional(typography::XS),
            theme.diff_hunk_text().gamma_multiply(0.7),
        );
        let text_x = rect.left() + 16.0;
        ui.painter().galley(
            egui::pos2(text_x, center_y - hidden_galley.size().y / 2.0),
            hidden_galley.clone(),
            theme.diff_hunk_text().gamma_multiply(0.7),
        );

        if !context_text.is_empty() {
            let ctx_x = text_x + hidden_galley.size().x + 12.0;
            ui.painter().text(
                egui::pos2(ctx_x, center_y),
                egui::Align2::LEFT_CENTER,
                context_text,
                typography::monospace(typography::XS),
                theme.syntax_function().gamma_multiply(0.8),
            );
        }
    } else {
        // File header - keep original style
        let line_height = typography::SM + 6.0;
        let (line_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, line_height),
            egui::Sense::hover(),
        );
        ui.painter()
            .rect_filled(line_rect, 0.0, theme.diff_file_header_bg());
        ui.painter().text(
            line_rect.left_center() + egui::vec2(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &line.content,
            typography::monospace(typography::SM),
            theme.diff_file_header(),
        );
    }
}

/// Renders a single line in the split view with syntax highlighting.
fn render_split_line_with_syntax(
    ui: &mut egui::Ui,
    line: Option<&DiffLine>,
    line_num_width: usize,
    is_left: bool,
    side_width: f32,
    theme: AppTheme,
    highlight: Option<&SyntaxHighlightData>,
) {
    ui.set_max_width(side_width);
    let line_height = typography::SM + 6.0;

    let Some(line) = line else {
        // Empty placeholder line
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(side_width, line_height), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, 0.0, theme.diff_line_number_bg().gamma_multiply(0.5));
        return;
    };

    let (text_color, bg_color, gutter_color) = match line.kind {
        DiffLineKind::Addition => (
            theme.diff_added_text(),
            Some(theme.diff_added_bg()),
            Some(theme.diff_added_gutter()),
        ),
        DiffLineKind::Deletion => (
            theme.diff_removed_text(),
            Some(theme.diff_removed_bg()),
            Some(theme.diff_removed_gutter()),
        ),
        DiffLineKind::HunkHeader => (theme.diff_hunk_text(), Some(theme.diff_hunk_bg()), None),
        DiffLineKind::FileHeader => (
            theme.diff_file_header(),
            Some(theme.diff_file_header_bg()),
            None,
        ),
        DiffLineKind::Context => (theme.diff_context_text(), None, None),
    };

    let gutter_width = 3.0;
    let line_num_area_width = (line_num_width + 1) as f32 * 8.0;
    let content_max_width = (side_width - gutter_width - line_num_area_width - 12.0).max(50.0);

    let (line_rect, _) =
        ui.allocate_exact_size(egui::vec2(side_width, line_height), egui::Sense::hover());

    if let Some(bg) = bg_color {
        ui.painter().rect_filled(line_rect, 0.0, bg);
    }

    let mut cursor_x = line_rect.left();

    // Gutter stripe
    if let Some(gc) = gutter_color {
        let gutter_rect = egui::Rect::from_min_size(
            egui::pos2(cursor_x, line_rect.top()),
            egui::vec2(gutter_width, line_height),
        );
        ui.painter().rect_filled(gutter_rect, 0.0, gc);
    }
    cursor_x += gutter_width + 2.0;

    // Line number
    let line_num_rect = egui::Rect::from_min_size(
        egui::pos2(cursor_x, line_rect.top()),
        egui::vec2(line_num_area_width, line_height),
    );
    ui.painter()
        .rect_filled(line_num_rect, 0.0, theme.diff_line_number_bg());

    let line_num = if is_left {
        line.old_line_num
    } else {
        line.new_line_num
    };
    let line_num_str = line_num
        .map(|n| format!("{n:>line_num_width$}"))
        .unwrap_or_else(|| " ".repeat(line_num_width));

    ui.painter().text(
        line_num_rect.left_center() + egui::vec2(2.0, 0.0),
        egui::Align2::LEFT_CENTER,
        line_num_str,
        typography::monospace(typography::SM),
        theme.diff_line_number(),
    );
    cursor_x += line_num_area_width + 4.0;

    // Content with syntax highlighting
    let content = if line.content.is_empty() {
        " ".to_string()
    } else {
        // Truncate to fit
        let char_width = 7.0;
        let max_chars = (content_max_width / char_width) as usize;
        let char_count = line.content.chars().count();
        if char_count > max_chars && max_chars > 3 {
            let truncate_at = max_chars.saturating_sub(1);
            let truncated: String = line.content.chars().take(truncate_at).collect();
            format!("{truncated}…")
        } else {
            line.content.clone()
        }
    };

    // Get syntax spans using reconstructed line numbers
    let syntax_spans = if let Some(hl) = highlight {
        let recon_num = if is_left {
            line.old_recon_num
        } else {
            line.new_recon_num
        };
        recon_num
            .map(|n| hl.get_line_spans(n, theme))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    if syntax_spans.is_empty() {
        // No syntax highlighting - use painter.text() as before
        ui.painter().text(
            egui::pos2(cursor_x, line_rect.center().y),
            egui::Align2::LEFT_CENTER,
            content,
            typography::monospace(typography::SM),
            text_color,
        );
    } else {
        // Build LayoutJob with syntax colors
        let word_bg = match line.kind {
            DiffLineKind::Addition => Some(theme.diff_added_word_bg()),
            DiffLineKind::Deletion => Some(theme.diff_removed_word_bg()),
            _ => None,
        };
        let job = build_diff_line_layout_job_sm(
            &content,
            &line.word_highlights,
            text_color,
            word_bg,
            &syntax_spans,
            &[], // search highlights not yet supported in split view
        );
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(
            egui::pos2(cursor_x, line_rect.center().y - galley.size().y / 2.0),
            galley,
            text_color,
        );
    }
}

/// Gets syntax color spans for a diff line, choosing the appropriate highlight data.
///
/// Uses `old_recon_num`/`new_recon_num` on the line to look up the correct position
/// in the reconstructed file's syntax data.
fn get_syntax_spans_for_line(
    line: &DiffLine,
    old_highlight: Option<&SyntaxHighlightData>,
    new_highlight: Option<&SyntaxHighlightData>,
    theme: AppTheme,
) -> Vec<(usize, usize, Color32)> {
    match line.kind {
        DiffLineKind::Deletion => {
            if let (Some(hl), Some(n)) = (old_highlight, line.old_recon_num) {
                hl.get_line_spans(n, theme)
            } else {
                Vec::new()
            }
        }
        DiffLineKind::Addition => {
            if let (Some(hl), Some(n)) = (new_highlight, line.new_recon_num) {
                hl.get_line_spans(n, theme)
            } else {
                Vec::new()
            }
        }
        DiffLineKind::Context => {
            // Prefer new highlight data for context lines
            if let (Some(hl), Some(n)) = (new_highlight, line.new_recon_num) {
                hl.get_line_spans(n, theme)
            } else if let (Some(hl), Some(n)) = (old_highlight, line.old_recon_num) {
                hl.get_line_spans(n, theme)
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

/// Builds a `LayoutJob` for a diff line that composites syntax colors with word-level highlights.
///
/// Uses `typography::MD` font size (for unified view).
fn build_diff_line_layout_job(
    content: &str,
    word_highlights: &[(usize, usize)],
    base_text_color: Color32,
    word_bg: Option<Color32>,
    syntax_spans: &[(usize, usize, Color32)],
    search_highlights: &[(usize, usize, bool)],
) -> LayoutJob {
    build_diff_line_layout_job_inner(
        content,
        word_highlights,
        base_text_color,
        word_bg,
        syntax_spans,
        search_highlights,
        typography::MD,
    )
}

/// Builds a `LayoutJob` for a diff line using `typography::SM` font size (for split view).
fn build_diff_line_layout_job_sm(
    content: &str,
    word_highlights: &[(usize, usize)],
    base_text_color: Color32,
    word_bg: Option<Color32>,
    syntax_spans: &[(usize, usize, Color32)],
    search_highlights: &[(usize, usize, bool)],
) -> LayoutJob {
    build_diff_line_layout_job_inner(
        content,
        word_highlights,
        base_text_color,
        word_bg,
        syntax_spans,
        search_highlights,
        typography::SM,
    )
}

/// Inner implementation for building a composite LayoutJob with syntax + word highlighting.
///
/// Uses a sweep-line approach: collects all span boundary points, sorts them,
/// then iterates through segments. O(s log s) where s = total spans, instead of O(n * s).
fn build_diff_line_layout_job_inner(
    content: &str,
    word_highlights: &[(usize, usize)],
    base_text_color: Color32,
    word_bg: Option<Color32>,
    syntax_spans: &[(usize, usize, Color32)],
    search_highlights: &[(usize, usize, bool)],
    font_size: f32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font_id = typography::monospace(font_size);

    if content.is_empty() {
        job.append(" ", 0.0, TextFormat::simple(font_id, base_text_color));
        return job;
    }

    let len = content.len();

    // Collect all boundary points where formatting changes
    let mut boundaries: Vec<usize> = Vec::with_capacity(
        2 + syntax_spans.len() * 2 + word_highlights.len() * 2 + search_highlights.len() * 2,
    );
    boundaries.push(0);
    boundaries.push(len);
    for &(start, end, _) in syntax_spans {
        if start < len {
            boundaries.push(start);
        }
        if end <= len {
            boundaries.push(end);
        }
    }
    for &(start, end) in word_highlights {
        if start < len {
            boundaries.push(start);
        }
        if end <= len {
            boundaries.push(end);
        }
    }
    for &(start, end, _) in search_highlights {
        if start < len {
            boundaries.push(start);
        }
        if end <= len {
            boundaries.push(end);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    // Snap boundaries to valid UTF-8 char boundaries
    for b in &mut boundaries {
        while *b < len && !content.is_char_boundary(*b) {
            *b += 1;
        }
    }
    boundaries.dedup();

    // Iterate through segments between boundaries
    for pair in boundaries.windows(2) {
        let seg_start = pair[0];
        let seg_end = pair[1];
        if seg_start >= seg_end || seg_start >= len {
            continue;
        }
        let seg_end = seg_end.min(len);

        let Some(text) = content.get(seg_start..seg_end) else {
            continue;
        };

        // Determine syntax color at this segment (first matching span)
        let text_color = syntax_spans
            .iter()
            .find(|&&(s, e, _)| seg_start >= s && seg_start < e)
            .map(|&(_, _, c)| c)
            .unwrap_or(base_text_color);

        // Determine if this segment is inside a word highlight
        let in_word_highlight = word_highlights
            .iter()
            .any(|&(s, e)| seg_start >= s && seg_start < e);

        // Determine if this segment is inside a search highlight
        let search_match = search_highlights
            .iter()
            .find(|&&(s, e, _)| seg_start >= s && seg_start < e);

        // Search highlights take priority over word highlights for background
        let bg = if let Some(&(_, _, is_current)) = search_match {
            if is_current {
                // Current match: bright orange background
                Some(Color32::from_rgba_premultiplied(230, 160, 0, 180))
            } else {
                // Other matches: dimmer yellow background
                Some(Color32::from_rgba_premultiplied(180, 140, 0, 100))
            }
        } else if in_word_highlight {
            word_bg
        } else {
            None
        };

        // For search highlights, use dark text for contrast
        let final_text_color = if search_match.is_some() {
            Color32::from_rgb(30, 30, 30)
        } else {
            text_color
        };

        let mut format = TextFormat::simple(font_id.clone(), final_text_color);
        if let Some(bg_color) = bg {
            format.background = bg_color;
        }
        job.append(text, 0.0, format);
    }

    if job.is_empty() {
        job.append(" ", 0.0, TextFormat::simple(font_id, base_text_color));
    }

    job
}

/// Parses a unified diff into per-file sections with word-level highlighting.
fn parse_diff_into_files(diff: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileDiff> = None;

    // Track line numbers
    let mut old_line_num: usize = 0;
    let mut new_line_num: usize = 0;

    // Track previous hunk end for computing hidden lines
    let mut prev_old_end: Option<usize> = None;

    // Collect consecutive add/delete pairs for word-level diff
    let mut pending_deletions: Vec<(String, usize)> = Vec::new(); // (content, index in lines)
    let mut pending_additions: Vec<(String, usize)> = Vec::new();

    for raw_line in diff.lines() {
        // New file: diff --git a/path b/path
        if raw_line.starts_with("diff --git") {
            // Process any pending add/delete pairs before saving the file
            if let Some(ref mut file) = current_file {
                compute_word_highlights(file, &pending_deletions, &pending_additions);
                pending_deletions.clear();
                pending_additions.clear();
            }

            // Save previous file
            if let Some(file) = current_file.take() {
                files.push(file);
            }

            // Reset hunk tracking for new file
            prev_old_end = None;

            // Extract path from "diff --git a/path b/path"
            let path = raw_line
                .strip_prefix("diff --git a/")
                .and_then(|s| s.split(" b/").next())
                .unwrap_or("")
                .to_string();

            current_file = Some(FileDiff {
                path,
                lines: vec![DiffLine {
                    content: raw_line.to_string(),
                    kind: DiffLineKind::FileHeader,
                    old_line_num: None,
                    new_line_num: None,
                    word_highlights: Vec::new(),
                    hidden_lines: None,
                    hunk_context: None,
                    old_recon_num: None,
                    new_recon_num: None,
                }],
                additions: 0,
                deletions: 0,
                old_highlight: None,
                new_highlight: None,
            });
            continue;
        }

        // If we have a current file, add lines to it
        if let Some(ref mut file) = current_file {
            // Parse hunk header to get line numbers
            if raw_line.starts_with("@@") {
                // Process any pending add/delete pairs
                compute_word_highlights(file, &pending_deletions, &pending_additions);
                pending_deletions.clear();
                pending_additions.clear();

                // Parse @@ -old_start,old_count +new_start,new_count @@
                let mut hidden_lines = None;
                let mut hunk_context = None;

                if let Some((old_start, new_start)) = parse_hunk_header(raw_line) {
                    // Compute hidden lines (gap between previous hunk end and this start)
                    if let Some(prev_end) = prev_old_end {
                        if old_start > prev_end {
                            hidden_lines = Some(old_start - prev_end);
                        }
                    } else if old_start > 1 {
                        // First hunk - lines before it are hidden
                        hidden_lines = Some(old_start.saturating_sub(1));
                    }

                    old_line_num = old_start;
                    new_line_num = new_start;
                }

                // Extract function context (text after second @@)
                if let Some(after_marker) = raw_line.splitn(3, "@@").nth(2) {
                    let ctx = after_marker.trim();
                    if !ctx.is_empty() {
                        hunk_context = Some(ctx.to_string());
                    }
                }

                // Parse old count to track where this hunk ends
                if let Some(old_count) = parse_hunk_old_count(raw_line) {
                    prev_old_end = Some(old_line_num + old_count);
                }

                file.lines.push(DiffLine {
                    content: raw_line.to_string(),
                    kind: DiffLineKind::HunkHeader,
                    old_line_num: None,
                    new_line_num: None,
                    word_highlights: Vec::new(),
                    hidden_lines,
                    hunk_context,
                    old_recon_num: None,
                    new_recon_num: None,
                });
                continue;
            }

            let kind = classify_diff_line(raw_line);

            // Strip the prefix for content
            let content = match kind {
                DiffLineKind::Addition | DiffLineKind::Deletion => {
                    raw_line.get(1..).unwrap_or("").to_string()
                }
                DiffLineKind::Context => raw_line.get(1..).unwrap_or(raw_line).to_string(),
                _ => raw_line.to_string(),
            };

            // Determine line numbers
            let (old_num, new_num) = match kind {
                DiffLineKind::Addition => {
                    let n = new_line_num;
                    new_line_num += 1;
                    file.additions += 1;
                    (None, Some(n))
                }
                DiffLineKind::Deletion => {
                    let n = old_line_num;
                    old_line_num += 1;
                    file.deletions += 1;
                    (Some(n), None)
                }
                DiffLineKind::Context => {
                    let old = old_line_num;
                    let new = new_line_num;
                    old_line_num += 1;
                    new_line_num += 1;
                    (Some(old), Some(new))
                }
                _ => (None, None),
            };

            let line_index = file.lines.len();
            file.lines.push(DiffLine {
                content: content.clone(),
                kind,
                old_line_num: old_num,
                new_line_num: new_num,
                word_highlights: Vec::new(),
                hidden_lines: None,
                hunk_context: None,
                old_recon_num: None,
                new_recon_num: None,
            });

            // Track consecutive additions and deletions for word-level diff
            match kind {
                DiffLineKind::Deletion => {
                    // If we had pending additions, process them first
                    if !pending_additions.is_empty() && pending_deletions.is_empty() {
                        pending_additions.clear();
                    }
                    pending_deletions.push((content, line_index));
                }
                DiffLineKind::Addition => {
                    pending_additions.push((content, line_index));
                }
                DiffLineKind::Context | DiffLineKind::HunkHeader | DiffLineKind::FileHeader => {
                    // Process any pending add/delete pairs when we hit context
                    compute_word_highlights(file, &pending_deletions, &pending_additions);
                    pending_deletions.clear();
                    pending_additions.clear();
                }
            }
        }
    }

    // Process final pending pairs and save last file
    if let Some(mut file) = current_file {
        compute_word_highlights(&mut file, &pending_deletions, &pending_additions);
        files.push(file);
    }

    files
}

/// Computes word-level highlights for paired addition/deletion lines.
///
/// Uses word-based diffing to highlight meaningful changes (identifiers, operators, etc.)
/// rather than individual characters. This produces much cleaner diffs for code.
fn compute_word_highlights(
    file: &mut FileDiff,
    deletions: &[(String, usize)],
    additions: &[(String, usize)],
) {
    // Match deletions with additions 1:1 for word-level diff
    let pairs = deletions.len().min(additions.len());

    for i in 0..pairs {
        let (del_content, del_idx) = &deletions[i];
        let (add_content, add_idx) = &additions[i];

        // Use word-level diffing for cleaner highlights on code
        // This groups changes by words/tokens rather than individual characters
        let diff = TextDiff::from_words(del_content, add_content);

        // Compute highlights for the deletion line
        let mut del_highlights: Vec<(usize, usize)> = Vec::new();
        let mut del_pos = 0;
        for change in diff.iter_all_changes() {
            let text = change.value();
            let len = text.len();
            match change.tag() {
                ChangeTag::Delete => {
                    del_highlights.push((del_pos, del_pos + len));
                    del_pos += len;
                }
                ChangeTag::Equal => {
                    del_pos += len;
                }
                ChangeTag::Insert => {
                    // Skip inserts for deletion line
                }
            }
        }

        // Compute highlights for the addition line
        let mut add_highlights: Vec<(usize, usize)> = Vec::new();
        let mut add_pos = 0;
        for change in diff.iter_all_changes() {
            let text = change.value();
            let len = text.len();
            match change.tag() {
                ChangeTag::Insert => {
                    add_highlights.push((add_pos, add_pos + len));
                    add_pos += len;
                }
                ChangeTag::Equal => {
                    add_pos += len;
                }
                ChangeTag::Delete => {
                    // Skip deletes for addition line
                }
            }
        }

        // Merge adjacent highlights to avoid fragmented highlighting
        let del_highlights = merge_adjacent_highlights(del_highlights);
        let add_highlights = merge_adjacent_highlights(add_highlights);

        // Apply highlights to the lines
        if let Some(line) = file.lines.get_mut(*del_idx) {
            line.word_highlights = del_highlights;
        }
        if let Some(line) = file.lines.get_mut(*add_idx) {
            line.word_highlights = add_highlights;
        }
    }
}

/// Merges adjacent or overlapping highlight ranges into contiguous spans.
/// This prevents fragmented highlighting like "f o o" becoming three separate highlights.
fn merge_adjacent_highlights(mut highlights: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if highlights.is_empty() {
        return highlights;
    }

    highlights.sort_by_key(|&(start, _)| start);

    let mut merged: Vec<(usize, usize)> = Vec::new();
    let mut current = highlights[0];

    for &(start, end) in &highlights[1..] {
        // If this highlight starts where the current one ends (or overlaps), merge them
        if start <= current.1 {
            current.1 = current.1.max(end);
        } else {
            merged.push(current);
            current = (start, end);
        }
    }
    merged.push(current);

    merged
}

/// Parses a hunk header to extract starting line numbers.
/// Format: @@ -old_start,old_count +new_start,new_count @@ optional_context
fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    // Strip @@ prefix and suffix
    let content = line.strip_prefix("@@")?.trim_start();
    let content = content.split("@@").next()?.trim();

    // Parse -old_start,old_count +new_start,new_count
    let mut parts = content.split_whitespace();

    // Parse old: -start,count or -start
    let old_part = parts.next()?.strip_prefix('-')?;
    let old_start: usize = old_part.split(',').next()?.parse().ok()?;

    // Parse new: +start,count or +start
    let new_part = parts.next()?.strip_prefix('+')?;
    let new_start: usize = new_part.split(',').next()?.parse().ok()?;

    Some((old_start, new_start))
}

/// Classifies a diff line by its type.
fn classify_diff_line(line: &str) -> DiffLineKind {
    if line.starts_with("@@") {
        DiffLineKind::HunkHeader
    } else if line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("new file mode")
        || line.starts_with("deleted file mode")
    {
        DiffLineKind::FileHeader
    } else if line.starts_with('+') {
        DiffLineKind::Addition
    } else if line.starts_with('-') {
        DiffLineKind::Deletion
    } else {
        DiffLineKind::Context
    }
}

/// Builds paired lines for split (side-by-side) view using references (zero-copy).
///
/// Returns a vector of (left_line, right_line) reference pairs where:
/// - Context lines appear on both sides
/// - Deletions appear on the left only
/// - Additions appear on the right only
/// - Paired deletions/additions are aligned on the same row
/// - Headers span both sides
fn build_split_view_lines(lines: &[DiffLine]) -> Vec<(Option<&DiffLine>, Option<&DiffLine>)> {
    let mut result: Vec<(Option<&DiffLine>, Option<&DiffLine>)> = Vec::new();

    // Collect consecutive deletions and additions for pairing
    let mut pending_deletions: Vec<&DiffLine> = Vec::new();
    let mut pending_additions: Vec<&DiffLine> = Vec::new();

    for line in lines {
        match line.kind {
            DiffLineKind::Context => {
                flush_pending_refs(&mut result, &mut pending_deletions, &mut pending_additions);
                result.push((Some(line), Some(line)));
            }
            DiffLineKind::Deletion => {
                pending_deletions.push(line);
            }
            DiffLineKind::Addition => {
                pending_additions.push(line);
            }
            DiffLineKind::HunkHeader | DiffLineKind::FileHeader => {
                flush_pending_refs(&mut result, &mut pending_deletions, &mut pending_additions);
                result.push((Some(line), Some(line)));
            }
        }
    }

    flush_pending_refs(&mut result, &mut pending_deletions, &mut pending_additions);
    result
}

/// Flushes pending deletions and additions into paired rows (reference version).
fn flush_pending_refs<'a>(
    result: &mut Vec<(Option<&'a DiffLine>, Option<&'a DiffLine>)>,
    deletions: &mut Vec<&'a DiffLine>,
    additions: &mut Vec<&'a DiffLine>,
) {
    let pairs = deletions.len().min(additions.len());

    for i in 0..pairs {
        result.push((Some(deletions[i]), Some(additions[i])));
    }

    for deletion in deletions.iter().skip(pairs) {
        result.push((Some(deletion), None));
    }

    for addition in additions.iter().skip(pairs) {
        result.push((None, Some(addition)));
    }

    deletions.clear();
    additions.clear();
}

/// Parses the old line count from a hunk header.
/// Format: @@ -old_start,old_count +new_start,new_count @@
fn parse_hunk_old_count(line: &str) -> Option<usize> {
    let content = line.strip_prefix("@@")?.trim_start();
    let content = content.split("@@").next()?.trim();
    let old_part = content.split_whitespace().next()?.strip_prefix('-')?;
    old_part.split(',').nth(1)?.parse().ok()
}

/// Reconstructs the old and new file contents from diff lines.
///
/// The old file is built from Context + Deletion lines.
/// The new file is built from Context + Addition lines.
///
/// Also sets `old_recon_num` / `new_recon_num` on each `DiffLine` so syntax
/// highlight lookups can use the correct line number in the reconstructed content.
fn reconstruct_file_contents(file: &mut FileDiff) -> (String, String) {
    let mut old_content = String::new();
    let mut new_content = String::new();
    let mut old_line_num: usize = 0;
    let mut new_line_num: usize = 0;

    for line in &mut file.lines {
        match line.kind {
            DiffLineKind::Context => {
                old_content.push_str(&line.content);
                old_content.push('\n');
                old_line_num += 1;
                new_content.push_str(&line.content);
                new_content.push('\n');
                new_line_num += 1;
                line.old_recon_num = Some(old_line_num);
                line.new_recon_num = Some(new_line_num);
            }
            DiffLineKind::Deletion => {
                old_content.push_str(&line.content);
                old_content.push('\n');
                old_line_num += 1;
                line.old_recon_num = Some(old_line_num);
            }
            DiffLineKind::Addition => {
                new_content.push_str(&line.content);
                new_content.push('\n');
                new_line_num += 1;
                line.new_recon_num = Some(new_line_num);
            }
            _ => {}
        }
    }

    (old_content, new_content)
}

/// Maps a file path to a language identifier for tree-sitter.
fn language_from_path(path: &str) -> &str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "rs" => "rust",
        "go" => "go",
        "py" => "python",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        other => other,
    }
}
