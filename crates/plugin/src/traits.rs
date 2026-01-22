//! Core plugin traits and types.

use std::any::Any;

use bitflags::bitflags;

use crate::hooks::{CommandHook, KeyboardHook, LifecycleHook, PaneHook, ThemeHook};
use crate::theme::ThemeDefinition;
use crate::types::{PluginContext, Theme};

/// Result type for plugin operations.
pub type PluginResult<T> = Result<T, PluginError>;

/// Errors that can occur during plugin operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    /// Plugin initialization failed
    InitializationFailed(String),
    /// Plugin is not compatible with the current editor version
    IncompatibleVersion { required: String, actual: String },
    /// A required capability is not available
    MissingCapability(String),
    /// Plugin configuration is invalid
    InvalidConfiguration(String),
    /// Plugin dependency is missing
    MissingDependency(String),
    /// Plugin is already registered
    AlreadyRegistered(String),
    /// Plugin not found
    NotFound(String),
    /// Plugin operation failed
    OperationFailed(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InitializationFailed(msg) => write!(f, "Plugin initialization failed: {msg}"),
            Self::IncompatibleVersion { required, actual } => {
                write!(f, "Incompatible version: requires {required}, got {actual}")
            }
            Self::MissingCapability(cap) => write!(f, "Missing capability: {cap}"),
            Self::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {msg}"),
            Self::MissingDependency(dep) => write!(f, "Missing dependency: {dep}"),
            Self::AlreadyRegistered(name) => write!(f, "Plugin already registered: {name}"),
            Self::NotFound(name) => write!(f, "Plugin not found: {name}"),
            Self::OperationFailed(msg) => write!(f, "Operation failed: {msg}"),
        }
    }
}

impl std::error::Error for PluginError {}

bitflags! {
    /// Capabilities that a plugin can provide.
    ///
    /// Plugins declare their capabilities to enable feature-based loading
    /// and to provide hints for the plugin registry.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct PluginCapabilities: u32 {
        /// Plugin provides custom commands for the command palette
        const COMMANDS = 1 << 0;
        /// Plugin provides custom pane types
        const PANES = 1 << 1;
        /// Plugin provides custom keyboard shortcuts
        const KEYBOARD = 1 << 2;
        /// Plugin hooks into theme changes
        const THEMING = 1 << 3;
        /// Plugin provides custom visualizations
        const VISUALIZATIONS = 1 << 4;
        /// Plugin provides data source integrations
        const DATA_SOURCES = 1 << 5;
        /// Plugin provides agent commands (AI integration)
        const AGENT_COMMANDS = 1 << 6;
        /// Plugin provides status line segments
        const STATUS_LINE = 1 << 7;
        /// Plugin provides finder sources (for unified finder)
        const FINDER_SOURCES = 1 << 8;
        /// Plugin provides custom color themes
        const CUSTOM_THEMES = 1 << 9;
    }
}

/// Configuration for a custom command provided by a plugin.
#[derive(Debug, Clone)]
pub struct CommandConfig {
    /// Command name (e.g., "my-command")
    pub name: String,
    /// Aliases for the command (e.g., ["mc", "mycmd"])
    pub aliases: Vec<String>,
    /// Human-readable description
    pub description: String,
    /// Whether the command accepts arguments
    pub accepts_args: bool,
}

/// Configuration for a custom pane type provided by a plugin.
#[derive(Debug, Clone)]
pub struct PaneConfig {
    /// Pane type name (e.g., "custom-chart")
    pub name: String,
    /// Human-readable label for the pane type
    pub label: String,
    /// Description shown in pane creation UI
    pub description: String,
    /// Icon identifier (optional)
    pub icon: Option<String>,
}

/// Configuration for a keyboard shortcut provided by a plugin.
#[derive(Debug, Clone)]
pub struct KeybindingConfig {
    /// Key sequence (e.g., "Space+p+c" for leader+p+c)
    pub keys: String,
    /// Command to execute when triggered
    pub command: String,
    /// Description shown in which-key overlay
    pub description: String,
    /// Modes where this binding is active (empty = all modes)
    pub modes: Vec<String>,
}

/// The core plugin trait that all plugins must implement.
///
/// This trait defines the interface for plugin lifecycle management,
/// capability declaration, and hook registration.
pub trait Plugin: Any + Send + Sync {
    /// Returns the unique name of the plugin.
    ///
    /// This name is used for identification and should be kebab-case
    /// (e.g., "git-blame", "custom-charts").
    fn name(&self) -> &'static str;

    /// Returns the version of the plugin (semver format).
    fn version(&self) -> &'static str;

    /// Returns a human-readable description of the plugin.
    fn description(&self) -> &'static str {
        ""
    }

    /// Returns the capabilities this plugin provides.
    fn capabilities(&self) -> PluginCapabilities;

    /// Returns the minimum editor version required.
    fn min_editor_version(&self) -> Option<&'static str> {
        None
    }

    /// Returns names of plugins this plugin depends on.
    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    /// Initialize the plugin with the given context.
    ///
    /// This is called once when the plugin is first loaded.
    /// Return an error to prevent the plugin from loading.
    fn init(&mut self, ctx: &PluginContext) -> PluginResult<()>;

    /// Activate the plugin.
    ///
    /// This is called when the plugin is enabled (either at startup
    /// if enabled by default, or when the user enables it).
    fn activate(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        Ok(())
    }

    /// Deactivate the plugin.
    ///
    /// This is called when the plugin is disabled. Plugins should
    /// clean up any resources and remove any UI elements.
    fn deactivate(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        Ok(())
    }

    /// Returns the commands this plugin provides.
    fn commands(&self) -> Vec<CommandConfig> {
        vec![]
    }

    /// Returns the pane types this plugin provides.
    fn pane_types(&self) -> Vec<PaneConfig> {
        vec![]
    }

    /// Returns the keybindings this plugin provides.
    fn keybindings(&self) -> Vec<KeybindingConfig> {
        vec![]
    }

    /// Returns the custom themes this plugin provides.
    ///
    /// Only called if the plugin declares `CUSTOM_THEMES` capability.
    fn themes(&self) -> Vec<ThemeDefinition> {
        vec![]
    }

    /// Returns the lifecycle hooks this plugin wants to receive.
    fn lifecycle_hooks(&self) -> Option<Box<dyn LifecycleHook>> {
        None
    }

    /// Returns the command hooks this plugin wants to receive.
    fn command_hooks(&self) -> Option<Box<dyn CommandHook>> {
        None
    }

    /// Returns the keyboard hooks this plugin wants to receive.
    fn keyboard_hooks(&self) -> Option<Box<dyn KeyboardHook>> {
        None
    }

    /// Returns the theme hooks this plugin wants to receive.
    fn theme_hooks(&self) -> Option<Box<dyn ThemeHook>> {
        None
    }

    /// Returns the pane hooks this plugin wants to receive.
    fn pane_hooks(&self) -> Option<Box<dyn PaneHook>> {
        None
    }

    /// Execute a command provided by this plugin.
    ///
    /// Only called if the plugin declares `COMMANDS` capability.
    fn execute_command(&mut self, _command: &str, _args: &str, _ctx: &PluginContext) -> bool {
        false
    }

    /// Called when the theme changes.
    ///
    /// Only called if the plugin declares `THEMING` capability.
    fn on_theme_changed(&mut self, _theme: Theme) {}

    /// Get a reference to self as Any (for downcasting).
    fn as_any(&self) -> &dyn Any;

    /// Get a mutable reference to self as Any (for downcasting).
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
