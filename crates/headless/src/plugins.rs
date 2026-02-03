use enya_plugin::{
    ConfigCommand, ConfigPlugin, HeadlessPluginHost, LuaPlugin, Plugin, PluginContext, PluginLoader,
};
use std::sync::Arc;

use crate::Result;

pub fn plugins(json: bool) -> Result {
    let loader = PluginLoader::new();
    let dir = loader
        .user_plugin_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let mut items: Vec<serde_json::Value> = Vec::new();

    // Load TOML config plugins
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

    // Load Lua plugins
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

    if json {
        println!("{}", serde_json::json!({"dir": dir, "plugins": items}));
        return Ok(());
    }

    if items.is_empty() {
        println!("No plugins found in {dir}");
        return Ok(());
    }

    println!("Plugins in {dir}:\n");
    for item in &items {
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
    let loader = PluginLoader::new();
    let mut items: Vec<serde_json::Value> = Vec::new();

    // Collect commands from TOML config plugins
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

    // Collect commands from Lua plugins
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

    if json {
        println!("{}", serde_json::json!({"commands": items}));
        return Ok(());
    }

    if items.is_empty() {
        println!("No plugin commands found");
        return Ok(());
    }

    println!("Plugin commands:\n");
    for item in &items {
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

    // Validate the plugin by trying to load it
    let name = if ext == "toml" {
        let plugin = ConfigPlugin::load(source_path)?;
        plugin.manifest().plugin.name.clone()
    } else {
        let plugin = LuaPlugin::load(source_path)?;
        plugin.name().to_string()
    };

    // Copy to user plugin directory
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

    if json {
        println!(
            "{}",
            serde_json::json!({"installed": name, "path": dest.display().to_string()})
        );
    } else {
        println!("Installed plugin '{}' to {}", name, dest.display());
    }
    Ok(())
}

pub fn plugins_remove(name: &str, json: bool) -> Result {
    let loader = PluginLoader::new();
    let dir = loader
        .user_plugin_dir()
        .ok_or("could not determine plugin directory")?;

    // Search for plugin by name across all files
    let mut found_path: Option<std::path::PathBuf> = None;

    // Check TOML plugins
    for path in loader.discover() {
        if let Ok(plugin) = ConfigPlugin::load(&path) {
            if plugin.manifest().plugin.name == name {
                found_path = Some(path);
                break;
            }
        }
    }

    // Check Lua plugins if not found
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

    // Only remove from user plugin directory
    if !path.starts_with(dir) {
        return Err(format!(
            "plugin '{}' is not in user plugin directory (found at {})",
            name,
            path.display()
        )
        .into());
    }

    std::fs::remove_file(&path)?;

    if json {
        println!(
            "{}",
            serde_json::json!({"removed": name, "path": path.display().to_string()})
        );
    } else {
        println!("Removed plugin '{}' ({})", name, path.display());
    }
    Ok(())
}

pub fn exec(command: &str, args: &str, json: bool) -> Result {
    let loader = PluginLoader::new();

    // Search config plugins for the command
    for result in loader.load_all() {
        let Ok(plugin) = result else { continue };
        let manifest = plugin.manifest();

        if !manifest.plugin.enabled {
            continue;
        }

        for cmd in &manifest.commands {
            if cmd.name == command || cmd.aliases.contains(&command.to_string()) {
                return exec_config_command(cmd, args, json);
            }
        }
    }

    // Search Lua plugins for the command
    for result in loader.load_all_lua() {
        let Ok(mut plugin) = result else { continue };
        for cmd_config in plugin.commands() {
            if cmd_config.name == command || cmd_config.aliases.contains(&command.to_string()) {
                let host = Arc::new(HeadlessPluginHost);
                let ctx = PluginContext::new(host);
                let success = plugin.execute_command(command, args, &ctx);

                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "command": command,
                            "plugin": plugin.name(),
                            "success": success,
                        })
                    );
                }

                if success {
                    return Ok(());
                } else {
                    return Err(format!("command '{command}' failed").into());
                }
            }
        }
    }

    Err(format!("unknown command: {command}").into())
}

fn exec_config_command(cmd: &ConfigCommand, args: &str, json: bool) -> Result {
    // Shell command — run synchronously and forward output
    if let Some(shell) = &cmd.shell {
        let full_cmd = if args.is_empty() {
            shell.clone()
        } else {
            format!("{shell} {args}")
        };

        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .status()?;

        if json {
            println!(
                "{}",
                serde_json::json!({
                    "command": cmd.name,
                    "shell": full_cmd,
                    "exit_code": status.code(),
                    "success": status.success(),
                })
            );
        }

        if !status.success() {
            return Err(format!("command exited with {status}").into());
        }
        return Ok(());
    }

    // URL command — print the URL (headless, no browser)
    if let Some(url) = &cmd.url {
        let full_url = if args.is_empty() {
            url.clone()
        } else {
            format!("{url}{args}")
        };

        if json {
            println!(
                "{}",
                serde_json::json!({"command": cmd.name, "url": full_url})
            );
        } else {
            println!("{full_url}");
        }
        return Ok(());
    }

    // Notify command — print the message
    if let Some(msg) = &cmd.notify {
        if json {
            println!(
                "{}",
                serde_json::json!({"command": cmd.name, "message": msg})
            );
        } else {
            println!("{msg}");
        }
        return Ok(());
    }

    Err(format!("command '{}' has no action defined", cmd.name).into())
}
