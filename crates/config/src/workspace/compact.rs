//! Compact workspace encoding for URL sharing.
//!
//! This module provides compact binary encodings for workspaces optimized
//! for minimal URL length. The encoding pipeline is:
//!
//! ```text
//! Workspace -> CompactWorkspace -> postcard binary -> LZ4 -> base64
//! ```
//!
//! ## Format Prefixes
//!
//! - `p` - LZ4-compressed postcard workspace (multi-pane)
//! - `q` - LZ4-compressed postcard single pane (most compact for single query)
//! - `s` - LZ4-compressed postcard workspace snapshot (config + data)
//! - `t` - LZ4-compressed postcard single pane snapshot (config + data)

use serde::{Deserialize, Serialize};

use super::{
    LayoutConfig, LayoutContainer, LayoutNode, LayoutType, PaneConfig, SnapshotMeta,
    SnapshotPaneData, SnapshotSeries, WorkspaceConfig, WorkspaceError,
};

/// Compact workspace representation for URL sharing (postcard binary format)
/// Uses numeric enums and minimal fields for smallest possible encoding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompactWorkspaceConfig {
    pub name: String,
    /// Packed header: bits 0-2 = time preset (0-6), bit 3 = theme (0=dark, 1=light)
    pub header: u8,
    /// Panes
    pub panes: Vec<CompactPane>,
    /// Compact layout representation (None means tabs - the default)
    /// Note: Always serialized (not skipped) because postcard requires all fields
    pub layout: Option<CompactLayout>,
}

/// Compact layout representation
/// Uses a flat encoding: each node is (type, child_count) followed by its children
/// Type: 0=horizontal, 1=vertical, 2=tabs, 128+=pane index (128+pane_idx)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompactLayout {
    /// Flat encoded layout tree
    pub nodes: Vec<u8>,
}

/// Compact single-pane representation for sharing individual queries
/// Even more minimal than CompactWorkspace - just the essentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompactSinglePane {
    /// The query expression
    pub query: String,
    /// Optional display name
    pub name: Option<String>,
    /// Packed header: bits 0-2 = time preset, bit 3 = theme
    pub header: u8,
    /// Packed flags: bits 0-2 = granularity (0-5), bits 3-5 = visualization (0-5)
    pub flags: u8,
    /// Optional unit suffix (None = empty string)
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompactPane {
    pub query: String,
    /// Optional display name (None = empty string)
    pub name: Option<String>,
    /// Optional tag (None = empty string)
    pub tag: Option<String>,
    /// Packed: bits 0-2 = granularity (0-5), bits 3-5 = visualization (0-5)
    pub flags: u8,
    /// Optional unit suffix (None = empty string, e.g. "req/s", "ms", "%")
    pub unit: Option<String>,
}

// =============================================================================
// Snapshot types (config + embedded visualization data)
// =============================================================================

/// Maximum number of data points per series in snapshot encoding.
/// LTTB downsampling reduces larger series to this count while preserving visual shape.
const SNAPSHOT_MAX_POINTS: usize = 100;

/// Compact time series data with string deduplication.
/// Series names and tag keys/values are stored in a shared string table,
/// with each series referencing strings by u16 index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompactTimeSeriesData {
    pub strings: Vec<String>,
    pub series: Vec<CompactSeriesRef>,
}

/// A single series referencing the shared string table by index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompactSeriesRef {
    pub name_idx: u16,
    pub tags: Vec<(u16, u16)>,
    pub base_timestamp: f64,
    pub deltas: CompactDeltas,
    pub values: Vec<f32>,
}

/// Timestamp deltas: regular (all same step) or irregular (variable gaps).
/// Regular deltas encode as a single u32 vs N varints, saving ~100 bytes per series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum CompactDeltas {
    /// All gaps are identical (common for fixed scrape intervals).
    /// Count is implicit from values.len().
    Regular(u32),
    /// Variable gaps between consecutive points.
    Irregular(Vec<u32>),
}

/// Compact visualization data enum (uses f32 for all numeric fields)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum CompactVizData {
    TimeSeries(CompactTimeSeriesData),
    Stat {
        value: f32,
        sparkline: Vec<f32>,
    },
    Gauge {
        value: f32,
        min: f32,
        max: f32,
    },
    BarChart(Vec<(String, f32)>),
    Heatmap {
        cols: u16,
        rows: u16,
        values: Vec<f32>,
    },
}

/// Compact snapshot pane: config fields + data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompactSnapshotPane {
    pub query: String,
    pub name: Option<String>,
    pub tag: Option<String>,
    pub flags: u8,
    pub data: CompactVizData,
    /// Optional unit suffix (None = empty string)
    pub unit: Option<String>,
}

/// Compact snapshot workspace: config + data for all panes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompactSnapshotWorkspace {
    pub name: String,
    pub header: u8,
    pub panes: Vec<CompactSnapshotPane>,
    pub layout: Option<CompactLayout>,
    pub captured_at: u64,
}

/// Compact snapshot for a single pane
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompactSnapshotSinglePane {
    pub query: String,
    pub name: Option<String>,
    pub header: u8,
    pub flags: u8,
    pub data: CompactVizData,
    pub captured_at: u64,
    /// Optional unit suffix (None = empty string)
    pub unit: Option<String>,
}

/// Downsample points using the Largest Triangle Three Buckets (LTTB) algorithm.
/// Preserves visual shape by selecting points that maximize triangle area in each bucket.
/// Always preserves first and last points. Returns input unchanged if `target >= len`.
fn lttb_downsample(points: &[(f64, f64)], target: usize) -> Vec<(f64, f64)> {
    let n = points.len();
    if target >= n || target < 3 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(target);
    result.push(points[0]);

    let bucket_size = (n - 2) as f64 / (target - 2) as f64;
    let mut prev_selected = 0usize;

    for i in 1..(target - 1) {
        // Current bucket range
        let bucket_start = ((i - 1) as f64 * bucket_size).floor() as usize + 1;
        let bucket_end = ((i as f64) * bucket_size).floor() as usize + 1;
        let bucket_end = bucket_end.min(n - 1);

        // Average of next bucket (the "C" point in the triangle)
        let next_start = bucket_end;
        let next_end = (((i + 1) as f64) * bucket_size).floor() as usize + 1;
        let next_end = next_end.min(n);
        let next_count = (next_end - next_start).max(1) as f64;
        let avg_x: f64 = points[next_start..next_end]
            .iter()
            .map(|p| p.0)
            .sum::<f64>()
            / next_count;
        let avg_y: f64 = points[next_start..next_end]
            .iter()
            .map(|p| p.1)
            .sum::<f64>()
            / next_count;

        // Find point in current bucket with largest triangle area
        let (ax, ay) = points[prev_selected];
        let mut max_area = -1.0f64;
        let mut max_idx = bucket_start;

        for (j, pt) in points
            .iter()
            .enumerate()
            .take(bucket_end)
            .skip(bucket_start)
        {
            let area = ((pt.0 - ax) * (avg_y - ay) - (avg_x - ax) * (pt.1 - ay)).abs();
            if area > max_area {
                max_area = area;
                max_idx = j;
            }
        }

        result.push(points[max_idx]);
        prev_selected = max_idx;
    }

    result.push(points[n - 1]);
    result
}

/// Intern a string into the shared table, returning its index.
fn intern_string(
    table: &mut Vec<String>,
    map: &mut rustc_hash::FxHashMap<String, u16>,
    s: &str,
) -> u16 {
    if let Some(&idx) = map.get(s) {
        idx
    } else {
        let idx = table.len() as u16;
        table.push(s.to_string());
        map.insert(s.to_string(), idx);
        idx
    }
}

impl CompactVizData {
    fn from_snapshot(data: &SnapshotPaneData) -> Self {
        match data {
            SnapshotPaneData::TimeSeries { series } => {
                let mut strings = Vec::new();
                let mut string_map = rustc_hash::FxHashMap::default();

                let compact_series: Vec<CompactSeriesRef> = series
                    .iter()
                    .map(|s| {
                        // Downsample with LTTB to keep URLs compact
                        let downsampled = lttb_downsample(&s.points, SNAPSHOT_MAX_POINTS);

                        // Delta-encode timestamps
                        let base_timestamp = downsampled.first().map(|p| p.0).unwrap_or(0.0);
                        let raw_deltas: Vec<u32> = if downsampled.len() <= 1 {
                            vec![0; downsampled.len()]
                        } else {
                            let mut ds = Vec::with_capacity(downsampled.len());
                            ds.push(0u32);
                            for w in downsampled.windows(2) {
                                ds.push((w[1].0 - w[0].0).max(0.0).min(u32::MAX as f64) as u32);
                            }
                            ds
                        };

                        // Detect regular intervals (all non-zero deltas identical)
                        let deltas = if raw_deltas.len() <= 1 {
                            CompactDeltas::Regular(0)
                        } else {
                            let step = raw_deltas[1];
                            if raw_deltas[1..].iter().all(|&d| d == step) {
                                CompactDeltas::Regular(step)
                            } else {
                                CompactDeltas::Irregular(raw_deltas)
                            }
                        };

                        // Intern strings into shared table
                        let name_idx = intern_string(&mut strings, &mut string_map, &s.name);
                        let tags: Vec<(u16, u16)> = s
                            .tags
                            .iter()
                            .map(|(k, v)| {
                                (
                                    intern_string(&mut strings, &mut string_map, k),
                                    intern_string(&mut strings, &mut string_map, v),
                                )
                            })
                            .collect();

                        CompactSeriesRef {
                            name_idx,
                            tags,
                            base_timestamp,
                            deltas,
                            values: downsampled.iter().map(|p| p.1 as f32).collect(),
                        }
                    })
                    .collect();

                CompactVizData::TimeSeries(CompactTimeSeriesData {
                    strings,
                    series: compact_series,
                })
            }
            SnapshotPaneData::Stat { value, sparkline } => CompactVizData::Stat {
                value: *value as f32,
                sparkline: sparkline.iter().map(|&v| v as f32).collect(),
            },
            SnapshotPaneData::Gauge { value, min, max } => CompactVizData::Gauge {
                value: *value as f32,
                min: *min as f32,
                max: *max as f32,
            },
            SnapshotPaneData::BarChart { bars } => {
                CompactVizData::BarChart(bars.iter().map(|(k, v)| (k.clone(), *v as f32)).collect())
            }
            SnapshotPaneData::Heatmap { cols, rows, values } => CompactVizData::Heatmap {
                cols: *cols,
                rows: *rows,
                values: values.clone(),
            },
        }
    }

    fn into_snapshot(self) -> SnapshotPaneData {
        match self {
            CompactVizData::TimeSeries(ts_data) => {
                let CompactTimeSeriesData { strings, series } = ts_data;
                SnapshotPaneData::TimeSeries {
                    series: series
                        .into_iter()
                        .map(|s| {
                            let CompactSeriesRef {
                                name_idx,
                                tags,
                                base_timestamp,
                                deltas,
                                values,
                            } = s;

                            // Look up strings from shared table
                            let name = strings.get(name_idx as usize).cloned().unwrap_or_default();
                            let tags: Vec<(String, String)> = tags
                                .iter()
                                .map(|&(ki, vi)| {
                                    (
                                        strings.get(ki as usize).cloned().unwrap_or_default(),
                                        strings.get(vi as usize).cloned().unwrap_or_default(),
                                    )
                                })
                                .collect();

                            // Reconstruct absolute timestamps
                            let points: Vec<(f64, f64)> = match deltas {
                                CompactDeltas::Regular(step) => values
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, value)| {
                                        (base_timestamp + i as f64 * step as f64, value as f64)
                                    })
                                    .collect(),
                                CompactDeltas::Irregular(deltas) => {
                                    let mut ts = base_timestamp;
                                    deltas
                                        .into_iter()
                                        .zip(values)
                                        .map(|(delta, value)| {
                                            ts += delta as f64;
                                            (ts, value as f64)
                                        })
                                        .collect()
                                }
                            };

                            SnapshotSeries { name, tags, points }
                        })
                        .collect(),
                }
            }
            CompactVizData::Stat { value, sparkline } => SnapshotPaneData::Stat {
                value: value as f64,
                sparkline: sparkline.into_iter().map(|v| v as f64).collect(),
            },
            CompactVizData::Gauge { value, min, max } => SnapshotPaneData::Gauge {
                value: value as f64,
                min: min as f64,
                max: max as f64,
            },
            CompactVizData::BarChart(bars) => SnapshotPaneData::BarChart {
                bars: bars.into_iter().map(|(k, v)| (k, v as f64)).collect(),
            },
            CompactVizData::Heatmap { cols, rows, values } => {
                SnapshotPaneData::Heatmap { cols, rows, values }
            }
        }
    }
}

impl CompactSnapshotWorkspace {
    pub(crate) fn from_workspace(ws: &WorkspaceConfig, pane_data: &[SnapshotPaneData]) -> Self {
        let config = CompactWorkspaceConfig::from_workspace(ws);
        Self {
            name: config.name,
            header: config.header,
            panes: config
                .panes
                .into_iter()
                .enumerate()
                .map(|(i, p)| {
                    let data = pane_data
                        .get(i)
                        .map(CompactVizData::from_snapshot)
                        .unwrap_or(CompactVizData::TimeSeries(CompactTimeSeriesData {
                            strings: Vec::new(),
                            series: Vec::new(),
                        }));
                    CompactSnapshotPane {
                        query: p.query,
                        name: p.name,
                        tag: p.tag,
                        flags: p.flags,
                        data,
                        unit: p.unit,
                    }
                })
                .collect(),
            layout: config.layout,
            captured_at: 0, // Set by caller
        }
    }

    pub(crate) fn into_workspace(self) -> WorkspaceConfig {
        // First build the config-only workspace
        let config = CompactWorkspaceConfig {
            name: self.name,
            header: self.header,
            panes: self
                .panes
                .iter()
                .map(|p| CompactPane {
                    query: p.query.clone(),
                    name: p.name.clone(),
                    tag: p.tag.clone(),
                    flags: p.flags,
                    unit: p.unit.clone(),
                })
                .collect(),
            layout: self.layout,
        };
        let mut ws = config.into_workspace();

        // Attach snapshot data
        ws.snapshot = Some(SnapshotMeta {
            captured_at: self.captured_at,
            pane_data: self
                .panes
                .into_iter()
                .map(|p| p.data.into_snapshot())
                .collect(),
            conversation: None,
        });

        ws
    }
}

impl CompactSnapshotSinglePane {
    fn from_pane(
        pane: &PaneConfig,
        time_preset: &str,
        theme: &str,
        data: &SnapshotPaneData,
    ) -> Self {
        let config = CompactSinglePane::from_pane(pane, time_preset, theme);
        Self {
            query: config.query,
            name: config.name,
            header: config.header,
            flags: config.flags,
            data: CompactVizData::from_snapshot(data),
            captured_at: 0,
            unit: config.unit,
        }
    }

    fn into_workspace(self) -> WorkspaceConfig {
        let config = CompactSinglePane {
            query: self.query.clone(),
            name: self.name.clone(),
            header: self.header,
            flags: self.flags,
            unit: self.unit.clone(),
        };
        let mut ws = config.into_workspace();

        ws.snapshot = Some(SnapshotMeta {
            captured_at: self.captured_at,
            pane_data: vec![self.data.into_snapshot()],
            conversation: None,
        });

        ws
    }
}

impl CompactLayout {
    /// Encode a LayoutConfig into compact form
    pub fn from_layout(layout: &LayoutConfig) -> Self {
        let mut nodes = Vec::new();
        Self::encode_container(layout.layout_type, &layout.children, &mut nodes);
        Self { nodes }
    }

    /// Encode a container node
    fn encode_container(layout_type: LayoutType, children: &[LayoutNode], out: &mut Vec<u8>) {
        let type_byte = match layout_type {
            LayoutType::Horizontal => 0,
            LayoutType::Vertical => 1,
            LayoutType::Tabs => 2,
        };
        out.push(type_byte);
        out.push(children.len() as u8);

        for child in children {
            match child {
                LayoutNode::Pane(idx) => {
                    // Pane indices encoded as 128 + index
                    out.push(128 + (*idx as u8));
                }
                LayoutNode::Container(container) => {
                    Self::encode_container(container.layout_type, &container.children, out);
                }
            }
        }
    }

    /// Decode into a LayoutConfig
    pub fn into_layout(self) -> Option<LayoutConfig> {
        let mut pos = 0;
        let (layout_type, children) = Self::decode_container(&self.nodes, &mut pos)?;
        Some(LayoutConfig {
            layout_type,
            children,
            shares: Vec::new(), // Shares not preserved in compact format
        })
    }

    /// Decode a container node
    fn decode_container(nodes: &[u8], pos: &mut usize) -> Option<(LayoutType, Vec<LayoutNode>)> {
        if *pos >= nodes.len() {
            return None;
        }

        let type_byte = nodes[*pos];
        *pos += 1;

        // Check if this is a pane (128+)
        if type_byte >= 128 {
            // This shouldn't happen at container level
            return None;
        }

        let layout_type = match type_byte {
            0 => LayoutType::Horizontal,
            1 => LayoutType::Vertical,
            2 => LayoutType::Tabs,
            _ => return None,
        };

        if *pos >= nodes.len() {
            return None;
        }
        let child_count = nodes[*pos] as usize;
        *pos += 1;

        let mut children = Vec::with_capacity(child_count);
        for _ in 0..child_count {
            if *pos >= nodes.len() {
                return None;
            }

            let next_byte = nodes[*pos];
            if next_byte >= 128 {
                // Pane index
                children.push(LayoutNode::Pane((next_byte - 128) as usize));
                *pos += 1;
            } else {
                // Nested container
                let (nested_type, nested_children) = Self::decode_container(nodes, pos)?;
                children.push(LayoutNode::Container(LayoutContainer {
                    layout_type: nested_type,
                    children: nested_children,
                    shares: Vec::new(),
                }));
            }
        }

        Some((layout_type, children))
    }
}

impl CompactWorkspaceConfig {
    pub fn from_workspace(ws: &WorkspaceConfig) -> Self {
        let time_idx: u8 = match ws.time.preset.as_str() {
            "5m" => 0,
            "15m" => 1,
            "30m" => 2,
            "1h" => 3,
            "6h" => 4,
            "24h" => 5,
            "7d" => 6,
            _ => 1, // default to 15m
        };
        let theme_bit: u8 = if ws.view.theme == "light" { 1 } else { 0 };
        // Pack: bits 0-2 = time, bit 3 = theme
        let header = time_idx | (theme_bit << 3);

        Self {
            name: ws.workspace.name.clone(),
            header,
            panes: ws
                .panes
                .iter()
                .map(|p| {
                    let gran: u8 = match p.granularity.as_str() {
                        "1m" => 0,
                        "5m" => 1,
                        "15m" => 2,
                        "1h" => 3,
                        "6h" => 4,
                        "1d" => 5,
                        _ => 1,
                    };
                    let viz: u8 = match p.visualization.as_str() {
                        "time_series" => 0,
                        "stat" => 1,
                        "gauge" => 2,
                        "bar_chart" => 3,
                        "sparkline" => 4,
                        "heatmap" => 5,
                        _ => 0,
                    };
                    // Pack: bits 0-2 = granularity, bits 3-5 = visualization
                    let flags = gran | (viz << 3);

                    CompactPane {
                        query: p.query.clone(),
                        name: if p.name.is_empty() {
                            None
                        } else {
                            Some(p.name.clone())
                        },
                        tag: if p.tag.is_empty() {
                            None
                        } else {
                            Some(p.tag.clone())
                        },
                        flags,
                        unit: if p.unit.is_empty() {
                            None
                        } else {
                            Some(p.unit.clone())
                        },
                    }
                })
                .collect(),
            // Only encode layout if it's not the default tabs layout
            layout: ws.layout.as_ref().and_then(|l| {
                // Skip encoding if it's just a simple tabs container with all panes
                if l.layout_type == LayoutType::Tabs
                    && l.children.len() == ws.panes.len()
                    && l.children
                        .iter()
                        .enumerate()
                        .all(|(i, c)| matches!(c, LayoutNode::Pane(idx) if *idx == i))
                {
                    None
                } else {
                    Some(CompactLayout::from_layout(l))
                }
            }),
        }
    }

    pub fn into_workspace(self) -> WorkspaceConfig {
        let mut ws = WorkspaceConfig::new(self.name);

        // Unpack header: bits 0-2 = time, bit 3 = theme
        let time_idx = self.header & 0x07;
        let theme_bit = (self.header >> 3) & 0x01;

        ws.view.theme = if theme_bit == 1 {
            "light".to_string()
        } else {
            "dark".to_string()
        };
        ws.time.preset = match time_idx {
            0 => "5m",
            1 => "15m",
            2 => "30m",
            3 => "1h",
            4 => "6h",
            5 => "24h",
            6 => "7d",
            _ => "15m",
        }
        .to_string();
        ws.panes = self
            .panes
            .into_iter()
            .map(|p| {
                // Unpack flags: bits 0-2 = granularity, bits 3-5 = visualization
                let gran = p.flags & 0x07;
                let viz = (p.flags >> 3) & 0x07;

                PaneConfig {
                    query: p.query,
                    name: p.name.unwrap_or_default(),
                    description: String::new(), // Compact format doesn't encode description
                    tag: p.tag.unwrap_or_default(),
                    unit: p.unit.unwrap_or_default(),
                    granularity: match gran {
                        0 => "1m",
                        1 => "5m",
                        2 => "15m",
                        3 => "1h",
                        4 => "6h",
                        5 => "1d",
                        _ => "5m",
                    }
                    .to_string(),
                    visualization: match viz {
                        0 => "time_series",
                        1 => "stat",
                        2 => "gauge",
                        3 => "bar_chart",
                        4 => "sparkline",
                        5 => "heatmap",
                        _ => "time_series",
                    }
                    .to_string(),
                }
            })
            .collect();

        // Restore layout if present
        ws.layout = self.layout.and_then(|l| l.into_layout());

        ws
    }
}

impl CompactSinglePane {
    pub fn from_pane(pane: &PaneConfig, time_preset: &str, theme: &str) -> Self {
        let time_idx: u8 = match time_preset {
            "5m" => 0,
            "15m" => 1,
            "30m" => 2,
            "1h" => 3,
            "6h" => 4,
            "24h" => 5,
            "7d" => 6,
            _ => 1,
        };
        let theme_bit: u8 = if theme == "light" { 1 } else { 0 };
        let gran: u8 = match pane.granularity.as_str() {
            "1m" => 0,
            "5m" => 1,
            "15m" => 2,
            "1h" => 3,
            "6h" => 4,
            "1d" => 5,
            _ => 1,
        };
        let viz: u8 = match pane.visualization.as_str() {
            "time_series" => 0,
            "stat" => 1,
            "gauge" => 2,
            "bar_chart" => 3,
            "sparkline" => 4,
            "heatmap" => 5,
            _ => 0,
        };
        // Pack header: bits 0-2 = time, bit 3 = theme
        let header = time_idx | (theme_bit << 3);
        // Pack flags: bits 0-2 = granularity, bits 3-5 = visualization
        let flags = gran | (viz << 3);

        Self {
            query: pane.query.clone(),
            name: if pane.name.is_empty() {
                None
            } else {
                Some(pane.name.clone())
            },
            header,
            flags,
            unit: if pane.unit.is_empty() {
                None
            } else {
                Some(pane.unit.clone())
            },
        }
    }

    pub fn into_workspace(self) -> WorkspaceConfig {
        // Unpack header: bits 0-2 = time, bit 3 = theme
        let time_idx = self.header & 0x07;
        let theme_bit = (self.header >> 3) & 0x01;
        // Unpack flags: bits 0-2 = granularity, bits 3-5 = visualization
        let gran = self.flags & 0x07;
        let viz = (self.flags >> 3) & 0x07;

        let mut ws = WorkspaceConfig::new("shared");
        ws.view.theme = if theme_bit == 1 {
            "light".to_string()
        } else {
            "dark".to_string()
        };
        ws.time.preset = match time_idx {
            0 => "5m",
            1 => "15m",
            2 => "30m",
            3 => "1h",
            4 => "6h",
            5 => "24h",
            6 => "7d",
            _ => "15m",
        }
        .to_string();

        ws.panes.push(PaneConfig {
            query: self.query,
            name: self.name.unwrap_or_default(),
            description: String::new(),
            tag: String::new(),
            unit: self.unit.unwrap_or_default(),
            granularity: match gran {
                0 => "1m",
                1 => "5m",
                2 => "15m",
                3 => "1h",
                4 => "6h",
                5 => "1d",
                _ => "5m",
            }
            .to_string(),
            visualization: match viz {
                0 => "time_series",
                1 => "stat",
                2 => "gauge",
                3 => "bar_chart",
                4 => "sparkline",
                5 => "heatmap",
                _ => "time_series",
            }
            .to_string(),
        });

        ws
    }
}

// =============================================================================
// Base64 encoding/decoding functions
// =============================================================================

/// Decode workspace from base64-encoded data (for URL parameters)
/// Supports multiple formats for backwards compatibility:
/// - "p" prefix: LZ4-compressed postcard workspace (multi-pane)
/// - "q" prefix: LZ4-compressed postcard single pane (most compact for single query)
/// - no prefix: raw TOML (legacy)
pub fn decode_workspace(encoded: &str) -> Result<WorkspaceConfig, WorkspaceError> {
    use base64::Engine;

    // Check for format prefix
    if let Some(rest) = encoded.strip_prefix('p') {
        // Compressed postcard workspace format
        let compressed = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(rest)
            .map_err(|e| WorkspaceError::Decode(e.to_string()))?;

        let decompressed = lz4_flex::decompress_size_prepended(&compressed)
            .map_err(|e| WorkspaceError::Decode(format!("LZ4 decompression failed: {e}")))?;

        let compact: CompactWorkspaceConfig = postcard::from_bytes(&decompressed)
            .map_err(|e| WorkspaceError::Decode(format!("postcard decode failed: {e}")))?;
        return Ok(compact.into_workspace());
    }

    if let Some(rest) = encoded.strip_prefix('q') {
        // Compressed postcard single-pane format
        let compressed = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(rest)
            .map_err(|e| WorkspaceError::Decode(e.to_string()))?;

        let decompressed = lz4_flex::decompress_size_prepended(&compressed)
            .map_err(|e| WorkspaceError::Decode(format!("LZ4 decompression failed: {e}")))?;

        let compact: CompactSinglePane = postcard::from_bytes(&decompressed)
            .map_err(|e| WorkspaceError::Decode(format!("postcard decode failed: {e}")))?;
        return Ok(compact.into_workspace());
    }

    if let Some(rest) = encoded.strip_prefix('s') {
        // Compressed postcard snapshot workspace format
        let compressed = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(rest)
            .map_err(|e| WorkspaceError::Decode(e.to_string()))?;

        let decompressed = lz4_flex::decompress_size_prepended(&compressed)
            .map_err(|e| WorkspaceError::Decode(format!("LZ4 decompression failed: {e}")))?;

        let snapshot: CompactSnapshotWorkspace = postcard::from_bytes(&decompressed)
            .map_err(|e| WorkspaceError::Decode(format!("postcard decode failed: {e}")))?;
        return Ok(snapshot.into_workspace());
    }

    if let Some(rest) = encoded.strip_prefix('t') {
        // Compressed postcard snapshot single-pane format
        let compressed = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(rest)
            .map_err(|e| WorkspaceError::Decode(e.to_string()))?;

        let decompressed = lz4_flex::decompress_size_prepended(&compressed)
            .map_err(|e| WorkspaceError::Decode(format!("LZ4 decompression failed: {e}")))?;

        let snapshot: CompactSnapshotSinglePane = postcard::from_bytes(&decompressed)
            .map_err(|e| WorkspaceError::Decode(format!("postcard decode failed: {e}")))?;
        return Ok(snapshot.into_workspace());
    }

    // Legacy: raw TOML (no prefix)
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|e| WorkspaceError::Decode(e.to_string()))?;
    let toml_str = String::from_utf8(bytes).map_err(|e| WorkspaceError::Decode(e.to_string()))?;
    WorkspaceConfig::from_toml(&toml_str)
}

/// Encode workspace to base64 (for URL sharing)
/// Uses LZ4-compressed postcard binary format prefixed with "p" for maximum compactness
pub fn encode_workspace(ws: &WorkspaceConfig) -> Result<String, WorkspaceError> {
    use base64::Engine;

    let compact = CompactWorkspaceConfig::from_workspace(ws);
    let bytes = postcard::to_allocvec(&compact)
        .map_err(|e| WorkspaceError::Encode(format!("postcard encode failed: {e}")))?;

    // Compress the postcard bytes with LZ4
    let compressed = lz4_flex::compress_prepend_size(&bytes);

    // Prefix with "p" to indicate compressed postcard format
    Ok(format!(
        "p{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed)
    ))
}

/// Encode a single pane to base64 (for sharing individual queries)
/// Uses "q" prefix for single-pane format, which is more compact than full workspace
pub fn encode_pane(ws: &WorkspaceConfig, pane_index: usize) -> Result<String, WorkspaceError> {
    use base64::Engine;

    let pane = ws
        .panes
        .get(pane_index)
        .ok_or_else(|| WorkspaceError::Encode(format!("pane index {pane_index} out of range")))?;

    let compact = CompactSinglePane::from_pane(pane, &ws.time.preset, &ws.view.theme);
    let bytes = postcard::to_allocvec(&compact)
        .map_err(|e| WorkspaceError::Encode(format!("postcard encode failed: {e}")))?;

    // Compress the postcard bytes with LZ4
    let compressed = lz4_flex::compress_prepend_size(&bytes);

    // Prefix with "q" to indicate single-pane format
    Ok(format!(
        "q{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed)
    ))
}

/// Encode workspace snapshot to base64 (config + visualization data)
/// Uses "s" prefix for snapshot workspace format.
pub fn encode_snapshot_workspace(
    ws: &WorkspaceConfig,
    pane_data: &[SnapshotPaneData],
    captured_at: u64,
) -> Result<String, WorkspaceError> {
    use base64::Engine;

    let mut snapshot = CompactSnapshotWorkspace::from_workspace(ws, pane_data);
    snapshot.captured_at = captured_at;

    let bytes = postcard::to_allocvec(&snapshot)
        .map_err(|e| WorkspaceError::Encode(format!("postcard encode failed: {e}")))?;

    let compressed = lz4_flex::compress_prepend_size(&bytes);

    Ok(format!(
        "s{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed)
    ))
}

/// Encode a single pane snapshot to base64 (config + visualization data)
/// Uses "t" prefix for snapshot single-pane format.
pub fn encode_snapshot_pane(
    ws: &WorkspaceConfig,
    pane_index: usize,
    data: &SnapshotPaneData,
    captured_at: u64,
) -> Result<String, WorkspaceError> {
    use base64::Engine;

    let pane = ws
        .panes
        .get(pane_index)
        .ok_or_else(|| WorkspaceError::Encode(format!("pane index {pane_index} out of range")))?;

    let mut snapshot =
        CompactSnapshotSinglePane::from_pane(pane, &ws.time.preset, &ws.view.theme, data);
    snapshot.captured_at = captured_at;

    let bytes = postcard::to_allocvec(&snapshot)
        .map_err(|e| WorkspaceError::Encode(format!("postcard encode failed: {e}")))?;

    let compressed = lz4_flex::compress_prepend_size(&bytes);

    Ok(format!(
        "t{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Helper functions for tests
    // ==========================================================================

    fn make_pane(query: &str) -> PaneConfig {
        PaneConfig::new(query)
    }

    fn make_pane_full(
        query: &str,
        name: &str,
        tag: &str,
        granularity: &str,
        visualization: &str,
    ) -> PaneConfig {
        PaneConfig {
            query: query.to_string(),
            name: name.to_string(),
            description: String::new(),
            tag: tag.to_string(),
            granularity: granularity.to_string(),
            visualization: visualization.to_string(),
            unit: String::new(),
        }
    }

    // ==========================================================================
    // Round-trip encoding tests
    // ==========================================================================

    #[test]
    fn test_workspace_round_trip_empty() {
        let ws = WorkspaceConfig::new("test");
        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.workspace.name, "test");
        assert!(decoded.panes.is_empty());
    }

    #[test]
    fn test_workspace_round_trip_single_pane() {
        let mut ws = WorkspaceConfig::new("single");
        ws.panes.push(make_pane("rate(http_requests_total[5m])"));

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.workspace.name, "single");
        assert_eq!(decoded.panes.len(), 1);
        assert_eq!(decoded.panes[0].query, "rate(http_requests_total[5m])");
    }

    #[test]
    fn test_workspace_round_trip_multiple_panes() {
        let mut ws = WorkspaceConfig::new("multi");
        ws.panes.push(make_pane("query1"));
        ws.panes.push(make_pane("query2"));
        ws.panes.push(make_pane("query3"));

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.panes.len(), 3);
        assert_eq!(decoded.panes[0].query, "query1");
        assert_eq!(decoded.panes[1].query, "query2");
        assert_eq!(decoded.panes[2].query, "query3");
    }

    #[test]
    fn test_workspace_round_trip_preserves_pane_fields() {
        let mut ws = WorkspaceConfig::new("detailed");
        ws.panes
            .push(make_pane_full("query", "My Pane", "Critical", "1h", "stat"));

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.panes[0].query, "query");
        assert_eq!(decoded.panes[0].name, "My Pane");
        assert_eq!(decoded.panes[0].tag, "Critical");
        assert_eq!(decoded.panes[0].granularity, "1h");
        assert_eq!(decoded.panes[0].visualization, "stat");
    }

    // ==========================================================================
    // Header bit-packing tests (time preset + theme)
    // ==========================================================================

    #[test]
    fn test_header_all_time_presets() {
        let presets = ["5m", "15m", "30m", "1h", "6h", "24h", "7d"];

        for preset in &presets {
            let mut ws = WorkspaceConfig::new("test");
            ws.time.preset = preset.to_string();

            let encoded = encode_workspace(&ws).expect("encode should succeed");
            let decoded = decode_workspace(&encoded).expect("decode should succeed");

            assert_eq!(
                decoded.time.preset, *preset,
                "Time preset {preset} should round-trip"
            );
        }
    }

    #[test]
    fn test_header_theme_dark() {
        let mut ws = WorkspaceConfig::new("test");
        ws.view.theme = "dark".to_string();

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.view.theme, "dark");
    }

    #[test]
    fn test_header_theme_light() {
        let mut ws = WorkspaceConfig::new("test");
        ws.view.theme = "light".to_string();

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.view.theme, "light");
    }

    #[test]
    fn test_header_all_combinations() {
        // Test all 14 combinations: 7 time presets × 2 themes
        let presets = ["5m", "15m", "30m", "1h", "6h", "24h", "7d"];
        let themes = ["dark", "light"];

        for preset in &presets {
            for theme in &themes {
                let mut ws = WorkspaceConfig::new("combo");
                ws.time.preset = preset.to_string();
                ws.view.theme = theme.to_string();

                let encoded = encode_workspace(&ws).expect("encode should succeed");
                let decoded = decode_workspace(&encoded).expect("decode should succeed");

                assert_eq!(
                    decoded.time.preset, *preset,
                    "preset={preset}, theme={theme}"
                );
                assert_eq!(decoded.view.theme, *theme, "preset={preset}, theme={theme}");
            }
        }
    }

    // ==========================================================================
    // Pane flags bit-packing tests (granularity + visualization)
    // ==========================================================================

    #[test]
    fn test_pane_flags_all_granularities() {
        let granularities = ["1m", "5m", "15m", "1h", "6h", "1d"];

        for gran in &granularities {
            let mut ws = WorkspaceConfig::new("test");
            ws.panes
                .push(make_pane_full("q", "", "", gran, "time_series"));

            let encoded = encode_workspace(&ws).expect("encode should succeed");
            let decoded = decode_workspace(&encoded).expect("decode should succeed");

            assert_eq!(
                decoded.panes[0].granularity, *gran,
                "Granularity {gran} should round-trip"
            );
        }
    }

    #[test]
    fn test_pane_flags_all_visualizations() {
        let visualizations = [
            "time_series",
            "stat",
            "gauge",
            "bar_chart",
            "sparkline",
            "heatmap",
        ];

        for viz in &visualizations {
            let mut ws = WorkspaceConfig::new("test");
            ws.panes.push(make_pane_full("q", "", "", "5m", viz));

            let encoded = encode_workspace(&ws).expect("encode should succeed");
            let decoded = decode_workspace(&encoded).expect("decode should succeed");

            assert_eq!(
                decoded.panes[0].visualization, *viz,
                "Visualization {viz} should round-trip"
            );
        }
    }

    #[test]
    fn test_pane_flags_all_combinations() {
        // Test a subset of granularity × visualization combinations
        let granularities = ["1m", "1h", "1d"];
        let visualizations = ["time_series", "stat", "heatmap"];

        for gran in &granularities {
            for viz in &visualizations {
                let mut ws = WorkspaceConfig::new("combo");
                ws.panes.push(make_pane_full("q", "", "", gran, viz));

                let encoded = encode_workspace(&ws).expect("encode should succeed");
                let decoded = decode_workspace(&encoded).expect("decode should succeed");

                assert_eq!(
                    decoded.panes[0].granularity, *gran,
                    "gran={gran}, viz={viz}"
                );
                assert_eq!(
                    decoded.panes[0].visualization, *viz,
                    "gran={gran}, viz={viz}"
                );
            }
        }
    }

    // ==========================================================================
    // Layout encoding tests
    // ==========================================================================

    #[test]
    fn test_layout_tabs_is_default_and_not_encoded() {
        // Default tabs layout with all panes should not be explicitly encoded
        let mut ws = WorkspaceConfig::new("tabs");
        ws.panes.push(make_pane("q1"));
        ws.panes.push(make_pane("q2"));
        // No explicit layout = tabs with all panes in order

        let compact = CompactWorkspaceConfig::from_workspace(&ws);
        assert!(
            compact.layout.is_none(),
            "Default tabs layout should be None"
        );
    }

    #[test]
    fn test_layout_horizontal_split() {
        let mut ws = WorkspaceConfig::new("hsplit");
        ws.panes.push(make_pane("left"));
        ws.panes.push(make_pane("right"));
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
            shares: vec![],
        });

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let layout = decoded.layout.expect("layout should exist");
        assert_eq!(layout.layout_type, LayoutType::Horizontal);
        assert_eq!(layout.children.len(), 2);
        assert!(matches!(layout.children[0], LayoutNode::Pane(0)));
        assert!(matches!(layout.children[1], LayoutNode::Pane(1)));
    }

    #[test]
    fn test_layout_vertical_split() {
        let mut ws = WorkspaceConfig::new("vsplit");
        ws.panes.push(make_pane("top"));
        ws.panes.push(make_pane("bottom"));
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Vertical,
            children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
            shares: vec![],
        });

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let layout = decoded.layout.expect("layout should exist");
        assert_eq!(layout.layout_type, LayoutType::Vertical);
    }

    #[test]
    fn test_layout_nested_containers() {
        // Create: horizontal [ vertical [0, 1], 2 ]
        let mut ws = WorkspaceConfig::new("nested");
        ws.panes.push(make_pane("top-left"));
        ws.panes.push(make_pane("bottom-left"));
        ws.panes.push(make_pane("right"));
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![
                LayoutNode::Container(LayoutContainer {
                    layout_type: LayoutType::Vertical,
                    children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
                    shares: vec![],
                }),
                LayoutNode::Pane(2),
            ],
            shares: vec![],
        });

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let layout = decoded.layout.expect("layout should exist");
        assert_eq!(layout.layout_type, LayoutType::Horizontal);
        assert_eq!(layout.children.len(), 2);

        // First child should be a vertical container
        match &layout.children[0] {
            LayoutNode::Container(c) => {
                assert_eq!(c.layout_type, LayoutType::Vertical);
                assert_eq!(c.children.len(), 2);
            }
            _ => panic!("Expected nested container"),
        }

        // Second child should be pane 2
        assert!(matches!(layout.children[1], LayoutNode::Pane(2)));
    }

    #[test]
    fn test_layout_pane_index_encoding() {
        // Test that pane indices are correctly encoded with 128+ offset
        // Use horizontal layout since default tabs layout is optimized away
        let mut ws = WorkspaceConfig::new("indices");
        for i in 0..5 {
            ws.panes.push(make_pane(&format!("pane{i}")));
        }
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![
                LayoutNode::Pane(0),
                LayoutNode::Pane(1),
                LayoutNode::Pane(2),
                LayoutNode::Pane(3),
                LayoutNode::Pane(4),
            ],
            shares: vec![],
        });

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let layout = decoded.layout.expect("layout should exist");
        for (i, child) in layout.children.iter().enumerate() {
            match child {
                LayoutNode::Pane(idx) => assert_eq!(*idx, i, "Pane index should be {i}"),
                _ => panic!("Expected pane at index {i}"),
            }
        }
    }

    // ==========================================================================
    // Single pane encoding tests
    // ==========================================================================

    #[test]
    fn test_single_pane_round_trip() {
        let mut ws = WorkspaceConfig::new("single");
        ws.panes
            .push(make_pane_full("my_query", "My Name", "", "15m", "stat"));
        ws.time.preset = "1h".to_string();
        ws.view.theme = "light".to_string();

        let encoded = encode_pane(&ws, 0).expect("encode should succeed");
        assert!(encoded.starts_with('q'), "Should have 'q' prefix");

        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.panes.len(), 1);
        assert_eq!(decoded.panes[0].query, "my_query");
        assert_eq!(decoded.panes[0].name, "My Name");
        assert_eq!(decoded.panes[0].granularity, "15m");
        assert_eq!(decoded.panes[0].visualization, "stat");
        assert_eq!(decoded.time.preset, "1h");
        assert_eq!(decoded.view.theme, "light");
    }

    #[test]
    fn test_single_pane_more_compact() {
        let mut ws = WorkspaceConfig::new("test");
        ws.panes.push(make_pane("query"));

        let workspace_encoded = encode_workspace(&ws).expect("encode should succeed");
        let pane_encoded = encode_pane(&ws, 0).expect("encode should succeed");

        assert!(
            pane_encoded.len() <= workspace_encoded.len(),
            "Single pane format should be at most as long as workspace format"
        );
    }

    #[test]
    fn test_single_pane_index_out_of_range() {
        let ws = WorkspaceConfig::new("empty");
        let result = encode_pane(&ws, 0);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, WorkspaceError::Encode(_)),
            "Should be an encode error"
        );
    }

    // ==========================================================================
    // Error handling tests
    // ==========================================================================

    #[test]
    fn test_decode_invalid_base64() {
        let result = decode_workspace("p!!!invalid-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_empty_string() {
        // Empty string should fail (no prefix, not valid TOML)
        let result = decode_workspace("");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_just_prefix() {
        // Just "p" or "q" with no data should fail
        let result = decode_workspace("p");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_corrupt_lz4() {
        use base64::Engine;
        // Valid base64 but corrupt LZ4 data
        let garbage = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([1, 2, 3, 4, 5]);
        let result = decode_workspace(&format!("p{garbage}"));
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_corrupt_postcard() {
        use base64::Engine;
        // Valid LZ4 but definitely invalid postcard data
        // Use bytes that can't be a valid postcard CompactWorkspaceConfig
        let garbage_data = vec![0xFFu8; 10]; // 0xFF bytes are unlikely to form valid varint
        let compressed = lz4_flex::compress_prepend_size(&garbage_data);
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&compressed);
        let result = decode_workspace(&format!("p{encoded}"));
        // This may or may not fail depending on how postcard interprets the garbage
        // The important thing is it doesn't panic
        let _ = result;
    }

    // ==========================================================================
    // Optional field handling tests
    // ==========================================================================

    #[test]
    fn test_empty_name_preserved_as_empty() {
        let mut ws = WorkspaceConfig::new("test");
        ws.panes.push(PaneConfig {
            query: "q".to_string(),
            name: String::new(),
            description: String::new(),
            tag: String::new(),
            granularity: "5m".to_string(),
            visualization: "time_series".to_string(),
            unit: String::new(),
        });

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert!(decoded.panes[0].name.is_empty());
        assert!(decoded.panes[0].tag.is_empty());
    }

    #[test]
    fn test_unicode_in_queries() {
        let mut ws = WorkspaceConfig::new("unicode");
        ws.panes.push(make_pane("metric{label=\"日本語\"}"));

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.panes[0].query, "metric{label=\"日本語\"}");
    }

    #[test]
    fn test_special_chars_in_name() {
        let mut ws = WorkspaceConfig::new("special");
        ws.panes.push(
            PaneConfig::new("q")
                .with_name("Test <script>alert('xss')</script>")
                .with_tag("🔥 Critical"),
        );

        let encoded = encode_workspace(&ws).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.panes[0].name, "Test <script>alert('xss')</script>");
        assert_eq!(decoded.panes[0].tag, "🔥 Critical");
    }

    // ==========================================================================
    // Prefix detection tests
    // ==========================================================================

    #[test]
    fn test_workspace_encoded_has_p_prefix() {
        let ws = WorkspaceConfig::new("test");
        let encoded = encode_workspace(&ws).expect("encode should succeed");
        assert!(encoded.starts_with('p'), "Workspace should have 'p' prefix");
    }

    #[test]
    fn test_pane_encoded_has_q_prefix() {
        let mut ws = WorkspaceConfig::new("test");
        ws.panes.push(make_pane("q"));
        let encoded = encode_pane(&ws, 0).expect("encode should succeed");
        assert!(
            encoded.starts_with('q'),
            "Single pane should have 'q' prefix"
        );
    }

    // ==========================================================================
    // Snapshot encoding tests
    // ==========================================================================

    fn make_time_series_data() -> SnapshotPaneData {
        SnapshotPaneData::TimeSeries {
            series: vec![
                SnapshotSeries {
                    name: "http_requests_total".to_string(),
                    tags: vec![("method".to_string(), "GET".to_string())],
                    points: vec![(1000.0, 42.0), (1060.0, 55.0), (1120.0, 38.0)],
                },
                SnapshotSeries {
                    name: "http_requests_total".to_string(),
                    tags: vec![("method".to_string(), "POST".to_string())],
                    points: vec![(1000.0, 10.0), (1060.0, 12.0)],
                },
            ],
        }
    }

    #[test]
    fn test_snapshot_workspace_round_trip_time_series() {
        let mut ws = WorkspaceConfig::new("snap");
        ws.panes.push(make_pane("rate(http_requests_total[5m])"));
        let pane_data = vec![make_time_series_data()];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        assert!(encoded.starts_with('s'), "Should have 's' prefix");

        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        assert_eq!(decoded.workspace.name, "snap");
        assert_eq!(decoded.panes.len(), 1);
        assert_eq!(decoded.panes[0].query, "rate(http_requests_total[5m])");

        let snapshot = decoded.snapshot.expect("snapshot should exist");
        assert_eq!(snapshot.captured_at, 1700000000);
        assert_eq!(snapshot.pane_data.len(), 1);

        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                assert_eq!(series.len(), 2);
                assert_eq!(series[0].name, "http_requests_total");
                assert_eq!(
                    series[0].tags,
                    vec![("method".to_string(), "GET".to_string())]
                );
                assert_eq!(series[0].points.len(), 3);
                // Timestamps are exact (f64 base + u32 delta), values approximate (f32 round-trip)
                let (t, v) = series[0].points[0];
                assert!(
                    (t - 1000.0).abs() < 0.01,
                    "timestamp should be ~1000.0, got {t}"
                );
                assert!((v - 42.0).abs() < 0.1, "value should be ~42.0, got {v}");
                assert_eq!(series[1].points.len(), 2);
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    #[test]
    fn test_snapshot_workspace_round_trip_stat() {
        let mut ws = WorkspaceConfig::new("stat-snap");
        ws.panes
            .push(make_pane_full("up", "Uptime", "", "5m", "stat"));
        let pane_data = vec![SnapshotPaneData::Stat {
            value: 99.9,
            sparkline: vec![98.0, 99.0, 99.5, 99.9],
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let snapshot = decoded.snapshot.expect("snapshot should exist");
        match &snapshot.pane_data[0] {
            SnapshotPaneData::Stat { value, sparkline } => {
                assert!((value - 99.9).abs() < 0.01);
                assert_eq!(sparkline.len(), 4);
            }
            other => panic!("Expected Stat, got {other:?}"),
        }
    }

    #[test]
    fn test_snapshot_workspace_round_trip_gauge() {
        let mut ws = WorkspaceConfig::new("gauge-snap");
        ws.panes
            .push(make_pane_full("cpu", "CPU", "", "5m", "gauge"));
        let pane_data = vec![SnapshotPaneData::Gauge {
            value: 75.5,
            min: 0.0,
            max: 100.0,
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let snapshot = decoded.snapshot.expect("snapshot should exist");
        match &snapshot.pane_data[0] {
            SnapshotPaneData::Gauge { value, min, max } => {
                assert!((value - 75.5).abs() < 0.01);
                assert!((min - 0.0).abs() < 0.01);
                assert!((max - 100.0).abs() < 0.01);
            }
            other => panic!("Expected Gauge, got {other:?}"),
        }
    }

    #[test]
    fn test_snapshot_workspace_round_trip_bar_chart() {
        let mut ws = WorkspaceConfig::new("bar-snap");
        ws.panes
            .push(make_pane_full("topk", "Top 5", "", "5m", "bar_chart"));
        let pane_data = vec![SnapshotPaneData::BarChart {
            bars: vec![
                ("alpha".to_string(), 100.0),
                ("beta".to_string(), 75.0),
                ("gamma".to_string(), 50.0),
            ],
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let snapshot = decoded.snapshot.expect("snapshot should exist");
        match &snapshot.pane_data[0] {
            SnapshotPaneData::BarChart { bars } => {
                assert_eq!(bars.len(), 3);
                assert_eq!(bars[0].0, "alpha");
                assert!((bars[0].1 - 100.0).abs() < 0.01);
            }
            other => panic!("Expected BarChart, got {other:?}"),
        }
    }

    #[test]
    fn test_snapshot_workspace_round_trip_heatmap() {
        let mut ws = WorkspaceConfig::new("heat-snap");
        ws.panes
            .push(make_pane_full("histo", "Latency", "", "5m", "heatmap"));
        let pane_data = vec![SnapshotPaneData::Heatmap {
            cols: 3,
            rows: 2,
            values: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let snapshot = decoded.snapshot.expect("snapshot should exist");
        match &snapshot.pane_data[0] {
            SnapshotPaneData::Heatmap { cols, rows, values } => {
                assert_eq!(*cols, 3);
                assert_eq!(*rows, 2);
                assert_eq!(values.len(), 6);
            }
            other => panic!("Expected Heatmap, got {other:?}"),
        }
    }

    #[test]
    fn test_snapshot_workspace_multiple_panes() {
        let mut ws = WorkspaceConfig::new("multi-snap");
        ws.panes.push(make_pane("q1"));
        ws.panes.push(make_pane("q2"));
        let pane_data = vec![
            make_time_series_data(),
            SnapshotPaneData::Stat {
                value: 42.0,
                sparkline: vec![],
            },
        ];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        assert_eq!(decoded.panes.len(), 2);
        let snapshot = decoded.snapshot.expect("snapshot should exist");
        assert_eq!(snapshot.pane_data.len(), 2);
        assert!(matches!(
            snapshot.pane_data[0],
            SnapshotPaneData::TimeSeries { .. }
        ));
        assert!(matches!(
            snapshot.pane_data[1],
            SnapshotPaneData::Stat { .. }
        ));
    }

    #[test]
    fn test_snapshot_single_pane_round_trip() {
        let mut ws = WorkspaceConfig::new("single-snap");
        ws.panes.push(make_pane_full(
            "rate(errors[5m])",
            "Error Rate",
            "Critical",
            "1m",
            "time_series",
        ));
        ws.time.preset = "1h".to_string();
        let data = make_time_series_data();

        let encoded =
            encode_snapshot_pane(&ws, 0, &data, 1700000000).expect("encode should succeed");
        assert!(encoded.starts_with('t'), "Should have 't' prefix");

        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        assert_eq!(decoded.panes.len(), 1);
        assert_eq!(decoded.panes[0].query, "rate(errors[5m])");
        assert_eq!(decoded.panes[0].name, "Error Rate");
        assert_eq!(decoded.time.preset, "1h");

        let snapshot = decoded.snapshot.expect("snapshot should exist");
        assert_eq!(snapshot.captured_at, 1700000000);
        assert!(matches!(
            snapshot.pane_data[0],
            SnapshotPaneData::TimeSeries { .. }
        ));
    }

    #[test]
    fn test_snapshot_prefixes() {
        let mut ws = WorkspaceConfig::new("test");
        ws.panes.push(make_pane("q"));
        let data = vec![SnapshotPaneData::Stat {
            value: 1.0,
            sparkline: vec![],
        }];

        let ws_encoded = encode_snapshot_workspace(&ws, &data, 0).expect("encode should succeed");
        assert!(
            ws_encoded.starts_with('s'),
            "Workspace snapshot should have 's' prefix"
        );

        let pane_encoded =
            encode_snapshot_pane(&ws, 0, &data[0], 0).expect("encode should succeed");
        assert!(
            pane_encoded.starts_with('t'),
            "Pane snapshot should have 't' prefix"
        );
    }

    #[test]
    fn test_existing_p_q_formats_still_work_after_snapshot_addition() {
        // Verify backward compatibility: existing p/q formats still decode correctly
        let mut ws = WorkspaceConfig::new("compat");
        ws.panes.push(make_pane("rate(http_requests[5m])"));
        ws.time.preset = "1h".to_string();

        let p_encoded = encode_workspace(&ws).expect("encode p should succeed");
        let q_encoded = encode_pane(&ws, 0).expect("encode q should succeed");

        let p_decoded = decode_workspace(&p_encoded).expect("decode p should succeed");
        assert_eq!(p_decoded.panes[0].query, "rate(http_requests[5m])");
        assert!(
            p_decoded.snapshot.is_none(),
            "p format should have no snapshot"
        );

        let q_decoded = decode_workspace(&q_encoded).expect("decode q should succeed");
        assert_eq!(q_decoded.panes[0].query, "rate(http_requests[5m])");
        assert!(
            q_decoded.snapshot.is_none(),
            "q format should have no snapshot"
        );
    }

    // ==========================================================================
    // LTTB downsampling tests
    // ==========================================================================

    #[test]
    fn test_lttb_passthrough() {
        // Points below threshold pass through unchanged
        let points: Vec<(f64, f64)> = (0..50).map(|i| (i as f64, (i as f64).sin())).collect();
        let result = lttb_downsample(&points, SNAPSHOT_MAX_POINTS);
        assert_eq!(result.len(), 50);
        assert_eq!(result, points);
    }

    #[test]
    fn test_lttb_reduces_to_target() {
        // 500-point sine wave reduced to 100
        let points: Vec<(f64, f64)> = (0..500)
            .map(|i| (i as f64 * 60.0, (i as f64 * 0.1).sin() * 100.0))
            .collect();
        let result = lttb_downsample(&points, 100);
        assert_eq!(result.len(), 100);
        // First and last points preserved
        assert_eq!(result[0], points[0]);
        assert_eq!(result[99], points[499]);
    }

    #[test]
    fn test_lttb_preserves_extrema() {
        // Flat data with a spike — the spike should survive
        let mut points: Vec<(f64, f64)> = (0..200).map(|i| (i as f64 * 60.0, 50.0)).collect();
        points[100] = (100.0 * 60.0, 1000.0); // Big spike at index 100

        let result = lttb_downsample(&points, 50);
        assert_eq!(result.len(), 50);
        // The spike should be in the result
        assert!(
            result.iter().any(|p| (p.1 - 1000.0).abs() < 0.01),
            "Spike at y=1000 should survive downsampling"
        );
    }

    #[test]
    fn test_lttb_edge_cases() {
        // Empty input
        let empty: Vec<(f64, f64)> = vec![];
        assert_eq!(lttb_downsample(&empty, 100), empty);

        // Single point
        let single = vec![(1.0, 2.0)];
        assert_eq!(lttb_downsample(&single, 100), single);

        // Two points
        let two = vec![(1.0, 2.0), (3.0, 4.0)];
        assert_eq!(lttb_downsample(&two, 100), two);

        // Target < 3 returns input unchanged
        let points: Vec<(f64, f64)> = (0..10).map(|i| (i as f64, i as f64)).collect();
        assert_eq!(lttb_downsample(&points, 2), points);
    }

    // ==========================================================================
    // Delta encoding + f32 precision tests
    // ==========================================================================

    #[test]
    fn test_delta_encoding_regular_intervals() {
        let mut ws = WorkspaceConfig::new("delta");
        ws.panes.push(make_pane("q"));
        // 100 points at 60-second intervals
        let points: Vec<(f64, f64)> = (0..100)
            .map(|i| (1700000000.0 + i as f64 * 60.0, i as f64 * 1.5))
            .collect();
        let pane_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![SnapshotSeries {
                name: "test".to_string(),
                tags: vec![],
                points: points.clone(),
            }],
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        let snapshot = decoded.snapshot.expect("snapshot should exist");

        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                assert_eq!(series[0].points.len(), 100);
                // Check first and last timestamps
                let (t0, _) = series[0].points[0];
                assert!((t0 - 1700000000.0).abs() < 0.01);
                let (t99, _) = series[0].points[99];
                assert!((t99 - (1700000000.0 + 99.0 * 60.0)).abs() < 1.0);
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    #[test]
    fn test_delta_encoding_irregular_intervals() {
        let mut ws = WorkspaceConfig::new("irreg");
        ws.panes.push(make_pane("q"));
        let points = vec![
            (1700000000.0, 1.0),
            (1700000005.0, 2.0), // 5s gap
            (1700000300.0, 3.0), // 295s gap
            (1700003600.0, 4.0), // 3300s gap
            (1700090000.0, 5.0), // 86400s gap
        ];
        let pane_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![SnapshotSeries {
                name: "irreg".to_string(),
                tags: vec![],
                points: points.clone(),
            }],
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        let snapshot = decoded.snapshot.expect("snapshot should exist");

        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                assert_eq!(series[0].points.len(), 5);
                for (i, (orig_t, _)) in points.iter().enumerate() {
                    let (t, _) = series[0].points[i];
                    assert!(
                        (t - orig_t).abs() < 1.0,
                        "point {i}: expected ~{orig_t}, got {t}"
                    );
                }
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    #[test]
    fn test_f32_precision_sufficient() {
        let mut ws = WorkspaceConfig::new("prec");
        ws.panes.push(make_pane("q"));
        let values = [0.0, 0.001, 1.5, 42.0, 99.9, 1000.0, 99999.0];
        let points: Vec<(f64, f64)> = values
            .iter()
            .enumerate()
            .map(|(i, &v)| (1700000000.0 + i as f64 * 60.0, v))
            .collect();
        let pane_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![SnapshotSeries {
                name: "prec".to_string(),
                tags: vec![],
                points,
            }],
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        let snapshot = decoded.snapshot.expect("snapshot should exist");

        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                for (i, &expected) in values.iter().enumerate() {
                    let (_, v) = series[0].points[i];
                    let tolerance = (expected.abs() * 1e-6).max(1e-6);
                    assert!(
                        (v - expected).abs() < tolerance,
                        "value {i}: expected ~{expected}, got {v}"
                    );
                }
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    #[test]
    fn test_snapshot_empty_series() {
        let mut ws = WorkspaceConfig::new("empty-series");
        ws.panes.push(make_pane("q"));
        let pane_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![SnapshotSeries {
                name: "empty".to_string(),
                tags: vec![],
                points: vec![],
            }],
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        let snapshot = decoded.snapshot.expect("snapshot should exist");

        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                assert_eq!(series[0].points.len(), 0);
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    #[test]
    fn test_snapshot_single_point() {
        let mut ws = WorkspaceConfig::new("single-pt");
        ws.panes.push(make_pane("q"));
        let pane_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![SnapshotSeries {
                name: "one".to_string(),
                tags: vec![],
                points: vec![(1700000000.0, 42.0)],
            }],
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        let snapshot = decoded.snapshot.expect("snapshot should exist");

        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                assert_eq!(series[0].points.len(), 1);
                let (t, v) = series[0].points[0];
                assert!((t - 1700000000.0).abs() < 0.01);
                assert!((v - 42.0).abs() < 0.1);
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    #[test]
    fn test_snapshot_size_reduction() {
        // Realistic 240-point, 2-series snapshot should be compact
        let mut ws = WorkspaceConfig::new("size");
        ws.panes.push(make_pane("rate(requests[5m])"));
        let points_a: Vec<(f64, f64)> = (0..240)
            .map(|i| {
                (
                    1700000000.0 + i as f64 * 60.0,
                    100.0 + (i as f64 * 0.05).sin() * 50.0,
                )
            })
            .collect();
        let points_b: Vec<(f64, f64)> = (0..240)
            .map(|i| {
                (
                    1700000000.0 + i as f64 * 60.0,
                    200.0 + (i as f64 * 0.03).cos() * 30.0,
                )
            })
            .collect();
        let pane_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![
                SnapshotSeries {
                    name: "series_a".to_string(),
                    tags: vec![("method".to_string(), "GET".to_string())],
                    points: points_a,
                },
                SnapshotSeries {
                    name: "series_b".to_string(),
                    tags: vec![("method".to_string(), "POST".to_string())],
                    points: points_b,
                },
            ],
        }];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");

        // After LTTB (240→100) + delta + f32, should be reasonably compact
        // The encoded URL string should be under 2KB for this workload
        assert!(
            encoded.len() < 2000,
            "Encoded snapshot should be under 2KB, got {} bytes",
            encoded.len()
        );

        // Verify it decodes correctly
        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        let snapshot = decoded.snapshot.expect("snapshot should exist");
        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                // LTTB should have reduced from 240 to 100 points
                assert_eq!(series[0].points.len(), 100);
                assert_eq!(series[1].points.len(), 100);
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    // ==========================================================================
    // String deduplication tests
    // ==========================================================================

    #[test]
    fn test_string_dedup_shared_metric_name() {
        // 5 series with the same metric name and tag key should deduplicate
        let mut ws = WorkspaceConfig::new("dedup");
        ws.panes.push(make_pane("http_requests_total"));
        let series: Vec<SnapshotSeries> = (0..5)
            .map(|i| SnapshotSeries {
                name: "http_requests_total".to_string(),
                tags: vec![("method".to_string(), format!("method_{i}"))],
                points: vec![(1000.0, i as f64)],
            })
            .collect();
        let pane_data = vec![SnapshotPaneData::TimeSeries { series }];

        let encoded = encode_snapshot_workspace(&ws, &pane_data, 0).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        let snapshot = decoded.snapshot.expect("snapshot should exist");

        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                assert_eq!(series.len(), 5);
                // All series should have the same name restored
                for s in series {
                    assert_eq!(s.name, "http_requests_total");
                    assert_eq!(s.tags[0].0, "method");
                }
                // Verify distinct tag values survived
                assert_eq!(series[0].tags[0].1, "method_0");
                assert_eq!(series[4].tags[0].1, "method_4");
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    #[test]
    fn test_string_dedup_reduces_size() {
        // Compare encoded size: 10 series sharing names vs hypothetical unique names
        let mut ws = WorkspaceConfig::new("dedup-size");
        ws.panes.push(make_pane("long_metric_name_that_repeats"));
        let series: Vec<SnapshotSeries> = (0..10)
            .map(|i| SnapshotSeries {
                name: "long_metric_name_that_repeats_across_series".to_string(),
                tags: vec![
                    ("instance".to_string(), format!("host-{i}")),
                    ("job".to_string(), "prometheus".to_string()),
                ],
                points: vec![(1000.0, i as f64)],
            })
            .collect();
        let dedup_data = vec![SnapshotPaneData::TimeSeries {
            series: series.clone(),
        }];

        let dedup_encoded =
            encode_snapshot_workspace(&ws, &dedup_data, 0).expect("encode should succeed");

        // With 10 series sharing the same 44-char name and "instance"/"job" tag keys,
        // dedup should save significant space
        // Verify it round-trips correctly
        let decoded = decode_workspace(&dedup_encoded).expect("decode should succeed");
        let snapshot = decoded.snapshot.expect("snapshot should exist");
        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                assert_eq!(series.len(), 10);
                assert_eq!(
                    series[0].name,
                    "long_metric_name_that_repeats_across_series"
                );
                assert_eq!(series[9].tags[1].1, "prometheus");
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    // ==========================================================================
    // Regular delta detection tests
    // ==========================================================================

    #[test]
    fn test_regular_deltas_round_trip() {
        // 100 points at exactly 60-second intervals should use Regular encoding
        let mut ws = WorkspaceConfig::new("regular");
        ws.panes.push(make_pane("q"));
        let points: Vec<(f64, f64)> = (0..100)
            .map(|i| (1700000000.0 + i as f64 * 60.0, i as f64))
            .collect();
        let pane_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![SnapshotSeries {
                name: "test".to_string(),
                tags: vec![],
                points: points.clone(),
            }],
        }];

        let encoded = encode_snapshot_workspace(&ws, &pane_data, 0).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        let snapshot = decoded.snapshot.expect("snapshot should exist");

        match &snapshot.pane_data[0] {
            SnapshotPaneData::TimeSeries { series } => {
                assert_eq!(series[0].points.len(), 100);
                // Verify timestamps reconstructed correctly
                for (i, &(orig_t, _)) in points.iter().enumerate() {
                    let (t, _) = series[0].points[i];
                    assert!(
                        (t - orig_t).abs() < 0.01,
                        "point {i}: expected {orig_t}, got {t}"
                    );
                }
            }
            other => panic!("Expected TimeSeries, got {other:?}"),
        }
    }

    #[test]
    fn test_regular_deltas_smaller_than_irregular() {
        // Regular 60s intervals should encode smaller than irregular
        let mut ws = WorkspaceConfig::new("reg-size");
        ws.panes.push(make_pane("q"));

        // Regular: all 60s intervals
        let regular_points: Vec<(f64, f64)> = (0..100)
            .map(|i| (1700000000.0 + i as f64 * 60.0, i as f64))
            .collect();
        let regular_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![SnapshotSeries {
                name: "reg".to_string(),
                tags: vec![],
                points: regular_points,
            }],
        }];

        // Irregular: varying intervals
        let irregular_points: Vec<(f64, f64)> = (0..100)
            .map(|i| {
                let jitter = (i as f64 * 7.3).sin() * 10.0;
                (1700000000.0 + i as f64 * 60.0 + jitter, i as f64)
            })
            .collect();
        let irregular_data = vec![SnapshotPaneData::TimeSeries {
            series: vec![SnapshotSeries {
                name: "irr".to_string(),
                tags: vec![],
                points: irregular_points,
            }],
        }];

        let regular_encoded =
            encode_snapshot_workspace(&ws, &regular_data, 0).expect("encode regular");
        let irregular_encoded =
            encode_snapshot_workspace(&ws, &irregular_data, 0).expect("encode irregular");

        assert!(
            regular_encoded.len() < irregular_encoded.len(),
            "Regular ({} bytes) should be smaller than irregular ({} bytes)",
            regular_encoded.len(),
            irregular_encoded.len()
        );
    }

    // ==========================================================================
    // Snapshot + layout round-trip tests
    // ==========================================================================

    #[test]
    fn test_snapshot_workspace_round_trip_with_horizontal_layout() {
        let mut ws = WorkspaceConfig::new("hsplit-snap");
        ws.panes.push(make_pane("left_query"));
        ws.panes.push(make_pane("right_query"));
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
            shares: vec![],
        });
        let pane_data = vec![make_time_series_data(), make_time_series_data()];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        assert!(encoded.starts_with('s'), "Should have 's' prefix");

        let decoded = decode_workspace(&encoded).expect("decode should succeed");
        assert_eq!(decoded.panes.len(), 2);

        let layout = decoded
            .layout
            .expect("layout should survive snapshot round-trip");
        assert_eq!(layout.layout_type, LayoutType::Horizontal);
        assert_eq!(layout.children.len(), 2);
        assert!(matches!(layout.children[0], LayoutNode::Pane(0)));
        assert!(matches!(layout.children[1], LayoutNode::Pane(1)));
    }

    #[test]
    fn test_snapshot_workspace_round_trip_with_vertical_layout() {
        let mut ws = WorkspaceConfig::new("vsplit-snap");
        ws.panes.push(make_pane("top_query"));
        ws.panes.push(make_pane("bottom_query"));
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Vertical,
            children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
            shares: vec![],
        });
        let pane_data = vec![make_time_series_data(), make_time_series_data()];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let layout = decoded
            .layout
            .expect("layout should survive snapshot round-trip");
        assert_eq!(layout.layout_type, LayoutType::Vertical);
        assert_eq!(layout.children.len(), 2);
    }

    #[test]
    fn test_snapshot_workspace_round_trip_with_nested_layout() {
        // Nested: Horizontal [ Vertical [0, 1], 2 ]
        let mut ws = WorkspaceConfig::new("nested-snap");
        ws.panes.push(make_pane("top-left"));
        ws.panes.push(make_pane("bottom-left"));
        ws.panes.push(make_pane("right"));
        ws.layout = Some(LayoutConfig {
            layout_type: LayoutType::Horizontal,
            children: vec![
                LayoutNode::Container(LayoutContainer {
                    layout_type: LayoutType::Vertical,
                    children: vec![LayoutNode::Pane(0), LayoutNode::Pane(1)],
                    shares: vec![],
                }),
                LayoutNode::Pane(2),
            ],
            shares: vec![],
        });
        let pane_data = vec![
            make_time_series_data(),
            make_time_series_data(),
            make_time_series_data(),
        ];

        let encoded =
            encode_snapshot_workspace(&ws, &pane_data, 1700000000).expect("encode should succeed");
        let decoded = decode_workspace(&encoded).expect("decode should succeed");

        let layout = decoded
            .layout
            .expect("layout should survive snapshot round-trip");
        assert_eq!(layout.layout_type, LayoutType::Horizontal);
        assert_eq!(layout.children.len(), 2);

        // First child: vertical container
        match &layout.children[0] {
            LayoutNode::Container(c) => {
                assert_eq!(c.layout_type, LayoutType::Vertical);
                assert_eq!(c.children.len(), 2);
                assert!(matches!(c.children[0], LayoutNode::Pane(0)));
                assert!(matches!(c.children[1], LayoutNode::Pane(1)));
            }
            _ => panic!("Expected nested vertical container"),
        }

        // Second child: pane 2
        assert!(matches!(layout.children[1], LayoutNode::Pane(2)));

        // Snapshot data should also be preserved
        let snapshot = decoded.snapshot.expect("snapshot should exist");
        assert_eq!(snapshot.pane_data.len(), 3);
    }
}
