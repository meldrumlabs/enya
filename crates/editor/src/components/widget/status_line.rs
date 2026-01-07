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

use super::team_status::TeamStatusInfo;

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
    /// Team collaboration status (only shown when connected to a team)
    team_status: Option<TeamStatusInfo>,
}

impl Default for StatusLine {
    fn default() -> Self {
        Self {
            theme: AppTheme::default(),
            mode: StatusMode::Normal,
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
            team_status: None,
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

    /// Set team collaboration status (only shown when connected to a team)
    pub fn set_team_status(&mut self, status: Option<TeamStatusInfo>) {
        self.team_status = status;
    }

    /// Mark the last refresh time (call when data is updated)
    pub fn mark_refresh(&mut self) {
        self.last_refresh = Some(std::time::Instant::now());
    }

    /// Format the relative time since last refresh
    fn format_relative_time(&self) -> Option<String> {
        let elapsed = self.last_refresh?.elapsed();
        let secs = elapsed.as_secs();

        let result = if secs < 5 {
            "just now".to_string()
        } else if secs < 60 {
            format!("{secs}s ago")
        } else if secs < 3600 {
            let mins = secs / 60;
            format!("{mins}m ago")
        } else {
            let hours = secs / 3600;
            format!("{hours}h ago")
        };
        Some(result)
    }

    /// Get the background color for segments based on theme
    fn segment_bg(&self) -> Color32 {
        self.theme.bg_surface()
    }

    /// Get the secondary background color for segments
    fn segment_bg_secondary(&self) -> Color32 {
        self.theme.bg_elevated()
    }

    /// Get the text color for segments
    fn segment_fg(&self) -> Color32 {
        self.theme.text_secondary()
    }

    /// Render the status line
    #[profiling::function]
    pub fn show(&self, ui: &mut Ui) {
        let height = 26.0; // Slightly taller for breathing room
        let padding = 8.0; // More generous padding

        // Premium status line styling
        let status_bg = self.theme.bg_surface();

        // Draw subtle top border for separation
        let top_border_color = self.theme.border_subtle();

        let full_rect = ui.available_rect_before_wrap();
        let top_line_rect =
            egui::Rect::from_min_size(full_rect.min, egui::vec2(full_rect.width(), 1.0));
        ui.painter()
            .rect_filled(top_line_rect, 0.0, top_border_color);

        // Use a horizontal layout that spans the full width
        ui.horizontal(|ui| {
            ui.set_height(height);
            ui.spacing_mut().item_spacing.x = 0.0;

            // Fill background
            let bar_rect = ui.available_rect_before_wrap();
            ui.painter().rect_filled(bar_rect, 0.0, status_bg);

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
        let mode_text_color = self.mode.text_color(self.theme);

        self.render_segment(
            ui,
            self.mode.label(),
            Some(semantic_icons::mode::COMMAND),
            mode_color,
            mode_text_color,
            height,
            padding,
            true,
        );

        // Display preference badges (zen/fullscreen) - use distinct colors
        if self.is_zen_mode {
            let (bg, fg) = (self.theme.badge_zen_bg(), self.theme.badge_zen_fg());
            ui.add_space(4.0);
            self.render_segment(ui, "ZEN", None, bg, fg, height, padding, false);
        }

        if self.is_fullscreen {
            let (bg, fg) = (
                self.theme.badge_fullscreen_bg(),
                self.theme.badge_fullscreen_fg(),
            );
            ui.add_space(4.0);
            self.render_segment(ui, "FULLSCREEN", None, bg, fg, height, padding, false);
        }

        // Git branch / project info (if available)
        if let Some(ref branch) = self.branch_info {
            // Separator
            self.render_separator(ui, height);

            self.render_segment(
                ui,
                branch,
                Some(semantic_icons::git::BRANCH),
                self.segment_bg(),
                self.segment_fg(),
                height,
                padding,
                false,
            );
        }

        // Selected metric
        if let Some(ref metric) = self.selected_metric {
            // Separator
            self.render_separator(ui, height);

            // Truncate long metric names
            let display_name = if metric.len() > 30 {
                format!("{}...", &metric[..27])
            } else {
                metric.clone()
            };
            self.render_segment(
                ui,
                &display_name,
                Some(semantic_icons::action::CHART),
                self.segment_bg_secondary(),
                self.segment_fg(),
                height,
                padding,
                false,
            );
        }

        // Sparkline with current value
        if let Some(ref sparkline) = self.sparkline {
            // Separator
            self.render_separator(ui, height);

            self.render_sparkline_segment(ui, sparkline, height, padding);
        }
    }

    /// Render a subtle separator between segments (left-to-right)
    fn render_separator(&self, ui: &mut Ui, height: f32) {
        let separator_width = 20.0; // Slightly wider for breathing room
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(separator_width, height), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            // Premium: use a thin vertical line instead of chevron for cleaner look
            let line_color = self.segment_fg().gamma_multiply(0.15);
            let center_x = rect.center().x;
            ui.painter().vline(
                center_x,
                egui::Rangef::new(rect.min.y + 6.0, rect.max.y - 6.0),
                egui::Stroke::new(1.0, line_color),
            );
        }
    }

    /// Render the center section (expands to fill space)
    fn render_center_section(&self, ui: &mut Ui, _height: f32, _padding: f32) {
        // Fill remaining horizontal space, showing extra status if available
        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
            if let Some(ref extra) = self.extra_status {
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(extra)
                        .color(self.segment_fg())
                        .size(typography::MD)
                        .family(egui::FontFamily::Monospace),
                );
            }

            // Codebase indexing status (shown in center to avoid layout jumping from varying file names)
            if let Some(ref status) = self.codebase_status {
                // Show loading status (cloning, indexing tree-sitter)
                if status.is_loading && !status.is_error {
                    ui.add_space(16.0);

                    // Language icon (if available) or loading spinner
                    let icon = status
                        .language
                        .as_ref()
                        .and_then(|lang| semantic_icons::language::from_name(lang))
                        .unwrap_or(semantic_icons::status::LOADING);

                    ui.label(
                        egui::RichText::new(icon)
                            .color(palette::text::SECONDARY)
                            .size(typography::MD),
                    );
                    ui.add_space(4.0);
                    // Status text in secondary (gray) color - consistent with Tantivy indexing
                    ui.label(
                        egui::RichText::new(&status.message)
                            .color(palette::text::SECONDARY)
                            .size(typography::MD),
                    );
                }
                // Show Tantivy indexing status (background task after tree-sitter completes)
                else if status.is_tantivy_indexing && !status.is_error {
                    ui.add_space(16.0);

                    // Build progress message with details (no icon, just text)
                    let progress_msg = if let Some(phase) = &status.tantivy_phase {
                        if let Some((current, total)) = status.tantivy_progress {
                            format!("{phase} [{current}/{total}]")
                        } else {
                            phase.clone()
                        }
                    } else {
                        "Building search index...".to_string()
                    };

                    ui.label(
                        egui::RichText::new(progress_msg)
                            .color(palette::text::SECONDARY)
                            .size(typography::MD)
                            .family(egui::FontFamily::Monospace),
                    );
                }
            }

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
                Some(semantic_icons::nav::TABS),
                self.mode.color(self.theme),
                self.mode.text_color(self.theme),
                height,
                padding,
                true,
            );

            // Separator
            self.render_separator_rtl(ui, height);

            // Viewport info (e.g., pane layout)
            if let Some(ref viewport) = self.viewport_info {
                self.render_segment_rtl(
                    ui,
                    viewport,
                    Some(semantic_icons::nav::GRID),
                    self.segment_bg(),
                    self.segment_fg(),
                    height,
                    padding,
                    false,
                );

                // Separator
                self.render_separator_rtl(ui, height);
            }

            // Last refresh time
            if let Some(ref relative_time) = self.format_relative_time() {
                self.render_segment_rtl(
                    ui,
                    relative_time,
                    Some(semantic_icons::statusline::CLOCK),
                    self.segment_bg(),
                    self.segment_fg(),
                    height,
                    padding,
                    false,
                );

                // Separator
                self.render_separator_rtl(ui, height);
            }

            // Diagnostics indicator (errors/warnings/infos)
            let (errors, warnings, infos) = self.diagnostics_count;
            if errors > 0 || warnings > 0 || infos > 0 {
                // Build text with relevant counts
                let mut parts = Vec::new();
                if errors > 0 {
                    parts.push(format!("{} {}", semantic_icons::diagnostic::ERROR, errors));
                }
                if warnings > 0 {
                    parts.push(format!(
                        "{} {}",
                        semantic_icons::diagnostic::WARNING,
                        warnings
                    ));
                }
                if infos > 0 {
                    parts.push(format!("{} {}", semantic_icons::diagnostic::INFO, infos));
                }
                let diag_text = parts.join(" ");

                // Color based on severity (errors > warnings > infos)
                let diag_color = if errors > 0 {
                    palette::semantic::ERROR
                } else if warnings > 0 {
                    palette::semantic::WARNING
                } else {
                    palette::semantic::INFO
                };

                let response = self.render_segment_rtl_with_response(
                    ui,
                    &diag_text,
                    None, // Icons are embedded in text
                    self.segment_bg(),
                    diag_color,
                    height,
                    padding,
                    false,
                );
                if response.hovered() {
                    response.show_tooltip_text("Diagnostics available (Space+d to open)");
                }

                // Separator
                self.render_separator_rtl(ui, height);
            }

            // Codebase status (Cloning..., Ready, Error - but NOT Indexing which is in center)
            if let Some(ref status) = self.codebase_status {
                // Skip loading/indexing status here - it's shown in center section to avoid layout jumping
                if !status.is_loading && !status.is_tantivy_indexing {
                    if status.is_error {
                        // Error state
                        self.render_segment_rtl(
                            ui,
                            &status.message,
                            Some(semantic_icons::diagnostic::ERROR),
                            self.segment_bg(),
                            palette::semantic::ERROR,
                            height,
                            padding,
                            false,
                        );
                    } else if let Some(ref repo_name) = status.repo_name {
                        // Ready state - show repo name and metrics count with language icon
                        let metrics_text = status
                            .metrics_count
                            .map(|c| format!("{c} metrics"))
                            .unwrap_or_default();

                        // Get language icon if available
                        let icon = status
                            .language
                            .as_ref()
                            .and_then(|lang| semantic_icons::language::from_name(lang))
                            .unwrap_or(semantic_icons::file::CODE);

                        // Render metrics count segment first (RTL order)
                        if !metrics_text.is_empty() {
                            self.render_segment_rtl(
                                ui,
                                &metrics_text,
                                Some(semantic_icons::file::METRIC),
                                Color32::TRANSPARENT,
                                palette::text::SECONDARY,
                                height,
                                padding,
                                false,
                            );
                            self.render_separator_rtl(ui, height);
                        }

                        // Render repo name with language icon
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

                    // Separator
                    self.render_separator_rtl(ui, height);
                }
            }

            // Team collaboration status (only shown when connected to a team)
            if let Some(ref team_info) = self.team_status {
                if team_info.should_show() {
                    // Build team status text
                    let mut parts = Vec::new();

                    // Team name (truncated if too long)
                    if let Some(ref name) = team_info.team_name {
                        let display_name = if name.len() > 12 {
                            format!("{}...", &name[..9])
                        } else {
                            name.clone()
                        };
                        parts.push(display_name);
                    }

                    // Online count
                    parts.push(format!("{} online", team_info.online_count));

                    let status_text = parts.join(" | ");

                    // Icon and color based on unread notifications
                    let (icon, fg_color) = if team_info.unread_count > 0 {
                        (
                            semantic_icons::status::NOTIFICATION,
                            self.theme.accent_primary(),
                        )
                    } else {
                        (semantic_icons::social::TEAM, palette::text::SECONDARY)
                    };

                    // Add unread badge if any
                    let content = if team_info.unread_count > 0 {
                        format!("{status_text} ({})", team_info.unread_count)
                    } else {
                        status_text
                    };

                    let response = self.render_segment_rtl_with_response(
                        ui,
                        &content,
                        Some(icon),
                        Color32::TRANSPARENT,
                        fg_color,
                        height,
                        padding,
                        false,
                    );

                    if response.hovered() {
                        response.show_tooltip_text("Team collaboration (Space+t)");
                    }

                    // Separator
                    self.render_separator_rtl(ui, height);
                }
            }

            // Connection status - refined text colors matching theme
            let (conn_icon, conn_text, conn_color) = if self.is_connected {
                (
                    semantic_icons::status::CONNECTED,
                    "ONLINE",
                    palette::text::SECONDARY, // Muted - no need for bright color when connected
                )
            } else {
                (
                    semantic_icons::status::DISCONNECTED,
                    "OFFLINE",
                    palette::text::TERTIARY, // Muted gray - neutral, not alarming
                )
            };
            self.render_segment_rtl(
                ui,
                conn_text,
                Some(conn_icon),
                Color32::TRANSPARENT,
                conn_color,
                height,
                padding,
                false,
            );
        });
    }

    /// Render a subtle separator between segments (right-to-left)
    fn render_separator_rtl(&self, ui: &mut Ui, height: f32) {
        let separator_width = 20.0; // Slightly wider for breathing room
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(separator_width, height), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            // Premium: use a thin vertical line instead of chevron for cleaner look
            let line_color = self.segment_fg().gamma_multiply(0.15);
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
        let content = if let Some(icon) = icon {
            format!("{icon} {text}")
        } else {
            text.to_string()
        };

        // Calculate width needed for the text
        let galley = ui.painter().layout_no_wrap(
            content.clone(),
            typography::proportional(typography::MD),
            fg_color,
        );
        let text_width = galley.size().x + padding * 2.0;

        // Draw the segment background and text
        let (rect, _response) =
            ui.allocate_exact_size(egui::vec2(text_width, height), egui::Sense::hover());

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

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &content,
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
        let content = if let Some(icon) = icon {
            format!("{icon} {text}")
        } else {
            text.to_string()
        };

        // Calculate width needed for the text
        let galley = ui.painter().layout_no_wrap(
            content.clone(),
            typography::proportional(typography::MD),
            fg_color,
        );
        let text_width = galley.size().x + padding * 2.0;

        // Draw the segment background and text
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(text_width, height), egui::Sense::click());

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

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &content,
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
        let bg_color = self.segment_bg();
        let fg_color = self.segment_fg();

        // Sparkline color - use emerald accent for brand consistency
        let sparkline_color = self.theme.accent_hover(); // Bright accent for visibility

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
