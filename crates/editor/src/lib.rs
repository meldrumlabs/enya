//! Enya Editor - A vim-inspired metrics visualization editor.
//!
//! This crate provides the core editor UI built on [egui](https://github.com/emilk/egui),
//! featuring a tile-based workspace layout, PromQL query editing with autocompletion,
//! and multiple visualization types for time-series data.
//!
//! # Architecture
//!
//! The editor is organized around these key concepts:
//!
//! - **Workspace**: A tile-based layout of query panes, similar to vim splits
//! - **Components**: Reusable UI elements (panes, overlays, widgets, utilities)
//! - **Commands**: Vim-style command palette with `:command` syntax

#![warn(clippy::all, rust_2018_idioms)]

/// Async runtime abstraction for cross-platform async execution.
mod async_runtime;
pub use async_runtime::AsyncRuntime;

/// Codebase integration: git repo management and metrics-rs instrumentation discovery.
/// Requires the "codebase" feature (enabled by default on native builds).
#[cfg(not(target_arch = "wasm32"))]
pub mod codebase;

/// Main application entry point and event loop.
mod app;

/// Command system for vim-style `:command` interactions.
pub mod command;

/// UI components organized by category: panes, overlays, widgets, and utilities.
pub mod components;

/// Backend connection management for Prometheus and demo data sources.
pub mod connection;

/// UI primitives: colors, typography, icons, design tokens, and theme definitions.
pub mod ui;

/// Update checker for new version notifications (native only).
#[cfg(not(target_arch = "wasm32"))]
pub mod update_checker;

/// General utilities including WASM-compatible time handling.
pub mod util;

/// Workspace runtime (pane layout) and configuration (serialization).
pub mod workspace;

/// GitHub authentication via the Authorization Code flow.
pub mod github_auth;

/// Plugin system for extending editor functionality.
pub mod plugin;

/// Platform-specific integration (URL schemes, native APIs, etc.).
#[cfg(not(target_arch = "wasm32"))]
pub mod platform;

pub use plugin::{
    Plugin, PluginCapabilities, PluginContext, PluginError, PluginId, PluginInfo, PluginRegistry,
    PluginResult, PluginState,
};

pub use app::{AppState, EnyaApp};

/// Launch the native GUI editor.
///
/// Handles the complete native app lifecycle: logging, async runtime,
/// TLS setup, profiling, and eframe window creation.
///
/// If `startup_workspace` is `Some`, that workspace will be loaded on the first frame.
/// If `startup_snapshot` is `Some`, that snapshot ID will be fetched from the blob server
/// and loaded on the first frame (used by `enya://snapshot/<id>` deep links).
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native_app(
    startup_workspace: Option<String>,
    startup_snapshot: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging. Use RUST_LOG env var to control log levels.
    // Default: enya_editor=info, everything else=warn (to suppress wgpu noise)
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .env()
        .with_module_level("enya_editor", log::LevelFilter::Info)
        .with_module_level("enya_ai", log::LevelFilter::Info)
        .init()
        .unwrap();

    // Create tokio runtime for async operations (AI agent, background tasks)
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");
    let async_runtime = AsyncRuntime::new(tokio_runtime.handle().clone());

    // Initialize puffin profiler server when puffin feature is enabled
    #[cfg(feature = "puffin")]
    let _puffin_server = {
        let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
        puffin::set_scopes_on(true);
        log::info!("Puffin profiler server listening on {server_addr}");
        puffin_http::Server::new(&server_addr).ok()
    };

    // Setup a CryptoProvider to be able to use wss connections
    match rustls::crypto::ring::default_provider().install_default() {
        Ok(()) => {}
        Err(_) => panic!("failed to install CryptoProvider"),
    }

    // Register macOS URL scheme handler before starting the event loop.
    // This catches enya:// URLs opened by the OS (both cold and warm launch).
    #[cfg(target_os = "macos")]
    platform::init_url_handler();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_icon(util::png_to_icon_data(
                &include_bytes!("../assets/logo.png")[..],
            ))
            .with_titlebar_shown(false)
            .with_titlebar_buttons_shown(false)
            .with_fullsize_content_view(true)
            .with_app_id("Enya"),
        ..Default::default()
    };

    let result = eframe::run_native(
        "",
        native_options,
        Box::new(move |cc| {
            let mut app = EnyaApp::new(cc, async_runtime);
            if let Some(ws) = startup_workspace {
                app.set_startup_workspace(ws);
            }
            if let Some(snapshot_id) = startup_snapshot {
                app.set_startup_snapshot(snapshot_id);
            }
            Ok(Box::new(app))
        }),
    );

    drop(tokio_runtime);

    result.map_err(|e| e.into())
}
