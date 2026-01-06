//! Diff viewer overlay for viewing commit changes.
//!
//! Displays a full commit diff with syntax highlighting and diff coloring,
//! with navigation between changed files using n/p keys.
//!
//! # Keyboard Shortcuts
//!
//! - `n` / `N` - Next/previous changed file
//! - `p` / `P` - Previous/next changed file
//! - `j` / `k` - Scroll down/up
//! - `h` / `l` - Scroll left/right
//! - `Escape` - Close overlay

use egui::{Color32, Key, RichText};

use crate::components::OverlayColors;
use crate::components::util::finder_utils::{OverlayStyle, draw_backdrop};
use crate::ui::palette;
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

/// A single line in a diff.
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// The line content.
    pub content: String,
    /// The line type.
    pub kind: DiffLineKind,
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
        }
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

        // Handle keyboard input
        ctx.input(|i| {
            // Escape to close
            if i.key_pressed(Key::Escape) {
                should_close = true;
            }

            // File navigation: n/p
            if !self.file_diffs.is_empty() {
                // N - next file
                if i.key_pressed(Key::N) && !i.modifiers.shift {
                    self.current_file_index = (self.current_file_index + 1) % self.file_diffs.len();
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

        if should_close {
            self.close();
            return DiffViewerResult::Closed;
        }

        // Draw backdrop
        draw_backdrop(ctx, self.theme, "diff_viewer");

        // Calculate popup dimensions (matching source_preview.rs pattern)
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.7).clamp(500.0, 900.0);
        let popup_max_height = (screen_rect.height() * 0.7).clamp(300.0, 600.0);

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

                    // Header section (like source_preview)
                    self.render_header(ui, &colors, separator_color);

                    // Diff content - main area
                    self.render_diff_content(ui, &colors);

                    // Footer
                    self.render_footer(ui, muted_text, separator_color);
                });
            });

        DiffViewerResult::None
    }

    /// Renders the header with file path and commit info (like source_preview).
    fn render_header(&self, ui: &mut egui::Ui, colors: &OverlayColors, separator_color: Color32) {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Git commit icon
            ui.label(
                RichText::new(egui_nerdfonts::regular::GIT_COMMIT)
                    .color(colors.accent)
                    .size(18.0),
            );
            ui.add_space(8.0);

            // File path with stats (or commit info if no file selected)
            if let Some(file_diff) = self.file_diffs.get(self.current_file_index) {
                // File path
                ui.label(
                    RichText::new(&file_diff.path)
                        .color(colors.accent)
                        .font(typography::monospace(typography::LG))
                        .strong(),
                );

                ui.add_space(12.0);

                // Stats badge
                if file_diff.additions > 0 || file_diff.deletions > 0 {
                    let stats = format!("+{} -{}", file_diff.additions, file_diff.deletions);
                    let badge_color = palette::semantic::SUCCESS;
                    let bg_color = badge_color.gamma_multiply(0.2);

                    egui::Frame::new()
                        .fill(bg_color)
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(8, 2))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(stats)
                                    .color(badge_color)
                                    .font(typography::monospace(typography::SM)),
                            );
                        });
                }
            } else {
                // Show commit hash if no files
                let short_hash = &self.commit_hash[..7.min(self.commit_hash.len())];
                ui.label(
                    RichText::new(short_hash)
                        .color(colors.accent)
                        .font(typography::monospace(typography::LG))
                        .strong(),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // File counter badge
                if self.file_diffs.len() > 1 {
                    ui.label(
                        RichText::new(format!(
                            "[{}/{}]",
                            self.current_file_index + 1,
                            self.file_diffs.len()
                        ))
                        .color(colors.muted_text)
                        .font(typography::proportional(typography::MD)),
                    );
                }
            });
        });
        ui.add_space(12.0);

        // Separator below header
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator_color),
        );
    }

    /// Renders the diff content (main area like source_preview's render_source_code).
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

        // Scrollable diff content - both horizontal and vertical
        egui::ScrollArea::both()
            .id_salt("diff_viewer_scroll")
            .scroll_offset(egui::vec2(self.scroll_offset_x, self.scroll_offset_y))
            .auto_shrink([false, false]) // Don't shrink - fill available space
            .show(ui, |ui| {
                ui.add_space(8.0);
                for line in &file_diff.lines {
                    self.render_diff_line(ui, line, colors);
                }
                ui.add_space(8.0);
            });
    }

    /// Renders a single diff line with appropriate styling.
    fn render_diff_line(&self, ui: &mut egui::Ui, line: &DiffLine, colors: &OverlayColors) {
        let (text_color, bg_color) = match line.kind {
            DiffLineKind::Addition => (palette::diff::ADDED_TEXT, Some(palette::diff::ADDED_BG)),
            DiffLineKind::Deletion => {
                (palette::diff::REMOVED_TEXT, Some(palette::diff::REMOVED_BG))
            }
            DiffLineKind::HunkHeader => (palette::diff::HUNK_TEXT, Some(palette::diff::HUNK_BG)),
            DiffLineKind::FileHeader => (palette::diff::FILE_HEADER, None),
            DiffLineKind::Context => (colors.text, None),
        };

        // Ensure the line content has at least some content to display properly
        let content = if line.content.is_empty() {
            " " // Empty lines need a space to maintain height
        } else {
            &line.content
        };

        // For lines with background, use a Frame to properly layer bg behind text
        if let Some(bg) = bg_color {
            egui::Frame::new()
                .fill(bg)
                .inner_margin(egui::Margin::symmetric(4, 1))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(content)
                            .color(text_color)
                            .size(typography::MD)
                            .monospace(),
                    );
                });
        } else {
            ui.label(
                RichText::new(content)
                    .color(text_color)
                    .size(typography::MD)
                    .monospace(),
            );
        }
    }

    /// Renders the footer with keyboard hints (like source_preview).
    fn render_footer(&self, ui: &mut egui::Ui, muted_text: Color32, separator_color: Color32) {
        // Separator above footer
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator_color),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Commit message (truncated)
            let msg = if self.commit_message.len() > 50 {
                format!("{}...", &self.commit_message[..47])
            } else {
                self.commit_message.clone()
            };
            ui.label(
                RichText::new(msg)
                    .color(muted_text)
                    .font(typography::proportional(typography::MD)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                // Keyboard hint
                let hint = if self.file_diffs.len() > 1 {
                    "N/P to cycle files • Esc to close"
                } else {
                    "Esc to close"
                };
                ui.label(
                    RichText::new(hint)
                        .color(muted_text)
                        .font(typography::proportional(typography::MD)),
                );
            });
        });
        ui.add_space(12.0);
    }
}

/// Parses a unified diff into per-file sections.
fn parse_diff_into_files(diff: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut current_file: Option<FileDiff> = None;

    for line in diff.lines() {
        // New file: diff --git a/path b/path
        if line.starts_with("diff --git") {
            // Save previous file
            if let Some(file) = current_file.take() {
                files.push(file);
            }

            // Extract path from "diff --git a/path b/path"
            let path = line
                .strip_prefix("diff --git a/")
                .and_then(|s| s.split(" b/").next())
                .unwrap_or("")
                .to_string();

            current_file = Some(FileDiff {
                path,
                lines: vec![DiffLine {
                    content: line.to_string(),
                    kind: DiffLineKind::FileHeader,
                }],
                additions: 0,
                deletions: 0,
            });
            continue;
        }

        // If we have a current file, add lines to it
        if let Some(ref mut file) = current_file {
            let kind = classify_diff_line(line);

            // Count additions/deletions
            match kind {
                DiffLineKind::Addition => file.additions += 1,
                DiffLineKind::Deletion => file.deletions += 1,
                _ => {}
            }

            file.lines.push(DiffLine {
                content: line.to_string(),
                kind,
            });
        }
    }

    // Don't forget the last file
    if let Some(file) = current_file {
        files.push(file);
    }

    files
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
