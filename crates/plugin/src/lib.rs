//! Enya Plugin System
//!
//! This crate provides a neovim-style plugin system for extending editor functionality.
//! It is designed to be decoupled from any specific editor implementation through
//! the `PluginHost` trait.
//!
//! # Overview
//!
//! The plugin system supports three types of plugins:
//!
//! - **Config plugins** (`.toml`) - Simple plugins defined in TOML
//! - **Lua plugins** (`.lua`) - Dynamic plugins using Lua scripting
//! - **Native plugins** - Rust plugins implementing the `Plugin` trait
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                     Host Application                     │
//! │  (implements PluginHost trait)                          │
//! └─────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │                     PluginContext                        │
//! │  (provides host services to plugins)                    │
//! └─────────────────────────────────────────────────────────┘
//!                            │
//!                            ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │                    PluginRegistry                        │
//! │  (manages plugin lifecycle)                             │
//! └─────────────────────────────────────────────────────────┘
//!                            │
//!           ┌────────────────┼────────────────┐
//!           ▼                ▼                ▼
//!     ┌──────────┐    ┌──────────┐    ┌──────────┐
//!     │  Config  │    │   Lua    │    │  Native  │
//!     │  Plugin  │    │  Plugin  │    │  Plugin  │
//!     └──────────┘    └──────────┘    └──────────┘
//! ```
//!
//! # Example: Implementing PluginHost
//!
//! ```ignore
//! use enya_plugin::{
//!     PluginHost, PluginContext, PluginRegistry, NotificationLevel, LogLevel, Theme, BoxFuture,
//! };
//! use std::sync::Arc;
//!
//! struct MyEditor {
//!     // ... editor state
//! }
//!
//! impl PluginHost for MyEditor {
//!     fn notify(&self, level: NotificationLevel, message: &str) {
//!         // Show notification to user
//!     }
//!
//!     fn request_repaint(&self) {
//!         // Request UI refresh
//!     }
//!
//!     fn log(&self, level: LogLevel, message: &str) {
//!         // Log message
//!     }
//!
//!     fn version(&self) -> &'static str {
//!         "1.0.0"
//!     }
//!
//!     fn is_wasm(&self) -> bool {
//!         false
//!     }
//!
//!     fn theme(&self) -> Theme {
//!         Theme::Dark
//!     }
//!
//!     fn spawn(&self, future: BoxFuture<()>) {
//!         // Spawn async task
//!     }
//! }
//!
//! fn setup_plugins(editor: Arc<MyEditor>) {
//!     let ctx = PluginContext::new(editor);
//!     let mut registry = PluginRegistry::new();
//!     registry.init(ctx);
//!
//!     // Register and activate plugins...
//! }
//! ```

mod hooks;
mod registry;
mod theme;
mod traits;
mod types;

// Loader and Lua support only available on native platforms
#[cfg(not(target_arch = "wasm32"))]
mod loader;
#[cfg(not(target_arch = "wasm32"))]
pub mod lua;

// Re-export core types
pub use types::{
    BoxFuture, ChartDataPoint, ChartSeries, CustomChartConfig, CustomChartData, CustomTableConfig,
    CustomTableData, CustomTableRow, FocusedPaneInfo, GaugePaneConfig, GaugePaneData, HttpError,
    HttpResponse, LogLevel, NotificationLevel, PluginContext, PluginContextRef, PluginHost,
    PluginHostRef, StatPaneConfig, StatPaneData, TableColumnConfig, Theme, ThresholdConfig,
};

// Re-export theme types
pub use theme::{ThemeBase, ThemeColors, ThemeDefinition};

// Re-export plugin traits and types
pub use traits::{
    CommandConfig, KeybindingConfig, PaneConfig, Plugin, PluginCapabilities, PluginError,
    PluginResult,
};

// Re-export hooks
pub use hooks::{
    CommandHook, CommandHookResult, KeyCombo, KeyEvent, KeyboardHook, KeyboardHookResult,
    LifecycleHook, PaneHook, ThemeCustomization, ThemeHook,
};

// Re-export registry
pub use registry::{PluginId, PluginInfo, PluginRegistry, PluginState};

// Re-export loader (native only)
#[cfg(not(target_arch = "wasm32"))]
pub use loader::{
    ConfigCommand, ConfigKeybinding, ConfigPlugin, EXAMPLE_PLUGIN, PluginLoader, PluginManifest,
    PluginMeta,
};

// Re-export Lua plugin support (native only)
#[cfg(not(target_arch = "wasm32"))]
pub use lua::{EXAMPLE_LUA_PLUGIN, LuaPlugin};
