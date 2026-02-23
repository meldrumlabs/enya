//! Workspace serialization and deserialization
//!
//! Workspaces capture the state of an Enya dashboard:
//! - Sections with collapsible headers containing grouped panes
//! - View preferences (theme, panel visibility)
//! - Time range settings
//! - Metrics (Prometheus) and Logs (Loki) connection settings
//!
//! # File Format
//!
//! Workspaces are stored as TOML files, designed to be human-readable and
//! git-friendly. Example with sections:
//!
//! ```toml
//! [workspace]
//! name = "prod-api"
//! description = "Production API monitoring"
//! endpoint = "https://prometheus.example.com"
//!
//! [view]
//! theme = "dark"
//!
//! [time]
//! preset = "1h"
//!
//! [[sections]]
//! name = "API Performance"
//! layout = "horizontal"
//!
//! [[sections.panes]]
//! query = "rate(http_requests_total[5m])"
//! name = "Request Rate"
//!
//! [[sections.panes]]
//! query = "histogram_quantile(0.99, http_request_duration_seconds)"
//! name = "Latency p99"
//!
//! [[sections]]
//! name = "Infrastructure"
//! layout = "grid"
//! columns = 2
//! collapsed = true
//!
//! [[sections.panes]]
//! query = "avg(cpu_usage)"
//! name = "CPU Usage"
//! ```
//!
//! Legacy format (deprecated - use sections instead):
//!
//! ```toml
//! [workspace]
//! name = "prod-api"
//!
//! [[panes]]
//! query = "env:prod AND service:api"
//! name = "API Requests"
//! tag = "Critical"
//! granularity = "5m"
//! ```
//!
//! For metrics connection with API key:
//!
//! ```toml
//! [metrics]
//! endpoint = "https://prometheus.example.com"
//! api_key = "sk-..."  # optional
//! ```
//!
//! For logs (Loki) connection:
//!
//! ```toml
//! [logs]
//! endpoint = "https://loki.example.com"
//! default_query = "{app=\"nginx\"}"  # optional
//! ```
//!
//! Git integration for go-to-definition and commit markers:
//!
//! ```toml
//! [git]
//! url = "https://github.com/org/repo.git"
//! branch = "main"  # optional
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
pub mod snapshot;
mod templates;

pub use templates::{
    ATLAS_WORKSPACE_TOML, GOLDEN_SIGNALS_TOML, INCIDENT_RESPONSE_TOML, INFRASTRUCTURE_TOML,
    MULTI_SERVICE_TOML, SERVICE_OVERVIEW_TOML,
};

use serde::{Deserialize, Serialize};

/// Current workspace format version
pub const WORKSPACE_VERSION: u32 = 1;

// =============================================================================
// Snapshot Types (for sharing workspaces with embedded data)
// =============================================================================

/// A single series of snapshot data (timestamps + values).
#[derive(Debug, Clone)]
pub struct SnapshotSeries {
    /// Series display name
    pub name: String,
    /// Tags identifying this series, sorted by key for deterministic encoding
    pub tags: Vec<(String, String)>,
    /// Data points as (timestamp_secs, value) pairs
    pub points: Vec<(f64, f64)>,
}

/// Snapshot data for a single pane's visualization.
#[derive(Debug, Clone)]
pub enum SnapshotPaneData {
    /// Time series data (used by TimeSeries and Sparkline viz types)
    TimeSeries { series: Vec<SnapshotSeries> },
    /// Stat display: big number + optional sparkline
    Stat { value: f64, sparkline: Vec<f64> },
    /// Gauge: value within a range
    Gauge { value: f64, min: f64, max: f64 },
    /// Bar chart: labeled bars
    BarChart { bars: Vec<(String, f64)> },
    /// Heatmap: 2D grid of values
    Heatmap {
        cols: u16,
        rows: u16,
        values: Vec<f32>,
    },
}

impl SnapshotPaneData {
    /// Returns true if this snapshot contains no meaningful data.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::TimeSeries { series } => {
                series.is_empty() || series.iter().all(|s| s.points.is_empty())
            }
            Self::Stat { sparkline, .. } => sparkline.is_empty(),
            Self::Gauge { .. } => false, // gauge always has a value
            Self::BarChart { bars } => bars.is_empty(),
            Self::Heatmap { values, .. } => values.is_empty(),
        }
    }
}

/// Metadata for a snapshot workspace (carried in memory, not serialized to TOML).
#[derive(Debug, Clone)]
pub struct SnapshotMeta {
    /// Unix timestamp (seconds) when the snapshot was captured
    pub captured_at: u64,
    /// Per-pane visualization data, indexed by pane position
    pub pane_data: Vec<SnapshotPaneData>,
    /// Optional conversation data (present when loaded from a full snapshot)
    pub conversation: Option<snapshot::SnapshotConversation>,
    /// Optional SQL pane data (query history with results)
    pub sql_pane: Option<snapshot::SnapshotSqlPane>,
}

// =============================================================================
// Core Configuration Types
// =============================================================================

/// A complete workspace definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Workspace metadata
    pub workspace: WorkspaceMeta,

    /// Metrics (Prometheus) connection settings
    #[serde(
        default,
        skip_serializing_if = "MetricsConfig::is_empty",
        alias = "connection"
    )]
    pub metrics: MetricsConfig,

    /// Logs (Loki) connection settings
    #[serde(default, skip_serializing_if = "LogsConfig::is_empty")]
    pub logs: LogsConfig,

    /// Git integration settings (repository for source code awareness)
    #[serde(default, skip_serializing_if = "GitConfig::is_empty")]
    pub git: GitConfig,

    /// View/UI preferences
    #[serde(default, skip_serializing_if = "ViewConfig::is_default")]
    pub view: ViewConfig,

    /// Time range configuration
    #[serde(default, skip_serializing_if = "TimeConfig::is_default")]
    pub time: TimeConfig,

    /// Plugin configuration (enable/disable plugins)
    #[serde(default, skip_serializing_if = "PluginsConfig::is_empty")]
    pub plugins: PluginsConfig,

    /// Section definitions (groups of panes with collapsible headers)
    /// If empty, falls back to legacy `panes` field for backward compatibility
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SectionConfig>,

    /// Legacy pane definitions (deprecated - use sections instead)
    /// Only used when sections is empty for backward compatibility
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panes: Vec<PaneConfig>,

    /// Legacy layout configuration (deprecated - sections manage their own layout)
    /// Only used when sections is empty for backward compatibility
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutConfig>,

    /// Snapshot data (only present when loaded from a snapshot URL).
    /// Not serialized to TOML — carried in memory only.
    #[serde(skip)]
    pub snapshot: Option<SnapshotMeta>,
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

    /// Inline endpoint for simple workspaces (alternative to [connection] section)
    /// If both this and [connection].endpoint are set, [connection] takes precedence.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint: String,
}

fn is_default_version(v: &u32) -> bool {
    *v == WORKSPACE_VERSION
}

fn default_version() -> u32 {
    WORKSPACE_VERSION
}

/// Metrics (Prometheus) connection configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsConfig {
    /// Prometheus API endpoint URL (e.g., "https://prometheus.example.com")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint: String,

    /// API key (optional - often omitted for security, loaded from env instead)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
}

impl MetricsConfig {
    /// Check if this config has any settings
    pub fn is_empty(&self) -> bool {
        self.endpoint.is_empty() && self.api_key.is_empty()
    }

    /// Create a new metrics config with an endpoint
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: String::new(),
        }
    }
}

/// Backward compatibility alias
pub type ConnectionConfig = MetricsConfig;

/// Logs (Loki) connection configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LogsConfig {
    /// Loki API endpoint URL (e.g., "https://loki.example.com")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint: String,

    /// API key (optional - often omitted for security, loaded from env instead)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,

    /// Default LogQL query (optional)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default_query: String,
}

impl LogsConfig {
    /// Check if this config has any settings
    pub fn is_empty(&self) -> bool {
        self.endpoint.is_empty() && self.api_key.is_empty() && self.default_query.is_empty()
    }

    /// Create a new logs config with an endpoint
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: String::new(),
            default_query: String::new(),
        }
    }

    /// Set the default query
    pub fn with_default_query(mut self, query: impl Into<String>) -> Self {
        self.default_query = query.into();
        self
    }
}

/// Git integration configuration
///
/// Allows the editor to connect to a git repository for source code awareness,
/// enabling features like metrics-to-code mapping.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitConfig {
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

impl GitConfig {
    /// Check if this config has any git settings
    pub fn is_empty(&self) -> bool {
        self.url.is_empty()
    }

    /// Create a new git config with a URL
    pub fn with_url(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            branch: String::new(),
            language: String::new(),
        }
    }

    /// Create a new git config with a URL and branch
    pub fn with_url_and_branch(url: impl Into<String>, branch: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            branch: branch.into(),
            language: String::new(),
        }
    }

    /// Set the language for this git config
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }
}

// =============================================================================
// Plugin Configuration
// =============================================================================

/// Plugin configuration for the workspace.
///
/// Allows enabling/disabling plugins and storing plugin-specific settings.
///
/// ```toml
/// [plugins]
/// enabled = ["query-history", "bookmarks", "zen-mode"]
/// disabled = ["metrics-aggregator"]
///
/// [plugins.settings.bookmarks]
/// auto_save = true
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginsConfig {
    /// Plugins to explicitly enable (overrides default disabled state)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enabled: Vec<String>,

    /// Plugins to explicitly disable (overrides default enabled state)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disabled: Vec<String>,

    /// Plugin-specific settings (keyed by plugin name)
    #[serde(
        default,
        skip_serializing_if = "rustc_hash::FxHashMap::is_empty",
        with = "hashmap_compat"
    )]
    pub settings: rustc_hash::FxHashMap<String, toml::Value>,
}

/// Serde compatibility module for FxHashMap (deserializes from any map).
mod hashmap_compat {
    use rustc_hash::FxHashMap;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S, K, V>(map: &FxHashMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        K: Serialize + std::hash::Hash + Eq,
        V: Serialize,
    {
        map.serialize(serializer)
    }

    #[allow(clippy::disallowed_types)]
    pub fn deserialize<'de, D, K, V>(deserializer: D) -> Result<FxHashMap<K, V>, D::Error>
    where
        D: Deserializer<'de>,
        K: Deserialize<'de> + std::hash::Hash + Eq,
        V: Deserialize<'de>,
    {
        // Need to use std::collections::HashMap for serde deserialization, then convert
        let map: std::collections::HashMap<K, V> = Deserialize::deserialize(deserializer)?;
        Ok(map.into_iter().collect())
    }
}

impl PluginsConfig {
    /// Check if this config has any settings
    pub fn is_empty(&self) -> bool {
        self.enabled.is_empty() && self.disabled.is_empty() && self.settings.is_empty()
    }

    /// Check if a plugin is explicitly enabled
    pub fn is_enabled(&self, name: &str) -> Option<bool> {
        if self.enabled.iter().any(|n| n == name) {
            Some(true)
        } else if self.disabled.iter().any(|n| n == name) {
            Some(false)
        } else {
            None // Use plugin's default
        }
    }

    /// Get settings for a specific plugin
    pub fn get_settings(&self, name: &str) -> Option<&toml::Value> {
        self.settings.get(name)
    }

    /// Add a plugin to the enabled list
    pub fn enable(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.disabled.retain(|n| n != &name);
        if !self.enabled.contains(&name) {
            self.enabled.push(name);
        }
    }

    /// Add a plugin to the disabled list
    pub fn disable(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.enabled.retain(|n| n != &name);
        if !self.disabled.contains(&name) {
            self.disabled.push(name);
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

    /// Auto-refresh interval: "off", "10s", "30s", "1m", "5m", "15m"
    /// Defaults to "off" (no auto-refresh)
    #[serde(default, skip_serializing_if = "is_refresh_off")]
    pub refresh: String,
}

fn is_default_time_preset(s: &String) -> bool {
    s == "15m"
}

fn default_time_preset() -> String {
    "15m".to_string()
}

fn is_refresh_off(s: &String) -> bool {
    s.is_empty() || s == "off"
}

impl Default for TimeConfig {
    fn default() -> Self {
        Self {
            preset: default_time_preset(),
            refresh: String::new(),
        }
    }
}

impl TimeConfig {
    /// Get the refresh interval in seconds, or None if disabled
    pub fn refresh_interval_secs(&self) -> Option<u64> {
        RefreshInterval::parse(&self.refresh).to_secs()
    }

    /// Check if auto-refresh is enabled
    pub fn is_refresh_enabled(&self) -> bool {
        self.refresh_interval_secs().is_some()
    }

    /// Check if all values are defaults
    pub fn is_default(&self) -> bool {
        self.preset == "15m" && is_refresh_off(&self.refresh)
    }
}

/// Auto-refresh interval options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefreshInterval {
    /// No auto-refresh
    #[default]
    Off,
    /// Refresh every 10 seconds
    TenSeconds,
    /// Refresh every 30 seconds
    ThirtySeconds,
    /// Refresh every 1 minute
    OneMinute,
    /// Refresh every 5 minutes
    FiveMinutes,
    /// Refresh every 15 minutes
    FifteenMinutes,
}

impl RefreshInterval {
    /// Parse a refresh interval string
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "" | "off" | "none" | "disabled" => Self::Off,
            "10s" => Self::TenSeconds,
            "30s" => Self::ThirtySeconds,
            "1m" | "60s" => Self::OneMinute,
            "5m" | "300s" => Self::FiveMinutes,
            "15m" | "900s" => Self::FifteenMinutes,
            _ => Self::Off,
        }
    }

    /// Get the interval in seconds, or None if disabled
    pub fn to_secs(self) -> Option<u64> {
        match self {
            Self::Off => None,
            Self::TenSeconds => Some(10),
            Self::ThirtySeconds => Some(30),
            Self::OneMinute => Some(60),
            Self::FiveMinutes => Some(300),
            Self::FifteenMinutes => Some(900),
        }
    }

    /// Get the display label
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::TenSeconds => "10s",
            Self::ThirtySeconds => "30s",
            Self::OneMinute => "1m",
            Self::FiveMinutes => "5m",
            Self::FifteenMinutes => "15m",
        }
    }

    /// Get all available options (for UI dropdown)
    pub fn all() -> &'static [Self] {
        &[
            Self::Off,
            Self::TenSeconds,
            Self::ThirtySeconds,
            Self::OneMinute,
            Self::FiveMinutes,
            Self::FifteenMinutes,
        ]
    }
}

impl std::fmt::Display for RefreshInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::TenSeconds => write!(f, "10s"),
            Self::ThirtySeconds => write!(f, "30s"),
            Self::OneMinute => write!(f, "1m"),
            Self::FiveMinutes => write!(f, "5m"),
            Self::FifteenMinutes => write!(f, "15m"),
        }
    }
}

// =============================================================================
// Section Configuration
// =============================================================================

/// Layout type for sections (how panes within a section are arranged)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SectionLayout {
    /// Horizontal arrangement (panes side by side)
    #[default]
    Horizontal,
    /// Vertical arrangement (panes stacked)
    Vertical,
    /// Grid arrangement (rows and columns)
    Grid,
    /// Tabbed arrangement
    Tabs,
}

impl SectionLayout {
    /// Check if this is the default layout (horizontal)
    pub fn is_default(&self) -> bool {
        *self == Self::Horizontal
    }

    /// Parse a layout string (e.g. "horizontal", "vertical", "grid", "tabs")
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "horizontal" => Some(Self::Horizontal),
            "vertical" => Some(Self::Vertical),
            "grid" => Some(Self::Grid),
            "tabs" => Some(Self::Tabs),
            _ => None,
        }
    }
}

/// A section grouping multiple panes with a collapsible header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionConfig {
    /// Section name displayed in the header
    pub name: String,

    /// Layout type for panes within this section
    #[serde(default, skip_serializing_if = "SectionLayout::is_default")]
    pub layout: SectionLayout,

    /// Whether the section is collapsed
    #[serde(default, skip_serializing_if = "is_false")]
    pub collapsed: bool,

    /// Number of columns for grid layout
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<usize>,

    /// Share ratios for panes (for horizontal/vertical layouts)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shares: Vec<f32>,

    /// Panes within this section
    #[serde(default)]
    pub panes: Vec<PaneConfig>,
}

impl SectionConfig {
    /// Create a new section with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            layout: SectionLayout::default(),
            collapsed: false,
            columns: None,
            shares: Vec::new(),
            panes: Vec::new(),
        }
    }

    /// Set the layout type
    pub fn with_layout(mut self, layout: SectionLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Set the collapsed state
    pub fn with_collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// Set the number of columns for grid layout
    pub fn with_columns(mut self, columns: usize) -> Self {
        self.columns = Some(columns);
        self
    }

    /// Add a pane to this section
    pub fn with_pane(mut self, pane: PaneConfig) -> Self {
        self.panes.push(pane);
        self
    }

    /// Get share for pane at index (defaults to 1.0 if not specified)
    pub fn share_for(&self, index: usize) -> f32 {
        self.shares.get(index).copied().unwrap_or(1.0)
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

    /// Description providing context about the pane (shown on hover)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,

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
            description: String::new(),
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

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the tag (e.g., "Critical", "Warning")
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
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
// Value Parsing Helpers (for set_value)
// =============================================================================

fn parse_bool(key: &str, value: &str) -> Result<toml::Value, WorkspaceError> {
    match value {
        "true" => Ok(toml::Value::Boolean(true)),
        "false" => Ok(toml::Value::Boolean(false)),
        _ => Err(WorkspaceError::Decode(format!(
            "{key} is a boolean (expected \"true\" or \"false\")"
        ))),
    }
}

fn parse_integer(key: &str, value: &str) -> Result<toml::Value, WorkspaceError> {
    let i: i64 = value
        .parse()
        .map_err(|_| WorkspaceError::Decode(format!("{key} is an integer")))?;
    Ok(toml::Value::Integer(i))
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
                endpoint: String::new(),
            },
            metrics: MetricsConfig::default(),
            logs: LogsConfig::default(),
            git: GitConfig::default(),
            view: ViewConfig::default(),
            time: TimeConfig::default(),
            plugins: PluginsConfig::default(),
            sections: Vec::new(),
            panes: Vec::new(),
            layout: None,
            snapshot: None,
        }
    }

    /// Create a workspace with an API endpoint (using inline workspace.endpoint)
    pub fn with_endpoint(name: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let mut ws = Self::new(name);
        ws.workspace.endpoint = endpoint.into();
        ws
    }

    /// Get the effective metrics endpoint, preferring [metrics].endpoint over workspace.endpoint
    pub fn effective_endpoint(&self) -> Option<&str> {
        if !self.metrics.endpoint.is_empty() {
            Some(&self.metrics.endpoint)
        } else if !self.workspace.endpoint.is_empty() {
            Some(&self.workspace.endpoint)
        } else {
            None
        }
    }

    /// Get the effective metrics config, merging workspace.endpoint if needed
    pub fn effective_metrics(&self) -> MetricsConfig {
        if !self.metrics.endpoint.is_empty() {
            // [metrics] section takes precedence
            self.metrics.clone()
        } else if !self.workspace.endpoint.is_empty() {
            // Use inline workspace.endpoint
            MetricsConfig::with_endpoint(&self.workspace.endpoint)
        } else {
            MetricsConfig::default()
        }
    }

    /// Backward compatibility alias for effective_metrics
    pub fn effective_connection(&self) -> MetricsConfig {
        self.effective_metrics()
    }

    /// Get the effective logs config
    pub fn effective_logs(&self) -> &LogsConfig {
        &self.logs
    }

    /// Check if logs are configured
    pub fn has_logs_config(&self) -> bool {
        !self.logs.endpoint.is_empty()
    }

    /// Add a pane to the workspace (legacy - prefer add_section)
    pub fn add_pane(&mut self, pane: PaneConfig) {
        self.panes.push(pane);
    }

    /// Add a section to the workspace
    pub fn add_section(&mut self, section: SectionConfig) {
        self.sections.push(section);
    }

    /// Find a section index by name
    pub fn find_section(&self, name: &str) -> Option<usize> {
        self.sections.iter().position(|s| s.name == name)
    }

    /// Find panes matching a name across all sections.
    /// Returns `Vec<(section_index, pane_index)>`.
    pub fn find_pane_by_name(&self, name: &str) -> Vec<(usize, usize)> {
        let mut results = Vec::new();
        for (si, section) in self.sections.iter().enumerate() {
            for (pi, pane) in section.panes.iter().enumerate() {
                if pane.name == name {
                    results.push((si, pi));
                }
            }
        }
        results
    }

    /// Ensure at least one section exists.
    /// Migrates legacy panes into a "Default" section, or creates an empty one.
    pub fn ensure_default_section(&mut self) {
        if self.sections.is_empty() {
            if !self.panes.is_empty() {
                let section = SectionConfig {
                    panes: std::mem::take(&mut self.panes),
                    ..SectionConfig::new("Default")
                };
                self.sections.push(section);
                self.layout = None;
            } else {
                self.sections.push(SectionConfig::new("Default"));
            }
        }
    }

    /// Get all panes across all sections (flattened view)
    /// If sections is empty, returns legacy panes
    pub fn all_panes(&self) -> Vec<&PaneConfig> {
        if !self.sections.is_empty() {
            self.sections.iter().flat_map(|s| &s.panes).collect()
        } else {
            self.panes.iter().collect()
        }
    }

    /// Check if using the new sections format
    pub fn uses_sections(&self) -> bool {
        !self.sections.is_empty()
    }

    /// Migrate legacy panes to a single default section
    /// Returns the workspace unchanged if already using sections
    pub fn migrate_to_sections(mut self) -> Self {
        if self.sections.is_empty() && !self.panes.is_empty() {
            let section = SectionConfig {
                name: "Default".to_string(),
                layout: SectionLayout::default(),
                collapsed: false,
                columns: None,
                shares: Vec::new(),
                panes: std::mem::take(&mut self.panes),
            };
            self.sections.push(section);
            self.layout = None;
        }
        self
    }

    /// Get a property value by dot-notation key (e.g. "time.preset", "metrics.endpoint").
    ///
    /// The struct always has defaults filled by serde, so this returns the effective
    /// value even for fields not explicitly set in the TOML file.
    pub fn get_value(&self, key: &str) -> Result<String, WorkspaceError> {
        let val = match key {
            "workspace.name" => self.workspace.name.as_str(),
            "workspace.description" => self.workspace.description.as_str(),
            "workspace.endpoint" => self.workspace.endpoint.as_str(),
            "metrics.endpoint" => self.metrics.endpoint.as_str(),
            "metrics.api_key" => self.metrics.api_key.as_str(),
            "logs.endpoint" => self.logs.endpoint.as_str(),
            "logs.api_key" => self.logs.api_key.as_str(),
            "logs.default_query" => self.logs.default_query.as_str(),
            "git.url" => self.git.url.as_str(),
            "git.branch" => self.git.branch.as_str(),
            "git.language" => self.git.language.as_str(),
            "view.theme" => self.view.theme.as_str(),
            "view.zen_mode" => return Ok(self.view.zen_mode.to_string()),
            "time.preset" => self.time.preset.as_str(),
            "time.refresh" => {
                if self.time.refresh.is_empty() {
                    return Ok("off".to_string());
                }
                self.time.refresh.as_str()
            }
            _ => {
                return Err(WorkspaceError::Decode(format!("unknown property: {key}")));
            }
        };
        Ok(val.to_string())
    }

    /// Set a property value by dot-notation key.
    ///
    /// Works by modifying the serialized TOML table, then round-tripping through
    /// deserialization for validation. This means any field the struct knows about
    /// is settable, and invalid values are caught by serde.
    pub fn set_value(&mut self, key: &str, value: &str) -> Result<(), WorkspaceError> {
        let (section, field) = key.split_once('.').ok_or_else(|| {
            WorkspaceError::Decode(format!(
                "invalid key format: {key} (expected section.field)"
            ))
        })?;

        // Serialize current config to TOML value tree
        let toml_str = self.to_toml()?;
        let mut root: toml::Value = toml_str.parse().map_err(WorkspaceError::Parse)?;

        let root_table = root
            .as_table_mut()
            .ok_or_else(|| WorkspaceError::Decode("config is not a table".into()))?;

        // Ensure the section table exists (may have been skipped by skip_serializing_if)
        let section_table = root_table
            .entry(section)
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
            .as_table_mut()
            .ok_or_else(|| WorkspaceError::Decode(format!("{section} is not a table")))?;

        // Infer the correct TOML type from the existing value or known schema
        let typed = match section_table.get(field) {
            Some(toml::Value::Boolean(_)) => parse_bool(key, value)?,
            Some(toml::Value::Integer(_)) => parse_integer(key, value)?,
            _ => {
                // Field doesn't exist yet or is a string — check known booleans/integers
                match key {
                    "view.zen_mode" => parse_bool(key, value)?,
                    "workspace.version" => parse_integer(key, value)?,
                    _ => toml::Value::String(value.to_string()),
                }
            }
        };

        section_table.insert(field.to_string(), typed);

        // Round-trip: serialize back to string, then deserialize as WorkspaceConfig.
        // This validates the value through serde (e.g. unknown fields, type mismatches).
        let new_toml = toml::to_string_pretty(&root).map_err(WorkspaceError::Serialize)?;
        *self = Self::from_toml(&new_toml)?;

        Ok(())
    }

    /// Parse a workspace from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, WorkspaceError> {
        toml::from_str(toml_str).map_err(WorkspaceError::Parse)
    }

    /// Get the default example workspace (golden signals)
    pub fn default_example() -> Self {
        Self::from_toml(templates::GOLDEN_SIGNALS_TOML)
            .expect("GOLDEN_SIGNALS_TOML should be valid")
    }

    /// Get the demo workspace (golden signals, no backend required)
    pub fn default_demo() -> Self {
        Self::from_toml(templates::GOLDEN_SIGNALS_TOML)
            .expect("GOLDEN_SIGNALS_TOML should be valid")
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

    /// Encode workspace as a snapshot to base64 (includes visualization data).
    /// `captured_at` is the Unix timestamp (seconds) — use a platform-safe source.
    pub fn snapshot_to_base64(
        &self,
        pane_data: &[SnapshotPaneData],
        captured_at: u64,
    ) -> Result<String, WorkspaceError> {
        compact::encode_snapshot_workspace(self, pane_data, captured_at)
    }

    /// Encode a single pane as a snapshot to base64 (includes visualization data).
    /// `captured_at` is the Unix timestamp (seconds) — use a platform-safe source.
    pub fn snapshot_pane_to_base64(
        &self,
        pane_index: usize,
        data: &SnapshotPaneData,
        captured_at: u64,
    ) -> Result<String, WorkspaceError> {
        compact::encode_snapshot_pane(self, pane_index, data, captured_at)
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
    fn test_base64_encoding() {
        let ws = WorkspaceConfig::new("shared");
        let encoded = ws.to_base64().unwrap();
        let decoded = WorkspaceConfig::from_base64(&encoded).unwrap();
        assert_eq!(decoded.workspace.name, "shared");
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

        // Metrics defaults (empty)
        assert!(ws.metrics.is_empty());

        // Logs defaults (empty)
        assert!(ws.logs.is_empty());
    }

    #[test]
    fn test_metrics_config() {
        let toml = r#"
[workspace]
name = "with-endpoint"

[metrics]
endpoint = "https://prometheus.example.com"

[[panes]]
query = "env:prod"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.metrics.endpoint, "https://prometheus.example.com");
        assert!(ws.metrics.api_key.is_empty());
        assert!(!ws.metrics.is_empty());

        // Test serialization - empty api_key should be omitted
        let serialized = ws.to_toml().unwrap();
        assert!(serialized.contains("endpoint = \"https://prometheus.example.com\""));
        assert!(!serialized.contains("api_key"));
    }

    #[test]
    fn test_metrics_with_api_key() {
        let toml = r#"
[workspace]
name = "with-key"

[metrics]
endpoint = "https://prometheus.example.com"
api_key = "sk-test-123"

[[panes]]
query = "env:prod"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.metrics.endpoint, "https://prometheus.example.com");
        assert_eq!(ws.metrics.api_key, "sk-test-123");
    }

    #[test]
    fn test_connection_backward_compat() {
        // Test that old [connection] section still works via serde alias
        let toml = r#"
[workspace]
name = "legacy"

[connection]
endpoint = "https://legacy.example.com"

[[panes]]
query = "env:prod"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.metrics.endpoint, "https://legacy.example.com");
    }

    #[test]
    fn test_logs_config() {
        let toml = r#"
[workspace]
name = "with-logs"

[logs]
endpoint = "https://loki.example.com"
default_query = "{app=\"nginx\"}"

[[panes]]
query = "env:prod"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.logs.endpoint, "https://loki.example.com");
        assert_eq!(ws.logs.default_query, "{app=\"nginx\"}");
        assert!(ws.logs.api_key.is_empty());
        assert!(!ws.logs.is_empty());
        assert!(ws.has_logs_config());
    }

    #[test]
    fn test_metrics_and_logs_config() {
        let toml = r#"
[workspace]
name = "full-observability"

[metrics]
endpoint = "https://prometheus.example.com"

[logs]
endpoint = "https://loki.example.com"

[[panes]]
query = "env:prod"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.metrics.endpoint, "https://prometheus.example.com");
        assert_eq!(ws.logs.endpoint, "https://loki.example.com");
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

    // ==================== MetricsConfig Tests ====================

    #[test]
    fn test_metrics_config_default() {
        let config = MetricsConfig::default();
        assert!(config.endpoint.is_empty());
        assert!(config.api_key.is_empty());
        assert!(config.is_empty());
    }

    #[test]
    fn test_metrics_config_with_endpoint() {
        let config = MetricsConfig::with_endpoint("https://prometheus.example.com");
        assert_eq!(config.endpoint, "https://prometheus.example.com");
        assert!(config.api_key.is_empty());
        assert!(!config.is_empty());
    }

    #[test]
    fn test_metrics_config_is_empty() {
        let mut config = MetricsConfig::default();
        assert!(config.is_empty());

        config.endpoint = "http://localhost".to_string();
        assert!(!config.is_empty());

        config.endpoint = String::new();
        config.api_key = "key".to_string();
        assert!(!config.is_empty());
    }

    // ==================== LogsConfig Tests ====================

    #[test]
    fn test_logs_config_default() {
        let config = LogsConfig::default();
        assert!(config.endpoint.is_empty());
        assert!(config.api_key.is_empty());
        assert!(config.default_query.is_empty());
        assert!(config.is_empty());
    }

    #[test]
    fn test_logs_config_with_endpoint() {
        let config = LogsConfig::with_endpoint("https://loki.example.com");
        assert_eq!(config.endpoint, "https://loki.example.com");
        assert!(config.api_key.is_empty());
        assert!(config.default_query.is_empty());
        assert!(!config.is_empty());
    }

    #[test]
    fn test_logs_config_with_default_query() {
        let config = LogsConfig::with_endpoint("https://loki.example.com")
            .with_default_query("{app=\"nginx\"}");
        assert_eq!(config.endpoint, "https://loki.example.com");
        assert_eq!(config.default_query, "{app=\"nginx\"}");
    }

    #[test]
    fn test_logs_config_is_empty() {
        let mut config = LogsConfig::default();
        assert!(config.is_empty());

        config.endpoint = "http://localhost".to_string();
        assert!(!config.is_empty());

        config.endpoint = String::new();
        config.default_query = "{job=\"test\"}".to_string();
        assert!(!config.is_empty());
    }

    // ==================== PaneConfig Builder Tests ====================

    #[test]
    fn test_pane_config_new_defaults() {
        let pane = PaneConfig::new("query");
        assert_eq!(pane.query, "query");
        assert!(pane.name.is_empty());
        assert!(pane.tag.is_empty());
        assert_eq!(pane.granularity, "5m");
        assert_eq!(pane.visualization, "time_series");
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
        assert!(ws.metrics.is_empty());
        assert!(ws.logs.is_empty());
        assert!(ws.panes.is_empty());
        assert!(ws.layout.is_none());
    }

    #[test]
    fn test_workspace_config_with_endpoint() {
        let ws = WorkspaceConfig::with_endpoint("test", "https://api.example.com");
        assert_eq!(ws.workspace.name, "test");
        // endpoint is now stored inline in workspace section
        assert_eq!(ws.workspace.endpoint, "https://api.example.com");
        assert!(ws.metrics.is_empty());
        // effective_endpoint should return the inline endpoint
        assert_eq!(ws.effective_endpoint(), Some("https://api.example.com"));
    }

    #[test]
    fn test_workspace_config_effective_endpoint_precedence() {
        // When both workspace.endpoint and metrics.endpoint are set,
        // metrics takes precedence
        let mut ws = WorkspaceConfig::with_endpoint("test", "http://inline:9090");
        ws.metrics = MetricsConfig::with_endpoint("http://metrics:9090");
        assert_eq!(ws.effective_endpoint(), Some("http://metrics:9090"));

        // When only workspace.endpoint is set
        let ws = WorkspaceConfig::with_endpoint("test", "http://inline:9090");
        assert_eq!(ws.effective_endpoint(), Some("http://inline:9090"));

        // When neither is set
        let ws = WorkspaceConfig::new("test");
        assert_eq!(ws.effective_endpoint(), None);
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
        assert!(!toml.contains("[metrics]")); // Empty metrics
        assert!(!toml.contains("[logs]")); // Empty logs
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

    // ==================== Section Configuration Tests ====================

    #[test]
    fn test_section_layout_default() {
        let layout = SectionLayout::default();
        assert_eq!(layout, SectionLayout::Horizontal);
    }

    #[test]
    fn test_section_layout_serde() {
        let toml = r#"
[workspace]
name = "section-test"

[[sections]]
name = "API"
layout = "horizontal"

[[sections.panes]]
query = "test1"

[[sections]]
name = "Infra"
layout = "grid"
columns = 2

[[sections.panes]]
query = "test2"

[[sections]]
name = "Logs"
layout = "vertical"

[[sections.panes]]
query = "test3"

[[sections]]
name = "Overview"
layout = "tabs"

[[sections.panes]]
query = "test4"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.sections.len(), 4);
        assert_eq!(ws.sections[0].layout, SectionLayout::Horizontal);
        assert_eq!(ws.sections[1].layout, SectionLayout::Grid);
        assert_eq!(ws.sections[1].columns, Some(2));
        assert_eq!(ws.sections[2].layout, SectionLayout::Vertical);
        assert_eq!(ws.sections[3].layout, SectionLayout::Tabs);
    }

    #[test]
    fn test_section_config_new() {
        let section = SectionConfig::new("Test Section");
        assert_eq!(section.name, "Test Section");
        assert_eq!(section.layout, SectionLayout::Horizontal);
        assert!(!section.collapsed);
        assert!(section.columns.is_none());
        assert!(section.shares.is_empty());
        assert!(section.panes.is_empty());
    }

    #[test]
    fn test_section_config_builder() {
        let section = SectionConfig::new("API Performance")
            .with_layout(SectionLayout::Grid)
            .with_columns(3)
            .with_collapsed(true)
            .with_pane(PaneConfig::new("query1").with_name("Pane 1"))
            .with_pane(PaneConfig::new("query2").with_name("Pane 2"));

        assert_eq!(section.name, "API Performance");
        assert_eq!(section.layout, SectionLayout::Grid);
        assert!(section.collapsed);
        assert_eq!(section.columns, Some(3));
        assert_eq!(section.panes.len(), 2);
    }

    #[test]
    fn test_section_config_share_for() {
        let mut section = SectionConfig::new("Test");
        section.shares = vec![0.3, 0.7];

        assert!((section.share_for(0) - 0.3).abs() < 0.001);
        assert!((section.share_for(1) - 0.7).abs() < 0.001);
        assert!((section.share_for(2) - 1.0).abs() < 0.001); // Default
    }

    #[test]
    fn test_parse_sections_toml() {
        let toml = r#"
[workspace]
name = "sections-dashboard"

[[sections]]
name = "API Performance"
layout = "horizontal"

[[sections.panes]]
query = "rate(http_requests_total[5m])"
name = "Request Rate"

[[sections.panes]]
query = "histogram_quantile(0.99, sum(rate(http_request_duration_seconds_bucket[5m])) by (le))"
name = "Latency p99"

[[sections]]
name = "Infrastructure"
layout = "grid"
columns = 2
collapsed = true

[[sections.panes]]
query = "avg(cpu_usage)"
name = "CPU Usage"

[[sections.panes]]
query = "avg(memory_usage)"
name = "Memory Usage"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.workspace.name, "sections-dashboard");
        assert_eq!(ws.sections.len(), 2);

        // First section
        assert_eq!(ws.sections[0].name, "API Performance");
        assert_eq!(ws.sections[0].layout, SectionLayout::Horizontal);
        assert!(!ws.sections[0].collapsed);
        assert_eq!(ws.sections[0].panes.len(), 2);
        assert_eq!(ws.sections[0].panes[0].name, "Request Rate");

        // Second section
        assert_eq!(ws.sections[1].name, "Infrastructure");
        assert_eq!(ws.sections[1].layout, SectionLayout::Grid);
        assert_eq!(ws.sections[1].columns, Some(2));
        assert!(ws.sections[1].collapsed);
        assert_eq!(ws.sections[1].panes.len(), 2);
    }

    #[test]
    fn test_workspace_uses_sections() {
        let mut ws = WorkspaceConfig::new("test");
        assert!(!ws.uses_sections());

        ws.add_section(SectionConfig::new("Test").with_pane(PaneConfig::new("query")));
        assert!(ws.uses_sections());
    }

    #[test]
    fn test_workspace_all_panes_with_sections() {
        let mut ws = WorkspaceConfig::new("test");
        ws.add_section(
            SectionConfig::new("Section 1")
                .with_pane(PaneConfig::new("q1"))
                .with_pane(PaneConfig::new("q2")),
        );
        ws.add_section(SectionConfig::new("Section 2").with_pane(PaneConfig::new("q3")));

        let all = ws.all_panes();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].query, "q1");
        assert_eq!(all[1].query, "q2");
        assert_eq!(all[2].query, "q3");
    }

    #[test]
    fn test_workspace_all_panes_legacy() {
        let mut ws = WorkspaceConfig::new("test");
        ws.add_pane(PaneConfig::new("q1"));
        ws.add_pane(PaneConfig::new("q2"));

        let all = ws.all_panes();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].query, "q1");
        assert_eq!(all[1].query, "q2");
    }

    #[test]
    fn test_workspace_migrate_to_sections() {
        let mut ws = WorkspaceConfig::new("test");
        ws.add_pane(PaneConfig::new("q1").with_name("Pane 1"));
        ws.add_pane(PaneConfig::new("q2").with_name("Pane 2"));

        assert!(!ws.uses_sections());
        assert_eq!(ws.panes.len(), 2);

        let ws = ws.migrate_to_sections();

        assert!(ws.uses_sections());
        assert_eq!(ws.sections.len(), 1);
        assert_eq!(ws.sections[0].name, "Default");
        assert_eq!(ws.sections[0].panes.len(), 2);
        assert!(ws.panes.is_empty());
    }

    #[test]
    fn test_section_layout_parse() {
        assert_eq!(
            SectionLayout::parse("horizontal"),
            Some(SectionLayout::Horizontal)
        );
        assert_eq!(
            SectionLayout::parse("Vertical"),
            Some(SectionLayout::Vertical)
        );
        assert_eq!(SectionLayout::parse("GRID"), Some(SectionLayout::Grid));
        assert_eq!(SectionLayout::parse("tabs"), Some(SectionLayout::Tabs));
        assert_eq!(SectionLayout::parse("invalid"), None);
    }

    #[test]
    fn test_find_section() {
        let mut ws = WorkspaceConfig::new("test");
        ws.add_section(SectionConfig::new("API"));
        ws.add_section(SectionConfig::new("Infra"));

        assert_eq!(ws.find_section("API"), Some(0));
        assert_eq!(ws.find_section("Infra"), Some(1));
        assert_eq!(ws.find_section("Missing"), None);
    }

    #[test]
    fn test_find_pane_by_name() {
        let mut ws = WorkspaceConfig::new("test");
        ws.add_section(
            SectionConfig::new("S1")
                .with_pane(PaneConfig::new("q1").with_name("Request Rate"))
                .with_pane(PaneConfig::new("q2").with_name("Latency")),
        );
        ws.add_section(SectionConfig::new("S2").with_pane(PaneConfig::new("q3").with_name("CPU")));

        assert_eq!(ws.find_pane_by_name("Request Rate"), vec![(0, 0)]);
        assert_eq!(ws.find_pane_by_name("CPU"), vec![(1, 0)]);
        assert!(ws.find_pane_by_name("Missing").is_empty());
    }

    #[test]
    fn test_ensure_default_section_empty() {
        let mut ws = WorkspaceConfig::new("test");
        assert!(ws.sections.is_empty());

        ws.ensure_default_section();
        assert_eq!(ws.sections.len(), 1);
        assert_eq!(ws.sections[0].name, "Default");
        assert!(ws.sections[0].panes.is_empty());
    }

    #[test]
    fn test_ensure_default_section_with_legacy_panes() {
        let mut ws = WorkspaceConfig::new("test");
        ws.add_pane(PaneConfig::new("q1").with_name("P1"));
        ws.add_pane(PaneConfig::new("q2").with_name("P2"));

        ws.ensure_default_section();
        assert_eq!(ws.sections.len(), 1);
        assert_eq!(ws.sections[0].name, "Default");
        assert_eq!(ws.sections[0].panes.len(), 2);
        assert!(ws.panes.is_empty());
    }

    #[test]
    fn test_ensure_default_section_noop_if_sections_exist() {
        let mut ws = WorkspaceConfig::new("test");
        ws.add_section(SectionConfig::new("Existing"));

        ws.ensure_default_section();
        assert_eq!(ws.sections.len(), 1);
        assert_eq!(ws.sections[0].name, "Existing");
    }

    #[test]
    fn test_workspace_migrate_to_sections_noop() {
        // Already using sections - should be a no-op
        let mut ws = WorkspaceConfig::new("test");
        ws.add_section(SectionConfig::new("Existing").with_pane(PaneConfig::new("q1")));

        let ws = ws.migrate_to_sections();

        assert_eq!(ws.sections.len(), 1);
        assert_eq!(ws.sections[0].name, "Existing");
    }

    #[test]
    fn test_get_value_defaults() {
        let ws = WorkspaceConfig::new("test-ws");
        assert_eq!(ws.get_value("workspace.name").unwrap(), "test-ws");
        assert_eq!(ws.get_value("workspace.description").unwrap(), "");
        assert_eq!(ws.get_value("workspace.endpoint").unwrap(), "");
        assert_eq!(ws.get_value("view.theme").unwrap(), "dark");
        assert_eq!(ws.get_value("view.zen_mode").unwrap(), "false");
        assert_eq!(ws.get_value("time.preset").unwrap(), "15m");
        assert_eq!(ws.get_value("time.refresh").unwrap(), "off");
        assert_eq!(ws.get_value("metrics.endpoint").unwrap(), "");
        assert_eq!(ws.get_value("git.url").unwrap(), "");
    }

    #[test]
    fn test_get_value_with_endpoint() {
        let ws = WorkspaceConfig::with_endpoint("test-ws", "http://prom:9090");
        assert_eq!(
            ws.get_value("workspace.endpoint").unwrap(),
            "http://prom:9090"
        );
    }

    #[test]
    fn test_get_value_unknown_key() {
        let ws = WorkspaceConfig::new("test");
        assert!(ws.get_value("bogus.field").is_err());
        assert!(ws.get_value("workspace.nonexistent").is_err());
    }

    #[test]
    fn test_set_value_string_field() {
        let mut ws = WorkspaceConfig::new("test");
        ws.set_value("time.preset", "1h").unwrap();
        assert_eq!(ws.time.preset, "1h");

        ws.set_value("workspace.description", "My dashboard")
            .unwrap();
        assert_eq!(ws.workspace.description, "My dashboard");
    }

    #[test]
    fn test_set_value_endpoint() {
        let mut ws = WorkspaceConfig::new("test");
        ws.set_value("metrics.endpoint", "http://prom:9090")
            .unwrap();
        assert_eq!(ws.metrics.endpoint, "http://prom:9090");
    }

    #[test]
    fn test_set_value_boolean() {
        let mut ws = WorkspaceConfig::new("test");
        ws.set_value("view.zen_mode", "true").unwrap();
        assert!(ws.view.zen_mode);

        ws.set_value("view.zen_mode", "false").unwrap();
        assert!(!ws.view.zen_mode);
    }

    #[test]
    fn test_set_value_boolean_invalid() {
        let mut ws = WorkspaceConfig::new("test");
        assert!(ws.set_value("view.zen_mode", "yes").is_err());
    }

    #[test]
    fn test_set_value_invalid_key_format() {
        let mut ws = WorkspaceConfig::new("test");
        assert!(ws.set_value("noperiod", "val").is_err());
    }

    #[test]
    fn test_set_value_roundtrip() {
        let mut ws = WorkspaceConfig::new("roundtrip-test");
        ws.set_value("workspace.endpoint", "http://localhost:9090")
            .unwrap();
        ws.set_value("time.preset", "30m").unwrap();
        ws.set_value("view.theme", "light").unwrap();

        // Verify all values stuck after multiple set operations
        assert_eq!(
            ws.get_value("workspace.endpoint").unwrap(),
            "http://localhost:9090"
        );
        assert_eq!(ws.get_value("time.preset").unwrap(), "30m");
        assert_eq!(ws.get_value("view.theme").unwrap(), "light");
        // Other defaults should be preserved
        assert_eq!(ws.get_value("workspace.name").unwrap(), "roundtrip-test");
        assert_eq!(ws.get_value("view.zen_mode").unwrap(), "false");
    }

    #[test]
    fn test_snapshot_workspace_with_sections() {
        let mut ws = WorkspaceConfig::new("sections-dashboard");
        ws.add_section(
            SectionConfig::new("API Performance")
                .with_layout(SectionLayout::Horizontal)
                .with_pane(
                    PaneConfig::new("rate(http_requests_total[5m])").with_name("Request Rate"),
                )
                .with_pane(
                    PaneConfig::new("histogram_quantile(0.99, latency)").with_name("Latency p99"),
                ),
        );
        ws.add_section(
            SectionConfig::new("Infrastructure")
                .with_layout(SectionLayout::Grid)
                .with_columns(2)
                .with_collapsed(true)
                .with_pane(PaneConfig::new("avg(cpu_usage)").with_name("CPU"))
                .with_pane(PaneConfig::new("avg(memory_usage)").with_name("Memory")),
        );

        let toml = ws.to_toml().unwrap();
        insta::assert_snapshot!(toml, @r#"
        [workspace]
        name = "sections-dashboard"

        [[sections]]
        name = "API Performance"

        [[sections.panes]]
        query = "rate(http_requests_total[5m])"
        name = "Request Rate"

        [[sections.panes]]
        query = "histogram_quantile(0.99, latency)"
        name = "Latency p99"

        [[sections]]
        name = "Infrastructure"
        layout = "grid"
        collapsed = true
        columns = 2

        [[sections.panes]]
        query = "avg(cpu_usage)"
        name = "CPU"

        [[sections.panes]]
        query = "avg(memory_usage)"
        name = "Memory"
        "#);
    }
}
