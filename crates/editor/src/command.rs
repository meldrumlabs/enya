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
    pub fn kb_shortcuts(self, _os: OperatingSystem) -> Vec<KeyboardShortcut> {
        fn key(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::NONE, key)
        }

        match self {
            Self::Home => vec![],
            Self::Help => vec![], // Help accessed via ? on landing page or :help command
            Self::Dashboard => vec![key(Key::D)],
            Self::ToggleTheme => vec![], // Removed: T key now used for gt/gT tab navigation
            Self::Theme(_) => vec![],
            Self::OpenExampleDashboard(_) => vec![],
            Self::ConnectionStatus(_) => vec![],
            Self::OpenFuzzyFinder => vec![], // Space+m leader key sequence
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

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== UICommand Tests ====================

    #[test]
    fn test_ui_command_all_returns_all_variants() {
        let commands: Vec<UICommand> = UICommand::all().collect();
        assert_eq!(commands.len(), 9);
        assert!(commands.contains(&UICommand::Home));
        assert!(commands.contains(&UICommand::Dashboard));
        assert!(commands.contains(&UICommand::Help));
        assert!(commands.contains(&UICommand::ToggleTheme));
        assert!(commands.contains(&UICommand::OpenFuzzyFinder));
        assert!(commands.contains(&UICommand::OpenCommandPalette));
    }

    #[test]
    fn test_ui_command_text_returns_expected_values() {
        assert_eq!(UICommand::Home.text(), "Home");
        assert_eq!(UICommand::Help.text(), "Help");
        assert_eq!(UICommand::Dashboard.text(), "Dashboard");
        assert_eq!(UICommand::ToggleTheme.text(), "Toggle Theme...");
        assert_eq!(UICommand::OpenFuzzyFinder.text(), "Search...");
        assert_eq!(UICommand::OpenCommandPalette.text(), "Command Palette");
    }

    #[test]
    fn test_ui_command_tooltip_returns_expected_values() {
        assert_eq!(UICommand::Home.tooltip(), "Open Welcome Screen");
        assert_eq!(
            UICommand::Help.tooltip(),
            "Get help with any Playground issues"
        );
        assert_eq!(UICommand::Dashboard.tooltip(), "Open Enya Dashboard");
        assert_eq!(
            UICommand::ToggleTheme.tooltip(),
            "Toggles the application theme"
        );
        assert_eq!(
            UICommand::OpenFuzzyFinder.tooltip(),
            "Open fuzzy finder to search metrics"
        );
        assert_eq!(
            UICommand::OpenCommandPalette.tooltip(),
            "Open command palette"
        );
    }

    #[test]
    fn test_ui_command_text_and_tooltip_consistency() {
        for cmd in UICommand::all() {
            let (text, tooltip) = cmd.text_and_tooltip();
            assert_eq!(cmd.text(), text);
            assert_eq!(cmd.tooltip(), tooltip);
        }
    }

    #[test]
    fn test_ui_command_is_link() {
        assert!(UICommand::Help.is_link());
        assert!(!UICommand::Home.is_link());
        assert!(!UICommand::Dashboard.is_link());
        assert!(!UICommand::ToggleTheme.is_link());
        assert!(!UICommand::OpenFuzzyFinder.is_link());
        assert!(!UICommand::OpenCommandPalette.is_link());
    }

    #[test]
    fn test_ui_command_icon() {
        assert!(UICommand::Help.icon().is_some());
        assert!(UICommand::Home.icon().is_none());
        assert!(UICommand::Dashboard.icon().is_none());
        assert!(UICommand::ToggleTheme.icon().is_none());
        assert!(UICommand::OpenFuzzyFinder.icon().is_none());
        assert!(UICommand::OpenCommandPalette.icon().is_none());
    }

    #[test]
    fn test_ui_command_kb_shortcuts_dashboard() {
        let shortcuts = UICommand::Dashboard.kb_shortcuts(OperatingSystem::Mac);
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].logical_key, Key::D);
        assert_eq!(shortcuts[0].modifiers, Modifiers::NONE);
    }

    #[test]
    fn test_ui_command_kb_shortcuts_command_palette() {
        let shortcuts = UICommand::OpenCommandPalette.kb_shortcuts(OperatingSystem::Mac);
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].logical_key, Key::Colon);
        assert_eq!(shortcuts[0].modifiers, Modifiers::NONE);
    }

    #[test]
    fn test_ui_command_kb_shortcuts_empty_for_some_commands() {
        assert!(
            UICommand::Home
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::Help
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::ToggleTheme
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::OpenFuzzyFinder
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::Theme(AppTheme::Dark)
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::OpenExampleDashboard(0)
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::ConnectionStatus(false)
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
    }

    #[test]
    fn test_ui_command_kb_shortcuts_consistent_across_os() {
        let mac_shortcuts = UICommand::Dashboard.kb_shortcuts(OperatingSystem::Mac);
        let windows_shortcuts = UICommand::Dashboard.kb_shortcuts(OperatingSystem::Windows);
        let linux_shortcuts = UICommand::Dashboard.kb_shortcuts(OperatingSystem::Nix);

        assert_eq!(mac_shortcuts, windows_shortcuts);
        assert_eq!(mac_shortcuts, linux_shortcuts);
    }

    #[test]
    fn test_ui_command_primary_kb_shortcut() {
        assert!(
            UICommand::Dashboard
                .primary_kb_shortcut(OperatingSystem::Mac)
                .is_some()
        );
        assert!(
            UICommand::OpenCommandPalette
                .primary_kb_shortcut(OperatingSystem::Mac)
                .is_some()
        );
        assert!(
            UICommand::Home
                .primary_kb_shortcut(OperatingSystem::Mac)
                .is_none()
        );
        assert!(
            UICommand::Help
                .primary_kb_shortcut(OperatingSystem::Mac)
                .is_none()
        );
    }

    #[test]
    fn test_ui_command_equality() {
        assert_eq!(UICommand::Home, UICommand::Home);
        assert_eq!(UICommand::Help, UICommand::Help);
        assert_eq!(UICommand::Dashboard, UICommand::Dashboard);
        assert_ne!(UICommand::Home, UICommand::Help);
        assert_ne!(UICommand::Dashboard, UICommand::Home);
    }

    #[test]
    fn test_ui_command_theme_variants() {
        let dark = UICommand::Theme(AppTheme::Dark);
        let light = UICommand::Theme(AppTheme::Light);
        assert_ne!(dark, light);
        assert_eq!(dark.text(), "...");
        assert_eq!(light.text(), "...");
    }

    #[test]
    fn test_ui_command_open_example_dashboard_with_index() {
        let cmd0 = UICommand::OpenExampleDashboard(0);
        let cmd1 = UICommand::OpenExampleDashboard(1);
        let cmd5 = UICommand::OpenExampleDashboard(5);
        assert_ne!(cmd0, cmd1);
        assert_ne!(cmd1, cmd5);
        assert_eq!(cmd0.text(), "...");
        assert_eq!(cmd1.text(), "...");
    }

    #[test]
    fn test_ui_command_connection_status_variants() {
        let connected = UICommand::ConnectionStatus(true);
        let disconnected = UICommand::ConnectionStatus(false);
        assert_ne!(connected, disconnected);
        assert_eq!(connected.text(), "");
        assert_eq!(disconnected.text(), "");
    }

    #[test]
    fn test_ui_command_clone() {
        let cmd = UICommand::Dashboard;
        let cloned = cmd;
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_ui_command_copy() {
        let cmd = UICommand::OpenCommandPalette;
        let copied: UICommand = cmd;
        assert_eq!(cmd, copied);
    }

    #[test]
    fn test_ui_command_debug() {
        let debug_str = format!("{:?}", UICommand::Home);
        assert!(debug_str.contains("Home"));
    }

    #[test]
    fn test_ui_command_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(UICommand::Home);
        set.insert(UICommand::Dashboard);
        set.insert(UICommand::Home); // duplicate
        assert_eq!(set.len(), 2);
    }

    // ==================== CommandSender/Receiver Tests ====================

    #[test]
    fn test_command_channel_creation() {
        let (sender, receiver) = command_channel();
        // Channel should be created successfully
        drop(sender);
        drop(receiver);
    }

    #[test]
    fn test_command_channel_send_and_receive() {
        let (sender, receiver) = command_channel();
        sender.send_ui(UICommand::Home);
        let received = receiver.recv_ui();
        assert_eq!(received, Some(UICommand::Home));
    }

    #[test]
    fn test_command_channel_multiple_messages() {
        let (sender, receiver) = command_channel();
        sender.send_ui(UICommand::Home);
        sender.send_ui(UICommand::Dashboard);
        sender.send_ui(UICommand::Help);

        assert_eq!(receiver.recv_ui(), Some(UICommand::Home));
        assert_eq!(receiver.recv_ui(), Some(UICommand::Dashboard));
        assert_eq!(receiver.recv_ui(), Some(UICommand::Help));
    }

    #[test]
    fn test_command_channel_empty_returns_none() {
        let (_sender, receiver) = command_channel();
        assert_eq!(receiver.recv_ui(), None);
    }

    #[test]
    fn test_command_channel_recv_after_all_consumed() {
        let (sender, receiver) = command_channel();
        sender.send_ui(UICommand::Home);
        let _ = receiver.recv_ui();
        assert_eq!(receiver.recv_ui(), None);
    }

    #[test]
    fn test_command_sender_clone() {
        let (sender, receiver) = command_channel();
        let sender_clone = sender.clone();

        sender.send_ui(UICommand::Home);
        sender_clone.send_ui(UICommand::Dashboard);

        assert_eq!(receiver.recv_ui(), Some(UICommand::Home));
        assert_eq!(receiver.recv_ui(), Some(UICommand::Dashboard));
    }

    #[test]
    fn test_command_sender_dropped_receiver_does_not_panic() {
        let (sender, receiver) = command_channel();
        drop(receiver);
        // Should not panic, just silently fail
        sender.send_ui(UICommand::Home);
    }

    #[test]
    fn test_command_receiver_dropped_sender_returns_none() {
        let (sender, receiver) = command_channel();
        drop(sender);
        assert_eq!(receiver.recv_ui(), None);
    }

    #[test]
    fn test_ui_command_sender_trait() {
        struct MockSender {
            commands: std::cell::RefCell<Vec<UICommand>>,
        }

        impl UICommandSender for MockSender {
            fn send_ui(&self, command: UICommand) {
                self.commands.borrow_mut().push(command);
            }
        }

        let mock = MockSender {
            commands: std::cell::RefCell::new(Vec::new()),
        };
        mock.send_ui(UICommand::Home);
        mock.send_ui(UICommand::Dashboard);

        let commands = mock.commands.borrow();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], UICommand::Home);
        assert_eq!(commands[1], UICommand::Dashboard);
    }

    #[test]
    fn test_command_channel_fifo_order() {
        let (sender, receiver) = command_channel();

        // Send in specific order
        sender.send_ui(UICommand::OpenExampleDashboard(1));
        sender.send_ui(UICommand::OpenExampleDashboard(2));
        sender.send_ui(UICommand::OpenExampleDashboard(3));

        // Receive in same order (FIFO)
        assert_eq!(receiver.recv_ui(), Some(UICommand::OpenExampleDashboard(1)));
        assert_eq!(receiver.recv_ui(), Some(UICommand::OpenExampleDashboard(2)));
        assert_eq!(receiver.recv_ui(), Some(UICommand::OpenExampleDashboard(3)));
    }
}
