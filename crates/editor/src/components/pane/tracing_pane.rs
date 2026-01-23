//! Tracing pane component for visualizing distributed traces
//!
//! Displays a trace ID input, waterfall visualization, and span details panel.

use std::any::Any;

use egui::RichText;
use enya_client::tracing::{Span, Trace, format_duration_us, tempo::demo_trace};

use crate::components::pane::tracing::WaterfallChart;
use crate::components::util::id_generator::next_id_usize;
use crate::ui::theme::AppTheme;

use super::super::Component;

/// Actions that can be triggered by the TracingPane
#[derive(Debug, Clone, PartialEq)]
pub enum TracingPaneAction {
    /// No action
    None,
    /// Load a trace by ID
    LoadTrace(String),
}

/// Pane component for visualizing distributed traces
pub struct TracingPane {
    /// Unique identifier
    id: usize,
    /// Pane name/title
    name: String,
    /// Current theme
    theme: AppTheme,
    /// Description
    description: String,

    // Trace state
    /// Input field for trace ID
    trace_id_input: String,
    /// Currently loaded trace
    current_trace: Option<Trace>,
    /// Error message if trace loading failed
    error_message: Option<String>,

    // UI state
    /// Waterfall chart widget
    waterfall: WaterfallChart,
    /// Whether trace is being loaded
    is_loading: bool,
    /// Whether a refresh is needed
    needs_refresh: bool,
    /// Show span detail panel
    show_detail_panel: bool,
    /// Focus the input field
    focus_input: bool,
}

impl Default for TracingPane {
    fn default() -> Self {
        Self::new()
    }
}

impl TracingPane {
    /// Create a new tracing pane
    pub fn new() -> Self {
        Self {
            id: next_id_usize(),
            name: "Trace".to_string(),
            theme: AppTheme::default(),
            description: String::new(),
            trace_id_input: String::new(),
            current_trace: None,
            error_message: None,
            waterfall: WaterfallChart::new(),
            is_loading: false,
            needs_refresh: false,
            show_detail_panel: true,
            focus_input: true,
        }
    }

    /// Create a new tracing pane with a trace ID to load
    pub fn with_trace_id(trace_id: impl Into<String>) -> Self {
        let mut pane = Self::new();
        pane.trace_id_input = trace_id.into();
        pane.needs_refresh = true;
        pane
    }

    /// Create a demo tracing pane with sample data
    pub fn with_demo() -> Self {
        let mut pane = Self::new();
        let trace = demo_trace();
        pane.trace_id_input = trace.trace_id.clone();
        pane.set_trace(trace);
        pane
    }

    /// Set the trace to display
    pub fn set_trace(&mut self, trace: Trace) {
        self.name = format!("Trace: {}", truncate_trace_id(&trace.trace_id));
        self.waterfall.set_trace(trace.clone());
        self.current_trace = Some(trace);
        self.error_message = None;
        self.is_loading = false;
    }

    /// Set an error message
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error_message = Some(message.into());
        self.current_trace = None;
        self.is_loading = false;
    }

    /// Check if a refresh is needed
    pub fn needs_refresh(&self) -> bool {
        self.needs_refresh
    }

    /// Clear the refresh flag
    pub fn clear_refresh(&mut self) {
        self.needs_refresh = false;
    }

    /// Get the trace ID to load
    pub fn trace_id_to_load(&self) -> Option<&str> {
        if self.needs_refresh && !self.trace_id_input.is_empty() {
            Some(&self.trace_id_input)
        } else {
            None
        }
    }

    /// Check if loading
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    /// Set loading state
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
    }

    /// Render the toolbar with trace ID input
    fn render_toolbar(&mut self, ui: &mut egui::Ui) -> TracingPaneAction {
        let mut action = TracingPaneAction::None;
        let text_col = self.theme.text_primary();
        let accent = self.theme.accent_primary();

        // Top padding for breathing room
        ui.add_space(12.0);

        // Toolbar row with consistent vertical centering
        let toolbar_height = 32.0;
        ui.allocate_ui_with_layout(
            egui::Vec2::new(ui.available_width(), toolbar_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(12.0);

                // Trace ID label
                ui.label(
                    RichText::new("Trace ID:")
                        .color(text_col.gamma_multiply(0.7))
                        .size(13.0),
                );

                ui.add_space(8.0);

                // Calculate input width - leave room for buttons
                let button_space = 160.0; // Load + Demo + spinner + spacing
                let input_width = (ui.available_width() - button_space).clamp(120.0, 400.0);

                // Styled text input
                let response = ui.add_sized(
                    [input_width, 26.0],
                    egui::TextEdit::singleline(&mut self.trace_id_input)
                        .hint_text("Enter trace ID...")
                        .font(egui::TextStyle::Monospace),
                );

                // Focus input on first render
                if self.focus_input {
                    response.request_focus();
                    self.focus_input = false;
                }

                // Load on Enter
                if response.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && !self.trace_id_input.is_empty()
                {
                    self.needs_refresh = true;
                    self.is_loading = true;
                    action = TracingPaneAction::LoadTrace(self.trace_id_input.clone());
                }

                ui.add_space(8.0);

                // Load button - styled with accent when enabled
                let load_enabled = !self.trace_id_input.is_empty() && !self.is_loading;
                let load_button = ui.add_enabled(
                    load_enabled,
                    egui::Button::new(RichText::new("Load").size(12.0).color(if load_enabled {
                        accent
                    } else {
                        text_col.gamma_multiply(0.4)
                    }))
                    .min_size(egui::Vec2::new(50.0, 26.0)),
                );
                if load_button.clicked() {
                    self.needs_refresh = true;
                    self.is_loading = true;
                    action = TracingPaneAction::LoadTrace(self.trace_id_input.clone());
                }

                ui.add_space(4.0);

                // Demo button
                let demo_button = ui.add(
                    egui::Button::new(RichText::new("Demo").size(12.0))
                        .min_size(egui::Vec2::new(50.0, 26.0)),
                );
                if demo_button.clicked() {
                    let trace = demo_trace();
                    self.trace_id_input = trace.trace_id.clone();
                    self.set_trace(trace);
                }

                // Loading indicator with spacing
                if self.is_loading {
                    ui.add_space(8.0);
                    ui.spinner();
                }
            },
        );

        // Bottom padding
        ui.add_space(8.0);

        action
    }

    /// Render the span detail panel. Returns true if close button was clicked.
    fn render_detail_panel(&self, ui: &mut egui::Ui, span: &Span) -> bool {
        let text_col = self.theme.text_primary();
        let mut close_clicked = false;

        ui.vertical(|ui| {
            ui.add_space(8.0);

            // Header with close button
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Span Details")
                        .color(text_col)
                        .size(14.0)
                        .strong(),
                );

                // Push close button to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    let close_btn = ui.add(
                        egui::Button::new(
                            RichText::new("×")
                                .size(16.0)
                                .color(text_col.gamma_multiply(0.6)),
                        )
                        .frame(false),
                    );
                    if close_btn.clicked() {
                        close_clicked = true;
                    }
                    // Tooltip for close button
                    close_btn.on_hover_text("Close panel (or click span again)");
                });
            });

            ui.add_space(8.0);

            // Scroll area for details
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            // Service name
                            self.detail_row(ui, "Service", &span.service_name, text_col);

                            // Operation name
                            self.detail_row(ui, "Operation", &span.operation_name, text_col);

                            // Span ID
                            self.detail_row(ui, "Span ID", &span.span_id, text_col);

                            // Parent ID
                            if let Some(ref parent_id) = span.parent_span_id {
                                self.detail_row(ui, "Parent ID", parent_id, text_col);
                            }

                            // Duration
                            self.detail_row(
                                ui,
                                "Duration",
                                &format_duration_us(span.duration_us),
                                text_col,
                            );

                            // Status
                            let status_text = match span.status {
                                enya_client::tracing::SpanStatus::Ok => "OK",
                                enya_client::tracing::SpanStatus::Error => "Error",
                                enya_client::tracing::SpanStatus::Unset => "Unset",
                            };
                            self.detail_row(ui, "Status", status_text, text_col);

                            // Tags section
                            if !span.tags.is_empty() {
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new("Tags")
                                        .color(text_col.gamma_multiply(0.6))
                                        .size(12.0),
                                );
                                ui.add_space(4.0);

                                for (key, value) in &span.tags {
                                    self.detail_row(ui, key, value, text_col);
                                }
                            }

                            // Logs section
                            if !span.logs.is_empty() {
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new("Logs")
                                        .color(text_col.gamma_multiply(0.6))
                                        .size(12.0),
                                );
                                ui.add_space(4.0);

                                for log in &span.logs {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(format_duration_us(log.timestamp_us))
                                                .color(text_col.gamma_multiply(0.5))
                                                .size(11.0),
                                        );
                                    });
                                    for (key, value) in &log.fields {
                                        self.detail_row(ui, key, value, text_col);
                                    }
                                    ui.add_space(4.0);
                                }
                            }
                        });
                    });
                });
        });

        close_clicked
    }

    /// Helper to render a detail row
    fn detail_row(&self, ui: &mut egui::Ui, label: &str, value: &str, text_col: egui::Color32) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{label}:"))
                    .color(text_col.gamma_multiply(0.6))
                    .size(11.0),
            );
            ui.add(
                egui::Label::new(RichText::new(value).color(text_col).size(11.0))
                    .wrap_mode(egui::TextWrapMode::Truncate),
            );
        });
    }

    /// Render error message
    fn render_error(&self, ui: &mut egui::Ui) {
        if let Some(ref error) = self.error_message {
            let error_col = self.theme.semantic_error();

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(egui_nerdfonts::regular::ALERT).color(error_col));
                ui.label(RichText::new(error).color(error_col));
            });
        }
    }
}

impl Component for TracingPane {
    fn show(&mut self, ui: &mut egui::Ui) {
        let _action = self.render_toolbar(ui);

        // Show error if any
        self.render_error(ui);

        // Separator line
        let sep_rect = ui.allocate_space(egui::Vec2::new(ui.available_width(), 1.0));
        ui.painter()
            .rect_filled(sep_rect.1, 0.0, self.theme.border_subtle());
        ui.add_space(4.0);

        // Main content area
        let available_height = ui.available_height();
        let show_panel = self.show_detail_panel && self.waterfall.selected_span_id().is_some();
        let panel_height = if show_panel { 200.0 } else { 0.0 };

        // Calculate waterfall height based on content
        // Cap to either content height or available space (minus panel), whichever is smaller
        let span_count = self
            .current_trace
            .as_ref()
            .map(|t| t.spans.len())
            .unwrap_or(0);
        let estimated_row_height = 32.0;
        let header_and_padding = 60.0;
        let estimated_content_height =
            header_and_padding + (span_count as f32 * estimated_row_height);
        let max_waterfall_height = available_height - panel_height;
        let waterfall_height = estimated_content_height
            .min(max_waterfall_height)
            .max(100.0);

        // Waterfall chart
        ui.allocate_ui_with_layout(
            egui::Vec2::new(ui.available_width(), waterfall_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                let _action = self.waterfall.show(ui);
                // Actions are handled internally by waterfall (toggle on click)
            },
        );

        // Track if we should close the panel
        let mut close_panel = false;

        // Detail panel (if a span is selected)
        if show_panel {
            // Separator
            let sep_rect = ui.allocate_space(egui::Vec2::new(ui.available_width(), 1.0));
            ui.painter()
                .rect_filled(sep_rect.1, 0.0, self.theme.border_subtle());

            // Get selected span and render panel
            if let Some(span_id) = self.waterfall.selected_span_id() {
                if let Some(trace) = &self.current_trace {
                    if let Some(span) = trace.get_span(span_id) {
                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(ui.available_width(), panel_height),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| {
                                if self.render_detail_panel(ui, span) {
                                    close_panel = true;
                                }
                            },
                        );
                    }
                }
            }
        }

        // Handle close button click - deselect the span
        if close_panel {
            self.waterfall.set_selected_span(None);
        }
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.waterfall.set_theme(theme);
    }

    fn label(&self) -> egui::RichText {
        RichText::new(format!(
            "{} {}",
            egui_nerdfonts::regular::CHART_TIMELINE_VARIANT,
            self.name
        ))
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Truncate a trace ID for display (first 8 chars)
fn truncate_trace_id(trace_id: &str) -> String {
    if trace_id.len() <= 12 {
        trace_id.to_string()
    } else {
        format!("{}...", &trace_id[..8])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_pane_new() {
        let pane = TracingPane::new();
        assert_eq!(pane.name(), "Trace");
        assert!(!pane.needs_refresh());
        assert!(!pane.is_loading());
    }

    #[test]
    fn test_tracing_pane_with_trace_id() {
        let pane = TracingPane::with_trace_id("abc123");
        assert!(pane.needs_refresh());
        assert_eq!(pane.trace_id_to_load(), Some("abc123"));
    }

    #[test]
    fn test_truncate_trace_id() {
        assert_eq!(truncate_trace_id("short"), "short");
        assert_eq!(truncate_trace_id("0123456789abcdef"), "01234567...");
    }
}
