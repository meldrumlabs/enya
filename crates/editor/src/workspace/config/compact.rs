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
                    tag: p.tag.unwrap_or_default(),
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
            tag: String::new(),
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
