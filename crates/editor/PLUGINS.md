# Enya Plugin System

Enya provides a neovim-style plugin system for extending editor functionality. Plugins are written in Lua, giving you full scripting capabilities with conditional logic, input validation, and complex workflows.

## Quick Start

1. **Create your plugin directory:**
   ```bash
   mkdir -p ~/.config/enya/plugins
   ```

2. **Create a simple plugin** (`~/.config/enya/plugins/my-plugin.lua`):
   ```lua
   -- Plugin metadata (required)
   plugin = {
       name = "my-plugin",
       version = "0.1.0",
       description = "My first Enya plugin"
   }

   -- Register a command
   enya.register_command("hello", {
       description = "Say hello"
   }, function(args)
       enya.notify("info", "Hello from my plugin!")
       return true
   end)

   -- Register a keybinding
   enya.keymap("Space+x+h", "hello", "Say hello")
   ```

3. **Restart Enya** - your plugin will be automatically loaded!

## Plugin Locations

Plugins are loaded from these directories (in priority order):

| Location | Purpose |
|----------|---------|
| `~/.config/enya/plugins/` | User plugins (highest priority) |
| `<workspace>/.enya/plugins/` | Workspace-local plugins |
| Built-in plugins | Core functionality (lowest priority) |

## Writing Lua Plugins

### Basic Structure

Every Lua plugin needs a `plugin` table with metadata:

```lua
-- ~/.config/enya/plugins/my-plugin.lua

-- Plugin metadata (required)
plugin = {
    name = "my-plugin",
    version = "0.1.0",
    description = "A description of what this plugin does"
}

-- Register commands, keybindings, themes, etc.
```

### Registering Commands

Commands appear in the command palette and can be triggered via keybindings:

```lua
enya.register_command("greet", {
    description = "Greet the user",
    aliases = {"hello", "hi"},      -- Alternative names
    accepts_args = true              -- Allow arguments
}, function(args)
    if args == "" then
        enya.notify("info", "Hello, World!")
    else
        enya.notify("info", "Hello, " .. args .. "!")
    end
    return true  -- Return true on success, false on failure
end)
```

### Registering Keybindings

Keybindings follow vim-style conventions:

```lua
-- Basic keybinding
enya.keymap("Space+g+h", "greet", "Greet user")

-- With mode restriction (only active in normal mode)
enya.keymap("Space+g+h", "greet", "Greet user", {"normal"})
```

#### Key Format

| Format | Example | Description |
|--------|---------|-------------|
| `Space+x+y` | `Space+g+h` | Leader key sequence |
| `Ctrl+x` | `Ctrl+s` | Control modifier |
| `Alt+x` | `Alt+f` | Alt/Option modifier |
| `Shift+x` | `Shift+Tab` | Shift modifier |

#### Available Modes

- `normal` - Default editing mode
- `command` - Command palette mode
- (empty array) - Active in all modes

### Lifecycle Hooks

Plugins can respond to activation and deactivation:

```lua
function on_activate()
    enya.log("info", "Plugin activated!")
end

function on_deactivate()
    enya.log("info", "Plugin deactivated!")
end
```

## Lua API Reference

### Registration Functions

Available during plugin load:

| Function | Description |
|----------|-------------|
| `enya.register_command(name, config, callback)` | Register a command |
| `enya.keymap(keys, command, description, [modes])` | Register a keybinding |

### Runtime Functions

Available in command callbacks:

| Function | Description |
|----------|-------------|
| `enya.notify(level, message)` | Show notification ("info", "warn", "error") |
| `enya.log(level, message)` | Log a message ("debug", "info", "warn", "error") |
| `enya.request_repaint()` | Request UI refresh |
| `enya.editor_version()` | Get editor version string |
| `enya.is_wasm()` | Check if running in WASM (always false for Lua) |
| `enya.theme_name()` | Get current theme name |
| `enya.clipboard_write(text)` | Write text to clipboard |
| `enya.clipboard_read()` | Read text from clipboard (returns nil if empty) |
| `enya.execute(command, [args])` | Execute another command |
| `enya.http_get(url, [headers])` | HTTP GET, returns `{status, body, headers}` or `{error}` |
| `enya.http_post(url, body, [headers])` | HTTP POST, returns `{status, body, headers}` or `{error}` |

### Pane Management Functions

Control workspace panes from Lua:

| Function | Description |
|----------|-------------|
| `enya.add_query_pane(query, [title])` | Add a query pane with PromQL query |
| `enya.add_logs_pane()` | Add a logs pane with current time range |
| `enya.add_tracing_pane([trace_id])` | Add a tracing pane, optionally with a trace ID |
| `enya.add_terminal_pane()` | Add a terminal pane (native only) |
| `enya.add_sql_pane()` | Add a SQL pane |
| `enya.close_pane()` | Close the focused pane |
| `enya.focus_pane(direction)` | Focus pane in direction ("left", "right", "up", "down") |

### Time Range Functions

Control the global time range:

| Function | Description |
|----------|-------------|
| `enya.set_time_range(preset)` | Set time range preset ("5m", "15m", "30m", "1h", "6h", "24h", "7d") |
| `enya.set_time_range_absolute(start, end)` | Set absolute time range (seconds since Unix epoch) |
| `enya.get_time_range()` | Get current time range as `{start, end}` (seconds) |

## Examples

### Shell Command Execution

```lua
plugin = { name = "devtools", version = "0.1.0" }

enya.register_command("build", {
    description = "Build the project",
    accepts_args = true
}, function(args)
    local cmd = "cargo build --release"
    if args ~= "" then
        cmd = cmd .. " " .. args
    end

    -- Execute shell command
    os.execute(cmd .. " &")  -- Run in background
    enya.notify("info", "Build started...")
    return true
end)

enya.keymap("Space+b+b", "build", "Build project")
```

### URL Opening

```lua
plugin = { name = "links", version = "0.1.0" }

enya.register_command("open-docs", {
    description = "Open documentation"
}, function(args)
    os.execute("open https://docs.rs/my-crate &")
    return true
end)
```

### Input Validation

```lua
plugin = { name = "validator", version = "0.1.0" }

enya.register_command("set-threshold", {
    description = "Set alert threshold",
    accepts_args = true
}, function(args)
    local num = tonumber(args)

    if num == nil then
        enya.notify("error", "Please provide a number")
        return false
    end

    if num < 0 then
        enya.notify("error", "Threshold must be positive")
        return false
    end

    if num > 1000 then
        enya.notify("warn", "Very high threshold: " .. tostring(num))
    end

    enya.notify("info", "Threshold set to " .. tostring(num))
    return true
end)
```

### HTTP Requests

```lua
plugin = { name = "api-tools", version = "0.1.0" }

enya.register_command("fetch-status", {
    description = "Check API status"
}, function(args)
    local response = enya.http_get("https://api.example.com/status", {
        ["Authorization"] = "Bearer token123"
    })

    if response.error then
        enya.notify("error", "Request failed: " .. response.error)
        return false
    end

    if response.status == 200 then
        enya.notify("info", "API is healthy!")
    else
        enya.notify("warn", "API returned status " .. tostring(response.status))
    end

    return true
end)
```

### Workflow Automation

```lua
plugin = { name = "incident-helper", version = "0.1.0" }

enya.register_command("start-incident", {
    description = "Set up incident investigation layout"
}, function(args)
    enya.log("info", "Starting incident investigation")
    enya.notify("info", "Setting up incident investigation...")

    -- Set time range to last hour
    enya.set_time_range("1h")

    -- Add relevant panes
    enya.add_query_pane("rate(http_requests_total[5m])", "Request Rate")
    enya.add_logs_pane()

    return true
end)

enya.keymap("Space+i+s", "start-incident", "Start incident investigation")
```

### Pane Management

```lua
plugin = { name = "pane-tools", version = "0.1.0" }

-- Add a query pane with a custom title
enya.register_command("add-cpu-chart", {
    description = "Add CPU usage chart"
}, function(args)
    enya.add_query_pane("rate(process_cpu_seconds_total[5m])", "CPU Usage")
    enya.notify("info", "Added CPU chart")
    return true
end)

-- Navigate between panes
enya.register_command("focus-left", {
    description = "Focus pane on the left"
}, function(args)
    enya.focus_pane("left")
    return true
end)

-- Close current pane
enya.register_command("close-current", {
    description = "Close the focused pane"
}, function(args)
    enya.close_pane()
    return true
end)

enya.keymap("Space+p+c", "add-cpu-chart", "Add CPU chart")
enya.keymap("Space+p+h", "focus-left", "Focus left pane")
enya.keymap("Space+p+x", "close-current", "Close pane")
```

### Time Range Control

```lua
plugin = { name = "time-presets", version = "0.1.0" }

-- Quick time range presets
enya.register_command("last-5m", {
    description = "Set time range to last 5 minutes"
}, function(args)
    enya.set_time_range("5m")
    return true
end)

enya.register_command("last-hour", {
    description = "Set time range to last hour"
}, function(args)
    enya.set_time_range("1h")
    return true
end)

enya.register_command("last-day", {
    description = "Set time range to last 24 hours"
}, function(args)
    enya.set_time_range("24h")
    return true
end)

-- Set custom absolute time range
enya.register_command("set-range", {
    description = "Set custom time range (start end in Unix seconds)",
    accepts_args = true
}, function(args)
    local parts = {}
    for part in args:gmatch("%S+") do
        table.insert(parts, tonumber(part))
    end

    if #parts ~= 2 then
        enya.notify("error", "Usage: set-range <start> <end>")
        return false
    end

    enya.set_time_range_absolute(parts[1], parts[2])
    enya.notify("info", "Time range set")
    return true
end)

-- Show current time range
enya.register_command("show-range", {
    description = "Show current time range"
}, function(args)
    local range = enya.get_time_range()
    enya.notify("info", "Range: " .. range.start .. " to " .. range["end"])
    return true
end)

enya.keymap("Space+t+5", "last-5m", "Last 5 minutes")
enya.keymap("Space+t+h", "last-hour", "Last hour")
enya.keymap("Space+t+d", "last-day", "Last 24 hours")
```

## Custom Themes

Plugins can define custom color themes:

```lua
plugin = {
    name = "tokyo-night-theme",
    version = "1.0.0",
    description = "Tokyo Night color theme"
}

theme = {
    name = "tokyo-night",
    display_name = "Tokyo Night",
    base = "dark",  -- Inherit missing colors from "dark" or "light"
    colors = {
        -- Backgrounds
        bg_base = "#1a1b26",
        bg_surface = "#24283b",
        bg_elevated = "#414868",

        -- Text
        text_primary = "#c0caf5",
        text_secondary = "#a9b1d6",
        text_muted = "#565f89",

        -- Accents
        accent_primary = "#7aa2f7",
        accent_hover = "#89b4fa",
        accent_muted = "#3d59a1",

        -- Borders
        border_subtle = "#414868",
        border_strong = "#565f89",

        -- Semantic colors
        success = "#9ece6a",
        warning = "#e0af68",
        error = "#f7768e",
        info = "#7dcfff",

        -- Chart palette (up to 8 colors)
        chart = {
            "#7aa2f7",
            "#9ece6a",
            "#e0af68",
            "#f7768e",
            "#bb9af7",
            "#7dcfff",
            "#73daca",
            "#ff9e64"
        }
    }
}
```

## Built-in Plugins

Enya comes with several built-in plugins that can be enabled/disabled:

| Plugin | Description | Default |
|--------|-------------|---------|
| `query-history` | Track and search executed queries | Enabled |
| `bookmarks` | Vim-style marks for queries | Enabled |
| `zen-mode` | Distraction-free viewing mode | Enabled |
| `session-manager` | Auto-save and session restoration | Enabled |
| `git-integration` | Git blame, history, and diff viewing | Enabled (native only) |

### Configuring Built-in Plugins

In your workspace configuration:

```toml
[plugins]
# Enable specific plugins
enabled = ["metrics-aggregator"]
# Disable specific plugins
disabled = ["zen-mode"]
```

## Advanced: Rust Plugins

For maximum performance or deep editor integration, you can implement the `Plugin` trait in Rust:

```rust
use enya_editor::plugin::{
    Plugin, PluginCapabilities, PluginContext, PluginResult,
    CommandConfig, KeybindingConfig,
};
use std::any::Any;

pub struct MyPlugin {
    active: bool,
}

impl Plugin for MyPlugin {
    fn name(&self) -> &'static str { "my-rust-plugin" }
    fn version(&self) -> &'static str { "0.1.0" }
    fn description(&self) -> &'static str { "A Rust plugin" }

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
            name: "my-command".to_string(),
            aliases: vec!["mc".to_string()],
            description: "Do something".to_string(),
            accepts_args: false,
        }]
    }

    fn execute_command(
        &mut self,
        command: &str,
        _args: &str,
        ctx: &PluginContext
    ) -> bool {
        if command == "my-command" || command == "mc" {
            ctx.notify("info", "Command executed!");
            return true;
        }
        false
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
```

Register your Rust plugin in the plugin registry during app initialization.

## Troubleshooting

### Plugin not loading?

1. Check the file is in the correct directory (`~/.config/enya/plugins/`)
2. Ensure the file has a `.lua` extension
3. Verify the `plugin` table is defined with at least a `name` field
4. Check Enya logs for Lua syntax errors

### Command not appearing?

- Ensure `enya.register_command()` is called at the top level (not inside a function)
- Check the command callback returns `true` or `false`
- Verify the command name doesn't conflict with built-in commands

### Keybinding not working?

- Check for conflicts with built-in keybindings
- Ensure the mode is correct (normal mode is default)
- Leader key sequences require pressing Space first
- Verify the command exists before binding to it

### HTTP requests failing?

- Check the URL is valid and accessible
- Ensure headers are passed as a table: `{ ["Key"] = "Value" }`
- Check the response for the `error` field before accessing `body`
