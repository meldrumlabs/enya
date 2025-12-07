use egui::{Color32, FontId, Key, RichText};

use crate::components::query_state::QueryState;
use crate::theme::AppTheme;
use crate::ui::colors::text_color;

/// Result of the buffer editor modal
#[derive(Debug, Clone)]
pub enum BufferEditorResult {
    /// No action (modal still open or was cancelled)
    None,
    /// Query was saved - contains the query string and query state
    Saved(String, QueryState),
    /// Editor was cancelled (Escape pressed)
    Cancelled,
}

/// A modal overlay for editing buffer queries, styled like the fuzzy finder.
/// Opens as a transparent overlay so the chart remains visible underneath.
/// Includes keybindings for aggregation mode and granularity.
pub struct BufferEditor {
    /// Whether the editor is currently open
    is_open: bool,
    /// The query being edited
    query: String,
    /// The original query (for revert on cancel)
    original_query: String,
    /// Name/title of the buffer being edited
    buffer_name: String,
    /// Current theme
    theme: AppTheme,
    /// Whether the text input should request focus
    needs_focus: bool,
    /// Query state (aggregation, granularity, time range)
    query_state: QueryState,
    /// Original query state (for revert on cancel)
    original_query_state: QueryState,
}

impl Default for BufferEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferEditor {
    pub fn new() -> Self {
        Self {
            is_open: false,
            query: String::new(),
            original_query: String::new(),
            buffer_name: String::new(),
            theme: AppTheme::default(),
            needs_focus: false,
            query_state: QueryState::default(),
            original_query_state: QueryState::default(),
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if the editor is currently open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Open the editor with the given query, buffer name, and query state
    pub fn open(&mut self, query: &str, buffer_name: &str) {
        self.open_with_state(query, buffer_name, QueryState::default());
    }

    /// Open the editor with a specific query state
    pub fn open_with_state(&mut self, query: &str, buffer_name: &str, state: QueryState) {
        self.is_open = true;
        self.query = query.to_string();
        self.original_query = query.to_string();
        self.buffer_name = buffer_name.to_string();
        self.needs_focus = true;
        self.query_state = state.clone();
        self.original_query_state = state;
    }

    /// Close the editor without saving
    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.original_query.clear();
        self.buffer_name.clear();
        self.query_state = QueryState::default();
        self.original_query_state = QueryState::default();
    }

    /// Get the current query content
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Get the current query state
    pub fn query_state(&self) -> &QueryState {
        &self.query_state
    }

    /// Check if the query has been modified
    pub fn is_modified(&self) -> bool {
        self.query != self.original_query || self.query_state != self.original_query_state
    }

    /// Show the buffer editor modal. Returns the result of the interaction.
    pub fn show(&mut self, ctx: &egui::Context) -> BufferEditorResult {
        if !self.is_open {
            return BufferEditorResult::None;
        }

        let mut result = BufferEditorResult::None;
        let mut should_close = false;
        let mut should_save = false;

        // Handle keyboard shortcuts
        ctx.input(|input| {
            // Escape - cancel and close
            if input.key_pressed(Key::Escape) {
                should_close = true;
            }
            // Ctrl+Enter or Cmd+Enter - save and close
            if input.key_pressed(Key::Enter) && input.modifiers.command {
                should_save = true;
            }
        });

        // Handle aggregation keybindings (Ctrl+key)
        ctx.input(|input| {
            if input.modifiers.ctrl {
                // Ctrl+S - toggle sum
                if input.key_pressed(Key::S) {
                    self.query_state.set_sum();
                }
                // Ctrl+A - toggle avg
                if input.key_pressed(Key::A) {
                    self.query_state.set_avg();
                }
                // Ctrl+P - cycle percentiles (p50 -> p95 -> p99 -> off)
                if input.key_pressed(Key::P) {
                    self.query_state.cycle_percentiles();
                }
                // Ctrl+M - toggle min
                if input.key_pressed(Key::M) {
                    self.query_state.set_min();
                }
                // Ctrl+X - toggle max
                if input.key_pressed(Key::X) {
                    self.query_state.set_max();
                }
                // Ctrl+G - cycle granularity
                if input.key_pressed(Key::G) {
                    self.query_state.cycle_granularity();
                }
            }
            // < and > to adjust granularity (no modifier needed)
            if input.key_pressed(Key::Period) && input.modifiers.shift {
                // > key (shift+period) - increase granularity
                self.query_state.cycle_granularity();
            }
            if input.key_pressed(Key::Comma) && input.modifiers.shift {
                // < key (shift+comma) - decrease granularity
                self.query_state.cycle_granularity_back();
            }
        });

        // Semi-transparent backdrop
        #[allow(deprecated)]
        let screen_rect = ctx.screen_rect();
        egui::Area::new(egui::Id::new("buffer_editor_backdrop"))
            .fixed_pos(screen_rect.min)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                let backdrop_color = match self.theme {
                    AppTheme::Light => Color32::from_rgba_unmultiplied(255, 255, 255, 120),
                    AppTheme::Dark => Color32::from_rgba_unmultiplied(0, 0, 0, 150),
                };
                ui.painter().rect_filled(screen_rect, 0.0, backdrop_color);
            });

        // Main editor popup
        let popup_width = (screen_rect.width() * 0.7).clamp(500.0, 900.0);

        egui::Area::new(egui::Id::new("buffer_editor_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let bg_color = match self.theme {
                    AppTheme::Light => Color32::from_rgb(250, 250, 250),
                    AppTheme::Dark => Color32::from_rgb(30, 30, 35),
                };
                let accent_color = match self.theme {
                    AppTheme::Light => Color32::from_rgb(80, 120, 200),
                    AppTheme::Dark => Color32::from_rgb(130, 180, 255),
                };

                egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(2.0, accent_color))
                    .corner_radius(8.0)
                    .inner_margin(0.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(100),
                    })
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Header with mode indicator and buffer name
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);

                            // INSERT mode badge
                            egui::Frame::new()
                                .fill(Color32::from_rgb(100, 160, 80))
                                .corner_radius(3.0)
                                .inner_margin(egui::vec2(8.0, 3.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("INSERT")
                                            .color(Color32::WHITE)
                                            .size(11.0)
                                            .strong(),
                                    );
                                });

                            ui.add_space(12.0);

                            // Buffer name
                            ui.label(
                                RichText::new(&self.buffer_name)
                                    .color(text_color(self.theme))
                                    .size(14.0),
                            );

                            // Modified indicator
                            if self.is_modified() {
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("[+]")
                                        .color(Color32::from_rgb(220, 160, 50))
                                        .size(12.0),
                                );
                            }

                            // Spacer and close hint
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(16.0);
                                    ui.label(
                                        RichText::new("Esc to cancel")
                                            .color(text_color(self.theme).gamma_multiply(0.4))
                                            .size(11.0),
                                    );
                                },
                            );
                        });

                        ui.add_space(12.0);

                        // Separator
                        let separator_color = match self.theme {
                            AppTheme::Light => Color32::from_rgb(220, 220, 220),
                            AppTheme::Dark => Color32::from_rgb(50, 50, 55),
                        };
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );

                        ui.add_space(8.0);

                        // Query input label
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(egui_phosphor::regular::CODE)
                                    .color(text_color(self.theme).gamma_multiply(0.6))
                                    .size(14.0),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Query")
                                    .color(text_color(self.theme).gamma_multiply(0.7))
                                    .size(12.0),
                            );
                        });

                        ui.add_space(8.0);

                        // Query text editor
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);

                            let editor_width = popup_width - 32.0;
                            let text_edit = egui::TextEdit::multiline(&mut self.query)
                                .font(FontId::monospace(14.0))
                                .hint_text(
                                    RichText::new("Enter query (e.g., env:prod AND service:db)")
                                        .color(text_color(self.theme).gamma_multiply(0.4)),
                                )
                                .desired_width(editor_width)
                                .desired_rows(4)
                                .lock_focus(true);

                            let response = ui.add(text_edit);

                            // Request focus on first show
                            if self.needs_focus {
                                response.request_focus();
                                self.needs_focus = false;
                            }

                            ui.add_space(16.0);
                        });

                        ui.add_space(12.0);

                        // Separator
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );

                        ui.add_space(8.0);

                        // Status line showing aggregation state
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);

                            let status_bg = match self.theme {
                                AppTheme::Light => Color32::from_rgb(235, 240, 250),
                                AppTheme::Dark => Color32::from_rgb(40, 45, 55),
                            };

                            egui::Frame::new()
                                .fill(status_bg)
                                .corner_radius(4.0)
                                .inner_margin(egui::vec2(8.0, 4.0))
                                .show(ui, |ui| {
                                    let agg_color = if self.query_state.aggregation
                                        == crate::components::query_state::AggregationMode::None
                                    {
                                        text_color(self.theme).gamma_multiply(0.5)
                                    } else {
                                        Color32::from_rgb(100, 180, 100)
                                    };

                                    // Aggregation badge
                                    ui.label(
                                        RichText::new(format!(
                                            "[{}]",
                                            self.query_state.aggregation.label()
                                        ))
                                        .color(agg_color)
                                        .size(12.0)
                                        .strong(),
                                    );

                                    ui.add_space(8.0);

                                    // Time range
                                    ui.label(
                                        RichText::new(&self.query_state.time_range_label)
                                            .color(text_color(self.theme).gamma_multiply(0.7))
                                            .size(12.0),
                                    );

                                    ui.add_space(8.0);

                                    // Granularity
                                    ui.label(
                                        RichText::new(self.query_state.granularity.label())
                                            .color(text_color(self.theme).gamma_multiply(0.7))
                                            .size(12.0),
                                    );
                                });

                            ui.add_space(16.0);

                            // Aggregation keyboard hints
                            let hint_color = text_color(self.theme).gamma_multiply(0.35);
                            ui.label(RichText::new("^s").color(hint_color).size(10.0));
                            ui.label(RichText::new("sum").color(hint_color).size(10.0));
                            ui.add_space(4.0);
                            ui.label(RichText::new("^a").color(hint_color).size(10.0));
                            ui.label(RichText::new("avg").color(hint_color).size(10.0));
                            ui.add_space(4.0);
                            ui.label(RichText::new("^p").color(hint_color).size(10.0));
                            ui.label(RichText::new("p95").color(hint_color).size(10.0));
                            ui.add_space(4.0);
                            ui.label(RichText::new("</>").color(hint_color).size(10.0));
                            ui.label(RichText::new("granularity").color(hint_color).size(10.0));
                        });

                        ui.add_space(8.0);

                        // Separator
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );

                        ui.add_space(8.0);

                        // Footer with keyboard hints and save button
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);

                            let hint_color = text_color(self.theme).gamma_multiply(0.4);

                            // Keyboard hints
                            ui.label(RichText::new("⌘↵").color(hint_color).size(11.0));
                            ui.label(RichText::new("save").color(hint_color).size(11.0));
                            ui.add_space(16.0);
                            ui.label(RichText::new("esc").color(hint_color).size(11.0));
                            ui.label(RichText::new("cancel").color(hint_color).size(11.0));

                            // Right side - save button
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(16.0);

                                    let save_btn = egui::Button::new(
                                        RichText::new(format!(
                                            "{} Save",
                                            egui_phosphor::regular::FLOPPY_DISK
                                        ))
                                        .size(12.0),
                                    )
                                    .fill(accent_color);

                                    if ui.add(save_btn).clicked() {
                                        should_save = true;
                                    }

                                    // Cancel button
                                    let cancel_btn = egui::Button::new(
                                        RichText::new("Cancel")
                                            .size(12.0)
                                            .color(text_color(self.theme)),
                                    );

                                    if ui.add(cancel_btn).clicked() {
                                        should_close = true;
                                    }
                                },
                            );
                        });

                        ui.add_space(12.0);
                    });
            });

        // Handle save/close
        if should_save {
            let saved_query = self.query.clone();
            let saved_state = self.query_state.clone();
            self.close();
            result = BufferEditorResult::Saved(saved_query, saved_state);
        } else if should_close {
            self.close();
            result = BufferEditorResult::Cancelled;
        }

        result
    }
}
