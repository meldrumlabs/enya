//! Pie chart visualization - circular segments for showing proportions

use egui::{Color32, Pos2, RichText, Stroke, Vec2};

use crate::ui::theme::AppTheme;

use super::{VIZ_PADDING_BOTTOM, VIZ_PADDING_TOP};
use crate::components::util::id_generator::next_id_usize;

/// A single segment in a pie chart
#[derive(Debug, Clone)]
pub struct Segment {
    /// Label for this segment (e.g., "us-east-1", "200 OK")
    pub label: String,
    /// Value of this segment
    pub value: f64,
    /// Optional custom color (uses theme chart palette if None)
    pub color: Option<Color32>,
}

impl Segment {
    pub fn new(label: impl Into<String>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
            color: None,
        }
    }

    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

/// A pie chart visualization for showing proportional data
pub struct PieChartViz {
    /// Unique identifier
    #[allow(dead_code)]
    id: usize,
    /// The metric name being displayed
    pub(crate) metric_name: String,
    /// Segments to display
    segments: Vec<Segment>,
    /// Current theme
    pub(crate) theme: AppTheme,
    /// Title (shown in tab)
    title: String,
    /// Unit suffix for values (e.g., "ms", "req/s", "%")
    unit: String,
    /// Whether to show the legend
    show_legend: bool,
    /// Whether segments are sorted by value (descending)
    sorted: bool,
    /// Index of hovered segment (for interaction)
    hovered_segment: Option<usize>,
}

impl Default for PieChartViz {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl PieChartViz {
    pub fn new(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        Self {
            id: next_id_usize(),
            title: name.clone(),
            metric_name: name,
            segments: Vec::new(),
            theme: AppTheme::default(),
            unit: String::new(),
            show_legend: true,
            sorted: true,
            hovered_segment: None,
        }
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    pub fn set_unit(&mut self, unit: impl Into<String>) {
        self.unit = unit.into();
    }

    pub fn set_metric_name(&mut self, name: impl Into<String>) {
        self.metric_name = name.into();
        self.title = self.metric_name.clone();
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    pub fn add_segment(&mut self, segment: Segment) {
        self.segments.push(segment);
    }

    pub fn set_segments(&mut self, segments: Vec<Segment>) {
        self.segments = segments;
    }

    pub fn clear(&mut self) {
        self.segments.clear();
        self.hovered_segment = None;
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub fn set_show_legend(&mut self, show: bool) {
        self.show_legend = show;
    }

    pub fn set_sorted(&mut self, sorted: bool) {
        self.sorted = sorted;
    }

    /// Get segment indices ordered for display
    fn get_display_order(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..self.segments.len()).collect();
        if self.sorted {
            indices.sort_by(|&a, &b| {
                self.segments[b]
                    .value
                    .partial_cmp(&self.segments[a].value)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        indices
    }

    /// Format a value for display
    fn format_value(value: f64) -> String {
        if value.abs() >= 1_000_000.0 {
            format!("{:.1}M", value / 1_000_000.0)
        } else if value.abs() >= 1_000.0 {
            format!("{:.1}K", value / 1_000.0)
        } else if value.fract() == 0.0 {
            format!("{value:.0}")
        } else {
            format!("{value:.1}")
        }
    }

    /// Render the pie chart
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_col = self.theme.text_primary();

        let available_width = ui.available_width();
        let available_height = ui.available_height();

        // Scale based on available space
        let base_size = available_width.min(available_height);
        let scale_factor = (base_size / 300.0).clamp(0.6, 2.0);

        let title_size = (14.0 * scale_factor).clamp(11.0, 20.0);
        let label_size = (11.0 * scale_factor).clamp(9.0, 15.0);
        let percent_size = (10.0 * scale_factor).clamp(8.0, 13.0);

        ui.vertical(|ui| {
            ui.add_space(VIZ_PADDING_TOP);

            // Title
            if !self.title.is_empty() && self.title != "Untitled" {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&self.title)
                            .color(text_col)
                            .size(title_size)
                            .strong(),
                    );
                });
                ui.add_space(8.0);
            }

            if self.segments.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("No data")
                            .color(text_col.gamma_multiply(0.4))
                            .size(title_size),
                    );
                });
                return;
            }

            let total: f64 = self.segments.iter().map(|s| s.value.max(0.0)).sum();
            if total <= 0.0 {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("No data")
                            .color(text_col.gamma_multiply(0.4))
                            .size(title_size),
                    );
                });
                return;
            }

            let display_order = self.get_display_order();

            // Available height for the pie + legend row
            let content_height =
                (available_height - VIZ_PADDING_TOP - VIZ_PADDING_BOTTOM - title_size - 16.0)
                    .max(60.0);

            // Pie sizes based on the full content height
            let max_radius = (available_width.min(content_height) / 2.0 - 12.0).max(20.0);
            let radius = max_radius.min(150.0 * scale_factor);
            let inner_radius = radius * 0.55; // Donut hole
            let pie_size = (radius + 16.0) * 2.0;

            // Side legend to the right of the pie, centered as a group
            let legend_width = if self.show_legend {
                (140.0 * scale_factor).min(available_width - pie_size - 16.0)
            } else {
                0.0
            };
            let content_width = pie_size
                + if self.show_legend {
                    12.0 + legend_width
                } else {
                    0.0
                };
            let left_pad = ((available_width - content_width) / 2.0).max(0.0);

            ui.horizontal(|ui| {
                ui.add_space(left_pad);

                self.render_pie(
                    ui,
                    pie_size,
                    radius,
                    inner_radius,
                    total,
                    &display_order,
                    text_col,
                    percent_size,
                );

                if self.show_legend && legend_width > 40.0 {
                    ui.add_space(12.0);

                    // Legend height matches the pie so it scrolls if needed
                    ui.vertical(|ui| {
                        egui::ScrollArea::vertical()
                            .max_height(pie_size)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_max_width(legend_width);
                                self.render_legend(
                                    ui,
                                    total,
                                    &display_order,
                                    text_col,
                                    label_size,
                                    percent_size,
                                    scale_factor,
                                );
                            });
                    });
                }
            });

            ui.add_space(VIZ_PADDING_BOTTOM);
        });
    }

    /// Render the pie/donut ring
    #[allow(clippy::too_many_arguments)]
    fn render_pie(
        &mut self,
        ui: &mut egui::Ui,
        pie_size: f32,
        radius: f32,
        inner_radius: f32,
        total: f64,
        display_order: &[usize],
        text_col: Color32,
        percent_size: f32,
    ) {
        let (response, painter) =
            ui.allocate_painter(Vec2::new(pie_size, pie_size), egui::Sense::hover());
        let center = response.rect.center();

        // Detect hover
        let hover_pos = response.hover_pos();
        let mut new_hovered: Option<usize> = None;

        // Draw segments
        let num_arc_points = 64;
        let gap_radians = 0.02_f32; // Small gap between segments for premium feel
        let mut start_angle = -std::f32::consts::FRAC_PI_2; // Start at 12 o'clock

        for (display_idx, &orig_idx) in display_order.iter().enumerate() {
            let seg = &self.segments[orig_idx];
            let fraction = (seg.value.max(0.0) / total) as f32;
            if fraction <= 0.0 {
                continue;
            }

            let sweep = fraction * std::f32::consts::TAU - gap_radians;
            if sweep <= 0.0 {
                start_angle += fraction * std::f32::consts::TAU;
                continue;
            }

            let seg_start = start_angle + gap_radians / 2.0;
            let seg_end = seg_start + sweep;

            let color = seg
                .color
                .unwrap_or_else(|| self.theme.chart_color(display_idx));

            // Check if mouse is in this segment
            let is_hovered = if let Some(pos) = hover_pos {
                let dx = pos.x - center.x;
                let dy = pos.y - center.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist >= inner_radius && dist <= radius {
                    let mut angle = dy.atan2(dx);
                    // Normalize angle to match our start (-PI/2)
                    if angle < -std::f32::consts::FRAC_PI_2 {
                        angle += std::f32::consts::TAU;
                    }
                    // Normalize seg_start/seg_end
                    let norm_start = normalize_angle(seg_start);
                    let norm_end = normalize_angle(seg_end);
                    let norm_angle = normalize_angle(angle);

                    if norm_start <= norm_end {
                        norm_angle >= norm_start && norm_angle <= norm_end
                    } else {
                        // Wraps around
                        norm_angle >= norm_start || norm_angle <= norm_end
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if is_hovered {
                new_hovered = Some(orig_idx);
            }

            let seg_center = center;

            // Build convex pie wedge(s). A wedge > 180° is concave,
            // so split into sub-wedges of at most PI radians each.
            let fill = if is_hovered {
                lighten(color, 0.15)
            } else {
                color
            };

            let max_sweep = std::f32::consts::PI;
            let num_splits = ((sweep / max_sweep).ceil() as usize).max(1);
            let split_sweep = sweep / num_splits as f32;
            let points_per_split = (num_arc_points / num_splits).max(8);

            for s in 0..num_splits {
                let sub_start = seg_start + s as f32 * split_sweep;
                let sub_end = sub_start + split_sweep;

                let mut wedge = Vec::with_capacity(points_per_split + 3);
                wedge.push(seg_center);
                for i in 0..=points_per_split {
                    let t = i as f32 / points_per_split as f32;
                    let angle = sub_start + t * (sub_end - sub_start);
                    wedge.push(Pos2::new(
                        seg_center.x + radius * angle.cos(),
                        seg_center.y + radius * angle.sin(),
                    ));
                }

                painter.add(egui::Shape::convex_polygon(wedge, fill, Stroke::NONE));
            }
        }

        // Draw center circle (donut hole) matching background
        let bg = self.theme.bg_base();
        painter.circle_filled(center, inner_radius - 1.0, bg);

        // Show hovered segment info in the center
        self.hovered_segment = new_hovered;
        if let Some(idx) = self.hovered_segment {
            if let Some(seg) = self.segments.get(idx) {
                let pct = (seg.value / total) * 100.0;
                let pct_text = format!("{pct:.1}%");
                let val_text = if self.unit.is_empty() {
                    Self::format_value(seg.value)
                } else {
                    format!("{} {}", Self::format_value(seg.value), self.unit)
                };

                // Percentage in center
                painter.text(
                    center + Vec2::new(0.0, -percent_size * 0.6),
                    egui::Align2::CENTER_CENTER,
                    &pct_text,
                    egui::FontId::proportional(percent_size * 1.4),
                    text_col,
                );
                // Value below
                painter.text(
                    center + Vec2::new(0.0, percent_size * 0.8),
                    egui::Align2::CENTER_CENTER,
                    &val_text,
                    egui::FontId::proportional(percent_size * 0.9),
                    text_col.gamma_multiply(0.6),
                );
            }
        } else {
            // Show total in center when nothing hovered
            let total_text = if self.unit.is_empty() {
                Self::format_value(total)
            } else {
                format!("{} {}", Self::format_value(total), self.unit)
            };
            painter.text(
                center + Vec2::new(0.0, -percent_size * 0.4),
                egui::Align2::CENTER_CENTER,
                "Total",
                egui::FontId::proportional(percent_size * 0.85),
                text_col.gamma_multiply(0.5),
            );
            painter.text(
                center + Vec2::new(0.0, percent_size * 0.7),
                egui::Align2::CENTER_CENTER,
                &total_text,
                egui::FontId::proportional(percent_size * 1.2),
                text_col,
            );
        }

        // Only repaint when hover state changes, not every frame
        if new_hovered != self.hovered_segment {
            ui.ctx().request_repaint();
        }
    }

    /// Render the legend
    #[allow(clippy::too_many_arguments)]
    fn render_legend(
        &self,
        ui: &mut egui::Ui,
        total: f64,
        display_order: &[usize],
        text_col: Color32,
        label_size: f32,
        percent_size: f32,
        scale_factor: f32,
    ) {
        let swatch_size = (10.0 * scale_factor).clamp(8.0, 14.0);

        for (display_idx, &orig_idx) in display_order.iter().enumerate() {
            let seg = &self.segments[orig_idx];
            let color = seg
                .color
                .unwrap_or_else(|| self.theme.chart_color(display_idx));
            let pct = if total > 0.0 {
                (seg.value / total) * 100.0
            } else {
                0.0
            };

            let is_hovered = self.hovered_segment == Some(orig_idx);

            ui.horizontal(|ui| {
                ui.add_space(4.0);

                // Color swatch — vertically centered with the two text lines
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::splat(swatch_size), egui::Sense::hover());
                let rounding = swatch_size * 0.25;
                ui.painter().rect_filled(rect, rounding, color);

                ui.add_space(4.0);

                // Label on top, percentage below
                let alpha = if is_hovered { 1.0 } else { 0.8 };
                let label_text = if seg.label.len() > 16 {
                    format!("{}...", &seg.label[..14])
                } else {
                    seg.label.clone()
                };
                ui.vertical(|ui| {
                    ui.set_row_height(label_size + 2.0);
                    ui.label(
                        RichText::new(label_text)
                            .color(text_col.gamma_multiply(alpha))
                            .size(label_size),
                    );
                    ui.label(
                        RichText::new(format!("{pct:.1}%"))
                            .color(text_col.gamma_multiply(0.5))
                            .size(percent_size),
                    );
                });
            });

            ui.add_space(2.0);
        }
    }
}

/// Normalize an angle to [0, TAU)
fn normalize_angle(angle: f32) -> f32 {
    let mut a = angle % std::f32::consts::TAU;
    if a < 0.0 {
        a += std::f32::consts::TAU;
    }
    a
}

/// Lighten a color by a fraction
fn lighten(color: Color32, amount: f32) -> Color32 {
    let r = color.r() as f32;
    let g = color.g() as f32;
    let b = color.b() as f32;
    Color32::from_rgb(
        (r + (255.0 - r) * amount) as u8,
        (g + (255.0 - g) * amount) as u8,
        (b + (255.0 - b) * amount) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pie_chart_format_value() {
        assert_eq!(PieChartViz::format_value(75.0), "75");
        assert_eq!(PieChartViz::format_value(1234.0), "1.2K");
        assert_eq!(PieChartViz::format_value(1_234_567.0), "1.2M");
        assert_eq!(PieChartViz::format_value(42.5), "42.5");
    }

    #[test]
    fn test_pie_chart_segments() {
        let mut pie = PieChartViz::new("test");
        pie.add_segment(Segment::new("A", 30.0));
        pie.add_segment(Segment::new("B", 50.0));
        pie.add_segment(Segment::new("C", 20.0));

        assert_eq!(pie.segments().len(), 3);

        // With sorting enabled (default), highest first
        let order = pie.get_display_order();
        assert_eq!(pie.segments()[order[0]].label, "B"); // 50
        assert_eq!(pie.segments()[order[1]].label, "A"); // 30
        assert_eq!(pie.segments()[order[2]].label, "C"); // 20

        // With sorting disabled
        pie.set_sorted(false);
        let order = pie.get_display_order();
        assert_eq!(pie.segments()[order[0]].label, "A");
        assert_eq!(pie.segments()[order[1]].label, "B");
        assert_eq!(pie.segments()[order[2]].label, "C");
    }

    #[test]
    fn test_pie_chart_clear() {
        let mut pie = PieChartViz::new("test");
        pie.add_segment(Segment::new("A", 10.0));
        assert_eq!(pie.segments().len(), 1);

        pie.clear();
        assert_eq!(pie.segments().len(), 0);
    }
}
