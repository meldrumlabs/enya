use egui::{Color32, Key, RichText, TextFormat, text::LayoutJob};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;
use crate::components::util::{ScrollShadowConfig, ScrollState, render_scroll_shadows};

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

/// A dynamic command (e.g., from plugins) that owns its strings
#[derive(Debug, Clone)]
pub struct DynamicCommand {
    /// The command name
    pub name: String,
    /// Aliases for the command
    pub aliases: Vec<String>,
    /// Description of what the command does
    pub description: String,
    /// Whether the command accepts arguments
    pub accepts_args: bool,
    /// Source of the command (e.g., plugin name)
    pub source: String,
}

impl DynamicCommand {
    /// Convert to a PaletteCommand by leaking strings for static lifetime.
    /// This is acceptable because plugins are loaded once at startup.
    fn to_palette_command(&self) -> PaletteCommand {
        // Leak strings to get 'static lifetime
        let name: &'static str = Box::leak(self.name.clone().into_boxed_str());
        let description: &'static str =
            Box::leak(format!("{} [{}]", self.description, self.source).into_boxed_str());
        let aliases: &'static [&'static str] = Box::leak(
            self.aliases
                .iter()
                .map(|a| Box::leak(a.clone().into_boxed_str()) as &'static str)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );

        PaletteCommand {
            name,
            aliases,
            description,
            kind: if self.accepts_args {
                CommandKind::MultiArg
            } else {
                CommandKind::NoArgs
            },
        }
    }
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
    /// Share workspace as URL — snapshot if data loaded, config-only otherwise (:share)
    ShareWorkspace,
    /// Upload snapshot to blob server with conversation data (:snapshot [title])
    UploadSnapshot(Option<String>),
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
    /// Float the focused pane (detach to floating window)
    FloatPane,
    /// Dock all floating panes back to tile layout
    DockAllPanes,
    /// Auto-arrange all floating panes in a grid
    ArrangeFloatingPanes,
    /// Sync git repository and re-index codebase (native only)
    SyncCodebase,
    /// Open the tutorial overlay
    OpenTutorial,
    /// Open the settings overlay
    OpenSettings,
    /// Try to execute a plugin command (command name, args)
    PluginCommand(String, String),
    /// No-op (command not recognized or cancelled)
    None,
}

/// Built-in commands (always available)
const BASE_COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        name: "version",
        aliases: &[],
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
        name: "share",
        aliases: &[],
        description: "Share workspace as URL (snapshot if data loaded)",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "snapshot",
        aliases: &[],
        description: "Upload snapshot to blob server (with conversation)",
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
    PaletteCommand {
        name: "tutorial",
        aliases: &["tut"],
        description: "Open the interactive tutorial",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "settings",
        aliases: &["set", "prefs", "preferences"],
        description: "Open settings (AI, styling, connections)",
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

/// Sync command (native only - git operations require filesystem)
#[cfg(not(target_arch = "wasm32"))]
const SYNC_COMMAND: PaletteCommand = PaletteCommand {
    name: "sync",
    aliases: &[],
    description: "Sync operations (:sync git)",
    kind: CommandKind::SingleArg,
};

/// Returns all available commands based on enabled features.
fn available_commands() -> Vec<&'static PaletteCommand> {
    #[allow(unused_mut)] // mut needed when terminal or sql features enabled
    let mut commands: Vec<&'static PaletteCommand> = BASE_COMMANDS.iter().collect();

    #[cfg(all(not(target_arch = "wasm32"), feature = "terminal"))]
    commands.push(&TERMINAL_COMMAND);

    #[cfg(all(not(target_arch = "wasm32"), feature = "sql"))]
    commands.push(&SQL_COMMAND);

    #[cfg(not(target_arch = "wasm32"))]
    commands.push(&SYNC_COMMAND);

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
    /// Current theme (supports Custom variant with plugin colors)
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
    /// Dynamic commands from plugins (source data)
    plugin_commands: Vec<DynamicCommand>,
    /// Converted plugin commands as static references (leaked for matching)
    plugin_palette_commands: &'static [PaletteCommand],
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
            plugin_commands: Vec::new(),
            plugin_palette_commands: &[],
        }
    }

    /// Set the theme (supports Custom variant with plugin colors)
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set plugin commands (called when plugins are loaded)
    pub fn set_plugin_commands(&mut self, commands: Vec<DynamicCommand>) {
        // Convert to static PaletteCommands for matching
        // Leak the Vec to get a static slice (acceptable since plugins are loaded once)
        let palette_commands: Vec<PaletteCommand> =
            commands.iter().map(|c| c.to_palette_command()).collect();
        self.plugin_palette_commands = Box::leak(palette_commands.into_boxed_slice());
        self.plugin_commands = commands;
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

        // Combine built-in commands and plugin commands
        let builtin_commands = available_commands();

        if cmd_part.is_empty() {
            // Show all commands sorted alphabetically
            for cmd in &builtin_commands {
                self.suggestions.push(CommandMatch {
                    command: cmd,
                    score: 0,
                    match_positions: Vec::new(),
                });
            }
            // Add plugin commands
            for cmd in self.plugin_palette_commands {
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

            // Helper to match a command and add to suggestions
            let match_command = |suggestions: &mut Vec<CommandMatch>,
                                 matcher: &mut Matcher,
                                 cmd: &'static PaletteCommand,
                                 pattern: &Pattern,
                                 indices: &mut Vec<u32>,
                                 buf: &mut Vec<char>| {
                // Check main name
                indices.clear();
                let haystack = Utf32Str::new(cmd.name, buf);
                if let Some(score) = pattern.indices(haystack, matcher, indices) {
                    suggestions.push(CommandMatch {
                        command: cmd,
                        score: i64::from(score),
                        match_positions: indices.iter().map(|&i| i as usize).collect(),
                    });
                    return;
                }

                // Check aliases
                for alias in cmd.aliases {
                    indices.clear();
                    let haystack = Utf32Str::new(alias, buf);
                    if let Some(score) = pattern.indices(haystack, matcher, indices) {
                        suggestions.push(CommandMatch {
                            command: cmd,
                            score: i64::from(score),
                            match_positions: indices.iter().map(|&i| i as usize).collect(),
                        });
                        break;
                    }
                }
            };

            // Match built-in commands
            for cmd in &builtin_commands {
                match_command(
                    &mut self.suggestions,
                    &mut self.matcher,
                    cmd,
                    &pattern,
                    &mut indices,
                    &mut buf,
                );
            }

            // Match plugin commands
            for cmd in self.plugin_palette_commands {
                match_command(
                    &mut self.suggestions,
                    &mut self.matcher,
                    cmd,
                    &pattern,
                    &mut indices,
                    &mut buf,
                );
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
            // Unknown command - try plugin commands
            None => CommandResult::PluginCommand(cmd_name.to_string(), args.join(" ")),
        }
    }

    /// Execute a specific command with arguments
    fn execute(&self, cmd: &PaletteCommand, args: &[&str]) -> CommandResult {
        match cmd.name {
            "version" => CommandResult::ShowInfo,
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
            "share" => CommandResult::ShareWorkspace,
            "snapshot" => {
                let title = if args.is_empty() {
                    None
                } else {
                    Some(args.join(" "))
                };
                CommandResult::UploadSnapshot(title)
            }
            "terminal" => CommandResult::OpenTerminal,
            "trace" | "tr" | "tracing" => {
                // Optional trace ID argument
                let trace_id = args.first().map(|s| s.to_string());
                CommandResult::OpenTracing(trace_id)
            }
            "sql" | "datafusion" => CommandResult::OpenSql,
            "logs" | "log" => CommandResult::OpenLogs,
            "float" | "fl" => {
                if !args.is_empty() && (args[0] == "arrange" || args[0] == "a") {
                    CommandResult::ArrangeFloatingPanes
                } else {
                    CommandResult::FloatPane
                }
            }
            "dock" | "dk" => CommandResult::DockAllPanes,
            "tutorial" | "tut" => CommandResult::OpenTutorial,
            "settings" | "set" | "prefs" | "preferences" => CommandResult::OpenSettings,
            "sync" => {
                if args.is_empty() {
                    CommandResult::Error("Usage: :sync git".to_string())
                } else {
                    match args[0].to_lowercase().as_str() {
                        "git" => CommandResult::SyncCodebase,
                        _ => CommandResult::Error(format!(
                            "Unknown sync command: {}. Use 'git'",
                            args[0]
                        )),
                    }
                }
            }
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

        // Use consume_key to prevent keys from being processed multiple times
        let (navigate_up, navigate_down, confirm, escape, tab) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                    || i.consume_key(egui::Modifiers::CTRL, Key::K),
                i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                    || i.consume_key(egui::Modifiers::CTRL, Key::J)
                    || i.consume_key(egui::Modifiers::CTRL, Key::N),
                i.consume_key(egui::Modifiers::NONE, Key::Enter),
                i.consume_key(egui::Modifiers::NONE, Key::Escape),
                i.consume_key(egui::Modifiers::NONE, Key::Tab),
            )
        });

        if escape {
            should_close = true;
        }

        if navigate_up && self.selected_index > 0 {
            self.selected_index -= 1;
            ctx.request_repaint(); // Ensure scroll_to_me is processed
        }

        if navigate_down && self.selected_index + 1 < self.suggestions.len() {
            self.selected_index += 1;
            ctx.request_repaint(); // Ensure scroll_to_me is processed
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

        // Extract colors from theme (Custom variant handles plugin colors internally)
        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let text_col = self.theme.text_primary();
        let text_muted = self.theme.text_primary().gamma_multiply(0.5);
        let accent_col = self.theme.accent_primary();
        let border_col = self.theme.border_subtle();
        let bg_elevated = self.theme.bg_elevated();

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
            .constrain_to(crate::util::overlay_content_rect(ctx))
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Input section with `:` prefix
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(":")
                                .color(text_col)
                                .size(typography::HEADING)
                                .strong(),
                        );

                        let text_edit = egui::TextEdit::singleline(&mut self.input)
                            .font(typography::heading())
                            .hint_text(
                                RichText::new("Type a command...")
                                    .color(text_muted.gamma_multiply(0.8)),
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
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, border_col),
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

                    // Suggestions with scroll shadows
                    let row_height = 32.0;
                    let visible_height = 300.0;
                    let scroll_id = egui::Id::new("command_palette_scroll");

                    let scroll_output = egui::ScrollArea::vertical()
                        .id_salt(scroll_id)
                        .max_height(visible_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if self.suggestions.is_empty() {
                                ui.add_space(12.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new("No matching commands")
                                            .color(text_muted)
                                            .size(typography::XL),
                                    );
                                });
                                ui.add_space(12.0);
                            } else {
                                for (i, suggestion) in self.suggestions.iter().enumerate() {
                                    let is_selected = i == self.selected_index;
                                    let response = self.render_suggestion_row_with_colors(
                                        ui,
                                        suggestion,
                                        is_selected,
                                        text_col,
                                        text_muted,
                                        accent_col,
                                    );
                                    // Use egui's built-in scroll_to_me for selected items
                                    if is_selected {
                                        response.scroll_to_me(Some(egui::Align::Center));
                                    }
                                }
                                // Bottom padding to prevent last item from being obscured by scroll shadow
                                ui.add_space(row_height);
                            }
                        });

                    // Render scroll shadows
                    let scroll_state = ScrollState::from_scroll_output(
                        scroll_output.content_size,
                        scroll_output.inner_rect,
                        scroll_output.state.offset,
                    );
                    let shadow_config = ScrollShadowConfig::default()
                        .with_color(bg_elevated)
                        .with_opacity(0.6);
                    render_scroll_shadows(
                        ui,
                        scroll_output.inner_rect,
                        scroll_state,
                        shadow_config,
                    );

                    ui.add_space(4.0);

                    // Footer with hints
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, border_col),
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let hint_color = text_muted.gamma_multiply(0.8);
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
            // Clear egui focus so vim keys work immediately after closing
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
        }

        result
    }

    /// Render a single suggestion row with explicit colors
    fn render_suggestion_row_with_colors(
        &self,
        ui: &mut egui::Ui,
        suggestion: &CommandMatch,
        is_selected: bool,
        text_col: Color32,
        text_muted: Color32,
        accent_col: Color32,
    ) -> egui::Response {
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
                text_muted.gamma_multiply(0.8),
            );
            ui.painter().galley(
                egui::pos2(
                    cursor_x,
                    content_rect.center().y - aliases_galley.size().y / 2.0,
                ),
                aliases_galley.clone(),
                text_muted.gamma_multiply(0.8),
            );
            cursor_x += aliases_galley.size().x + 12.0;
        }

        // Description (on the right, dimmed)
        let desc_galley = ui.painter().layout_no_wrap(
            suggestion.command.description.to_string(),
            typography::proportional(typography::MD),
            text_muted,
        );
        let desc_x = content_rect.right() - desc_galley.size().x - 8.0;
        if desc_x > cursor_x {
            ui.painter().galley(
                egui::pos2(desc_x, content_rect.center().y - desc_galley.size().y / 2.0),
                desc_galley,
                text_muted,
            );
        }

        response
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_palette_is_closed() {
        let palette = CommandPalette::new();
        assert!(!palette.is_open());
    }

    #[test]
    fn test_open_close() {
        let mut palette = CommandPalette::new();
        palette.open();
        assert!(palette.is_open());
        palette.close();
        assert!(!palette.is_open());
    }

    #[test]
    fn test_close_clears_input() {
        let mut palette = CommandPalette::new();
        palette.open();
        palette.input = "test".to_string();
        palette.close();
        assert!(palette.input.is_empty());
    }

    #[test]
    fn test_command_result_none_exists() {
        // Verify that None variant exists for no-op/unknown commands
        let result = CommandResult::None;
        // Use pattern matching since CommandResult doesn't implement PartialEq
        assert!(matches!(result, CommandResult::None));
    }

    // Note: Testing surrender_focus behavior requires egui::Context.
    // The surrender_focus pattern is verified through code review and
    // manual testing. Key invariant: When show() triggers a close (via Escape
    // or command execution), the overlay must call
    // ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL))
    // BEFORE calling self.close() to ensure vim navigation works immediately.
}
