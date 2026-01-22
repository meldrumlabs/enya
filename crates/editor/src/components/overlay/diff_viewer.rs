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

use egui::{Color32, Key, RichText};
use similar::{ChangeTag, TextDiff};

use crate::components::OverlayColors;
use crate::components::util::finder_utils::{OverlayStyle, draw_backdrop};
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Result of showing the diff viewer overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffViewerResult {
    /// No action taken.
    None,
    /// Overlay was closed.
    Closed,
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
        }
    }

    /// Disable keyboard handling (call when another overlay is on top).
    pub fn set_keyboard_disabled(&mut self, disabled: bool) {
        self.keyboard_disabled = disabled;
    }

    /// Sets the UI theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
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
        self.is_open = true;

        log::debug!(
            "DiffViewerOverlay::open() - {} files in diff",
            self.file_diffs.len()
        );
    }

    /// Closes the overlay.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Show the overlay. Returns the result of the interaction.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> DiffViewerResult {
        if !self.is_open {
            return DiffViewerResult::None;
        }

        let mut should_close = false;

        // Handle keyboard input (unless another overlay is on top)
        if !self.keyboard_disabled {
            ctx.input(|i| {
                // Escape to close
                if i.key_pressed(Key::Escape) {
                    should_close = true;
                }

                // File navigation: n/p
                if !self.file_diffs.is_empty() {
                    // N - next file
                    if i.key_pressed(Key::N) && !i.modifiers.shift {
                        self.current_file_index =
                            (self.current_file_index + 1) % self.file_diffs.len();
                        self.scroll_offset_x = 0.0;
                        self.scroll_offset_y = 0.0;
                    }
                    // P or Shift+N - previous file
                    if i.key_pressed(Key::P) || (i.key_pressed(Key::N) && i.modifiers.shift) {
                        self.current_file_index = if self.current_file_index == 0 {
                            self.file_diffs.len() - 1
                        } else {
                            self.current_file_index - 1
                        };
                        self.scroll_offset_x = 0.0;
                        self.scroll_offset_y = 0.0;
                    }
                }

                // S - toggle split/unified view
                if i.key_pressed(Key::S) {
                    self.split_view = !self.split_view;
                }

                // Vim-style scrolling
                let scroll_step = 40.0;
                let h_scroll_step = 50.0;
                if i.key_pressed(Key::J) {
                    self.scroll_offset_y += scroll_step;
                }
                if i.key_pressed(Key::K) {
                    self.scroll_offset_y = (self.scroll_offset_y - scroll_step).max(0.0);
                }
                if i.key_pressed(Key::H) {
                    self.scroll_offset_x = (self.scroll_offset_x - h_scroll_step).max(0.0);
                }
                if i.key_pressed(Key::L) {
                    self.scroll_offset_x += h_scroll_step;
                }
            });
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
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.85).clamp(700.0, 1400.0);
        let popup_max_height = (screen_rect.height() * 0.85).clamp(500.0, 900.0);

        // File panel width
        let file_panel_width = 240.0;

        egui::Area::new(egui::Id::new("diff_viewer_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
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

                    // Main content: horizontal split with diff on left, file panel on right
                    // Calculate content height (leave room for footer)
                    let content_height = (ui.available_height() - 50.0).max(100.0);

                    ui.horizontal(|ui| {
                        // Left side: Diff content (takes remaining width)
                        let diff_width = popup_width - file_panel_width - 24.0; // account for margins
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

        DiffViewerResult::None
    }

    /// Renders the header with commit info (no file tabs - those are now in the side panel).
    fn render_header(&self, ui: &mut egui::Ui, colors: &OverlayColors, separator_color: Color32) {
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

            // Right side: total stats
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
    fn render_diff_content(&self, ui: &mut egui::Ui, colors: &OverlayColors) {
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

        // Calculate max line number width for alignment
        let max_line_num = file_diff
            .lines
            .iter()
            .filter_map(|l| l.old_line_num.max(l.new_line_num))
            .max()
            .unwrap_or(1);
        let line_num_width = max_line_num.to_string().len().max(3);

        // Scrollable diff content
        egui::ScrollArea::both()
            .id_salt("diff_viewer_scroll")
            .scroll_offset(egui::vec2(self.scroll_offset_x, self.scroll_offset_y))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);

                // Use a vertical layout with no spacing for tight line rendering
                ui.style_mut().spacing.item_spacing.y = 0.0;

                for line in &file_diff.lines {
                    self.render_diff_line(ui, line, line_num_width);
                }

                ui.add_space(8.0);
            });
    }

    /// Renders a single diff line with gutter, line numbers, and word highlighting.
    fn render_diff_line(&self, ui: &mut egui::Ui, line: &DiffLine, line_num_width: usize) {
        let theme = self.theme;
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
            DiffLineKind::HunkHeader => (theme.diff_hunk_text(), Some(theme.diff_hunk_bg()), None),
            DiffLineKind::FileHeader => (
                theme.diff_file_header(),
                Some(theme.diff_file_header_bg()),
                None,
            ),
            DiffLineKind::Context => (theme.diff_context_text(), None, None),
        };

        // Get the full available width for the background
        let available_width = ui.available_width();

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

            // Line numbers area with darker background
            let line_num_area_width = (line_num_width * 2 + 3) as f32 * 8.0; // Approximate char width
            let (line_num_rect, _) = ui.allocate_exact_size(
                egui::vec2(line_num_area_width, typography::MD + 4.0),
                egui::Sense::hover(),
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

            ui.add_space(8.0);

            // Content area
            let content = if line.content.is_empty() {
                " "
            } else {
                &line.content
            };

            // Render content with optional word-level highlights
            render_highlighted_text(
                ui,
                content,
                &line.word_highlights,
                base_text_color,
                line.kind,
                theme,
            );
        });

        // Draw full-width background behind the line
        if let Some(bg) = bg_color {
            let rect = egui::Rect::from_min_size(
                response.response.rect.min,
                egui::vec2(available_width, response.response.rect.height()),
            );
            // Draw background behind everything (lower z-order)
            let bg_painter = ui.painter().clone().with_layer_id(egui::LayerId::new(
                egui::Order::Background,
                egui::Id::new("diff_line_bg"),
            ));
            bg_painter.rect_filled(rect, 0.0, bg);
        }
    }

    /// Renders diff content in side-by-side split view.
    ///
    /// Shows old version on the left, new version on the right, with aligned lines.
    fn render_split_diff_content(
        &self,
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

        // Each side gets half the width minus some padding
        let side_width = (available_width - 8.0) / 2.0;

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
                            self.render_split_header_line(ui, line, available_width);
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
                                    self.render_split_line(
                                        ui,
                                        left.as_ref(),
                                        line_num_width,
                                        true,
                                        side_width,
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
                                    self.render_split_line(
                                        ui,
                                        right.as_ref(),
                                        line_num_width,
                                        false,
                                        side_width,
                                    );
                                },
                            );
                        });
                    }
                }

                ui.add_space(8.0);
            });
    }

    /// Renders a header line (file header or hunk header) spanning full width in split view.
    fn render_split_header_line(&self, ui: &mut egui::Ui, line: &DiffLine, available_width: f32) {
        let theme = self.theme;
        let (text_color, bg_color) = match line.kind {
            DiffLineKind::HunkHeader => (theme.diff_hunk_text(), theme.diff_hunk_bg()),
            DiffLineKind::FileHeader => (theme.diff_file_header(), theme.diff_file_header_bg()),
            _ => return, // Should not happen
        };

        let line_height = typography::SM + 6.0;

        // Allocate space first, then draw background, then text
        let (line_rect, _) = ui.allocate_exact_size(
            egui::vec2(available_width, line_height),
            egui::Sense::hover(),
        );

        // Draw background
        ui.painter().rect_filled(line_rect, 0.0, bg_color);

        // Draw text on top
        ui.painter().text(
            line_rect.left_center() + egui::vec2(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &line.content,
            typography::monospace(typography::SM),
            text_color,
        );
    }

    /// Renders a single line in the split view.
    ///
    /// `is_left` indicates whether this is the left (old) or right (new) side.
    /// `side_width` is the fixed width for this side panel.
    fn render_split_line(
        &self,
        ui: &mut egui::Ui,
        line: Option<&DiffLine>,
        line_num_width: usize,
        is_left: bool,
        side_width: f32,
    ) {
        let theme = self.theme;

        // Constrain the layout to prevent content from expanding
        ui.set_max_width(side_width);

        let line_height = typography::SM + 6.0;

        let Some(line) = line else {
            // Empty placeholder line - just allocate space with subtle background
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(side_width, line_height), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 0.0, theme.diff_line_number_bg().gamma_multiply(0.5));
            return;
        };

        // Determine colors based on line kind
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

        // Calculate widths for each element
        let gutter_width = 3.0;
        let line_num_area_width = (line_num_width + 1) as f32 * 8.0;
        let content_max_width = (side_width - gutter_width - line_num_area_width - 12.0).max(50.0);

        // Allocate the full line rect first, then paint background, then content
        let (line_rect, _) =
            ui.allocate_exact_size(egui::vec2(side_width, line_height), egui::Sense::hover());

        // Draw background first (if any)
        if let Some(bg) = bg_color {
            ui.painter().rect_filled(line_rect, 0.0, bg);
        }

        // Now draw all the content on top using painter directly
        let mut cursor_x = line_rect.left();

        // Gutter stripe (3px wide)
        if let Some(gc) = gutter_color {
            let gutter_rect = egui::Rect::from_min_size(
                egui::pos2(cursor_x, line_rect.top()),
                egui::vec2(gutter_width, line_height),
            );
            ui.painter().rect_filled(gutter_rect, 0.0, gc);
        }
        cursor_x += gutter_width + 2.0;

        // Line number background and text
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

        // Content - truncate to fit available width
        let content = if line.content.is_empty() {
            " ".to_string()
        } else {
            // Estimate max characters that fit (assuming ~7px per monospace char at SM size)
            let char_width = 7.0;
            let max_chars = (content_max_width / char_width) as usize;
            let char_count = line.content.chars().count();
            if char_count > max_chars && max_chars > 3 {
                // Use char_indices to safely truncate at character boundaries
                let truncate_at = max_chars.saturating_sub(1);
                let truncated: String = line.content.chars().take(truncate_at).collect();
                format!("{truncated}…")
            } else {
                line.content.clone()
            }
        };

        ui.painter().text(
            egui::pos2(cursor_x, line_rect.center().y),
            egui::Align2::LEFT_CENTER,
            content,
            typography::monospace(typography::SM),
            text_color,
        );
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

                    // Handle click
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
                let hint = if self.file_diffs.len() > 1 {
                    format!("s {view_mode} • n/p files • j/k scroll • Esc")
                } else {
                    format!("s {view_mode} • j/k scroll • Esc")
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

/// Renders text with optional word-level diff highlighting.
///
/// Word-level changes get a background highlight (brighter green/red).
fn render_highlighted_text(
    ui: &mut egui::Ui,
    content: &str,
    word_highlights: &[(usize, usize)],
    base_color: Color32,
    kind: DiffLineKind,
    theme: AppTheme,
) {
    // Get word-level highlight background color
    let word_bg = match kind {
        DiffLineKind::Addition => Some(theme.diff_added_word_bg()),
        DiffLineKind::Deletion => Some(theme.diff_removed_word_bg()),
        _ => None,
    };

    // If no highlights, render plain text
    if word_highlights.is_empty() {
        ui.label(
            RichText::new(content)
                .color(base_color)
                .font(typography::monospace(typography::MD)),
        );
        return;
    }

    // Build segments based on word highlights
    let mut segments: Vec<(&str, bool)> = Vec::new();
    let mut pos = 0;

    for &(start, end) in word_highlights {
        // Add unhighlighted text before this highlight
        if start > pos {
            if let Some(text) = content.get(pos..start) {
                segments.push((text, false));
            }
        }

        // Add highlighted text
        if let Some(text) = content.get(start..end) {
            segments.push((text, true));
        }

        pos = end;
    }

    // Add remaining unhighlighted text
    if pos < content.len() {
        if let Some(text) = content.get(pos..) {
            segments.push((text, false));
        }
    }

    // Render segments inline
    for (text, is_highlighted) in segments {
        if is_highlighted {
            if let Some(bg) = word_bg {
                // Draw with word highlight background
                egui::Frame::new()
                    .fill(bg)
                    .corner_radius(2.0)
                    .inner_margin(egui::Margin::symmetric(0, 0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(text)
                                .color(base_color)
                                .font(typography::monospace(typography::MD)),
                        );
                    });
            } else {
                ui.label(
                    RichText::new(text)
                        .color(base_color)
                        .font(typography::monospace(typography::MD)),
                );
            }
        } else {
            ui.label(
                RichText::new(text)
                    .color(base_color)
                    .font(typography::monospace(typography::MD)),
            );
        }
    }
}

/// Parses a unified diff into per-file sections with word-level highlighting.
fn parse_diff_into_files(diff: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileDiff> = None;

    // Track line numbers
    let mut old_line_num: usize = 0;
    let mut new_line_num: usize = 0;

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
                }],
                additions: 0,
                deletions: 0,
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
                if let Some((old_start, new_start)) = parse_hunk_header(raw_line) {
                    old_line_num = old_start;
                    new_line_num = new_start;
                }

                file.lines.push(DiffLine {
                    content: raw_line.to_string(),
                    kind: DiffLineKind::HunkHeader,
                    old_line_num: None,
                    new_line_num: None,
                    word_highlights: Vec::new(),
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

/// Builds paired lines for split (side-by-side) view.
///
/// Returns a vector of (left_line, right_line) pairs where:
/// - Context lines appear on both sides
/// - Deletions appear on the left only
/// - Additions appear on the right only
/// - Paired deletions/additions are aligned on the same row
/// - Headers span both sides
fn build_split_view_lines(lines: &[DiffLine]) -> Vec<(Option<DiffLine>, Option<DiffLine>)> {
    let mut result: Vec<(Option<DiffLine>, Option<DiffLine>)> = Vec::new();

    // Collect consecutive deletions and additions for pairing
    let mut pending_deletions: Vec<DiffLine> = Vec::new();
    let mut pending_additions: Vec<DiffLine> = Vec::new();

    for line in lines {
        match line.kind {
            DiffLineKind::Context => {
                // Flush any pending changes first
                flush_pending_changes(&mut result, &mut pending_deletions, &mut pending_additions);
                // Context lines appear on both sides
                result.push((Some(line.clone()), Some(line.clone())));
            }
            DiffLineKind::Deletion => {
                // Collect deletions to pair with additions
                pending_deletions.push(line.clone());
            }
            DiffLineKind::Addition => {
                // Collect additions to pair with deletions
                pending_additions.push(line.clone());
            }
            DiffLineKind::HunkHeader | DiffLineKind::FileHeader => {
                // Flush any pending changes first
                flush_pending_changes(&mut result, &mut pending_deletions, &mut pending_additions);
                // Headers appear on both sides
                result.push((Some(line.clone()), Some(line.clone())));
            }
        }
    }

    // Flush any remaining pending changes
    flush_pending_changes(&mut result, &mut pending_deletions, &mut pending_additions);

    result
}

/// Flushes pending deletions and additions into paired rows.
fn flush_pending_changes(
    result: &mut Vec<(Option<DiffLine>, Option<DiffLine>)>,
    deletions: &mut Vec<DiffLine>,
    additions: &mut Vec<DiffLine>,
) {
    // Pair up deletions with additions where possible
    let pairs = deletions.len().min(additions.len());

    for i in 0..pairs {
        result.push((Some(deletions[i].clone()), Some(additions[i].clone())));
    }

    // Add any remaining unpaired deletions (left side only)
    for deletion in deletions.iter().skip(pairs) {
        result.push((Some(deletion.clone()), None));
    }

    // Add any remaining unpaired additions (right side only)
    for addition in additions.iter().skip(pairs) {
        result.push((None, Some(addition.clone())));
    }

    deletions.clear();
    additions.clear();
}
