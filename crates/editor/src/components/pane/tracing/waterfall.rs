//! Waterfall chart visualization for distributed traces
//!
//! Displays spans as horizontal bars on a timeline, with hierarchy indentation
//! showing parent-child relationships.

use egui::{Color32, Rect, RichText, Sense, Vec2};
use enya_client::tracing::{Span, SpanStatus, Trace, format_duration_us};
use rustc_hash::FxHashMap;

use crate::ui::colors::text_color;
use crate::ui::theme::AppTheme;

/// Padding at top and bottom of the waterfall chart
const PADDING_TOP: f32 = 8.0;
const PADDING_BOTTOM: f32 = 8.0;

/// Actions that can be triggered by the waterfall chart
#[derive(Debug, Clone, PartialEq)]
pub enum WaterfallAction {
    /// No action
    None,
    /// A span was selected (clicked)
    SpanSelected(String),
    /// A span was deselected (clicked again or closed)
    SpanDeselected,
    /// A span was hovered
    SpanHovered(Option<String>),
}

/// Waterfall chart for visualizing distributed traces
pub struct WaterfallChart {
    /// The trace being displayed
    trace: Option<Trace>,
    /// Current theme
    theme: AppTheme,
    /// Currently selected span ID
    selected_span_id: Option<String>,
    /// Currently hovered span ID
    hovered_span_id: Option<String>,
    /// Mapping from service name to color index
    service_colors: FxHashMap<String, usize>,
    /// Zoom level (1.0 = normal)
    #[allow(dead_code)]
    zoom_level: f32,
    /// Show span labels inline
    show_labels: bool,
}

impl Default for WaterfallChart {
    fn default() -> Self {
        Self::new()
    }
}

impl WaterfallChart {
    /// Create a new waterfall chart
    pub fn new() -> Self {
        Self {
            trace: None,
            theme: AppTheme::default(),
            selected_span_id: None,
            hovered_span_id: None,
            service_colors: FxHashMap::default(),
            zoom_level: 1.0,
            show_labels: true,
        }
    }

    /// Set the trace to display
    pub fn set_trace(&mut self, trace: Trace) {
        // Build service color mapping
        self.service_colors.clear();
        for (idx, service) in trace.services.iter().enumerate() {
            self.service_colors.insert(service.clone(), idx);
        }
        self.trace = Some(trace);
        self.selected_span_id = None;
        self.hovered_span_id = None;
    }

    /// Clear the trace data
    pub fn clear(&mut self) {
        self.trace = None;
        self.service_colors.clear();
        self.selected_span_id = None;
        self.hovered_span_id = None;
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Get the currently selected span ID
    pub fn selected_span_id(&self) -> Option<&str> {
        self.selected_span_id.as_deref()
    }

    /// Set the selected span ID
    pub fn set_selected_span(&mut self, span_id: Option<String>) {
        self.selected_span_id = span_id;
    }

    /// Toggle label visibility
    pub fn toggle_labels(&mut self) {
        self.show_labels = !self.show_labels;
    }

    /// Get the span color based on service and status
    fn span_color(&self, span: &Span) -> Color32 {
        if span.status == SpanStatus::Error {
            return self.theme.semantic_error();
        }

        let color_idx = self
            .service_colors
            .get(&span.service_name)
            .copied()
            .unwrap_or(0);
        self.theme.chart_color(color_idx)
    }

    /// Render the waterfall chart
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) -> WaterfallAction {
        let mut action = WaterfallAction::None;

        // Extract data from trace upfront to avoid borrow conflicts
        let (trace_duration, trace_start, sorted_spans) = {
            let Some(trace) = &self.trace else {
                self.show_empty_state(ui);
                return action;
            };

            let trace_duration = trace.duration_us.max(1) as f64;
            let trace_start = trace.start_time_us;

            // Clone and sort spans by depth and start time for display
            let mut sorted_spans: Vec<Span> = trace.spans.clone();
            sorted_spans.sort_by_key(|s| (s.depth, s.start_time_us));

            (trace_duration, trace_start, sorted_spans)
        };

        // Use theme-specific colors for a distinct look per theme
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();
        let border_subtle = self.theme.border_subtle();
        let bg_inset = self.theme.bg_inset();
        let accent_selection = self.theme.accent_selection();

        let available_width = ui.available_width();
        let available_height = ui.available_height();

        // Scale based on available space
        let base_size = available_width.min(available_height);
        let scale_factor = (base_size / 400.0).clamp(0.7, 1.5);

        // Scaled dimensions
        let row_height = (28.0 * scale_factor).clamp(24.0, 36.0);
        let row_spacing = (2.0 * scale_factor).clamp(1.0, 3.0);
        let indent_per_level = (16.0 * scale_factor).clamp(12.0, 24.0);
        let bar_corner_radius = (3.0 * scale_factor).clamp(2.0, 4.0);
        let font_size = (12.0 * scale_factor).clamp(11.0, 14.0);
        let small_font_size = (10.0 * scale_factor).clamp(9.0, 12.0);

        // Layout: fixed-width columns with clean proportions
        let content_width = available_width - 16.0;
        let left_margin = 12.0;
        let right_margin = 12.0;

        // Column widths - duration is fixed, others flex
        let duration_col_width = 72.0; // Fixed width for duration column
        let usable_width = content_width - left_margin - right_margin - duration_col_width;

        // Service/Operation gets 30% of usable, Timeline gets 70%
        let label_col_width = (usable_width * 0.30).clamp(100.0, 220.0);
        let timeline_col_width = usable_width - label_col_width;

        // Column boundaries (x positions relative to content_left)
        let label_col_start = left_margin;
        let label_col_end = label_col_start + label_col_width;
        let timeline_col_start = label_col_end;
        let timeline_col_end = timeline_col_start + timeline_col_width;
        let duration_col_start = timeline_col_end;
        let duration_col_end = duration_col_start + duration_col_width;

        ui.vertical(|ui| {
            ui.add_space(PADDING_TOP);

            // === HEADER ROW ===
            let header_height = row_height * 0.9;
            let (header_rect, _) =
                ui.allocate_exact_size(Vec2::new(content_width, header_height), Sense::hover());
            let content_left = header_rect.min.x;
            let header_y = header_rect.center().y;

            // Column header: Service / Operation
            ui.painter().text(
                egui::pos2(content_left + label_col_start, header_y),
                egui::Align2::LEFT_CENTER,
                "Service / Operation",
                egui::FontId::proportional(small_font_size),
                text_tertiary,
            );

            // Column header: Timeline with time scale
            let timeline_header_rect = Rect::from_min_size(
                egui::pos2(content_left + timeline_col_start, header_rect.min.y),
                Vec2::new(timeline_col_width, header_height),
            );
            self.draw_time_axis(
                ui,
                timeline_header_rect,
                trace_duration,
                text_tertiary,
                small_font_size,
            );

            // Column header: Duration (right-aligned to match values)
            ui.painter().text(
                egui::pos2(content_left + duration_col_end - right_margin, header_y),
                egui::Align2::RIGHT_CENTER,
                "Duration",
                egui::FontId::proportional(small_font_size),
                text_tertiary,
            );

            // === HEADER DIVIDER ===
            ui.add_space(6.0);
            let (divider_rect, _) =
                ui.allocate_exact_size(Vec2::new(content_width, 1.0), Sense::hover());
            ui.painter().rect_filled(divider_rect, 0.0, border_subtle);
            ui.add_space(6.0);

            // === SPAN ROWS ===
            egui::ScrollArea::vertical()
                .id_salt("waterfall_spans")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (row_idx, span) in sorted_spans.iter().enumerate() {
                        let is_selected = self.selected_span_id.as_ref() == Some(&span.span_id);
                        let is_hovered = self.hovered_span_id.as_ref() == Some(&span.span_id);

                        // Allocate row with unique ID
                        let (row_rect, response) = ui
                            .push_id(row_idx, |ui| {
                                ui.allocate_exact_size(
                                    Vec2::new(content_width, row_height),
                                    Sense::click(),
                                )
                            })
                            .inner;

                        let row_left = row_rect.min.x;
                        let row_y = row_rect.min.y;
                        let row_center_y = row_rect.center().y;

                        // Row background - use theme-specific selection colors
                        if is_selected {
                            ui.painter().rect_filled(row_rect, 3.0, accent_selection);
                        } else if is_hovered || response.hovered() {
                            ui.painter()
                                .rect_filled(row_rect, 3.0, self.theme.bg_hover());
                        }

                        // Handle interactions - toggle selection on click
                        if response.clicked() {
                            if is_selected {
                                // Clicking on already selected span deselects it
                                self.selected_span_id = None;
                                action = WaterfallAction::SpanDeselected;
                            } else {
                                self.selected_span_id = Some(span.span_id.clone());
                                action = WaterfallAction::SpanSelected(span.span_id.clone());
                            }
                        }
                        if response.hovered()
                            && self.hovered_span_id.as_ref() != Some(&span.span_id)
                        {
                            self.hovered_span_id = Some(span.span_id.clone());
                            action = WaterfallAction::SpanHovered(Some(span.span_id.clone()));
                        }

                        // === LABEL COLUMN ===
                        let indent = span.depth as f32 * indent_per_level;
                        let service_color = self.span_color(span);

                        // Service color indicator (vertical bar)
                        let indicator_rect = Rect::from_min_size(
                            egui::pos2(row_left + label_col_start + indent, row_y + 5.0),
                            Vec2::new(3.0, row_height - 10.0),
                        );
                        ui.painter().rect_filled(indicator_rect, 1.5, service_color);

                        // Operation name (truncated to fit)
                        let label_text = if self.show_labels {
                            &span.operation_name
                        } else {
                            &span.service_name
                        };
                        let available_label_width = label_col_width - indent - 20.0;
                        let max_chars = ((available_label_width) / (font_size * 0.55)) as usize;
                        let truncated = truncate_string(label_text, max_chars.max(8));

                        ui.painter().text(
                            egui::pos2(row_left + label_col_start + indent + 10.0, row_center_y),
                            egui::Align2::LEFT_CENTER,
                            &truncated,
                            egui::FontId::proportional(font_size),
                            if is_selected {
                                text_primary
                            } else {
                                text_secondary
                            },
                        );

                        // === TIMELINE COLUMN ===
                        let timeline_rect = Rect::from_min_size(
                            egui::pos2(row_left + timeline_col_start, row_y),
                            Vec2::new(timeline_col_width, row_height),
                        );

                        // Subtle track background using theme's inset color
                        let track_rect = Rect::from_min_size(
                            timeline_rect.min + Vec2::new(0.0, row_height * 0.38),
                            Vec2::new(timeline_col_width, row_height * 0.24),
                        );
                        ui.painter()
                            .rect_filled(track_rect, bar_corner_radius, bg_inset);

                        // Span bar
                        let span_offset = (span.start_time_us - trace_start) as f64;
                        let bar_x = (span_offset / trace_duration) * timeline_col_width as f64;
                        let bar_width = ((span.duration_us as f64 / trace_duration)
                            * timeline_col_width as f64)
                            .max(3.0);

                        let bar_rect = Rect::from_min_size(
                            timeline_rect.min + Vec2::new(bar_x as f32, row_height * 0.28),
                            Vec2::new(bar_width as f32, row_height * 0.44),
                        );

                        let bar_color = if is_selected {
                            self.theme.accent_primary()
                        } else {
                            service_color
                        };
                        ui.painter()
                            .rect_filled(bar_rect, bar_corner_radius, bar_color);

                        // Selected bar border
                        if is_selected {
                            ui.painter().rect_stroke(
                                bar_rect,
                                bar_corner_radius,
                                egui::Stroke::new(1.5, self.theme.accent_hover()),
                                egui::StrokeKind::Inside,
                            );
                        }

                        // === DURATION COLUMN (right-aligned) ===
                        ui.painter().text(
                            egui::pos2(row_left + duration_col_end - right_margin, row_center_y),
                            egui::Align2::RIGHT_CENTER,
                            format_duration_us(span.duration_us),
                            egui::FontId::proportional(small_font_size),
                            if is_selected {
                                text_secondary
                            } else {
                                text_tertiary
                            },
                        );

                        // Tooltip
                        response.on_hover_ui_at_pointer(|ui| {
                            self.show_span_tooltip(ui, span);
                        });

                        ui.add_space(row_spacing);
                    }
                });

            ui.add_space(PADDING_BOTTOM);
        });

        // Clear hover when mouse leaves
        if !ui.rect_contains_pointer(ui.min_rect()) && self.hovered_span_id.is_some() {
            self.hovered_span_id = None;
            action = WaterfallAction::SpanHovered(None);
        }

        action
    }

    /// Draw the time axis with evenly spaced markers
    fn draw_time_axis(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        total_duration_us: f64,
        label_color: Color32,
        font_size: f32,
    ) {
        let painter = ui.painter();
        let tick_color = self.theme.border_subtle();

        // Determine optimal number of markers based on width
        let num_markers = if rect.width() > 400.0 {
            5
        } else if rect.width() > 200.0 {
            4
        } else {
            3
        };

        // Draw markers at regular intervals (0%, 25%, 50%, 75%, 100% for 4 markers)
        for i in 0..=num_markers {
            let t = i as f32 / num_markers as f32;
            let x = rect.min.x + (t * rect.width());
            let time_us = (t as f64 * total_duration_us) as u64;

            // Small tick mark using theme's subtle border color
            painter.line_segment(
                [egui::pos2(x, rect.max.y - 3.0), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(1.0, tick_color),
            );

            // Time labels: show first (0), middle markers, but not the last one
            // (Duration column already shows the span durations)
            if i == 0 {
                // "0" at the start
                painter.text(
                    egui::pos2(x + 2.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "0",
                    egui::FontId::proportional(font_size * 0.9),
                    label_color,
                );
            } else if i < num_markers {
                // Intermediate markers centered on tick
                painter.text(
                    egui::pos2(x, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    format_duration_us(time_us),
                    egui::FontId::proportional(font_size * 0.9),
                    label_color,
                );
            }
            // Skip the last marker label to avoid overlap with Duration header
        }
    }

    /// Show span tooltip with details
    fn show_span_tooltip(&self, ui: &mut egui::Ui, span: &Span) {
        ui.set_max_width(300.0);

        let text_col = text_color(self.theme);

        ui.vertical(|ui| {
            // Service and operation
            ui.label(
                RichText::new(&span.service_name)
                    .color(self.span_color(span))
                    .strong(),
            );
            ui.label(RichText::new(&span.operation_name).color(text_col));

            ui.add_space(4.0);

            // Duration
            ui.horizontal(|ui| {
                ui.label(RichText::new("Duration:").color(text_col.gamma_multiply(0.6)));
                ui.label(RichText::new(span.format_duration()).color(text_col));
            });

            // Status
            if span.status == SpanStatus::Error {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Status:").color(text_col.gamma_multiply(0.6)));
                    ui.label(RichText::new("Error").color(self.theme.semantic_error()));
                });
            }

            // Show some tags (limit to 5)
            if !span.tags.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new("Tags:").color(text_col.gamma_multiply(0.6)));

                for (idx, (key, value)) in span.tags.iter().enumerate() {
                    if idx >= 5 {
                        ui.label(
                            RichText::new(format!("... and {} more", span.tags.len() - 5))
                                .color(text_col.gamma_multiply(0.5)),
                        );
                        break;
                    }

                    let truncated_value = truncate_string(value, 30);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("  {key}:")).color(text_col.gamma_multiply(0.7)),
                        );
                        ui.label(
                            RichText::new(truncated_value).color(text_col.gamma_multiply(0.9)),
                        );
                    });
                }
            }
        });
    }

    /// Show empty state when no trace is loaded
    fn show_empty_state(&self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);

        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.35);

                ui.label(
                    RichText::new(egui_nerdfonts::regular::CHART_TIMELINE_VARIANT)
                        .size(48.0)
                        .color(text_col.gamma_multiply(0.3)),
                );

                ui.add_space(8.0);

                ui.label(
                    RichText::new("No trace loaded")
                        .size(16.0)
                        .color(text_col.gamma_multiply(0.5)),
                );

                ui.add_space(4.0);

                ui.label(
                    RichText::new("Enter a trace ID above to visualize spans")
                        .size(12.0)
                        .color(text_col.gamma_multiply(0.4)),
                );
            });
        });
    }
}

/// Truncate a string to a maximum length, adding ellipsis if needed
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 2), "hi");
    }

    #[test]
    fn test_waterfall_chart_new() {
        let chart = WaterfallChart::new();
        assert!(chart.trace.is_none());
        assert!(chart.selected_span_id.is_none());
    }
}
