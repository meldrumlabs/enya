# Plugin Authoring Guide

Enya supports Lua plugins for custom commands, keybindings, panes, and themes. Plugins are loaded from `~/.enya/plugins/` on startup.

## Quick Start

Create `~/.enya/plugins/hello.lua`:

```lua
plugin = {
    name = "hello",
    version = "0.1.0",
    description = "A hello world plugin"
}

enya.register_command("greet", {
    description = "Greet the user",
    accepts_args = true
}, function(args)
    enya.notify("info", args == "" and "Hello!" or "Hello, " .. args .. "!")
    return true
end)
```

Restart Enya and type `:greet` in the command palette.

## Plugin Metadata

Every plugin must set a global `plugin` table:

```lua
plugin = {
    name = "my-plugin",          -- unique identifier
    version = "0.1.0",           -- semver
    description = "What it does"
}
```

## Lifecycle

Load -> init -> activate (`on_activate`) -> runtime -> deactivate (`on_deactivate`).

```lua
function on_activate()
    enya.log("info", "Plugin activated!")
end
```

## Commands

```lua
enya.register_command("name", {
    description = "What it does",
    aliases = {"alias1"},
    accepts_args = true
}, function(args)
    return true
end)
```

## Keybindings

```lua
enya.keymap("Space+x+g", "greet", "Greet user")
enya.keymap("<leader>ss", "share-slack", "Share to Slack")
```

## Custom Panes

Four pane types: `register_table_pane`, `register_chart_pane`, `register_stat_pane`, `register_gauge_pane`. Each takes a name, config table, and refresh callback.

```lua
enya.register_table_pane("my-table", {
    title = "My Table",
    columns = { { name = "Name" }, { name = "Value" } },
    refresh_interval = 5,
}, function()
    return { rows = { { "cpu", "42%" } } }
end)
```

## Custom Themes

```lua
theme = {
    name = "tokyo-night",
    display_name = "Tokyo Night",
    base = "dark",  -- or "light"
    colors = {
        bg_primary = "#1a1b26",
        text_primary = "#c0caf5",
        accent_primary = "#7aa2f7",
        -- unset colors fall back to base theme
    }
}
```

## Runtime API

Available inside callbacks and lifecycle hooks:

| Function | Description |
|----------|-------------|
| `enya.notify(level, msg)` | Show notification (`"info"`, `"warn"`, `"error"`) |
| `enya.log(level, msg)` | Log a message |
| `enya.execute(cmd, [args])` | Execute another command |
| `enya.http_get(url, [headers])` | HTTP GET -> `{status, body, headers}` or `{error}` |
| `enya.http_post(url, body, [headers])` | HTTP POST (same return) |
| `enya.clipboard_write(text)` | Write to clipboard |
| `enya.clipboard_read()` | Read from clipboard |
| `enya.get_focused_pane()` | `{pane_type, title, query, metric_name}` or `nil` |
| `enya.get_time_range()` | `{start, end}` as Unix timestamps |
| `enya.set_time_range(preset)` | Set preset: `"5m"`, `"1h"`, etc. |
| `enya.add_query_pane(query, [name])` | Add a query pane |
| `enya.close_pane()` | Close focused pane |

HTTP is **not available during loading** — only in callbacks.

## Example

See `plugins/share-to-slack.lua` for a real-world plugin that uses HTTP, clipboard, focused pane context, and keybindings.

## Testing Locally

Place your `.lua` file in `~/.enya/plugins/` and restart Enya. Use `enya.log()` for debug output. The Plugins overlay shows load status and errors.
