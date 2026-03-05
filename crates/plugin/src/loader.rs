//! Plugin loader for user-defined plugins.
//!
//! This module handles loading plugins from the filesystem, allowing users
//! to extend the host application without modifying the source code.
//!
//! # Plugin Locations
//!
//! Plugins are loaded from these directories (in order of priority):
//! 1. `~/.enya/plugins/` - User plugins (highest priority)
//! 2. `<workspace>/.enya/plugins/` - Workspace-local plugins
//! 3. Built-in plugins (lowest priority)
//!
//! # Plugin Formats
//!
//! ## Config Plugins (`.toml`)
//!
//! Simple plugins defined in TOML that can add commands and keybindings:
//!
//! ```toml
//! [plugin]
//! name = "my-plugin"
//! version = "0.1.0"
//! description = "My custom plugin"
//!
//! [[commands]]
//! name = "hello"
//! description = "Say hello"
//! # Run a shell command
//! shell = "echo 'Hello, World!'"
//!
//! [[commands]]
//! name = "open-grafana"
//! description = "Open Grafana in browser"
//! # Open a URL
//! url = "http://localhost:3000"
//!
//! [[keybindings]]
//! keys = "Space+x+h"
//! command = "hello"
//! description = "Say hello"
//! ```
//!
//! ## Script Plugins (`.lua`) - See lua.rs
//!
//! Lua scripts for more complex plugins.

use std::any::Any;
use std::path::{Path, PathBuf};

use crate::traits::{CommandConfig, KeybindingConfig, Plugin, PluginCapabilities};
use crate::types::PluginContext;
use crate::{PluginError, PluginResult};

/// Metadata for a loadable plugin.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PluginManifest {
    /// Plugin metadata
    pub plugin: PluginMeta,
    /// Commands provided by the plugin
    #[serde(default)]
    pub commands: Vec<ConfigCommand>,
    /// Keybindings provided by the plugin
    #[serde(default)]
    pub keybindings: Vec<ConfigKeybinding>,
}

/// Plugin metadata from manifest.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PluginMeta {
    /// Plugin name (unique identifier)
    pub name: String,
    /// Plugin version (semver)
    pub version: String,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Author name
    #[serde(default)]
    pub author: String,
    /// Plugin homepage URL
    #[serde(default)]
    pub homepage: String,
    /// Minimum host version required
    #[serde(default)]
    pub min_enya_version: Option<String>,
    /// Whether the plugin is enabled by default
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// A command defined in a config plugin.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigCommand {
    /// Command name
    pub name: String,
    /// Command description
    #[serde(default)]
    pub description: String,
    /// Aliases for the command
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Shell command to execute (mutually exclusive with `url`)
    #[serde(default)]
    pub shell: Option<String>,
    /// URL to open (mutually exclusive with `shell`)
    #[serde(default)]
    pub url: Option<String>,
    /// Notify message to show
    #[serde(default)]
    pub notify: Option<String>,
    /// Whether this command accepts arguments
    #[serde(default)]
    pub accepts_args: bool,
}

/// A keybinding defined in a config plugin.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigKeybinding {
    /// Key sequence (e.g., "Space+x+h")
    pub keys: String,
    /// Command to execute
    pub command: String,
    /// Description for which-key overlay
    #[serde(default)]
    pub description: String,
    /// Modes where binding is active (empty = all)
    #[serde(default)]
    pub modes: Vec<String>,
}

/// A plugin loaded from a config file.
pub struct ConfigPlugin {
    /// Plugin manifest
    manifest: PluginManifest,
    /// Path to the plugin file
    path: PathBuf,
    /// Whether the plugin is active
    active: bool,
    /// Cached static name (leaked once at load)
    name: &'static str,
    /// Cached static version (leaked once at load)
    version: &'static str,
    /// Cached static description (leaked once at load)
    description: &'static str,
    /// Cached static min_editor_version (leaked once at load)
    min_editor_version: Option<&'static str>,
}

impl ConfigPlugin {
    /// Load a plugin from a TOML file.
    pub fn load(path: &Path) -> PluginResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to read {}: {e}", path.display()))
        })?;

        let manifest: PluginManifest = toml::from_str(&content).map_err(|e| {
            PluginError::InvalidConfiguration(format!("Failed to parse {}: {e}", path.display()))
        })?;

        // Leak strings once at load time (plugins live for the duration of the program)
        let name = Box::leak(manifest.plugin.name.clone().into_boxed_str());
        let version = Box::leak(manifest.plugin.version.clone().into_boxed_str());
        let description = Box::leak(manifest.plugin.description.clone().into_boxed_str());
        let min_editor_version = manifest
            .plugin
            .min_enya_version
            .as_ref()
            .map(|v| Box::leak(v.clone().into_boxed_str()) as &'static str);

        Ok(Self {
            manifest,
            path: path.to_path_buf(),
            active: false,
            name,
            version,
            description,
            min_editor_version,
        })
    }

    /// Get the plugin manifest.
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    /// Create a plugin from a manifest (for testing).
    #[cfg(test)]
    pub fn from_manifest(manifest: PluginManifest, path: PathBuf) -> Self {
        let name = Box::leak(manifest.plugin.name.clone().into_boxed_str());
        let version = Box::leak(manifest.plugin.version.clone().into_boxed_str());
        let description = Box::leak(manifest.plugin.description.clone().into_boxed_str());
        let min_editor_version = manifest
            .plugin
            .min_enya_version
            .as_ref()
            .map(|v| Box::leak(v.clone().into_boxed_str()) as &'static str);

        Self {
            manifest,
            path,
            active: false,
            name,
            version,
            description,
            min_editor_version,
        }
    }

    /// Execute a shell command.
    fn execute_shell(&self, cmd: &str, args: &str) -> bool {
        let full_cmd = if args.is_empty() {
            cmd.to_string()
        } else {
            format!("{cmd} {args}")
        };

        log::info!(
            "[plugin:{}] Executing: {full_cmd}",
            self.manifest.plugin.name
        );

        let plugin_name = self.manifest.plugin.name.clone();
        let cmd_for_log = full_cmd.clone();

        match std::process::Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .spawn()
        {
            Ok(mut child) => {
                // Don't wait for the process to complete, but log exit status
                std::thread::spawn(move || match child.wait() {
                    Ok(status) => {
                        if !status.success() {
                            log::warn!(
                                "[plugin:{plugin_name}] Command exited with {status}: {cmd_for_log}"
                            );
                        }
                    }
                    Err(e) => {
                        log::error!("[plugin:{plugin_name}] Failed to wait for command: {e}");
                    }
                });
                true
            }
            Err(e) => {
                log::error!(
                    "[plugin:{}] Failed to execute command: {e}",
                    self.manifest.plugin.name
                );
                false
            }
        }
    }
}

impl Plugin for ConfigPlugin {
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
        if !self.manifest.commands.is_empty() {
            caps |= PluginCapabilities::COMMANDS;
        }
        if !self.manifest.keybindings.is_empty() {
            caps |= PluginCapabilities::KEYBOARD;
        }
        caps
    }

    fn min_editor_version(&self) -> Option<&'static str> {
        self.min_editor_version
    }

    fn init(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        log::info!(
            "[plugin:{}] Loaded from {}",
            self.manifest.plugin.name,
            self.path.display()
        );
        Ok(())
    }

    fn activate(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        self.active = true;
        log::info!("[plugin:{}] Activated", self.manifest.plugin.name);
        Ok(())
    }

    fn deactivate(&mut self, _ctx: &PluginContext) -> PluginResult<()> {
        self.active = false;
        log::info!("[plugin:{}] Deactivated", self.manifest.plugin.name);
        Ok(())
    }

    fn commands(&self) -> Vec<CommandConfig> {
        self.manifest
            .commands
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
        self.manifest
            .keybindings
            .iter()
            .map(|k| KeybindingConfig {
                keys: k.keys.clone(),
                command: k.command.clone(),
                description: k.description.clone(),
                modes: k.modes.clone(),
            })
            .collect()
    }

    fn execute_command(&mut self, command: &str, args: &str, ctx: &PluginContext) -> bool {
        let cmd_config = self
            .manifest
            .commands
            .iter()
            .find(|c| c.name == command || c.aliases.contains(&command.to_string()));

        let Some(cmd) = cmd_config else {
            return false;
        };

        // Handle shell command
        if let Some(shell) = &cmd.shell {
            return self.execute_shell(shell, args);
        }

        // Handle URL opening
        if let Some(url) = &cmd.url {
            let url = if args.is_empty() {
                url.clone()
            } else {
                format!("{url}{args}")
            };
            log::info!("[plugin:{}] Opening URL: {url}", self.manifest.plugin.name);
            if let Err(e) = open::that(&url) {
                log::error!(
                    "[plugin:{}] Failed to open URL: {e}",
                    self.manifest.plugin.name
                );
                ctx.notify("error", &format!("Failed to open URL: {e}"));
            }
            return true;
        }

        // Handle notify message
        if let Some(msg) = &cmd.notify {
            ctx.notify("info", msg);
            return true;
        }

        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Scan a directory for plugin files with a specific extension.
fn scan_directory_with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut plugins = Vec::new();

    if !dir.exists() {
        return plugins;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        log::warn!("Failed to read plugin directory: {}", dir.display());
        return plugins;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(file_ext) = path.extension() {
                if file_ext == ext {
                    plugins.push(path);
                }
            }
        }
    }

    plugins
}

/// Plugin loader that discovers and loads plugins from the filesystem.
pub struct PluginLoader {
    /// User plugin directory (~/.enya/plugins/)
    user_dir: Option<PathBuf>,
    /// Workspace plugin directory (.enya/plugins/)
    workspace_dir: Option<PathBuf>,
}

impl PluginLoader {
    /// Create a new plugin loader.
    pub fn new() -> Self {
        Self {
            user_dir: Self::default_user_plugin_dir(),
            workspace_dir: None,
        }
    }

    /// Set the workspace plugin directory.
    pub fn with_workspace_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.workspace_dir = Some(dir.into());
        self
    }

    /// Get the default user plugin directory (`~/.enya/plugins/`).
    fn default_user_plugin_dir() -> Option<PathBuf> {
        Some(enya_config::plugins_dir())
    }

    /// Discover all plugin files in the configured directories.
    pub fn discover(&self) -> Vec<PathBuf> {
        let mut plugins = Vec::new();

        // Load from user directory
        if let Some(ref user_dir) = self.user_dir {
            plugins.extend(Self::scan_directory(user_dir));
        }

        // Load from workspace directory
        if let Some(ref workspace_dir) = self.workspace_dir {
            plugins.extend(Self::scan_directory(workspace_dir));
        }

        plugins
    }

    /// Scan a directory for plugin files with .toml extension.
    fn scan_directory(dir: &Path) -> Vec<PathBuf> {
        scan_directory_with_ext(dir, "toml")
    }

    /// Load all discovered plugins.
    pub fn load_all(&self) -> Vec<PluginResult<ConfigPlugin>> {
        self.discover()
            .into_iter()
            .map(|path| ConfigPlugin::load(&path))
            .collect()
    }

    /// Ensure the user plugin directory exists.
    pub fn ensure_user_dir(&self) -> std::io::Result<()> {
        if let Some(ref dir) = self.user_dir {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Get the user plugin directory path.
    pub fn user_plugin_dir(&self) -> Option<&Path> {
        self.user_dir.as_deref()
    }

    /// Create an example plugin file.
    pub fn create_example_plugin(&self) -> std::io::Result<PathBuf> {
        let dir = self
            .user_dir
            .as_ref()
            .ok_or_else(|| std::io::Error::other("No user plugin directory"))?;

        std::fs::create_dir_all(dir)?;

        let example_path = dir.join("example.toml");
        std::fs::write(&example_path, EXAMPLE_PLUGIN)?;

        Ok(example_path)
    }

    /// Discover all Lua plugin files in the configured directories.
    pub fn discover_lua(&self) -> Vec<PathBuf> {
        let mut plugins = Vec::new();

        // Load from user directory
        if let Some(ref user_dir) = self.user_dir {
            plugins.extend(Self::scan_directory_lua(user_dir));
        }

        // Load from workspace directory
        if let Some(ref workspace_dir) = self.workspace_dir {
            plugins.extend(Self::scan_directory_lua(workspace_dir));
        }

        plugins
    }

    /// Scan a directory for Lua plugin files.
    fn scan_directory_lua(dir: &Path) -> Vec<PathBuf> {
        scan_directory_with_ext(dir, "lua")
    }

    /// Load all discovered Lua plugins.
    pub fn load_all_lua(&self) -> Vec<PluginResult<super::lua::LuaPlugin>> {
        self.discover_lua()
            .into_iter()
            .map(|path| super::lua::LuaPlugin::load(&path))
            .collect()
    }

    /// Create an example Lua plugin file.
    pub fn create_example_lua_plugin(&self) -> std::io::Result<PathBuf> {
        let dir = self
            .user_dir
            .as_ref()
            .ok_or_else(|| std::io::Error::other("No user plugin directory"))?;

        std::fs::create_dir_all(dir)?;

        let example_path = dir.join("example.lua");
        std::fs::write(&example_path, super::lua::EXAMPLE_LUA_PLUGIN)?;

        Ok(example_path)
    }
}

impl Default for PluginLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Example plugin template.
pub const EXAMPLE_PLUGIN: &str = r#"# Example Enya Plugin
# Place this file in ~/.enya/plugins/

[plugin]
name = "example"
version = "0.1.0"
description = "An example plugin showing available features"
author = "Your Name"
enabled = true

# Commands are accessible via the command palette (:command-name)
[[commands]]
name = "hello"
description = "Display a greeting message"
notify = "Hello from the example plugin!"

[[commands]]
name = "open-docs"
aliases = ["docs"]
description = "Open Enya documentation"
url = "https://enya.build/docs"

[[commands]]
name = "run-tests"
description = "Run tests in current directory"
# Shell commands run asynchronously
shell = "cargo test"
accepts_args = true

# Keybindings for quick access
# Format: "Modifier+Key" (Space for leader key)
[[keybindings]]
keys = "Space+x+h"
command = "hello"
description = "Say hello"

[[keybindings]]
keys = "Space+x+d"
command = "open-docs"
description = "Open docs"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
            [plugin]
            name = "minimal"
            version = "1.0.0"
        "#;

        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.plugin.name, "minimal");
        assert_eq!(manifest.plugin.version, "1.0.0");
        assert!(manifest.plugin.description.is_empty());
        assert!(manifest.plugin.enabled); // default true
        assert!(manifest.commands.is_empty());
        assert!(manifest.keybindings.is_empty());
    }

    #[test]
    fn test_parse_full_manifest() {
        let toml = r#"
            [plugin]
            name = "full-plugin"
            version = "2.0.0"
            description = "A fully configured plugin"
            author = "Test Author"
            homepage = "https://example.com"
            min_enya_version = "1.0.0"
            enabled = false

            [[commands]]
            name = "test-cmd"
            description = "A test command"
            aliases = ["tc", "test"]
            shell = "echo hello"
            accepts_args = true

            [[commands]]
            name = "open-url"
            url = "https://example.com"

            [[commands]]
            name = "notify-cmd"
            notify = "Hello!"

            [[keybindings]]
            keys = "Space+t+t"
            command = "test-cmd"
            description = "Run test"
            modes = ["normal", "visual"]
        "#;

        let manifest: PluginManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.plugin.name, "full-plugin");
        assert_eq!(manifest.plugin.version, "2.0.0");
        assert_eq!(manifest.plugin.description, "A fully configured plugin");
        assert_eq!(manifest.plugin.author, "Test Author");
        assert!(!manifest.plugin.enabled);
        assert_eq!(manifest.plugin.min_enya_version, Some("1.0.0".to_string()));

        assert_eq!(manifest.commands.len(), 3);
        let cmd = &manifest.commands[0];
        assert_eq!(cmd.name, "test-cmd");
        assert_eq!(cmd.aliases, vec!["tc", "test"]);
        assert_eq!(cmd.shell, Some("echo hello".to_string()));
        assert!(cmd.accepts_args);

        let url_cmd = &manifest.commands[1];
        assert_eq!(url_cmd.url, Some("https://example.com".to_string()));

        let notify_cmd = &manifest.commands[2];
        assert_eq!(notify_cmd.notify, Some("Hello!".to_string()));

        assert_eq!(manifest.keybindings.len(), 1);
        let kb = &manifest.keybindings[0];
        assert_eq!(kb.keys, "Space+t+t");
        assert_eq!(kb.command, "test-cmd");
        assert_eq!(kb.modes, vec!["normal", "visual"]);
    }

    #[test]
    fn test_parse_invalid_toml() {
        let toml = r#"
            [plugin
            name = "broken"
        "#;

        let result: Result<PluginManifest, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_required_fields() {
        // Missing version
        let toml = r#"
            [plugin]
            name = "no-version"
        "#;

        let result: Result<PluginManifest, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_plugin_capabilities() {
        // Empty plugin - no capabilities
        let empty_toml = r#"
            [plugin]
            name = "empty"
            version = "1.0.0"
        "#;
        let manifest: PluginManifest = toml::from_str(empty_toml).unwrap();
        let plugin = ConfigPlugin::from_manifest(manifest, PathBuf::from("test.toml"));
        assert!(plugin.capabilities().is_empty());

        // With commands
        let cmd_toml = r#"
            [plugin]
            name = "with-cmd"
            version = "1.0.0"

            [[commands]]
            name = "test"
        "#;
        let manifest: PluginManifest = toml::from_str(cmd_toml).unwrap();
        let plugin = ConfigPlugin::from_manifest(manifest, PathBuf::from("test.toml"));
        assert!(plugin.capabilities().contains(PluginCapabilities::COMMANDS));

        // With keybindings
        let kb_toml = r#"
            [plugin]
            name = "with-kb"
            version = "1.0.0"

            [[keybindings]]
            keys = "Space+t"
            command = "test"
        "#;
        let manifest: PluginManifest = toml::from_str(kb_toml).unwrap();
        let plugin = ConfigPlugin::from_manifest(manifest, PathBuf::from("test.toml"));
        assert!(plugin.capabilities().contains(PluginCapabilities::KEYBOARD));
    }

    #[test]
    fn test_plugin_loader_with_workspace() {
        let loader = PluginLoader::new().with_workspace_dir("/tmp/test-workspace");
        assert!(loader.workspace_dir.is_some());
        assert_eq!(
            loader.workspace_dir.unwrap(),
            PathBuf::from("/tmp/test-workspace")
        );
    }

    #[test]
    fn test_discover_nonexistent_directory() {
        let loader = PluginLoader {
            user_dir: Some(PathBuf::from("/nonexistent/path/12345")),
            workspace_dir: None,
        };
        let discovered = loader.discover();
        assert!(discovered.is_empty());
    }

    #[test]
    fn test_example_plugin_parses() {
        // Ensure the example plugin constant is valid TOML
        let manifest: PluginManifest = toml::from_str(EXAMPLE_PLUGIN).unwrap();
        assert_eq!(manifest.plugin.name, "example");
        assert!(!manifest.commands.is_empty());
        assert!(!manifest.keybindings.is_empty());
    }
}
