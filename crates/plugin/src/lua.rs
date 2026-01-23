//! Lua plugin support using mlua.
//!
//! This module enables writing plugins in Lua for dynamic behavior beyond
//! what TOML config plugins allow. Lua plugins can have conditional logic,
//! access editor state, and create complex workflows.
//!
//! # Example Lua Plugin
//!
//! ```lua
//! -- ~/.config/enya/plugins/my-plugin.lua
//!
//! -- Plugin metadata (required)
//! plugin = {
//!     name = "my-lua-plugin",
//!     version = "0.1.0",
//!     description = "A Lua plugin example"
//! }
//!
//! -- Register a command
//! enya.register_command("greet", {
//!     description = "Greet the user",
//!     aliases = {"hello", "hi"},
//!     accepts_args = true
//! }, function(args)
//!     if args == "" then
//!         enya.notify("info", "Hello, World!")
//!     else
//!         enya.notify("info", "Hello, " .. args .. "!")
//!     end
//!     return true
//! end)
//!
//! -- Register a keybinding
//! enya.keymap("Space+x+g", "greet", "Greet user")
//!
//! -- Lifecycle hooks (optional)
//! function on_activate()
//!     enya.log("info", "Plugin activated!")
//! end
//!
//! function on_deactivate()
//!     enya.log("info", "Plugin deactivated!")
//! end
//! ```
//!
//! # Available API
//!
//! ## Registration Functions (available during load)
//!
//! - `enya.register_command(name, config, callback)` - Register a command
//! - `enya.keymap(keys, command, description, [modes])` - Register a keybinding
//!
//! ## Runtime Functions (available in callbacks)
//!
//! - `enya.notify(level, message)` - Show notification ("info", "warn", "error")
//! - `enya.log(level, message)` - Log a message
//! - `enya.request_repaint()` - Request UI refresh
//! - `enya.editor_version()` - Get editor version string
//! - `enya.is_wasm()` - Check if running in WASM
//! - `enya.theme_name()` - Get current theme name
//! - `enya.clipboard_write(text)` - Write text to clipboard
//! - `enya.clipboard_read()` - Read text from clipboard (returns nil if empty)
//! - `enya.execute(command, [args])` - Execute another command
//! - `enya.http_get(url, [headers])` - HTTP GET, returns `{status, body, headers}` or `{error}`
//! - `enya.http_post(url, body, [headers])` - HTTP POST, returns `{status, body, headers}` or `{error}`

use std::any::Any;
use std::path::{Path, PathBuf};

use mlua::{Function, Lua, RegistryKey, Result as LuaResult, Table, Value, Variadic};
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

use crate::theme::{ThemeBase, ThemeColors, ThemeDefinition};
use crate::traits::{CommandConfig, KeybindingConfig, Plugin, PluginCapabilities};
use crate::types::{
    CustomChartConfig, CustomTableConfig, GaugePaneConfig, LogLevel, PluginContext, StatPaneConfig,
    TableColumnConfig,
};
use crate::{PluginError, PluginResult};

/// A command registered by a Lua plugin.
struct LuaCommand {
    /// Command name
    name: String,
    /// Aliases for the command
    aliases: Vec<String>,
    /// Description
    description: String,
    /// Whether the command accepts arguments
    accepts_args: bool,
    /// Registry key for the callback function
    callback_key: RegistryKey,
}

/// Set up no-op placeholder functions on the enya table.
/// These are needed because plugins may call API functions during initial loading,
/// before the real implementations are available via setup_runtime_api.
fn setup_noop_api(lua: &Lua, enya: &Table) -> LuaResult<()> {
    // Core functions
    enya.set(
        "notify",
        lua.create_function(|_, _: (String, String)| Ok(()))?,
    )?;
    enya.set("log", lua.create_function(|_, _: (String, String)| Ok(()))?)?;
    enya.set("request_repaint", lua.create_function(|_, ()| Ok(()))?)?;
    enya.set(
        "editor_version",
        lua.create_function(|_, ()| Ok("unknown".to_string()))?,
    )?;
    enya.set("is_wasm", lua.create_function(|_, ()| Ok(false))?)?;
    enya.set(
        "theme_name",
        lua.create_function(|_, ()| Ok("unknown".to_string()))?,
    )?;
    enya.set(
        "clipboard_write",
        lua.create_function(|_, _: String| Ok(false))?,
    )?;
    enya.set(
        "clipboard_read",
        lua.create_function(|_, ()| Ok(None::<String>))?,
    )?;
    enya.set(
        "execute",
        lua.create_function(|_, _: (String, Option<String>)| Ok(false))?,
    )?;

    // HTTP functions - return error during loading phase
    enya.set(
        "http_get",
        lua.create_function(|_, _: (String, Option<Table>)| -> LuaResult<Table> {
            Err(mlua::Error::runtime(
                "HTTP not available during plugin loading",
            ))
        })?,
    )?;
    enya.set(
        "http_post",
        lua.create_function(
            |_, _: (String, String, Option<Table>)| -> LuaResult<Table> {
                Err(mlua::Error::runtime(
                    "HTTP not available during plugin loading",
                ))
            },
        )?,
    )?;

    // Pane management
    enya.set(
        "add_query_pane",
        lua.create_function(|_, _: (String, Option<String>)| Ok(()))?,
    )?;
    enya.set("add_logs_pane", lua.create_function(|_, ()| Ok(()))?)?;
    enya.set(
        "add_tracing_pane",
        lua.create_function(|_, _: Option<String>| Ok(()))?,
    )?;
    enya.set("add_terminal_pane", lua.create_function(|_, ()| Ok(()))?)?;
    enya.set("add_sql_pane", lua.create_function(|_, ()| Ok(()))?)?;
    enya.set("close_pane", lua.create_function(|_, ()| Ok(()))?)?;
    enya.set("focus_pane", lua.create_function(|_, _: String| Ok(()))?)?;

    // Time range
    enya.set(
        "set_time_range",
        lua.create_function(|_, _: String| Ok(()))?,
    )?;
    enya.set(
        "set_time_range_absolute",
        lua.create_function(|_, _: (f64, f64)| Ok(()))?,
    )?;
    enya.set(
        "get_time_range",
        lua.create_function(|lua, ()| {
            let table = lua.create_table()?;
            table.set("start", 0.0)?;
            table.set("end", 0.0)?;
            Ok(table)
        })?,
    )?;

    // Custom pane functions
    enya.set(
        "add_custom_pane",
        lua.create_function(|_, _: String| Ok(()))?,
    )?;
    enya.set(
        "set_pane_data",
        lua.create_function(|_, _: (String, Table)| Ok(()))?,
    )?;
    enya.set(
        "add_chart_pane",
        lua.create_function(|_, _: String| Ok(()))?,
    )?;
    enya.set(
        "set_chart_data",
        lua.create_function(|_, _: (String, Table)| Ok(()))?,
    )?;
    enya.set("add_stat_pane", lua.create_function(|_, _: String| Ok(()))?)?;
    enya.set(
        "set_stat_data",
        lua.create_function(|_, _: (String, Table)| Ok(()))?,
    )?;
    enya.set(
        "add_gauge_pane",
        lua.create_function(|_, _: String| Ok(()))?,
    )?;
    enya.set(
        "set_gauge_data",
        lua.create_function(|_, _: (String, Table)| Ok(()))?,
    )?;

    Ok(())
}

/// A Lua-based plugin.
pub struct LuaPlugin {
    /// The Lua state (wrapped in Mutex for Sync)
    lua: Mutex<Lua>,
    /// Plugin name (leaked for 'static lifetime)
    name: &'static str,
    /// Plugin version (leaked for 'static lifetime)
    version: &'static str,
    /// Plugin description (leaked for 'static lifetime)
    description: &'static str,
    /// Commands registered by this plugin
    commands: Vec<LuaCommand>,
    /// Keybindings registered by this plugin
    keybindings: Vec<KeybindingConfig>,
    /// Custom table pane types registered by this plugin
    table_pane_types: Vec<CustomTableConfig>,
    /// Custom chart pane types registered by this plugin
    chart_pane_types: Vec<CustomChartConfig>,
    /// Custom stat pane types registered by this plugin
    stat_pane_types: Vec<StatPaneConfig>,
    /// Custom gauge pane types registered by this plugin
    gauge_pane_types: Vec<GaugePaneConfig>,
    /// Refresh callbacks keyed by pane type name
    refresh_callbacks: FxHashMap<String, RegistryKey>,
    /// Custom theme defined by this plugin (if any)
    theme: Option<ThemeDefinition>,
    /// Path to the plugin file
    path: PathBuf,
    /// Whether the plugin is active
    active: bool,
    /// Registry key for on_activate hook (if defined)
    on_activate_key: Option<RegistryKey>,
    /// Registry key for on_deactivate hook (if defined)
    on_deactivate_key: Option<RegistryKey>,
}

impl LuaPlugin {
    /// Load a Lua plugin from a file.
    pub fn load(path: &Path) -> PluginResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to read {}: {e}", path.display()))
        })?;

        Self::load_from_source(&content, path)
    }

    /// Load a Lua plugin from source code.
    pub fn load_from_source(source: &str, path: &Path) -> PluginResult<Self> {
        let lua = Lua::new();

        // Set up the initial enya table with registration functions
        Self::setup_registration_api(&lua).map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to set up Lua API: {e}"))
        })?;

        // Execute the plugin script
        lua.load(source)
            .set_name(path.to_string_lossy())
            .exec()
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to execute {}: {e}",
                    path.display()
                ))
            })?;

        // Extract plugin metadata
        let (name, version, description) = Self::extract_metadata(&lua).map_err(|e| {
            PluginError::InvalidConfiguration(format!(
                "Failed to read plugin metadata from {}: {e}",
                path.display()
            ))
        })?;

        // Extract registered commands
        let commands = Self::extract_commands(&lua).map_err(|e| {
            PluginError::InvalidConfiguration(format!(
                "Failed to read commands from {}: {e}",
                path.display()
            ))
        })?;

        // Extract registered keybindings
        let keybindings = Self::extract_keybindings(&lua).map_err(|e| {
            PluginError::InvalidConfiguration(format!(
                "Failed to read keybindings from {}: {e}",
                path.display()
            ))
        })?;

        // Extract lifecycle hooks
        let (on_activate_key, on_deactivate_key) = Self::extract_lifecycle_hooks(&lua);

        // Extract custom theme (if defined)
        let theme = Self::extract_theme(&lua);

        // Create refresh callbacks map
        let mut refresh_callbacks = FxHashMap::default();

        // Extract custom table pane types
        let table_pane_types = Self::extract_table_pane_types(&lua, &name, &mut refresh_callbacks)
            .map_err(|e| {
                PluginError::InvalidConfiguration(format!(
                    "Failed to read table pane types from {}: {e}",
                    path.display()
                ))
            })?;

        // Extract custom chart pane types
        let chart_pane_types = Self::extract_chart_pane_types(&lua, &name, &mut refresh_callbacks)
            .map_err(|e| {
                PluginError::InvalidConfiguration(format!(
                    "Failed to read chart pane types from {}: {e}",
                    path.display()
                ))
            })?;

        // Extract custom stat pane types
        let stat_pane_types = Self::extract_stat_pane_types(&lua, &name, &mut refresh_callbacks)
            .map_err(|e| {
                PluginError::InvalidConfiguration(format!(
                    "Failed to read stat pane types from {}: {e}",
                    path.display()
                ))
            })?;

        // Extract custom gauge pane types
        let gauge_pane_types = Self::extract_gauge_pane_types(&lua, &name, &mut refresh_callbacks)
            .map_err(|e| {
                PluginError::InvalidConfiguration(format!(
                    "Failed to read gauge pane types from {}: {e}",
                    path.display()
                ))
            })?;

        Ok(Self {
            lua: Mutex::new(lua),
            name: Box::leak(name.into_boxed_str()),
            version: Box::leak(version.into_boxed_str()),
            description: Box::leak(description.into_boxed_str()),
            commands,
            keybindings,
            table_pane_types,
            chart_pane_types,
            stat_pane_types,
            gauge_pane_types,
            refresh_callbacks,
            theme,
            path: path.to_path_buf(),
            active: false,
            on_activate_key,
            on_deactivate_key,
        })
    }

    /// Set up the registration API (enya.register_command, enya.keymap).
    fn setup_registration_api(lua: &Lua) -> LuaResult<()> {
        let globals = lua.globals();

        // Create the enya table
        let enya = lua.create_table()?;

        // Storage for registered commands, keybindings, and custom panes (will be extracted later)
        let registered_commands = lua.create_table()?;
        let registered_keybindings = lua.create_table()?;
        let registered_table_panes = lua.create_table()?;
        let registered_chart_panes = lua.create_table()?;
        let registered_stat_panes = lua.create_table()?;
        let registered_gauge_panes = lua.create_table()?;

        // enya.register_command(name, config, callback)
        let commands_ref = registered_commands.clone();
        let register_command = lua.create_function(
            move |lua, (name, config, callback): (String, Table, Function)| {
                let cmd_table = lua.create_table()?;
                cmd_table.set("name", name)?;
                cmd_table.set(
                    "description",
                    config
                        .get::<Option<String>>("description")?
                        .unwrap_or_default(),
                )?;
                cmd_table.set(
                    "aliases",
                    config
                        .get::<Option<Vec<String>>>("aliases")?
                        .unwrap_or_default(),
                )?;
                cmd_table.set(
                    "accepts_args",
                    config.get::<Option<bool>>("accepts_args")?.unwrap_or(false),
                )?;
                cmd_table.set("callback", callback)?;

                let len = commands_ref.len()? + 1;
                commands_ref.set(len, cmd_table)?;

                Ok(())
            },
        )?;
        enya.set("register_command", register_command)?;

        // enya.keymap(keys, command, description, modes?)
        let keybindings_ref = registered_keybindings.clone();
        let keymap = lua.create_function(move |lua, args: Variadic<Value>| {
            let args: Vec<Value> = args.into_iter().collect();
            if args.len() < 2 {
                return Err(mlua::Error::runtime(
                    "keymap requires at least 2 arguments: keys, command",
                ));
            }

            let keys = match &args[0] {
                Value::String(s) => s.to_str()?.to_string(),
                _ => return Err(mlua::Error::runtime("keys must be a string")),
            };

            let command = match &args[1] {
                Value::String(s) => s.to_str()?.to_string(),
                _ => return Err(mlua::Error::runtime("command must be a string")),
            };

            let description = args
                .get(2)
                .and_then(|v| match v {
                    Value::String(s) => s.to_str().ok().map(|s| s.to_string()),
                    _ => None,
                })
                .unwrap_or_default();

            let modes: Vec<String> = args
                .get(3)
                .and_then(|v| match v {
                    Value::Table(t) => {
                        let modes: Vec<String> = t
                            .clone()
                            .pairs::<i64, String>()
                            .flatten()
                            .map(|(_, mode)| mode)
                            .collect();
                        Some(modes)
                    }
                    _ => None,
                })
                .unwrap_or_default();

            let kb_table = lua.create_table()?;
            kb_table.set("keys", keys)?;
            kb_table.set("command", command)?;
            kb_table.set("description", description)?;
            kb_table.set("modes", modes)?;

            let len = keybindings_ref.len()? + 1;
            keybindings_ref.set(len, kb_table)?;

            Ok(())
        })?;
        enya.set("keymap", keymap)?;

        // enya.register_table_pane(name, config, [callback]) - Register a custom table pane type
        // The optional callback is called to refresh pane data
        let table_panes_ref = registered_table_panes.clone();
        let register_table_pane = lua.create_function(move |lua, args: Variadic<Value>| {
            let args: Vec<Value> = args.into_iter().collect();
            if args.len() < 2 {
                return Err(mlua::Error::runtime(
                    "register_table_pane requires at least 2 arguments: name, config",
                ));
            }

            let name = match &args[0] {
                Value::String(s) => s.to_str()?.to_string(),
                _ => return Err(mlua::Error::runtime("name must be a string")),
            };

            let config = match &args[1] {
                Value::Table(t) => t.clone(),
                _ => return Err(mlua::Error::runtime("config must be a table")),
            };

            let pane_table = lua.create_table()?;
            pane_table.set("name", name)?;
            pane_table.set(
                "title",
                config
                    .get::<Option<String>>("title")?
                    .unwrap_or_else(|| "Custom Table".to_string()),
            )?;
            pane_table.set(
                "refresh_interval",
                config.get::<Option<u32>>("refresh_interval")?.unwrap_or(0),
            )?;

            // Extract columns array
            let columns_table = lua.create_table()?;
            if let Ok(columns) = config.get::<Table>("columns") {
                for (idx, col) in columns.pairs::<i64, Table>().flatten() {
                    let col_table = lua.create_table()?;
                    col_table.set("name", col.get::<String>("name")?)?;
                    col_table.set("key", col.get::<Option<String>>("key")?.unwrap_or_default())?;
                    col_table.set("width", col.get::<Option<f32>>("width")?.unwrap_or(0.0))?;
                    columns_table.set(idx, col_table)?;
                }
            }
            pane_table.set("columns", columns_table)?;

            // Store callback if provided
            if let Some(Value::Function(callback)) = args.get(2) {
                pane_table.set("callback", callback.clone())?;
            }

            let len = table_panes_ref.len()? + 1;
            table_panes_ref.set(len, pane_table)?;

            Ok(())
        })?;
        enya.set("register_table_pane", register_table_pane)?;

        // enya.register_chart_pane(name, config, [callback]) - Register a custom chart pane type
        let chart_panes_ref = registered_chart_panes.clone();
        let register_chart_pane = lua.create_function(move |lua, args: Variadic<Value>| {
            let args: Vec<Value> = args.into_iter().collect();
            if args.len() < 2 {
                return Err(mlua::Error::runtime(
                    "register_chart_pane requires at least 2 arguments: name, config",
                ));
            }

            let name = match &args[0] {
                Value::String(s) => s.to_str()?.to_string(),
                _ => return Err(mlua::Error::runtime("name must be a string")),
            };

            let config = match &args[1] {
                Value::Table(t) => t.clone(),
                _ => return Err(mlua::Error::runtime("config must be a table")),
            };

            let pane_table = lua.create_table()?;
            pane_table.set("name", name)?;
            pane_table.set(
                "title",
                config
                    .get::<Option<String>>("title")?
                    .unwrap_or_else(|| "Custom Chart".to_string()),
            )?;
            pane_table.set(
                "y_unit",
                config.get::<Option<String>>("y_unit")?.unwrap_or_default(),
            )?;
            pane_table.set(
                "refresh_interval",
                config.get::<Option<u32>>("refresh_interval")?.unwrap_or(0),
            )?;

            // Store callback if provided
            if let Some(Value::Function(callback)) = args.get(2) {
                pane_table.set("callback", callback.clone())?;
            }

            let len = chart_panes_ref.len()? + 1;
            chart_panes_ref.set(len, pane_table)?;

            Ok(())
        })?;
        enya.set("register_chart_pane", register_chart_pane)?;

        // enya.register_stat_pane(name, config, [callback]) - Register a custom stat pane type
        let stat_panes_ref = registered_stat_panes.clone();
        let register_stat_pane = lua.create_function(move |lua, args: Variadic<Value>| {
            let args: Vec<Value> = args.into_iter().collect();
            if args.len() < 2 {
                return Err(mlua::Error::runtime(
                    "register_stat_pane requires at least 2 arguments: name, config",
                ));
            }

            let name = match &args[0] {
                Value::String(s) => s.to_str()?.to_string(),
                _ => return Err(mlua::Error::runtime("name must be a string")),
            };

            let config = match &args[1] {
                Value::Table(t) => t.clone(),
                _ => return Err(mlua::Error::runtime("config must be a table")),
            };

            let pane_table = lua.create_table()?;
            pane_table.set("name", name)?;
            pane_table.set(
                "title",
                config
                    .get::<Option<String>>("title")?
                    .unwrap_or_else(|| "Custom Stat".to_string()),
            )?;
            pane_table.set(
                "unit",
                config.get::<Option<String>>("unit")?.unwrap_or_default(),
            )?;
            pane_table.set(
                "refresh_interval",
                config.get::<Option<u32>>("refresh_interval")?.unwrap_or(0),
            )?;

            // Store callback if provided
            if let Some(Value::Function(callback)) = args.get(2) {
                pane_table.set("callback", callback.clone())?;
            }

            let len = stat_panes_ref.len()? + 1;
            stat_panes_ref.set(len, pane_table)?;

            Ok(())
        })?;
        enya.set("register_stat_pane", register_stat_pane)?;

        // enya.register_gauge_pane(name, config, [callback]) - Register a custom gauge pane type
        let gauge_panes_ref = registered_gauge_panes.clone();
        let register_gauge_pane = lua.create_function(move |lua, args: Variadic<Value>| {
            let args: Vec<Value> = args.into_iter().collect();
            if args.len() < 2 {
                return Err(mlua::Error::runtime(
                    "register_gauge_pane requires at least 2 arguments: name, config",
                ));
            }

            let name = match &args[0] {
                Value::String(s) => s.to_str()?.to_string(),
                _ => return Err(mlua::Error::runtime("name must be a string")),
            };

            let config = match &args[1] {
                Value::Table(t) => t.clone(),
                _ => return Err(mlua::Error::runtime("config must be a table")),
            };

            let pane_table = lua.create_table()?;
            pane_table.set("name", name)?;
            pane_table.set(
                "title",
                config
                    .get::<Option<String>>("title")?
                    .unwrap_or_else(|| "Custom Gauge".to_string()),
            )?;
            pane_table.set(
                "unit",
                config.get::<Option<String>>("unit")?.unwrap_or_default(),
            )?;
            pane_table.set("min", config.get::<Option<f64>>("min")?.unwrap_or(0.0))?;
            pane_table.set("max", config.get::<Option<f64>>("max")?.unwrap_or(100.0))?;
            pane_table.set(
                "refresh_interval",
                config.get::<Option<u32>>("refresh_interval")?.unwrap_or(0),
            )?;

            // Store callback if provided
            if let Some(Value::Function(callback)) = args.get(2) {
                pane_table.set("callback", callback.clone())?;
            }

            let len = gauge_panes_ref.len()? + 1;
            gauge_panes_ref.set(len, pane_table)?;

            Ok(())
        })?;
        enya.set("register_gauge_pane", register_gauge_pane)?;

        // Store references for later extraction
        enya.set("_registered_commands", registered_commands)?;
        enya.set("_registered_keybindings", registered_keybindings)?;
        enya.set("_registered_table_panes", registered_table_panes)?;
        enya.set("_registered_chart_panes", registered_chart_panes)?;
        enya.set("_registered_stat_panes", registered_stat_panes)?;
        enya.set("_registered_gauge_panes", registered_gauge_panes)?;

        // Placeholder functions that will be overwritten by setup_runtime_api when we have context.
        // These no-ops are needed because plugins may call these functions during initial loading.
        // IMPORTANT: When adding new API functions, add both a no-op here AND the real impl in
        // setup_runtime_api to keep them in sync.
        setup_noop_api(lua, &enya)?;

        globals.set("enya", enya)?;

        Ok(())
    }

    /// Extract plugin metadata from the Lua state.
    fn extract_metadata(lua: &Lua) -> LuaResult<(String, String, String)> {
        let globals = lua.globals();

        // Look for a `plugin` table
        let plugin: Table = globals.get("plugin")?;

        let name: String = plugin.get("name")?;
        let version: String = plugin
            .get::<Option<String>>("version")?
            .unwrap_or_else(|| "0.1.0".to_string());
        let description: String = plugin
            .get::<Option<String>>("description")?
            .unwrap_or_default();

        Ok((name, version, description))
    }

    /// Extract registered commands from the Lua state.
    fn extract_commands(lua: &Lua) -> LuaResult<Vec<LuaCommand>> {
        let globals = lua.globals();
        let enya: Table = globals.get("enya")?;
        let registered: Table = enya.get("_registered_commands")?;

        let mut commands = Vec::new();

        for pair in registered.pairs::<i64, Table>() {
            let (_, cmd_table) = pair?;

            let name: String = cmd_table.get("name")?;
            let description: String = cmd_table.get("description")?;
            let aliases: Vec<String> = cmd_table.get("aliases")?;
            let accepts_args: bool = cmd_table.get("accepts_args")?;
            let callback: Function = cmd_table.get("callback")?;

            // Store callback in registry for later retrieval
            let callback_key = lua.create_registry_value(callback)?;

            commands.push(LuaCommand {
                name,
                description,
                aliases,
                accepts_args,
                callback_key,
            });
        }

        Ok(commands)
    }

    /// Extract registered keybindings from the Lua state.
    fn extract_keybindings(lua: &Lua) -> LuaResult<Vec<KeybindingConfig>> {
        let globals = lua.globals();
        let enya: Table = globals.get("enya")?;
        let registered: Table = enya.get("_registered_keybindings")?;

        let mut keybindings = Vec::new();

        for pair in registered.pairs::<i64, Table>() {
            let (_, kb_table) = pair?;

            keybindings.push(KeybindingConfig {
                keys: kb_table.get("keys")?,
                command: kb_table.get("command")?,
                description: kb_table.get("description")?,
                modes: kb_table.get("modes")?,
            });
        }

        Ok(keybindings)
    }

    /// Extract registered custom table pane types from the Lua state.
    /// Also extracts and stores refresh callbacks in the registry.
    fn extract_table_pane_types(
        lua: &Lua,
        plugin_name: &str,
        refresh_callbacks: &mut FxHashMap<String, RegistryKey>,
    ) -> LuaResult<Vec<CustomTableConfig>> {
        let globals = lua.globals();
        let enya: Table = globals.get("enya")?;
        let registered: Table = enya.get("_registered_table_panes")?;

        let mut pane_types = Vec::new();

        for pair in registered.pairs::<i64, Table>() {
            let (_, pane_table) = pair?;

            let name: String = pane_table.get("name")?;
            let title: String = pane_table.get("title")?;
            let refresh_interval: u32 = pane_table.get("refresh_interval")?;

            // Extract columns
            let columns_table: Table = pane_table.get("columns")?;
            let mut columns = Vec::new();

            for col_pair in columns_table.pairs::<i64, Table>() {
                let (_, col) = col_pair?;
                let col_name: String = col.get("name")?;
                let col_key: String = col.get("key")?;
                let col_width: f32 = col.get("width")?;

                let mut column = TableColumnConfig::new(col_name);
                if !col_key.is_empty() {
                    column = column.with_key(col_key);
                }
                if col_width > 0.0 {
                    // Clamp to valid u32 range to prevent overflow
                    let width = col_width.min(u32::MAX as f32) as u32;
                    column = column.with_width(width);
                }
                columns.push(column);
            }

            // Extract and store callback if present
            if let Ok(callback) = pane_table.get::<Function>("callback") {
                let callback_key = lua.create_registry_value(callback)?;
                refresh_callbacks.insert(name.clone(), callback_key);
            }

            pane_types.push(CustomTableConfig {
                name,
                title,
                columns,
                refresh_interval,
                plugin_name: plugin_name.to_string(),
            });
        }

        Ok(pane_types)
    }

    /// Extract registered custom chart pane types from the Lua state.
    /// Also extracts and stores refresh callbacks in the registry.
    fn extract_chart_pane_types(
        lua: &Lua,
        plugin_name: &str,
        refresh_callbacks: &mut FxHashMap<String, RegistryKey>,
    ) -> LuaResult<Vec<CustomChartConfig>> {
        let globals = lua.globals();
        let enya: Table = globals.get("enya")?;
        let registered: Table = enya.get("_registered_chart_panes")?;

        let mut pane_types = Vec::new();

        for pair in registered.pairs::<i64, Table>() {
            let (_, pane_table) = pair?;

            let name: String = pane_table.get("name")?;
            let title: String = pane_table.get("title")?;
            let y_unit: String = pane_table.get("y_unit")?;
            let refresh_interval: u32 = pane_table.get("refresh_interval")?;

            // Extract and store callback if present
            if let Ok(callback) = pane_table.get::<Function>("callback") {
                let callback_key = lua.create_registry_value(callback)?;
                refresh_callbacks.insert(name.clone(), callback_key);
            }

            pane_types.push(CustomChartConfig {
                name,
                title,
                y_unit: if y_unit.is_empty() {
                    None
                } else {
                    Some(y_unit)
                },
                refresh_interval,
                plugin_name: plugin_name.to_string(),
            });
        }

        Ok(pane_types)
    }

    /// Extract registered custom stat pane types from the Lua state.
    /// Also extracts and stores refresh callbacks in the registry.
    fn extract_stat_pane_types(
        lua: &Lua,
        plugin_name: &str,
        refresh_callbacks: &mut FxHashMap<String, RegistryKey>,
    ) -> LuaResult<Vec<StatPaneConfig>> {
        let globals = lua.globals();
        let enya: Table = globals.get("enya")?;
        let registered: Table = enya.get("_registered_stat_panes")?;

        let mut pane_types = Vec::new();

        for pair in registered.pairs::<i64, Table>() {
            let (_, pane_table) = pair?;

            let name: String = pane_table.get("name")?;
            let title: String = pane_table.get("title")?;
            let unit: String = pane_table.get("unit")?;
            let refresh_interval: u32 = pane_table.get("refresh_interval")?;

            // Extract and store callback if present
            if let Ok(callback) = pane_table.get::<Function>("callback") {
                let callback_key = lua.create_registry_value(callback)?;
                refresh_callbacks.insert(name.clone(), callback_key);
            }

            pane_types.push(StatPaneConfig {
                name,
                title,
                unit: if unit.is_empty() { None } else { Some(unit) },
                refresh_interval,
                plugin_name: plugin_name.to_string(),
            });
        }

        Ok(pane_types)
    }

    /// Extract registered custom gauge pane types from the Lua state.
    /// Also extracts and stores refresh callbacks in the registry.
    fn extract_gauge_pane_types(
        lua: &Lua,
        plugin_name: &str,
        refresh_callbacks: &mut FxHashMap<String, RegistryKey>,
    ) -> LuaResult<Vec<GaugePaneConfig>> {
        let globals = lua.globals();
        let enya: Table = globals.get("enya")?;
        let registered: Table = enya.get("_registered_gauge_panes")?;

        let mut pane_types = Vec::new();

        for pair in registered.pairs::<i64, Table>() {
            let (_, pane_table) = pair?;

            let name: String = pane_table.get("name")?;
            let title: String = pane_table.get("title")?;
            let unit: String = pane_table.get("unit")?;
            let min: f64 = pane_table.get("min")?;
            let max: f64 = pane_table.get("max")?;
            let refresh_interval: u32 = pane_table.get("refresh_interval")?;

            // Extract and store callback if present
            if let Ok(callback) = pane_table.get::<Function>("callback") {
                let callback_key = lua.create_registry_value(callback)?;
                refresh_callbacks.insert(name.clone(), callback_key);
            }

            pane_types.push(GaugePaneConfig {
                name,
                title,
                unit: if unit.is_empty() { None } else { Some(unit) },
                min_scaled: (min * 1_000_000.0) as i64,
                max_scaled: (max * 1_000_000.0) as i64,
                refresh_interval,
                plugin_name: plugin_name.to_string(),
            });
        }

        Ok(pane_types)
    }

    /// Extract lifecycle hook functions from the Lua state.
    fn extract_lifecycle_hooks(lua: &Lua) -> (Option<RegistryKey>, Option<RegistryKey>) {
        let globals = lua.globals();

        let on_activate_key = globals
            .get::<Function>("on_activate")
            .ok()
            .and_then(|f| lua.create_registry_value(f).ok());

        let on_deactivate_key = globals
            .get::<Function>("on_deactivate")
            .ok()
            .and_then(|f| lua.create_registry_value(f).ok());

        (on_activate_key, on_deactivate_key)
    }

    /// Extract custom theme definition from the Lua state.
    fn extract_theme(lua: &Lua) -> Option<ThemeDefinition> {
        let globals = lua.globals();

        // Look for a `theme` table
        let theme_table: Table = globals.get("theme").ok()?;

        // Required fields
        let name: String = theme_table.get("name").ok()?;

        // Optional fields with defaults
        let display_name: String = theme_table
            .get::<Option<String>>("display_name")
            .ok()
            .flatten()
            .unwrap_or_else(|| name.clone());

        let base_str: String = theme_table
            .get::<Option<String>>("base")
            .ok()
            .flatten()
            .unwrap_or_else(|| "dark".to_string());
        let base = ThemeBase::parse(&base_str);

        // Parse colors table
        let colors = Self::extract_theme_colors(lua, &theme_table);

        Some(ThemeDefinition {
            name,
            display_name,
            base,
            colors,
        })
    }

    /// Extract color palette from theme table.
    fn extract_theme_colors(_lua: &Lua, theme_table: &Table) -> ThemeColors {
        let colors_table: Option<Table> = theme_table.get("colors").ok();
        let mut colors = ThemeColors::default();

        let Some(ct) = colors_table else {
            return colors;
        };

        // Helper to parse a color field
        let parse_color = |key: &str| -> Option<u32> {
            ct.get::<Option<String>>(key)
                .ok()
                .flatten()
                .and_then(|s| ThemeColors::parse_hex(&s))
        };

        // Backgrounds
        colors.bg_base = parse_color("bg_base");
        colors.bg_surface = parse_color("bg_surface");
        colors.bg_elevated = parse_color("bg_elevated");

        // Text
        colors.text_primary = parse_color("text_primary");
        colors.text_secondary = parse_color("text_secondary");
        colors.text_muted = parse_color("text_muted");

        // Accents
        colors.accent_primary = parse_color("accent_primary");
        colors.accent_hover = parse_color("accent_hover");
        colors.accent_muted = parse_color("accent_muted");

        // Borders
        colors.border_subtle = parse_color("border_subtle");
        colors.border_strong = parse_color("border_strong");

        // Semantic colors
        colors.success = parse_color("success");
        colors.warning = parse_color("warning");
        colors.error = parse_color("error");
        colors.info = parse_color("info");

        // Chart palette (array of hex colors)
        if let Ok(Some(chart_table)) = ct.get::<Option<Table>>("chart") {
            let mut palette = Vec::new();
            for (_, hex) in chart_table.pairs::<i64, String>().flatten() {
                if let Some(color) = ThemeColors::parse_hex(&hex) {
                    palette.push(color);
                }
            }
            colors.chart_palette = palette;
        }

        colors
    }

    /// Set up the runtime API with access to PluginContext.
    fn setup_runtime_api<'lua, 'scope>(
        lua: &'lua Lua,
        scope: &'lua mlua::Scope<'lua, 'scope>,
        ctx: &'scope PluginContext,
    ) -> LuaResult<()>
    where
        'scope: 'lua,
    {
        let globals = lua.globals();
        let enya: Table = globals.get("enya")?;

        // enya.notify(level, message)
        let notify_fn = scope.create_function(|_, (level, msg): (String, String)| {
            ctx.notify(&level, &msg);
            Ok(())
        })?;
        enya.set("notify", notify_fn)?;

        // enya.log(level, message)
        let log_fn = scope.create_function(|_, (level, msg): (String, String)| {
            let log_level = LogLevel::parse(&level);
            ctx.log(log_level, &msg);
            Ok(())
        })?;
        enya.set("log", log_fn)?;

        // enya.request_repaint()
        let repaint_fn = scope.create_function(|_, ()| {
            ctx.request_repaint();
            Ok(())
        })?;
        enya.set("request_repaint", repaint_fn)?;

        // enya.editor_version()
        let version = ctx.editor_version();
        let version_fn = scope.create_function(move |_, ()| Ok(version.to_string()))?;
        enya.set("editor_version", version_fn)?;

        // enya.is_wasm()
        let is_wasm = ctx.is_wasm();
        let wasm_fn = scope.create_function(move |_, ()| Ok(is_wasm))?;
        enya.set("is_wasm", wasm_fn)?;

        // enya.theme_name()
        let theme_name = ctx.theme_name();
        let theme_fn = scope.create_function(move |_, ()| Ok(theme_name.to_string()))?;
        enya.set("theme_name", theme_fn)?;

        // enya.clipboard_write(text)
        let clipboard_write_fn =
            scope.create_function(|_, text: String| Ok(ctx.clipboard_write(&text)))?;
        enya.set("clipboard_write", clipboard_write_fn)?;

        // enya.clipboard_read()
        let clipboard_read_fn = scope.create_function(|_, ()| Ok(ctx.clipboard_read()))?;
        enya.set("clipboard_read", clipboard_read_fn)?;

        // enya.execute(command, args?) - Execute another command
        // Note: This is a simplified version that just logs the intent
        // A full implementation would need access to the command dispatcher
        let execute_fn = scope.create_function(|_, (cmd, args): (String, Option<String>)| {
            let args_str = args.as_deref().unwrap_or("");
            log::info!("[lua] Execute request: {cmd} {args_str}");
            // For now, we can't actually execute commands from within Lua
            // This would require deeper integration with the command system
            Ok(true)
        })?;
        enya.set("execute", execute_fn)?;

        // enya.http_get(url, headers?) - Perform HTTP GET request
        // Returns { status = 200, body = "...", headers = {...} } or { error = "..." }
        let http_get_fn =
            scope.create_function(|lua, (url, headers): (String, Option<Table>)| {
                use rustc_hash::FxHashMap;

                let mut header_map = FxHashMap::default();
                if let Some(h) = headers {
                    for pair in h.pairs::<String, String>() {
                        let (k, v) = pair?;
                        header_map.insert(k, v);
                    }
                }

                let result = ctx.http_get(&url, &header_map);
                let response_table = lua.create_table()?;

                match result {
                    Ok(resp) => {
                        response_table.set("status", resp.status)?;
                        response_table.set("body", resp.body)?;
                        let headers_table = lua.create_table()?;
                        for (k, v) in resp.headers {
                            headers_table.set(k, v)?;
                        }
                        response_table.set("headers", headers_table)?;
                    }
                    Err(e) => {
                        response_table.set("error", e.message)?;
                    }
                }

                Ok(response_table)
            })?;
        enya.set("http_get", http_get_fn)?;

        // enya.http_post(url, body, headers?) - Perform HTTP POST request
        // Returns { status = 200, body = "...", headers = {...} } or { error = "..." }
        let http_post_fn = scope.create_function(
            |lua, (url, body, headers): (String, String, Option<Table>)| {
                use rustc_hash::FxHashMap;

                let mut header_map = FxHashMap::default();
                if let Some(h) = headers {
                    for pair in h.pairs::<String, String>() {
                        let (k, v) = pair?;
                        header_map.insert(k, v);
                    }
                }

                let result = ctx.http_post(&url, &body, &header_map);
                let response_table = lua.create_table()?;

                match result {
                    Ok(resp) => {
                        response_table.set("status", resp.status)?;
                        response_table.set("body", resp.body)?;
                        let headers_table = lua.create_table()?;
                        for (k, v) in resp.headers {
                            headers_table.set(k, v)?;
                        }
                        response_table.set("headers", headers_table)?;
                    }
                    Err(e) => {
                        response_table.set("error", e.message)?;
                    }
                }

                Ok(response_table)
            },
        )?;
        enya.set("http_post", http_post_fn)?;

        // ==================== Pane Management API ====================

        // enya.add_query_pane(query, [title]) - Add a query pane with PromQL query
        let add_query_pane_fn =
            scope.create_function(|_, (query, title): (String, Option<String>)| {
                ctx.add_query_pane(&query, title.as_deref());
                Ok(())
            })?;
        enya.set("add_query_pane", add_query_pane_fn)?;

        // enya.add_logs_pane() - Add a logs pane
        let add_logs_pane_fn = scope.create_function(|_, ()| {
            ctx.add_logs_pane();
            Ok(())
        })?;
        enya.set("add_logs_pane", add_logs_pane_fn)?;

        // enya.add_tracing_pane([trace_id]) - Add a tracing pane, optionally with a trace ID
        let add_tracing_pane_fn = scope.create_function(|_, trace_id: Option<String>| {
            ctx.add_tracing_pane(trace_id.as_deref());
            Ok(())
        })?;
        enya.set("add_tracing_pane", add_tracing_pane_fn)?;

        // enya.add_terminal_pane() - Add a terminal pane (native only)
        let add_terminal_pane_fn = scope.create_function(|_, ()| {
            ctx.add_terminal_pane();
            Ok(())
        })?;
        enya.set("add_terminal_pane", add_terminal_pane_fn)?;

        // enya.add_sql_pane() - Add a SQL pane
        let add_sql_pane_fn = scope.create_function(|_, ()| {
            ctx.add_sql_pane();
            Ok(())
        })?;
        enya.set("add_sql_pane", add_sql_pane_fn)?;

        // enya.close_pane() - Close the focused pane
        let close_pane_fn = scope.create_function(|_, ()| {
            ctx.close_focused_pane();
            Ok(())
        })?;
        enya.set("close_pane", close_pane_fn)?;

        // enya.focus_pane(direction) - Focus pane in direction ("left", "right", "up", "down")
        let focus_pane_fn = scope.create_function(|_, direction: String| {
            ctx.focus_pane(&direction);
            Ok(())
        })?;
        enya.set("focus_pane", focus_pane_fn)?;

        // ==================== Time Range API ====================

        // enya.set_time_range(preset) - Set time range to preset ("5m", "1h", "24h", etc.)
        let set_time_range_fn = scope.create_function(|_, preset: String| {
            ctx.set_time_range_preset(&preset);
            Ok(())
        })?;
        enya.set("set_time_range", set_time_range_fn)?;

        // enya.set_time_range_absolute(start_secs, end_secs) - Set absolute time range
        let set_time_range_absolute_fn = scope.create_function(|_, (start, end): (f64, f64)| {
            ctx.set_time_range_absolute(start, end);
            Ok(())
        })?;
        enya.set("set_time_range_absolute", set_time_range_absolute_fn)?;

        // enya.get_time_range() - Get current time range as {start, end}
        let get_time_range_fn = scope.create_function(|lua, ()| {
            let (start, end) = ctx.get_time_range();
            let table = lua.create_table()?;
            table.set("start", start)?;
            table.set("end", end)?;
            Ok(table)
        })?;
        enya.set("get_time_range", get_time_range_fn)?;

        // ==================== Custom Pane API ====================

        // enya.add_custom_pane(pane_type) - Add an instance of a custom table pane
        let add_custom_pane_fn = scope.create_function(|_, pane_type: String| {
            ctx.add_custom_table_pane(&pane_type);
            Ok(())
        })?;
        enya.set("add_custom_pane", add_custom_pane_fn)?;

        // enya.set_pane_data(pane_type, data) - Update data for all instances of a custom table pane type
        // data = { rows = { {col1 = "value1", col2 = "value2"}, {...} } } or { error = "message" }
        let set_pane_data_fn = scope.create_function(|_, (pane_type, data): (String, Table)| {
            use crate::{CustomTableData, CustomTableRow};

            // Check for error field
            if let Ok(Some(error)) = data.get::<Option<String>>("error") {
                let table_data = CustomTableData::with_error(error);
                ctx.update_custom_table_data_by_type(&pane_type, table_data);
                return Ok(());
            }

            // Extract rows
            let mut rows = Vec::new();
            if let Ok(rows_table) = data.get::<Table>("rows") {
                for (_, row_table) in rows_table.pairs::<i64, Table>().flatten() {
                    let mut row = CustomTableRow::new();
                    for (key, value) in row_table.pairs::<String, String>().flatten() {
                        row = row.with_cell(key, value);
                    }
                    rows.push(row);
                }
            }

            let table_data = CustomTableData::with_rows(rows);
            ctx.update_custom_table_data_by_type(&pane_type, table_data);
            Ok(())
        })?;
        enya.set("set_pane_data", set_pane_data_fn)?;

        // ==================== Custom Chart Pane API ====================

        // enya.add_chart_pane(pane_type) - Add an instance of a custom chart pane
        let add_chart_pane_fn = scope.create_function(|_, pane_type: String| {
            ctx.add_custom_chart_pane(&pane_type);
            Ok(())
        })?;
        enya.set("add_chart_pane", add_chart_pane_fn)?;

        // enya.set_chart_data(pane_type, data) - Update data for all instances of a custom chart pane type
        // data = {
        //   series = {
        //     { name = "Series1", tags = { key = "value" }, points = { { timestamp = 1234567890, value = 123.4 }, ... } },
        //     ...
        //   }
        // } or { error = "message" }
        let set_chart_data_fn =
            scope.create_function(|_, (pane_type, data): (String, Table)| {
                use crate::{ChartDataPoint, ChartSeries, CustomChartData};

                // Check for error field
                if let Ok(Some(error)) = data.get::<Option<String>>("error") {
                    let chart_data = CustomChartData::with_error(error);
                    ctx.update_custom_chart_data_by_type(&pane_type, chart_data);
                    return Ok(());
                }

                // Extract series
                let mut series_vec = Vec::new();
                if let Ok(series_table) = data.get::<Table>("series") {
                    for (_, series_entry) in series_table.pairs::<i64, Table>().flatten() {
                        let name: String = series_entry
                            .get::<Option<String>>("name")?
                            .unwrap_or_else(|| "unnamed".to_string());

                        let mut series = ChartSeries::new(name);

                        // Extract tags
                        if let Ok(tags_table) = series_entry.get::<Table>("tags") {
                            for (key, value) in tags_table.pairs::<String, String>().flatten() {
                                series = series.with_tag(key, value);
                            }
                        }

                        // Extract points
                        if let Ok(points_table) = series_entry.get::<Table>("points") {
                            for (_, point_entry) in points_table.pairs::<i64, Table>().flatten() {
                                let timestamp: f64 = point_entry.get("timestamp")?;
                                let value: f64 = point_entry.get("value")?;
                                series.points.push(ChartDataPoint::new(timestamp, value));
                            }
                        }

                        series_vec.push(series);
                    }
                }

                let chart_data = CustomChartData::with_series(series_vec);
                ctx.update_custom_chart_data_by_type(&pane_type, chart_data);
                Ok(())
            })?;
        enya.set("set_chart_data", set_chart_data_fn)?;

        // ==================== Custom Stat Pane API ====================

        // enya.add_stat_pane(pane_type) - Add an instance of a custom stat pane
        let add_stat_pane_fn = scope.create_function(|_, pane_type: String| {
            ctx.add_stat_pane(&pane_type);
            Ok(())
        })?;
        enya.set("add_stat_pane", add_stat_pane_fn)?;

        // enya.set_stat_data(pane_type, data) - Update data for all instances of a custom stat pane type
        // data = {
        //   value = 123.4,                        -- The current value
        //   sparkline = { 1, 2, 3, 4, 5 },       -- Optional sparkline history
        //   change_value = 5.2,                   -- Optional % change
        //   change_period = "vs last hour",       -- Optional change period description
        //   thresholds = {                        -- Optional thresholds
        //     { value = 50, color = "yellow" },
        //     { value = 80, color = "red" }
        //   }
        // } or { error = "message" }
        let set_stat_data_fn = scope.create_function(|_, (pane_type, data): (String, Table)| {
            use crate::{StatPaneData, ThresholdConfig};

            // Check for error field
            if let Ok(Some(error)) = data.get::<Option<String>>("error") {
                let stat_data = StatPaneData::with_error(error);
                ctx.update_stat_data_by_type(&pane_type, stat_data);
                return Ok(());
            }

            let value: f64 = data.get::<Option<f64>>("value")?.unwrap_or(0.0);
            let mut stat_data = StatPaneData::with_value(value);

            // Extract sparkline
            if let Ok(sparkline_table) = data.get::<Table>("sparkline") {
                let mut sparkline = Vec::new();
                for (_, val) in sparkline_table.pairs::<i64, f64>().flatten() {
                    sparkline.push(val);
                }
                stat_data.sparkline = sparkline;
            }

            // Extract change
            if let Ok(Some(change_value)) = data.get::<Option<f64>>("change_value") {
                let change_period: String = data
                    .get::<Option<String>>("change_period")?
                    .unwrap_or_else(|| "vs last period".to_string());
                stat_data.change_value = Some(change_value);
                stat_data.change_period = Some(change_period);
            }

            // Extract thresholds
            if let Ok(thresholds_table) = data.get::<Table>("thresholds") {
                for (_, thresh) in thresholds_table.pairs::<i64, Table>().flatten() {
                    let value: f64 = thresh.get("value")?;
                    let color: String = thresh.get("color")?;
                    let label: Option<String> = thresh.get::<Option<String>>("label")?;
                    let mut threshold = ThresholdConfig::new(value, color);
                    if let Some(lbl) = label {
                        threshold = threshold.with_label(lbl);
                    }
                    stat_data.thresholds.push(threshold);
                }
            }

            ctx.update_stat_data_by_type(&pane_type, stat_data);
            Ok(())
        })?;
        enya.set("set_stat_data", set_stat_data_fn)?;

        // ==================== Custom Gauge Pane API ====================

        // enya.add_gauge_pane(pane_type) - Add an instance of a custom gauge pane
        let add_gauge_pane_fn = scope.create_function(|_, pane_type: String| {
            ctx.add_gauge_pane(&pane_type);
            Ok(())
        })?;
        enya.set("add_gauge_pane", add_gauge_pane_fn)?;

        // enya.set_gauge_data(pane_type, data) - Update data for all instances of a custom gauge pane type
        // data = {
        //   value = 75.5,                         -- The current value
        //   thresholds = {                        -- Optional thresholds
        //     { value = 50, color = "yellow" },
        //     { value = 80, color = "red" }
        //   }
        // } or { error = "message" }
        let set_gauge_data_fn =
            scope.create_function(|_, (pane_type, data): (String, Table)| {
                use crate::{GaugePaneData, ThresholdConfig};

                // Check for error field
                if let Ok(Some(error)) = data.get::<Option<String>>("error") {
                    let gauge_data = GaugePaneData::with_error(error);
                    ctx.update_gauge_data_by_type(&pane_type, gauge_data);
                    return Ok(());
                }

                let value: f64 = data.get::<Option<f64>>("value")?.unwrap_or(0.0);
                let mut gauge_data = GaugePaneData::with_value(value);

                // Extract thresholds
                if let Ok(thresholds_table) = data.get::<Table>("thresholds") {
                    for (_, thresh) in thresholds_table.pairs::<i64, Table>().flatten() {
                        let value: f64 = thresh.get("value")?;
                        let color: String = thresh.get("color")?;
                        let label: Option<String> = thresh.get::<Option<String>>("label")?;
                        let mut threshold = ThresholdConfig::new(value, color);
                        if let Some(lbl) = label {
                            threshold = threshold.with_label(lbl);
                        }
                        gauge_data.thresholds.push(threshold);
                    }
                }

                ctx.update_gauge_data_by_type(&pane_type, gauge_data);
                Ok(())
            })?;
        enya.set("set_gauge_data", set_gauge_data_fn)?;

        Ok(())
    }

    /// Call a lifecycle hook if it exists.
    fn call_lifecycle_hook(
        &self,
        hook_key: &Option<RegistryKey>,
        ctx: &PluginContext,
    ) -> PluginResult<()> {
        let Some(key) = hook_key else {
            return Ok(());
        };

        let lua = self.lua.lock();

        lua.scope(|scope| {
            Self::setup_runtime_api(&lua, scope, ctx)?;

            let hook: Function = lua.registry_value(key)?;
            hook.call::<()>(())?;

            Ok(())
        })
        .map_err(|e| PluginError::OperationFailed(format!("Lifecycle hook failed: {e}")))
    }
}

impl Plugin for LuaPlugin {
    fn name(&self) -> &'static str {
        self.name
    }

    fn version(&self) -> &'static str {
        self.version
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn capabilities(&self) -> PluginCapabilities {
        let mut caps = PluginCapabilities::empty();
        if !self.commands.is_empty() {
            caps |= PluginCapabilities::COMMANDS;
        }
        if !self.keybindings.is_empty() {
            caps |= PluginCapabilities::KEYBOARD;
        }
        if self.theme.is_some() {
            caps |= PluginCapabilities::CUSTOM_THEMES;
        }
        if !self.table_pane_types.is_empty()
            || !self.chart_pane_types.is_empty()
            || !self.stat_pane_types.is_empty()
            || !self.gauge_pane_types.is_empty()
        {
            caps |= PluginCapabilities::PANES;
        }
        caps
    }

    fn init(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        log::info!(
            "[plugin:{}] Lua plugin loaded from {}",
            self.name,
            self.path.display()
        );
        Ok(())
    }

    fn activate(&mut self, ctx: &PluginContext) -> PluginResult<()> {
        self.active = true;

        // Call on_activate hook if defined
        if self.on_activate_key.is_some() {
            self.call_lifecycle_hook(&self.on_activate_key, ctx)?;
        }

        log::info!("[plugin:{}] Activated", self.name);
        Ok(())
    }

    fn deactivate(&mut self, ctx: &PluginContext) -> PluginResult<()> {
        // Call on_deactivate hook if defined
        if self.on_deactivate_key.is_some() {
            self.call_lifecycle_hook(&self.on_deactivate_key, ctx)?;
        }

        self.active = false;
        log::info!("[plugin:{}] Deactivated", self.name);
        Ok(())
    }

    fn commands(&self) -> Vec<CommandConfig> {
        self.commands
            .iter()
            .map(|c| CommandConfig {
                name: c.name.clone(),
                aliases: c.aliases.clone(),
                description: c.description.clone(),
                accepts_args: c.accepts_args,
            })
            .collect()
    }

    fn keybindings(&self) -> Vec<KeybindingConfig> {
        self.keybindings.clone()
    }

    fn themes(&self) -> Vec<ThemeDefinition> {
        self.theme.clone().into_iter().collect()
    }

    fn custom_table_panes(&self) -> Vec<crate::CustomTableConfig> {
        self.table_pane_types.clone()
    }

    fn custom_chart_panes(&self) -> Vec<crate::CustomChartConfig> {
        self.chart_pane_types.clone()
    }

    fn custom_stat_panes(&self) -> Vec<crate::StatPaneConfig> {
        self.stat_pane_types.clone()
    }

    fn custom_gauge_panes(&self) -> Vec<crate::GaugePaneConfig> {
        self.gauge_pane_types.clone()
    }

    fn refreshable_pane_types(&self) -> Vec<(&str, u32)> {
        let mut result = Vec::new();

        // Only include pane types that have both a refresh callback and a non-zero interval
        for config in &self.table_pane_types {
            if config.refresh_interval > 0 && self.refresh_callbacks.contains_key(&config.name) {
                result.push((config.name.as_str(), config.refresh_interval));
            }
        }
        for config in &self.chart_pane_types {
            if config.refresh_interval > 0 && self.refresh_callbacks.contains_key(&config.name) {
                result.push((config.name.as_str(), config.refresh_interval));
            }
        }
        for config in &self.stat_pane_types {
            if config.refresh_interval > 0 && self.refresh_callbacks.contains_key(&config.name) {
                result.push((config.name.as_str(), config.refresh_interval));
            }
        }
        for config in &self.gauge_pane_types {
            if config.refresh_interval > 0 && self.refresh_callbacks.contains_key(&config.name) {
                result.push((config.name.as_str(), config.refresh_interval));
            }
        }

        result
    }

    fn trigger_pane_refresh(&mut self, pane_type: &str, ctx: &PluginContext) -> bool {
        let Some(callback_key) = self.refresh_callbacks.get(pane_type) else {
            log::warn!(
                "[plugin:{}] No refresh callback for pane type '{}'",
                self.name,
                pane_type
            );
            return false;
        };

        let lua = self.lua.lock();

        let result = lua.scope(|scope| {
            // Set up runtime API with current context
            Self::setup_runtime_api(&lua, scope, ctx)?;

            // Get the callback from registry
            let callback: Function = lua.registry_value(callback_key)?;

            // Call the callback (no arguments)
            callback.call::<()>(())?;

            Ok(())
        });

        match result {
            Ok(()) => {
                log::debug!("[plugin:{}] Refreshed pane type '{}'", self.name, pane_type);
                true
            }
            Err(e) => {
                log::error!(
                    "[plugin:{}] Error refreshing pane type '{}': {e}",
                    self.name,
                    pane_type
                );
                ctx.notify("error", &format!("Plugin refresh error: {e}"));
                false
            }
        }
    }

    fn execute_command(&mut self, command: &str, args: &str, ctx: &PluginContext) -> bool {
        // Find the command
        let cmd = self
            .commands
            .iter()
            .find(|c| c.name == command || c.aliases.contains(&command.to_string()));

        let Some(cmd) = cmd else {
            return false;
        };

        let lua = self.lua.lock();

        // Use scope to create functions with non-'static lifetime
        let result = lua.scope(|scope| {
            // Set up runtime API with current context
            Self::setup_runtime_api(&lua, scope, ctx)?;

            // Get the callback from registry
            let callback: Function = lua.registry_value(&cmd.callback_key)?;

            // Call the callback with args
            let success: bool = callback.call(args)?;

            Ok(success)
        });

        match result {
            Ok(success) => success,
            Err(e) => {
                log::error!(
                    "[plugin:{}] Error executing command '{}': {e}",
                    self.name,
                    command
                );
                ctx.notify("error", &format!("Plugin error: {e}"));
                false
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Example Lua plugin template.
pub const EXAMPLE_LUA_PLUGIN: &str = r#"-- Example Enya Lua Plugin
-- Place this file in ~/.config/enya/plugins/

-- Plugin metadata (required)
plugin = {
    name = "example-lua",
    version = "0.1.0",
    description = "An example Lua plugin showing available features"
}

-- Register a simple command
enya.register_command("lua-hello", {
    description = "Say hello from Lua",
    aliases = {"lhello"},
    accepts_args = true
}, function(args)
    if args == "" then
        enya.notify("info", "Hello from Lua!")
    else
        enya.notify("info", "Hello, " .. args .. "!")
    end
    return true
end)

-- Register a command with conditional logic
enya.register_command("lua-check", {
    description = "Check something with conditional logic",
    accepts_args = true
}, function(args)
    local num = tonumber(args)
    if num == nil then
        enya.notify("error", "Please provide a number")
        return false
    end

    if num > 100 then
        enya.notify("warn", "That's a large number: " .. tostring(num))
    elseif num < 0 then
        enya.notify("error", "Negative numbers not allowed")
        return false
    else
        enya.notify("info", "Number " .. tostring(num) .. " is valid")
    end

    return true
end)

-- Register keybindings
enya.keymap("Space+l+h", "lua-hello", "Lua hello")
enya.keymap("Space+l+c", "lua-check", "Lua check")

-- Lifecycle hooks (optional)
function on_activate()
    enya.log("info", "Example Lua plugin activated!")
end

function on_deactivate()
    enya.log("info", "Example Lua plugin deactivated!")
end
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_simple_plugin() {
        let source = r#"
            plugin = {
                name = "test-plugin",
                version = "1.0.0",
                description = "A test plugin"
            }

            enya.register_command("test-cmd", {
                description = "Test command",
                aliases = {"tc"},
                accepts_args = false
            }, function(args)
                return true
            end)
        "#;

        let plugin = LuaPlugin::load_from_source(source, &PathBuf::from("test.lua")).unwrap();

        assert_eq!(plugin.name(), "test-plugin");
        assert_eq!(plugin.version(), "1.0.0");
        assert_eq!(plugin.description(), "A test plugin");
        assert_eq!(plugin.commands().len(), 1);
        assert_eq!(plugin.commands()[0].name, "test-cmd");
    }

    #[test]
    fn test_keybindings() {
        let source = r#"
            plugin = { name = "kb-test" }

            enya.register_command("my-cmd", {}, function() return true end)
            enya.keymap("Space+t+t", "my-cmd", "Test binding", {"normal"})
        "#;

        let plugin = LuaPlugin::load_from_source(source, &PathBuf::from("test.lua")).unwrap();

        assert_eq!(plugin.keybindings().len(), 1);
        assert_eq!(plugin.keybindings()[0].keys, "Space+t+t");
        assert_eq!(plugin.keybindings()[0].command, "my-cmd");
        assert_eq!(plugin.keybindings()[0].modes, vec!["normal"]);
    }

    #[test]
    fn test_missing_metadata() {
        let source = r#"
            -- No plugin table defined
            enya.register_command("test", {}, function() return true end)
        "#;

        let result = LuaPlugin::load_from_source(source, &PathBuf::from("test.lua"));
        assert!(result.is_err());
    }

    #[test]
    fn test_syntax_error() {
        let source = r#"
            plugin = { name = "broken"
            -- Missing closing brace
        "#;

        let result = LuaPlugin::load_from_source(source, &PathBuf::from("test.lua"));
        assert!(result.is_err());
    }

    #[test]
    fn test_theme_plugin() {
        let source = r##"
            plugin = {
                name = "tokyo-night-theme",
                version = "1.0.0",
                description = "Tokyo Night color theme"
            }

            theme = {
                name = "tokyo-night",
                display_name = "Tokyo Night",
                base = "dark",
                colors = {
                    bg_base = "#1a1b26",
                    bg_surface = "#24283b",
                    accent_primary = "#7aa2f7",
                    accent_hover = "#89b4fa",
                    success = "#9ece6a",
                    error = "#f7768e",
                    chart = {
                        "#7aa2f7",
                        "#9ece6a",
                        "#e0af68",
                        "#f7768e",
                    }
                }
            }
        "##;

        let plugin = LuaPlugin::load_from_source(source, &PathBuf::from("test.lua")).unwrap();

        assert_eq!(plugin.name(), "tokyo-night-theme");

        // Check capabilities include CUSTOM_THEMES
        assert!(
            plugin
                .capabilities()
                .contains(PluginCapabilities::CUSTOM_THEMES)
        );

        // Check theme was parsed
        let themes = plugin.themes();
        assert_eq!(themes.len(), 1);

        let theme = &themes[0];
        assert_eq!(theme.name, "tokyo-night");
        assert_eq!(theme.display_name, "Tokyo Night");
        assert_eq!(theme.base, ThemeBase::Dark);

        // Check colors
        assert_eq!(theme.colors.bg_base, Some(0x1a1b26));
        assert_eq!(theme.colors.bg_surface, Some(0x24283b));
        assert_eq!(theme.colors.accent_primary, Some(0x7aa2f7));
        assert_eq!(theme.colors.success, Some(0x9ece6a));
        assert_eq!(theme.colors.chart_palette.len(), 4);
    }

    #[test]
    fn test_theme_with_light_base() {
        let source = r##"
            plugin = { name = "light-theme" }

            theme = {
                name = "my-light",
                base = "light",
                colors = {
                    bg_base = "#ffffff",
                    text_primary = "#000000"
                }
            }
        "##;

        let plugin = LuaPlugin::load_from_source(source, &PathBuf::from("test.lua")).unwrap();

        let themes = plugin.themes();
        assert_eq!(themes.len(), 1);
        assert_eq!(themes[0].base, ThemeBase::Light);
        assert_eq!(themes[0].colors.bg_base, Some(0xffffff));
        assert_eq!(themes[0].colors.text_primary, Some(0x000000));
    }

    #[test]
    fn test_plugin_without_theme() {
        let source = r#"
            plugin = { name = "no-theme" }

            enya.register_command("test", {}, function() return true end)
        "#;

        let plugin = LuaPlugin::load_from_source(source, &PathBuf::from("test.lua")).unwrap();

        // Should not have CUSTOM_THEMES capability
        assert!(
            !plugin
                .capabilities()
                .contains(PluginCapabilities::CUSTOM_THEMES)
        );

        // themes() should return empty
        assert!(plugin.themes().is_empty());
    }
}
