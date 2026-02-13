//! Configuration types for the Enya observability platform.
//!
//! This crate provides two kinds of configuration:
//!
//! - **Daemon config** ([`Config`]): Infrastructure settings like datasource
//!   endpoints and server bind address, stored at `~/.enya/config.toml`.
//!
//! - **Workspace config** ([`WorkspaceConfig`]): Dashboard layout, pane queries,
//!   view preferences, and time ranges. Stored as `.toml` files in
//!   `~/.enya/workspaces/`, with compact binary encoding for URL sharing.
//!
//! All types are decoupled from the editor UI (no egui dependency),
//! enabling use by CLI tools and other consumers.

pub mod daemon;
#[cfg(not(target_arch = "wasm32"))]
mod dir;
pub mod workspace;

// Re-export all public types at crate root for convenience
pub use workspace::{
    ATLAS_WORKSPACE_TOML, COMPLEX_VIEWPORT_TOML, ConnectionConfig, DEFAULT_WORKSPACE_TOML,
    DEMO_WORKSPACE_TOML, GitConfig, LayoutConfig, LayoutContainer, LayoutNode, LayoutType,
    LogsConfig, MetricsConfig, PaneConfig, PluginsConfig, RefreshInterval, SectionConfig,
    SectionLayout, SnapshotMeta, SnapshotPaneData, SnapshotSeries, TimeConfig, ViewConfig,
    WORKSPACE_VERSION, WorkspaceConfig, WorkspaceError, WorkspaceMeta,
};

pub use daemon::{Config, Datasource, Datasources, Server};

#[cfg(not(target_arch = "wasm32"))]
pub use dir::{config_path, enya_dir, list_workspaces, resolve_workspace_path, workspace_dir};
