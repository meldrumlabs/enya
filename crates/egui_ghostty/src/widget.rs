//! Terminal widget for egui.

use egui::{FontFamily, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::colors::{ColorScheme, rgb_to_color32};

/// Padding around the terminal content (in pixels).
const TERMINAL_PADDING: f32 = 8.0;
use crate::config::TerminalConfig;
use crate::input::{
    MouseButton, egui_key_to_name, egui_to_ghostty_modifiers, encode_sgr_mouse, encode_text_input,
};
use crate::session::{SessionError, TerminalSession};

/// Response from showing a terminal widget.
#[derive(Default)]
pub struct TerminalResponse {
    /// Whether the terminal has focus.
    pub has_focus: bool,
    /// The terminal title (from OSC sequences).
    pub title: Option<String>,
    /// Whether the user requested to close the terminal.
    pub close_requested: bool,
    /// Whether the user released focus from the terminal (Escape key).
    pub focus_released: bool,
}

/// Selection position in the terminal (col, row), both 0-indexed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectionPos {
    col: usize,
    row: usize,
}

impl SelectionPos {
    fn new(col: usize, row: usize) -> Self {
        Self { col, row }
    }

    /// Returns true if self comes before other in reading order.
    fn is_before(&self, other: &Self) -> bool {
        self.row < other.row || (self.row == other.row && self.col < other.col)
    }
}

/// A terminal emulator widget for egui.
pub struct TerminalWidget {
    /// The terminal session.
    session: TerminalSession,
    /// Configuration.
    config: TerminalConfig,
    /// Color scheme.
    color_scheme: ColorScheme,
    /// Unique ID for this widget (for focus management).
    #[allow(dead_code)]
    id: egui::Id,
    /// Time of last cursor blink toggle.
    last_blink_time: f64,
    /// Current cursor visibility (for blinking).
    cursor_visible: bool,
    /// Whether mouse reporting is enabled.
    mouse_reporting: bool,
    /// Cached cell dimensions.
    cell_size: Option<Vec2>,
    /// Selection start position (anchor).
    selection_start: Option<SelectionPos>,
    /// Selection end position (current drag position).
    selection_end: Option<SelectionPos>,
    /// Whether we're currently dragging to select.
    is_selecting: bool,
    /// Whether to auto-focus on next show (set on creation).
    auto_focus: bool,
}

impl TerminalWidget {
    /// Create a new terminal widget.
    ///
    /// # Arguments
    /// * `config` - Terminal configuration
    /// * `shell` - Shell command to run (e.g., "/bin/zsh", "/bin/bash")
    pub fn new(config: TerminalConfig, shell: &str) -> Result<Self, SessionError> {
        let session = TerminalSession::new(&config, shell)?;
        Ok(Self {
            session,
            config,
            color_scheme: ColorScheme::default(),
            id: egui::Id::new("terminal").with(fastrand::u64(..)),
            last_blink_time: 0.0,
            cursor_visible: true,
            mouse_reporting: false,
            cell_size: None,
            selection_start: None,
            selection_end: None,
            is_selecting: false,
            auto_focus: true,
        })
    }

    /// Set the color scheme.
    pub fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.session
            .set_default_colors(scheme.foreground, scheme.background);
        self.color_scheme = scheme;
    }

    /// Get the terminal title.
    pub fn title(&self) -> &str {
        self.session.title()
    }

    /// Show the terminal widget.
    pub fn show(&mut self, ui: &mut Ui) -> TerminalResponse {
        let mut response = TerminalResponse::default();

        // Process any pending PTY output
        let had_output = self.session.process_output();

        // Get available space (accounting for padding)
        let available_size = ui.available_size();
        let content_size = Vec2::new(
            (available_size.x - TERMINAL_PADDING * 2.0).max(0.0),
            (available_size.y - TERMINAL_PADDING * 2.0).max(0.0),
        );

        // Calculate cell dimensions from font
        let cell_size = self.calculate_cell_size(ui);
        self.cell_size = Some(cell_size);

        // Calculate how many cells fit in the content area
        let cols = (content_size.x / cell_size.x).floor() as u16;
        let rows = (content_size.y / cell_size.y).floor() as u16;

        // Resize if needed (with minimum size)
        let cols = cols.max(10);
        let rows = rows.max(3);
        if let Err(e) = self.session.resize(cols, rows) {
            log::warn!("Failed to resize terminal: {e}");
        }

        // Allocate the full terminal area (including padding)
        let content_rect_size = Vec2::new(cols as f32 * cell_size.x, rows as f32 * cell_size.y);
        let terminal_size = content_rect_size + Vec2::splat(TERMINAL_PADDING * 2.0);
        let (outer_rect, resp) = ui.allocate_exact_size(terminal_size, Sense::click_and_drag());

        // Calculate inner content rect (inset by padding)
        let content_rect = Rect::from_min_size(
            outer_rect.min + Vec2::splat(TERMINAL_PADDING),
            content_rect_size,
        );

        // Handle focus - via click, Enter when hovered, or auto-focus on first show
        if resp.clicked() {
            ui.memory_mut(|mem| mem.request_focus(resp.id));
        }

        // Auto-focus on first show (e.g., when terminal is newly created)
        if self.auto_focus {
            self.auto_focus = false;
            ui.memory_mut(|mem| mem.request_focus(resp.id));
        }

        // Allow Enter to focus the terminal when not focused
        // This lets users re-enter the terminal after switching panes without using mouse
        if !resp.has_focus() {
            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter_pressed {
                ui.memory_mut(|mem| mem.request_focus(resp.id));
            }
        }

        response.has_focus = resp.has_focus();

        // Update cursor blinking
        let time = ui.ctx().input(|i| i.time);
        if self.config.cursor_blink_interval > 0.0 {
            if time - self.last_blink_time > self.config.cursor_blink_interval as f64 {
                self.cursor_visible = !self.cursor_visible;
                self.last_blink_time = time;
            }
        } else {
            self.cursor_visible = true;
        }

        // Handle keyboard input when focused; release focus on Ctrl+Shift+Escape
        if resp.has_focus() && self.handle_keyboard_input(ui) {
            ui.memory_mut(|mem| mem.surrender_focus(resp.id));
            response.focus_released = true;
        }

        // Handle mouse input (use content_rect for cell calculations)
        self.handle_mouse_input(ui, &resp, content_rect, cell_size);

        // Render the terminal (outer_rect for background, content_rect for text)
        self.render(
            ui,
            outer_rect,
            content_rect,
            cell_size,
            rows,
            resp.has_focus(),
        );

        // Request repaint to keep terminal responsive:
        // - Immediate repaint if we had output (process more quickly)
        // - Immediate repaint if cursor is blinking with focus
        // - Otherwise, schedule repaint after 50ms to poll for new PTY output
        //   (keeps htop, kubectl logs, etc. updating even without focus)
        if had_output || (resp.has_focus() && self.config.cursor_blink_interval > 0.0) {
            ui.ctx().request_repaint();
        } else {
            // Poll for new output at ~20fps when terminal is idle/unfocused
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }

        // Update title
        response.title = Some(self.session.title().to_string());

        response
    }

    /// Calculate cell dimensions based on the monospace font.
    fn calculate_cell_size(&self, ui: &Ui) -> Vec2 {
        let font_id = FontId::new(14.0, FontFamily::Monospace);

        // Measure character width and row height by laying out a reference character
        // We use 'M' as it's typically the widest character in most fonts
        let galley = ui
            .painter()
            .layout_no_wrap("M".to_string(), font_id, egui::Color32::WHITE);
        let char_width = galley.rect.width();
        let row_height = galley.rect.height();

        Vec2::new(char_width, row_height)
    }

    /// Handle keyboard input.
    ///
    /// Returns `true` if focus should be released from the terminal.
    fn handle_keyboard_input(&mut self, ui: &Ui) -> bool {
        let mut release_focus = false;
        let mut had_text_input = false;

        // Check for paste (Cmd/Ctrl+V)
        let paste_text = ui.input(|input| {
            if input.modifiers.command && input.key_pressed(egui::Key::V) {
                // Get clipboard content
                ui.ctx().input(|i| {
                    i.events.iter().find_map(|e| {
                        if let egui::Event::Paste(text) = e {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                })
            } else {
                None
            }
        });

        if let Some(text) = paste_text {
            // Write pasted text to terminal
            if let Err(e) = self.session.write(text.as_bytes()) {
                log::warn!("Failed to paste to terminal: {e}");
            }
            self.clear_selection();
        }

        ui.input(|input| {
            // Handle special keys
            for event in &input.events {
                match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        // Escape (with or without modifiers) releases focus from terminal
                        // Users can send actual Escape to the shell using Ctrl+[
                        if *key == egui::Key::Escape {
                            release_focus = true;
                            continue;
                        }

                        // Handle scrollback navigation with Shift+PageUp/PageDown
                        if modifiers.shift {
                            match key {
                                egui::Key::PageUp => {
                                    // Scroll up by ~half a page
                                    let (_, rows) = self.session.size();
                                    let lines = (rows / 2).max(1) as i32;
                                    let _ = self.session.scroll(lines);
                                    continue;
                                }
                                egui::Key::PageDown => {
                                    // Scroll down by ~half a page
                                    let (_, rows) = self.session.size();
                                    let lines = (rows / 2).max(1) as i32;
                                    let _ = self.session.scroll(-lines);
                                    continue;
                                }
                                egui::Key::Home => {
                                    // Scroll to top
                                    let _ = self.session.scroll_to_top();
                                    continue;
                                }
                                egui::Key::End => {
                                    // Scroll to bottom
                                    let _ = self.session.scroll_to_bottom();
                                    continue;
                                }
                                _ => {}
                            }
                        }

                        // Try to encode as a named key
                        if let Some(name) = egui_key_to_name(*key) {
                            let mods = egui_to_ghostty_modifiers(modifiers);
                            if let Some(encoded) = ghostty_vt::encode_key_named(name, mods) {
                                if let Err(e) = self.session.write(&encoded) {
                                    log::warn!("Failed to write to terminal: {e}");
                                }
                                had_text_input = true;
                            }
                        }
                    }
                    egui::Event::Text(text) => {
                        // Regular text input
                        let modifiers = input.modifiers;
                        let encoded = encode_text_input(text, &modifiers);
                        if !encoded.is_empty() {
                            if let Err(e) = self.session.write(&encoded) {
                                log::warn!("Failed to write to terminal: {e}");
                            }
                            had_text_input = true;
                        }
                    }
                    _ => {}
                }
            }
        });

        // Clear selection when user types
        if had_text_input {
            self.clear_selection();
        }

        release_focus
    }

    /// Clear the current selection.
    fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
        self.is_selecting = false;
    }

    /// Convert screen position to terminal cell position.
    fn pos_to_cell(&self, pos: Pos2, rect: Rect, cell_size: Vec2) -> SelectionPos {
        let col = ((pos.x - rect.left()) / cell_size.x).max(0.0) as usize;
        let row = ((pos.y - rect.top()) / cell_size.y).max(0.0) as usize;
        SelectionPos::new(col, row)
    }

    /// Handle mouse input.
    fn handle_mouse_input(&mut self, ui: &Ui, resp: &Response, rect: Rect, cell_size: Vec2) {
        // Check if pointer is in terminal area
        let pointer_in_rect = ui.input(|input| {
            input
                .pointer
                .hover_pos()
                .map(|pos| rect.contains(pos))
                .unwrap_or(false)
        });

        // Get scroll delta
        let scroll_delta = ui.input(|input| input.smooth_scroll_delta.y);

        if self.mouse_reporting {
            if !pointer_in_rect {
                return;
            }
            // In mouse reporting mode, send mouse events to the application
            ui.input(|input| {
                if let Some(pos) = input.pointer.hover_pos() {
                    let col = ((pos.x - rect.left()) / cell_size.x) as u16 + 1;
                    let row = ((pos.y - rect.top()) / cell_size.y) as u16 + 1;

                    // Handle clicks
                    if input.pointer.primary_clicked() {
                        let encoded =
                            encode_sgr_mouse(MouseButton::Left, col, row, true, &input.modifiers);
                        let _ = self.session.write(&encoded);
                    }
                    if input.pointer.primary_released() {
                        let encoded =
                            encode_sgr_mouse(MouseButton::Left, col, row, false, &input.modifiers);
                        let _ = self.session.write(&encoded);
                    }

                    // Handle scroll (send to application)
                    let scroll = input.raw_scroll_delta.y;
                    if scroll > 0.0 {
                        let encoded = encode_sgr_mouse(
                            MouseButton::WheelUp,
                            col,
                            row,
                            true,
                            &input.modifiers,
                        );
                        let _ = self.session.write(&encoded);
                    } else if scroll < 0.0 {
                        let encoded = encode_sgr_mouse(
                            MouseButton::WheelDown,
                            col,
                            row,
                            true,
                            &input.modifiers,
                        );
                        let _ = self.session.write(&encoded);
                    }
                }
            });
        } else {
            // Normal mode: handle selection and scrollback

            // Handle scrollback navigation
            if pointer_in_rect {
                let scroll_lines = (scroll_delta / cell_size.y * 3.0) as i32;
                if scroll_lines != 0 {
                    if let Err(e) = self.session.scroll(scroll_lines) {
                        log::warn!("Failed to scroll terminal: {e}");
                    }
                }
            }

            // Handle text selection
            let (primary_pressed, primary_down, primary_released, pointer_pos) =
                ui.input(|input| {
                    (
                        input.pointer.primary_pressed(),
                        input.pointer.primary_down(),
                        input.pointer.primary_released(),
                        input.pointer.hover_pos(),
                    )
                });

            if let Some(pos) = pointer_pos {
                // Start selection on click
                if primary_pressed && rect.contains(pos) {
                    let cell_pos = self.pos_to_cell(pos, rect, cell_size);
                    self.selection_start = Some(cell_pos);
                    self.selection_end = Some(cell_pos);
                    self.is_selecting = true;
                }

                // Update selection while dragging
                if self.is_selecting && primary_down {
                    let cell_pos = self.pos_to_cell(pos, rect, cell_size);
                    self.selection_end = Some(cell_pos);
                }

                // End selection on release
                if primary_released && self.is_selecting {
                    self.is_selecting = false;
                    // If start and end are the same, clear selection (it was just a click)
                    if self.selection_start == self.selection_end {
                        self.selection_start = None;
                        self.selection_end = None;
                    }
                }
            }

            // Handle copy with Cmd/Ctrl+C when there's a selection
            let copy_requested =
                ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::C));
            if copy_requested && self.has_selection() {
                if let Some(text) = self.get_selected_text() {
                    ui.ctx().copy_text(text);
                }
            }
        }

        // Request focus on click (for both modes)
        if resp.clicked() {
            ui.memory_mut(|mem| mem.request_focus(resp.id));
        }
    }

    /// Check if there's an active selection.
    fn has_selection(&self) -> bool {
        self.selection_start.is_some()
            && self.selection_end.is_some()
            && self.selection_start != self.selection_end
    }

    /// Get the selected text.
    fn get_selected_text(&self) -> Option<String> {
        let start = self.selection_start?;
        let end = self.selection_end?;

        // Normalize so start is before end
        let (start, end) = if start.is_before(&end) {
            (start, end)
        } else {
            (end, start)
        };

        let terminal = self.session.terminal();
        let mut result = String::new();

        for row in start.row..=end.row {
            let row_text = terminal.dump_viewport_row(row as u16).ok()?;
            let chars: Vec<char> = row_text.chars().collect();

            let col_start = if row == start.row { start.col } else { 0 };
            let col_end = if row == end.row {
                end.col.min(chars.len())
            } else {
                chars.len()
            };

            if col_start < chars.len() {
                let selected: String = chars[col_start..col_end].iter().collect();
                result.push_str(&selected);
            }

            // Add newline between rows (but not at the very end)
            if row < end.row {
                result.push('\n');
            }
        }

        Some(result.trim_end().to_string())
    }

    /// Get normalized selection range (start before end).
    fn get_selection_range(&self) -> Option<(SelectionPos, SelectionPos)> {
        let start = self.selection_start?;
        let end = self.selection_end?;
        if start.is_before(&end) {
            Some((start, end))
        } else {
            Some((end, start))
        }
    }

    /// Render the terminal contents.
    ///
    /// - `outer_rect`: The full terminal area including padding (for background/border)
    /// - `content_rect`: The inner area where text is rendered
    fn render(
        &self,
        ui: &Ui,
        outer_rect: Rect,
        content_rect: Rect,
        cell_size: Vec2,
        rows: u16,
        has_focus: bool,
    ) {
        let painter = ui.painter_at(outer_rect);

        // Draw background (fills entire outer rect including padding)
        painter.rect_filled(
            outer_rect,
            0.0,
            rgb_to_color32(self.color_scheme.background),
        );

        // Draw focus indicator border
        if has_focus {
            // Focused: subtle accent border
            let border_color = rgb_to_color32(self.color_scheme.cursor).gamma_multiply(0.6);
            painter.rect_stroke(
                outer_rect,
                2.0,
                Stroke::new(2.0, border_color),
                egui::StrokeKind::Inside,
            );
        } else {
            // Unfocused: dim dashed border to clearly show terminal is not receiving input
            let border_color = rgb_to_color32(self.color_scheme.foreground).gamma_multiply(0.15);
            painter.rect_stroke(
                outer_rect,
                2.0,
                Stroke::new(1.0, border_color),
                egui::StrokeKind::Inside,
            );

            // Show "Click to focus" hint in the center when unfocused and hovered
            if ui.rect_contains_pointer(outer_rect) {
                let hint_text = "Click to focus terminal";
                let hint_color = rgb_to_color32(self.color_scheme.foreground).gamma_multiply(0.4);
                painter.text(
                    outer_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    hint_text,
                    FontId::new(12.0, FontFamily::Proportional),
                    hint_color,
                );
            }
        }

        let font_id = FontId::new(14.0, FontFamily::Monospace);
        let terminal = self.session.terminal();
        let default_fg = rgb_to_color32(self.color_scheme.foreground);
        let default_bg = rgb_to_color32(self.color_scheme.background);

        // Render each row (in content_rect, which is inset by padding)
        for row_idx in 0..rows {
            let y = content_rect.top() + (row_idx as f32 * cell_size.y);

            // Get row text and styles
            let row_text = match terminal.dump_viewport_row(row_idx) {
                Ok(text) => text,
                Err(_) => continue,
            };

            if row_text.is_empty() {
                continue;
            }

            // Get style runs for efficient rendering
            let style_runs = terminal
                .dump_viewport_row_style_runs(row_idx)
                .unwrap_or_default();

            // Always render the full row text first with default colors
            // This ensures no characters are missed even if style runs have gaps
            painter.text(
                Pos2::new(content_rect.left(), y),
                egui::Align2::LEFT_TOP,
                &row_text,
                font_id.clone(),
                default_fg,
            );

            // Now overlay styled portions
            if !style_runs.is_empty() {
                let chars: Vec<char> = row_text.chars().collect();

                for run in &style_runs {
                    // Style runs use 1-indexed columns
                    let start = run.start_col.saturating_sub(1) as usize;
                    // end_col is 1-indexed inclusive, convert to 0-indexed exclusive
                    // by using end_col directly (not subtracting 1)
                    let end = run.end_col as usize;

                    if start >= chars.len() {
                        continue;
                    }

                    let end = end.min(chars.len());
                    if start >= end {
                        continue;
                    }

                    let run_text: String = chars[start..end].iter().collect();
                    if run_text.is_empty() {
                        continue;
                    }

                    let x = content_rect.left() + (start as f32 * cell_size.x);
                    let run_width = (end - start) as f32 * cell_size.x;

                    // Check if this run has non-default styling
                    let bg_color = rgb_to_color32(run.bg);
                    let fg_color = rgb_to_color32(run.fg);
                    let has_custom_bg = bg_color != default_bg;
                    let has_custom_fg = fg_color != default_fg;
                    let has_decorations = run.is_underline() || run.is_strikethrough();

                    // Only redraw if styling differs from default
                    if has_custom_bg || has_custom_fg || has_decorations {
                        // Draw background (always draw to cover the default text)
                        let bg_rect =
                            Rect::from_min_size(Pos2::new(x, y), Vec2::new(run_width, cell_size.y));
                        painter.rect_filled(bg_rect, 0.0, bg_color);

                        // Draw text with styled color
                        painter.text(
                            Pos2::new(x, y),
                            egui::Align2::LEFT_TOP,
                            &run_text,
                            font_id.clone(),
                            fg_color,
                        );

                        // Draw underline if needed
                        if run.is_underline() {
                            let underline_y = y + cell_size.y - 2.0;
                            painter.line_segment(
                                [
                                    Pos2::new(x, underline_y),
                                    Pos2::new(x + run_width, underline_y),
                                ],
                                Stroke::new(1.0, fg_color),
                            );
                        }

                        // Draw strikethrough if needed
                        if run.is_strikethrough() {
                            let strike_y = y + cell_size.y / 2.0;
                            painter.line_segment(
                                [Pos2::new(x, strike_y), Pos2::new(x + run_width, strike_y)],
                                Stroke::new(1.0, fg_color),
                            );
                        }
                    }
                }
            }
        }

        // Draw selection highlight
        if let Some((sel_start, sel_end)) = self.get_selection_range() {
            let selection_color = self.color_scheme.selection_bg;
            let selection_bg = rgb_to_color32(selection_color).gamma_multiply(0.5);

            for row in sel_start.row..=sel_end.row {
                if row >= rows as usize {
                    break;
                }

                // Get the row text to know its length
                let row_len = terminal
                    .dump_viewport_row(row as u16)
                    .map(|t| t.chars().count())
                    .unwrap_or(0);

                let col_start = if row == sel_start.row {
                    sel_start.col
                } else {
                    0
                };
                let col_end = if row == sel_end.row {
                    sel_end.col.min(row_len)
                } else {
                    row_len
                };

                if col_start < col_end {
                    let sel_x = content_rect.left() + (col_start as f32 * cell_size.x);
                    let sel_y = content_rect.top() + (row as f32 * cell_size.y);
                    let sel_width = (col_end - col_start) as f32 * cell_size.x;

                    let sel_rect = Rect::from_min_size(
                        Pos2::new(sel_x, sel_y),
                        Vec2::new(sel_width, cell_size.y),
                    );
                    painter.rect_filled(sel_rect, 0.0, selection_bg);
                }
            }
        }

        // Draw cursor
        if let Some((col, row)) = terminal.cursor_position() {
            let cursor_x = content_rect.left() + ((col - 1) as f32 * cell_size.x);
            let cursor_y = content_rect.top() + ((row - 1) as f32 * cell_size.y);

            let cursor_rect = Rect::from_min_size(Pos2::new(cursor_x, cursor_y), cell_size);
            let cursor_color = rgb_to_color32(self.color_scheme.cursor);

            if has_focus && self.cursor_visible {
                // Focused: solid cursor
                if self.config.block_cursor {
                    // Block cursor - solid fill with outline for visibility
                    painter.rect_filled(cursor_rect, 0.0, cursor_color.gamma_multiply(0.85));
                    // Add a bright outline for better visibility
                    painter.rect_stroke(
                        cursor_rect,
                        0.0,
                        Stroke::new(1.5, cursor_color),
                        egui::StrokeKind::Outside,
                    );
                } else {
                    // Beam cursor - thicker line with glow effect
                    let beam_width = 2.5;
                    let beam_rect = Rect::from_min_size(
                        Pos2::new(cursor_x, cursor_y),
                        Vec2::new(beam_width, cell_size.y),
                    );
                    // Glow effect
                    let glow_rect = Rect::from_min_size(
                        Pos2::new(cursor_x - 1.0, cursor_y),
                        Vec2::new(beam_width + 2.0, cell_size.y),
                    );
                    painter.rect_filled(glow_rect, 0.0, cursor_color.gamma_multiply(0.3));
                    // Solid beam
                    painter.rect_filled(beam_rect, 0.0, cursor_color);
                }
            } else if !has_focus {
                // Unfocused: hollow/outline cursor to show position
                painter.rect_stroke(
                    cursor_rect,
                    0.0,
                    Stroke::new(1.5, cursor_color.gamma_multiply(0.5)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    /// Write data directly to the terminal (for programmatic input).
    pub fn write(&mut self, data: &[u8]) -> Result<(), SessionError> {
        self.session.write(data)
    }

    /// Get the current terminal size in cells.
    pub fn size(&self) -> (u16, u16) {
        self.session.size()
    }

    /// Enable or disable mouse reporting.
    pub fn set_mouse_reporting(&mut self, enabled: bool) {
        self.mouse_reporting = enabled;
    }
}
