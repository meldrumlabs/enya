//! Slash Commands - Agent mode command suggestions triggered by `/`.
//!
//! This module provides command suggestions for the observability agent, inspired by
//! tools like Claude Code, Codex, and opencode. Commands are triggered by typing `/`
//! in agent mode and provide structured prefixes for investigation, exploration, and
//! analysis workflows.
//!
//! # Available Commands
//!
//! - `/investigate` - Deep-dive analysis with correlations and anomaly detection
//! - `/diff` - Compare metric states between two time ranges
//! - `/blame` - Trace metric changes back to commits, deploys, or config changes
//!
//! # How It Works
//!
//! Unlike a command palette, slash commands work like the `@` mention system:
//! 1. User types `/` in the input bar
//! 2. A popup appears with fuzzy-matched command suggestions
//! 3. User selects a command with Enter/Tab or clicks
//! 4. The command is inserted into the input bar (e.g., `/investigate `)
//! 5. User continues typing (e.g., `/investigate @http_requests_total why is it spiking?`)
//! 6. When Enter is pressed, the full input is sent as a prompt to the agent

use egui::{Color32, Key, RichText};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::super::util::finder_utils::OverlayStyle;

// =============================================================================
// Slash Command Definition
// =============================================================================

/// A slash command that can be used in agent mode.
#[derive(Debug, Clone, Copy)]
pub struct SlashCommand {
    /// The command name (without the leading `/`)
    pub name: &'static str,
    /// Short aliases for the command
    pub aliases: &'static [&'static str],
    /// Brief description shown in the popup
    pub description: &'static str,
    /// Icon for the command (semantic icon)
    pub icon: &'static str,
    /// Category for grouping in the UI
    pub category: SlashCommandCategory,
}

/// The kind of slash command determines argument requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommandKind {
    /// No arguments needed
    NoArgs,
    /// Requires a metric name (supports @ autocomplete)
    MetricArg,
    /// Requires two time range arguments
    TimeRangeArgs,
    /// Requires a search pattern
    PatternArg,
    /// Freeform text argument
    TextArg,
}

/// Category for grouping commands in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommandCategory {
    /// Investigation and incident response
    Investigation,
    /// Exploration and discovery
    Discovery,
    /// Query building and PromQL
    Query,
    /// Alert and SLO management
    Alerts,
    /// Context and session management
    Context,
}

impl SlashCommandCategory {
    /// Get the display name for this category.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Investigation => "Investigation",
            Self::Discovery => "Discovery",
            Self::Query => "Query",
            Self::Alerts => "Alerts",
            Self::Context => "Context",
        }
    }
}

// =============================================================================
// Command Registry
// =============================================================================

/// Built-in slash commands.
pub const SLASH_COMMANDS: &[SlashCommand] = &[
    // Investigation - the core use case
    SlashCommand {
        name: "investigate",
        aliases: &["inv", "dig"],
        description: "Deep-dive analysis with correlations and anomalies",
        icon: semantic_icons::action::SEARCH,
        category: SlashCommandCategory::Investigation,
    },
    SlashCommand {
        name: "diff",
        aliases: &["compare", "cmp"],
        description: "Compare metric states between two time ranges",
        icon: semantic_icons::action::SPLIT,
        category: SlashCommandCategory::Investigation,
    },
    // Query - natural language to PromQL
    SlashCommand {
        name: "query",
        aliases: &["q", "promql"],
        description: "Generate PromQL from natural language",
        icon: semantic_icons::file::CODE,
        category: SlashCommandCategory::Query,
    },
    // Explain - understand what you're looking at
    SlashCommand {
        name: "explain",
        aliases: &["exp", "what"],
        description: "Explain what the current query or chart shows",
        icon: semantic_icons::action::HELP,
        category: SlashCommandCategory::Query,
    },
];

// =============================================================================
// Slash Command Result
// =============================================================================

/// Result of the slash command popup interaction.
#[derive(Debug, Clone, Default)]
pub enum SlashCommandResult {
    /// No action (popup still open)
    #[default]
    None,
    /// Command was selected - returns the command text to insert (e.g., "/investigate ")
    Selected(String),
    /// Popup was closed without selection
    Cancelled,
}

// =============================================================================
// Slash Command Popup (similar to MentionPopup)
// =============================================================================

/// A match result for command completion.
struct CommandMatch {
    command: &'static SlashCommand,
    score: i64,
    match_positions: Vec<usize>,
}

/// Slash command popup state - works like the @ mention popup.
#[derive(Default)]
pub struct SlashCommandPopup {
    /// Whether the popup is visible
    pub active: bool,
    /// The search query (text after /)
    query: String,
    /// Position in input where / was typed
    slash_position: usize,
    /// Filtered results with scores
    results: Vec<CommandMatch>,
    /// Selected index in results
    selected_index: usize,
    /// Fuzzy matcher
    matcher: Matcher,
    /// Current theme
    theme: AppTheme,
}

impl SlashCommandPopup {
    /// Create a new slash command popup.
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
            ..Default::default()
        }
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set available metrics (not used for slash commands but kept for API compatibility).
    pub fn set_available_metrics(&mut self, _metrics: Vec<String>) {
        // Not used for slash commands
    }

    /// Check if the popup is open.
    pub fn is_open(&self) -> bool {
        self.active
    }

    /// Start the slash command popup at the given cursor position.
    pub fn start(&mut self, slash_position: usize) {
        self.active = true;
        self.slash_position = slash_position;
        self.query.clear();
        self.selected_index = 0;
        self.refresh_results();
    }

    /// Close the popup.
    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.selected_index = 0;
        self.results.clear();
    }

    /// Explicit open method (kept for API compatibility).
    pub fn open(&mut self) {
        self.start(0);
    }

    /// Update the search query.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.refresh_results();
    }

    /// Refresh filtered results based on query.
    fn refresh_results(&mut self) {
        self.results.clear();

        if self.query.is_empty() {
            // Show all commands when query is empty
            for cmd in SLASH_COMMANDS {
                self.results.push(CommandMatch {
                    command: cmd,
                    score: 0,
                    match_positions: Vec::new(),
                });
            }
        } else {
            // Fuzzy match commands
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );

            let mut indices: Vec<u32> = Vec::new();
            let mut buf = Vec::new();

            for cmd in SLASH_COMMANDS {
                // Check main name
                indices.clear();
                let haystack = Utf32Str::new(cmd.name, &mut buf);
                if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                    self.results.push(CommandMatch {
                        command: cmd,
                        score: i64::from(score),
                        match_positions: indices.iter().map(|&i| i as usize).collect(),
                    });
                    continue;
                }

                // Check aliases
                for alias in cmd.aliases {
                    indices.clear();
                    let haystack = Utf32Str::new(alias, &mut buf);
                    if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices)
                    {
                        self.results.push(CommandMatch {
                            command: cmd,
                            score: i64::from(score),
                            match_positions: Vec::new(), // Don't highlight alias matches
                        });
                        break;
                    }
                }
            }

            // Sort by score (best first)
            self.results.sort_by(|a, b| b.score.cmp(&a.score));
        }

        // Reset selection if out of bounds
        if self.selected_index >= self.results.len() {
            self.selected_index = 0;
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if self.selected_index + 1 < self.results.len() {
            self.selected_index += 1;
        }
    }

    /// Get the currently selected command.
    pub fn selected(&self) -> Option<&'static SlashCommand> {
        self.results.get(self.selected_index).map(|m| m.command)
    }

    /// Get the slash position.
    pub fn get_slash_position(&self) -> usize {
        self.slash_position
    }

    /// Show the slash command popup (renders above the input bar like @ mentions).
    /// cursor_x is the optional X position of the / character for popup alignment.
    #[profiling::function]
    pub fn show(&self, ui: &mut egui::Ui, input_rect: egui::Rect, cursor_x: Option<f32>) {
        if !self.active || self.results.is_empty() {
            return;
        }

        let text_col = crate::ui::colors::text_color(self.theme);
        let popup_width = 480.0;
        let row_height = 32.0;
        let header_height = 32.0;
        let footer_height = 28.0;
        let results_height = (self.results.len() as f32 * row_height).min(320.0);
        let popup_height = header_height + results_height + footer_height;

        // Position popup above cursor if available, otherwise center above input
        let popup_x = if let Some(cx) = cursor_x {
            // Align popup left edge with cursor, but clamp to screen
            cx.max(8.0).min(input_rect.right() - popup_width)
        } else {
            (input_rect.center().x - popup_width / 2.0).max(8.0)
        };

        // Position popup well above the input bar (24px gap) so it doesn't obscure text
        let ideal_y = input_rect.top() - popup_height - 24.0;
        let popup_y = ideal_y.max(8.0);

        let popup_pos = egui::pos2(popup_x, popup_y);

        // Premium Obsidian Glass styling
        let style = OverlayStyle::frosted_glass(self.theme);

        // Accent color
        let accent = self.theme.accent_primary();

        // Separator color
        let separator_color = self.theme.border_subtle();

        // Muted text
        let muted_text = text_col.gamma_multiply(0.6);
        let faint_text = text_col.gamma_multiply(0.4);

        // Use Area to render as a floating overlay
        egui::Area::new(egui::Id::new("slash_command_popup"))
            .fixed_pos(popup_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                let frame_response = style
                    .frame()
                    .inner_margin(egui::Margin::symmetric(0, 6))
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Header with slash icon
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new("/")
                                    .color(accent)
                                    .size(typography::LG)
                                    .strong(),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Select command")
                                    .color(muted_text)
                                    .size(typography::SM),
                            );
                        });

                        ui.add_space(4.0);

                        // Separator
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(2.0);

                        // Results list
                        egui::ScrollArea::vertical()
                            .max_height(results_height)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                for (i, result) in self.results.iter().enumerate() {
                                    let cmd = result.command;
                                    let is_selected = i == self.selected_index;

                                    // Row
                                    let (row_rect, response) = ui.allocate_exact_size(
                                        egui::vec2(popup_width, row_height),
                                        egui::Sense::click(),
                                    );
                                    let is_hovered = response.hovered();

                                    // Background
                                    let bg_color = if is_selected {
                                        accent.gamma_multiply(0.15)
                                    } else if is_hovered {
                                        text_col.gamma_multiply(0.05)
                                    } else {
                                        Color32::TRANSPARENT
                                    };

                                    if bg_color != Color32::TRANSPARENT {
                                        ui.painter().rect_filled(row_rect, 4.0, bg_color);
                                    }

                                    // Icon
                                    let icon_color = if is_selected || is_hovered {
                                        accent
                                    } else {
                                        muted_text
                                    };
                                    ui.painter().text(
                                        row_rect.left_center() + egui::vec2(16.0, 0.0),
                                        egui::Align2::LEFT_CENTER,
                                        cmd.icon,
                                        typography::proportional(typography::MD),
                                        icon_color,
                                    );

                                    // Command name with highlights
                                    let name_pos = row_rect.left_center() + egui::vec2(40.0, 0.0);
                                    if result.match_positions.is_empty() {
                                        ui.painter().text(
                                            name_pos,
                                            egui::Align2::LEFT_CENTER,
                                            cmd.name,
                                            typography::proportional(typography::MD),
                                            text_col,
                                        );
                                    } else {
                                        // Build highlighted text
                                        let mut job = egui::text::LayoutJob::default();
                                        for (idx, c) in cmd.name.chars().enumerate() {
                                            let color = if result.match_positions.contains(&idx) {
                                                accent
                                            } else {
                                                text_col
                                            };
                                            job.append(
                                                &c.to_string(),
                                                0.0,
                                                egui::TextFormat {
                                                    font_id: typography::proportional(
                                                        typography::MD,
                                                    ),
                                                    color,
                                                    ..Default::default()
                                                },
                                            );
                                        }
                                        let galley = ui.fonts_mut(|f| f.layout_job(job));
                                        ui.painter().galley(
                                            egui::pos2(
                                                name_pos.x,
                                                name_pos.y - galley.size().y / 2.0,
                                            ),
                                            galley,
                                            text_col,
                                        );
                                    }

                                    // Description (right side)
                                    let desc_galley = ui.painter().layout_no_wrap(
                                        cmd.description.to_string(),
                                        typography::proportional(typography::SM),
                                        faint_text,
                                    );
                                    let desc_x = row_rect.right() - desc_galley.size().x - 12.0;
                                    if desc_x > name_pos.x + 100.0 {
                                        ui.painter().galley(
                                            egui::pos2(
                                                desc_x,
                                                row_rect.center().y - desc_galley.size().y / 2.0,
                                            ),
                                            desc_galley,
                                            faint_text,
                                        );
                                    }

                                    // Scroll selected into view
                                    if is_selected {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                }
                            });

                        ui.add_space(2.0);

                        // Footer separator
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(4.0);

                        // Footer with hints
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(RichText::new("↑↓").color(accent).size(typography::XS));
                            ui.label(
                                RichText::new("navigate")
                                    .color(faint_text)
                                    .size(typography::XS),
                            );
                            ui.add_space(8.0);
                            ui.label(RichText::new("⏎/Tab").color(accent).size(typography::XS));
                            ui.label(
                                RichText::new("select")
                                    .color(faint_text)
                                    .size(typography::XS),
                            );
                            ui.add_space(8.0);
                            ui.label(RichText::new("Esc").color(accent).size(typography::XS));
                            ui.label(
                                RichText::new("close")
                                    .color(faint_text)
                                    .size(typography::XS),
                            );
                        });
                    });

                // Draw inner highlight for glass effect
                let rect = frame_response.response.rect;
                if let Some(highlight_color) = style.inner_highlight() {
                    let highlight_rect = egui::Rect::from_min_size(
                        rect.left_top() + egui::vec2(1.0, 1.0),
                        egui::vec2(rect.width() - 2.0, 1.0),
                    );
                    ui.painter().rect_filled(
                        highlight_rect,
                        style.corner_radius - 1.0,
                        highlight_color,
                    );
                }
            });
    }

    /// Handle keyboard input. Returns true if input was handled.
    pub fn handle_keyboard(&mut self, ctx: &egui::Context) -> Option<SlashCommandResult> {
        if !self.active {
            return None;
        }

        let mut result = None;

        ctx.input_mut(|input| {
            // Navigate up
            if input.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                || input.consume_key(egui::Modifiers::CTRL, Key::K)
                || input.consume_key(egui::Modifiers::CTRL, Key::P)
            {
                self.select_prev();
            }
            // Navigate down
            else if input.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                || input.consume_key(egui::Modifiers::CTRL, Key::J)
                || input.consume_key(egui::Modifiers::CTRL, Key::N)
            {
                self.select_next();
            }
            // Select with Enter or Tab
            else if input.consume_key(egui::Modifiers::NONE, Key::Enter)
                || input.consume_key(egui::Modifiers::NONE, Key::Tab)
            {
                if let Some(cmd) = self.selected() {
                    // Return the command text to insert (e.g., "/investigate ")
                    let cmd_text = format!("/{} ", cmd.name);
                    result = Some(SlashCommandResult::Selected(cmd_text));
                }
                self.close();
            }
            // Cancel with Escape
            else if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                self.close();
                result = Some(SlashCommandResult::Cancelled);
            }
        });

        result
    }
}
