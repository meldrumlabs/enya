//! StatusLine component - A lualine-inspired status bar for the Enya UI.
//!
//! The status line displays contextual information at the bottom of the screen
//! in a segmented bar style similar to Neovim's lualine plugin.

use std::collections::VecDeque;

use egui::{Color32, Layout, Ui};

use crate::theme::AppTheme;

/// Unicode block characters for sparkline rendering (1/8 to 8/8 height)
const SPARKLINE_BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Maximum number of data points to show in a sparkline
const SPARKLINE_MAX_POINTS: usize = 15;

/// A sparkline displays a mini chart of recent values
#[derive(Debug, Clone)]
pub struct Sparkline {
    /// The label/name for this sparkline
    pub label: String,
    /// Recent values (newest at back)
    values: VecDeque<f64>,
    /// Minimum value for scaling (if None, auto-scale)
    min: Option<f64>,
    /// Maximum value for scaling (if None, auto-scale)
    max: Option<f64>,
    /// Unit suffix for display (e.g., "%", "ms")
    pub unit: String,
}

impl Sparkline {
    /// Create a new sparkline with the given label
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            values: VecDeque::with_capacity(SPARKLINE_MAX_POINTS),
            min: None,
            max: None,
            unit: String::new(),
        }
    }

    /// Set the unit suffix (e.g., "%", "ms", "req/s")
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    /// Set fixed min/max bounds for scaling
    pub fn with_bounds(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// Push a new value to the sparkline
    pub fn push(&mut self, value: f64) {
        if self.values.len() >= SPARKLINE_MAX_POINTS {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    /// Get the current (most recent) value
    pub fn current_value(&self) -> Option<f64> {
        self.values.back().copied()
    }

    /// Render the sparkline as a string of block characters
    pub fn render(&self) -> String {
        if self.values.is_empty() {
            return String::new();
        }

        // Calculate min/max for scaling
        let (min, max) = if let (Some(min), Some(max)) = (self.min, self.max) {
            (min, max)
        } else {
            let min = self.values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = self
                .values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            // Add small padding if min == max to avoid division by zero
            if (max - min).abs() < f64::EPSILON {
                (min - 1.0, max + 1.0)
            } else {
                (min, max)
            }
        };

        let range = max - min;

        self.values
            .iter()
            .map(|&v| {
                // Normalize value to 0.0-1.0 range
                let normalized = ((v - min) / range).clamp(0.0, 1.0);
                // Map to block index (0-7)
                let index = (normalized * 7.0).round() as usize;
                SPARKLINE_BLOCKS[index.min(7)]
            })
            .collect()
    }

    /// Format the current value for display
    pub fn format_current(&self) -> String {
        match self.current_value() {
            Some(v) => {
                if self.unit == "%" {
                    format!("{:.0}{}", v, self.unit)
                } else if v >= 1000.0 {
                    format!("{:.1}k{}", v / 1000.0, self.unit)
                } else if v >= 100.0 {
                    format!("{:.0}{}", v, self.unit)
                } else {
                    format!("{:.1}{}", v, self.unit)
                }
            }
            None => "—".to_string(),
        }
    }
}

/// Mode indicator for the status line (similar to vim modes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusMode {
    /// Normal dashboard mode
    #[default]
    Normal,
    /// Settings mode
    Settings,
    /// Home/welcome screen
    Home,
    /// Command mode (when command palette is open)
    Command,
    /// Search mode (when fuzzy finder is open)
    Search,
    /// Zen mode (distraction-free view)
    Zen,
    /// Fullscreen mode (single pane maximized)
    Fullscreen,
}

impl StatusMode {
    /// Get the display label for this mode
    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Settings => "SETTINGS",
            Self::Home => "HOME",
            Self::Command => "COMMAND",
            Self::Search => "SEARCH",
            Self::Zen => "ZEN",
            Self::Fullscreen => "FULLSCREEN",
        }
    }

    /// Get the background color for this mode's segment
    /// Uses Enya's color scheme: golden yellow primary, white/gray secondary
    pub fn color(&self, theme: AppTheme) -> Color32 {
        match self {
            Self::Normal => match theme {
                AppTheme::Light => Color32::from_rgb(180, 140, 20), // Golden yellow
                AppTheme::Dark => Color32::from_rgb(255, 200, 50),  // Bright gold
            },
            Self::Settings => match theme {
                AppTheme::Light => Color32::from_rgb(120, 120, 130), // Gray
                AppTheme::Dark => Color32::from_rgb(160, 160, 170),  // Light gray
            },
            Self::Home => match theme {
                AppTheme::Light => Color32::from_rgb(180, 140, 20), // Golden yellow
                AppTheme::Dark => Color32::from_rgb(255, 200, 50),  // Bright gold
            },
            Self::Command => match theme {
                AppTheme::Light => Color32::from_rgb(200, 160, 40), // Warm gold
                AppTheme::Dark => Color32::from_rgb(255, 210, 80),  // Light gold
            },
            Self::Search => match theme {
                AppTheme::Light => Color32::from_rgb(140, 140, 150), // Muted gray
                AppTheme::Dark => Color32::from_rgb(180, 180, 190),  // Light gray
            },
            Self::Zen => match theme {
                AppTheme::Light => Color32::from_rgb(80, 80, 85), // Dark gray
                AppTheme::Dark => Color32::from_rgb(60, 60, 65),  // Muted dark gray
            },
            Self::Fullscreen => match theme {
                AppTheme::Light => Color32::from_rgb(100, 100, 110), // Dark gray
                AppTheme::Dark => Color32::from_rgb(140, 140, 150),  // Medium gray
            },
        }
    }
}

/// Configuration for a status line segment
#[derive(Debug, Clone)]
pub struct StatusSegment {
    /// The text content of the segment
    pub content: String,
    /// Optional icon prefix (using Phosphor icons)
    pub icon: Option<&'static str>,
    /// Background color for this segment
    pub bg_color: Color32,
    /// Foreground (text) color for this segment
    pub fg_color: Color32,
}

impl StatusSegment {
    /// Create a new segment with the given content and colors
    pub fn new(content: impl Into<String>, bg_color: Color32, fg_color: Color32) -> Self {
        Self {
            content: content.into(),
            icon: None,
            bg_color,
            fg_color,
        }
    }

    /// Add an icon to this segment
    pub fn with_icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// The StatusLine component - displays at the bottom of the application window
pub struct StatusLine {
    /// Current theme
    theme: AppTheme,
    /// Current mode
    mode: StatusMode,
    /// Connection status
    is_connected: bool,
    /// Number of open charts/tabs
    open_tabs: usize,
    /// Currently selected metric (if any)
    selected_metric: Option<String>,
    /// Git branch or project info (optional)
    branch_info: Option<String>,
    /// Viewport info (e.g., "2 panes")
    viewport_info: Option<String>,
    /// Optional sparkline to display in the status bar
    sparkline: Option<Sparkline>,
}

impl Default for StatusLine {
    fn default() -> Self {
        Self {
            theme: AppTheme::Dark,
            mode: StatusMode::Normal,
            is_connected: false,
            open_tabs: 0,
            selected_metric: None,
            branch_info: None,
            viewport_info: None,
            sparkline: None,
        }
    }
}

impl StatusLine {
    /// Create a new StatusLine with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the current mode
    pub fn set_mode(&mut self, mode: StatusMode) {
        self.mode = mode;
    }

    /// Set the connection status
    pub fn set_connected(&mut self, connected: bool) {
        self.is_connected = connected;
    }

    /// Set the number of open tabs
    pub fn set_open_tabs(&mut self, count: usize) {
        self.open_tabs = count;
    }

    /// Set the currently selected metric
    pub fn set_selected_metric(&mut self, metric: Option<String>) {
        self.selected_metric = metric;
    }

    /// Set the branch info
    pub fn set_branch_info(&mut self, info: Option<String>) {
        self.branch_info = info;
    }

    /// Set viewport info
    pub fn set_viewport_info(&mut self, info: Option<String>) {
        self.viewport_info = info;
    }

    /// Set a sparkline to display in the status bar
    pub fn set_sparkline(&mut self, sparkline: Option<Sparkline>) {
        self.sparkline = sparkline;
    }

    /// Get the background color for segments based on theme
    fn segment_bg(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(68, 71, 90),
            AppTheme::Dark => Color32::from_rgb(40, 44, 52),
        }
    }

    /// Get the secondary background color for segments
    fn segment_bg_secondary(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(88, 91, 110),
            AppTheme::Dark => Color32::from_rgb(50, 54, 62),
        }
    }

    /// Get the text color for segments
    fn segment_fg(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(248, 248, 242),
            AppTheme::Dark => Color32::from_rgb(171, 178, 191),
        }
    }

    /// Render the status line
    pub fn show(&self, ui: &mut Ui) {
        let height = 24.0;
        let padding = 6.0;

        // Use a horizontal layout that spans the full width
        ui.horizontal(|ui| {
            ui.set_height(height);
            ui.spacing_mut().item_spacing.x = 0.0;

            // === LEFT SECTION ===
            self.render_left_section(ui, height, padding);

            // === CENTER SECTION (fills remaining space) ===
            self.render_center_section(ui, height, padding);

            // === RIGHT SECTION ===
            self.render_right_section(ui, height, padding);
        });
    }

    /// Render the left section of the status line
    fn render_left_section(&self, ui: &mut Ui, height: f32, padding: f32) {
        // Mode indicator (like vim mode in lualine)
        let mode_color = self.mode.color(self.theme);
        let mode_text_color = Color32::from_rgb(40, 44, 52); // Dark text on colored bg

        self.render_segment(
            ui,
            self.mode.label(),
            Some(egui_phosphor::regular::COMMAND),
            mode_color,
            mode_text_color,
            height,
            padding,
            true,
        );

        // Git branch / project info (if available)
        if let Some(ref branch) = self.branch_info {
            self.render_segment(
                ui,
                branch,
                Some(egui_phosphor::regular::GIT_BRANCH),
                self.segment_bg(),
                self.segment_fg(),
                height,
                padding,
                false,
            );
        }

        // Selected metric
        if let Some(ref metric) = self.selected_metric {
            // Truncate long metric names
            let display_name = if metric.len() > 30 {
                format!("{}...", &metric[..27])
            } else {
                metric.clone()
            };
            self.render_segment(
                ui,
                &display_name,
                Some(egui_phosphor::regular::CHART_LINE_UP),
                self.segment_bg_secondary(),
                self.segment_fg(),
                height,
                padding,
                false,
            );
        }

        // Sparkline with current value
        if let Some(ref sparkline) = self.sparkline {
            self.render_sparkline_segment(ui, sparkline, height, padding);
        }
    }

    /// Render the center section (expands to fill space)
    fn render_center_section(&self, ui: &mut Ui, _height: f32, _padding: f32) {
        // Fill remaining horizontal space
        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
            ui.add_space(ui.available_width());
        });
    }

    /// Render the right section of the status line
    fn render_right_section(&self, ui: &mut Ui, height: f32, padding: f32) {
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            // Position info (like cursor position in vim)
            let position_text = format!("{} tabs", self.open_tabs);
            self.render_segment_rtl(
                ui,
                &position_text,
                Some(egui_phosphor::regular::TABS),
                self.mode.color(self.theme),
                Color32::from_rgb(40, 44, 52),
                height,
                padding,
                true,
            );

            // Viewport info (e.g., pane layout)
            if let Some(ref viewport) = self.viewport_info {
                self.render_segment_rtl(
                    ui,
                    viewport,
                    Some(egui_phosphor::regular::SQUARES_FOUR),
                    self.segment_bg(),
                    self.segment_fg(),
                    height,
                    padding,
                    false,
                );
            }

            // Connection status
            let (conn_icon, conn_text, conn_color) = if self.is_connected {
                (
                    egui_phosphor::regular::WIFI_HIGH,
                    "CONNECTED",
                    Color32::from_rgb(152, 195, 121), // Green
                )
            } else {
                (
                    egui_phosphor::regular::WIFI_SLASH,
                    "OFFLINE",
                    Color32::from_rgb(224, 108, 117), // Red
                )
            };
            self.render_segment_rtl(
                ui,
                conn_text,
                Some(conn_icon),
                self.segment_bg_secondary(),
                conn_color,
                height,
                padding,
                false,
            );
        });
    }

    /// Render a single segment (left-to-right)
    #[allow(clippy::too_many_arguments)]
    fn render_segment(
        &self,
        ui: &mut Ui,
        text: &str,
        icon: Option<&str>,
        bg_color: Color32,
        fg_color: Color32,
        height: f32,
        padding: f32,
        _bold: bool,
    ) {
        let content = if let Some(icon) = icon {
            format!("{icon} {text}")
        } else {
            text.to_string()
        };

        // Calculate width needed for the text
        let galley = ui.painter().layout_no_wrap(
            content.clone(),
            egui::FontId::proportional(12.0),
            fg_color,
        );
        let text_width = galley.size().x + padding * 2.0;

        // Draw the segment background and text
        let (rect, _response) =
            ui.allocate_exact_size(egui::vec2(text_width, height), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, 0.0, bg_color);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &content,
                egui::FontId::proportional(12.0),
                fg_color,
            );
        }
    }

    /// Render a single segment (right-to-left layout)
    #[allow(clippy::too_many_arguments)]
    fn render_segment_rtl(
        &self,
        ui: &mut Ui,
        text: &str,
        icon: Option<&str>,
        bg_color: Color32,
        fg_color: Color32,
        height: f32,
        padding: f32,
        _bold: bool,
    ) {
        let content = if let Some(icon) = icon {
            format!("{icon} {text}")
        } else {
            text.to_string()
        };

        // Calculate width needed for the text
        let galley = ui.painter().layout_no_wrap(
            content.clone(),
            egui::FontId::proportional(12.0),
            fg_color,
        );
        let text_width = galley.size().x + padding * 2.0;

        // Draw the segment background and text
        let (rect, _response) =
            ui.allocate_exact_size(egui::vec2(text_width, height), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, 0.0, bg_color);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &content,
                egui::FontId::proportional(12.0),
                fg_color,
            );
        }
    }

    /// Render a sparkline segment with current value and label
    /// Format: "▁▂▃▅▇▅▃▂▄▆▇▅▃ 16.7ms 60 fps"
    fn render_sparkline_segment(
        &self,
        ui: &mut Ui,
        sparkline: &Sparkline,
        height: f32,
        padding: f32,
    ) {
        let bg_color = self.segment_bg();
        let fg_color = self.segment_fg();

        // Sparkline color - use a cyan/teal for the chart to stand out
        let sparkline_color = match self.theme {
            AppTheme::Light => Color32::from_rgb(20, 140, 140), // Teal
            AppTheme::Dark => Color32::from_rgb(80, 200, 200),  // Bright cyan
        };

        // Build the content: "▁▂▃▅▇ 16.7ms label"
        let chart = sparkline.render();
        let value = sparkline.format_current();
        let label = &sparkline.label;

        // Calculate widths for each part
        let chart_galley = ui.painter().layout_no_wrap(
            chart.clone(),
            egui::FontId::proportional(12.0),
            sparkline_color,
        );
        let value_galley = ui.painter().layout_no_wrap(
            format!(" {value}"),
            egui::FontId::proportional(12.0),
            fg_color,
        );
        let label_galley = ui.painter().layout_no_wrap(
            format!(" {label}"),
            egui::FontId::proportional(12.0),
            fg_color,
        );

        let total_width =
            chart_galley.size().x + value_galley.size().x + label_galley.size().x + padding * 2.0;

        // Allocate the rectangle
        let (rect, _response) =
            ui.allocate_exact_size(egui::vec2(total_width, height), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            // Draw background
            ui.painter().rect_filled(rect, 0.0, bg_color);

            // Draw sparkline chart (colored)
            let chart_x = rect.min.x + padding;
            ui.painter().text(
                egui::pos2(chart_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &chart,
                egui::FontId::proportional(12.0),
                sparkline_color,
            );

            // Draw value
            let value_x = chart_x + chart_galley.size().x;
            ui.painter().text(
                egui::pos2(value_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                format!(" {value}"),
                egui::FontId::proportional(12.0),
                fg_color,
            );

            // Draw label (slightly muted)
            let label_x = value_x + value_galley.size().x;
            ui.painter().text(
                egui::pos2(label_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                format!(" {label}"),
                egui::FontId::proportional(12.0),
                fg_color.gamma_multiply(0.7),
            );
        }
    }
}
