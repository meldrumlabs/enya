//! GPU rendering module using wgpu
//!
//! This module contains GPU-accelerated rendering implementations
//! for visualizations that benefit from GPU acceleration (like heatmaps
//! with thousands of cells). Flamegraphs use CPU rendering since they
//! require text labels on each frame.

pub mod heatmap;

pub use heatmap::{HeatmapCallback, HeatmapGpuResources, init_heatmap_resources};
