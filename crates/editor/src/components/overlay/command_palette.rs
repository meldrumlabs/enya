use egui::{Color32, Key, RichText, TextFormat, text::LayoutJob};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;

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
    /// Open style picker (unified theme + font picker)
    OpenStylePicker,
    /// Show info overlay with build info
    ShowInfo,
    /// Horizontal split
    SplitHorizontal,
    /// Vertical split
    SplitVertical,
    /// Quit the workspace
    QuitWorkspace,
    /// Write/save the current workspace
    WriteWorkspace,
    /// Take a screenshot of the window (optionally with a custom path)
    TakeScreenshot(Option<String>),
    /// Load workspace (:source <name>)
    LoadWorkspace(String),
    /// Share workspace as URL (:share)
    ShareWorkspace,
    /// Set AI provider (claude, codex)
    SetProvider(String),
    /// Set auto-refresh interval (off/10s/30s/1m/5m/15m)
    SetRefresh(String),
    /// Toggle team demo mode
    TeamDemo,
    /// Connect to team server with URL and token
    TeamConnect { url: String, token: String },
    /// Disconnect from team server
    TeamDisconnect,
    /// Open a terminal pane (native only)
    OpenTerminal,
    /// Open a tracing pane (optionally with a trace ID)
    OpenTracing(Option<String>),
    /// Open a SQL pane (native only)
    OpenSql,
    /// Error with message
    Error(String),
    /// Open logs pane (demo mode)
    OpenLogs,
    /// Open logs pane connected to Loki
    OpenLoki(String),
    /// Float the focused pane (detach to floating window)
    FloatPane,
    /// Dock all floating panes back to tile layout
    DockAllPanes,
    /// Auto-arrange all floating panes in a grid
    ArrangeFloatingPanes,
    /// No-op (command not recognized or cancelled)
    None,
}

/// Built-in commands (always available)
const BASE_COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        name: "style",
        aliases: &["st", "theme", "t"],
        description: "Open style picker (theme + font)",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "info",
        aliases: &["version"],
        description: "Show version and build info",
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
        name: "quit",
        aliases: &["q"],
        description: "Quit workspace",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "write",
        aliases: &["w"],
        description: "Save workspace",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "screenshot",
        aliases: &["ss"],
        description: "Take a screenshot",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "source",
        aliases: &["so"],
        description: "Load workspace",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "share",
        aliases: &[],
        description: "Share workspace as URL",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "provider",
        aliases: &["ai"],
        description: "Set AI provider (claude, codex)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "refresh",
        aliases: &["r"],
        description: "Set auto-refresh interval (off/10s/30s/1m/5m/15m)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "team",
        aliases: &[],
        description: "Team (demo | connect <url> <token> | disconnect)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "trace",
        aliases: &["tr", "tracing"],
        description: "Open a tracing pane (optionally with trace ID)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "logs",
        aliases: &["log"],
        description: "Open logs pane (demo mode)",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "loki",
        aliases: &[],
        description: "Connect to Loki server (e.g., :loki localhost:3100)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "float",
        aliases: &["fl"],
        description: "Float focused pane (detach to floating window)",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "dock",
        aliases: &["dk"],
        description: "Dock all floating panes back to tile layout",
        kind: CommandKind::NoArgs,
    },
];

/// Terminal command (requires terminal feature on native)
#[cfg(all(not(target_arch = "wasm32"), feature = "terminal"))]
const TERMINAL_COMMAND: PaletteCommand = PaletteCommand {
    name: "terminal",
    aliases: &["term"],
    description: "Open a terminal pane",
    kind: CommandKind::NoArgs,
};

/// SQL command (requires sql feature on native)
#[cfg(all(not(target_arch = "wasm32"), feature = "sql"))]
const SQL_COMMAND: PaletteCommand = PaletteCommand {
    name: "sql",
    aliases: &["datafusion"],
    description: "Open a SQL pane (DataFusion)",
    kind: CommandKind::NoArgs,
};

/// Returns all available commands based on enabled features.
fn available_commands() -> Vec<&'static PaletteCommand> {
    #[allow(unused_mut)] // mut needed when terminal or sql features enabled
    let mut commands: Vec<&'static PaletteCommand> = BASE_COMMANDS.iter().collect();

    #[cfg(all(not(target_arch = "wasm32"), feature = "terminal"))]
    commands.push(&TERMINAL_COMMAND);

    #[cfg(all(not(target_arch = "wasm32"), feature = "sql"))]
    commands.push(&SQL_COMMAND);

    commands
}

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

        let commands = available_commands();

        if cmd_part.is_empty() {
            // Show all commands sorted alphabetically
            for cmd in &commands {
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
            for cmd in &commands {
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
        let commands = available_commands();
        let command = commands
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
            "style" => CommandResult::OpenStylePicker,
            "info" => CommandResult::ShowInfo,
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
            "quit" => CommandResult::QuitWorkspace,
            "write" => CommandResult::WriteWorkspace,
            "screenshot" => {
                // Join all args as the path (handles paths with spaces)
                let path = if args.is_empty() {
                    None
                } else {
                    Some(args.join(" "))
                };
                CommandResult::TakeScreenshot(path)
            }
            "source" => {
                // :source name - load workspace by name
                if args.is_empty() {
                    CommandResult::Error("Usage: :source <workspace-name>".to_string())
                } else {
                    CommandResult::LoadWorkspace(args.join(" "))
                }
            }
            "share" => CommandResult::ShareWorkspace,
            "provider" | "ai" => {
                // :provider <name> - set AI provider
                if args.is_empty() {
                    CommandResult::Error("Usage: :provider <claude|codex>".to_string())
                } else {
                    CommandResult::SetProvider(args[0].to_lowercase())
                }
            }
            "refresh" => {
                // :refresh <interval> - set auto-refresh interval
                if args.is_empty() {
                    // No argument: disable refresh
                    CommandResult::SetRefresh("off".to_string())
                } else {
                    CommandResult::SetRefresh(args[0].to_lowercase())
                }
            }
            "team" => {
                // :team demo - toggle team demo mode
                // :team connect <url> <token> - connect to server
                // :team disconnect - disconnect from server
                if args.is_empty() {
                    CommandResult::Error(
                        "Usage: :team demo | :team connect <url> <token> | :team disconnect"
                            .to_string(),
                    )
                } else {
                    match args[0].to_lowercase().as_str() {
                        "demo" => CommandResult::TeamDemo,
                        "connect" => {
                            if args.len() < 3 {
                                CommandResult::Error(
                                    "Usage: :team connect <url> <token>".to_string(),
                                )
                            } else {
                                CommandResult::TeamConnect {
                                    url: args[1].to_string(),
                                    token: args[2].to_string(),
                                }
                            }
                        }
                        "disconnect" => CommandResult::TeamDisconnect,
                        _ => CommandResult::Error(format!(
                            "Unknown team command: {}. Use 'demo', 'connect', or 'disconnect'",
                            args[0]
                        )),
                    }
                }
            }
            "terminal" => CommandResult::OpenTerminal,
            "trace" | "tr" | "tracing" => {
                // Optional trace ID argument
                let trace_id = args.first().map(|s| s.to_string());
                CommandResult::OpenTracing(trace_id)
            }
            "sql" | "datafusion" => CommandResult::OpenSql,
            "logs" | "log" => CommandResult::OpenLogs,
            "loki" => {
                if args.is_empty() {
                    CommandResult::Error(
                        "Usage: :loki <url> (e.g., :loki localhost:3100)".to_string(),
                    )
                } else {
                    CommandResult::OpenLoki(args[0].to_string())
                }
            }
            "float" | "fl" => {
                if !args.is_empty() && (args[0] == "arrange" || args[0] == "a") {
                    CommandResult::ArrangeFloatingPanes
                } else {
                    CommandResult::FloatPane
                }
            }
            "dock" | "dk" => CommandResult::DockAllPanes,
            _ => CommandResult::None,
        }
    }

    /// Show the command palette. Returns a CommandResult if a command was executed.
    #[profiling::function]
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
            // Move cursor to end after tab completion
            self.cursor_to_end = true;
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
            .order(egui::Order::Tooltip)
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
                            .desired_width(popup_width - 50.0)
                            .lock_focus(true); // Prevent Tab from navigating away

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
                    let separator_color = self.theme.border_subtle();
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
        let accent_col = self.theme.accent_primary();

        let row_height = 32.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_height),
            egui::Sense::click(),
        );

        let is_hovered = response.hovered();

        // Background - use subtle hover style like landing page
        let bg_color = if is_selected {
            accent_col.gamma_multiply(0.12)
        } else if is_hovered {
            text_col.gamma_multiply(0.05)
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 0.0, bg_color);
        }

        // Selection indicator
        if is_selected {
            let indicator_rect = egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height));
            ui.painter().rect_filled(indicator_rect, 0.0, accent_col);
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
            accent_col,
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
