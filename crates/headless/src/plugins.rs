use enya_plugin::{
    ConfigCommand, ConfigPlugin, HeadlessPluginHost, LuaPlugin, Plugin, PluginContext, PluginLoader,
};
use serde::Serialize;
use std::sync::Arc;

use crate::Result;

// -- Result types -------------------------------------------------------------

#[derive(Serialize)]
pub struct PluginsListResult {
    pub dir: String,
    pub plugins: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct PluginsCommandsResult {
    pub commands: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct PluginInstallResult {
    pub installed: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct PluginRemoveResult {
    pub removed: String,
    pub path: String,
}

// -- Core functions (return data, no printing) --------------------------------

pub fn plugins_list_core() -> PluginsListResult {
    let loader = PluginLoader::new();
    let dir = loader
        .user_plugin_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let mut items: Vec<serde_json::Value> = Vec::new();

    for result in loader.load_all() {
        match result {
            Ok(plugin) => {
                let manifest = plugin.manifest();
                items.push(serde_json::json!({
                    "name": manifest.plugin.name,
                    "version": manifest.plugin.version,
                    "description": manifest.plugin.description,
                    "type": "config",
                    "enabled": manifest.plugin.enabled,
                    "commands": manifest.commands.len(),
                    "keybindings": manifest.keybindings.len(),
                }));
            }
            Err(e) => {
                items.push(serde_json::json!({
                    "error": e.to_string(),
                    "type": "config",
                }));
            }
        }
    }

    for result in loader.load_all_lua() {
        match result {
            Ok(plugin) => {
                items.push(serde_json::json!({
                    "name": plugin.name(),
                    "version": plugin.version(),
                    "description": plugin.description(),
                    "type": "lua",
                    "enabled": true,
                }));
            }
            Err(e) => {
                items.push(serde_json::json!({
                    "error": e.to_string(),
                    "type": "lua",
                }));
            }
        }
    }

    PluginsListResult {
        dir,
        plugins: items,
    }
}

pub fn plugins_commands_core() -> PluginsCommandsResult {
    let loader = PluginLoader::new();
    let mut items: Vec<serde_json::Value> = Vec::new();

    for result in loader.load_all() {
        let Ok(plugin) = result else { continue };
        let manifest = plugin.manifest();
        if !manifest.plugin.enabled {
            continue;
        }
        for cmd in &manifest.commands {
            let mut item = serde_json::json!({
                "name": cmd.name,
                "plugin": manifest.plugin.name,
                "type": "config",
            });
            if !cmd.description.is_empty() {
                item["description"] = serde_json::json!(cmd.description);
            }
            if !cmd.aliases.is_empty() {
                item["aliases"] = serde_json::json!(cmd.aliases);
            }
            if cmd.accepts_args {
                item["accepts_args"] = serde_json::json!(true);
            }
            if cmd.shell.is_some() {
                item["action"] = serde_json::json!("shell");
            } else if cmd.url.is_some() {
                item["action"] = serde_json::json!("url");
            } else if cmd.notify.is_some() {
                item["action"] = serde_json::json!("notify");
            }
            items.push(item);
        }
    }

    for result in loader.load_all_lua() {
        let Ok(plugin) = result else { continue };
        for cmd in plugin.commands() {
            let mut item = serde_json::json!({
                "name": cmd.name,
                "plugin": plugin.name(),
                "type": "lua",
            });
            if !cmd.description.is_empty() {
                item["description"] = serde_json::json!(cmd.description);
            }
            if !cmd.aliases.is_empty() {
                item["aliases"] = serde_json::json!(cmd.aliases);
            }
            if cmd.accepts_args {
                item["accepts_args"] = serde_json::json!(true);
            }
            items.push(item);
        }
    }

    PluginsCommandsResult { commands: items }
}

pub fn plugins_install_core(source: &str) -> Result<PluginInstallResult> {
    let source_path = std::path::Path::new(source);

    if !source_path.exists() {
        return Err(format!("file not found: {source}").into());
    }

    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if ext != "toml" && ext != "lua" {
        return Err("plugin file must be .toml or .lua".into());
    }

    let name = if ext == "toml" {
        let plugin = ConfigPlugin::load(source_path)?;
        plugin.manifest().plugin.name.clone()
    } else {
        let plugin = LuaPlugin::load(source_path)?;
        plugin.name().to_string()
    };

    let loader = PluginLoader::new();
    loader.ensure_user_dir()?;
    let dest_dir = loader
        .user_plugin_dir()
        .ok_or("could not determine plugin directory")?;

    let file_name = source_path.file_name().ok_or("invalid source path")?;
    let dest = dest_dir.join(file_name);

    if dest.exists() {
        return Err(format!(
            "{} already exists in plugin directory",
            file_name.to_string_lossy()
        )
        .into());
    }

    std::fs::copy(source_path, &dest)?;
    Ok(PluginInstallResult {
        installed: name,
        path: dest.display().to_string(),
    })
}

pub fn plugins_remove_core(name: &str) -> Result<PluginRemoveResult> {
    let loader = PluginLoader::new();
    let dir = loader
        .user_plugin_dir()
        .ok_or("could not determine plugin directory")?;

    let mut found_path: Option<std::path::PathBuf> = None;

    for path in loader.discover() {
        if let Ok(plugin) = ConfigPlugin::load(&path) {
            if plugin.manifest().plugin.name == name {
                found_path = Some(path);
                break;
            }
        }
    }

    if found_path.is_none() {
        for path in loader.discover_lua() {
            if let Ok(plugin) = LuaPlugin::load(&path) {
                if plugin.name() == name {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    let path = found_path.ok_or(format!("plugin not found: {name}"))?;

    if !path.starts_with(dir) {
        return Err(format!(
            "plugin '{}' is not in user plugin directory (found at {})",
            name,
            path.display()
        )
        .into());
    }

    std::fs::remove_file(&path)?;
    Ok(PluginRemoveResult {
        removed: name.to_string(),
        path: path.display().to_string(),
    })
}

/// Core exec: find and execute a plugin command, return structured result.
pub fn exec_core(command: &str, args: &str) -> Result<serde_json::Value> {
    let loader = PluginLoader::new();

    for result in loader.load_all() {
        let Ok(plugin) = result else { continue };
        let manifest = plugin.manifest();

        if !manifest.plugin.enabled {
            continue;
        }

        for cmd in &manifest.commands {
            if cmd.name == command || cmd.aliases.contains(&command.to_string()) {
                return exec_config_command_core(cmd, args);
            }
        }
    }

    for result in loader.load_all_lua() {
        let Ok(mut plugin) = result else { continue };
        for cmd_config in plugin.commands() {
            if cmd_config.name == command || cmd_config.aliases.contains(&command.to_string()) {
                let host = Arc::new(HeadlessPluginHost);
                let ctx = PluginContext::new(host);
                let success = plugin.execute_command(command, args, &ctx);

                if !success {
                    return Err(format!("command '{command}' failed").into());
                }

                return Ok(serde_json::json!({
                    "command": command,
                    "plugin": plugin.name(),
                    "success": success,
                }));
            }
        }
    }

    Err(format!("unknown command: {command}").into())
}

/// Core exec for config commands: captures output and returns structured result.
pub fn exec_config_command_core(cmd: &ConfigCommand, args: &str) -> Result<serde_json::Value> {
    if let Some(shell) = &cmd.shell {
        let full_cmd = if args.is_empty() {
            shell.clone()
        } else {
            format!("{shell} {args}")
        };

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .output()?;

        let result = serde_json::json!({
            "command": cmd.name,
            "shell": full_cmd,
            "exit_code": output.status.code(),
            "success": output.status.success(),
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
        });

        if !output.status.success() {
            return Err(format!("command exited with {}", output.status).into());
        }
        return Ok(result);
    }

    if let Some(url) = &cmd.url {
        let full_url = if args.is_empty() {
            url.clone()
        } else {
            format!("{url}{args}")
        };
        return Ok(serde_json::json!({"command": cmd.name, "url": full_url}));
    }

    if let Some(msg) = &cmd.notify {
        return Ok(serde_json::json!({"command": cmd.name, "message": msg}));
    }

    Err(format!("command '{}' has no action defined", cmd.name).into())
}

// -- CLI wrappers (call core + format output) ---------------------------------

pub fn plugins(json: bool) -> Result {
    let result = plugins_list_core();

    if json {
        println!("{}", serde_json::to_string(&result)?);
        return Ok(());
    }

    if result.plugins.is_empty() {
        println!("No plugins found in {}", result.dir);
        return Ok(());
    }

    println!("Plugins in {}:\n", result.dir);
    for item in &result.plugins {
        if let Some(error) = item.get("error") {
            let typ = item["type"].as_str().unwrap_or("unknown");
            println!("  (error) [{typ}] {error}");
            continue;
        }
        let name = item["name"].as_str().unwrap_or("?");
        let version = item["version"].as_str().unwrap_or("?");
        let desc = item["description"].as_str().unwrap_or("");
        let typ = item["type"].as_str().unwrap_or("?");
        let enabled = item["enabled"].as_bool().unwrap_or(false);
        let status = if enabled { "" } else { " (disabled)" };
        if desc.is_empty() {
            println!("  {name} v{version} [{typ}]{status}");
        } else {
            println!("  {name} v{version} [{typ}]{status} — {desc}");
        }
    }
    Ok(())
}

pub fn plugins_commands(json: bool) -> Result {
    let result = plugins_commands_core();

    if json {
        println!("{}", serde_json::to_string(&result)?);
        return Ok(());
    }

    if result.commands.is_empty() {
        println!("No plugin commands found");
        return Ok(());
    }

    println!("Plugin commands:\n");
    for item in &result.commands {
        let name = item["name"].as_str().unwrap_or("?");
        let plugin = item["plugin"].as_str().unwrap_or("?");
        let desc = item.get("description").and_then(|d| d.as_str());
        let typ = item["type"].as_str().unwrap_or("?");
        let aliases = item
            .get("aliases")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        match desc {
            Some(d) => println!("  {name:20} [{plugin}, {typ}] {d}"),
            None => println!("  {name:20} [{plugin}, {typ}]"),
        }
        if !aliases.is_empty() {
            println!("  {:20} aliases: {aliases}", "");
        }
    }
    Ok(())
}

pub fn plugins_install(source: &str, json: bool) -> Result {
    let result = plugins_install_core(source)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("Installed plugin '{}' to {}", result.installed, result.path);
    }
    Ok(())
}

pub fn plugins_remove(name: &str, json: bool) -> Result {
    let result = plugins_remove_core(name)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        println!("Removed plugin '{}' ({})", result.removed, result.path);
    }
    Ok(())
}

pub fn exec(command: &str, args: &str, json: bool) -> Result {
    let result = exec_core(command, args)?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        // Print human-readable output based on result type
        if let Some(stdout) = result.get("stdout").and_then(|v| v.as_str()) {
            if !stdout.is_empty() {
                print!("{stdout}");
            }
        }
        if let Some(stderr) = result.get("stderr").and_then(|v| v.as_str()) {
            if !stderr.is_empty() {
                eprint!("{stderr}");
            }
        }
        if let Some(url) = result.get("url").and_then(|v| v.as_str()) {
            println!("{url}");
        }
        if let Some(msg) = result.get("message").and_then(|v| v.as_str()) {
            println!("{msg}");
        }
    }
    Ok(())
}
