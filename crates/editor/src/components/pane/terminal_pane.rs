//! Terminal Pane - Embedded terminal emulator for running shell commands.
//!
//! This pane provides a terminal interface backed by ghostty's VT library,
//! allowing users to run commands like kubectl, k9s, etc. while debugging incidents.

#![cfg(not(target_arch = "wasm32"))]

use egui::RichText;

use egui_ghostty::{ColorScheme, Rgb, TerminalConfig, TerminalWidget};

use crate::components::util::id_generator::next_id_usize;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;

/// Actions that can be triggered by the terminal pane.
#[derive(Debug, Clone)]
pub enum TerminalPaneAction {
    /// No action.
    None,
    /// Terminal exited.
    Exited,
}

/// A terminal pane that runs a shell.
pub struct TerminalPane {
    /// Unique identifier for this pane.
    id: usize,
    /// The terminal widget.
    terminal: TerminalWidget,
    /// Current theme.
    theme: AppTheme,
    /// Terminal title (from OSC sequences or default).
    title: String,
    /// Description for the pane.
    description: String,
    /// Shell command being run (stored for potential restart).
    #[allow(dead_code)]
    shell: String,
    /// Whether an action is pending.
    pending_action: TerminalPaneAction,
}

impl TerminalPane {
    /// Create a new terminal pane with the default shell.
    pub fn new(theme: AppTheme) -> Result<Self, String> {
        Self::with_shell(theme, &Self::default_shell())
    }

    /// Create a new terminal pane with a specific shell.
    pub fn with_shell(theme: AppTheme, shell: &str) -> Result<Self, String> {
        let config = TerminalConfig::default();
        let mut terminal = TerminalWidget::new(config, shell)
            .map_err(|e| format!("Failed to create terminal: {e}"))?;

        // Apply theme colors
        terminal.set_color_scheme(Self::theme_to_color_scheme(&theme));

        Ok(Self {
            id: next_id_usize(),
            terminal,
            theme,
            title: "Terminal".to_string(),
            description: format!("Terminal running {shell}"),
            shell: shell.to_string(),
            pending_action: TerminalPaneAction::None,
        })
    }

    /// Get the default shell for the current platform.
    ///
    /// Uses $SHELL environment variable (the user's preferred shell) first,
    /// then falls back to common shell paths.
    fn default_shell() -> String {
        // Prefer $SHELL (user's configured login shell) - this is what Ghostty does
        if let Ok(shell) = std::env::var("SHELL") {
            if !shell.is_empty() && std::path::Path::new(&shell).exists() {
                return shell;
            }
        }

        // Fallback to common shells
        if std::path::Path::new("/bin/zsh").exists() {
            "/bin/zsh".to_string()
        } else if std::path::Path::new("/bin/bash").exists() {
            "/bin/bash".to_string()
        } else {
            "/bin/sh".to_string()
        }
    }

    /// Convert an AppTheme to a terminal ColorScheme.
    fn theme_to_color_scheme(theme: &AppTheme) -> ColorScheme {
        let bg = theme.bg_base();
        let fg = theme.text_primary();
        let accent = theme.accent_primary();

        // Use theme colors for semantic terminal colors
        let black = theme.bg_elevated();
        let bright_black = theme.text_tertiary();
        let white = theme.text_secondary();
        let bright_white = theme.text_primary();
        let selection = theme.accent_selection();

        // Use terminal palette for semantically-correct ANSI colors
        // terminal_palette returns [Red, Green, Yellow, Blue, Magenta, Cyan]
        let palette = theme.terminal_palette();

        // Helper to brighten a color for "bright" variants
        let brighten = |c: egui::Color32| -> Rgb {
            // Increase luminosity by ~20% while preserving hue
            let r = (c.r() as u16 * 120 / 100).min(255) as u8;
            let g = (c.g() as u16 * 120 / 100).min(255) as u8;
            let b = (c.b() as u16 * 120 / 100).min(255) as u8;
            Rgb::new(r.max(c.r()), g.max(c.g()), b.max(c.b()))
        };

        ColorScheme {
            foreground: Rgb::new(fg.r(), fg.g(), fg.b()),
            background: Rgb::new(bg.r(), bg.g(), bg.b()),
            cursor: Rgb::new(accent.r(), accent.g(), accent.b()),
            selection_bg: Rgb::new(selection.r(), selection.g(), selection.b()),
            colors: [
                // Normal colors (0-7)
                Rgb::new(black.r(), black.g(), black.b()), // 0: Black
                Rgb::new(palette[0].r(), palette[0].g(), palette[0].b()), // 1: Red
                Rgb::new(palette[1].r(), palette[1].g(), palette[1].b()), // 2: Green
                Rgb::new(palette[2].r(), palette[2].g(), palette[2].b()), // 3: Yellow
                Rgb::new(palette[3].r(), palette[3].g(), palette[3].b()), // 4: Blue
                Rgb::new(palette[4].r(), palette[4].g(), palette[4].b()), // 5: Magenta
                Rgb::new(palette[5].r(), palette[5].g(), palette[5].b()), // 6: Cyan
                Rgb::new(white.r(), white.g(), white.b()), // 7: White
                // Bright colors (8-15) - brightened versions
                Rgb::new(bright_black.r(), bright_black.g(), bright_black.b()), // 8: Bright Black
                brighten(palette[0]),                                           // 9: Bright Red
                brighten(palette[1]),                                           // 10: Bright Green
                brighten(palette[2]),                                           // 11: Bright Yellow
                brighten(palette[3]),                                           // 12: Bright Blue
                brighten(palette[4]), // 13: Bright Magenta
                brighten(palette[5]), // 14: Bright Cyan
                Rgb::new(bright_white.r(), bright_white.g(), bright_white.b()), // 15: Bright White
            ],
        }
    }

    /// Show the terminal pane.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        // Show the terminal
        let response = self.terminal.show(ui);

        // Update title from OSC sequences
        if let Some(title) = response.title {
            if !title.is_empty() && title != self.title {
                self.title = title;
            }
        }
    }

    /// Set the theme.
    ///
    /// Updates the terminal's color scheme for rendering. The terminal
    /// widget uses this scheme when painting cells, cursor, and selection.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.terminal
            .set_color_scheme(Self::theme_to_color_scheme(&theme));
    }

    /// Get the terminal title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Write data to the terminal.
    pub fn write(&mut self, data: &[u8]) -> Result<(), String> {
        self.terminal
            .write(data)
            .map_err(|e| format!("Write failed: {e}"))
    }

    /// Take the pending action, if any.
    pub fn take_action(&mut self) -> TerminalPaneAction {
        std::mem::replace(&mut self.pending_action, TerminalPaneAction::None)
    }

    /// Enable or disable keyboard input processing.
    ///
    /// When disabled, the terminal will not process keyboard input even if focused.
    /// Use this when modal overlays are open to prevent keys from being captured.
    pub fn set_keyboard_enabled(&mut self, enabled: bool) {
        self.terminal.set_keyboard_enabled(enabled);
    }
}

impl crate::components::Component for TerminalPane {
    fn show(&mut self, ui: &mut egui::Ui) {
        TerminalPane::show(self, ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        self.title.clone()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        TerminalPane::set_theme(self, theme);
    }

    fn set_api_key(&mut self, _key: &str) {
        // Not needed for terminal
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed for terminal
    }

    fn label(&self) -> RichText {
        RichText::new(format!(
            "{} {}",
            semantic_icons::action::TERMINAL,
            self.title
        ))
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
