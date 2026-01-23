# enya-plugin

A host-agnostic plugin system for extending application functionality, inspired by neovim's plugin architecture.

## Overview

This crate provides the core infrastructure for a plugin system that can be embedded in any Rust application. It's designed to be completely decoupled from any specific host implementation through the `PluginHost` trait.

### Key Features

- **Host-agnostic**: Works with any application that implements `PluginHost`
- **Lua scripting**: Write plugins in Lua with conditional logic, validation, and HTTP requests
- **Native plugins**: Implement the `Plugin` trait in Rust for maximum performance
- **Capability-based**: Plugins declare what features they provide
- **Hook system**: Intercept and extend host behavior
- **Custom themes**: Plugins can define complete color schemes
- **Lifecycle management**: Full control over plugin initialization, activation, and cleanup

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                 Host Application                      │
│            (implements PluginHost trait)             │
└──────────────────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────┐
│                   PluginContext                       │
│           (provides host services to plugins)        │
└──────────────────────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────┐
│                   PluginRegistry                      │
│             (manages plugin lifecycle)               │
└──────────────────────────────────────────────────────┘
                         │
               ┌─────────┴─────────┐
               ▼                   ▼
         ┌──────────┐        ┌──────────┐
         │   Lua    │        │  Native  │
         │ (.lua)   │        │  (Rust)  │
         └──────────┘        └──────────┘
```

## Quick Start

### 1. Implement `PluginHost` for your application

```rust
use enya_plugin::{
    PluginHost, PluginContext, PluginRegistry,
    NotificationLevel, LogLevel, Theme, BoxFuture,
    HttpResponse, HttpError,
};
use rustc_hash::FxHashMap;
use std::sync::Arc;

struct MyApp {
    // ... your application state
}

impl PluginHost for MyApp {
    fn notify(&self, level: NotificationLevel, message: &str) {
        println!("[{:?}] {}", level, message);
    }

    fn request_repaint(&self) {
        // Trigger UI refresh
    }

    fn log(&self, level: LogLevel, message: &str) {
        println!("[{:?}] {}", level, message);
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn is_wasm(&self) -> bool {
        false
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn theme_name(&self) -> &'static str {
        "dark"
    }

    fn clipboard_write(&self, text: &str) -> bool {
        // Write to system clipboard
        true
    }

    fn clipboard_read(&self) -> Option<String> {
        // Read from system clipboard
        None
    }

    fn spawn(&self, future: BoxFuture<()>) {
        // Spawn async task
        tokio::spawn(future);
    }

    fn http_get(
        &self,
        url: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError> {
        // Perform HTTP GET
        Err(HttpError { message: "Not implemented".to_string() })
    }

    fn http_post(
        &self,
        url: &str,
        body: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError> {
        // Perform HTTP POST
        Err(HttpError { message: "Not implemented".to_string() })
    }
}
```

### 2. Set up the plugin registry

```rust
use enya_plugin::{PluginContext, PluginRegistry};

fn setup_plugins(app: Arc<MyApp>) {
    // Create context from host
    let ctx = PluginContext::new(app);

    // Create and initialize registry
    let mut registry = PluginRegistry::new();
    registry.init(ctx);

    // Load Lua plugins from filesystem (native only)
    #[cfg(not(target_arch = "wasm32"))]
    {
        use enya_plugin::PluginLoader;

        let loader = PluginLoader::new();

        for result in loader.load_all_lua() {
            match result {
                Ok(plugin) => {
                    let id = registry.register(plugin, true).unwrap();
                    registry.init_plugin(id).unwrap();
                    registry.activate_plugin(id).unwrap();
                }
                Err(e) => eprintln!("Failed to load Lua plugin: {}", e),
            }
        }
    }
}
```

### 3. Execute plugin commands

```rust
// Execute a command provided by any active plugin
if registry.execute_command("my-command", "arg1 arg2") {
    println!("Command handled by plugin");
}

// Get all commands from active plugins
for (plugin_info, command) in registry.all_commands() {
    println!("{}: {} - {}", plugin_info.name, command.name, command.description);
}
```

## Lua Plugins

Lua plugins provide a dynamic scripting environment with full access to conditional logic, input validation, and HTTP requests.

### Basic Example

```lua
plugin = {
    name = "my-plugin",
    version = "0.1.0",
    description = "A Lua plugin example"
}

enya.register_command("greet", {
    description = "Greet with validation",
    accepts_args = true
}, function(args)
    if args == "" then
        enya.notify("info", "Hello, World!")
    elseif tonumber(args) then
        enya.notify("warn", "That's a number: " .. args)
    else
        enya.notify("info", "Hello, " .. args .. "!")
    end
    return true
end)

enya.keymap("Space+g", "greet", "Greet user")

-- Lifecycle hooks
function on_activate()
    enya.log("info", "Plugin activated!")
end

function on_deactivate()
    enya.log("info", "Plugin deactivated!")
end
```

### Lua API

**Registration (during load)**:
- `enya.register_command(name, config, callback)` - Register a command
- `enya.keymap(keys, command, description, [modes])` - Register a keybinding

**Runtime (in callbacks)**:
- `enya.notify(level, message)` - Show notification ("info", "warn", "error")
- `enya.log(level, message)` - Log a message
- `enya.request_repaint()` - Request UI refresh
- `enya.editor_version()` - Get host version
- `enya.is_wasm()` - Check if running in WASM
- `enya.theme_name()` - Get current theme name
- `enya.clipboard_write(text)` - Write to clipboard
- `enya.clipboard_read()` - Read from clipboard
- `enya.execute(command, [args])` - Execute another command
- `enya.http_get(url, [headers])` - HTTP GET request
- `enya.http_post(url, body, [headers])` - HTTP POST request

**Pane Management**:
- `enya.add_query_pane(query, [title])` - Add a query pane with PromQL query
- `enya.add_logs_pane()` - Add a logs pane with current time range
- `enya.add_tracing_pane([trace_id])` - Add a tracing pane
- `enya.add_terminal_pane()` - Add a terminal pane (native only)
- `enya.add_sql_pane()` - Add a SQL pane
- `enya.close_pane()` - Close the focused pane
- `enya.focus_pane(direction)` - Focus pane in direction ("left", "right", "up", "down")

**Time Range**:
- `enya.set_time_range(preset)` - Set time range preset ("5m", "1h", "24h", etc.)
- `enya.set_time_range_absolute(start, end)` - Set absolute time range (seconds)
- `enya.get_time_range()` - Get current time range as `{start, end}` (seconds)

### Custom Themes in Lua

```lua
plugin = { name = "tokyo-night-theme" }

theme = {
    name = "tokyo-night",
    display_name = "Tokyo Night",
    base = "dark",  -- inherit missing colors from dark theme
    colors = {
        bg_base = "#1a1b26",
        bg_surface = "#24283b",
        bg_elevated = "#414868",
        text_primary = "#c0caf5",
        text_secondary = "#a9b1d6",
        accent_primary = "#7aa2f7",
        accent_hover = "#89b4fa",
        success = "#9ece6a",
        warning = "#e0af68",
        error = "#f7768e",
        info = "#7dcfff",
        chart = {
            "#7aa2f7", "#9ece6a", "#e0af68", "#f7768e",
            "#bb9af7", "#7dcfff", "#73daca", "#ff9e64"
        }
    }
}
```

## Native Plugins (Rust)

For maximum performance or deep integration, implement the `Plugin` trait:

```rust
use enya_plugin::{
    Plugin, PluginCapabilities, PluginContext, PluginResult,
    CommandConfig, KeybindingConfig,
};
use std::any::Any;

pub struct MyPlugin {
    active: bool,
}

impl Plugin for MyPlugin {
    fn name(&self) -> &'static str { "my-native-plugin" }
    fn version(&self) -> &'static str { "1.0.0" }
    fn description(&self) -> &'static str { "A native Rust plugin" }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::COMMANDS | PluginCapabilities::KEYBOARD
    }

    fn init(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        Ok(())
    }

    fn activate(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        self.active = true;
        Ok(())
    }

    fn deactivate(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        self.active = false;
        Ok(())
    }

    fn commands(&self) -> Vec<CommandConfig> {
        vec![CommandConfig {
            name: "my-cmd".to_string(),
            aliases: vec!["mc".to_string()],
            description: "Do something".to_string(),
            accepts_args: false,
        }]
    }

    fn keybindings(&self) -> Vec<KeybindingConfig> {
        vec![KeybindingConfig {
            keys: "Space+m+c".to_string(),
            command: "my-cmd".to_string(),
            description: "Run my command".to_string(),
            modes: vec![],
        }]
    }

    fn execute_command(&mut self, command: &str, args: &str, ctx: &PluginContext) -> bool {
        if command == "my-cmd" || command == "mc" {
            ctx.notify("info", "Command executed!");
            return true;
        }
        false
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
```

### Custom Themes in Rust

```rust
use enya_plugin::{ThemeDefinition, ThemeBase};

fn themes(&self) -> Vec<ThemeDefinition> {
    vec![
        ThemeDefinition::new("my-theme", "My Theme", ThemeBase::Dark)
            .with_backgrounds(Some(0x1a1b26), Some(0x24283b), Some(0x414868))
            .with_text(Some(0xc0caf5), Some(0xa9b1d6), Some(0x565f89))
            .with_accents(Some(0x7aa2f7), Some(0x89b4fa), Some(0x3d59a1))
            .with_semantic(Some(0x9ece6a), Some(0xe0af68), Some(0xf7768e), Some(0x7dcfff))
            .with_chart_palette(vec![0x7aa2f7, 0x9ece6a, 0xe0af68, 0xf7768e])
    ]
}
```

## Plugin Capabilities

Plugins declare their capabilities using bitflags:

| Capability | Description |
|------------|-------------|
| `COMMANDS` | Provides command palette commands |
| `PANES` | Provides custom pane types |
| `KEYBOARD` | Provides custom keybindings |
| `THEMING` | Reacts to theme changes |
| `CUSTOM_THEMES` | Defines custom color themes |
| `VISUALIZATIONS` | Provides custom chart types |
| `DATA_SOURCES` | Provides backend integrations |
| `AGENT_COMMANDS` | Provides AI integration commands |
| `STATUS_LINE` | Provides status line segments |
| `FINDER_SOURCES` | Provides unified finder sources |

## Hook System

Plugins can intercept and extend host behavior through hooks:

### Lifecycle Hooks

```rust
impl LifecycleHook for MyHook {
    fn on_workspace_loaded(&mut self) { }
    fn on_workspace_saving(&mut self) { }
    fn on_pane_added(&mut self, pane_id: usize) { }
    fn on_pane_removing(&mut self, pane_id: usize) { }
    fn on_pane_focused(&mut self, pane_id: usize) { }
    fn on_closing(&mut self) { }
    fn on_frame(&mut self) { } // Use sparingly!
}
```

### Command Hooks

```rust
impl CommandHook for MyHook {
    fn before_command(&mut self, command: &str, args: &str) -> CommandHookResult {
        // Return Handled to prevent default handler
        CommandHookResult::Continue
    }

    fn after_command(&mut self, command: &str, args: &str, success: bool) { }
}
```

### Keyboard Hooks

```rust
impl KeyboardHook for MyHook {
    fn on_key_pressed(&mut self, key: &KeyEvent) -> KeyboardHookResult {
        KeyboardHookResult::Continue
    }

    fn on_key_combo(&mut self, combo: &KeyCombo) -> KeyboardHookResult {
        KeyboardHookResult::Continue
    }
}
```

### Theme Hooks

```rust
impl ThemeHook for MyHook {
    fn before_theme_change(&mut self, old: Theme, new: Theme) { }
    fn after_theme_change(&mut self, theme: Theme) { }
    fn customize_theme(&self, theme: Theme) -> Option<ThemeCustomization> { None }
}
```

### Pane Hooks

```rust
impl PaneHook for MyHook {
    fn on_pane_created(&mut self, pane_id: usize, pane_type: &str) { }
    fn on_query_changed(&mut self, pane_id: usize, query: &str) { }
    fn on_data_received(&mut self, pane_id: usize) { }
    fn on_pane_error(&mut self, pane_id: usize, error: &str) { }
}
```

## Plugin Lifecycle

```
Registration ──► Initialization ──► Activation ──► Runtime ──► Deactivation
     │                │                  │            │              │
     │                │                  │            │              │
     ▼                ▼                  ▼            ▼              ▼
 Registered       Inactive           Active       Hooks         Inactive
   state           state             state       dispatch         state
```

1. **Registration**: Plugin added to registry (`PluginState::Registered`)
2. **Initialization**: `init()` called, metadata collected (`PluginState::Inactive`)
3. **Activation**: `activate()` called (`PluginState::Active`)
4. **Runtime**: Commands executed, hooks dispatched
5. **Deactivation**: `deactivate()` called (`PluginState::Inactive`)

## Platform Support

| Feature | Native | WASM |
|---------|--------|------|
| Lua plugins | ✓ | ✗ |
| Native plugins | ✓ | ✓ |
| Custom themes | ✓ | ✓ |
| HTTP requests | ✓ | Host-dependent |
| Clipboard | ✓ | Host-dependent |
| Shell commands | ✓ | ✗ |

## Module Structure

```
src/
├── lib.rs          # Crate entry, re-exports
├── types.rs        # Core types (PluginHost, PluginContext, etc.)
├── traits.rs       # Plugin trait, capabilities, configs
├── registry.rs     # PluginRegistry, lifecycle management
├── hooks.rs        # Hook traits (Lifecycle, Command, Keyboard, etc.)
├── theme.rs        # ThemeDefinition, ThemeColors
├── loader.rs       # PluginLoader, filesystem discovery (native only)
└── lua.rs          # LuaPlugin, Lua API (native only)
```

## License

MIT
