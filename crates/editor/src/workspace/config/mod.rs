//! Workspace serialization and deserialization
//!
//! Workspaces capture the state of an Enya dashboard:
//! - Panes with their queries and settings
//! - View preferences (theme, panel visibility)
//! - Time range settings
//! - API connection settings
//!
//! # File Format
//!
//! Workspaces are stored as TOML files, designed to be human-readable and
//! git-friendly. Example:
//!
//! ```toml
//! [workspace]
//! name = "prod-api"
//! description = "Production API monitoring"
//!
//! [connection]
//! endpoint = "https://metrics.example.com"
//! # api_key can be set but is often omitted for security
//! # api_key = "sk-..."
//!
//! [codebase]
//! url = "https://github.com/org/repo.git"
//! # branch = "main"  # optional, defaults to repo's default branch
//!
//! [view]
//! theme = "dark"
//!
//! [time]
//! preset = "1h"
//!
//! [[panes]]
//! query = "env:prod AND service:api"
//! name = "API Requests"
//! tag = "Critical"
//! aggregation = "avg"
//! granularity = "5m"
//! ```
//!
//! # Web Loading
//!
//! For web users, workspaces can be loaded via:
//! - URL parameter: `?workspace=<encoded>`
//! - Drag and drop `.toml` file onto the UI
//!
//! # URL Sharing Format
//!
//! Workspaces can be shared via URL using a compact binary encoding optimized
//! for minimal URL length. The encoding pipeline is:
//!
//! ```text
//! Workspace -> CompactWorkspace -> postcard binary -> LZ4 -> base64
//! ```
//!
//! ## Crates Used
//!
//! | Crate     | Purpose                                         |
//! |-----------|-------------------------------------------------|
//! | postcard  | Compact binary serialization (serde-compatible) |
//! | lz4_flex  | LZ4 compression (pure Rust, WASM compatible)    |
//! | base64    | URL-safe base64 encoding                        |
//!
//! ## Format Prefixes (backwards compatible)
//!
//! - `p` - LZ4-compressed postcard workspace (multi-pane)
//! - `q` - LZ4-compressed postcard single pane (most compact for sharing one query)
//! - No prefix - Raw TOML (legacy)
//!
//! ## Optimizations
//!
//! The `CompactWorkspace` struct uses several techniques to minimize size:
//!
//! 1. **Bit-packed header** - Theme (1 bit) and time preset index (3 bits)
//!    are packed into a single `u8` byte
//!
//! 2. **Bit-packed pane flags** - Aggregation mode (3 bits) and granularity
//!    (3 bits) are packed into a single `u8` per pane
//!
//! 3. **Optional strings** - Empty name/tag fields use `Option<String>` with
//!    `None` for empty values (postcard encodes None as a single tag byte)
//!
//! 4. **Enum indices** - String enums like "p95", "1h" are converted to
//!    numeric indices (0-7) before encoding
//!
//! ## Typical URL Lengths
//!
//! - Empty workspace: ~20 chars
//! - 2-pane workspace: ~121 chars
//! - 3-pane workspace: ~185 chars
//!
//! The bulk of the encoded size comes from query strings and pane names.
//! Further reduction would require server-side URL shortening or query aliasing.

mod compact;
mod templates;

use serde::{Deserialize, Serialize};

use crate::components::{Granularity, QueryState, TimeRangePreset, VisualizationType};
use crate::ui::theme::AppTheme;

// Re-export templates
pub use templates::{
    ATLAS_WORKSPACE_TOML, COMPLEX_VIEWPORT_TOML, DEFAULT_WORKSPACE_TOML, DEMO_WORKSPACE_TOML,
};

/// Current workspace format version
pub const WORKSPACE_VERSION: u32 = 1;

// =============================================================================
// Core Configuration Types
// =============================================================================

/// A complete workspace definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Workspace metadata
    pub workspace: WorkspaceMeta,

    /// API connection settings
    #[serde(default, skip_serializing_if = "ConnectionConfig::is_empty")]
    pub connection: ConnectionConfig,

    /// Codebase integration settings (git repo for source code awareness)
    #[serde(default, skip_serializing_if = "CodebaseConfig::is_empty")]
    pub codebase: CodebaseConfig,

    /// View/UI preferences
    #[serde(default, skip_serializing_if = "ViewConfig::is_default")]
    pub view: ViewConfig,

    /// Time range configuration
    #[serde(default, skip_serializing_if = "TimeConfig::is_default")]
    pub time: TimeConfig,

    /// Pane definitions (queries and their settings)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panes: Vec<PaneConfig>,

    /// Layout configuration (optional - defaults to tabs if omitted)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutConfig>,
}

/// Workspace metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    /// Human-readable name
    pub name: String,

    /// Optional description
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,

    /// Format version (for future migrations)
    #[serde(
        default = "default_version",
        skip_serializing_if = "is_default_version"
    )]
    pub version: u32,
}

fn is_default_version(v: &u32) -> bool {
    *v == WORKSPACE_VERSION
}

fn default_version() -> u32 {
    WORKSPACE_VERSION
}

/// API connection configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectionConfig {
    /// API endpoint URL (e.g., "https://metrics.example.com")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint: String,

    /// API key (optional - often omitted for security, loaded from env instead)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
}

impl ConnectionConfig {
    /// Check if this config has any connection settings
    pub fn is_empty(&self) -> bool {
        self.endpoint.is_empty() && self.api_key.is_empty()
    }

    /// Create a new connection config with an endpoint
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: String::new(),
        }
    }
}

/// Codebase integration configuration
///
/// Allows the editor to connect to a git repository for source code awareness,
/// enabling features like metrics-to-code mapping.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodebaseConfig {
    /// Git repository URL (e.g., "https://github.com/org/repo.git")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,

    /// Branch to track (defaults to the repo's default branch if not specified)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,

    /// Primary language for metric scanning (e.g., "rust", "go", "python", "typescript")
    /// If not specified, all supported languages are scanned.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
}

impl CodebaseConfig {
    /// Check if this config has any codebase settings
    pub fn is_empty(&self) -> bool {
        self.url.is_empty()
    }

    /// Create a new codebase config with a URL
    pub fn with_url(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            branch: String::new(),
            language: String::new(),
        }
    }

    /// Create a new codebase config with a URL and branch
    pub fn with_url_and_branch(url: impl Into<String>, branch: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            branch: branch.into(),
            language: String::new(),
        }
    }

    /// Set the language for this codebase config
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }
}

/// View/UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    /// Theme: "dark" or "light"
    #[serde(default = "default_theme", skip_serializing_if = "is_default_theme")]
    pub theme: String,

    /// Zen mode (hide all panels)
    #[serde(default, skip_serializing_if = "is_false")]
    pub zen_mode: bool,
}

fn is_default_theme(s: &String) -> bool {
    s == "dark"
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            zen_mode: false,
        }
    }
}

impl ViewConfig {
    /// Convert theme string to AppTheme
    pub fn app_theme(&self) -> AppTheme {
        match self.theme.to_lowercase().as_str() {
            "light" => AppTheme::Light,
            _ => AppTheme::Dark,
        }
    }

    /// Check if all values are defaults
    pub fn is_default(&self) -> bool {
        self.theme == "dark" && !self.zen_mode
    }
}

/// Time range configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeConfig {
    /// Preset like "5m", "15m", "1h", "6h", "24h", "7d"
    #[serde(
        default = "default_time_preset",
        skip_serializing_if = "is_default_time_preset"
    )]
    pub preset: String,
}

fn is_default_time_preset(s: &String) -> bool {
    s == "15m"
}

fn default_time_preset() -> String {
    "15m".to_string()
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            preset: default_time_preset(),
        }
    }
}

impl TimeConfig {
    /// Convert preset string to TimeRangePreset
    pub fn to_preset(&self) -> TimeRangePreset {
        match self.preset.to_lowercase().as_str() {
            "5m" => TimeRangePreset::Last5Minutes,
            "15m" => TimeRangePreset::Last15Minutes,
            "30m" => TimeRangePreset::Last30Minutes,
            "1h" => TimeRangePreset::Last1Hour,
            "6h" => TimeRangePreset::Last6Hours,
            "24h" => TimeRangePreset::Last24Hours,
            "7d" => TimeRangePreset::Last7Days,
            _ => TimeRangePreset::Last15Minutes,
        }
    }

    /// Create from TimeRangePreset
    pub fn from_preset(preset: TimeRangePreset) -> Self {
        Self {
            preset: preset.label().to_string(),
        }
    }

    /// Check if all values are defaults
    pub fn is_default(&self) -> bool {
        self.preset == "15m"
    }
}

// =============================================================================
// Pane Configuration
// =============================================================================

/// A single pane (query + display settings)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneConfig {
    /// The query expression (e.g., "sum(*) by (host)" or "env:prod AND service:api")
    pub query: String,

    /// Display name (optional)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,

    /// User-defined tag for organizing panes (e.g., "Critical", "Warning", "Info")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tag: String,

    /// Unit suffix for values (e.g., "ms", "req/s", "%", "MB/s")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,

    /// Granularity: "1m", "5m", "15m", "1h", "6h", "1d"
    #[serde(
        default = "default_granularity",
        skip_serializing_if = "is_default_granularity"
    )]
    pub granularity: String,

    /// Visualization type: "time_series", "stat", etc.
    #[serde(
        default = "default_visualization",
        skip_serializing_if = "is_default_visualization"
    )]
    pub visualization: String,
}

fn default_granularity() -> String {
    "5m".to_string()
}

fn is_default_granularity(s: &String) -> bool {
    s == "5m"
}

fn default_visualization() -> String {
    "time_series".to_string()
}

fn is_default_visualization(s: &String) -> bool {
    s == "time_series"
}

impl PaneConfig {
    /// Create a new pane config
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            name: String::new(),
            tag: String::new(),
            unit: String::new(),
            granularity: default_granularity(),
            visualization: default_visualization(),
        }
    }

    /// Set the name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the tag (e.g., "Critical", "Warning")
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    /// Set granularity
    pub fn with_granularity(mut self, gran: Granularity) -> Self {
        self.granularity = gran.label().to_string();
        self
    }

    /// Set visualization type
    pub fn with_visualization(mut self, viz: VisualizationType) -> Self {
        self.visualization = viz.as_str().to_string();
        self
    }

    /// Convert granularity string to Granularity
    pub fn granularity_value(&self) -> Granularity {
        match self.granularity.to_lowercase().as_str() {
            "1m" => Granularity::OneMinute,
            "5m" => Granularity::FiveMinutes,
            "15m" => Granularity::FifteenMinutes,
            "1h" => Granularity::OneHour,
            "6h" => Granularity::SixHours,
            "1d" => Granularity::OneDay,
            _ => Granularity::FiveMinutes,
        }
    }

    /// Convert visualization string to VisualizationType
    pub fn visualization_type(&self) -> VisualizationType {
        VisualizationType::parse(&self.visualization)
    }

    /// Convert to QueryState
    pub fn to_query_state(&self, time_preset: &str) -> QueryState {
        QueryState {
            granularity: self.granularity_value(),
            time_range_label: time_preset.to_string(),
        }
    }

    /// Create from query and QueryState
    pub fn from_query_state(query: &str, name: &str, tag: &str, state: &QueryState) -> Self {
        Self {
            query: query.to_string(),
            name: name.to_string(),
            tag: tag.to_string(),
            unit: String::new(),
            granularity: state.granularity.label().to_string(),
            visualization: default_visualization(),
        }
    }

    /// Create from query, QueryState, and visualization type
    pub fn from_query_state_with_viz(
        query: &str,
        name: &str,
        tag: &str,
        state: &QueryState,
        viz_type: VisualizationType,
    ) -> Self {
        Self {
            query: query.to_string(),
            name: name.to_string(),
            tag: tag.to_string(),
            unit: String::new(),
            granularity: state.granularity.label().to_string(),
            visualization: viz_type.as_str().to_string(),
        }
    }
}

// =============================================================================
// Layout Configuration
// =============================================================================

/// Layout container types (i3-style)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LayoutType {
    /// Horizontal split (children side by side)
    Horizontal,
    /// Vertical split (children stacked)
    Vertical,
    /// Tabbed container
    Tabs,
}

/// A node in the layout tree - either a pane reference or a nested container
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LayoutNode {
    /// Reference to a pane by index in the [[panes]] array
    Pane(usize),
    /// Nested container with children
    Container(LayoutContainer),
}

/// A container node in the layout tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutContainer {
    /// Container type (horizontal, vertical, or tabs)
    #[serde(rename = "type")]
    pub layout_type: LayoutType,

    /// Children (pane indices or nested containers)
    pub children: Vec<LayoutNode>,

    /// Optional shares for linear containers (horizontal/vertical)
    /// If omitted, children are sized equally (1.0 share each)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shares: Vec<f32>,
}

impl LayoutContainer {
    /// Get share for child at index (defaults to 1.0 if not specified)
    pub fn share_for(&self, index: usize) -> f32 {
        self.shares.get(index).copied().unwrap_or(1.0)
    }
}

/// Root layout configuration for the workspace
/// This appears in the [layout] section of the TOML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Container type for root
    #[serde(rename = "type")]
    pub layout_type: LayoutType,

    /// Children at root level (pane indices or nested containers)
    pub children: Vec<LayoutNode>,

    /// Shares for root children (optional)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shares: Vec<f32>,
}

impl LayoutConfig {
    /// Create a tabs layout containing all panes by index
    pub fn default_tabs(pane_count: usize) -> Self {
        Self {
            layout_type: LayoutType::Tabs,
            children: (0..pane_count).map(LayoutNode::Pane).collect(),
            shares: Vec::new(),
        }
    }

    /// Get share for child at index (defaults to 1.0 if not specified)
    pub fn share_for(&self, index: usize) -> f32 {
        self.shares.get(index).copied().unwrap_or(1.0)
    }

    /// Validate that all pane references are within bounds
    pub fn validate(&self, pane_count: usize) -> Result<(), String> {
        validate_layout_nodes(&self.children, pane_count)
    }
}

/// Recursively validate that all pane references in layout nodes are within bounds
fn validate_layout_nodes(nodes: &[LayoutNode], pane_count: usize) -> Result<(), String> {
    for node in nodes {
        match node {
            LayoutNode::Pane(index) => {
                if *index >= pane_count {
                    return Err(format!(
                        "layout references pane index {index} but only {pane_count} panes exist"
                    ));
                }
            }
            LayoutNode::Container(container) => {
                validate_layout_nodes(&container.children, pane_count)?;
            }
        }
    }
    Ok(())
}

// =============================================================================
// WorkspaceConfig Implementation
// =============================================================================

impl WorkspaceConfig {
    /// Create a new empty workspace
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            workspace: WorkspaceMeta {
                name: name.into(),
                description: String::new(),
                version: WORKSPACE_VERSION,
            },
            connection: ConnectionConfig::default(),
            codebase: CodebaseConfig::default(),
            view: ViewConfig::default(),
            time: TimeConfig::default(),
            panes: Vec::new(),
            layout: None,
        }
    }

    /// Create a workspace with an API endpoint
    pub fn with_endpoint(name: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let mut ws = Self::new(name);
        ws.connection = ConnectionConfig::with_endpoint(endpoint);
        ws
    }

    /// Add a pane to the workspace
    pub fn add_pane(&mut self, pane: PaneConfig) {
        self.panes.push(pane);
    }

    /// Parse a workspace from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, WorkspaceError> {
        toml::from_str(toml_str).map_err(WorkspaceError::Parse)
    }

    /// Get the default example workspace
    pub fn default_example() -> Self {
        Self::from_toml(DEFAULT_WORKSPACE_TOML).expect("DEFAULT_WORKSPACE_TOML should be valid")
    }

    /// Get the demo workspace (uses synthetic data, no backend required)
    pub fn default_demo() -> Self {
        Self::from_toml(DEMO_WORKSPACE_TOML).expect("DEMO_WORKSPACE_TOML should be valid")
    }

    /// Serialize workspace to TOML string
    pub fn to_toml(&self) -> Result<String, WorkspaceError> {
        toml::to_string_pretty(self).map_err(WorkspaceError::Serialize)
    }

    /// Load workspace from a file path
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: &std::path::Path) -> Result<Self, WorkspaceError> {
        let content = std::fs::read_to_string(path).map_err(WorkspaceError::Io)?;
        Self::from_toml(&content)
    }

    /// Save workspace to a file path
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self, path: &std::path::Path) -> Result<(), WorkspaceError> {
        let content = self.to_toml()?;
        std::fs::write(path, content).map_err(WorkspaceError::Io)
    }

    /// Decode workspace from base64-encoded data (for URL parameters)
    pub fn from_base64(encoded: &str) -> Result<Self, WorkspaceError> {
        compact::decode_workspace(encoded)
    }

    /// Encode workspace to base64 (for URL sharing)
    pub fn to_base64(&self) -> Result<String, WorkspaceError> {
        compact::encode_workspace(self)
    }

    /// Encode a single pane to base64 (for sharing individual queries)
    pub fn pane_to_base64(&self, pane_index: usize) -> Result<String, WorkspaceError> {
        compact::encode_pane(self, pane_index)
    }

    /// Validate workspace structure
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.workspace.version > WORKSPACE_VERSION {
            return Err(WorkspaceError::UnsupportedVersion(self.workspace.version));
        }
        Ok(())
    }
}

// =============================================================================
// Error Type
// =============================================================================

/// Errors that can occur when loading/saving workspaces
#[derive(Debug)]
pub enum WorkspaceError {
    /// TOML parsing error
    Parse(toml::de::Error),
    /// TOML serialization error
    Serialize(toml::ser::Error),
    /// IO error (file read/write)
    #[cfg(not(target_arch = "wasm32"))]
    Io(std::io::Error),
    /// Base64 decoding error
    Decode(String),
    /// Encoding error (compression)
    Encode(String),
    /// Unsupported workspace version
    UnsupportedVersion(u32),
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "failed to parse workspace: {e}"),
            Self::Serialize(e) => write!(f, "failed to serialize workspace: {e}"),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Decode(e) => write!(f, "decode error: {e}"),
            Self::Encode(e) => write!(f, "encode error: {e}"),
            Self::UnsupportedVersion(v) => {
                write!(
                    f,
                    "unsupported workspace version: {v} (max: {WORKSPACE_VERSION})"
                )
            }
        }
    }
}

impl std::error::Error for WorkspaceError {}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_workspace() {
        let toml = r#"
[workspace]
name = "test"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.workspace.name, "test");
        assert_eq!(ws.workspace.version, WORKSPACE_VERSION);
        assert_eq!(ws.view.theme, "dark");
    }

    #[test]
    fn test_parse_full_workspace() {
        let toml = r#"
[workspace]
name = "prod-dashboard"
description = "Production monitoring"
version = 1

[view]
theme = "light"
zen_mode = false

[time]
preset = "1h"

[[panes]]
query = "avg(env:prod AND service:api) by (service)"
name = "API Requests"
granularity = "1m"

[[panes]]
query = "sum(env:prod AND name:error_rate) by (name)"
granularity = "5m"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.workspace.name, "prod-dashboard");
        assert_eq!(ws.view.theme, "light");
        assert_eq!(ws.view.app_theme(), AppTheme::Light);
        assert_eq!(ws.time.preset, "1h");
        assert_eq!(ws.panes.len(), 2);
        assert_eq!(
            ws.panes[0].query,
            "avg(env:prod AND service:api) by (service)"
        );
        assert_eq!(ws.panes[0].name, "API Requests");
        assert_eq!(ws.panes[0].granularity, "1m");
        assert_eq!(ws.panes[1].granularity, "5m");
    }

    #[test]
    fn test_roundtrip() {
        let mut ws = WorkspaceConfig::new("test");
        ws.workspace.description = "Test workspace".to_string();
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("avg(env:prod) by (service)")
                .with_name("Production")
                .with_granularity(Granularity::OneMinute),
        );

        let toml = ws.to_toml().unwrap();
        let parsed = WorkspaceConfig::from_toml(&toml).unwrap();

        assert_eq!(parsed.workspace.name, "test");
        assert_eq!(parsed.workspace.description, "Test workspace");
        assert_eq!(parsed.view.theme, "light");
        assert_eq!(parsed.time.preset, "1h");
        assert_eq!(parsed.panes.len(), 1);
        assert_eq!(parsed.panes[0].name, "Production");
    }

    #[test]
    fn test_base64_encoding() {
        let ws = WorkspaceConfig::new("shared");
        let encoded = ws.to_base64().unwrap();
        let decoded = WorkspaceConfig::from_base64(&encoded).unwrap();
        assert_eq!(decoded.workspace.name, "shared");
    }

    #[test]
    fn test_base64_encoding_with_panes() {
        let mut ws = WorkspaceConfig::new("dashboard");
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("sum(env:prod AND service:api) by (service)")
                .with_name("API Latency")
                .with_tag("Critical")
                .with_granularity(Granularity::OneMinute),
        );
        ws.add_pane(PaneConfig::new("env:prod AND name:error_rate"));

        let encoded = ws.to_base64().unwrap();
        // Postcard format should be much shorter than TOML
        assert!(encoded.starts_with('p'));

        let decoded = WorkspaceConfig::from_base64(&encoded).unwrap();
        assert_eq!(decoded.workspace.name, "dashboard");
        assert_eq!(decoded.view.theme, "light");
        assert_eq!(decoded.time.preset, "1h");
        assert_eq!(decoded.panes.len(), 2);
        assert_eq!(
            decoded.panes[0].query,
            "sum(env:prod AND service:api) by (service)"
        );
        assert_eq!(decoded.panes[0].name, "API Latency");
        assert_eq!(decoded.panes[0].tag, "Critical");
        assert_eq!(decoded.panes[0].granularity, "1m");
        assert_eq!(decoded.panes[1].query, "env:prod AND name:error_rate");
    }

    #[test]
    fn test_single_pane_encoding() {
        let mut ws = WorkspaceConfig::new("dashboard");
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("sum(env:prod AND service:api) by (service)")
                .with_name("API Latency")
                .with_granularity(Granularity::OneMinute),
        );

        // Single pane encoding should be more compact
        let pane_encoded = ws.pane_to_base64(0).unwrap();
        let ws_encoded = ws.to_base64().unwrap();

        assert!(pane_encoded.starts_with('q'));
        assert!(
            pane_encoded.len() < ws_encoded.len(),
            "single pane ({}) should be shorter than workspace ({})",
            pane_encoded.len(),
            ws_encoded.len()
        );

        // Decode and verify
        let decoded = WorkspaceConfig::from_base64(&pane_encoded).unwrap();
        assert_eq!(decoded.workspace.name, "shared"); // default name for single pane
        assert_eq!(decoded.view.theme, "light");
        assert_eq!(decoded.time.preset, "1h");
        assert_eq!(decoded.panes.len(), 1);
        assert_eq!(
            decoded.panes[0].query,
            "sum(env:prod AND service:api) by (service)"
        );
        assert_eq!(decoded.panes[0].name, "API Latency");
        assert_eq!(decoded.panes[0].granularity, "1m");
    }

    #[test]
    fn test_pane_config_conversions() {
        let pane = PaneConfig {
            query: "sum(*) by (service)".to_string(),
            name: "Test".to_string(),
            tag: "Critical".to_string(),
            granularity: "15m".to_string(),
            visualization: "time_series".to_string(),
            unit: String::new(),
        };

        assert_eq!(pane.granularity_value(), Granularity::FifteenMinutes);
        assert_eq!(pane.tag, "Critical");

        let state = pane.to_query_state("1h");
        assert_eq!(state.granularity, Granularity::FifteenMinutes);
        assert_eq!(state.time_range_label, "1h");
    }

    #[test]
    fn test_pane_with_tag() {
        let toml = r#"
[workspace]
name = "tagged"

[[panes]]
query = "avg(env:prod) by (service)"
name = "Production"
tag = "Critical"
granularity = "1m"

[[panes]]
query = "env:staging"
name = "Staging"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.panes[0].tag, "Critical");
        assert!(ws.panes[1].tag.is_empty()); // Tag is optional

        // Test serialization - empty tags should be omitted
        let serialized = ws.to_toml().unwrap();
        assert!(serialized.contains(r#"tag = "Critical""#));
        // Empty tag should not appear in output
        assert!(!serialized.contains("tag = \"\""));
    }

    #[test]
    fn test_time_config_presets() {
        let cases = [
            ("5m", TimeRangePreset::Last5Minutes),
            ("15m", TimeRangePreset::Last15Minutes),
            ("1h", TimeRangePreset::Last1Hour),
            ("24h", TimeRangePreset::Last24Hours),
            ("7d", TimeRangePreset::Last7Days),
        ];

        for (input, expected) in cases {
            let config = TimeConfig {
                preset: input.to_string(),
            };
            assert_eq!(config.to_preset(), expected);
        }
    }

    #[test]
    fn test_defaults() {
        let toml = r#"
[workspace]
name = "minimal"

[[panes]]
query = "test"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();

        // View defaults
        assert_eq!(ws.view.theme, "dark");
        assert!(!ws.view.zen_mode);

        // Time defaults
        assert_eq!(ws.time.preset, "15m");

        // Pane defaults
        assert_eq!(ws.panes[0].granularity, "5m");
        assert!(ws.panes[0].name.is_empty());

        // Connection defaults (empty)
        assert!(ws.connection.is_empty());
    }

    #[test]
    fn test_connection_config() {
        let toml = r#"
[workspace]
name = "with-endpoint"

[connection]
endpoint = "https://metrics.example.com"

[[panes]]
query = "env:prod"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.connection.endpoint, "https://metrics.example.com");
        assert!(ws.connection.api_key.is_empty());
        assert!(!ws.connection.is_empty());

        // Test serialization - empty api_key should be omitted
        let serialized = ws.to_toml().unwrap();
        assert!(serialized.contains("endpoint = \"https://metrics.example.com\""));
        assert!(!serialized.contains("api_key"));
    }

    #[test]
    fn test_connection_with_api_key() {
        let toml = r#"
[workspace]
name = "with-key"

[connection]
endpoint = "https://metrics.example.com"
api_key = "sk-test-123"

[[panes]]
query = "env:prod"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.connection.endpoint, "https://metrics.example.com");
        assert_eq!(ws.connection.api_key, "sk-test-123");
    }

    // ==================== LayoutConfig Tests ====================

    #[test]
    fn test_layout_config_default_tabs() {
        let layout = LayoutConfig::default_tabs(3);
        assert_eq!(layout.layout_type, LayoutType::Tabs);
        assert_eq!(layout.children.len(), 3);
        assert!(layout.shares.is_empty());

        // Children should be pane references 0, 1, 2
        match &layout.children[0] {
            LayoutNode::Pane(i) => assert_eq!(*i, 0),
            _ => panic!("expected Pane"),
        }
        match &layout.children[1] {
            LayoutNode::Pane(i) => assert_eq!(*i, 1),
            _ => panic!("expected Pane"),
        }
        match &layout.children[2] {
            LayoutNode::Pane(i) => assert_eq!(*i, 2),
            _ => panic!("expected Pane"),
        }
    }

    #[test]
    fn test_layout_config_share_for() {
        let layout = LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
            shares: vec![0.3, 0.7],
        };
        assert!((layout.share_for(0) - 0.3).abs() < 0.001);
        assert!((layout.share_for(1) - 0.7).abs() < 0.001);
        // Out of bounds defaults to 1.0
        assert!((layout.share_for(2) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_layout_config_share_for_empty() {
        let layout = LayoutConfig::default_tabs(2);
        // Empty shares defaults to 1.0
        assert!((layout.share_for(0) - 1.0).abs() < 0.001);
        assert!((layout.share_for(1) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_layout_config_validate_valid() {
        let layout = LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
            shares: Vec::new(),
        };
        assert!(layout.validate(2).is_ok());
        assert!(layout.validate(3).is_ok()); // More panes than referenced is ok
    }

    #[test]
    fn test_layout_config_validate_invalid_index() {
        let layout = LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![LayoutNode::Pane(0), LayoutNode::Pane(5)],
            shares: Vec::new(),
        };
        let result = layout.validate(3);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("pane index 5"));
    }

    #[test]
    fn test_layout_config_validate_nested_container() {
        let layout = LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![
                LayoutNode::Pane(0),
                LayoutNode::Container(LayoutContainer {
                    layout_type: LayoutType::Vertical,
                    children: vec![LayoutNode::Pane(1), LayoutNode::Pane(10)], // 10 is invalid
                    shares: Vec::new(),
                }),
            ],
            shares: Vec::new(),
        };
        let result = layout.validate(3);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("pane index 10"));
    }

    // ==================== LayoutContainer Tests ====================

    #[test]
    fn test_layout_container_share_for() {
        let container = LayoutContainer {
            layout_type: LayoutType::Vertical,
            children: vec![
                LayoutNode::Pane(0),
                LayoutNode::Pane(1),
                LayoutNode::Pane(2),
            ],
            shares: vec![1.0, 2.0, 1.0],
        };
        assert!((container.share_for(0) - 1.0).abs() < 0.001);
        assert!((container.share_for(1) - 2.0).abs() < 0.001);
        assert!((container.share_for(2) - 1.0).abs() < 0.001);
        // Out of bounds defaults to 1.0
        assert!((container.share_for(3) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_layout_container_share_for_empty() {
        let container = LayoutContainer {
            layout_type: LayoutType::Tabs,
            children: vec![LayoutNode::Pane(0)],
            shares: Vec::new(),
        };
        assert!((container.share_for(0) - 1.0).abs() < 0.001);
    }

    // ==================== LayoutType Tests ====================

    #[test]
    fn test_layout_type_equality() {
        assert_eq!(LayoutType::Horizontal, LayoutType::Horizontal);
        assert_eq!(LayoutType::Vertical, LayoutType::Vertical);
        assert_eq!(LayoutType::Tabs, LayoutType::Tabs);
        assert_ne!(LayoutType::Horizontal, LayoutType::Vertical);
        assert_ne!(LayoutType::Tabs, LayoutType::Horizontal);
    }

    #[test]
    fn test_layout_type_serde() {
        // Test serialization
        let toml = r#"
[workspace]
name = "layout-test"

[[panes]]
query = "a"

[[panes]]
query = "b"

[layout]
type = "horizontal"
children = [0, 1]
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        let layout = ws.layout.unwrap();
        assert_eq!(layout.layout_type, LayoutType::Horizontal);
    }

    #[test]
    fn test_layout_type_vertical_serde() {
        let toml = r#"
[workspace]
name = "layout-test"

[[panes]]
query = "a"

[[panes]]
query = "b"

[layout]
type = "vertical"
children = [0, 1]
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        let layout = ws.layout.unwrap();
        assert_eq!(layout.layout_type, LayoutType::Vertical);
    }

    #[test]
    fn test_layout_type_tabs_serde() {
        let toml = r#"
[workspace]
name = "layout-test"

[[panes]]
query = "a"

[[panes]]
query = "b"

[layout]
type = "tabs"
children = [0, 1]
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        let layout = ws.layout.unwrap();
        assert_eq!(layout.layout_type, LayoutType::Tabs);
    }

    // ==================== Layout with Shares Tests ====================

    #[test]
    fn test_layout_with_shares() {
        let toml = r#"
[workspace]
name = "layout-shares"

[[panes]]
query = "a"

[[panes]]
query = "b"

[layout]
type = "horizontal"
children = [0, 1]
shares = [0.25, 0.75]
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        let layout = ws.layout.unwrap();
        assert_eq!(layout.shares.len(), 2);
        assert!((layout.shares[0] - 0.25).abs() < 0.001);
        assert!((layout.shares[1] - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_nested_layout() {
        let toml = r#"
[workspace]
name = "nested-layout"

[[panes]]
query = "a"

[[panes]]
query = "b"

[[panes]]
query = "c"

[layout]
type = "horizontal"
children = [0, { type = "vertical", children = [1, 2] }]
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        let layout = ws.layout.unwrap();
        assert_eq!(layout.layout_type, LayoutType::Horizontal);
        assert_eq!(layout.children.len(), 2);

        match &layout.children[0] {
            LayoutNode::Pane(i) => assert_eq!(*i, 0),
            _ => panic!("expected Pane"),
        }

        match &layout.children[1] {
            LayoutNode::Container(c) => {
                assert_eq!(c.layout_type, LayoutType::Vertical);
                assert_eq!(c.children.len(), 2);
            }
            _ => panic!("expected Container"),
        }
    }

    // ==================== ViewConfig Tests ====================

    #[test]
    fn test_view_config_default() {
        let config = ViewConfig::default();
        assert_eq!(config.theme, "dark");
        assert!(!config.zen_mode);
        assert!(config.is_default());
    }

    #[test]
    fn test_view_config_is_default() {
        let mut config = ViewConfig::default();
        assert!(config.is_default());

        config.theme = "light".to_string();
        assert!(!config.is_default());

        config.theme = "dark".to_string();
        config.zen_mode = true;
        assert!(!config.is_default());
    }

    #[test]
    fn test_view_config_app_theme() {
        let mut config = ViewConfig::default();
        assert_eq!(config.app_theme(), AppTheme::Dark);

        config.theme = "light".to_string();
        assert_eq!(config.app_theme(), AppTheme::Light);

        config.theme = "LIGHT".to_string();
        assert_eq!(config.app_theme(), AppTheme::Light);

        config.theme = "invalid".to_string();
        assert_eq!(config.app_theme(), AppTheme::Dark); // Defaults to dark
    }

    // ==================== TimeConfig Tests ====================

    #[test]
    fn test_time_config_default() {
        let config = TimeConfig::default();
        assert_eq!(config.preset, "15m");
        assert!(config.is_default());
    }

    #[test]
    fn test_time_config_is_default() {
        let mut config = TimeConfig::default();
        assert!(config.is_default());

        config.preset = "1h".to_string();
        assert!(!config.is_default());
    }

    #[test]
    fn test_time_config_from_preset() {
        let config = TimeConfig::from_preset(TimeRangePreset::Last1Hour);
        assert_eq!(config.preset, "1h");

        let config = TimeConfig::from_preset(TimeRangePreset::Last7Days);
        assert_eq!(config.preset, "7d");
    }

    #[test]
    fn test_time_config_to_preset_all() {
        let cases = [
            ("5m", TimeRangePreset::Last5Minutes),
            ("15m", TimeRangePreset::Last15Minutes),
            ("30m", TimeRangePreset::Last30Minutes),
            ("1h", TimeRangePreset::Last1Hour),
            ("6h", TimeRangePreset::Last6Hours),
            ("24h", TimeRangePreset::Last24Hours),
            ("7d", TimeRangePreset::Last7Days),
            ("invalid", TimeRangePreset::Last15Minutes), // Default fallback
        ];

        for (input, expected) in cases {
            let config = TimeConfig {
                preset: input.to_string(),
            };
            assert_eq!(config.to_preset(), expected, "Failed for input: {input}");
        }
    }

    // ==================== ConnectionConfig Tests ====================

    #[test]
    fn test_connection_config_default() {
        let config = ConnectionConfig::default();
        assert!(config.endpoint.is_empty());
        assert!(config.api_key.is_empty());
        assert!(config.is_empty());
    }

    #[test]
    fn test_connection_config_with_endpoint() {
        let config = ConnectionConfig::with_endpoint("https://api.example.com");
        assert_eq!(config.endpoint, "https://api.example.com");
        assert!(config.api_key.is_empty());
        assert!(!config.is_empty());
    }

    #[test]
    fn test_connection_config_is_empty() {
        let mut config = ConnectionConfig::default();
        assert!(config.is_empty());

        config.endpoint = "http://localhost".to_string();
        assert!(!config.is_empty());

        config.endpoint = String::new();
        config.api_key = "key".to_string();
        assert!(!config.is_empty());
    }

    // ==================== PaneConfig Builder Tests ====================

    #[test]
    fn test_pane_config_builder() {
        let pane = PaneConfig::new("sum(*) by (host)")
            .with_name("Host Metrics")
            .with_tag("Important")
            .with_granularity(Granularity::OneHour)
            .with_visualization(VisualizationType::Stat);

        assert_eq!(pane.query, "sum(*) by (host)");
        assert_eq!(pane.name, "Host Metrics");
        assert_eq!(pane.tag, "Important");
        assert_eq!(pane.granularity, "1h");
        assert_eq!(pane.visualization, "stat");
    }

    #[test]
    fn test_pane_config_new_defaults() {
        let pane = PaneConfig::new("query");
        assert_eq!(pane.query, "query");
        assert!(pane.name.is_empty());
        assert!(pane.tag.is_empty());
        assert_eq!(pane.granularity, "5m");
        assert_eq!(pane.visualization, "time_series");
    }

    #[test]
    fn test_pane_config_granularity_value_all() {
        let cases = [
            ("1m", Granularity::OneMinute),
            ("5m", Granularity::FiveMinutes),
            ("15m", Granularity::FifteenMinutes),
            ("1h", Granularity::OneHour),
            ("6h", Granularity::SixHours),
            ("1d", Granularity::OneDay),
            ("invalid", Granularity::FiveMinutes), // Default fallback
        ];

        for (input, expected) in cases {
            let pane = PaneConfig {
                query: "test".to_string(),
                name: String::new(),
                tag: String::new(),
                granularity: input.to_string(),
                visualization: "time_series".to_string(),
                unit: String::new(),
            };
            assert_eq!(pane.granularity_value(), expected, "Failed for: {input}");
        }
    }

    #[test]
    fn test_pane_config_from_query_state_with_viz() {
        let state = QueryState {
            granularity: Granularity::OneMinute,
            time_range_label: "1h".to_string(),
        };
        let pane = PaneConfig::from_query_state_with_viz(
            "sum(*)",
            "Test",
            "MyTag",
            &state,
            VisualizationType::Stat,
        );

        assert_eq!(pane.query, "sum(*)");
        assert_eq!(pane.name, "Test");
        assert_eq!(pane.tag, "MyTag");
        assert_eq!(pane.granularity, "1m");
        assert_eq!(pane.visualization, "stat");
    }

    // ==================== WorkspaceMeta Tests ====================

    #[test]
    fn test_workspace_meta_version() {
        let toml = r#"
[workspace]
name = "versioned"
version = 1
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.workspace.version, 1);
    }

    #[test]
    fn test_workspace_meta_default_version() {
        let toml = r#"
[workspace]
name = "no-version"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.workspace.version, WORKSPACE_VERSION);
    }

    // ==================== WorkspaceConfig Tests ====================

    #[test]
    fn test_workspace_config_new() {
        let ws = WorkspaceConfig::new("my-workspace");
        assert_eq!(ws.workspace.name, "my-workspace");
        assert!(ws.workspace.description.is_empty());
        assert_eq!(ws.workspace.version, WORKSPACE_VERSION);
        assert!(ws.connection.is_empty());
        assert!(ws.panes.is_empty());
        assert!(ws.layout.is_none());
    }

    #[test]
    fn test_workspace_config_with_endpoint() {
        let ws = WorkspaceConfig::with_endpoint("test", "https://api.example.com");
        assert_eq!(ws.workspace.name, "test");
        assert_eq!(ws.connection.endpoint, "https://api.example.com");
    }

    #[test]
    fn test_workspace_config_add_pane() {
        let mut ws = WorkspaceConfig::new("test");
        assert!(ws.panes.is_empty());

        ws.add_pane(PaneConfig::new("query1"));
        assert_eq!(ws.panes.len(), 1);

        ws.add_pane(PaneConfig::new("query2"));
        assert_eq!(ws.panes.len(), 2);
    }

    #[test]
    fn test_workspace_config_validate_ok() {
        let ws = WorkspaceConfig::new("test");
        assert!(ws.validate().is_ok());
    }

    #[test]
    fn test_workspace_config_validate_unsupported_version() {
        let toml = r#"
[workspace]
name = "future"
version = 999
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        let result = ws.validate();
        assert!(result.is_err());
        match result.unwrap_err() {
            WorkspaceError::UnsupportedVersion(v) => assert_eq!(v, 999),
            _ => panic!("expected UnsupportedVersion"),
        }
    }

    #[test]
    fn test_workspace_config_default_example() {
        let ws = WorkspaceConfig::default_example();
        assert!(!ws.workspace.name.is_empty());
    }

    #[test]
    fn test_workspace_config_default_demo() {
        let ws = WorkspaceConfig::default_demo();
        assert!(!ws.workspace.name.is_empty());
    }

    // ==================== WorkspaceError Tests ====================

    #[test]
    fn test_workspace_error_display_parse() {
        let error =
            WorkspaceError::Parse(toml::from_str::<WorkspaceConfig>("invalid").unwrap_err());
        let msg = format!("{error}");
        assert!(msg.contains("failed to parse workspace"));
    }

    #[test]
    fn test_workspace_error_display_decode() {
        let error = WorkspaceError::Decode("invalid base64".to_string());
        let msg = format!("{error}");
        assert!(msg.contains("decode error"));
        assert!(msg.contains("invalid base64"));
    }

    #[test]
    fn test_workspace_error_display_encode() {
        let error = WorkspaceError::Encode("compression failed".to_string());
        let msg = format!("{error}");
        assert!(msg.contains("encode error"));
        assert!(msg.contains("compression failed"));
    }

    #[test]
    fn test_workspace_error_display_unsupported_version() {
        let error = WorkspaceError::UnsupportedVersion(99);
        let msg = format!("{error}");
        assert!(msg.contains("unsupported workspace version: 99"));
    }

    // ==================== TOML Serialization Skip Tests ====================

    #[test]
    fn test_skip_default_values_in_serialization() {
        let ws = WorkspaceConfig::new("test");
        let toml = ws.to_toml().unwrap();

        // Default values should be skipped
        assert!(!toml.contains("theme")); // "dark" is default
        assert!(!toml.contains("zen_mode"));
        assert!(!toml.contains("preset")); // "15m" is default
        assert!(!toml.contains("[connection]")); // Empty connection
        assert!(!toml.contains("[time]")); // Default time
        assert!(!toml.contains("[[panes]]")); // No panes
    }

    #[test]
    fn test_include_non_default_values_in_serialization() {
        let mut ws = WorkspaceConfig::new("test");
        ws.view.theme = "light".to_string();
        ws.view.zen_mode = true;
        ws.time.preset = "1h".to_string();

        let toml = ws.to_toml().unwrap();

        assert!(toml.contains("theme = \"light\""));
        assert!(toml.contains("zen_mode = true"));
        assert!(toml.contains("preset = \"1h\""));
    }

    // ==================== Insta Snapshot Tests ====================
    //
    // These tests use insta inline snapshots for serialization stability.
    // To update snapshots: cargo insta test --accept

    #[test]
    fn test_snapshot_minimal_workspace_toml() {
        let ws = WorkspaceConfig::new("minimal-test");
        let toml = ws.to_toml().unwrap();
        insta::assert_snapshot!(toml, @r#"
        [workspace]
        name = "minimal-test"
        "#);
    }

    #[test]
    fn test_snapshot_full_workspace_toml() {
        let mut ws = WorkspaceConfig::new("full-dashboard");
        ws.workspace.description = "A comprehensive monitoring dashboard".to_string();
        ws.connection = ConnectionConfig::with_endpoint("https://metrics.example.com");
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("sum(env:prod AND service:api) by (service)")
                .with_name("API Latency")
                .with_tag("Critical")
                .with_granularity(Granularity::OneMinute),
        );
        ws.add_pane(
            PaneConfig::new("avg(env:prod AND name:cpu_usage) by (host)")
                .with_name("CPU Usage")
                .with_granularity(Granularity::FiveMinutes)
                .with_visualization(VisualizationType::Stat),
        );

        let toml = ws.to_toml().unwrap();
        insta::assert_snapshot!(toml, @r#"
        [workspace]
        name = "full-dashboard"
        description = "A comprehensive monitoring dashboard"

        [connection]
        endpoint = "https://metrics.example.com"

        [view]
        theme = "light"

        [time]
        preset = "1h"

        [[panes]]
        query = "sum(env:prod AND service:api) by (service)"
        name = "API Latency"
        tag = "Critical"
        granularity = "1m"

        [[panes]]
        query = "avg(env:prod AND name:cpu_usage) by (host)"
        name = "CPU Usage"
        visualization = "stat"
        "#);
    }

    #[test]
    fn test_snapshot_workspace_with_layout() {
        let mut ws = WorkspaceConfig::new("layout-test");
        ws.add_pane(PaneConfig::new("query1").with_name("Pane 1"));
        ws.add_pane(PaneConfig::new("query2").with_name("Pane 2"));
        ws.add_pane(PaneConfig::new("query3").with_name("Pane 3"));
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![
                LayoutNode::Pane(0),
                LayoutNode::Container(LayoutContainer {
                    layout_type: LayoutType::Vertical,
                    children: vec![LayoutNode::Pane(1), LayoutNode::Pane(2)],
                    shares: vec![1.0, 2.0],
                }),
            ],
            shares: vec![0.4, 0.6],
        });

        let toml = ws.to_toml().unwrap();
        insta::assert_snapshot!(toml, @r#"
        [workspace]
        name = "layout-test"

        [[panes]]
        query = "query1"
        name = "Pane 1"

        [[panes]]
        query = "query2"
        name = "Pane 2"

        [[panes]]
        query = "query3"
        name = "Pane 3"

        [layout]
        type = "horizontal"
        children = [
            0,
            { type = "vertical", children = [
            1,
            2,
        ], shares = [
            1.0,
            2.0,
        ] },
        ]
        shares = [
            0.4000000059604645,
            0.6000000238418579,
        ]
        "#);
    }

    #[test]
    fn test_snapshot_pane_config_toml() {
        let pane = PaneConfig::new("sum(*) by (host)")
            .with_name("Host Metrics")
            .with_tag("Critical")
            .with_granularity(Granularity::OneHour)
            .with_visualization(VisualizationType::Gauge);

        insta::assert_toml_snapshot!(pane, @r#"
        query = 'sum(*) by (host)'
        name = 'Host Metrics'
        tag = 'Critical'
        granularity = '1h'
        visualization = 'gauge'
        "#);
    }

    #[test]
    fn test_snapshot_base64_encoding_stability() {
        // This test ensures our URL encoding format remains stable
        let mut ws = WorkspaceConfig::new("shared");
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("sum(env:prod) by (service)")
                .with_name("Production")
                .with_granularity(Granularity::FiveMinutes),
        );

        let encoded = ws.to_base64().unwrap();
        insta::assert_snapshot!(encoded, @"pMwAAAPAkBnNoYXJlZAsBGnN1bShlbnY6cHJvZCkgYnkgKHNlcnZpY2UpAQpQcm9kdWN0aW9uAAEA");
    }
}
