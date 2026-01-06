//! Grafana dashboard JSON to Enya workspace TOML converter
//!
//! This module provides functionality to convert Grafana dashboard JSON exports
//! into Enya's workspace TOML format.
//!
//! # Usage
//!
//! ```ignore
//! use enya_editor::grafana::GrafanaDashboard;
//!
//! let json = std::fs::read_to_string("dashboard.json")?;
//! let dashboard: GrafanaDashboard = serde_json::from_str(&json)?;
//! let workspace = dashboard.to_workspace()?;
//! let toml = workspace.to_toml()?;
//! ```
//!
//! # Supported Features
//!
//! - Panel types: timeseries, graph, stat, singlestat, gauge, barchart, bargauge, heatmap
//! - Grid layout conversion to i3-style nested containers
//! - Time range preset mapping
//! - Prometheus query extraction
//!
//! # Limitations
//!
//! - Only Prometheus-style queries are fully supported
//! - Some panel types (table, logs, nodeGraph) have no Enya equivalent
//! - Dashboard variables are not yet converted
//! - Panel-specific options (thresholds, legends) are not preserved

use serde::{Deserialize, Serialize};

use super::config::{
    LayoutConfig, LayoutContainer, LayoutNode, LayoutType, PaneConfig, TimeConfig, ViewConfig,
    WORKSPACE_VERSION, WorkspaceConfig, WorkspaceMeta,
};

/// A Grafana dashboard JSON structure
/// Only includes fields relevant for conversion
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrafanaDashboard {
    /// Dashboard title
    #[serde(default)]
    pub title: String,

    /// Dashboard description
    #[serde(default)]
    pub description: Option<String>,

    /// Dashboard UID (optional)
    #[serde(default)]
    pub uid: Option<String>,

    /// Panels in the dashboard
    #[serde(default)]
    pub panels: Vec<GrafanaPanel>,

    /// Time range settings
    #[serde(default)]
    pub time: Option<GrafanaTimeRange>,

    /// Template variables
    #[serde(default)]
    pub templating: Option<GrafanaTemplating>,

    /// Refresh interval
    #[serde(default)]
    pub refresh: Option<String>,
}

/// A panel in a Grafana dashboard
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrafanaPanel {
    /// Panel ID
    #[serde(default)]
    pub id: u64,

    /// Panel title
    #[serde(default)]
    pub title: String,

    /// Panel type (timeseries, graph, stat, gauge, etc.)
    #[serde(rename = "type", default)]
    pub panel_type: String,

    /// Grid position
    #[serde(default)]
    pub grid_pos: Option<GridPos>,

    /// Query targets
    #[serde(default)]
    pub targets: Vec<GrafanaTarget>,

    /// Datasource reference
    #[serde(default)]
    pub datasource: Option<DatasourceRef>,

    /// Panel description
    #[serde(default)]
    pub description: Option<String>,

    /// Nested panels (for row panels)
    #[serde(default)]
    pub panels: Vec<GrafanaPanel>,

    /// Whether this is a collapsed row
    #[serde(default)]
    pub collapsed: bool,

    /// Field configuration (contains unit, thresholds, etc.)
    #[serde(default)]
    pub field_config: Option<FieldConfig>,
}

/// Grid position for a panel (24-column grid)
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default)]
pub struct GridPos {
    /// X position (0-23)
    pub x: u32,
    /// Y position
    pub y: u32,
    /// Width (1-24)
    pub w: u32,
    /// Height
    pub h: u32,
}

/// A query target in a panel
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrafanaTarget {
    /// Prometheus expression
    #[serde(default)]
    pub expr: Option<String>,

    /// PromQL expression (alternative field name)
    #[serde(default)]
    pub prom_ql: Option<String>,

    /// Legend format
    #[serde(default)]
    pub legend_format: Option<String>,

    /// Reference ID
    #[serde(default, rename = "refId")]
    pub ref_id: Option<String>,

    /// Datasource for this target
    #[serde(default)]
    pub datasource: Option<DatasourceRef>,

    /// Query type (for mixed datasources)
    #[serde(default)]
    pub query_type: Option<String>,

    /// Raw query (for some datasource types)
    #[serde(default)]
    pub query: Option<String>,
}

/// Datasource reference
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatasourceRef {
    /// Datasource type (prometheus, loki, etc.)
    #[serde(rename = "type", default)]
    pub ds_type: Option<String>,

    /// Datasource UID
    #[serde(default)]
    pub uid: Option<String>,
}

/// Time range configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrafanaTimeRange {
    /// From time (e.g., "now-1h", "now-6h")
    pub from: String,
    /// To time (usually "now")
    pub to: String,
}

/// Template variables
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrafanaTemplating {
    /// List of template variables
    #[serde(default)]
    pub list: Vec<GrafanaVariable>,
}

/// A template variable
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrafanaVariable {
    /// Variable name
    #[serde(default)]
    pub name: String,

    /// Variable type (query, custom, constant, etc.)
    #[serde(rename = "type", default)]
    pub var_type: String,

    /// Current value
    #[serde(default)]
    pub current: Option<VariableValue>,

    /// Query (for query-type variables)
    #[serde(default)]
    pub query: Option<String>,
}

/// Current value of a variable
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VariableValue {
    /// Selected value
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    /// Display text
    #[serde(default)]
    pub text: Option<serde_json::Value>,
}

/// Field configuration for a panel
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct FieldConfig {
    /// Default field configuration
    #[serde(default)]
    pub defaults: Option<FieldDefaults>,
}

/// Default field settings
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct FieldDefaults {
    /// Unit for the values (e.g., "ms", "percent", "bytes", "reqps")
    #[serde(default)]
    pub unit: Option<String>,
}

/// Errors that can occur during conversion
#[derive(Debug)]
pub enum ConversionError {
    /// No panels found in dashboard
    NoPanels,
    /// Failed to parse JSON
    JsonParse(String),
    /// Unsupported panel type
    UnsupportedPanelType(String),
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPanels => write!(f, "dashboard has no panels"),
            Self::JsonParse(e) => write!(f, "failed to parse JSON: {e}"),
            Self::UnsupportedPanelType(t) => write!(f, "unsupported panel type: {t}"),
        }
    }
}

impl std::error::Error for ConversionError {}

/// Result of a conversion with warnings
#[derive(Debug)]
pub struct ConversionResult {
    /// The converted workspace configuration
    pub workspace: WorkspaceConfig,
    /// Warnings generated during conversion
    pub warnings: Vec<String>,
}

impl GrafanaDashboard {
    /// Parse a Grafana dashboard from JSON string
    pub fn from_json(json: &str) -> Result<Self, ConversionError> {
        serde_json::from_str(json).map_err(|e| ConversionError::JsonParse(e.to_string()))
    }

    /// Convert this Grafana dashboard to an Enya workspace
    pub fn to_workspace(&self) -> Result<ConversionResult, ConversionError> {
        let mut warnings = Vec::new();

        // Collect all panels, including those nested in collapsed rows
        let all_panels = self.collect_all_panels();

        // Filter to only convertible panels
        let convertible_panels: Vec<&GrafanaPanel> = all_panels
            .into_iter()
            .filter(|p| {
                // Skip row panels (they're just organizational)
                if p.panel_type == "row" {
                    return false;
                }
                // Check if panel type is supported
                if !is_panel_type_supported(&p.panel_type) {
                    warnings.push(format!(
                        "Panel '{}' has unsupported type '{}', skipping",
                        p.title, p.panel_type
                    ));
                    return false;
                }
                true
            })
            .collect();

        if convertible_panels.is_empty() {
            return Err(ConversionError::NoPanels);
        }

        // Create workspace config
        let mut workspace = WorkspaceConfig {
            workspace: WorkspaceMeta {
                name: self.title.clone(),
                description: self.description.clone().unwrap_or_default(),
                version: WORKSPACE_VERSION,
                endpoint: String::new(),
            },
            connection: Default::default(),
            git: Default::default(),
            view: ViewConfig::default(),
            time: TimeConfig::default(),
            panes: Vec::new(),
            layout: None,
        };

        // Convert time range
        if let Some(time) = &self.time {
            workspace.time.preset = convert_time_range(&time.from);
        }

        // Warn about variables
        if let Some(templating) = &self.templating {
            if !templating.list.is_empty() {
                warnings.push(format!(
                    "Dashboard has {} template variables that are not converted: {}",
                    templating.list.len(),
                    templating
                        .list
                        .iter()
                        .map(|v| format!("${}", v.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }

        // Convert panels to panes
        for panel in &convertible_panels {
            if let Some(pane) = convert_panel(panel, &mut warnings) {
                workspace.panes.push(pane);
            }
        }

        // Convert layout
        if let Some(layout) = convert_layout(&convertible_panels) {
            workspace.layout = Some(layout);
        }

        Ok(ConversionResult {
            workspace,
            warnings,
        })
    }

    /// Collect all panels, including those nested in collapsed rows
    fn collect_all_panels(&self) -> Vec<&GrafanaPanel> {
        let mut all_panels = Vec::new();
        for panel in &self.panels {
            all_panels.push(panel);
            // If this is a collapsed row, include its nested panels
            if panel.panel_type == "row" && !panel.panels.is_empty() {
                for nested in &panel.panels {
                    all_panels.push(nested);
                }
            }
        }
        all_panels
    }
}

/// Check if a panel type is supported for conversion
fn is_panel_type_supported(panel_type: &str) -> bool {
    matches!(
        panel_type,
        "timeseries"
            | "graph"
            | "stat"
            | "singlestat"
            | "gauge"
            | "barchart"
            | "bargauge"
            | "heatmap"
            | "sparkline"
            | "text" // Text panels become time_series with empty query
    )
}

/// Convert Grafana panel type to Enya visualization type
fn convert_panel_type(panel_type: &str) -> &'static str {
    match panel_type {
        "timeseries" | "graph" => "time_series",
        "stat" | "singlestat" => "stat",
        "gauge" => "gauge",
        "barchart" | "bargauge" => "bar_chart",
        "heatmap" => "heatmap",
        "sparkline" => "sparkline",
        _ => "time_series",
    }
}

/// Convert Grafana time range to Enya preset
fn convert_time_range(from: &str) -> String {
    // Parse "now-Xm", "now-Xh", "now-Xd" format
    let from_lower = from.to_lowercase();

    if let Some(rest) = from_lower.strip_prefix("now-") {
        // Try to parse the duration
        if rest == "5m" {
            return "5m".to_string();
        } else if rest == "15m" {
            return "15m".to_string();
        } else if rest == "30m" {
            return "30m".to_string();
        } else if rest == "1h" {
            return "1h".to_string();
        } else if rest == "6h" {
            return "6h".to_string();
        } else if rest == "24h" || rest == "1d" {
            return "24h".to_string();
        } else if rest == "7d" || rest == "1w" {
            return "7d".to_string();
        }

        // Try to parse numeric values
        if let Some(mins) = rest.strip_suffix('m') {
            if let Ok(m) = mins.parse::<u32>() {
                return match m {
                    0..=7 => "5m",
                    8..=20 => "15m",
                    21..=45 => "30m",
                    _ => "1h",
                }
                .to_string();
            }
        } else if let Some(hours) = rest.strip_suffix('h') {
            if let Ok(h) = hours.parse::<u32>() {
                return match h {
                    0..=1 => "1h",
                    2..=6 => "6h",
                    _ => "24h",
                }
                .to_string();
            }
        } else if let Some(days) = rest.strip_suffix('d') {
            if let Ok(d) = days.parse::<u32>() {
                return if d <= 1 { "24h" } else { "7d" }.to_string();
            }
        }
    }

    // Default to 1h
    "1h".to_string()
}

/// Convert a Grafana panel to an Enya pane
fn convert_panel(panel: &GrafanaPanel, warnings: &mut Vec<String>) -> Option<PaneConfig> {
    // Extract query from targets
    let query = extract_query(panel);

    if query.is_empty() && panel.panel_type != "text" {
        warnings.push(format!(
            "Panel '{}' has no query, using placeholder",
            panel.title
        ));
    }

    let visualization = convert_panel_type(&panel.panel_type);

    // Extract unit from field config
    let unit = extract_unit(panel);

    Some(PaneConfig {
        query: if query.is_empty() {
            format!("# {}", panel.title) // Comment-style placeholder
        } else {
            query
        },
        name: panel.title.clone(),
        description: String::new(), // Grafana panel descriptions not supported yet
        tag: String::new(),
        granularity: "5m".to_string(), // Default, Grafana doesn't have an equivalent
        visualization: visualization.to_string(),
        unit,
    })
}

/// Extract and convert unit from panel field config
fn extract_unit(panel: &GrafanaPanel) -> String {
    panel
        .field_config
        .as_ref()
        .and_then(|fc| fc.defaults.as_ref())
        .and_then(|d| d.unit.as_ref())
        .map(|u| convert_grafana_unit(u))
        .unwrap_or_default()
}

/// Convert Grafana unit identifiers to human-readable suffixes
fn convert_grafana_unit(unit: &str) -> String {
    match unit {
        // Time units
        "s" | "seconds" => "s".to_string(),
        "ms" | "milliseconds" => "ms".to_string(),
        "µs" | "us" | "microseconds" => "µs".to_string(),
        "ns" | "nanoseconds" => "ns".to_string(),
        "m" | "minutes" => "min".to_string(),
        "h" | "hours" => "h".to_string(),
        "d" | "days" => "d".to_string(),

        // Data size
        "bytes" | "decbytes" => "B".to_string(),
        "bits" | "decbits" => "b".to_string(),
        "kbytes" | "deckbytes" => "KB".to_string(),
        "mbytes" | "decmbytes" => "MB".to_string(),
        "gbytes" | "decgbytes" => "GB".to_string(),
        "tbytes" | "dectbytes" => "TB".to_string(),

        // Data rate
        "binBps" | "Bps" => "B/s".to_string(),
        "binbps" | "bps" => "b/s".to_string(),
        "KBs" | "kBs" => "KB/s".to_string(),
        "MBs" | "mBs" => "MB/s".to_string(),
        "GBs" | "gBs" => "GB/s".to_string(),

        // Throughput
        "ops" | "opm" | "ops/s" => "ops".to_string(),
        "reqps" | "rps" => "req/s".to_string(),
        "cps" => "conn/s".to_string(),
        "iops" => "iops".to_string(),
        "wps" => "writes/s".to_string(),
        "rps_read" => "reads/s".to_string(),

        // Percentage
        "percent" | "percentunit" => "%".to_string(),
        "percent0" => "%".to_string(),
        "percent100" => "%".to_string(),

        // Currency
        "currencyUSD" => "$".to_string(),
        "currencyEUR" => "€".to_string(),
        "currencyGBP" => "£".to_string(),

        // Energy
        "watt" | "W" => "W".to_string(),
        "kwatt" | "kW" => "kW".to_string(),
        "mwatt" | "mW" => "mW".to_string(),
        "voltamp" | "VA" => "VA".to_string(),

        // Temperature
        "celsius" => "°C".to_string(),
        "fahrenheit" => "°F".to_string(),
        "kelvin" => "K".to_string(),

        // Frequency
        "hertz" | "Hz" => "Hz".to_string(),
        "mhertz" | "mHz" => "mHz".to_string(),
        "khertz" | "kHz" => "kHz".to_string(),
        "mHertz" | "MHz" => "MHz".to_string(),
        "ghertz" | "GHz" => "GHz".to_string(),

        // Other common units
        "short" | "none" => String::new(), // No unit suffix for these
        "locale" | "string" => String::new(),

        // Pass through unknown units as-is (they might be custom)
        other => other.to_string(),
    }
}

/// Extract the query expression from panel targets
fn extract_query(panel: &GrafanaPanel) -> String {
    for target in &panel.targets {
        // Try expr field (standard Prometheus)
        if let Some(expr) = &target.expr {
            if !expr.is_empty() {
                return expr.clone();
            }
        }
        // Try promQL field (alternative)
        if let Some(prom_ql) = &target.prom_ql {
            if !prom_ql.is_empty() {
                return prom_ql.clone();
            }
        }
        // Try query field (generic)
        if let Some(query) = &target.query {
            if !query.is_empty() {
                return query.clone();
            }
        }
    }
    String::new()
}

/// A panel with its grid position for layout calculation
#[derive(Debug, Clone)]
struct PanelWithPos<'a> {
    #[allow(dead_code)] // Kept for potential future use (e.g., extracting more panel info)
    panel: &'a GrafanaPanel,
    index: usize,
    grid_pos: GridPos,
}

/// Convert Grafana grid layout to i3-style nested containers
fn convert_layout(panels: &[&GrafanaPanel]) -> Option<LayoutConfig> {
    if panels.is_empty() {
        return None;
    }

    // Collect panels with their grid positions
    let mut positioned: Vec<PanelWithPos<'_>> = panels
        .iter()
        .enumerate()
        .filter_map(|(index, panel)| {
            panel.grid_pos.map(|grid_pos| PanelWithPos {
                panel,
                index,
                grid_pos,
            })
        })
        .collect();

    if positioned.is_empty() {
        // No grid positions, fall back to simple tabs
        return Some(LayoutConfig {
            layout_type: LayoutType::Tabs,
            children: (0..panels.len()).map(LayoutNode::Pane).collect(),
            shares: Vec::new(),
        });
    }

    // Sort by Y position, then X position
    positioned.sort_by(|a, b| {
        a.grid_pos
            .y
            .cmp(&b.grid_pos.y)
            .then(a.grid_pos.x.cmp(&b.grid_pos.x))
    });

    // Group panels into rows (panels that share the same Y position)
    let rows = group_into_rows(&positioned);

    if rows.len() == 1 {
        // Single row - horizontal layout
        return Some(create_horizontal_layout(&rows[0]));
    }

    // Multiple rows - vertical layout with horizontal children
    let mut children = Vec::new();
    let mut shares = Vec::new();

    for row in &rows {
        if row.len() == 1 {
            // Single panel in row
            children.push(LayoutNode::Pane(row[0].index));
        } else {
            // Multiple panels in row - horizontal container
            let horizontal = create_horizontal_container(row);
            children.push(LayoutNode::Container(horizontal));
        }
        // Use the height of the first panel in the row as the share
        shares.push(row[0].grid_pos.h as f32);
    }

    Some(LayoutConfig {
        layout_type: LayoutType::Vertical,
        children,
        shares,
    })
}

/// Group panels into rows based on Y position
fn group_into_rows<'a>(panels: &'a [PanelWithPos<'a>]) -> Vec<Vec<&'a PanelWithPos<'a>>> {
    let mut rows: Vec<Vec<&'a PanelWithPos<'a>>> = Vec::new();
    let mut current_row: Vec<&'a PanelWithPos<'a>> = Vec::new();
    let mut current_y: Option<u32> = None;

    for panel in panels {
        match current_y {
            Some(y) if panel.grid_pos.y == y => {
                // Same row
                current_row.push(panel);
            }
            Some(_) => {
                // New row
                if !current_row.is_empty() {
                    rows.push(current_row);
                }
                current_row = vec![panel];
                current_y = Some(panel.grid_pos.y);
            }
            None => {
                // First panel
                current_row.push(panel);
                current_y = Some(panel.grid_pos.y);
            }
        }
    }

    if !current_row.is_empty() {
        rows.push(current_row);
    }

    rows
}

/// Create a horizontal layout from a single row of panels
fn create_horizontal_layout(row: &[&PanelWithPos<'_>]) -> LayoutConfig {
    let children: Vec<LayoutNode> = row.iter().map(|p| LayoutNode::Pane(p.index)).collect();
    let shares: Vec<f32> = row.iter().map(|p| p.grid_pos.w as f32).collect();

    LayoutConfig {
        layout_type: LayoutType::Horizontal,
        children,
        shares,
    }
}

/// Create a horizontal container from a row of panels
fn create_horizontal_container(row: &[&PanelWithPos<'_>]) -> LayoutContainer {
    let children: Vec<LayoutNode> = row.iter().map(|p| LayoutNode::Pane(p.index)).collect();
    let shares: Vec<f32> = row.iter().map(|p| p.grid_pos.w as f32).collect();

    LayoutContainer {
        layout_type: LayoutType::Horizontal,
        children,
        shares,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_DASHBOARD: &str = r#"
    {
        "title": "Test Dashboard",
        "description": "A test dashboard for conversion",
        "time": {
            "from": "now-1h",
            "to": "now"
        },
        "panels": [
            {
                "id": 1,
                "title": "HTTP Requests",
                "type": "timeseries",
                "gridPos": { "x": 0, "y": 0, "w": 12, "h": 8 },
                "targets": [
                    {
                        "expr": "sum(rate(http_requests_total[5m])) by (method)",
                        "legendFormat": "{{method}}"
                    }
                ]
            },
            {
                "id": 2,
                "title": "Error Rate",
                "type": "stat",
                "gridPos": { "x": 12, "y": 0, "w": 12, "h": 8 },
                "targets": [
                    {
                        "expr": "sum(rate(http_errors_total[5m])) / sum(rate(http_requests_total[5m]))"
                    }
                ]
            }
        ]
    }
    "#;

    const COMPLEX_DASHBOARD: &str = r#"
    {
        "title": "Production Dashboard",
        "time": {
            "from": "now-6h",
            "to": "now"
        },
        "templating": {
            "list": [
                {
                    "name": "env",
                    "type": "query",
                    "current": { "value": "prod", "text": "prod" }
                }
            ]
        },
        "panels": [
            {
                "id": 1,
                "title": "Overview",
                "type": "timeseries",
                "gridPos": { "x": 0, "y": 0, "w": 24, "h": 8 },
                "targets": [{ "expr": "up{env=\"$env\"}" }]
            },
            {
                "id": 2,
                "title": "CPU",
                "type": "gauge",
                "gridPos": { "x": 0, "y": 8, "w": 8, "h": 6 },
                "targets": [{ "expr": "avg(cpu_usage)" }]
            },
            {
                "id": 3,
                "title": "Memory",
                "type": "gauge",
                "gridPos": { "x": 8, "y": 8, "w": 8, "h": 6 },
                "targets": [{ "expr": "avg(memory_usage)" }]
            },
            {
                "id": 4,
                "title": "Disk",
                "type": "gauge",
                "gridPos": { "x": 16, "y": 8, "w": 8, "h": 6 },
                "targets": [{ "expr": "avg(disk_usage)" }]
            }
        ]
    }
    "#;

    #[test]
    fn test_parse_simple_dashboard() {
        let dashboard = GrafanaDashboard::from_json(SIMPLE_DASHBOARD).unwrap();
        assert_eq!(dashboard.title, "Test Dashboard");
        assert_eq!(dashboard.panels.len(), 2);
        assert_eq!(dashboard.panels[0].title, "HTTP Requests");
        assert_eq!(dashboard.panels[0].panel_type, "timeseries");
    }

    #[test]
    fn test_convert_simple_dashboard() {
        let dashboard = GrafanaDashboard::from_json(SIMPLE_DASHBOARD).unwrap();
        let result = dashboard.to_workspace().unwrap();

        assert_eq!(result.workspace.workspace.name, "Test Dashboard");
        assert_eq!(result.workspace.time.preset, "1h");
        assert_eq!(result.workspace.panes.len(), 2);

        // Check first pane
        assert_eq!(result.workspace.panes[0].name, "HTTP Requests");
        assert_eq!(
            result.workspace.panes[0].query,
            "sum(rate(http_requests_total[5m])) by (method)"
        );
        assert_eq!(result.workspace.panes[0].visualization, "time_series");

        // Check second pane
        assert_eq!(result.workspace.panes[1].name, "Error Rate");
        assert_eq!(result.workspace.panes[1].visualization, "stat");
    }

    #[test]
    fn test_convert_layout() {
        let dashboard = GrafanaDashboard::from_json(SIMPLE_DASHBOARD).unwrap();
        let result = dashboard.to_workspace().unwrap();

        // Should be a single horizontal row
        let layout = result.workspace.layout.unwrap();
        assert_eq!(layout.layout_type, LayoutType::Horizontal);
        assert_eq!(layout.children.len(), 2);
        assert_eq!(layout.shares, vec![12.0, 12.0]);
    }

    #[test]
    fn test_convert_complex_layout() {
        let dashboard = GrafanaDashboard::from_json(COMPLEX_DASHBOARD).unwrap();
        let result = dashboard.to_workspace().unwrap();

        // Should warn about template variable
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("template variables"))
        );

        // Should be vertical with 2 rows
        let layout = result.workspace.layout.unwrap();
        assert_eq!(layout.layout_type, LayoutType::Vertical);
        assert_eq!(layout.children.len(), 2);

        // First row is a single pane
        assert!(matches!(layout.children[0], LayoutNode::Pane(0)));

        // Second row is a horizontal container with 3 panes
        if let LayoutNode::Container(container) = &layout.children[1] {
            assert_eq!(container.layout_type, LayoutType::Horizontal);
            assert_eq!(container.children.len(), 3);
            assert_eq!(container.shares, vec![8.0, 8.0, 8.0]);
        } else {
            panic!("Expected container for second row");
        }
    }

    #[test]
    fn test_time_range_conversion() {
        assert_eq!(convert_time_range("now-5m"), "5m");
        assert_eq!(convert_time_range("now-15m"), "15m");
        assert_eq!(convert_time_range("now-30m"), "30m");
        assert_eq!(convert_time_range("now-1h"), "1h");
        assert_eq!(convert_time_range("now-6h"), "6h");
        assert_eq!(convert_time_range("now-24h"), "24h");
        assert_eq!(convert_time_range("now-1d"), "24h");
        assert_eq!(convert_time_range("now-7d"), "7d");
        assert_eq!(convert_time_range("now-1w"), "7d");

        // Approximate mappings
        assert_eq!(convert_time_range("now-3m"), "5m");
        assert_eq!(convert_time_range("now-10m"), "15m");
        assert_eq!(convert_time_range("now-2h"), "6h");
        assert_eq!(convert_time_range("now-12h"), "24h");
        assert_eq!(convert_time_range("now-3d"), "7d");
    }

    #[test]
    fn test_panel_type_conversion() {
        assert_eq!(convert_panel_type("timeseries"), "time_series");
        assert_eq!(convert_panel_type("graph"), "time_series");
        assert_eq!(convert_panel_type("stat"), "stat");
        assert_eq!(convert_panel_type("singlestat"), "stat");
        assert_eq!(convert_panel_type("gauge"), "gauge");
        assert_eq!(convert_panel_type("barchart"), "bar_chart");
        assert_eq!(convert_panel_type("bargauge"), "bar_chart");
        assert_eq!(convert_panel_type("heatmap"), "heatmap");
    }

    #[test]
    fn test_roundtrip_to_toml() {
        let dashboard = GrafanaDashboard::from_json(SIMPLE_DASHBOARD).unwrap();
        let result = dashboard.to_workspace().unwrap();
        let toml = result.workspace.to_toml().unwrap();

        // Parse the TOML back
        let parsed = WorkspaceConfig::from_toml(&toml).unwrap();
        assert_eq!(parsed.workspace.name, "Test Dashboard");
        assert_eq!(parsed.panes.len(), 2);
    }

    #[test]
    fn test_unsupported_panel_warning() {
        let json = r#"
        {
            "title": "Mixed Dashboard",
            "panels": [
                {
                    "id": 1,
                    "title": "Good Panel",
                    "type": "timeseries",
                    "gridPos": { "x": 0, "y": 0, "w": 12, "h": 8 },
                    "targets": [{ "expr": "up" }]
                },
                {
                    "id": 2,
                    "title": "Logs Panel",
                    "type": "logs",
                    "gridPos": { "x": 12, "y": 0, "w": 12, "h": 8 },
                    "targets": [{ "expr": "{job=\"app\"}" }]
                }
            ]
        }
        "#;

        let dashboard = GrafanaDashboard::from_json(json).unwrap();
        let result = dashboard.to_workspace().unwrap();

        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("unsupported type 'logs'"))
        );
        assert_eq!(result.workspace.panes.len(), 1);
    }

    #[test]
    fn test_collapsed_row_panels() {
        let json = r#"
        {
            "title": "Dashboard with Row",
            "panels": [
                {
                    "id": 1,
                    "title": "Row 1",
                    "type": "row",
                    "collapsed": true,
                    "panels": [
                        {
                            "id": 2,
                            "title": "Nested Panel",
                            "type": "stat",
                            "targets": [{ "expr": "up" }]
                        }
                    ]
                }
            ]
        }
        "#;

        let dashboard = GrafanaDashboard::from_json(json).unwrap();
        let result = dashboard.to_workspace().unwrap();

        // Should extract the nested panel
        assert_eq!(result.workspace.panes.len(), 1);
        assert_eq!(result.workspace.panes[0].name, "Nested Panel");
    }

    #[test]
    fn test_example_dashboard_conversion() {
        // This tests the example dashboard in examples/grafana-dashboard.json
        let json = r#"
        {
            "title": "Production API Dashboard",
            "description": "Overview of production API health and performance",
            "time": { "from": "now-1h", "to": "now" },
            "templating": {
                "list": [{ "name": "env", "type": "custom", "current": { "value": "prod" } }]
            },
            "panels": [
                {
                    "id": 1,
                    "title": "Request Rate",
                    "type": "timeseries",
                    "gridPos": { "x": 0, "y": 0, "w": 12, "h": 8 },
                    "targets": [{ "expr": "sum(rate(http_requests_total{env=\"prod\"}[5m])) by (method)" }]
                },
                {
                    "id": 2,
                    "title": "Error Rate",
                    "type": "stat",
                    "gridPos": { "x": 12, "y": 0, "w": 6, "h": 4 },
                    "targets": [{ "expr": "sum(rate(http_errors_total[5m])) / sum(rate(http_requests_total[5m])) * 100" }]
                },
                {
                    "id": 3,
                    "title": "Active Connections",
                    "type": "gauge",
                    "gridPos": { "x": 18, "y": 0, "w": 6, "h": 4 },
                    "targets": [{ "expr": "sum(db_connections_active)" }]
                },
                {
                    "id": 4,
                    "title": "CPU Usage",
                    "type": "timeseries",
                    "gridPos": { "x": 0, "y": 8, "w": 12, "h": 8 },
                    "targets": [{ "expr": "avg(rate(process_cpu_seconds_total[5m])) by (service) * 100" }]
                },
                {
                    "id": 5,
                    "title": "Latency Heatmap",
                    "type": "heatmap",
                    "gridPos": { "x": 12, "y": 8, "w": 12, "h": 8 },
                    "targets": [{ "expr": "sum(rate(http_request_duration_seconds_bucket[5m])) by (le)" }]
                }
            ]
        }
        "#;

        let dashboard = GrafanaDashboard::from_json(json).unwrap();
        let result = dashboard.to_workspace().unwrap();

        // Check conversion
        assert_eq!(result.workspace.workspace.name, "Production API Dashboard");
        assert_eq!(
            result.workspace.workspace.description,
            "Overview of production API health and performance"
        );
        assert_eq!(result.workspace.time.preset, "1h");
        assert_eq!(result.workspace.panes.len(), 5);

        // Check pane visualizations
        assert_eq!(result.workspace.panes[0].visualization, "time_series");
        assert_eq!(result.workspace.panes[1].visualization, "stat");
        assert_eq!(result.workspace.panes[2].visualization, "gauge");
        assert_eq!(result.workspace.panes[3].visualization, "time_series");
        assert_eq!(result.workspace.panes[4].visualization, "heatmap");

        // Check layout structure
        let layout = result.workspace.layout.as_ref().unwrap();
        assert_eq!(layout.layout_type, LayoutType::Vertical);
        // Row 1: Request Rate (y=0), Error Rate (y=0), Active Connections (y=0)
        // Row 2: CPU Usage (y=8), Latency Heatmap (y=8)
        assert_eq!(layout.children.len(), 2); // 2 rows based on Y positions

        // Should warn about template variable
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("template variables"))
        );

        // Print the generated TOML for manual inspection
        let toml = result.workspace.to_toml().unwrap();
        println!("\n=== Generated TOML ===\n{toml}");
    }
}
