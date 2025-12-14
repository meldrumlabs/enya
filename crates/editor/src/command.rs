// Ref: https://github.com/rerun-io/rerun/blob/5949a229032660911b0c49f67c002dde23a714f4/crates/viewer/re_ui/src/command.rs#L490

use egui::{Key, KeyboardShortcut, Modifiers, os::OperatingSystem};

use crate::{
    theme::AppTheme,
    ui::{colors::text_color, icons::EXTERNAL_LINK},
};

/// Interface for sending [`UICommand`] messages.
pub trait UICommandSender {
    fn send_ui(&self, command: UICommand);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UICommand {
    Home,
    Dashboard,
    OpenExampleDashboard(usize),
    Help,
    ConnectionStatus(bool),
    Theme(AppTheme),
    ToggleTheme,
    OpenFuzzyFinder,
    OpenCommandPalette,
}

impl UICommand {
    /// Returns all command variants (for iteration).
    /// Note: OpenExampleDashboard uses index 0 as placeholder.
    fn all() -> impl Iterator<Item = Self> {
        [
            Self::Home,
            Self::Dashboard,
            Self::OpenExampleDashboard(0),
            Self::Help,
            Self::ConnectionStatus(false),
            Self::Theme(AppTheme::Dark),
            Self::ToggleTheme,
            Self::OpenFuzzyFinder,
            Self::OpenCommandPalette,
        ]
        .into_iter()
    }

    pub fn text(self) -> &'static str {
        self.text_and_tooltip().0
    }

    pub fn tooltip(self) -> &'static str {
        self.text_and_tooltip().1
    }

    pub fn text_and_tooltip(self) -> (&'static str, &'static str) {
        match self {
            Self::Home => ("Home", "Open Welcome Screen"),
            Self::Help => ("Help", "Get help with any Playground issues"),
            Self::Dashboard => ("Dashboard", "Open Enya Dashboard"),
            Self::OpenExampleDashboard(_) => ("...", "Create an Enya dashboard"),
            Self::Theme(_) => ("...", "..."),
            Self::ToggleTheme => ("Toggle Theme...", "Toggles the application theme"),
            Self::ConnectionStatus(_) => ("", ""),
            Self::OpenFuzzyFinder => ("Search...", "Open fuzzy finder to search metrics"),
            Self::OpenCommandPalette => ("Command Palette", "Open command palette"),
        }
    }

    /// Show this command as a menu-button.
    ///
    /// If clicked, enqueue the command.
    pub fn menu_button_ui(
        self,
        ui: &mut egui::Ui,
        theme: AppTheme,
        command_sender: &impl UICommandSender,
    ) -> egui::Response {
        let button = self.menu_button(ui.ctx(), theme);
        let mut response = ui.add(button).on_hover_text(self.tooltip());

        if self.is_link() {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }

        if response.clicked() {
            command_sender.send_ui(self);
            ui.close();
        }

        response
    }

    pub fn is_link(self) -> bool {
        matches!(self, Self::Help)
    }

    pub fn icon(self) -> Option<&'static crate::ui::icons::Icon> {
        match self {
            Self::Help => Some(&EXTERNAL_LINK),
            _ => None,
        }
    }

    #[must_use = "Returns the Command that was triggered by some keyboard shortcut"]
    pub fn listen_for_kb_shortcut(egui_ctx: &egui::Context) -> Option<Self> {
        let anything_has_focus = egui_ctx.memory(|mem| mem.focused().is_some());
        if anything_has_focus {
            return None; // e.g. we're typing in a TextField
        }

        let mut commands: Vec<(KeyboardShortcut, Self)> = Self::all()
            .flat_map(|cmd| {
                cmd.kb_shortcuts(egui_ctx.os())
                    .into_iter()
                    .map(move |kb_shortcut| (kb_shortcut, cmd))
            })
            .collect();

        // If the user pressed `Cmd-Shift-S` then egui will match that
        // with both `Cmd-Shift-S` and `Cmd-S`.
        // The reason is that `Shift` (and `Alt`) are sometimes required to produce certain keys,
        // such as `+` (`Shift =` on an american keyboard).
        // The result of this is that we must check for `Cmd-Shift-S` before `Cmd-S`, etc.
        // So we order the commands here so that the commands with `Shift` and `Alt` in them
        // are checked first.
        commands.sort_by_key(|(kb_shortcut, _cmd)| {
            let num_shift_alts =
                kb_shortcut.modifiers.shift as i32 + kb_shortcut.modifiers.alt as i32;
            -num_shift_alts // most first
        });

        egui_ctx.input_mut(|input| {
            for (kb_shortcut, command) in commands {
                if input.consume_shortcut(&kb_shortcut) {
                    return Some(command);
                }
            }
            None
        })
    }

    pub fn menu_button(self, egui_ctx: &egui::Context, theme: AppTheme) -> egui::Button<'static> {
        let mut button = if let Some(icon) = self.icon() {
            egui::Button::image_and_text(icon.as_image().tint(text_color(theme)), self.text())
        } else {
            egui::Button::new(self.text())
        };

        if let Some(shortcut_text) = self.formatted_kb_shortcut(egui_ctx) {
            button = button.shortcut_text(shortcut_text);
        }

        button
    }

    /// All keyboard shortcuts, with the primary first.
    pub fn kb_shortcuts(self, os: OperatingSystem) -> Vec<KeyboardShortcut> {
        fn key(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::NONE, key)
        }

        fn cmd_key(key: Key, os: OperatingSystem) -> KeyboardShortcut {
            let modifiers = if os == OperatingSystem::Mac {
                Modifiers::MAC_CMD
            } else {
                Modifiers::CTRL
            };
            KeyboardShortcut::new(modifiers, key)
        }

        match self {
            Self::Home => vec![],
            Self::Help => vec![], // Help accessed via ? on landing page or :help command
            Self::Dashboard => vec![key(Key::D)],
            Self::ToggleTheme => vec![], // Removed: T key now used for gt/gT tab navigation
            Self::Theme(_) => vec![],
            Self::OpenExampleDashboard(_) => vec![],
            Self::ConnectionStatus(_) => vec![],
            Self::OpenFuzzyFinder => vec![cmd_key(Key::K, os), cmd_key(Key::P, os)],
            // ':' key (colon) - no modifiers needed since it's already the shifted key
            Self::OpenCommandPalette => vec![key(Key::Colon)],
        }
    }

    /// Primary keyboard shortcut
    fn primary_kb_shortcut(self, os: OperatingSystem) -> Option<KeyboardShortcut> {
        self.kb_shortcuts(os).first().copied()
    }

    /// Return the keyboard shortcut for this command, nicely formatted
    pub fn formatted_kb_shortcut(self, egui_ctx: &egui::Context) -> Option<String> {
        // Note: we only show the primary shortcut to the user.
        // The fallbacks are there for people who have muscle memory for the other shortcuts.
        self.primary_kb_shortcut(egui_ctx.os())
            .map(|shortcut| egui_ctx.format_shortcut(&shortcut))
    }

    /// Add e.g. " (Ctrl+F11)" as a suffix
    pub fn format_shortcut_tooltip_suffix(self, egui_ctx: &egui::Context) -> String {
        if let Some(shortcut_text) = self.formatted_kb_shortcut(egui_ctx) {
            format!(" ({shortcut_text})")
        } else {
            Default::default()
        }
    }
    pub fn tooltip_with_shortcut(self, egui_ctx: &egui::Context) -> String {
        format!(
            "{}{}",
            self.tooltip(),
            self.format_shortcut_tooltip_suffix(egui_ctx)
        )
    }
}

/// Sender that queues up the execution of commands.
#[derive(Clone)]
pub struct CommandSender {
    //system_sender: std::sync::mpsc::Sender<SystemCommand>,
    ui_sender: std::sync::mpsc::Sender<UICommand>,
}

// Creates a new command channel.
pub fn command_channel() -> (CommandSender, CommandReceiver) {
    let (ui_sender, ui_receiver) = std::sync::mpsc::channel();
    (CommandSender { ui_sender }, CommandReceiver { ui_receiver })
}

/// Receiver for the [`CommandSender`]
pub struct CommandReceiver {
    ui_receiver: std::sync::mpsc::Receiver<UICommand>,
}

impl CommandReceiver {
    /// Receive a [`UICommand`] to be executed if any is queued.
    pub fn recv_ui(&self) -> Option<UICommand> {
        // The only way this can fail (other than being empty)
        // is if the sender has been dropped.
        self.ui_receiver.try_recv().ok()
    }
}

impl UICommandSender for CommandSender {
    /// Send a command to be executed.
    fn send_ui(&self, command: UICommand) {
        // The only way this can fail is if the receiver has been dropped.
        self.ui_sender.send(command).ok();
    }
}
