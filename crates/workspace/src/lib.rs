//! Workspace configuration types for Enya observability editor.
//!
//! This crate provides the serializable workspace format used by Enya,
//! including TOML file parsing, compact binary encoding for URL sharing,
//! and built-in workspace templates.
//!
//! The types are decoupled from the editor UI (no egui dependency),
//! enabling use by CLI tools and other consumers.

mod compact;
pub mod config;
#[cfg(not(target_arch = "wasm32"))]
mod dir;
mod templates;

// Re-export all public types at crate root for convenience
pub use config::{
    ConnectionConfig, GitConfig, LayoutConfig, LayoutContainer, LayoutNode, LayoutType, LogsConfig,
    MetricsConfig, PaneConfig, PluginsConfig, RefreshInterval, SectionConfig, SectionLayout,
    TimeConfig, ViewConfig, WORKSPACE_VERSION, WorkspaceConfig, WorkspaceError, WorkspaceMeta,
};

#[cfg(not(target_arch = "wasm32"))]
pub use dir::{list_workspaces, resolve_workspace_path, workspace_dir};

pub use templates::{
    ATLAS_WORKSPACE_TOML, COMPLEX_VIEWPORT_TOML, DEFAULT_WORKSPACE_TOML, DEMO_WORKSPACE_TOML,
};
