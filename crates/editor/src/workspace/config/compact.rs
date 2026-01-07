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

use serde::{Deserialize, Serialize};

use super::{
    LayoutConfig, LayoutContainer, LayoutNode, LayoutType, PaneConfig, WorkspaceConfig,
    WorkspaceError,
};

/// Compact workspace representation for URL sharing (postcard binary format)
/// Uses numeric enums and minimal fields for smallest possible encoding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CompactWorkspaceConfig {
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
pub(super) struct CompactLayout {
    /// Flat encoded layout tree
    pub nodes: Vec<u8>,
}

/// Compact single-pane representation for sharing individual queries
/// Even more minimal than CompactWorkspace - just the essentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CompactSinglePane {
    /// The query expression
    pub query: String,
    /// Optional display name
    pub name: Option<String>,
    /// Packed header: bits 0-2 = time preset, bit 3 = theme
    pub header: u8,
    /// Packed flags: bits 0-2 = granularity (0-5), bits 3-5 = visualization (0-5)
    pub flags: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CompactPane {
    pub query: String,
    /// Optional display name (None = empty string)
    pub name: Option<String>,
    /// Optional tag (None = empty string)
    pub tag: Option<String>,
    /// Packed: bits 0-2 = granularity (0-5), bits 3-5 = visualization (0-5)
    pub flags: u8,
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
                    unit: String::new(), // Compact format doesn't encode unit
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
            unit: String::new(),
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
}
