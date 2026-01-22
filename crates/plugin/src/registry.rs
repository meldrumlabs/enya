//! Plugin registry for managing plugin lifecycle.

use rustc_hash::FxHashMap;

use crate::hooks::{
    CommandHook, CommandHookResult, KeyCombo, KeyEvent, KeyboardHook, KeyboardHookResult,
    LifecycleHook, PaneHook, ThemeHook,
};
use crate::theme::ThemeDefinition;
use crate::traits::{CommandConfig, KeybindingConfig, PaneConfig, Plugin, PluginCapabilities};
use crate::types::{PluginContext, Theme};
use crate::{PluginError, PluginResult};

/// Unique identifier for a registered plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PluginId(usize);

impl PluginId {
    /// Get the inner numeric value.
    pub fn value(&self) -> usize {
        self.0
    }
}

/// Runtime state of a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    /// Plugin is registered but not initialized
    Registered,
    /// Plugin is initialized but not active
    Inactive,
    /// Plugin is active and running
    Active,
    /// Plugin failed to initialize or activate
    Failed,
    /// Plugin is disabled by user
    Disabled,
}

/// Information about a registered plugin.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin identifier
    pub id: PluginId,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin capabilities
    pub capabilities: PluginCapabilities,
    /// Current state
    pub state: PluginState,
    /// Whether the plugin is enabled by default
    pub enabled_by_default: bool,
}

/// Registry entry for a plugin.
struct PluginEntry {
    /// The plugin instance
    plugin: Box<dyn Plugin>,
    /// Plugin metadata
    info: PluginInfo,
    /// Commands provided by the plugin
    commands: Vec<CommandConfig>,
    /// Pane types provided by the plugin
    pane_types: Vec<PaneConfig>,
    /// Keybindings provided by the plugin
    keybindings: Vec<KeybindingConfig>,
    /// Lifecycle hooks
    lifecycle_hook: Option<Box<dyn LifecycleHook>>,
    /// Command hooks
    command_hook: Option<Box<dyn CommandHook>>,
    /// Keyboard hooks
    keyboard_hook: Option<Box<dyn KeyboardHook>>,
    /// Theme hooks
    theme_hook: Option<Box<dyn ThemeHook>>,
    /// Pane hooks
    pane_hook: Option<Box<dyn PaneHook>>,
}

/// Central registry for managing plugins.
///
/// The registry handles plugin lifecycle:
/// - Registration: Adding plugins to the system
/// - Initialization: Setting up plugins with context
/// - Activation/Deactivation: Enabling/disabling plugins
/// - Hook dispatch: Routing events to interested plugins
pub struct PluginRegistry {
    /// Registered plugins by ID
    plugins: FxHashMap<PluginId, PluginEntry>,
    /// Plugin name to ID mapping
    name_to_id: FxHashMap<String, PluginId>,
    /// Next plugin ID
    next_id: usize,
    /// Plugin context (shared with all plugins)
    context: Option<PluginContext>,
    /// Plugins enabled by user configuration
    enabled_plugins: FxHashMap<String, bool>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    /// Create a new empty plugin registry.
    pub fn new() -> Self {
        Self {
            plugins: FxHashMap::default(),
            name_to_id: FxHashMap::default(),
            next_id: 0,
            context: None,
            enabled_plugins: FxHashMap::default(),
        }
    }

    /// Initialize the registry with the plugin context.
    pub fn init(&mut self, context: PluginContext) {
        self.context = Some(context);
    }

    /// Get a reference to the plugin context.
    pub fn context(&self) -> Option<&PluginContext> {
        self.context.as_ref()
    }

    /// Set the enabled state for a plugin by name.
    pub fn set_plugin_enabled(&mut self, name: &str, enabled: bool) {
        self.enabled_plugins.insert(name.to_string(), enabled);
    }

    /// Check if a plugin is enabled.
    pub fn is_plugin_enabled(&self, name: &str) -> bool {
        self.enabled_plugins.get(name).copied().unwrap_or(true)
    }

    /// Register a plugin with the registry.
    ///
    /// This does not initialize or activate the plugin - call `init_plugin`
    /// and `activate_plugin` separately.
    pub fn register<P: Plugin + 'static>(
        &mut self,
        plugin: P,
        enabled_by_default: bool,
    ) -> PluginResult<PluginId> {
        let name = plugin.name().to_string();

        if self.name_to_id.contains_key(&name) {
            return Err(PluginError::AlreadyRegistered(name));
        }

        let id = PluginId(self.next_id);
        self.next_id += 1;

        let info = PluginInfo {
            id,
            name: name.clone(),
            version: plugin.version().to_string(),
            description: plugin.description().to_string(),
            capabilities: plugin.capabilities(),
            state: PluginState::Registered,
            enabled_by_default,
        };

        let entry = PluginEntry {
            plugin: Box::new(plugin),
            info,
            commands: vec![],
            pane_types: vec![],
            keybindings: vec![],
            lifecycle_hook: None,
            command_hook: None,
            keyboard_hook: None,
            theme_hook: None,
            pane_hook: None,
        };

        self.plugins.insert(id, entry);
        self.name_to_id.insert(name, id);

        Ok(id)
    }

    /// Initialize a registered plugin.
    pub fn init_plugin(&mut self, id: PluginId) -> PluginResult<()> {
        let ctx = self
            .context
            .as_ref()
            .ok_or_else(|| PluginError::OperationFailed("Registry not initialized".to_string()))?;

        let entry = self
            .plugins
            .get_mut(&id)
            .ok_or_else(|| PluginError::NotFound(format!("Plugin ID {}", id.0)))?;

        if entry.info.state != PluginState::Registered {
            return Ok(()); // Already initialized
        }

        // Check minimum editor version
        if let Some(min_version) = entry.plugin.min_editor_version() {
            let current = ctx.editor_version();
            if !Self::check_version(current, min_version) {
                entry.info.state = PluginState::Failed;
                return Err(PluginError::IncompatibleVersion {
                    required: min_version.to_string(),
                    actual: current.to_string(),
                });
            }
        }

        // Initialize the plugin
        if let Err(e) = entry.plugin.init(ctx) {
            entry.info.state = PluginState::Failed;
            return Err(e);
        }

        // Collect plugin-provided items
        entry.commands = entry.plugin.commands();
        entry.pane_types = entry.plugin.pane_types();
        entry.keybindings = entry.plugin.keybindings();

        // Collect hooks
        entry.lifecycle_hook = entry.plugin.lifecycle_hooks();
        entry.command_hook = entry.plugin.command_hooks();
        entry.keyboard_hook = entry.plugin.keyboard_hooks();
        entry.theme_hook = entry.plugin.theme_hooks();
        entry.pane_hook = entry.plugin.pane_hooks();

        entry.info.state = PluginState::Inactive;
        Ok(())
    }

    /// Activate a plugin (must be initialized first).
    pub fn activate_plugin(&mut self, id: PluginId) -> PluginResult<()> {
        let ctx = self
            .context
            .as_ref()
            .ok_or_else(|| PluginError::OperationFailed("Registry not initialized".to_string()))?;

        let entry = self
            .plugins
            .get_mut(&id)
            .ok_or_else(|| PluginError::NotFound(format!("Plugin ID {}", id.0)))?;

        if entry.info.state != PluginState::Inactive {
            return Ok(()); // Already active or in wrong state
        }

        // Check if user has disabled this plugin
        if !self
            .enabled_plugins
            .get(&entry.info.name)
            .copied()
            .unwrap_or(entry.info.enabled_by_default)
        {
            entry.info.state = PluginState::Disabled;
            return Ok(());
        }

        if let Err(e) = entry.plugin.activate(ctx) {
            entry.info.state = PluginState::Failed;
            return Err(e);
        }

        entry.info.state = PluginState::Active;
        Ok(())
    }

    /// Deactivate a plugin.
    pub fn deactivate_plugin(&mut self, id: PluginId) -> PluginResult<()> {
        let ctx = self
            .context
            .as_ref()
            .ok_or_else(|| PluginError::OperationFailed("Registry not initialized".to_string()))?;

        let entry = self
            .plugins
            .get_mut(&id)
            .ok_or_else(|| PluginError::NotFound(format!("Plugin ID {}", id.0)))?;

        if entry.info.state != PluginState::Active {
            return Ok(()); // Not active
        }

        if let Err(e) = entry.plugin.deactivate(ctx) {
            log::warn!("Plugin {} deactivation error: {e}", entry.info.name);
        }

        entry.info.state = PluginState::Inactive;
        Ok(())
    }

    /// Get a plugin by ID.
    pub fn get(&self, id: PluginId) -> Option<&dyn Plugin> {
        self.plugins.get(&id).map(|e| e.plugin.as_ref())
    }

    /// Get a mutable plugin by ID.
    pub fn get_mut(&mut self, id: PluginId) -> Option<&mut dyn Plugin> {
        self.plugins.get_mut(&id).map(|e| e.plugin.as_mut())
    }

    /// Get a plugin by name.
    pub fn get_by_name(&self, name: &str) -> Option<&dyn Plugin> {
        self.name_to_id
            .get(name)
            .and_then(|id| self.plugins.get(id))
            .map(|e| e.plugin.as_ref())
    }

    /// Get plugin info by ID.
    pub fn info(&self, id: PluginId) -> Option<&PluginInfo> {
        self.plugins.get(&id).map(|e| &e.info)
    }

    /// Get plugin info by name.
    pub fn info_by_name(&self, name: &str) -> Option<&PluginInfo> {
        self.name_to_id
            .get(name)
            .and_then(|id| self.plugins.get(id))
            .map(|e| &e.info)
    }

    /// List all registered plugins.
    pub fn list_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins.values().map(|e| &e.info).collect()
    }

    /// List active plugins.
    pub fn active_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins
            .values()
            .filter(|e| e.info.state == PluginState::Active)
            .map(|e| &e.info)
            .collect()
    }

    /// Get all commands from active plugins.
    pub fn all_commands(&self) -> Vec<(&PluginInfo, &CommandConfig)> {
        self.plugins
            .values()
            .filter(|e| e.info.state == PluginState::Active)
            .flat_map(|e| e.commands.iter().map(move |c| (&e.info, c)))
            .collect()
    }

    /// Get all pane types from active plugins.
    pub fn all_pane_types(&self) -> Vec<(&PluginInfo, &PaneConfig)> {
        self.plugins
            .values()
            .filter(|e| e.info.state == PluginState::Active)
            .flat_map(|e| e.pane_types.iter().map(move |p| (&e.info, p)))
            .collect()
    }

    /// Get all keybindings from active plugins.
    pub fn all_keybindings(&self) -> Vec<(&PluginInfo, &KeybindingConfig)> {
        self.plugins
            .values()
            .filter(|e| e.info.state == PluginState::Active)
            .flat_map(|e| e.keybindings.iter().map(move |k| (&e.info, k)))
            .collect()
    }

    /// Get all custom themes from active plugins.
    pub fn all_themes(&self) -> Vec<ThemeDefinition> {
        self.plugins
            .values()
            .filter(|e| e.info.state == PluginState::Active)
            .flat_map(|e| e.plugin.themes())
            .collect()
    }

    /// Get commands for a specific plugin.
    pub fn commands_for_plugin(&self, id: PluginId) -> Vec<&CommandConfig> {
        self.plugins
            .get(&id)
            .map(|e| e.commands.iter().collect())
            .unwrap_or_default()
    }

    /// Get keybindings for a specific plugin.
    pub fn keybindings_for_plugin(&self, id: PluginId) -> Vec<&KeybindingConfig> {
        self.plugins
            .get(&id)
            .map(|e| e.keybindings.iter().collect())
            .unwrap_or_default()
    }

    /// Execute a plugin command.
    pub fn execute_command(&mut self, command: &str, args: &str) -> bool {
        let ctx = match &self.context {
            Some(c) => c,
            None => return false,
        };

        for entry in self.plugins.values_mut() {
            if entry.info.state != PluginState::Active {
                continue;
            }

            if entry
                .commands
                .iter()
                .any(|c| c.name == command || c.aliases.contains(&command.to_string()))
                && entry.plugin.execute_command(command, args, ctx)
            {
                return true;
            }
        }

        false
    }

    // ==================== Hook Dispatch ====================

    /// Dispatch lifecycle: workspace loaded.
    pub fn on_workspace_loaded(&mut self) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.lifecycle_hook {
                    hook.on_workspace_loaded();
                }
            }
        }
    }

    /// Dispatch lifecycle: workspace saving.
    pub fn on_workspace_saving(&mut self) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.lifecycle_hook {
                    hook.on_workspace_saving();
                }
            }
        }
    }

    /// Dispatch lifecycle: pane added.
    pub fn on_pane_added(&mut self, pane_id: usize) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.lifecycle_hook {
                    hook.on_pane_added(pane_id);
                }
            }
        }
    }

    /// Dispatch lifecycle: pane removing.
    pub fn on_pane_removing(&mut self, pane_id: usize) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.lifecycle_hook {
                    hook.on_pane_removing(pane_id);
                }
            }
        }
    }

    /// Dispatch lifecycle: pane focused.
    pub fn on_pane_focused(&mut self, pane_id: usize) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.lifecycle_hook {
                    hook.on_pane_focused(pane_id);
                }
            }
        }
    }

    /// Dispatch lifecycle: closing.
    pub fn on_closing(&mut self) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.lifecycle_hook {
                    hook.on_closing();
                }
            }
        }
    }

    /// Dispatch lifecycle: frame update.
    pub fn on_frame(&mut self) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.lifecycle_hook {
                    hook.on_frame();
                }
            }
        }
    }

    /// Dispatch command hook: before command.
    pub fn before_command(&mut self, command: &str, args: &str) -> CommandHookResult {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.command_hook {
                    let result = hook.before_command(command, args);
                    if result != CommandHookResult::Continue {
                        return result;
                    }
                }
            }
        }
        CommandHookResult::Continue
    }

    /// Dispatch command hook: after command.
    pub fn after_command(&mut self, command: &str, args: &str, success: bool) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.command_hook {
                    hook.after_command(command, args, success);
                }
            }
        }
    }

    /// Dispatch keyboard hook: key pressed.
    pub fn on_key_pressed(&mut self, key: &KeyEvent) -> KeyboardHookResult {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.keyboard_hook {
                    let result = hook.on_key_pressed(key);
                    if result != KeyboardHookResult::Continue {
                        return result;
                    }
                }
            }
        }
        KeyboardHookResult::Continue
    }

    /// Dispatch keyboard hook: key combo.
    pub fn on_key_combo(&mut self, combo: &KeyCombo) -> KeyboardHookResult {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.keyboard_hook {
                    let result = hook.on_key_combo(combo);
                    if result != KeyboardHookResult::Continue {
                        return result;
                    }
                }
            }
        }
        KeyboardHookResult::Continue
    }

    /// Dispatch theme hook: theme changing.
    pub fn on_theme_changing(&mut self, old_theme: Theme, new_theme: Theme) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.theme_hook {
                    hook.before_theme_change(old_theme, new_theme);
                }
            }
        }
    }

    /// Dispatch theme hook: theme changed.
    pub fn on_theme_changed(&mut self, theme: Theme) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                // Notify the plugin trait method
                entry.plugin.on_theme_changed(theme);
                // Notify the theme hook
                if let Some(ref mut hook) = entry.theme_hook {
                    hook.after_theme_change(theme);
                }
            }
        }
    }

    /// Dispatch pane hook: pane created.
    pub fn on_pane_created(&mut self, pane_id: usize, pane_type: &str) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.pane_hook {
                    hook.on_pane_created(pane_id, pane_type);
                }
            }
        }
    }

    /// Dispatch pane hook: query changed.
    pub fn on_query_changed(&mut self, pane_id: usize, query: &str) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.pane_hook {
                    hook.on_query_changed(pane_id, query);
                }
            }
        }
    }

    /// Dispatch pane hook: data received.
    pub fn on_data_received(&mut self, pane_id: usize) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.pane_hook {
                    hook.on_data_received(pane_id);
                }
            }
        }
    }

    /// Dispatch pane hook: pane error.
    pub fn on_pane_error(&mut self, pane_id: usize, error: &str) {
        for entry in self.plugins.values_mut() {
            if entry.info.state == PluginState::Active {
                if let Some(ref mut hook) = entry.pane_hook {
                    hook.on_pane_error(pane_id, error);
                }
            }
        }
    }

    // ==================== Private Helpers ====================

    /// Simple semver check (major.minor.patch).
    fn check_version(current: &str, required: &str) -> bool {
        let parse = |v: &str| -> (u32, u32, u32) {
            let parts: Vec<&str> = v.split('.').collect();
            (
                parts.first().and_then(|s| s.parse().ok()).unwrap_or(0),
                parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
                parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0),
            )
        };

        let curr = parse(current);
        let req = parse(required);

        // Current must be >= required
        curr >= req
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        BoxFuture, HttpError, HttpResponse, LogLevel, NotificationLevel, PluginHost,
    };
    use std::any::Any;
    use std::sync::Arc;

    /// Mock plugin host for testing.
    struct MockPluginHost;

    impl PluginHost for MockPluginHost {
        fn notify(&self, _level: NotificationLevel, _message: &str) {}
        fn request_repaint(&self) {}
        fn log(&self, _level: LogLevel, _message: &str) {}
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
        fn clipboard_write(&self, _text: &str) -> bool {
            true
        }
        fn clipboard_read(&self) -> Option<String> {
            None
        }
        fn spawn(&self, _future: BoxFuture<()>) {}
        fn http_get(
            &self,
            _url: &str,
            _headers: &rustc_hash::FxHashMap<String, String>,
        ) -> Result<HttpResponse, HttpError> {
            Err(HttpError {
                message: "Not implemented".to_string(),
            })
        }
        fn http_post(
            &self,
            _url: &str,
            _body: &str,
            _headers: &rustc_hash::FxHashMap<String, String>,
        ) -> Result<HttpResponse, HttpError> {
            Err(HttpError {
                message: "Not implemented".to_string(),
            })
        }
    }

    /// Simple test plugin for testing registry operations.
    struct TestPlugin {
        name: &'static str,
        version: &'static str,
        min_version: Option<&'static str>,
        commands: Vec<CommandConfig>,
        executed_commands: std::sync::atomic::AtomicUsize,
    }

    impl TestPlugin {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                version: "1.0.0",
                min_version: None,
                commands: vec![],
                executed_commands: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn with_version(mut self, version: &'static str) -> Self {
            self.version = version;
            self
        }

        fn with_min_version(mut self, min_version: &'static str) -> Self {
            self.min_version = Some(min_version);
            self
        }

        fn with_command(mut self, name: &str) -> Self {
            self.commands.push(CommandConfig {
                name: name.to_string(),
                aliases: vec![],
                description: format!("Test command: {name}"),
                accepts_args: false,
            });
            self
        }
    }

    impl crate::traits::Plugin for TestPlugin {
        fn name(&self) -> &'static str {
            self.name
        }

        fn version(&self) -> &'static str {
            self.version
        }

        fn description(&self) -> &'static str {
            "A test plugin"
        }

        fn capabilities(&self) -> PluginCapabilities {
            if self.commands.is_empty() {
                PluginCapabilities::empty()
            } else {
                PluginCapabilities::COMMANDS
            }
        }

        fn min_editor_version(&self) -> Option<&'static str> {
            self.min_version
        }

        fn init(&mut self, _ctx: &PluginContext) -> crate::PluginResult<()> {
            Ok(())
        }

        fn commands(&self) -> Vec<CommandConfig> {
            self.commands.clone()
        }

        fn execute_command(&mut self, command: &str, _args: &str, _ctx: &PluginContext) -> bool {
            if self.commands.iter().any(|c| c.name == command) {
                self.executed_commands
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                true
            } else {
                false
            }
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    fn create_test_context() -> PluginContext {
        PluginContext::new(Arc::new(MockPluginHost))
    }

    #[test]
    fn test_registry_new() {
        let registry = PluginRegistry::new();
        assert!(registry.list_plugins().is_empty());
        assert!(registry.context().is_none());
    }

    #[test]
    fn test_registry_init() {
        let mut registry = PluginRegistry::new();
        let ctx = create_test_context();
        registry.init(ctx);
        assert!(registry.context().is_some());
    }

    #[test]
    fn test_register_plugin() {
        let mut registry = PluginRegistry::new();
        let plugin = TestPlugin::new("test-plugin").with_version("2.5.0");

        let id = registry.register(plugin, true).unwrap();
        assert_eq!(id.value(), 0);
        assert_eq!(registry.list_plugins().len(), 1);

        let info = registry.info(id).unwrap();
        assert_eq!(info.name, "test-plugin");
        assert_eq!(info.version, "2.5.0");
        assert_eq!(info.state, PluginState::Registered);
    }

    #[test]
    fn test_register_duplicate_fails() {
        let mut registry = PluginRegistry::new();
        registry.register(TestPlugin::new("dupe"), true).unwrap();

        let result = registry.register(TestPlugin::new("dupe"), true);
        assert!(matches!(result, Err(PluginError::AlreadyRegistered(_))));
    }

    #[test]
    fn test_plugin_lifecycle() {
        let mut registry = PluginRegistry::new();
        registry.init(create_test_context());

        let id = registry
            .register(TestPlugin::new("lifecycle"), true)
            .unwrap();

        // After register: Registered state
        assert_eq!(registry.info(id).unwrap().state, PluginState::Registered);

        // After init: Inactive state
        registry.init_plugin(id).unwrap();
        assert_eq!(registry.info(id).unwrap().state, PluginState::Inactive);

        // After activate: Active state
        registry.activate_plugin(id).unwrap();
        assert_eq!(registry.info(id).unwrap().state, PluginState::Active);

        // After deactivate: Inactive state
        registry.deactivate_plugin(id).unwrap();
        assert_eq!(registry.info(id).unwrap().state, PluginState::Inactive);
    }

    #[test]
    fn test_active_plugins() {
        let mut registry = PluginRegistry::new();
        registry.init(create_test_context());

        let id1 = registry
            .register(TestPlugin::new("plugin-1"), true)
            .unwrap();
        let id2 = registry
            .register(TestPlugin::new("plugin-2"), true)
            .unwrap();

        // Neither active yet
        assert!(registry.active_plugins().is_empty());

        // Activate first
        registry.init_plugin(id1).unwrap();
        registry.activate_plugin(id1).unwrap();
        assert_eq!(registry.active_plugins().len(), 1);

        // Activate second
        registry.init_plugin(id2).unwrap();
        registry.activate_plugin(id2).unwrap();
        assert_eq!(registry.active_plugins().len(), 2);
    }

    #[test]
    fn test_get_by_name() {
        let mut registry = PluginRegistry::new();
        registry
            .register(TestPlugin::new("named-plugin"), true)
            .unwrap();

        assert!(registry.get_by_name("named-plugin").is_some());
        assert!(registry.get_by_name("nonexistent").is_none());

        let info = registry.info_by_name("named-plugin").unwrap();
        assert_eq!(info.name, "named-plugin");
    }

    #[test]
    fn test_version_check() {
        // Equal versions
        assert!(PluginRegistry::check_version("1.0.0", "1.0.0"));

        // Current > required
        assert!(PluginRegistry::check_version("2.0.0", "1.0.0"));
        assert!(PluginRegistry::check_version("1.1.0", "1.0.0"));
        assert!(PluginRegistry::check_version("1.0.1", "1.0.0"));

        // Current < required
        assert!(!PluginRegistry::check_version("1.0.0", "2.0.0"));
        assert!(!PluginRegistry::check_version("1.0.0", "1.1.0"));
        assert!(!PluginRegistry::check_version("1.0.0", "1.0.1"));

        // Partial versions
        assert!(PluginRegistry::check_version("1.0", "1.0.0"));
        assert!(PluginRegistry::check_version("1", "1.0.0"));
    }

    #[test]
    fn test_min_version_enforcement() {
        let mut registry = PluginRegistry::new();
        registry.init(create_test_context()); // Host version is "1.0.0"

        // Plugin requires 2.0.0 but host is 1.0.0
        let id = registry
            .register(
                TestPlugin::new("future-plugin").with_min_version("2.0.0"),
                true,
            )
            .unwrap();

        let result = registry.init_plugin(id);
        assert!(matches!(
            result,
            Err(PluginError::IncompatibleVersion { .. })
        ));
        assert_eq!(registry.info(id).unwrap().state, PluginState::Failed);
    }

    #[test]
    fn test_command_collection() {
        let mut registry = PluginRegistry::new();
        registry.init(create_test_context());

        let id = registry
            .register(
                TestPlugin::new("cmd-plugin")
                    .with_command("cmd-1")
                    .with_command("cmd-2"),
                true,
            )
            .unwrap();

        registry.init_plugin(id).unwrap();
        registry.activate_plugin(id).unwrap();

        let commands = registry.all_commands();
        assert_eq!(commands.len(), 2);

        let plugin_cmds = registry.commands_for_plugin(id);
        assert_eq!(plugin_cmds.len(), 2);
    }

    #[test]
    fn test_execute_command() {
        let mut registry = PluginRegistry::new();
        registry.init(create_test_context());

        let id = registry
            .register(TestPlugin::new("exec-plugin").with_command("my-cmd"), true)
            .unwrap();

        registry.init_plugin(id).unwrap();
        registry.activate_plugin(id).unwrap();

        // Execute existing command
        assert!(registry.execute_command("my-cmd", ""));

        // Execute non-existent command
        assert!(!registry.execute_command("nonexistent", ""));
    }

    #[test]
    fn test_disabled_plugin() {
        let mut registry = PluginRegistry::new();
        registry.init(create_test_context());

        // Disable the plugin before activation
        registry.set_plugin_enabled("disabled-plugin", false);

        let id = registry
            .register(TestPlugin::new("disabled-plugin"), true)
            .unwrap();

        registry.init_plugin(id).unwrap();
        registry.activate_plugin(id).unwrap();

        // Plugin should be in Disabled state, not Active
        assert_eq!(registry.info(id).unwrap().state, PluginState::Disabled);
        assert!(registry.active_plugins().is_empty());
    }

    #[test]
    fn test_plugin_enabled_check() {
        let mut registry = PluginRegistry::new();

        // Unknown plugin defaults to enabled
        assert!(registry.is_plugin_enabled("unknown"));

        // Explicitly disabled
        registry.set_plugin_enabled("my-plugin", false);
        assert!(!registry.is_plugin_enabled("my-plugin"));

        // Explicitly enabled
        registry.set_plugin_enabled("my-plugin", true);
        assert!(registry.is_plugin_enabled("my-plugin"));
    }
}
