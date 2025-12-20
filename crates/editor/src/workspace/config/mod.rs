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
use crate::theme::AppTheme;

// Re-export templates
pub use templates::{COMPLEX_VIEWPORT_TOML, DEFAULT_WORKSPACE_TOML, DEMO_WORKSPACE_TOML};

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
}
