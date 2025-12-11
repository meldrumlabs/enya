//! Heatmap visualization
//!
//! This module provides a heatmap visualization for displaying 2D grids of values.
//! Currently uses CPU rendering with a Viridis-style color palette.
//! GPU acceleration via wgpu can be added later using the companion `heatmap.wgsl` shader.

use std::sync::atomic::{AtomicUsize, Ordering};

use egui::{Color32, RichText, Stroke, StrokeKind};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;

/// Global counter for unique heatmap IDs
static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

/// A single cell in the heatmap
#[derive(Debug, Clone, Copy)]
pub struct HeatmapCell {
    /// Column index (x)
    pub col: usize,
    /// Row index (y)
    pub row: usize,
    /// Value (0.0 to 1.0, normalized)
    pub value: f32,
}

/// Row and column labels for the heatmap
#[derive(Debug, Clone, Default)]
pub struct HeatmapLabels {
    /// Column labels (x-axis, e.g., time buckets or categories)
    pub columns: Vec<String>,
    /// Row labels (y-axis, e.g., endpoints or services)
    pub rows: Vec<String>,
}

/// Heatmap visualization
///
/// The heatmap displays a 2D grid of colored cells where color intensity
/// represents the value. This is useful for:
/// - Error rates by endpoint over time
/// - Latency distributions
/// - Request counts by service
pub struct HeatmapViz {
    /// Unique identifier
    id: usize,
    /// The metric name being displayed
    pub(crate) metric_name: String,
    /// Title (shown in tab)
    title: String,
    /// Grid dimensions (cols, rows)
    grid_size: (usize, usize),
    /// Cell data
    cells: Vec<HeatmapCell>,
    /// Labels for rows and columns
    labels: HeatmapLabels,
    /// Current theme
    pub(crate) theme: AppTheme,
    /// Value range for normalization (min, max)
    value_range: (f64, f64),
    /// Whether to show a color scale legend
    show_legend: bool,
}

impl Default for HeatmapViz {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl HeatmapViz {
    /// Create a new heatmap visualization
    pub fn new(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            title: name.clone(),
            metric_name: name,
            grid_size: (24, 10), // Default: 24 columns (hours), 10 rows
            cells: Vec::new(),
            labels: HeatmapLabels::default(),
            theme: AppTheme::default(),
            value_range: (0.0, 1.0),
            show_legend: true,
        }
    }

    /// Get the unique identifier
    pub fn id(&self) -> usize {
        self.id
    }

    /// Set the metric name
    pub fn set_metric_name(&mut self, name: impl Into<String>) {
        self.metric_name = name.into();
        self.title = self.metric_name.clone();
    }

    /// Set the title
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the grid dimensions
    pub fn set_grid_size(&mut self, cols: usize, rows: usize) {
        self.grid_size = (cols, rows);
    }

    /// Set cell data from a 2D array of values
    pub fn set_data(&mut self, data: Vec<Vec<f64>>) {
        self.cells.clear();

        // Find min/max for normalization
        let mut min_val = f64::INFINITY;
        let mut max_val = f64::NEG_INFINITY;

        for row in &data {
            for &val in row {
                min_val = min_val.min(val);
                max_val = max_val.max(val);
            }
        }

        self.value_range = (min_val, max_val);
        let range = (max_val - min_val).max(0.001);

        // Convert to cells
        for (row_idx, row) in data.iter().enumerate() {
            for (col_idx, &val) in row.iter().enumerate() {
                let normalized = ((val - min_val) / range) as f32;
                self.cells.push(HeatmapCell {
                    col: col_idx,
                    row: row_idx,
                    value: normalized,
                });
            }
        }

        self.grid_size = (data.first().map(|r| r.len()).unwrap_or(0), data.len());
    }

    /// Set labels for rows and columns
    pub fn set_labels(&mut self, labels: HeatmapLabels) {
        self.labels = labels;
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.cells.clear();
    }

    /// Get a color for a normalized value (0-1) - used for CPU fallback
    fn get_color(value: f32) -> Color32 {
        // Viridis-inspired palette
        let colors = [
            Color32::from_rgb(68, 1, 84),    // Dark purple
            Color32::from_rgb(72, 36, 117),  // Purple
            Color32::from_rgb(65, 68, 135),  // Blue-purple
            Color32::from_rgb(48, 104, 142), // Blue
            Color32::from_rgb(32, 144, 141), // Teal
            Color32::from_rgb(52, 183, 121), // Green
            Color32::from_rgb(144, 215, 67), // Yellow-green
            Color32::from_rgb(253, 231, 37), // Yellow
        ];

        let t = value.clamp(0.0, 1.0);

        // Handle edge case at exactly 1.0
        if t >= 1.0 {
            return colors[7];
        }

        let segment = t * 7.0;
        let idx = segment.floor() as usize;
        let frac = segment - segment.floor();

        let c1 = colors[idx];
        let c2 = colors[idx + 1];

        // Linear interpolation between colors
        Color32::from_rgb(
            (c1.r() as f32 + (c2.r() as f32 - c1.r() as f32) * frac) as u8,
            (c1.g() as f32 + (c2.g() as f32 - c1.g() as f32) * frac) as u8,
            (c1.b() as f32 + (c2.b() as f32 - c1.b() as f32) * frac) as u8,
        )
    }

    /// Render the heatmap (CPU fallback when GPU not available)
    fn show_cpu_fallback(&self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);

        ui.vertical(|ui| {
            ui.add_space(8.0);

            // Title
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&self.metric_name)
                        .color(text_col.gamma_multiply(0.6))
                        .size(13.0),
                );
            });

            ui.add_space(8.0);

            if self.cells.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("No data")
                            .color(text_col.gamma_multiply(0.4))
                            .size(14.0),
                    );
                });
                return;
            }

            // Calculate available space
            let available = ui.available_size();
            let label_space = 60.0; // Space for row labels
            let legend_space = if self.show_legend { 50.0 } else { 0.0 };

            let chart_width = available.x - label_space - legend_space - 16.0;
            let chart_height = (available.y - 40.0).min(400.0);

            let (cols, rows) = self.grid_size;
            if cols == 0 || rows == 0 {
                return;
            }

            let cell_width = chart_width / cols as f32;
            let cell_height = chart_height / rows as f32;
            let gap = 1.0;

            ui.horizontal(|ui| {
                // Row labels
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    for row_idx in 0..rows {
                        let label = self
                            .labels
                            .rows
                            .get(row_idx)
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        let truncated = if label.len() > 8 {
                            format!("{}...", &label[..5])
                        } else {
                            label.to_string()
                        };
                        ui.add_sized(
                            [label_space - 8.0, cell_height],
                            egui::Label::new(
                                RichText::new(truncated)
                                    .color(text_col.gamma_multiply(0.5))
                                    .size(10.0),
                            ),
                        );
                    }
                });

                // Heatmap grid
                let (response, painter) = ui
                    .allocate_painter(egui::vec2(chart_width, chart_height), egui::Sense::hover());
                let rect = response.rect;

                // Draw cells
                for cell in &self.cells {
                    let x = rect.left() + cell.col as f32 * cell_width + gap / 2.0;
                    let y = rect.top() + cell.row as f32 * cell_height + gap / 2.0;

                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(x, y),
                        egui::vec2(cell_width - gap, cell_height - gap),
                    );

                    let color = Self::get_color(cell.value);
                    painter.rect_filled(cell_rect, 2.0, color);
                }

                // Draw border
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.0, palette::border::SUBTLE),
                    StrokeKind::Outside,
                );

                // Color scale legend
                if self.show_legend {
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        let legend_height = chart_height;
                        let legend_width = 20.0;

                        let (legend_response, legend_painter) = ui.allocate_painter(
                            egui::vec2(legend_width, legend_height),
                            egui::Sense::hover(),
                        );
                        let legend_rect = legend_response.rect;

                        // Draw gradient
                        let steps = 50;
                        let step_height = legend_height / steps as f32;
                        for i in 0..steps {
                            let t = 1.0 - (i as f32 / steps as f32);
                            let y = legend_rect.top() + i as f32 * step_height;
                            let step_rect = egui::Rect::from_min_size(
                                egui::pos2(legend_rect.left(), y),
                                egui::vec2(legend_width, step_height + 1.0),
                            );
                            legend_painter.rect_filled(step_rect, 0.0, Self::get_color(t));
                        }

                        // Min/max labels
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("{:.1}", self.value_range.1))
                                .color(text_col.gamma_multiply(0.5))
                                .size(9.0),
                        );
                        ui.add_space(legend_height - 30.0);
                        ui.label(
                            RichText::new(format!("{:.1}", self.value_range.0))
                                .color(text_col.gamma_multiply(0.5))
                                .size(9.0),
                        );
                    });
                }
            });

            // Column labels
            ui.horizontal(|ui| {
                ui.add_space(label_space);
                for col_idx in 0..cols {
                    let label = self
                        .labels
                        .columns
                        .get(col_idx)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    ui.add_sized(
                        [cell_width, 16.0],
                        egui::Label::new(
                            RichText::new(label)
                                .color(text_col.gamma_multiply(0.5))
                                .size(9.0),
                        ),
                    );
                }
            });
        });
    }

    /// Render the heatmap
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.show_cpu_fallback(ui);
    }
}

/// Populate demo data for the heatmap
pub fn populate_heatmap_demo(heatmap: &mut HeatmapViz, query: &str) {
    // Generate demo data based on query hash
    let hash = query
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_add(b as u64));

    let cols = 24; // 24 hours
    let rows = 8; // 8 endpoints/services

    let mut data = Vec::with_capacity(rows);

    for row in 0..rows {
        let mut row_data = Vec::with_capacity(cols);
        for col in 0..cols {
            // Generate interesting patterns
            let base = ((hash + row as u64 * 13 + col as u64 * 7) % 100) as f64 / 100.0;

            // Add time-based pattern (higher during business hours)
            let hour_factor = if (9..18).contains(&col) { 1.5 } else { 0.5 };

            // Add some row variation (some services are busier)
            let row_factor = 1.0 + (row as f64 * 0.1);

            // Add some noise
            let noise = ((hash.wrapping_add(row as u64 * col as u64) % 30) as f64 - 15.0) / 100.0;

            let value = (base * hour_factor * row_factor + noise).clamp(0.0, 1.0);
            row_data.push(value * 100.0); // Scale to 0-100 for error rate percentage
        }
        data.push(row_data);
    }

    heatmap.set_data(data);

    // Set labels
    let column_labels: Vec<String> = (0..cols).map(|h| format!("{h:02}")).collect();
    let row_labels: Vec<String> = vec![
        "api-gateway".into(),
        "auth-service".into(),
        "user-service".into(),
        "order-service".into(),
        "payment-service".into(),
        "inventory-svc".into(),
        "notification".into(),
        "analytics".into(),
    ];

    heatmap.set_labels(HeatmapLabels {
        columns: column_labels,
        rows: row_labels,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heatmap_creation() {
        let heatmap = HeatmapViz::new("test_metric");
        assert_eq!(heatmap.metric_name, "test_metric");
        assert!(heatmap.cells.is_empty());
    }

    #[test]
    fn test_heatmap_set_data() {
        let mut heatmap = HeatmapViz::new("test");
        let data = vec![vec![0.0, 0.5, 1.0], vec![0.25, 0.75, 0.5]];
        heatmap.set_data(data);

        assert_eq!(heatmap.grid_size, (3, 2));
        assert_eq!(heatmap.cells.len(), 6);
        assert_eq!(heatmap.value_range, (0.0, 1.0));
    }

    #[test]
    fn test_get_color() {
        // Test color at boundaries
        let color_0 = HeatmapViz::get_color(0.0);
        let color_1 = HeatmapViz::get_color(1.0);

        // Should be different colors
        assert_ne!(color_0, color_1);

        // Color at 0 should be dark purple-ish
        assert!(color_0.r() < 100);

        // Color at 1 should be yellow-ish
        assert!(color_1.r() > 200);
        assert!(color_1.g() > 200);
    }

    #[test]
    fn test_populate_demo() {
        let mut heatmap = HeatmapViz::new("demo");
        populate_heatmap_demo(&mut heatmap, "test query");

        assert_eq!(heatmap.grid_size, (24, 8));
        assert!(!heatmap.cells.is_empty());
        assert!(!heatmap.labels.columns.is_empty());
        assert!(!heatmap.labels.rows.is_empty());
    }
}
