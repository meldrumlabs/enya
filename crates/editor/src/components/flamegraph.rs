//! Flamegraph visualization
//!
//! This module provides a flamegraph visualization for displaying profiling data
//! such as CPU and memory profiles from pprof.
//!
//! Unlike heatmaps, flamegraphs use CPU rendering because:
//! - Text labels on each frame are essential for usability
//! - Typical flamegraphs have hundreds of frames, not tens of thousands
//! - The complexity of GPU text rendering isn't worth the minimal benefit

use std::sync::atomic::{AtomicUsize, Ordering};

use egui::{Color32, RichText, Stroke, StrokeKind};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;

/// Global counter for unique flamegraph IDs
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

/// A single frame in the flamegraph
#[derive(Debug, Clone)]
pub struct FlameFrame {
    /// Normalized x start position (0-1)
    pub x_start: f32,
    /// Normalized x end position (0-1)
    pub x_end: f32,
    /// Stack depth (0 = root)
    pub depth: usize,
    /// Color seed for deterministic coloring
    pub color_seed: f32,
    /// Function/symbol name
    pub name: String,
    /// Number of samples or time value
    pub value: u64,
    /// Self time/samples (excluding children)
    pub self_value: u64,
}

/// Profiling data type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileType {
    /// CPU profiling (time/samples)
    #[default]
    Cpu,
    /// Memory profiling (allocations/bytes)
    Memory,
}

impl ProfileType {
    /// Get display name
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
        }
    }
}

/// Flamegraph visualization
///
/// The flamegraph displays stacked call frames where:
/// - Width represents time/samples spent in that function
/// - Depth shows the call stack
/// - Color is based on function name for consistency
pub struct FlamegraphViz {
    /// Unique identifier
    id: usize,
    /// Title (shown in tab)
    pub(crate) title: String,
    /// Profile type (CPU/Memory)
    profile_type: ProfileType,
    /// All frames in the flamegraph
    frames: Vec<FlameFrame>,
    /// Maximum stack depth
    max_depth: usize,
    /// Total samples/value at root
    total_value: u64,
    /// Current theme
    pub(crate) theme: AppTheme,
    /// Currently hovered frame index
    hovered_frame: Option<usize>,
    /// Zoom state: (x_start, x_end) in normalized coords
    zoom: (f32, f32),
    /// Search filter
    search_filter: String,
}

impl Default for FlamegraphViz {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl FlamegraphViz {
    /// Create a new flamegraph visualization
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            title: title.into(),
            profile_type: ProfileType::default(),
            frames: Vec::new(),
            max_depth: 0,
            total_value: 0,
            theme: AppTheme::default(),
            hovered_frame: None,
            zoom: (0.0, 1.0),
            search_filter: String::new(),
        }
    }

    /// Get the unique identifier
    pub fn id(&self) -> usize {
        self.id
    }

    /// Set the title
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Set the profile type
    pub fn set_profile_type(&mut self, profile_type: ProfileType) {
        self.profile_type = profile_type;
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set frames directly
    pub fn set_frames(&mut self, frames: Vec<FlameFrame>, total_value: u64) {
        self.max_depth = frames.iter().map(|f| f.depth).max().unwrap_or(0) + 1;
        self.total_value = total_value;
        self.frames = frames;
        self.zoom = (0.0, 1.0);
        self.hovered_frame = None;
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.frames.clear();
        self.max_depth = 0;
        self.total_value = 0;
        self.hovered_frame = None;
        self.zoom = (0.0, 1.0);
    }

    /// Set search filter
    pub fn set_search_filter(&mut self, filter: impl Into<String>) {
        self.search_filter = filter.into();
    }

    /// Reset zoom to full view
    pub fn reset_zoom(&mut self) {
        self.zoom = (0.0, 1.0);
    }

    /// Zoom to a specific frame
    pub fn zoom_to_frame(&mut self, frame_idx: usize) {
        if let Some(frame) = self.frames.get(frame_idx) {
            self.zoom = (frame.x_start, frame.x_end);
        }
    }

    /// Get color for a frame based on name hash - Obsidian Glass emerald palette
    fn get_cpu_color(name: &str) -> Color32 {
        let hash = name
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        let t = (hash % 1000) as f32 / 1000.0;
        // Bias toward brighter colors for better visibility
        let biased_t = 0.3 + t * 0.7;

        // Obsidian Glass emerald palette (matches heatmap and brand)
        let colors = [
            Color32::from_rgb(10, 10, 10),   // bg::BASE - almost black
            Color32::from_rgb(20, 28, 25),   // Dark with subtle green tint
            Color32::from_rgb(18, 38, 32),   // accent::MUTED - subtle emerald
            Color32::from_rgb(20, 60, 50),   // Deeper emerald
            Color32::from_rgb(32, 100, 85),  // Mid teal-emerald
            Color32::from_rgb(16, 140, 100), // Approaching accent
            Color32::from_rgb(16, 185, 129), // accent::PRIMARY - emerald
            Color32::from_rgb(52, 211, 153), // accent::HOVER - bright emerald
        ];

        let segment = biased_t * 7.0;
        let idx = segment.floor() as usize;
        let frac = segment - segment.floor();

        if idx >= 7 {
            return colors[7];
        }

        let c1 = colors[idx];
        let c2 = colors[idx + 1];

        Color32::from_rgb(
            (c1.r() as f32 + (c2.r() as f32 - c1.r() as f32) * frac) as u8,
            (c1.g() as f32 + (c2.g() as f32 - c1.g() as f32) * frac) as u8,
            (c1.b() as f32 + (c2.b() as f32 - c1.b() as f32) * frac) as u8,
        )
    }

    /// Get color for a frame based on name hash - teal/cyan variant for memory
    fn get_memory_color(name: &str) -> Color32 {
        let hash = name
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        let t = (hash % 1000) as f32 / 1000.0;
        // Bias toward brighter colors for better visibility
        let biased_t = 0.3 + t * 0.7;

        // Teal-cyan palette for memory profiling (cooler emerald variant)
        let colors = [
            Color32::from_rgb(10, 10, 10),   // bg::BASE - almost black
            Color32::from_rgb(15, 25, 28),   // Dark with cyan tint
            Color32::from_rgb(14, 34, 38),   // Subtle teal
            Color32::from_rgb(16, 50, 60),   // Deeper teal
            Color32::from_rgb(24, 85, 100),  // Mid teal
            Color32::from_rgb(15, 115, 130), // Approaching cyan
            Color32::from_rgb(11, 152, 158), // Teal-cyan
            Color32::from_rgb(94, 206, 210), // Bright cyan
        ];

        let segment = biased_t * 7.0;
        let idx = segment.floor() as usize;
        let frac = segment - segment.floor();

        if idx >= 7 {
            return colors[7];
        }

        let c1 = colors[idx];
        let c2 = colors[idx + 1];

        Color32::from_rgb(
            (c1.r() as f32 + (c2.r() as f32 - c1.r() as f32) * frac) as u8,
            (c1.g() as f32 + (c2.g() as f32 - c1.g() as f32) * frac) as u8,
            (c1.b() as f32 + (c2.b() as f32 - c1.b() as f32) * frac) as u8,
        )
    }

    /// Get frame color based on profile type
    fn get_frame_color(&self, name: &str) -> Color32 {
        match self.profile_type {
            ProfileType::Cpu => Self::get_cpu_color(name),
            ProfileType::Memory => Self::get_memory_color(name),
        }
    }

    /// Format value for display
    fn format_value(&self, value: u64) -> String {
        match self.profile_type {
            ProfileType::Cpu => {
                if value >= 1_000_000 {
                    format!("{:.2}s", value as f64 / 1_000_000.0)
                } else if value >= 1_000 {
                    format!("{:.2}ms", value as f64 / 1_000.0)
                } else {
                    format!("{value}µs")
                }
            }
            ProfileType::Memory => {
                if value >= 1_073_741_824 {
                    format!("{:.2}GB", value as f64 / 1_073_741_824.0)
                } else if value >= 1_048_576 {
                    format!("{:.2}MB", value as f64 / 1_048_576.0)
                } else if value >= 1024 {
                    format!("{:.2}KB", value as f64 / 1024.0)
                } else {
                    format!("{value}B")
                }
            }
        }
    }

    /// Render the flamegraph
    fn show_cpu(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        let (zoom_start, zoom_end) = self.zoom;
        let zoom_width = zoom_end - zoom_start;

        let frame_height = rect.height() / self.max_depth.max(1) as f32;

        // Track hovered frame
        let mut new_hovered = None;
        let pointer_pos = ui.input(|i| i.pointer.hover_pos());

        for (idx, frame) in self.frames.iter().enumerate() {
            // Skip frames outside zoom
            if frame.x_end <= zoom_start || frame.x_start >= zoom_end {
                continue;
            }

            // Calculate zoomed position
            let x_start = ((frame.x_start - zoom_start) / zoom_width).clamp(0.0, 1.0);
            let x_end = ((frame.x_end - zoom_start) / zoom_width).clamp(0.0, 1.0);

            // Skip too-small frames
            let width = (x_end - x_start) * rect.width();
            if width < 1.0 {
                continue;
            }

            let frame_rect = egui::Rect::from_min_max(
                egui::pos2(
                    rect.left() + x_start * rect.width(),
                    rect.bottom() - (frame.depth + 1) as f32 * frame_height,
                ),
                egui::pos2(
                    rect.left() + x_end * rect.width(),
                    rect.bottom() - frame.depth as f32 * frame_height - 1.0,
                ),
            );

            // Check hover
            let is_hovered = pointer_pos
                .map(|pos| frame_rect.contains(pos))
                .unwrap_or(false);
            if is_hovered {
                new_hovered = Some(idx);
            }

            // Get color
            let mut color = self.get_frame_color(&frame.name);
            if is_hovered {
                color = color.gamma_multiply(1.3);
            }

            // Draw frame
            painter.rect_filled(frame_rect, 1.0, color);

            // Draw text if frame is wide enough
            if width > 40.0 {
                let text = if width > 150.0 {
                    frame.name.clone()
                } else if width > 80.0 {
                    frame.name.chars().take(15).collect::<String>()
                } else {
                    frame.name.chars().take(5).collect::<String>()
                };

                painter.text(
                    frame_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(10.0),
                    Color32::WHITE,
                );
            }
        }

        self.hovered_frame = new_hovered;
    }

    /// Render the flamegraph
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);

        ui.vertical(|ui| {
            ui.add_space(8.0);

            // Header
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&self.title)
                        .color(text_col)
                        .size(16.0)
                        .strong(),
                );

                ui.add_space(8.0);

                // Profile type badge (CPU/Memory profiling type)
                let badge_color = match self.profile_type {
                    ProfileType::Cpu => palette::semantic::WARNING,
                    ProfileType::Memory => palette::semantic::INFO,
                };
                ui.label(
                    RichText::new(format!("[{} Profile]", self.profile_type.label()))
                        .color(badge_color)
                        .size(12.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);

                    // Reset zoom button
                    if self.zoom != (0.0, 1.0) && ui.small_button("Reset Zoom").clicked() {
                        self.reset_zoom();
                    }

                    // Total value display
                    ui.label(
                        RichText::new(format!("Total: {}", self.format_value(self.total_value)))
                            .color(text_col.gamma_multiply(0.7))
                            .size(12.0),
                    );
                });
            });

            ui.add_space(8.0);

            if self.frames.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("No profiling data")
                            .color(text_col.gamma_multiply(0.4))
                            .size(14.0),
                    );
                });
                return;
            }

            // Calculate available space
            let available = ui.available_size();
            let chart_height = (available.y - 60.0).clamp(100.0, 600.0);
            let chart_width = available.x - 16.0;

            // Allocate space for the flamegraph
            let (response, _painter) =
                ui.allocate_painter(egui::vec2(chart_width, chart_height), egui::Sense::click());
            let rect = response.rect;

            // Handle click to zoom
            if response.clicked() {
                if let Some(idx) = self.hovered_frame {
                    self.zoom_to_frame(idx);
                }
            }

            // Render the flamegraph
            self.show_cpu(ui, rect);

            // Draw border
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, palette::border::SUBTLE),
                StrokeKind::Outside,
            );

            // Show tooltip for hovered frame
            if let Some(idx) = self.hovered_frame {
                if let Some(frame) = self.frames.get(idx) {
                    let percentage = if self.total_value > 0 {
                        (frame.value as f64 / self.total_value as f64) * 100.0
                    } else {
                        0.0
                    };

                    let total_formatted = self.format_value(frame.value);
                    let self_formatted = self.format_value(frame.self_value);

                    response.on_hover_ui(|ui| {
                        ui.label(RichText::new(&frame.name).strong());
                        ui.label(format!("Total: {total_formatted} ({percentage:.2}%)"));
                        ui.label(format!("Self: {self_formatted}"));
                        ui.label(format!("Depth: {}", frame.depth));
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Click to zoom")
                                .color(text_col.gamma_multiply(0.5))
                                .size(10.0),
                        );
                    });
                }
            }

            ui.add_space(8.0);
        });
    }
}

/// Populate demo data for the flamegraph
///
/// Creates a realistic CPU profile flamegraph simulating a web server workload
/// with enough frames to trigger GPU rendering (>200 frames)
pub fn populate_flamegraph_demo(flamegraph: &mut FlamegraphViz, query: &str) {
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    let total_samples = 10_000_000u64; // 10 seconds in microseconds
    let mut frames = Vec::new();

    // Helper to generate color seed from name
    let color_seed_for = |name: &str| -> f32 {
        let h = name
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        (h % 1000) as f32 / 1000.0
    };

    // Helper to add a frame with variation
    let mut add_frame = |name: &str, depth: usize, x_start: f32, x_end: f32| {
        let variation = ((hash.wrapping_add(depth as u64) % 10) as f32 - 5.0) / 100.0;
        let adjusted_start = (x_start + variation * 0.02).clamp(0.0, 1.0);
        let adjusted_end = (x_end + variation * 0.01).clamp(adjusted_start + 0.001, 1.0);

        let value = ((adjusted_end - adjusted_start) * total_samples as f32) as u64;
        let self_value = value / (depth as u64 + 2);

        frames.push(FlameFrame {
            x_start: adjusted_start,
            x_end: adjusted_end,
            depth,
            color_seed: color_seed_for(name),
            name: name.to_string(),
            value,
            self_value,
        });
    };

    // Root frames
    add_frame("main", 0, 0.0, 1.0);
    add_frame("tokio::runtime::Runtime::block_on", 1, 0.0, 1.0);
    add_frame("tokio::runtime::enter::enter", 2, 0.0, 0.98);

    // HTTP Server branch (0.0 - 0.45)
    add_frame("hyper::server::Server::serve", 3, 0.0, 0.45);
    add_frame("hyper::server::conn::Http::serve_connection", 4, 0.0, 0.44);

    // Request handling (multiple requests simulated)
    let request_ranges = [
        (0.0, 0.08),
        (0.08, 0.15),
        (0.15, 0.22),
        (0.22, 0.28),
        (0.28, 0.35),
        (0.35, 0.44),
    ];

    for (i, (start, end)) in request_ranges.iter().enumerate() {
        let base = format!("request_{i}");
        add_frame(&format!("handle_request::{base}"), 5, *start, *end);
        add_frame(
            &format!("tower::ServiceExt::ready::{base}"),
            6,
            *start,
            *end - 0.005,
        );

        // Parse phase
        let parse_end = start + (end - start) * 0.15;
        add_frame(
            &format!("http::request::parse::{base}"),
            7,
            *start,
            parse_end,
        );
        add_frame(
            &format!("httparse::Request::parse::{base}"),
            8,
            *start,
            parse_end - 0.002,
        );
        add_frame(
            &format!("httparse::parse_headers::{base}"),
            9,
            *start,
            parse_end - 0.004,
        );

        // Auth phase
        let auth_start = parse_end;
        let auth_end = start + (end - start) * 0.35;
        add_frame(
            &format!("auth::middleware::validate::{base}"),
            7,
            auth_start,
            auth_end,
        );
        add_frame(
            &format!("jsonwebtoken::decode::{base}"),
            8,
            auth_start,
            auth_end - 0.003,
        );
        add_frame(
            &format!("jsonwebtoken::crypto::verify::{base}"),
            9,
            auth_start + 0.002,
            auth_end - 0.005,
        );
        add_frame(
            &format!("ring::signature::verify::{base}"),
            10,
            auth_start + 0.003,
            auth_end - 0.006,
        );

        // DB query phase
        let db_start = auth_end;
        let db_end = start + (end - start) * 0.75;
        add_frame(
            &format!("sqlx::query::Query::fetch::{base}"),
            7,
            db_start,
            db_end,
        );
        add_frame(
            &format!("sqlx::pool::Pool::acquire::{base}"),
            8,
            db_start,
            db_start + 0.005,
        );
        add_frame(
            &format!("sqlx::postgres::connection::execute::{base}"),
            8,
            db_start + 0.005,
            db_end - 0.003,
        );
        add_frame(
            &format!("tokio_postgres::Client::query::{base}"),
            9,
            db_start + 0.006,
            db_end - 0.005,
        );
        add_frame(
            &format!("tokio_postgres::codec::encode::{base}"),
            10,
            db_start + 0.007,
            db_start + 0.015,
        );
        add_frame(
            &format!("tokio::net::TcpStream::write::{base}"),
            10,
            db_start + 0.015,
            db_start + 0.020,
        );
        add_frame(
            &format!("tokio::net::TcpStream::read::{base}"),
            10,
            db_start + 0.020,
            db_end - 0.010,
        );
        add_frame(
            &format!("tokio_postgres::codec::decode::{base}"),
            10,
            db_end - 0.010,
            db_end - 0.006,
        );

        // Serialize response
        let ser_start = db_end;
        let ser_end = *end - 0.005;
        add_frame(
            &format!("serde_json::to_vec::{base}"),
            7,
            ser_start,
            ser_end,
        );
        add_frame(
            &format!("serde::ser::Serialize::serialize::{base}"),
            8,
            ser_start,
            ser_end - 0.002,
        );
        add_frame(
            &format!("serde_json::ser::Serializer::serialize_struct::{base}"),
            9,
            ser_start + 0.001,
            ser_end - 0.003,
        );
    }

    // Metrics processing branch (0.45 - 0.70)
    add_frame("metrics::process::run", 3, 0.45, 0.70);
    add_frame("metrics::collector::Collector::collect", 4, 0.45, 0.68);

    // Multiple metric collection cycles
    let metric_ranges = [(0.45, 0.52), (0.52, 0.58), (0.58, 0.64), (0.64, 0.68)];
    for (i, (start, end)) in metric_ranges.iter().enumerate() {
        let base = format!("cycle_{i}");
        add_frame(&format!("metrics::gather::{base}"), 5, *start, *end);
        add_frame(
            &format!("prometheus::Registry::gather::{base}"),
            6,
            *start,
            *end - 0.005,
        );

        // Different metric types
        let counter_end = start + (end - start) * 0.3;
        add_frame(
            &format!("prometheus::Counter::get::{base}"),
            7,
            *start,
            counter_end,
        );
        add_frame(
            &format!("std::sync::atomic::AtomicU64::load::{base}"),
            8,
            *start,
            counter_end - 0.002,
        );

        let gauge_start = counter_end;
        let gauge_end = start + (end - start) * 0.5;
        add_frame(
            &format!("prometheus::Gauge::get::{base}"),
            7,
            gauge_start,
            gauge_end,
        );

        let hist_start = gauge_end;
        let hist_end = *end - 0.006;
        add_frame(
            &format!("prometheus::Histogram::observe::{base}"),
            7,
            hist_start,
            hist_end,
        );
        add_frame(
            &format!("prometheus::core::bucket_search::{base}"),
            8,
            hist_start,
            hist_start + 0.008,
        );
        add_frame(
            &format!("prometheus::core::Histogram::inner::{base}"),
            8,
            hist_start + 0.008,
            hist_end - 0.003,
        );
    }

    // Background tasks branch (0.70 - 0.98)
    add_frame("background::tasks::run", 3, 0.70, 0.98);

    // GC branch
    add_frame("gc::Collector::collect", 4, 0.70, 0.80);
    add_frame("gc::mark_sweep::mark", 5, 0.70, 0.74);
    add_frame("gc::mark_sweep::trace_roots", 6, 0.70, 0.72);
    add_frame("gc::mark_sweep::trace_stack", 7, 0.70, 0.71);
    add_frame("gc::mark_sweep::trace_heap", 7, 0.71, 0.72);
    add_frame("gc::mark_sweep::sweep", 5, 0.74, 0.80);
    add_frame("gc::allocator::free_block", 6, 0.74, 0.77);
    add_frame("gc::allocator::coalesce", 6, 0.77, 0.80);

    // Cache eviction
    add_frame("cache::lru::LruCache::evict", 4, 0.80, 0.88);
    for i in 0..6 {
        let start = 0.80 + (i as f32) * 0.012;
        let end = start + 0.011;
        add_frame(
            &format!("cache::lru::Node::remove::entry_{i}"),
            5,
            start,
            end,
        );
        add_frame(
            &format!("std::collections::HashMap::remove::entry_{i}"),
            6,
            start,
            end - 0.002,
        );
        add_frame(
            &format!("hashbrown::raw::RawTable::remove::entry_{i}"),
            7,
            start + 0.001,
            end - 0.003,
        );
    }

    // Logging flush
    add_frame("tracing::subscriber::flush", 4, 0.88, 0.95);
    add_frame("tracing_subscriber::fmt::Layer::flush", 5, 0.88, 0.94);
    for i in 0..4 {
        let start = 0.88 + (i as f32) * 0.014;
        let end = start + 0.013;
        add_frame(
            &format!("std::io::Write::write_all::batch_{i}"),
            6,
            start,
            end,
        );
        add_frame(
            &format!("std::io::BufWriter::flush::batch_{i}"),
            7,
            start,
            end - 0.002,
        );
    }

    // Final cleanup
    add_frame("tokio::runtime::park", 4, 0.95, 0.98);
    add_frame("std::thread::park", 5, 0.95, 0.97);

    flamegraph.set_frames(frames, total_samples);
    flamegraph.set_profile_type(if hash % 2 == 0 {
        ProfileType::Cpu
    } else {
        ProfileType::Memory
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flamegraph_creation() {
        let fg = FlamegraphViz::new("test");
        assert_eq!(fg.title, "test");
        assert!(fg.frames.is_empty());
    }

    #[test]
    fn test_set_frames() {
        let mut fg = FlamegraphViz::new("test");
        let frames = vec![
            FlameFrame {
                x_start: 0.0,
                x_end: 1.0,
                depth: 0,
                color_seed: 0.5,
                name: "main".to_string(),
                value: 1000,
                self_value: 100,
            },
            FlameFrame {
                x_start: 0.1,
                x_end: 0.9,
                depth: 1,
                color_seed: 0.3,
                name: "child".to_string(),
                value: 800,
                self_value: 800,
            },
        ];
        fg.set_frames(frames, 1000);

        assert_eq!(fg.frames.len(), 2);
        assert_eq!(fg.max_depth, 2);
        assert_eq!(fg.total_value, 1000);
    }

    #[test]
    fn test_zoom() {
        let mut fg = FlamegraphViz::new("test");
        fg.set_frames(
            vec![FlameFrame {
                x_start: 0.2,
                x_end: 0.8,
                depth: 0,
                color_seed: 0.5,
                name: "test".to_string(),
                value: 100,
                self_value: 100,
            }],
            100,
        );

        assert_eq!(fg.zoom, (0.0, 1.0));
        fg.zoom_to_frame(0);
        assert_eq!(fg.zoom, (0.2, 0.8));
        fg.reset_zoom();
        assert_eq!(fg.zoom, (0.0, 1.0));
    }

    #[test]
    fn test_populate_demo() {
        let mut fg = FlamegraphViz::new("demo");
        populate_flamegraph_demo(&mut fg, "test query");

        assert!(!fg.frames.is_empty());
        assert!(fg.max_depth > 0);
        assert!(fg.total_value > 0);
    }

    #[test]
    fn test_set_frames_calculates_max_depth() {
        let mut fg = FlamegraphViz::new("test");

        // Test max depth calculation
        let frames: Vec<FlameFrame> = (0..10)
            .map(|i| FlameFrame {
                x_start: 0.0,
                x_end: 1.0,
                depth: i,
                color_seed: 0.5,
                name: format!("frame_{i}"),
                value: 100,
                self_value: 10,
            })
            .collect();
        fg.set_frames(frames, 1000);
        assert_eq!(fg.max_depth, 10); // 0-9 depths = 10 levels
    }
}
