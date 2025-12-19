use egui::{Color32, Key, RichText, TextFormat, text::LayoutJob};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::typography;

use super::finder_utils::OverlayStyle;

/// A command that can be executed from the command palette
#[derive(Debug, Clone)]
pub struct PaletteCommand {
    /// The command name (e.g., "theme", "export", "split")
    pub name: &'static str,
    /// Aliases for the command (e.g., "t" for "theme")
    pub aliases: &'static [&'static str],
    /// Description of what the command does
    pub description: &'static str,
    /// The kind of command (affects parsing and execution)
    pub kind: CommandKind,
}

/// The kind of command determines how arguments are parsed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// No arguments needed (e.g., `:q`, `:help`)
    NoArgs,
    /// Single argument (e.g., `:theme dark`)
    SingleArg,
    /// Multiple arguments (e.g., `:filter host=prod env=staging`)
    MultiArg,
}

/// Result of executing a command
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// Command executed successfully
    Success,
    /// Toggle theme
    ToggleTheme,
    /// Set specific theme
    SetTheme(AppTheme),
    /// Open the fuzzy finder
    OpenSearch,
    /// Show info overlay with build info
    ShowInfo,
    /// Show help
    ShowHelp,
    /// Horizontal split
    SplitHorizontal,
    /// Vertical split
    SplitVertical,
    /// Close current tab
    CloseTab,
    /// Quit the application
    QuitApp,
    /// Toggle zen mode (distraction-free view)
    ToggleZenMode,
    /// Toggle fullscreen for focused pane
    ToggleFullscreen,
    /// Show the landing page (home screen)
    ShowLandingPage,
    /// Take a screenshot of the window (optionally with a custom path)
    TakeScreenshot(Option<String>),
    /// Save workspace (:mksession [name])
    SaveWorkspace(Option<String>),
    /// Load workspace (:source <name>)
    LoadWorkspace(String),
    /// List available workspaces (:workspaces)
    ListWorkspaces,
    /// Share workspace as URL (:share)
    ShareWorkspace,
    /// Toggle commit markers visibility on charts
    ToggleCommits,
    /// Connect to Prometheus endpoint
    Connect(String),
    /// Disconnect from Prometheus (return to demo mode)
    Disconnect,
    /// Toggle diagnostics pane
    ToggleDiagnostics,
    /// Show diagnostics pane
    ShowDiagnostics,
    /// Hide diagnostics pane
    HideDiagnostics,
    /// Clear all diagnostics
    ClearDiagnostics,
    /// Jump to next diagnostic
    NextDiagnostic,
    /// Jump to previous diagnostic
    PrevDiagnostic,
    /// Create a new workspace tab
    NewWorkspaceTab(Option<String>),
    /// Close current workspace tab
    CloseWorkspaceTab,
    /// Go to next workspace tab
    NextWorkspaceTab,
    /// Go to previous workspace tab
    PrevWorkspaceTab,
    /// Error with message
    Error(String),
    /// No-op (command not recognized or cancelled)
    None,
}

/// Built-in commands
const COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        name: "theme",
        aliases: &["t"],
        description: "Toggle or set theme (dark/light)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "search",
        aliases: &["s"],
        description: "Open fuzzy finder search",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "info",
        aliases: &["version"],
        description: "Show version and build info",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "help",
        aliases: &["h"],
        description: "Show help and available commands",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "split",
        aliases: &["sp"],
        description: "Split view (horizontal/vertical/h/v)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "vsplit",
        aliases: &["vs"],
        description: "Vertical split",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "close",
        aliases: &["q"],
        description: "Close current tab",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "exit",
        aliases: &[],
        description: "Quit application",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "zen",
        aliases: &["z"],
        description: "Toggle zen mode",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "fullscreen",
        aliases: &["full"],
        description: "Toggle fullscreen for focused chart",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "home",
        aliases: &[],
        description: "Show the landing page",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "screenshot",
        aliases: &["ss"],
        description: "Take a screenshot",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "mksession",
        aliases: &["mks"],
        description: "Save workspace",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "source",
        aliases: &["so"],
        description: "Load workspace",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "workspaces",
        aliases: &["ws"],
        description: "List available workspaces",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "share",
        aliases: &[],
        description: "Share workspace as URL",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "commits",
        aliases: &["git"],
        description: "Toggle git commit markers",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "connect",
        aliases: &["c"],
        description: "Connect to Prometheus compatible endpoint (or 'disconnect')",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "diagnostics",
        aliases: &["diag"],
        description: "Toggle/show/hide/clear diagnostics",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "tabnew",
        aliases: &[],
        description: "Create new workspace tab",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "tabclose",
        aliases: &[],
        description: "Close current workspace tab",
        kind: CommandKind::NoArgs,
    },
];

/// A match result for command completion
#[derive(Debug, Clone)]
struct CommandMatch {
    command: &'static PaletteCommand,
    score: i64,
    match_positions: Vec<usize>,
}

/// A neovim-style command palette (triggered with `:`)
pub struct CommandPalette {
    /// Current input (without the leading `:`)
    input: String,
    /// Whether the palette is open
    is_open: bool,
    /// Current theme
    theme: AppTheme,
    /// Fuzzy matcher for command completion
    matcher: Matcher,
    /// Filtered command suggestions
    suggestions: Vec<CommandMatch>,
    /// Currently selected suggestion index
    selected_index: usize,
    /// Error message to display (clears on next input)
    error_message: Option<String>,
    /// Whether to move cursor to end on next render (for pre-filled text)
    cursor_to_end: bool,
    /// Whether to center the palette vertically (for landing page)
    centered: bool,
    /// Whether to request focus on next render (only once when opening)
    needs_focus: bool,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            is_open: false,
            theme: AppTheme::default(),
            matcher: Matcher::new(Config::DEFAULT),
            suggestions: Vec::new(),
            selected_index: 0,
            error_message: None,
            cursor_to_end: false,
            centered: false,
            needs_focus: false,
        }
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Check if the palette is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Open the command palette
    pub fn open(&mut self) {
        self.is_open = true;
        self.input.clear();
        self.error_message = None;
        self.selected_index = 0;
        self.centered = true; // Always center the palette
        self.needs_focus = true;
        self.refresh_suggestions();
    }

    /// Open the command palette with pre-filled text, centered on screen
    pub fn open_with_text(&mut self, text: &str) {
        self.is_open = true;
        self.input = text.to_string();
        self.error_message = None;
        self.selected_index = 0;
        self.cursor_to_end = true;
        self.centered = true;
        self.needs_focus = true;
        self.refresh_suggestions();
    }

    /// Close the command palette
    pub fn close(&mut self) {
        self.is_open = false;
        self.input.clear();
        self.error_message = None;
        self.selected_index = 0;
        self.suggestions.clear();
    }

    /// Refresh command suggestions based on current input
    fn refresh_suggestions(&mut self) {
        self.suggestions.clear();

        // Extract the command part (before any space/arguments)
        let cmd_part = self.input.split_whitespace().next().unwrap_or("");

        if cmd_part.is_empty() {
            // Show all commands sorted alphabetically
            for cmd in COMMANDS {
                self.suggestions.push(CommandMatch {
                    command: cmd,
                    score: 0,
                    match_positions: Vec::new(),
                });
            }
            self.suggestions
                .sort_by(|a, b| a.command.name.cmp(b.command.name));
        } else {
            // Create a pattern for fuzzy matching
            let pattern = Pattern::new(
                cmd_part,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );

            let mut indices: Vec<u32> = Vec::new();
            let mut buf = Vec::new();

            // Fuzzy match commands
            for cmd in COMMANDS {
                // Check main name
                indices.clear();
                let haystack = Utf32Str::new(cmd.name, &mut buf);
                if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                    self.suggestions.push(CommandMatch {
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
                        self.suggestions.push(CommandMatch {
                            command: cmd,
                            score: i64::from(score),
                            match_positions: indices.iter().map(|&i| i as usize).collect(),
                        });
                        break;
                    }
                }
            }
            // Sort by score (best first)
            self.suggestions.sort_by(|a, b| b.score.cmp(&a.score));
        }

        // Reset selection if out of bounds
        if self.selected_index >= self.suggestions.len() {
            self.selected_index = 0;
        }
    }

    /// Parse and execute the current command
    fn execute_command(&self) -> CommandResult {
        let input = self.input.trim();
        if input.is_empty() {
            return CommandResult::None;
        }

        let mut parts = input.split_whitespace();
        let cmd_name = parts.next().unwrap_or("");
        let args: Vec<&str> = parts.collect();

        // Find matching command
        let command = COMMANDS
            .iter()
            .find(|c| c.name == cmd_name || c.aliases.contains(&cmd_name));

        match command {
            Some(cmd) => self.execute(cmd, &args),
            None => CommandResult::Error(format!("Unknown command: {cmd_name}")),
        }
    }

    /// Execute a specific command with arguments
    fn execute(&self, cmd: &PaletteCommand, args: &[&str]) -> CommandResult {
        match cmd.name {
            "theme" => {
                if args.is_empty() {
                    CommandResult::ToggleTheme
                } else {
                    match args[0].to_lowercase().as_str() {
                        "dark" | "d" => CommandResult::SetTheme(AppTheme::Dark),
                        "light" | "l" => CommandResult::SetTheme(AppTheme::Light),
                        "toggle" | "t" => CommandResult::ToggleTheme,
                        _ => CommandResult::Error(format!(
                            "Unknown theme: {}. Use 'dark' or 'light'",
                            args[0]
                        )),
                    }
                }
            }
            "search" => CommandResult::OpenSearch,
            "info" => CommandResult::ShowInfo,
            "help" => CommandResult::ShowHelp,
            "split" => {
                if args.is_empty() {
                    CommandResult::SplitHorizontal
                } else {
                    match args[0].to_lowercase().as_str() {
                        "h" | "horizontal" => CommandResult::SplitHorizontal,
                        "v" | "vertical" => CommandResult::SplitVertical,
                        _ => CommandResult::Error(format!(
                            "Unknown split direction: {}. Use 'h' or 'v'",
                            args[0]
                        )),
                    }
                }
            }
            "vsplit" => CommandResult::SplitVertical,
            "close" => CommandResult::CloseTab,
            "exit" => CommandResult::QuitApp,
            "zen" => CommandResult::ToggleZenMode,
            "fullscreen" => CommandResult::ToggleFullscreen,
            "home" => CommandResult::ShowLandingPage,
            "screenshot" => {
                // Join all args as the path (handles paths with spaces)
                let path = if args.is_empty() {
                    None
                } else {
                    Some(args.join(" "))
                };
                CommandResult::TakeScreenshot(path)
            }
            "mksession" => {
                // :mksession or :mksession name - save workspace with optional name
                let name = if args.is_empty() {
                    None
                } else {
                    Some(args.join(" "))
                };
                CommandResult::SaveWorkspace(name)
            }
            "source" => {
                // :source name - load workspace by name
                if args.is_empty() {
                    CommandResult::Error("Usage: :source <workspace-name>".to_string())
                } else {
                    CommandResult::LoadWorkspace(args.join(" "))
                }
            }
            "workspaces" => CommandResult::ListWorkspaces,
            "share" => CommandResult::ShareWorkspace,
            "commits" => CommandResult::ToggleCommits,
            "connect" => {
                // :connect <url> - connect to Prometheus
                // :connect disconnect - return to demo mode
                if args.is_empty() {
                    CommandResult::Error("Usage: :connect <url> or :connect disconnect".to_string())
                } else if args[0].to_lowercase() == "disconnect" {
                    CommandResult::Disconnect
                } else {
                    CommandResult::Connect(args.join(" "))
                }
            }
            "diagnostics" => {
                if args.is_empty() {
                    // :diag with no args - toggle
                    CommandResult::ToggleDiagnostics
                } else {
                    match args[0].to_lowercase().as_str() {
                        "show" | "open" => CommandResult::ShowDiagnostics,
                        "hide" | "close" => CommandResult::HideDiagnostics,
                        "clear" | "reset" => CommandResult::ClearDiagnostics,
                        "toggle" | "t" => CommandResult::ToggleDiagnostics,
                        "next" | "n" => CommandResult::NextDiagnostic,
                        "prev" | "previous" | "p" => CommandResult::PrevDiagnostic,
                        _ => CommandResult::Error(format!(
                            "Unknown diagnostics subcommand: {}. Use show/hide/clear/toggle/next/prev",
                            args[0]
                        )),
                    }
                }
            }
            "tabnew" => {
                // :tabnew or :tabnew name - create new workspace tab with optional name
                let name = if args.is_empty() {
                    None
                } else {
                    Some(args.join(" "))
                };
                CommandResult::NewWorkspaceTab(name)
            }
            "tabclose" => CommandResult::CloseWorkspaceTab,
            _ => CommandResult::None,
        }
    }

    /// Show the command palette. Returns a CommandResult if a command was executed.
    pub fn show(&mut self, ctx: &egui::Context) -> CommandResult {
        if !self.is_open {
            return CommandResult::None;
        }

        let mut result = CommandResult::None;
        let mut should_close = false;

        // Handle keyboard input
        let (navigate_up, navigate_down, confirm, escape, tab) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowUp) || (i.key_pressed(Key::K) && i.modifiers.ctrl),
                i.key_pressed(Key::ArrowDown) || (i.key_pressed(Key::J) && i.modifiers.ctrl),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Tab),
            )
        });

        if escape {
            should_close = true;
        }

        if navigate_up && self.selected_index > 0 {
            self.selected_index -= 1;
        }

        if navigate_down && self.selected_index + 1 < self.suggestions.len() {
            self.selected_index += 1;
        }

        // Tab completion - insert the selected command name
        if tab && !self.suggestions.is_empty() {
            let cmd = self.suggestions[self.selected_index].command;
            self.input = format!("{} ", cmd.name);
            self.refresh_suggestions();
        }

        if confirm {
            result = self.execute_command();
            should_close = true;
        }

        // Render the palette
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.5).clamp(350.0, 600.0);

        // Position: centered vertically when opened from landing page, otherwise near top
        let (anchor, offset) = if self.centered {
            (egui::Align2::CENTER_CENTER, [0.0, -50.0])
        } else {
            (egui::Align2::CENTER_TOP, [0.0, 80.0])
        };

        egui::Area::new(egui::Id::new("command_palette"))
            .anchor(anchor, offset)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);

                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Input section with `:` prefix
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(":")
                                .color(text_color(self.theme))
                                .size(typography::HEADING)
                                .strong(),
                        );

                        let text_edit = egui::TextEdit::singleline(&mut self.input)
                            .font(typography::heading())
                            .hint_text(
                                RichText::new("Type a command...")
                                    .color(text_color(self.theme).gamma_multiply(0.4)),
                            )
                            .frame(false)
                            .desired_width(popup_width - 50.0);

                        let response = ui.add(text_edit);

                        // Only request focus once when opening (not every frame)
                        if self.needs_focus {
                            response.request_focus();
                            self.needs_focus = false;
                        }

                        // Move cursor to end if we pre-filled text
                        if self.cursor_to_end {
                            if let Some(mut state) =
                                egui::TextEdit::load_state(ui.ctx(), response.id)
                            {
                                let ccursor = egui::text::CCursor::new(self.input.chars().count());
                                state
                                    .cursor
                                    .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                                state.store(ui.ctx(), response.id);
                            }
                            self.cursor_to_end = false;
                        }

                        if response.changed() {
                            self.error_message = None;
                            self.refresh_suggestions();
                        }
                    });

                    ui.add_space(8.0);

                    // Separator
                    let separator_color = match self.theme {
                        AppTheme::Light => palette::light_border::SUBTLE,
                        AppTheme::Dark => palette::border::SUBTLE,
                    };
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(4.0);

                    // Error message if any
                    if let Some(ref error) = self.error_message {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    semantic_icons::status::WARNING,
                                    error
                                ))
                                .color(Color32::from_rgb(220, 80, 80))
                                .size(typography::LG),
                            );
                        });
                        ui.add_space(4.0);
                    }

                    // Suggestions
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            if self.suggestions.is_empty() {
                                ui.add_space(12.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new("No matching commands")
                                            .color(text_color(self.theme).gamma_multiply(0.5))
                                            .size(typography::XL),
                                    );
                                });
                                ui.add_space(12.0);
                            } else {
                                for (i, suggestion) in self.suggestions.iter().enumerate() {
                                    let is_selected = i == self.selected_index;
                                    self.render_suggestion_row(ui, suggestion, is_selected);
                                }
                            }
                        });

                    ui.add_space(4.0);

                    // Footer with hints
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let hint_color = text_color(self.theme).gamma_multiply(0.4);
                        ui.label(RichText::new("↑↓").color(hint_color).size(typography::SM));
                        ui.label(
                            RichText::new("navigate")
                                .color(hint_color)
                                .size(typography::SM),
                        );
                        ui.add_space(12.0);
                        ui.label(RichText::new("Tab").color(hint_color).size(typography::SM));
                        ui.label(
                            RichText::new("complete")
                                .color(hint_color)
                                .size(typography::SM),
                        );
                        ui.add_space(12.0);
                        ui.label(RichText::new("↵").color(hint_color).size(typography::SM));
                        ui.label(
                            RichText::new("execute")
                                .color(hint_color)
                                .size(typography::SM),
                        );
                        ui.add_space(12.0);
                        ui.label(RichText::new("esc").color(hint_color).size(typography::SM));
                        ui.label(
                            RichText::new("close")
                                .color(hint_color)
                                .size(typography::SM),
                        );
                    });
                    ui.add_space(8.0);
                });
            });

        if should_close {
            self.close();
        }

        result
    }

    /// Render a single suggestion row
    fn render_suggestion_row(
        &self,
        ui: &mut egui::Ui,
        suggestion: &CommandMatch,
        is_selected: bool,
    ) {
        let text_col = text_color(self.theme);
        // Use emerald accent for highlights to match brand
        let highlight_color = match self.theme {
            AppTheme::Light => palette::accent::LIGHT,
            AppTheme::Dark => palette::accent::HOVER, // Bright emerald
        };
        let selected_bg = match self.theme {
            AppTheme::Light => palette::light_bg::ELEVATED,
            AppTheme::Dark => palette::accent::MUTED, // Emerald-tinted selection
        };
        let hover_bg = match self.theme {
            AppTheme::Light => palette::light_bg::HOVER,
            AppTheme::Dark => palette::bg::HOVER,
        };

        let row_height = 32.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_height),
            egui::Sense::click(),
        );

        // Background
        let bg_color = if is_selected {
            selected_bg
        } else if response.hovered() {
            hover_bg
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 0.0, bg_color);
        }

        // Selection indicator
        if is_selected {
            let indicator_rect = egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height));
            ui.painter()
                .rect_filled(indicator_rect, 0.0, highlight_color);
        }

        // Content
        let content_rect = rect.shrink2(egui::vec2(16.0, 0.0));
        let mut cursor_x = content_rect.left();

        // Command name with highlights
        let name_galley = self.create_highlighted_text(
            ui,
            suggestion.command.name,
            &suggestion.match_positions,
            text_col,
            highlight_color,
        );
        ui.painter().galley(
            egui::pos2(
                cursor_x,
                content_rect.center().y - name_galley.size().y / 2.0,
            ),
            name_galley.clone(),
            text_col,
        );
        cursor_x += name_galley.size().x + 12.0;

        // Aliases (dimmed)
        if !suggestion.command.aliases.is_empty() {
            let aliases_text = format!("({})", suggestion.command.aliases.join(", "));
            let aliases_galley = ui.painter().layout_no_wrap(
                aliases_text,
                typography::proportional(typography::MD),
                text_col.gamma_multiply(0.4),
            );
            ui.painter().galley(
                egui::pos2(
                    cursor_x,
                    content_rect.center().y - aliases_galley.size().y / 2.0,
                ),
                aliases_galley.clone(),
                text_col.gamma_multiply(0.4),
            );
            cursor_x += aliases_galley.size().x + 12.0;
        }

        // Description (on the right, dimmed)
        let desc_galley = ui.painter().layout_no_wrap(
            suggestion.command.description.to_string(),
            typography::proportional(typography::MD),
            text_col.gamma_multiply(0.5),
        );
        let desc_x = content_rect.right() - desc_galley.size().x - 8.0;
        if desc_x > cursor_x {
            ui.painter().galley(
                egui::pos2(desc_x, content_rect.center().y - desc_galley.size().y / 2.0),
                desc_galley,
                text_col.gamma_multiply(0.5),
            );
        }

        // Scroll into view
        if is_selected {
            response.scroll_to_me(Some(egui::Align::Center));
        }
    }

    /// Create text with highlighted match positions
    fn create_highlighted_text(
        &self,
        ui: &egui::Ui,
        text: &str,
        positions: &[usize],
        normal_color: Color32,
        highlight_color: Color32,
    ) -> std::sync::Arc<egui::Galley> {
        let mut job = LayoutJob::default();
        let font_id = typography::proportional(typography::XL);

        for (i, ch) in text.chars().enumerate() {
            let color = if positions.contains(&i) {
                highlight_color
            } else {
                normal_color
            };

            let format = TextFormat {
                font_id: font_id.clone(),
                color,
                ..Default::default()
            };

            job.append(&ch.to_string(), 0.0, format);
        }

        ui.fonts_mut(|f| f.layout_job(job))
    }
}
