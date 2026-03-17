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
pub use workspace::snapshot::{
    Snapshot, SnapshotBenchmarkData, SnapshotCellKind, SnapshotColumnDiffStatus,
    SnapshotColumnStats, SnapshotConversation, SnapshotDescribeData, SnapshotDiffData,
    SnapshotDiffFile, SnapshotDiffLine, SnapshotDiffLineKind, SnapshotDiffStats, SnapshotDiffType,
    SnapshotInlineChart, SnapshotInlineContent, SnapshotInlineDiff, SnapshotInlineSearchResults,
    SnapshotInlineSource, SnapshotInlineTable, SnapshotMessage, SnapshotMessageRole,
    SnapshotOperatorMetrics, SnapshotPhaseTiming, SnapshotPlanNode, SnapshotQueryCell,
    SnapshotQueryStats, SnapshotSchemaDiff, SnapshotSchemaDiffColumn, SnapshotSearchResultItem,
    SnapshotSqlPane, SnapshotTableColumn,
};
pub use workspace::{
    ConnectionConfig, GOLDEN_SIGNALS_TOML, GitConfig, INFRASTRUCTURE_TOML, LayoutConfig,
    LayoutContainer, LayoutNode, LayoutType, LogsConfig, MULTI_SERVICE_TOML, MetricsConfig,
    PaneConfig, PluginsConfig, RefreshInterval, SNAPSHOT_MAX_LOG_ENTRIES, SnapshotLogEntry,
    SnapshotMeta, SnapshotPaneData, SnapshotSeries, SnapshotSpan, SnapshotSpanLog, TimeConfig,
    TracingConfig, ViewConfig, WORKSPACE_VERSION, WorkspaceConfig, WorkspaceError, WorkspaceMeta,
};

pub use daemon::{Config, Datasource, Datasources, Server};

#[cfg(not(target_arch = "wasm32"))]
pub use dir::{
    config_path, create_project_dir, delete_project_dir, enya_dir, index_dir,
    list_project_workspaces, list_projects, plugins_dir, project_conversations_dir,
    project_workspace_dir, projects_dir, resolve_project_workspace_path,
};
