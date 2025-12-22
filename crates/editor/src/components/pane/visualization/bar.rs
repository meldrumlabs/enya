//! Bar chart visualization - horizontal bars for comparing values

use egui::{Color32, RichText};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;

use super::{VIZ_PADDING_BOTTOM, VIZ_PADDING_TOP};
use crate::components::util::id_generator::next_id_usize;

/// A single bar in a bar chart
#[derive(Debug, Clone)]
pub struct Bar {
    /// Label for this bar (e.g., "server1", "us-east")
    pub label: String,
    /// Value of this bar
    pub value: f64,
    /// Optional custom color (uses theme color if None)
    pub color: Option<Color32>,
}

impl Bar {
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

/// A horizontal bar chart visualization for comparing values across categories
pub struct BarChartViz {
    /// Unique identifier
    #[allow(dead_code)]
    id: usize,
    /// The metric name being displayed
    pub(crate) metric_name: String,
    /// Bars to display
    bars: Vec<Bar>,
    /// Current theme
    pub(crate) theme: AppTheme,
    /// Title (shown in tab)
    title: String,
    /// Unit suffix for values (e.g., "ms", "req/s", "%")
    unit: String,
    /// Whether to show values on bars
    show_values: bool,
    /// Whether bars are sorted by value (descending)
    sorted: bool,
}

impl Default for BarChartViz {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl BarChartViz {
    pub fn new(metric_name: impl Into<String>) -> Self {
        let name = metric_name.into();
        Self {
            id: next_id_usize(),
            title: name.clone(),
            metric_name: name,
            bars: Vec::new(),
            theme: AppTheme::default(),
            unit: String::new(),
            show_values: true,
            sorted: true,
        }
    }

    /// Set the unit suffix for values (e.g., "ms", "req/s", "%")
    pub fn set_unit(&mut self, unit: impl Into<String>) {
        self.unit = unit.into();
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

    /// Add a bar to the chart
    pub fn add_bar(&mut self, bar: Bar) {
        self.bars.push(bar);
    }

    /// Set all bars at once
    pub fn set_bars(&mut self, bars: Vec<Bar>) {
        self.bars = bars;
    }

    /// Clear all bars
    pub fn clear(&mut self) {
        self.bars.clear();
    }

    /// Set whether to show values on bars
    pub fn set_show_values(&mut self, show: bool) {
        self.show_values = show;
    }

    /// Set whether bars are sorted by value
    pub fn set_sorted(&mut self, sorted: bool) {
        self.sorted = sorted;
    }

    /// Get bars sorted by value (descending) if sorted is true
    pub(crate) fn get_display_bars(&self) -> Vec<&Bar> {
        let mut bars: Vec<&Bar> = self.bars.iter().collect();
        if self.sorted {
            bars.sort_by(|a, b| {
                b.value
                    .partial_cmp(&a.value)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        bars
    }

    /// Format a value for display
    pub(crate) fn format_value(value: f64) -> String {
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

    /// Render the bar chart
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);
        let accent_color = palette::accent::PRIMARY;

        ui.vertical(|ui| {
            ui.add_space(VIZ_PADDING_TOP);

            // Title (only show if explicitly set and different from default)
            if !self.title.is_empty() && self.title != "Untitled" {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(&self.title)
                            .color(text_col)
                            .size(14.0)
                            .strong(),
                    );
                });
                ui.add_space(8.0);
            }

            if self.bars.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("No data")
                            .color(text_col.gamma_multiply(0.4))
                            .size(14.0),
                    );
                });
                return;
            }

            let bars = self.get_display_bars();
            let max_value = bars
                .iter()
                .map(|b| b.value)
                .fold(0.0_f64, |a, b| a.max(b))
                .max(0.001);

            // Calculate label width (for alignment)
            let label_width = 100.0_f32;
            let value_width = 60.0_f32;
            let bar_height = 24.0_f32;
            let bar_spacing = 4.0_f32;

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for bar in bars {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);

                            // Label (left-aligned, fixed width)
                            let label_text = if bar.label.len() > 12 {
                                format!("{}...", &bar.label[..12])
                            } else {
                                bar.label.clone()
                            };
                            ui.add_sized(
                                [label_width, bar_height],
                                egui::Label::new(
                                    RichText::new(label_text)
                                        .color(text_col.gamma_multiply(0.8))
                                        .size(12.0),
                                ),
                            );

                            // Bar
                            let available_width = ui.available_width() - value_width - 16.0;
                            let bar_width = (bar.value / max_value) as f32 * available_width;
                            let bar_color = bar.color.unwrap_or(accent_color);

                            let (rect, _response) = ui.allocate_exact_size(
                                egui::vec2(available_width, bar_height),
                                egui::Sense::hover(),
                            );

                            // Draw background
                            ui.painter()
                                .rect_filled(rect, 4.0, text_col.gamma_multiply(0.05));

                            // Draw filled bar
                            if bar_width > 0.0 {
                                let bar_rect = egui::Rect::from_min_size(
                                    rect.min,
                                    egui::vec2(bar_width.max(4.0), bar_height),
                                );
                                ui.painter().rect_filled(bar_rect, 4.0, bar_color);
                            }

                            // Value (right-aligned)
                            if self.show_values {
                                ui.add_sized(
                                    [value_width, bar_height],
                                    egui::Label::new(
                                        RichText::new(Self::format_value(bar.value))
                                            .color(text_col.gamma_multiply(0.7))
                                            .size(12.0),
                                    ),
                                );
                            }
                        });

                        ui.add_space(bar_spacing);
                    }
                });

            ui.add_space(VIZ_PADDING_BOTTOM);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bar_chart_format_value() {
        assert_eq!(BarChartViz::format_value(75.0), "75");
        assert_eq!(BarChartViz::format_value(1234.0), "1.2K");
        assert_eq!(BarChartViz::format_value(1_234_567.0), "1.2M");
        assert_eq!(BarChartViz::format_value(42.5), "42.5");
    }

    #[test]
    fn test_bar_chart_sorting() {
        let mut bar = BarChartViz::new("test");
        bar.add_bar(Bar::new("A", 10.0));
        bar.add_bar(Bar::new("B", 30.0));
        bar.add_bar(Bar::new("C", 20.0));

        // With sorting enabled (default)
        let sorted = bar.get_display_bars();
        assert_eq!(sorted[0].label, "B"); // highest
        assert_eq!(sorted[1].label, "C");
        assert_eq!(sorted[2].label, "A"); // lowest

        // With sorting disabled
        bar.set_sorted(false);
        let unsorted = bar.get_display_bars();
        assert_eq!(unsorted[0].label, "A"); // insertion order
        assert_eq!(unsorted[1].label, "B");
        assert_eq!(unsorted[2].label, "C");
    }
}
