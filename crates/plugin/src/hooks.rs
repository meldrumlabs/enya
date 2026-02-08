//! Hook traits for plugins to intercept and extend host behavior.

use rustc_hash::FxHashMap;

use crate::types::Theme;

/// Result of a command hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandHookResult {
    /// Allow the command to continue to the next handler
    Continue,
    /// Stop processing, command was handled
    Handled,
    /// Stop processing, command was blocked
    Blocked,
}

/// Result of a keyboard hook execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardHookResult {
    /// Allow the key event to continue to the next handler
    Continue,
    /// Stop processing, key was handled
    Handled,
    /// Stop processing, key was blocked
    Blocked,
}

/// Hook for intercepting lifecycle events.
pub trait LifecycleHook: Send + Sync {
    /// Called when the workspace is loaded.
    fn on_workspace_loaded(&mut self) {}

    /// Called when the workspace is about to be saved.
    fn on_workspace_saving(&mut self) {}

    /// Called when a pane is added to the workspace.
    fn on_pane_added(&mut self, _pane_id: usize) {}

    /// Called when a pane is about to be removed.
    fn on_pane_removing(&mut self, _pane_id: usize) {}

    /// Called when a pane gains focus.
    fn on_pane_focused(&mut self, _pane_id: usize) {}

    /// Called when the application is about to close.
    fn on_closing(&mut self) {}

    /// Called on each frame update (be careful with performance).
    fn on_frame(&mut self) {}
}

/// Hook for intercepting command execution.
pub trait CommandHook: Send + Sync {
    /// Called before a command is executed.
    ///
    /// Return `CommandHookResult::Handled` to prevent the default handler.
    fn before_command(&mut self, _command: &str, _args: &str) -> CommandHookResult {
        CommandHookResult::Continue
    }

    /// Called after a command is executed.
    fn after_command(&mut self, _command: &str, _args: &str, _success: bool) {}
}

/// Hook for intercepting keyboard events.
pub trait KeyboardHook: Send + Sync {
    /// Called when a key is pressed.
    ///
    /// Return `KeyboardHookResult::Handled` to prevent the default handler.
    fn on_key_pressed(&mut self, _key: &KeyEvent) -> KeyboardHookResult {
        KeyboardHookResult::Continue
    }

    /// Called when a key combination is pressed (e.g., Ctrl+S).
    fn on_key_combo(&mut self, _combo: &KeyCombo) -> KeyboardHookResult {
        KeyboardHookResult::Continue
    }
}

/// Hook for theme-related events.
pub trait ThemeHook: Send + Sync {
    /// Called before a theme change is applied.
    fn before_theme_change(&mut self, _old_theme: Theme, _new_theme: Theme) {}

    /// Called after a theme change is applied.
    fn after_theme_change(&mut self, _theme: Theme) {}

    /// Optionally provide custom colors for the theme.
    fn customize_theme(&self, _theme: Theme) -> Option<ThemeCustomization> {
        None
    }
}

/// Hook for pane-related events.
pub trait PaneHook: Send + Sync {
    /// Called when a pane is created.
    fn on_pane_created(&mut self, _pane_id: usize, _pane_type: &str) {}

    /// Called when a pane's query changes.
    fn on_query_changed(&mut self, _pane_id: usize, _query: &str) {}

    /// Called when a pane receives data.
    fn on_data_received(&mut self, _pane_id: usize) {}

    /// Called when a pane encounters an error.
    fn on_pane_error(&mut self, _pane_id: usize, _error: &str) {}
}

/// Represents a keyboard event.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// The key code
    pub key: egui::Key,
    /// Whether shift was held
    pub shift: bool,
    /// Whether ctrl/cmd was held
    pub ctrl: bool,
    /// Whether alt was held
    pub alt: bool,
}

/// Represents a key combination.
#[derive(Debug, Clone)]
pub struct KeyCombo {
    /// Primary key
    pub key: egui::Key,
    /// Modifier keys
    pub modifiers: egui::Modifiers,
}

/// Theme customization provided by a plugin.
#[derive(Debug, Clone)]
pub struct ThemeCustomization {
    /// Custom colors (name -> color)
    pub colors: FxHashMap<String, egui::Color32>,
}
