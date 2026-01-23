use egui::{Color32, Key, RichText, Stroke, TextEdit, Vec2};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::id_generator::next_id_usize;

/// The mode a buffer can be in (vim-style)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BufferMode {
    /// Normal mode - viewing/navigating the buffer
    #[default]
    Normal,
    /// Insert mode - editing the buffer content
    Insert,
}

/// A buffer represents an editable query that can be saved to produce a chart.
/// Inspired by vim buffers, each buffer has:
/// - A unique ID
/// - Content (the query string like "env:prod AND service:db")
/// - A mode (Normal or Insert)
/// - Modified state (dirty flag)
/// - An optional name/title
#[derive(Debug, Clone)]
pub struct Buffer {
    /// Unique identifier for this buffer
    id: usize,
    /// The buffer content (query string)
    content: String,
    /// The saved/committed content (what the chart shows)
    saved_content: String,
    /// Current editing mode
    mode: BufferMode,
    /// Whether the buffer has unsaved changes
    modified: bool,
    /// Display name for this buffer
    name: String,
    /// Current theme
    theme: AppTheme,
    /// API key (required by Component trait)
    api_key: String,
    /// Cursor position in the content (for insert mode)
    cursor_pos: usize,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new("")
    }
}

impl Buffer {
    /// Create a new buffer with the given initial content
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let saved_content = content.clone();
        Self {
            id: next_id_usize(),
            content,
            saved_content,
            mode: BufferMode::Normal,
            modified: false,
            name: String::new(),
            theme: AppTheme::default(),
            api_key: String::new(),
            cursor_pos: 0,
        }
    }

    /// Create a new buffer with a name
    pub fn with_name(content: impl Into<String>, name: impl Into<String>) -> Self {
        let mut buffer = Self::new(content);
        buffer.name = name.into();
        buffer
    }

    /// Get the buffer ID
    pub fn id(&self) -> usize {
        self.id
    }

    /// Get the current content (may have unsaved changes)
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the saved/committed content
    pub fn saved_content(&self) -> &str {
        &self.saved_content
    }

    /// Get the buffer name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set the buffer name
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Get the current mode
    pub fn mode(&self) -> BufferMode {
        self.mode
    }

    /// Check if buffer has unsaved changes
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Enter insert mode
    pub fn enter_insert_mode(&mut self) {
        self.mode = BufferMode::Insert;
        self.cursor_pos = self.content.len();
    }

    /// Enter normal mode
    pub fn enter_normal_mode(&mut self) {
        self.mode = BufferMode::Normal;
    }

    /// Save the buffer (commit current content)
    /// Returns true if there were changes to save
    pub fn save(&mut self) -> bool {
        if self.modified {
            self.saved_content = self.content.clone();
            self.modified = false;
            true
        } else {
            false
        }
    }

    /// Revert to saved content (discard unsaved changes)
    pub fn revert(&mut self) {
        self.content = self.saved_content.clone();
        self.modified = false;
        self.cursor_pos = self.content.len().min(self.cursor_pos);
    }

    /// Set the content directly
    pub fn set_content(&mut self, content: impl Into<String>) {
        let new_content = content.into();
        if new_content != self.content {
            self.content = new_content;
            self.modified = self.content != self.saved_content;
            self.cursor_pos = self.content.len().min(self.cursor_pos);
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the API key
    pub fn set_api_key(&mut self, key: &str) {
        self.api_key = key.to_string();
    }

    /// Get the display title for this buffer
    pub fn display_title(&self) -> String {
        let name = if self.name.is_empty() {
            format!("Buffer {}", self.id)
        } else {
            self.name.clone()
        };

        if self.modified {
            format!("{name} [+]")
        } else {
            name
        }
    }

    /// Get the mode indicator string
    fn mode_indicator(&self) -> &'static str {
        match self.mode {
            BufferMode::Normal => "NORMAL",
            BufferMode::Insert => "INSERT",
        }
    }

    /// Get the mode indicator color
    fn mode_color(&self) -> Color32 {
        match self.mode {
            BufferMode::Normal => self.theme.mode_normal(),
            BufferMode::Insert => self.theme.mode_insert(),
        }
    }

    /// Render the buffer UI
    /// Returns a `BufferAction` indicating what action should be taken
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) -> BufferAction {
        let text_col = self.theme.text_primary();

        // Buffer frame with mode-dependent styling
        let border_color = if self.mode == BufferMode::Insert {
            self.mode_color()
        } else {
            self.theme.buffer_border()
        };

        let bg_color = self.theme.buffer_bg();

        egui::Frame::new()
            .fill(bg_color)
            .stroke(Stroke::new(
                if self.mode == BufferMode::Insert {
                    2.0
                } else {
                    1.0
                },
                border_color,
            ))
            .corner_radius(4.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Top bar with mode indicator and buffer info
                    ui.horizontal(|ui| {
                        // Mode indicator badge
                        let mode_bg = self.mode_color();
                        let mode_text_color = Color32::WHITE;

                        egui::Frame::new()
                            .fill(mode_bg)
                            .corner_radius(3.0)
                            .inner_margin(egui::vec2(6.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(self.mode_indicator())
                                        .color(mode_text_color)
                                        .size(typography::XS)
                                        .strong(),
                                );
                            });

                        ui.add_space(8.0);

                        // Buffer name/title
                        ui.label(
                            RichText::new(self.display_title())
                                .color(text_col)
                                .size(typography::MD),
                        );

                        // Spacer
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Help hint
                            let hint = match self.mode {
                                BufferMode::Normal => "Press 'e' to edit, ':w' to save",
                                BufferMode::Insert => "Press 'Esc' to exit, ':w' to save",
                            };
                            ui.label(
                                RichText::new(hint)
                                    .color(text_col.gamma_multiply(0.4))
                                    .size(typography::XS),
                            );
                        });
                    });

                    ui.add_space(8.0);

                    // Query content area
                    let content_height = 60.0;

                    match self.mode {
                        BufferMode::Normal => {
                            // Display-only view with syntax highlighting
                            egui::Frame::new()
                                .fill(self.theme.buffer_content_bg())
                                .corner_radius(3.0)
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    ui.set_min_height(content_height);
                                    ui.set_width(ui.available_width());

                                    if self.content.is_empty() {
                                        ui.label(
                                            RichText::new("(empty query)")
                                                .color(text_col.gamma_multiply(0.3))
                                                .italics(),
                                        );
                                    } else {
                                        // Render the query with basic syntax highlighting
                                        self.render_highlighted_query(ui);
                                    }
                                });
                        }
                        BufferMode::Insert => {
                            // Editable text area
                            let mut content = self.content.clone();
                            let response = ui.add_sized(
                                Vec2::new(ui.available_width(), content_height),
                                TextEdit::multiline(&mut content)
                                    .font(typography::code())
                                    .hint_text("Enter your query (e.g., env:prod AND service:db)")
                                    .desired_rows(3),
                            );

                            // Request focus when entering insert mode
                            response.request_focus();

                            if response.changed() {
                                self.content = content;
                                self.modified = self.content != self.saved_content;
                            }
                        }
                    }

                    ui.add_space(8.0);

                    // Status bar
                    ui.horizontal(|ui| {
                        // Line/character count
                        let char_count = self.content.chars().count();
                        ui.label(
                            RichText::new(format!("{char_count} chars"))
                                .color(text_col.gamma_multiply(0.5))
                                .size(typography::XS),
                        );

                        ui.separator();

                        // Modified indicator
                        if self.modified {
                            ui.label(
                                RichText::new("[Modified]")
                                    .color(Color32::from_rgb(220, 160, 50))
                                    .size(typography::XS),
                            );
                        } else {
                            ui.label(
                                RichText::new("[Saved]")
                                    .color(text_col.gamma_multiply(0.4))
                                    .size(typography::XS),
                            );
                        }
                    });
                });
            });

        // Handle keyboard shortcuts based on mode
        self.handle_keyboard(ui.ctx())
    }

    /// Render the query with basic syntax highlighting
    fn render_highlighted_query(&self, ui: &mut egui::Ui) {
        let text_col = self.theme.text_primary();
        let keyword_color = self.theme.syntax_keyword();
        let operator_color = self.theme.syntax_key();
        let value_color = self.theme.syntax_value();

        // Simple token-based highlighting
        let mut job = egui::text::LayoutJob::default();
        let font_id = typography::code();

        let keywords = ["AND", "OR", "NOT"];

        for word in self.content.split_inclusive(|c: char| c.is_whitespace()) {
            let trimmed = word.trim();

            let color = if keywords.contains(&trimmed.to_uppercase().as_str()) {
                keyword_color
            } else if trimmed.contains(':') {
                // key:value pair - highlight the value part differently
                let parts: Vec<&str> = word.splitn(2, ':').collect();
                if parts.len() == 2 {
                    // Add key part
                    job.append(
                        parts[0],
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color: operator_color,
                            ..Default::default()
                        },
                    );
                    job.append(
                        ":",
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color: text_col.gamma_multiply(0.6),
                            ..Default::default()
                        },
                    );
                    job.append(
                        parts[1],
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color: value_color,
                            ..Default::default()
                        },
                    );
                    continue;
                }
                text_col
            } else {
                text_col
            };

            job.append(
                word,
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color,
                    ..Default::default()
                },
            );
        }

        ui.label(job);
    }

    /// Handle keyboard input based on current mode
    fn handle_keyboard(&mut self, ctx: &egui::Context) -> BufferAction {
        // Don't process shortcuts if a text field has focus (let it handle input)
        if self.mode == BufferMode::Insert {
            // Escape - exit insert mode without saving
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.enter_normal_mode();
                return BufferAction::ModeChanged(BufferMode::Normal);
            }
            // Enter - save and exit insert mode (for single-line query editing)
            if ctx.input(|i| i.key_pressed(Key::Enter)) {
                let had_changes = self.save();
                self.enter_normal_mode();
                if had_changes {
                    return BufferAction::Saved;
                }
                return BufferAction::ModeChanged(BufferMode::Normal);
            }
            return BufferAction::None;
        }

        // Normal mode keyboard handling
        let mut action = BufferAction::None;

        ctx.input(|input| {
            // 'e' or 'i' - enter insert mode
            if input.key_pressed(Key::E) || input.key_pressed(Key::I) {
                action = BufferAction::EnterInsertMode;
            }
        });

        if action == BufferAction::EnterInsertMode {
            self.enter_insert_mode();
            return BufferAction::ModeChanged(BufferMode::Insert);
        }

        action
    }
}

/// Actions that can result from buffer interaction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferAction {
    /// No action
    None,
    /// Request to enter insert mode
    EnterInsertMode,
    /// Mode was changed
    ModeChanged(BufferMode),
    /// Buffer was saved
    Saved,
    /// Buffer was reverted
    Reverted,
}

/// Implement Component trait so Buffer can be used in the dashboard
impl crate::components::Component for Buffer {
    fn show(&mut self, ui: &mut egui::Ui) {
        Buffer::show(self, ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        if self.name.is_empty() {
            format!("Buffer {}", self.id)
        } else {
            self.name.clone()
        }
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    fn set_api_key(&mut self, key: &str) {
        self.api_key = key.to_string();
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed
    }

    fn label(&self) -> egui::RichText {
        let icon = match self.mode {
            BufferMode::Normal => semantic_icons::file::TEXT,
            BufferMode::Insert => semantic_icons::action::EDIT,
        };

        let title = self.display_title();
        egui::RichText::new(format!("{icon} {title}"))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
