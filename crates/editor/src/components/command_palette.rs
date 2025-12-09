use egui::{Color32, FontId, Key, RichText, TextFormat, text::LayoutJob};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use super::tags::TagPath;
use crate::theme::AppTheme;
use crate::ui::colors::text_color;

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
    /// Toggle the metrics panel
    ToggleMetricsPanel,
    /// Toggle the inspector panel
    ToggleInspectorPanel,
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
    /// Save the current buffer (:w [name])
    SaveBuffer(Option<String>),
    /// Edit the current buffer (:e) - enter insert mode
    EditBuffer,
    /// Create a new buffer (:new or :enew)
    NewBuffer,
    /// Toggle zen mode (distraction-free view)
    ToggleZenMode,
    /// Toggle fullscreen for focused pane
    ToggleFullscreen,
    /// Float the focused pane into a draggable window
    FloatPane,
    /// Dock all floating windows back to tiled layout
    DockAll,
    /// Show a test notification
    TestNotify(String),
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
    /// Set tag filter (None = clear filter)
    SetTagFilter(Option<TagPath>),
    /// Add tag to focused buffer
    AddTag(TagPath),
    /// Remove tag from focused buffer
    RemoveTag(TagPath),
    /// Show all tags
    ShowTags,
    /// Toggle commit markers visibility on charts
    ToggleCommits,
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
        aliases: &["s", "find", "f"],
        description: "Open fuzzy finder search",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "info",
        aliases: &["version", "ver", "about"],
        description: "Show version and build info",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "help",
        aliases: &["h", "?"],
        description: "Show help and available commands",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "metrics",
        aliases: &["m"],
        description: "Toggle metrics panel visibility",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "inspector",
        aliases: &["i", "info"],
        description: "Toggle inspector panel visibility",
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
        aliases: &["q", "quit"],
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
        name: "write",
        aliases: &["w", "save"],
        description: "Save buffer (:w [name])",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "edit",
        aliases: &["e"],
        description: "Edit buffer (enter insert mode)",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "new",
        aliases: &["enew", "buffer"],
        description: "Create a new buffer",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "zen",
        aliases: &["z", "focus", "distraction-free"],
        description: "Toggle zen mode (distraction-free view)",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "fullscreen",
        aliases: &["full", "maximize", "max"],
        description: "Toggle fullscreen for focused chart",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "notify",
        aliases: &["n", "toast"],
        description: "Show a test notification (info/success/warn/error)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "float",
        aliases: &["fl", "popup", "detach"],
        description: "Float focused chart into a draggable window",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "dock",
        aliases: &["d", "attach", "tile"],
        description: "Dock all floating windows back to tiled layout",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "home",
        aliases: &["landing", "start", "welcome"],
        description: "Show the landing page / home screen",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "screenshot",
        aliases: &["ss", "snap", "capture"],
        description: "Take a screenshot (optional: path to save)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "mksession",
        aliases: &["mks", "savews", "saveworkspace"],
        description: "Save workspace (:mksession [name])",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "source",
        aliases: &["so", "loadws", "loadworkspace"],
        description: "Load workspace (:source <name>)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "workspaces",
        aliases: &["ws", "sessions"],
        description: "List available workspaces",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "share",
        aliases: &["export", "url"],
        description: "Share current workspace as URL (copies to clipboard)",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "tag",
        aliases: &["#"],
        description: "Filter by tag or add/remove tags (+tag, -tag)",
        kind: CommandKind::SingleArg,
    },
    PaletteCommand {
        name: "tags",
        aliases: &["taglist", "tl"],
        description: "Show all tags with buffer counts",
        kind: CommandKind::NoArgs,
    },
    PaletteCommand {
        name: "commits",
        aliases: &["git", "markers"],
        description: "Toggle git commit markers on charts",
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
            "metrics" => CommandResult::ToggleMetricsPanel,
            "inspector" => CommandResult::ToggleInspectorPanel,
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
            "write" => {
                // :w or :w name - save buffer with optional name
                let name = if args.is_empty() {
                    None
                } else {
                    // Join all args as the name (allows spaces in names)
                    Some(args.join(" "))
                };
                CommandResult::SaveBuffer(name)
            }
            "edit" => CommandResult::EditBuffer,
            "new" => CommandResult::NewBuffer,
            "zen" => CommandResult::ToggleZenMode,
            "fullscreen" => CommandResult::ToggleFullscreen,
            "float" => CommandResult::FloatPane,
            "dock" => CommandResult::DockAll,
            "notify" => {
                let level = args.first().copied().unwrap_or("info");
                CommandResult::TestNotify(level.to_string())
            }
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
            "tag" => self.execute_tag_command(args),
            "tags" => CommandResult::ShowTags,
            "commits" => CommandResult::ToggleCommits,
            _ => CommandResult::None,
        }
    }

    /// Execute the :tag command with various subcommands
    fn execute_tag_command(&self, args: &[&str]) -> CommandResult {
        if args.is_empty() {
            // :tag with no args - clear the filter
            return CommandResult::SetTagFilter(None);
        }

        let arg = args[0];

        // Check for add/remove prefixes
        if let Some(tag_name) = arg.strip_prefix('+') {
            // :tag +production - add tag to focused buffer
            let path = TagPath::parse(tag_name);
            if path.is_empty() {
                return CommandResult::Error("Empty tag path".to_string());
            }
            return CommandResult::AddTag(path);
        }

        if let Some(tag_name) = arg.strip_prefix('-') {
            // :tag -production - remove tag from focused buffer
            let path = TagPath::parse(tag_name);
            if path.is_empty() {
                return CommandResult::Error("Empty tag path".to_string());
            }
            return CommandResult::RemoveTag(path);
        }

        // :tag production - set filter
        let path = TagPath::parse(arg);
        if path.is_empty() {
            return CommandResult::SetTagFilter(None);
        }
        CommandResult::SetTagFilter(Some(path))
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

        egui::Area::new(egui::Id::new("command_palette"))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let bg_color = match self.theme {
                    AppTheme::Light => Color32::from_rgb(250, 250, 250),
                    AppTheme::Dark => Color32::from_rgb(30, 30, 35),
                };
                let border_color = match self.theme {
                    AppTheme::Light => Color32::from_rgb(200, 200, 200),
                    AppTheme::Dark => Color32::from_rgb(60, 60, 70),
                };

                egui::Frame::new()
                    .fill(bg_color)
                    .stroke(egui::Stroke::new(1.0, border_color))
                    .corner_radius(8.0)
                    .inner_margin(0.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(80),
                    })
                    .show(ui, |ui| {
                        ui.set_width(popup_width);

                        // Input section with `:` prefix
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(":")
                                    .color(text_color(self.theme))
                                    .size(18.0)
                                    .strong(),
                            );

                            let text_edit = egui::TextEdit::singleline(&mut self.input)
                                .font(FontId::proportional(16.0))
                                .hint_text(
                                    RichText::new("Type a command...")
                                        .color(text_color(self.theme).gamma_multiply(0.4)),
                                )
                                .frame(false)
                                .desired_width(popup_width - 50.0);

                            let response = ui.add(text_edit);
                            response.request_focus();

                            if response.changed() {
                                self.error_message = None;
                                self.refresh_suggestions();
                            }
                        });

                        ui.add_space(8.0);

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
                        ui.add_space(4.0);

                        // Error message if any
                        if let Some(ref error) = self.error_message {
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        egui_phosphor::regular::WARNING,
                                        error
                                    ))
                                    .color(Color32::from_rgb(220, 80, 80))
                                    .size(13.0),
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
                                                .size(14.0),
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
                            ui.label(RichText::new("↑↓").color(hint_color).size(11.0));
                            ui.label(RichText::new("navigate").color(hint_color).size(11.0));
                            ui.add_space(12.0);
                            ui.label(RichText::new("Tab").color(hint_color).size(11.0));
                            ui.label(RichText::new("complete").color(hint_color).size(11.0));
                            ui.add_space(12.0);
                            ui.label(RichText::new("↵").color(hint_color).size(11.0));
                            ui.label(RichText::new("execute").color(hint_color).size(11.0));
                            ui.add_space(12.0);
                            ui.label(RichText::new("esc").color(hint_color).size(11.0));
                            ui.label(RichText::new("close").color(hint_color).size(11.0));
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
        let highlight_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(200, 150, 0),
            AppTheme::Dark => Color32::from_rgb(255, 200, 50),
        };
        let selected_bg = match self.theme {
            AppTheme::Light => Color32::from_rgb(230, 240, 255),
            AppTheme::Dark => Color32::from_rgb(45, 50, 70),
        };
        let hover_bg = match self.theme {
            AppTheme::Light => Color32::from_rgb(240, 245, 250),
            AppTheme::Dark => Color32::from_rgb(40, 42, 50),
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
                FontId::proportional(12.0),
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
            FontId::proportional(12.0),
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
        let font_id = FontId::proportional(14.0);

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
