//! Plugin execution integration tests.
//!
//! These tests verify the full pipeline from Lua plugin loading through
//! `PluginRegistry` and `EditorPluginHost` to `UICommand` dispatch.

use std::path::Path;
use std::sync::Arc;

use enya_editor::AsyncRuntime;
use enya_editor::command::{UICommand, command_channel};
use enya_editor::plugin::{
    EditorPluginHost, LuaPlugin, PluginContext, PluginRegistry, PluginSharedState,
};
use enya_editor::ui::theme::AppTheme;
use parking_lot::RwLock;

/// Create an `AsyncRuntime` backed by a temporary tokio runtime.
fn test_async_runtime() -> (AsyncRuntime, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let async_rt = AsyncRuntime::new(rt.handle().clone());
    (async_rt, rt)
}

/// Create a fully-wired test context: `EditorPluginHost` → `PluginContext`,
/// plus the `CommandReceiver` so tests can observe dispatched commands.
fn test_context() -> (
    PluginContext,
    enya_editor::command::CommandReceiver,
    tokio::runtime::Runtime,
) {
    let (sender, receiver) = command_channel();
    let (async_rt, tokio_rt) = test_async_runtime();
    let shared_state = Arc::new(RwLock::new(PluginSharedState::default()));
    let host = EditorPluginHost::new(sender, async_rt, AppTheme::default(), shared_state);
    let ctx = PluginContext::new(Arc::new(host));
    (ctx, receiver, tokio_rt)
}

/// Helper to load a Lua plugin, register it in a registry, init, and activate.
fn setup_plugin(source: &str, registry: &mut PluginRegistry) -> enya_editor::plugin::PluginId {
    let plugin =
        LuaPlugin::load_from_source(source, Path::new("test.lua")).expect("load lua plugin");
    let id = registry.register(plugin, true).expect("register plugin");
    registry.init_plugin(id).expect("init plugin");
    registry.activate_plugin(id).expect("activate plugin");
    id
}

#[test]
fn test_plugin_command_sends_notify_ui_command() {
    let (ctx, receiver, _rt) = test_context();

    let mut registry = PluginRegistry::new();
    registry.init(ctx);

    let source = r#"
        plugin = { name = "notify-test", version = "0.1.0", description = "test" }
        enya.register_command("greet", {
            description = "Say hello",
        }, function(args)
            enya.notify("info", "Hello from plugin!")
            return true
        end)
    "#;

    setup_plugin(source, &mut registry);

    // Execute the command
    let executed = registry.execute_command("greet", "");
    assert!(executed, "command should execute successfully");

    // Verify the UICommand arrived
    let cmd = receiver.recv_ui().expect("should receive a UICommand");
    match cmd {
        UICommand::Notify { level, message } => {
            assert_eq!(level, "info");
            assert_eq!(message, "Hello from plugin!");
        }
        other => panic!("expected Notify, got {other:?}"),
    }
}

#[test]
fn test_plugin_command_execution_returns_success() {
    let (ctx, _receiver, _rt) = test_context();

    let mut registry = PluginRegistry::new();
    registry.init(ctx);

    let source = r#"
        plugin = { name = "success-test", version = "0.1.0", description = "test" }
        enya.register_command("noop", {
            description = "Do nothing",
        }, function(args)
            return true
        end)
    "#;

    setup_plugin(source, &mut registry);

    assert!(registry.execute_command("noop", ""));
    assert!(!registry.execute_command("nonexistent", ""));
}

#[test]
fn test_plugin_theme_is_collected() {
    let (ctx, _receiver, _rt) = test_context();

    let mut registry = PluginRegistry::new();
    registry.init(ctx);

    let source = r##"
        plugin = { name = "theme-test", version = "0.1.0", description = "test" }
        theme = {
            name = "my-custom-theme",
            display_name = "My Custom Theme",
            base = "dark",
            colors = {
                bg_primary = "#1a1b26",
                text_primary = "#c0caf5",
                accent_primary = "#7aa2f7",
            }
        }
    "##;

    setup_plugin(source, &mut registry);

    let themes = registry.all_themes();
    assert_eq!(themes.len(), 1, "should have one theme");
    assert_eq!(themes[0].name, "my-custom-theme");
    assert_eq!(themes[0].display_name, "My Custom Theme");
}

#[test]
fn test_plugin_registers_custom_table_pane() {
    let (ctx, _receiver, _rt) = test_context();

    let mut registry = PluginRegistry::new();
    registry.init(ctx);

    let source = r#"
        plugin = { name = "table-test", version = "0.1.0", description = "test" }
        enya.register_table_pane("my-table", {
            title = "My Table",
            columns = {
                { name = "Name" },
                { name = "Value" },
            },
            refresh_interval = 5,
        }, function()
            return { rows = {} }
        end)
    "#;

    setup_plugin(source, &mut registry);

    let tables = registry.all_custom_table_panes();
    assert_eq!(tables.len(), 1, "should have one custom table pane");
    assert_eq!(tables[0].name, "my-table");
    assert_eq!(tables[0].title, "My Table");
    assert_eq!(tables[0].columns.len(), 2);
}

#[test]
fn test_plugin_lifecycle_hooks_fire() {
    let (ctx, receiver, _rt) = test_context();

    let mut registry = PluginRegistry::new();
    registry.init(ctx);

    // The on_activate hook calls enya.notify, which sends a UICommand
    let source = r#"
        plugin = { name = "lifecycle-test", version = "0.1.0", description = "test" }
        function on_activate()
            enya.notify("info", "activated!")
        end
    "#;

    setup_plugin(source, &mut registry);

    // The activate call should have triggered on_activate → notify
    let cmd = receiver
        .recv_ui()
        .expect("should receive activation notify");
    match cmd {
        UICommand::Notify { level, message } => {
            assert_eq!(level, "info");
            assert_eq!(message, "activated!");
        }
        other => panic!("expected Notify, got {other:?}"),
    }
}
