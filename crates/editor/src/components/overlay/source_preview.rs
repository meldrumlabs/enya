//! Source code preview overlay for viewing metric definitions.
//!
//! Displays a source file centered on a specific line with syntax highlighting,
//! used for "go to metric definition" functionality.

#[cfg(not(target_arch = "wasm32"))]
use std::ops::Range;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use egui::{Color32, Key, RichText, text::LayoutJob};
#[cfg(not(target_arch = "wasm32"))]
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::{OverlayStyle, draw_backdrop};

#[cfg(not(target_arch = "wasm32"))]
use crate::codebase::{AlertRule, MetricInstrumentation, MetricKind};

/// Highlight names recognized by our syntax highlighter.
/// These map to tree-sitter highlight capture names.
#[cfg(not(target_arch = "wasm32"))]
const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "escape",
    "function",
    "function.builtin",
    "function.macro",
    "keyword",
    "label",
    "number",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
];

/// A cached highlight span: byte range and the highlight type index.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
struct HighlightSpan {
    range: Range<usize>,
    highlight_idx: usize,
}

/// Result of showing the source preview overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreviewResult {
    /// No action taken.
    None,
    /// Overlay was closed.
    Closed,
}

/// The kind of preview being shown.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PreviewKind {
    /// Showing a metric definition.
    #[default]
    Metric,
    /// Showing an alert rule.
    Alert,
}

/// A modal overlay that displays source code context around a metric or alert definition.
pub struct SourcePreviewOverlay {
    /// Whether the overlay is open.
    is_open: bool,
    /// Current theme.
    theme: AppTheme,
    /// What kind of content is being previewed.
    preview_kind: PreviewKind,
    /// Relative path to display (from repo root).
    relative_path: String,
    /// Full path to source file for reading.
    #[cfg(not(target_arch = "wasm32"))]
    full_path: PathBuf,
    /// Lines of source code.
    source_lines: Vec<String>,
    /// Raw source content for tree-sitter highlighting.
    #[cfg(not(target_arch = "wasm32"))]
    source_content: String,
    /// Byte offsets for the start of each line (for mapping spans to lines).
    #[cfg(not(target_arch = "wasm32"))]
    line_offsets: Vec<usize>,
    /// Cached highlight spans from tree-sitter.
    #[cfg(not(target_arch = "wasm32"))]
    highlight_spans: Vec<HighlightSpan>,
    /// Target line number (1-indexed) to highlight.
    target_line: usize,
    /// The metric name being shown (for metrics) or alert name (for alerts).
    metric_name: String,
    /// The kind of metric (counter, gauge, histogram).
    #[cfg(not(target_arch = "wasm32"))]
    metric_kind: Option<MetricKind>,
    /// Labels discovered for this metric.
    labels: Vec<String>,
    /// The function containing this metric (if any).
    #[cfg(not(target_arch = "wasm32"))]
    function_name: Option<String>,
    /// The impl type if inside an impl block (if any).
    #[cfg(not(target_arch = "wasm32"))]
    impl_type: Option<String>,
    /// Error message if file couldn't be loaded.
    error: Option<String>,
    /// Alert severity (if showing an alert).
    alert_severity: Option<String>,
    /// Alert message (if showing an alert).
    alert_message: Option<String>,
    /// Alert expression (if showing an alert).
    alert_expr: Option<String>,
    /// Horizontal scroll offset for vim-style navigation.
    scroll_offset_x: f32,
    /// All metric locations for cycling (only used for metrics).
    #[cfg(not(target_arch = "wasm32"))]
    metric_locations: Vec<MetricInstrumentation>,
    /// Current location index (0-based).
    #[cfg(not(target_arch = "wasm32"))]
    current_location_index: usize,
    /// Path to the repository root for constructing full paths.
    #[cfg(not(target_arch = "wasm32"))]
    repo_path: PathBuf,
}

impl Default for SourcePreviewOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl SourcePreviewOverlay {
    /// Creates a new source preview overlay.
    pub fn new() -> Self {
        Self {
            is_open: false,
            theme: AppTheme::default(),
            preview_kind: PreviewKind::default(),
            relative_path: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            full_path: PathBuf::new(),
            source_lines: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            source_content: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            line_offsets: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            highlight_spans: Vec::new(),
            target_line: 0,
            metric_name: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            metric_kind: None,
            labels: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            function_name: None,
            #[cfg(not(target_arch = "wasm32"))]
            impl_type: None,
            error: None,
            alert_severity: None,
            alert_message: None,
            alert_expr: None,
            scroll_offset_x: 0.0,
            #[cfg(not(target_arch = "wasm32"))]
            metric_locations: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            current_location_index: 0,
            #[cfg(not(target_arch = "wasm32"))]
            repo_path: PathBuf::new(),
        }
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the overlay.
    pub fn open(&mut self) {
        self.is_open = true;
    }

    /// Close the overlay.
    pub fn close(&mut self) {
        self.is_open = false;
        self.source_lines.clear();
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.source_content.clear();
            self.line_offsets.clear();
            self.highlight_spans.clear();
            self.metric_locations.clear();
            self.current_location_index = 0;
        }
        self.error = None;
        self.alert_severity = None;
        self.alert_message = None;
        self.alert_expr = None;
        self.preview_kind = PreviewKind::default();
        self.scroll_offset_x = 0.0;
    }

    /// Check if the overlay is open.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Open the overlay with a metric instrumentation point.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_metric(
        &mut self,
        instrumentation: &MetricInstrumentation,
        repo_path: &std::path::Path,
    ) {
        self.preview_kind = PreviewKind::Metric;
        self.metric_name = instrumentation.name.clone();
        self.metric_kind = Some(instrumentation.kind);
        self.labels = instrumentation.labels.clone();
        self.target_line = instrumentation.line;
        self.relative_path = instrumentation.file.display().to_string();
        self.full_path = repo_path.join(&instrumentation.file);
        self.function_name = instrumentation.function_name.clone();
        self.impl_type = instrumentation.impl_type.clone();
        self.alert_severity = None;
        self.alert_message = None;
        self.alert_expr = None;

        // Load the source file
        self.load_source_file();
        self.is_open = true;
    }

    /// Open the overlay with multiple metric instrumentation locations.
    ///
    /// This allows cycling through multiple locations with N/P keys.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_metric_with_locations(
        &mut self,
        locations: Vec<MetricInstrumentation>,
        repo_path: &std::path::Path,
    ) {
        if locations.is_empty() {
            return;
        }

        self.metric_locations = locations;
        self.current_location_index = 0;
        self.repo_path = repo_path.to_path_buf();

        // Load the first location
        self.load_location(0);
        self.is_open = true;
    }

    /// Load a specific location by index.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_location(&mut self, index: usize) {
        let Some(instrumentation) = self.metric_locations.get(index) else {
            return;
        };

        self.preview_kind = PreviewKind::Metric;
        self.metric_name = instrumentation.name.clone();
        self.metric_kind = Some(instrumentation.kind);
        self.labels = instrumentation.labels.clone();
        self.target_line = instrumentation.line;
        self.relative_path = instrumentation.file.display().to_string();
        self.full_path = self.repo_path.join(&instrumentation.file);
        self.function_name = instrumentation.function_name.clone();
        self.impl_type = instrumentation.impl_type.clone();
        self.alert_severity = None;
        self.alert_message = None;
        self.alert_expr = None;
        self.current_location_index = index;

        self.load_source_file();
    }

    /// Open the overlay with an alert rule.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_alert(&mut self, alert: &AlertRule, repo_path: &std::path::Path) {
        self.preview_kind = PreviewKind::Alert;
        self.metric_name = alert.name.clone();
        self.metric_kind = None;
        self.labels = Vec::new();
        self.target_line = alert.line;
        self.relative_path = alert.file.display().to_string();
        self.full_path = repo_path.join(&alert.file);
        self.function_name = None;
        self.impl_type = None;
        self.alert_severity = alert.severity.clone();
        self.alert_message = alert.message.clone();
        self.alert_expr = Some(alert.expr.clone());

        // Load the source file
        self.load_source_file();
        self.is_open = true;
    }

    /// Open the overlay showing an error message.
    pub fn open_error(&mut self, metric_name: &str, error: &str) {
        self.metric_name = metric_name.to_string();
        self.error = Some(error.to_string());
        self.source_lines.clear();
        self.is_open = true;
    }

    /// Open the overlay with demo/mock data for testing the UI.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_demo(&mut self) {
        log::debug!("SourcePreviewOverlay::open_demo() called");
        self.metric_name = "http.requests".to_string();
        self.metric_kind = Some(MetricKind::Counter);
        self.labels = vec!["method".to_string(), "status".to_string()];
        self.relative_path = "src/handlers/http.rs".to_string();
        self.target_line = 12;
        self.function_name = Some("handle".to_string());
        self.impl_type = Some("HttpHandler".to_string());
        self.error = None;

        // Mock Rust source code
        let demo_source = r#"use metrics::counter;

pub struct HttpHandler {
    client: reqwest::Client,
}

impl HttpHandler {
    pub async fn handle(&self, req: Request) -> Response {
        let method = req.method().to_string();
        let path = req.uri().path();

        counter!("http.requests", "method" => method, "status" => "200").increment(1);

        // Process the request
        let response = self.client
            .request(req.method().clone(), path)
            .send()
            .await?;

        Response::new(response.status())
    }
}
"#;

        // Compute line offsets
        self.line_offsets.clear();
        self.line_offsets.push(0);
        for (i, ch) in demo_source.char_indices() {
            if ch == '\n' {
                self.line_offsets.push(i + 1);
            }
        }

        self.source_lines = demo_source.lines().map(String::from).collect();
        self.source_content = demo_source.to_string();

        // Compute syntax highlights
        self.compute_highlights();

        self.is_open = true;
        log::debug!(
            "SourcePreviewOverlay::open_demo() complete - is_open={}, spans={}",
            self.is_open,
            self.highlight_spans.len()
        );
    }

    /// Load source file contents and compute syntax highlights.
    #[cfg(not(target_arch = "wasm32"))]
    fn load_source_file(&mut self) {
        match std::fs::read_to_string(&self.full_path) {
            Ok(content) => {
                // Compute line offsets for mapping byte positions to lines
                self.line_offsets.clear();
                self.line_offsets.push(0);
                for (i, ch) in content.char_indices() {
                    if ch == '\n' {
                        self.line_offsets.push(i + 1);
                    }
                }

                self.source_lines = content.lines().map(String::from).collect();
                self.source_content = content;
                self.error = None;

                // Compute syntax highlights
                self.compute_highlights();
            }
            Err(e) => {
                self.error = Some(format!("Failed to read file: {e}"));
                self.source_lines.clear();
                self.source_content.clear();
                self.line_offsets.clear();
                self.highlight_spans.clear();
            }
        }
    }

    /// Compute syntax highlights for the loaded source using tree-sitter.
    #[cfg(not(target_arch = "wasm32"))]
    fn compute_highlights(&mut self) {
        self.highlight_spans.clear();

        if self.source_content.is_empty() {
            return;
        }

        // Create highlight configuration
        let mut config = match HighlightConfiguration::new(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            "", // injections query
            "", // locals query
        ) {
            Ok(config) => config,
            Err(e) => {
                log::warn!("Failed to create highlight config: {e}");
                return;
            }
        };

        // Configure recognized highlight names
        config.configure(HIGHLIGHT_NAMES);

        // Create highlighter and process the source
        let mut highlighter = Highlighter::new();
        let source_bytes = self.source_content.as_bytes();

        let highlights = match highlighter.highlight(&config, source_bytes, None, |_| None) {
            Ok(h) => h,
            Err(e) => {
                log::warn!("Failed to highlight: {e}");
                return;
            }
        };

        // Track highlight stack for nested highlights
        let mut highlight_stack: Vec<usize> = Vec::new();

        for event in highlights {
            let event = match event {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("Highlight event error: {e}");
                    continue;
                }
            };

            match event {
                HighlightEvent::Source { start, end } => {
                    // If we have an active highlight, record this span
                    if let Some(&highlight_idx) = highlight_stack.last() {
                        self.highlight_spans.push(HighlightSpan {
                            range: start..end,
                            highlight_idx,
                        });
                    }
                }
                HighlightEvent::HighlightStart(highlight) => {
                    highlight_stack.push(highlight.0);
                }
                HighlightEvent::HighlightEnd => {
                    highlight_stack.pop();
                }
            }
        }
    }

    /// Show the overlay. Returns the result of the interaction.
    pub fn show(&mut self, ctx: &egui::Context) -> SourcePreviewResult {
        if !self.is_open {
            return SourcePreviewResult::None;
        }

        let mut should_close = false;
        #[cfg(not(target_arch = "wasm32"))]
        let mut next_location: Option<usize> = None;

        // Handle keyboard input
        ctx.input(|i| {
            // Escape to close
            if i.key_pressed(Key::Escape) {
                should_close = true;
            }
            // Vim-style horizontal scrolling: h/l
            let scroll_step = 50.0;
            if i.key_pressed(Key::H) {
                self.scroll_offset_x = (self.scroll_offset_x - scroll_step).max(0.0);
            }
            if i.key_pressed(Key::L) {
                self.scroll_offset_x += scroll_step;
            }

            // N/P navigation for cycling through multiple locations
            #[cfg(not(target_arch = "wasm32"))]
            if self.metric_locations.len() > 1 {
                // N - next location (vim quickfix-style)
                if i.key_pressed(Key::N) && !i.modifiers.shift {
                    next_location =
                        Some((self.current_location_index + 1) % self.metric_locations.len());
                }
                // Shift+N or P - previous location
                if i.key_pressed(Key::P) || (i.key_pressed(Key::N) && i.modifiers.shift) {
                    next_location = Some(if self.current_location_index == 0 {
                        self.metric_locations.len() - 1
                    } else {
                        self.current_location_index - 1
                    });
                }
            }
        });

        // Apply location change outside the input closure
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(idx) = next_location {
            self.load_location(idx);
        }

        // Draw backdrop
        draw_backdrop(ctx, self.theme, "source_preview");

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.7).clamp(500.0, 900.0);
        let popup_max_height = (screen_rect.height() * 0.7).clamp(300.0, 600.0);

        egui::Area::new(egui::Id::new("source_preview_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let separator_color = match self.theme {
                    AppTheme::Light => palette::light_border::SUBTLE,
                    AppTheme::Dark => palette::border::SUBTLE,
                };
                let muted_text = text_color(self.theme).gamma_multiply(0.6);
                let accent_color = match self.theme {
                    AppTheme::Light => palette::accent::LIGHT,
                    AppTheme::Dark => palette::accent::HOVER,
                };

                overlay_style.frame().show(ui, |ui| {
                    // Cap width to prevent content from stretching the overlay
                    ui.set_width(popup_width);
                    ui.set_max_width(popup_width);
                    ui.set_max_height(popup_max_height);

                    // Header section
                    self.render_header(ui, accent_color, separator_color);

                    // Content area
                    if let Some(error) = &self.error {
                        self.render_error(ui, error);
                    } else {
                        self.render_source_code(ui, popup_max_height - 100.0);
                    }

                    // Footer
                    self.render_footer(ui, muted_text, separator_color, popup_width);
                });
            });

        if should_close {
            self.close();
            return SourcePreviewResult::Closed;
        }

        SourcePreviewResult::None
    }

    fn render_header(&self, ui: &mut egui::Ui, accent_color: Color32, separator_color: Color32) {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Icon varies by preview type
            let icon = match self.preview_kind {
                PreviewKind::Metric => semantic_icons::status::INFO,
                PreviewKind::Alert => semantic_icons::status::WARNING,
            };
            ui.label(RichText::new(icon).color(accent_color).size(18.0));
            ui.add_space(8.0);

            // File path and line number, with optional function context
            let mut path_text = if self.target_line > 0 {
                format!("{}:{}", self.relative_path, self.target_line)
            } else {
                self.relative_path.clone()
            };

            // Append function context to path string
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(ref fn_name) = self.function_name {
                if let Some(ref impl_type) = self.impl_type {
                    path_text.push_str(&format!(" • {impl_type}::{fn_name}"));
                } else {
                    path_text.push_str(&format!(" • {fn_name}"));
                }
            }

            // Reserve space for badge (~100px) and margins
            let max_path_width = ui.available_width() - 120.0;
            let truncated_path = if path_text.chars().count() > 70 {
                let truncated: String = path_text.chars().take(67).collect();
                format!("{truncated}...")
            } else {
                path_text
            };

            ui.add_sized(
                [max_path_width.max(100.0), ui.spacing().interact_size.y],
                egui::Label::new(
                    RichText::new(&truncated_path)
                        .color(accent_color)
                        .font(typography::monospace(typography::LG))
                        .strong(),
                ),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(16.0);

                match self.preview_kind {
                    PreviewKind::Metric => {
                        // Metric kind badge
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(kind) = self.metric_kind {
                            let (badge_text, badge_color) = match kind {
                                MetricKind::Counter => ("counter", palette::semantic::SUCCESS),
                                MetricKind::Gauge => ("gauge", palette::semantic::WARNING),
                                MetricKind::Histogram => ("histogram", palette::semantic::INFO),
                            };
                            let bg_color = badge_color.gamma_multiply(0.2);

                            egui::Frame::new()
                                .fill(bg_color)
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(8, 2))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(badge_text)
                                            .color(badge_color)
                                            .font(typography::monospace(typography::SM)),
                                    );
                                });
                        }
                    }
                    PreviewKind::Alert => {
                        // Alert severity badge
                        if let Some(ref severity) = self.alert_severity {
                            let badge_color = match severity.as_str() {
                                "critical" => palette::semantic::ERROR,
                                "warning" => palette::semantic::WARNING,
                                _ => palette::semantic::INFO,
                            };
                            let bg_color = badge_color.gamma_multiply(0.2);

                            egui::Frame::new()
                                .fill(bg_color)
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(8, 2))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(severity.as_str())
                                            .color(badge_color)
                                            .font(typography::monospace(typography::SM)),
                                    );
                                });
                        } else {
                            // Default "alert" badge
                            let badge_color = palette::semantic::WARNING;
                            let bg_color = badge_color.gamma_multiply(0.2);

                            egui::Frame::new()
                                .fill(bg_color)
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(8, 2))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("alert")
                                            .color(badge_color)
                                            .font(typography::monospace(typography::SM)),
                                    );
                                });
                        }
                    }
                }
            });
        });
        ui.add_space(12.0);

        // Separator below header
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator_color),
        );
    }

    fn render_error(&self, ui: &mut egui::Ui, error: &str) {
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(semantic_icons::status::ERROR)
                        .color(palette::semantic::ERROR)
                        .size(24.0),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(error)
                        .color(palette::semantic::ERROR)
                        .font(typography::proportional(typography::MD)),
                );
            });
        });
        ui.add_space(24.0);
    }

    fn render_source_code(&mut self, ui: &mut egui::Ui, _max_height: f32) {
        if self.source_lines.is_empty() {
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("No source code available")
                        .color(text_color(self.theme).gamma_multiply(0.5))
                        .font(typography::proportional(typography::MD)),
                );
            });
            ui.add_space(24.0);
            return;
        }

        // Fixed window: show 10 lines before and 10 lines after the target
        let context_lines = 10;
        let start_line = self.target_line.saturating_sub(context_lines).max(1);
        let end_line = (self.target_line + context_lines).min(self.source_lines.len());

        let line_num_width = format!("{}", self.source_lines.len()).len();

        let line_num_color = match self.theme {
            AppTheme::Light => palette::light_text::TERTIARY,
            AppTheme::Dark => palette::text::TERTIARY,
        };
        let highlight_bg = match self.theme {
            AppTheme::Light => Color32::from_rgba_unmultiplied(255, 220, 0, 40),
            AppTheme::Dark => Color32::from_rgba_unmultiplied(255, 220, 0, 30),
        };

        ui.add_space(8.0);

        // Use a scroll area to clip long lines instead of expanding the overlay
        // Apply vim-style scroll offset via scroll_to_x
        egui::ScrollArea::horizontal()
            .id_salt("source_preview_scroll")
            .scroll_offset(egui::vec2(self.scroll_offset_x, 0.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        // Render only lines in the fixed window
                        for line_num in start_line..=end_line {
                            let line_idx = line_num.saturating_sub(1);
                            let line_content = self
                                .source_lines
                                .get(line_idx)
                                .map(String::as_str)
                                .unwrap_or("");
                            let is_target = line_num == self.target_line;

                            // Syntax highlight the code
                            let highlighted = self.highlight_rust_line(line_num, line_content);

                            // Draw highlight background for target line
                            if is_target {
                                let response = ui.horizontal(|ui| {
                                    // Line number with arrow
                                    ui.label(
                                        RichText::new(format!("{line_num:>line_num_width$} →"))
                                            .color(palette::semantic::WARNING)
                                            .font(typography::monospace(typography::MD)),
                                    );
                                    ui.add_space(4.0);
                                    // Code content with syntax highlighting
                                    ui.label(highlighted);
                                });

                                // Draw background behind the row
                                let rect = response.response.rect.expand2(egui::vec2(4.0, 1.0));
                                ui.painter().rect_filled(rect, 2.0, highlight_bg);
                            } else {
                                ui.horizontal(|ui| {
                                    // Line number
                                    ui.label(
                                        RichText::new(format!("{line_num:>line_num_width$}  "))
                                            .color(line_num_color)
                                            .font(typography::monospace(typography::MD)),
                                    );
                                    ui.add_space(4.0);
                                    // Code content with syntax highlighting
                                    ui.label(highlighted);
                                });
                            }
                        }
                    });
                });
            });

        ui.add_space(8.0);
    }

    fn render_footer(
        &self,
        ui: &mut egui::Ui,
        muted_text: Color32,
        separator_color: Color32,
        popup_width: f32,
    ) {
        // Separator above footer
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator_color),
        );
        ui.add_space(8.0);

        let key_color = match self.theme {
            AppTheme::Light => palette::light_text::TERTIARY,
            AppTheme::Dark => palette::text::TERTIARY,
        };

        match self.preview_kind {
            PreviewKind::Metric => {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);

                    // Show location indicator if multiple locations exist
                    #[cfg(not(target_arch = "wasm32"))]
                    if self.metric_locations.len() > 1 {
                        ui.label(
                            RichText::new(format!(
                                "[{}/{}]",
                                self.current_location_index + 1,
                                self.metric_locations.len()
                            ))
                            .color(muted_text)
                            .font(typography::proportional(typography::MD)),
                        );
                        ui.add_space(12.0);
                    }

                    // Show labels if any
                    if !self.labels.is_empty() {
                        ui.label(
                            RichText::new("Labels: ")
                                .color(key_color)
                                .font(typography::proportional(typography::MD)),
                        );
                        ui.label(
                            RichText::new(self.labels.join(", "))
                                .color(text_color(self.theme))
                                .font(typography::monospace(typography::MD)),
                        );
                        ui.add_space(16.0);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);

                        // Show hint with N/P if multiple locations
                        #[cfg(not(target_arch = "wasm32"))]
                        let hint = if self.metric_locations.len() > 1 {
                            "N/P to cycle • Esc to close"
                        } else {
                            "Esc to close"
                        };
                        #[cfg(target_arch = "wasm32")]
                        let hint = "Esc to close";

                        ui.label(
                            RichText::new(hint)
                                .color(muted_text)
                                .font(typography::proportional(typography::MD)),
                        );
                    });
                });
            }
            PreviewKind::Alert => {
                // Constrain the vertical layout to the popup width minus margins
                let content_width = popup_width - 32.0;
                ui.vertical(|ui| {
                    ui.set_max_width(content_width);

                    // Show alert name
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("Alert: ")
                                .color(key_color)
                                .font(typography::proportional(typography::MD)),
                        );
                        ui.label(
                            RichText::new(&self.metric_name)
                                .color(text_color(self.theme))
                                .font(typography::monospace(typography::MD))
                                .strong(),
                        );
                    });

                    // Show message if available - constrained to popup width
                    if let Some(ref message) = self.alert_message {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            // Use a fixed max width based on popup size (leave room for margins)
                            let max_msg_width = content_width - 48.0;
                            ui.add_sized(
                                [max_msg_width, ui.spacing().interact_size.y],
                                egui::Label::new(
                                    RichText::new(message)
                                        .color(muted_text)
                                        .font(typography::proportional(typography::SM)),
                                )
                                .truncate(),
                            );
                        });
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new("Esc to close")
                                    .color(muted_text)
                                    .font(typography::proportional(typography::MD)),
                            );
                        });
                    });
                });
            }
        }
        ui.add_space(12.0);
    }

    /// Highlight a line of Rust code using cached tree-sitter spans.
    /// `line_num` is 1-indexed.
    #[cfg(not(target_arch = "wasm32"))]
    fn highlight_rust_line(&self, line_num: usize, line: &str) -> LayoutJob {
        let mut job = LayoutJob::default();
        let font_id = typography::monospace(typography::MD);
        let default_color = text_color(self.theme);

        // If we have no highlight spans or line offsets, fall back to plain text
        if self.highlight_spans.is_empty() || self.line_offsets.is_empty() {
            job.append(line, 0.0, egui::TextFormat::simple(font_id, default_color));
            return job;
        }

        // Get the byte range for this line (0-indexed)
        let line_idx = line_num.saturating_sub(1);
        let line_start = self.line_offsets.get(line_idx).copied().unwrap_or(0);
        let line_end = self
            .line_offsets
            .get(line_idx + 1)
            .copied()
            .unwrap_or(self.source_content.len());

        // Find spans that overlap with this line
        let mut current_pos = 0usize; // Position within the line string

        // Collect spans for this line, adjusting to line-relative positions
        let mut line_spans: Vec<(usize, usize, Color32)> = Vec::new();

        for span in &self.highlight_spans {
            // Check if span overlaps with this line
            if span.range.end <= line_start || span.range.start >= line_end {
                continue;
            }

            // Clamp span to line boundaries and convert to line-relative positions
            let span_start_in_line = span.range.start.saturating_sub(line_start);
            let span_end_in_line = span.range.end.saturating_sub(line_start).min(line.len());

            if span_start_in_line >= span_end_in_line {
                continue;
            }

            let color = self.highlight_color(span.highlight_idx);
            line_spans.push((span_start_in_line, span_end_in_line, color));
        }

        // Sort spans by start position
        line_spans.sort_by_key(|(start, _, _)| *start);

        // Build the LayoutJob by iterating through spans
        for (span_start, span_end, color) in line_spans {
            // Add any unhighlighted text before this span
            if span_start > current_pos {
                if let Some(text) = line.get(current_pos..span_start) {
                    job.append(
                        text,
                        0.0,
                        egui::TextFormat::simple(font_id.clone(), default_color),
                    );
                }
            }

            // Add the highlighted span
            if let Some(text) = line.get(span_start..span_end) {
                job.append(text, 0.0, egui::TextFormat::simple(font_id.clone(), color));
            }

            current_pos = span_end;
        }

        // Add any remaining unhighlighted text
        if current_pos < line.len() {
            if let Some(text) = line.get(current_pos..) {
                job.append(
                    text,
                    0.0,
                    egui::TextFormat::simple(font_id.clone(), default_color),
                );
            }
        }

        // Handle empty line
        if job.is_empty() && line.is_empty() {
            job.append("", 0.0, egui::TextFormat::simple(font_id, default_color));
        }

        job
    }

    /// Fallback for WASM - no syntax highlighting.
    #[cfg(target_arch = "wasm32")]
    fn highlight_rust_line(&self, _line_num: usize, line: &str) -> LayoutJob {
        let mut job = LayoutJob::default();
        let font_id = typography::monospace(typography::MD);
        let default_color = text_color(self.theme);
        job.append(line, 0.0, egui::TextFormat::simple(font_id, default_color));
        job
    }

    /// Get the color for a highlight index.
    #[cfg(not(target_arch = "wasm32"))]
    fn highlight_color(&self, idx: usize) -> Color32 {
        let name = HIGHLIGHT_NAMES.get(idx).copied().unwrap_or("");

        match self.theme {
            AppTheme::Light => match name {
                "keyword" => Color32::from_rgb(160, 50, 160), // purple
                "string" | "escape" => Color32::from_rgb(50, 130, 50), // green
                "comment" => Color32::from_rgb(120, 120, 120), // gray
                "function" | "function.builtin" => Color32::from_rgb(50, 100, 180), // blue
                "function.macro" => Color32::from_rgb(0, 130, 150), // teal
                "type" | "type.builtin" | "constructor" => Color32::from_rgb(180, 100, 50), // orange
                "number" | "constant" | "constant.builtin" => Color32::from_rgb(180, 80, 80), // red-ish
                "attribute" => Color32::from_rgb(150, 120, 50), // yellow-brown
                "variable" | "variable.parameter" | "property" | "label" => {
                    Color32::from_rgb(40, 40, 40)
                } // dark
                "variable.builtin" => Color32::from_rgb(160, 50, 160), // purple (self)
                "operator" | "punctuation" | "punctuation.bracket" | "punctuation.delimiter" => {
                    Color32::from_rgb(60, 60, 60) // dark gray
                }
                _ => Color32::from_rgb(40, 40, 40), // default dark
            },
            AppTheme::Dark => match name {
                "keyword" => Color32::from_rgb(200, 120, 220), // purple
                "string" | "escape" => Color32::from_rgb(130, 200, 130), // green
                "comment" => Color32::from_rgb(128, 128, 128), // gray
                "function" | "function.builtin" => Color32::from_rgb(100, 160, 255), // blue
                "function.macro" => Color32::from_rgb(80, 200, 200), // cyan
                "type" | "type.builtin" | "constructor" => Color32::from_rgb(220, 160, 100), // orange
                "number" | "constant" | "constant.builtin" => Color32::from_rgb(220, 120, 120), // red-ish
                "attribute" => Color32::from_rgb(220, 190, 100), // yellow
                "variable" | "variable.parameter" | "property" | "label" => {
                    Color32::from_rgb(220, 220, 220)
                } // light
                "variable.builtin" => Color32::from_rgb(200, 120, 220), // purple (self)
                "operator" | "punctuation" | "punctuation.bracket" | "punctuation.delimiter" => {
                    Color32::from_rgb(180, 180, 180) // light gray
                }
                _ => Color32::from_rgb(220, 220, 220), // default light
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_overlay_is_closed() {
        let overlay = SourcePreviewOverlay::new();
        assert!(!overlay.is_open());
    }

    #[test]
    fn test_open_close() {
        let mut overlay = SourcePreviewOverlay::new();
        overlay.open();
        assert!(overlay.is_open());
        overlay.close();
        assert!(!overlay.is_open());
    }

    #[test]
    fn test_open_error() {
        let mut overlay = SourcePreviewOverlay::new();
        overlay.open_error("my.metric", "File not found");
        assert!(overlay.is_open());
        assert!(overlay.error.is_some());
        assert_eq!(overlay.metric_name, "my.metric");
    }
}
