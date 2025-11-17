// Ref: https://github.com/rerun-io/rerun/blob/5949a229032660911b0c49f67c002dde23a714f4/crates/viewer/re_ui/src/command.rs#L490

use egui::{Key, KeyboardShortcut, Modifiers, os::OperatingSystem};
use smallvec::{SmallVec, smallvec};

use crate::{
    theme::AppTheme,
    ui::{colors::text_color, icons::EXTERNAL_LINK},
};

/// Interface for sending [`UICommand`] messages.
pub trait UICommandSender {
    fn send_ui(&self, command: UICommand);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::EnumIter)]
pub enum UICommand {
    Open,
    OpenExampleDashboard(usize),
    Settings,
    Help,
    CloseSettings,
    ConnectionStatus(bool),
    Theme(AppTheme),
    ToggleTheme,
}

impl UICommand {
    pub fn text(self) -> &'static str {
        self.text_and_tooltip().0
    }

    pub fn tooltip(self) -> &'static str {
        self.text_and_tooltip().1
    }

    pub fn text_and_tooltip(self) -> (&'static str, &'static str) {
        match self {
            Self::Help => ("Help", "Get help with any Playground issues"),
            Self::Open => ("Open", "Open Meldrum Dashboard"),
            Self::OpenExampleDashboard(_) => ("...", "Create a Meldrum dashboard"),
            Self::Settings => ("Settings…", "Open Meldrum Settings"),
            Self::Theme(_) => ("...", "..."),
            Self::ToggleTheme => ("Toggle Theme...", "Toogles the application theme"),
            Self::CloseSettings => ("Close Settings…", "Close Meldrum Settings"),
            Self::ConnectionStatus(_) => ("", ""),
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
            ui.close_menu();
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
        use strum::IntoEnumIterator as _;

        let anything_has_focus = egui_ctx.memory(|mem| mem.focused().is_some());
        if anything_has_focus {
            return None; // e.g. we're typing in a TextField
        }

        let mut commands: Vec<(KeyboardShortcut, Self)> = Self::iter()
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
    pub fn kb_shortcuts(self, _os: OperatingSystem) -> SmallVec<[KeyboardShortcut; 2]> {
        fn key(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::NONE, key)
        }

        /*
        fn cmd(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND, key)
        }

        fn cmd_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, key)
        }

        fn cmd_alt(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, key)
        }

        fn ctrl(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL, key)
        }

        fn cmd_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, key)
        }

        fn ctrl_shift(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, key)
        }
        */

        match self {
            Self::Help => smallvec![key(Key::X)],
            Self::Open => smallvec![key(Key::O)],
            Self::Settings => smallvec![key(Key::S)],
            Self::CloseSettings => smallvec![],
            Self::ToggleTheme => smallvec![key(Key::T)],
            Self::Theme(_) => smallvec![],
            Self::OpenExampleDashboard(_) => smallvec![],
            Self::ConnectionStatus(_) => smallvec![],
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
