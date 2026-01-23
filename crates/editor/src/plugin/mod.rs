//! Plugin system for extending Enya editor functionality.
//!
//! This module re-exports types from `enya-plugin` and provides editor-specific
//! extensions like the `PluginContext`.
//!
//! # Plugin Lifecycle
//!
//! 1. **Registration**: Plugins are registered with the `PluginRegistry`
//! 2. **Initialization**: `Plugin::init()` is called with a `PluginContext`
//! 3. **Activation**: `Plugin::activate()` is called when the plugin is enabled
//! 4. **Runtime**: Plugin hooks are called during editor operation
//! 5. **Deactivation**: `Plugin::deactivate()` is called when disabled
//!
//! # Example
//!
//! ```ignore
//! use enya_editor::plugin::{Plugin, PluginContext, PluginCapabilities};
//!
//! struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn name(&self) -> &'static str { "my-plugin" }
//!     fn version(&self) -> &'static str { "0.1.0" }
//!     fn capabilities(&self) -> PluginCapabilities {
//!         PluginCapabilities::COMMANDS | PluginCapabilities::PANES
//!     }
//!     // ... implement other methods
//! }
//! ```

mod context;

// Re-export core types from enya-plugin
pub use enya_plugin::{
    // Core types
    BoxFuture,
    // Plugin traits and types
    CommandConfig,
    // Hooks
    CommandHook,
    CommandHookResult,
    KeyCombo,
    KeyEvent,
    KeybindingConfig,
    KeyboardHook,
    KeyboardHookResult,
    LifecycleHook,
    LogLevel,
    NotificationLevel,
    PaneConfig as PluginPaneConfig,
    PaneHook,
    Plugin,
    PluginCapabilities,
    PluginError,
    PluginHost,
    PluginHostRef,
    // Registry
    PluginId,
    PluginInfo,
    PluginRegistry,
    PluginResult,
    PluginState,
    Theme,
    ThemeCustomization,
    ThemeHook,
};

// Re-export loader types (native only)
#[cfg(not(target_arch = "wasm32"))]
pub use enya_plugin::{
    ConfigCommand, ConfigKeybinding, ConfigPlugin, EXAMPLE_LUA_PLUGIN, EXAMPLE_PLUGIN, LuaPlugin,
    PluginLoader, PluginManifest, PluginMeta,
};

// Editor-specific plugin context
pub use context::{EditorPluginHost, PluginContext, PluginContextRef};
