//! StatusLine component - A lualine-inspired status bar for the Enya UI.
//!
//! The status line displays contextual information at the bottom of the screen
//! in a segmented bar style similar to Neovim's lualine plugin.

use std::collections::VecDeque;

use egui::{Color32, Layout, Ui};

use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// State for inline agent input in the status line
pub struct InlineAgentInput<'a> {
    /// The input text buffer
    pub text: &'a mut String,
    /// Whether the input should be focused
    pub focus: &'a mut bool,
    /// Provider name (e.g., "Claude", "Codex")
    pub provider_name: &'a str,
}

/// Result from showing the status line with agent input
#[derive(Debug, Clone, Default)]
pub struct StatusLineResult {
    /// User submitted a query (pressed Enter)
    pub query_submitted: Option<String>,
    /// User wants to exit agent mode (pressed Escape)
    pub exit_requested: bool,
}

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
/// Note: Zen and Fullscreen are display preferences, not modes - they stay in Normal mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusMode {
    /// Normal dashboard mode
    #[default]
    Normal,
    /// Home/welcome screen
    Home,
    /// Command mode (when command palette is open)
    Command,
    /// Search mode (when fuzzy finder is open)
    Search,
    /// Diff mode (comparing time periods)
    Diff,
    /// Visual multi-select mode (selecting multiple panes)
    VisualMulti,
    /// Agent mode (AI-assisted interaction)
    Agent,
}

impl StatusMode {
    /// Get the display label for this mode
    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Home => "HOME",
            Self::Command => "COMMAND",
            Self::Search => "SEARCH",
            Self::Diff => "DIFF",
            Self::VisualMulti => "V-MULTI",
            Self::Agent => "AGENT",
        }
    }

    /// Get the background color for this mode's segment
    /// Uses Enya's color scheme: theme accent as primary, other colors for secondary modes
    pub fn color(&self, theme: AppTheme) -> Color32 {
        match self {
            Self::Normal | Self::Home => theme.accent_primary(),
            Self::Command | Self::VisualMulti => theme.accent_hover(),
            Self::Search => theme.text_secondary(),
            Self::Diff => theme.semantic_info(),
            Self::Agent => theme.semantic_warning(),
        }
    }

    /// Get the text color for this mode's segment
    /// Uses contrasting text for visibility
    pub fn text_color(&self, theme: AppTheme) -> Color32 {
        // Light theme uses dark accent backgrounds (ink aesthetic) - need light text
        if theme.is_light() {
            return match self {
                Self::Search => Color32::from_rgb(30, 30, 30), // Dark text on light bg
                _ => Color32::from_rgb(250, 248, 245),         // Cream/paper text on dark bg
            };
        }

        // Dark themes: bright accent backgrounds - use dark text
        match self {
            // Most modes have bright backgrounds - use dark text
            Self::Normal | Self::Home | Self::Command | Self::VisualMulti | Self::Agent => {
                Color32::from_rgb(10, 10, 10)
            }
            // Search uses secondary text color as bg - use contrasting color
            Self::Search => Color32::from_rgb(255, 255, 255),
            // Blue backgrounds - use white text
            Self::Diff => Color32::from_rgb(255, 255, 255),
        }
    }
}

/// Codebase status information for status line display
#[derive(Debug, Clone, Default)]
pub struct CodebaseStatusInfo {
    /// Status message (e.g., "Cloning...", "Indexing main.rs + 5 more", "42 metrics")
    pub message: String,
    /// Repository name (when ready)
    pub repo_name: Option<String>,
    /// Number of metrics discovered (when ready)
    pub metrics_count: Option<usize>,
    /// Language being used for scanning
    pub language: Option<String>,
    /// HEAD commit message (subject line) when ready
    pub commit_msg: Option<String>,
    /// HEAD commit hash (short form, e.g., "abc1234") when ready
    pub commit_hash: Option<String>,
    /// Whether an operation is in progress
    pub is_loading: bool,
    /// Whether there's an error
    pub is_error: bool,
    /// Whether Tantivy full-text search index is being built (background task after tree-sitter)
    pub is_tantivy_indexing: bool,
    /// Tantivy indexing phase label (e.g., "Indexing commits")
    pub tantivy_phase: Option<String>,
    /// Tantivy indexing current item (e.g., commit hash or metric name)
    pub tantivy_item: Option<String>,
    /// Tantivy indexing progress (current, total)
    pub tantivy_progress: Option<(usize, usize)>,
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
    /// Agent provider name (e.g., "Claude", "Codex") - shown as mode badge when in Agent mode
    agent_provider_name: Option<String>,
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
    /// Extra status message (e.g., multi-buffer edit status)
    extra_status: Option<String>,
    /// Optional sparkline to display in the status bar
    sparkline: Option<Sparkline>,
    /// Timestamp of last data refresh (for relative time display)
    last_refresh: Option<std::time::Instant>,
    /// Diagnostics counts (errors, warnings, infos)
    diagnostics_count: (usize, usize, usize),
    /// Codebase operation status
    codebase_status: Option<CodebaseStatusInfo>,
    /// Whether zen mode is active (display preference badge)
    is_zen_mode: bool,
    /// Whether fullscreen mode is active (display preference badge)
    is_fullscreen: bool,
}

impl Default for StatusLine {
    fn default() -> Self {
        Self {
            theme: AppTheme::default(),
            mode: StatusMode::Normal,
            agent_provider_name: None,
            is_connected: false,
            open_tabs: 0,
            selected_metric: None,
            branch_info: None,
            viewport_info: None,
            extra_status: None,
            sparkline: None,
            last_refresh: None,
            diagnostics_count: (0, 0, 0),
            codebase_status: None,
            is_zen_mode: false,
            is_fullscreen: false,
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

    /// Set the agent provider name (shown as mode badge when in Agent mode)
    pub fn set_agent_provider_name(&mut self, name: Option<String>) {
        self.agent_provider_name = name;
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

    /// Set extra status message (e.g., multi-buffer edit status)
    pub fn set_extra_status(&mut self, status: Option<String>) {
        self.extra_status = status;
    }

    /// Set a sparkline to display in the status bar
    pub fn set_sparkline(&mut self, sparkline: Option<Sparkline>) {
        self.sparkline = sparkline;
    }

    /// Set diagnostics counts (errors, warnings, infos)
    pub fn set_diagnostics_count(&mut self, errors: usize, warnings: usize, infos: usize) {
        self.diagnostics_count = (errors, warnings, infos);
    }

    /// Set codebase status info
    pub fn set_codebase_status(&mut self, status: Option<CodebaseStatusInfo>) {
        self.codebase_status = status;
    }

    /// Set zen mode state (for display preference badge)
    pub fn set_zen_mode(&mut self, is_zen: bool) {
        self.is_zen_mode = is_zen;
    }

    /// Set fullscreen state (for display preference badge)
    pub fn set_fullscreen(&mut self, is_fullscreen: bool) {
        self.is_fullscreen = is_fullscreen;
    }

    /// Mark the last refresh time (call when data is updated)
    #[allow(dead_code)]
    pub fn mark_refresh(&mut self) {
        self.last_refresh = Some(std::time::Instant::now());
    }

    /// Render the status line
    #[profiling::function]
    pub fn show(&self, ui: &mut Ui) -> StatusLineResult {
        self.show_with_extra_content(ui, |_| {})
    }

    /// Render the status line with custom content after the mode badge
    /// The closure is called right after the mode badge to render inline content
    #[profiling::function]
    pub fn show_with_extra_content<F>(&self, ui: &mut Ui, render_after_mode: F) -> StatusLineResult
    where
        F: FnOnce(&mut Ui),
    {
        let result = StatusLineResult::default();
        let height = 26.0; // Slightly taller for breathing room
        let padding = 8.0; // More generous padding

        // Premium status line styling
        let status_bg = self.theme.bg_surface();

        // Use clip_rect width to ensure we paint to the full visible window width
        // available_rect might be constrained by parent layouts, but clip_rect is the drawable area
        let clip_rect = ui.clip_rect();
        let available_rect = ui.available_rect_before_wrap();

        // Use available_rect for vertical position, but clip_rect width for full coverage
        let full_width = clip_rect.width().max(available_rect.width());
        let left_x = clip_rect.left().min(available_rect.left());

        // Paint the full background BEFORE entering the horizontal layout
        // This ensures the background spans the entire width regardless of content
        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(left_x, available_rect.min.y),
            egui::vec2(full_width, height),
        );
        ui.painter().rect_filled(bar_rect, 0.0, status_bg);

        // Use a horizontal layout that spans the full width
        ui.horizontal(|ui| {
            ui.set_height(height);
            ui.spacing_mut().item_spacing.x = 0.0;

            // === LEFT SECTION (mode badge only when extra content provided) ===
            self.render_mode_badge(ui, height, padding);

            // === CUSTOM CONTENT (e.g., agent input bar) ===
            render_after_mode(ui);

            // === REST OF LEFT SECTION ===
            self.render_left_section_after_mode(ui, height, padding);

            // === CENTER SECTION (extra status if any) ===
            self.render_center_section(ui, height, padding);

            // === RIGHT SECTION (fills remaining space, content aligned right) ===
            let remaining_rect = ui.available_rect_before_wrap();
            ui.scope_builder(egui::UiBuilder::new().max_rect(remaining_rect), |ui| {
                self.render_right_section(ui, height, padding);
            });
        });

        result
    }

    /// Render just the mode badge (used when extra content is being injected)
    fn render_mode_badge(&self, ui: &mut Ui, height: f32, padding: f32) {
        // In Agent mode with a provider name, show the provider as the mode badge
        if self.mode == StatusMode::Agent {
            if let Some(ref provider_name) = self.agent_provider_name {
                self.render_agent_provider_badge(ui, provider_name, height, padding);
                return;
            }
        }

        // Default mode badge for all other modes
        let mode_bg = self.mode.color(self.theme);
        let mode_fg = self.mode.text_color(self.theme);

        self.render_segment(
            ui,
            self.mode.label(),
            Some(semantic_icons::mode::COMMAND),
            mode_bg,
            mode_fg,
            height,
            padding,
            true,
        );
    }

    /// Render the agent provider badge (logo + name) as the mode indicator
    fn render_agent_provider_badge(
        &self,
        ui: &mut Ui,
        provider_name: &str,
        height: f32,
        padding: f32,
    ) {
        let mode_color = self.mode.color(self.theme);
        let mode_text_color = self.mode.text_color(self.theme);
        let logo_size = typography::MD;
        let provider_lower = provider_name.to_lowercase();

        // Calculate badge width: logo + space + text + padding
        let icon_width = logo_size + 6.0;
        let text_galley = ui.painter().layout_no_wrap(
            provider_name.to_string(),
            typography::proportional(typography::MD),
            mode_text_color,
        );
        let badge_width = icon_width + text_galley.size().x + padding * 2.0;

        // Allocate badge rect
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(badge_width, height), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            // Premium primary segment styling with rounded right edge
            let corner_radius = egui::CornerRadius {
                nw: 0,
                ne: 4,
                sw: 0,
                se: 4,
            };

            // Subtle glow behind
            let glow_rect = rect.expand(1.0);
            ui.painter()
                .rect_filled(glow_rect, corner_radius, mode_color.gamma_multiply(0.3));
            ui.painter().rect_filled(rect, corner_radius, mode_color);

            // Inner top highlight for 3D effect
            let highlight_rect = egui::Rect::from_min_size(
                rect.left_top() + egui::vec2(0.0, 1.0),
                egui::vec2(rect.width(), 1.0),
            );
            ui.painter().rect_filled(
                highlight_rect,
                0.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 20),
            );

            // Draw provider logo
            let logo_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + padding, rect.center().y - logo_size / 2.0),
                egui::vec2(logo_size, logo_size),
            );

            let image_source = if provider_lower.contains("claude") {
                Some(egui::include_image!("../../../assets/claude.png"))
            } else if provider_lower.contains("openai")
                || provider_lower.contains("codex")
                || provider_lower.contains("gpt")
            {
                Some(egui::include_image!("../../../assets/openai.png"))
            } else {
                None
            };

            if let Some(source) = image_source {
                let image = egui::Image::new(source).tint(mode_text_color);
                image.paint_at(ui, logo_rect);
            }

            // Draw provider name text
            ui.painter().text(
                egui::pos2(rect.min.x + padding + icon_width, rect.center().y),
                egui::Align2::LEFT_CENTER,
                provider_name,
                typography::proportional(typography::MD),
                mode_text_color,
            );
        }
    }

    /// Render the left section after the mode badge (zen/fullscreen badges, branch, metric, sparkline)
    fn render_left_section_after_mode(&self, ui: &mut Ui, height: f32, padding: f32) {
        // Display preference badges (zen/fullscreen) - use theme colors (Custom variant handles plugin colors internally)
        if self.is_zen_mode {
            let bg = self.theme.badge_zen_bg();
            let fg = self.theme.badge_zen_fg();
            ui.add_space(4.0);
            self.render_segment(ui, "ZEN", None, bg, fg, height, padding, false);
        }

        if self.is_fullscreen {
            let bg = self.theme.badge_fullscreen_bg();
            let fg = self.theme.badge_fullscreen_fg();
            ui.add_space(4.0);
            self.render_segment(ui, "FULLSCREEN", None, bg, fg, height, padding, false);
        }

        // Git branch / project info (if available)
        if let Some(ref branch) = self.branch_info {
            self.render_separator(ui, height);
            self.render_segment(
                ui,
                branch,
                Some(semantic_icons::git::BRANCH),
                self.theme.bg_surface(),
                self.theme.text_secondary(),
                height,
                padding,
                false,
            );
        }

        // Selected metric
        if let Some(ref metric) = self.selected_metric {
            self.render_separator(ui, height);
            let display_name = if metric.len() > 30 {
                format!("{}...", &metric[..27])
            } else {
                metric.clone()
            };
            self.render_segment(
                ui,
                &display_name,
                Some(semantic_icons::action::CHART),
                self.theme.bg_elevated(),
                self.theme.text_secondary(),
                height,
                padding,
                false,
            );
        }

        // Sparkline with current value (hidden in agent mode to save space)
        if self.mode != StatusMode::Agent {
            if let Some(ref sparkline) = self.sparkline {
                self.render_separator(ui, height);
                self.render_sparkline_segment(ui, sparkline, height, padding);
            }
        }
    }

    /// Render a subtle separator between segments (left-to-right)
    fn render_separator(&self, ui: &mut Ui, height: f32) {
        let separator_width = 20.0; // Slightly wider for breathing room
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(separator_width, height), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            // Premium: use a thin vertical line instead of chevron for cleaner look
            let line_color = self.theme.text_secondary().gamma_multiply(0.15);
            let center_x = rect.center().x;
            ui.painter().vline(
                center_x,
                egui::Rangef::new(rect.min.y + 6.0, rect.max.y - 6.0),
                egui::Stroke::new(1.0, line_color),
            );
        }
    }

    /// Render the center section (shows extra status if available, doesn't consume all space)
    fn render_center_section(&self, ui: &mut Ui, _height: f32, _padding: f32) {
        // Show extra status if available, but don't consume all remaining space
        if let Some(ref extra) = self.extra_status {
            ui.add_space(16.0);
            ui.label(
                egui::RichText::new(extra)
                    .color(self.theme.text_secondary())
                    .size(typography::MD)
                    .family(egui::FontFamily::Monospace),
            );
        }
    }

    /// Get the single health indicator for the minimalist status line
    /// Returns (icon, color, tooltip_text)
    fn get_health_indicator(&self) -> (&'static str, Color32, &'static str) {
        let (errors, warnings, _) = self.diagnostics_count;

        if errors > 0 {
            (
                semantic_icons::diagnostic::ERROR,
                self.theme.semantic_error(),
                "Errors detected (Space+d)",
            )
        } else if !self.is_connected {
            (
                semantic_icons::status::DISCONNECTED,
                self.theme.semantic_error(),
                "Connection lost (Space+d)",
            )
        } else if warnings > 0 {
            (
                semantic_icons::diagnostic::WARNING,
                self.theme.semantic_warning(),
                "Warnings present (Space+d)",
            )
        } else {
            (
                semantic_icons::status::SUCCESS,
                self.theme.semantic_success(),
                "All systems operational (Space+d)",
            )
        }
    }

    /// Render the right section of the status line (minimalist design)
    fn render_right_section(&self, ui: &mut Ui, height: f32, padding: f32) {
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            // Health indicator (far right) - simple colored icon with tooltip
            let (icon, color, tooltip) = self.get_health_indicator();
            ui.add_space(padding);
            let response = ui.label(egui::RichText::new(icon).color(color).size(typography::MD));
            if response.hovered() {
                response.show_tooltip_text(tooltip);
            }

            // Separator between health indicator and codebase status
            self.render_separator_rtl(ui, height);

            // Codebase status (repo name + commit message, or indexing state)
            if let Some(ref status) = self.codebase_status {
                let is_indexing =
                    (status.is_loading || status.is_tantivy_indexing) && !status.is_error;

                if status.is_error {
                    // Error state - truncate long error messages
                    let (display_msg, is_truncated) = if status.message.chars().count() > 30 {
                        let boundary = status
                            .message
                            .char_indices()
                            .nth(29)
                            .map_or(status.message.len(), |(i, _)| i);
                        (format!("{}…", &status.message[..boundary]), true)
                    } else {
                        (status.message.clone(), false)
                    };
                    let response = self.render_segment_rtl_with_response(
                        ui,
                        &display_msg,
                        Some(semantic_icons::diagnostic::ERROR),
                        self.theme.bg_surface(),
                        palette::semantic::ERROR,
                        height,
                        padding,
                        false,
                    );
                    // Show full error message tooltip when truncated and hovered
                    if is_truncated && response.hovered() {
                        response.show_tooltip_text(&status.message);
                    }
                } else if is_indexing {
                    // Indexing state - show spinner + "Indexing" in accent color
                    let accent = self.theme.accent_primary();

                    // Render "Indexing" text (RTL, so this appears on the right)
                    ui.label(
                        egui::RichText::new("Indexing")
                            .color(accent)
                            .size(typography::MD),
                    );
                    ui.add_space(6.0);

                    // Braille spinner
                    const BRAILLE_FRAMES: [char; 10] =
                        ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                    let time = ui.ctx().input(|i| i.time);
                    let frame_index = ((time * 10.0) as usize) % BRAILLE_FRAMES.len();
                    ui.label(
                        egui::RichText::new(BRAILLE_FRAMES[frame_index].to_string())
                            .color(accent)
                            .size(typography::MD),
                    );
                    ui.add_space(8.0);

                    // Show repo name if available (appears to the left of spinner)
                    if let Some(ref repo_name) = status.repo_name {
                        let icon = status
                            .language
                            .as_ref()
                            .and_then(|lang| semantic_icons::language::from_name(lang))
                            .unwrap_or(semantic_icons::file::CODE);

                        self.render_segment_rtl(
                            ui,
                            repo_name,
                            Some(icon),
                            Color32::TRANSPARENT,
                            palette::text::SECONDARY,
                            height,
                            padding,
                            false,
                        );
                    }
                } else if let Some(ref repo_name) = status.repo_name {
                    // Ready state - show repo name with short commit hash
                    // Use git branch icon since we're displaying repo/commit info
                    let icon = semantic_icons::git::BRANCH;

                    // Display: repo_name · abc1234 (short hash)
                    // Hover: shows full commit message
                    let display_name = if let Some(ref hash) = status.commit_hash {
                        // Use short hash (first 7 chars)
                        let short_hash = if hash.len() > 7 { &hash[..7] } else { hash };
                        format!("{repo_name} · {short_hash}")
                    } else {
                        repo_name.clone()
                    };

                    let response = self.render_segment_rtl_with_response(
                        ui,
                        &display_name,
                        Some(icon),
                        Color32::TRANSPARENT,
                        palette::text::SECONDARY,
                        height,
                        padding,
                        false,
                    );

                    // Show commit message on hover (if available)
                    if response.hovered() {
                        if let Some(ref msg) = status.commit_msg {
                            response.show_tooltip_text(msg);
                        }
                    }
                } else {
                    // Fallback - just show message
                    self.render_segment_rtl(
                        ui,
                        &status.message,
                        Some(semantic_icons::file::CODE),
                        Color32::TRANSPARENT,
                        palette::text::SECONDARY,
                        height,
                        padding,
                        false,
                    );
                }
            }
        });
    }

    /// Render a subtle separator between segments (right-to-left)
    fn render_separator_rtl(&self, ui: &mut Ui, height: f32) {
        let separator_width = 20.0; // Slightly wider for breathing room
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(separator_width, height), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            // Premium: use a thin vertical line instead of chevron for cleaner look
            let line_color = self.theme.text_secondary().gamma_multiply(0.15);
            let center_x = rect.center().x;
            ui.painter().vline(
                center_x,
                egui::Rangef::new(rect.min.y + 6.0, rect.max.y - 6.0),
                egui::Stroke::new(1.0, line_color),
            );
        }
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
        is_primary: bool,
    ) {
        // Calculate width needed - measure icon and text separately for proper alignment
        let icon_width = if let Some(icon) = icon {
            let galley = ui.painter().layout_no_wrap(
                icon.to_string(),
                typography::proportional(typography::MD),
                fg_color,
            );
            galley.size().x
        } else {
            0.0
        };

        let text_galley = ui.painter().layout_no_wrap(
            text.to_string(),
            typography::proportional(typography::MD),
            fg_color,
        );
        let text_width = text_galley.size().x;

        // Total width: padding + icon + spacing + text + padding
        let icon_text_spacing = if icon.is_some() { 4.0 } else { 0.0 };
        let total_width = padding + icon_width + icon_text_spacing + text_width + padding;

        // Draw the segment background and text
        let (rect, _response) =
            ui.allocate_exact_size(egui::vec2(total_width, height), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            if is_primary {
                // Premium primary segment with rounded right edge and subtle inner glow
                let corner_radius = egui::CornerRadius {
                    nw: 0,
                    ne: 4,
                    sw: 0,
                    se: 4,
                };
                // Subtle glow behind
                let glow_rect = rect.expand(1.0);
                ui.painter()
                    .rect_filled(glow_rect, corner_radius, bg_color.gamma_multiply(0.3));
                ui.painter().rect_filled(rect, corner_radius, bg_color);

                // Inner top highlight for 3D effect
                let highlight_rect = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(0.0, 1.0),
                    egui::vec2(rect.width(), 1.0),
                );
                ui.painter().rect_filled(
                    highlight_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 20),
                );
            } else {
                ui.painter().rect_filled(rect, 0.0, bg_color);
            }

            // Render icon and text separately for proper vertical alignment
            let center_y = rect.center().y;
            let mut x = rect.left() + padding;

            if let Some(icon) = icon {
                // Render icon centered vertically
                ui.painter().text(
                    egui::pos2(x, center_y),
                    egui::Align2::LEFT_CENTER,
                    icon,
                    typography::proportional(typography::MD),
                    fg_color,
                );
                x += icon_width + icon_text_spacing;
            }

            // Render text centered vertically
            ui.painter().text(
                egui::pos2(x, center_y),
                egui::Align2::LEFT_CENTER,
                text,
                typography::proportional(typography::MD),
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
        bold: bool,
    ) {
        self.render_segment_rtl_with_response(
            ui, text, icon, bg_color, fg_color, height, padding, bold,
        );
    }

    /// Render a single segment (right-to-left layout) and return the response for interactions
    #[allow(clippy::too_many_arguments)]
    fn render_segment_rtl_with_response(
        &self,
        ui: &mut Ui,
        text: &str,
        icon: Option<&str>,
        bg_color: Color32,
        fg_color: Color32,
        height: f32,
        padding: f32,
        is_primary: bool,
    ) -> egui::Response {
        // Calculate width needed - measure icon and text separately for proper alignment
        let icon_width = if let Some(icon) = icon {
            let galley = ui.painter().layout_no_wrap(
                icon.to_string(),
                typography::proportional(typography::MD),
                fg_color,
            );
            galley.size().x
        } else {
            0.0
        };

        let text_galley = ui.painter().layout_no_wrap(
            text.to_string(),
            typography::proportional(typography::MD),
            fg_color,
        );
        let text_width = text_galley.size().x;

        // Total width: padding + icon + spacing + text + padding
        let icon_text_spacing = if icon.is_some() { 4.0 } else { 0.0 };
        let total_width = padding + icon_width + icon_text_spacing + text_width + padding;

        // Draw the segment background and text
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(total_width, height), egui::Sense::click());

        if ui.is_rect_visible(rect) {
            if is_primary {
                // Premium primary segment with rounded left edge and subtle inner glow
                let corner_radius = egui::CornerRadius {
                    nw: 4,
                    ne: 0,
                    sw: 4,
                    se: 0,
                };
                // Subtle glow behind
                let glow_rect = rect.expand(1.0);
                ui.painter()
                    .rect_filled(glow_rect, corner_radius, bg_color.gamma_multiply(0.3));
                ui.painter().rect_filled(rect, corner_radius, bg_color);

                // Inner top highlight for 3D effect
                let highlight_rect = egui::Rect::from_min_size(
                    rect.left_top() + egui::vec2(0.0, 1.0),
                    egui::vec2(rect.width(), 1.0),
                );
                ui.painter().rect_filled(
                    highlight_rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 20),
                );
            } else {
                ui.painter().rect_filled(rect, 0.0, bg_color);
            }

            // Render icon and text separately for proper vertical alignment
            let center_y = rect.center().y;
            let mut x = rect.left() + padding;

            if let Some(icon) = icon {
                // Render icon centered vertically
                ui.painter().text(
                    egui::pos2(x, center_y),
                    egui::Align2::LEFT_CENTER,
                    icon,
                    typography::proportional(typography::MD),
                    fg_color,
                );
                x += icon_width + icon_text_spacing;
            }

            // Render text centered vertically
            ui.painter().text(
                egui::pos2(x, center_y),
                egui::Align2::LEFT_CENTER,
                text,
                typography::proportional(typography::MD),
                fg_color,
            );
        }

        response
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
        let bg_color = self.theme.bg_surface();
        let fg_color = self.theme.text_secondary();

        // Sparkline color - use accent for brand consistency
        let sparkline_color = self.theme.accent_hover();

        // Build the content: "▁▂▃▅▇ 16.7ms label"
        let chart = sparkline.render();
        let value = sparkline.format_current();
        let label = &sparkline.label;

        // Calculate widths for each part
        let chart_galley = ui.painter().layout_no_wrap(
            chart.clone(),
            typography::proportional(typography::MD),
            sparkline_color,
        );
        let value_galley = ui.painter().layout_no_wrap(
            format!(" {value}"),
            typography::proportional(typography::MD),
            fg_color,
        );
        let label_galley = ui.painter().layout_no_wrap(
            format!(" {label}"),
            typography::proportional(typography::MD),
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
                typography::proportional(typography::MD),
                sparkline_color,
            );

            // Draw value
            let value_x = chart_x + chart_galley.size().x;
            ui.painter().text(
                egui::pos2(value_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                format!(" {value}"),
                typography::proportional(typography::MD),
                fg_color,
            );

            // Draw label (slightly muted)
            let label_x = value_x + value_galley.size().x;
            ui.painter().text(
                egui::pos2(label_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                format!(" {label}"),
                typography::proportional(typography::MD),
                fg_color.gamma_multiply(0.7),
            );
        }
    }
}
