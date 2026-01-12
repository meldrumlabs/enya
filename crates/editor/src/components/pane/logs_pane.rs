//! Logs pane component for displaying log entries.
//!
//! Displays log entries fetched from a [`LogsClient`], enabling metric→log correlation
//! where users can drill down from metric spikes to see the actual SQL queries
//! (or other logs) from that time period.

use std::any::Any;
use std::sync::Arc;

use egui::{Color32, RichText, ScrollArea, Vec2};
use enya_client::Promise;
use enya_client::logs::{
    DemoLogsClient, LogLevel, LogsClient, LogsQuery, LogsResponse, LogsResult, LokiClient,
};

use crate::components::util::id_generator::next_id_usize;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;

// ============================================================================
// Constants for consistent styling
// ============================================================================

const ROW_HEIGHT: f32 = 28.0;
const HEADER_HEIGHT: f32 = 36.0;
const PADDING: f32 = 12.0;
const CORNER_RADIUS: f32 = 6.0;
const SMALL_CORNER_RADIUS: f32 = 4.0;

/// Backend configuration for the logs pane.
#[derive(Clone)]
pub enum LogsBackend {
    /// Demo backend with synthetic SQL query logs.
    Demo,
    /// Loki backend with the given base URL (e.g., "http://localhost:3100").
    Loki(String),
}

impl Default for LogsBackend {
    fn default() -> Self {
        Self::Demo
    }
}

/// Action returned from LogsPane::show() for workspace to handle.
#[derive(Debug, Clone, PartialEq)]
pub enum LogsPaneAction {
    /// No action needed.
    None,
    /// Query was changed/saved - workspace may want to update state.
    QueryChanged,
}

/// Maximum number of queries to keep in history.
const MAX_HISTORY_SIZE: usize = 20;

/// A logs pane that displays log entries from a logs client.
///
/// This pane shows log entries in a scrollable table with:
/// - Editable LogQL query (press 'e' to edit via modal BufferEditor)
/// - Color-coded log levels (error=red, warn=yellow, info=blue, debug=gray)
/// - Timestamp formatting
/// - Text filter input for searching within logs
/// - Level filter dropdown (All, Error, Warn, Info, Debug)
/// - Query history for recalling previous queries
pub struct LogsPane {
    /// Unique identifier for this pane
    id: usize,
    /// Display name for the pane
    name: String,
    /// Current theme
    theme: AppTheme,

    // Data source - Arc for cheap cloning in async tasks
    logs_client: Arc<dyn LogsClient + Send + Sync>,
    /// The backend type (for display purposes)
    backend: LogsBackend,

    /// The saved LogQL query (edited via modal BufferEditor like QueryPane)
    saved_query: String,
    /// Whether edit was requested via button click (workspace opens BufferEditor)
    edit_requested: bool,

    // Query parameters
    start_ns: i64,
    end_ns: i64,

    // Filter state (for local filtering within results)
    filter_text: String,
    filter_level: Option<LogLevel>,

    // Results
    results: Option<LogsResponse>,
    is_loading: bool,
    error: Option<String>,

    /// Whether this pane needs a query refresh (set on save)
    needs_refresh: bool,

    // Promise for async fetch
    promise: Option<Promise<LogsResult>>,

    // UI state
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    level_dropdown_open: bool,
    /// Whether the query history dropdown is open
    history_dropdown_open: bool,
    /// Query history (most recent first)
    query_history: Vec<String>,
}

impl Default for LogsPane {
    fn default() -> Self {
        // Default to last hour
        let now_ns = now_unix_ns();
        let one_hour_ns = 3_600_000_000_000_i64;
        Self::new(now_ns - one_hour_ns, now_ns)
    }
}

impl LogsPane {
    /// Create a new logs pane for the given time range using the demo backend.
    ///
    /// # Arguments
    ///
    /// * `start_ns` - Start of time range in nanoseconds since Unix epoch
    /// * `end_ns` - End of time range in nanoseconds since Unix epoch
    #[must_use]
    pub fn new(start_ns: i64, end_ns: i64) -> Self {
        Self::with_backend(start_ns, end_ns, LogsBackend::Demo)
    }

    /// Create a logs pane with a specific backend.
    ///
    /// # Arguments
    ///
    /// * `start_ns` - Start of time range in nanoseconds since Unix epoch
    /// * `end_ns` - End of time range in nanoseconds since Unix epoch
    /// * `backend` - The logs backend to use (Demo or Loki)
    #[must_use]
    pub fn with_backend(start_ns: i64, end_ns: i64, backend: LogsBackend) -> Self {
        let id = next_id_usize();

        let logs_client: Arc<dyn LogsClient + Send + Sync> = match &backend {
            LogsBackend::Demo => Arc::new(DemoLogsClient::new()),
            LogsBackend::Loki(url) => Arc::new(LokiClient::new(url.clone())),
        };

        let (name, default_query) = match &backend {
            LogsBackend::Demo => (format!("Logs {id} (demo)"), String::new()),
            LogsBackend::Loki(url) => {
                // Extract host from URL for display
                let host = url
                    .trim_start_matches("http://")
                    .trim_start_matches("https://")
                    .split('/')
                    .next()
                    .unwrap_or(url);
                // Default LogQL query - users can customize
                (
                    format!("Logs {id} ({host})"),
                    "{job=\"varlogs\"}".to_string(),
                )
            }
        };

        Self {
            id,
            name,
            theme: AppTheme::default(),
            logs_client,
            backend,
            saved_query: default_query,
            edit_requested: false,
            start_ns,
            end_ns,
            filter_text: String::new(),
            filter_level: None,
            results: None,
            is_loading: false,
            error: None,
            needs_refresh: false,
            promise: None,
            selected_index: None,
            hovered_index: None,
            level_dropdown_open: false,
            history_dropdown_open: false,
            query_history: Vec::new(),
        }
    }

    /// Create a logs pane connected to a Loki server.
    ///
    /// # Arguments
    ///
    /// * `start_ns` - Start of time range in nanoseconds since Unix epoch
    /// * `end_ns` - End of time range in nanoseconds since Unix epoch
    /// * `loki_url` - The Loki server URL (e.g., "http://localhost:3100")
    #[must_use]
    pub fn with_loki(start_ns: i64, end_ns: i64, loki_url: impl Into<String>) -> Self {
        Self::with_backend(start_ns, end_ns, LogsBackend::Loki(loki_url.into()))
    }

    /// Create a logs pane with a custom name.
    #[must_use]
    pub fn with_name(start_ns: i64, end_ns: i64, name: impl Into<String>) -> Self {
        let mut pane = Self::new(start_ns, end_ns);
        pane.name = name.into();
        pane
    }

    /// Get the backend type.
    pub fn backend(&self) -> &LogsBackend {
        &self.backend
    }

    /// Set the time range and trigger a refresh.
    pub fn set_time_range(&mut self, start_ns: i64, end_ns: i64) {
        self.start_ns = start_ns;
        self.end_ns = end_ns;
        self.results = None;
        self.promise = None;
    }

    /// Set the text filter.
    pub fn set_filter_text(&mut self, text: impl Into<String>) {
        self.filter_text = text.into();
        self.results = None;
        self.promise = None;
    }

    /// Set the level filter.
    pub fn set_filter_level(&mut self, level: Option<LogLevel>) {
        self.filter_level = level;
    }

    /// Check if this pane is currently loading.
    pub fn is_loading(&self) -> bool {
        self.is_loading
    }

    /// Get the saved LogQL query.
    pub fn saved_query(&self) -> &str {
        &self.saved_query
    }

    /// Set the LogQL query and trigger a refresh.
    ///
    /// The query is added to history (if non-empty and different from the last entry).
    pub fn set_query(&mut self, query: impl Into<String>) {
        let query = query.into();

        // Add to history if non-empty and different from the most recent entry
        if !query.is_empty() && self.query_history.first() != Some(&query) {
            // Remove any existing occurrence to avoid duplicates
            self.query_history.retain(|q| q != &query);
            // Add to front (most recent)
            self.query_history.insert(0, query.clone());
            // Trim to max size
            if self.query_history.len() > MAX_HISTORY_SIZE {
                self.query_history.truncate(MAX_HISTORY_SIZE);
            }
        }

        self.saved_query = query;
        self.needs_refresh = true;
        self.results = None;
        self.promise = None;
    }

    /// Get the query history (most recent first).
    pub fn query_history(&self) -> &[String] {
        &self.query_history
    }

    /// Clear the query history.
    pub fn clear_history(&mut self) {
        self.query_history.clear();
    }

    /// Check if edit was requested via button click.
    pub fn edit_requested(&self) -> bool {
        self.edit_requested
    }

    /// Clear the edit requested flag (called after workspace handles it).
    pub fn clear_edit_requested(&mut self) {
        self.edit_requested = false;
    }

    /// Poll the promise for results.
    fn poll_results(&mut self, ctx: &egui::Context) {
        // If we have a pending promise, check if it's ready
        if let Some(promise) = self.promise.take() {
            match promise.try_take() {
                Ok(result) => {
                    // Promise completed
                    self.is_loading = false;
                    match result {
                        Ok(response) => {
                            self.results = Some(response);
                            self.error = None;
                        }
                        Err(e) => {
                            self.error = Some(e.to_string());
                            self.results = None;
                        }
                    }
                }
                Err(promise) => {
                    // Not ready yet, put it back
                    self.promise = Some(promise);
                }
            }
        }

        // If no results and no pending promise, start a new query
        // Also start if needs_refresh is set (query was saved)
        if (self.results.is_none() && self.promise.is_none() && self.error.is_none())
            || (self.needs_refresh && self.promise.is_none())
        {
            self.start_query(ctx);
        }
    }

    /// Start a new logs query.
    fn start_query(&mut self, ctx: &egui::Context) {
        let mut query = LogsQuery::new(self.start_ns, self.end_ns);

        // Use the saved LogQL query
        if !self.saved_query.is_empty() {
            query = query.with_query(&self.saved_query);
        }

        // Apply text filter for local filtering
        if !self.filter_text.is_empty() {
            query = query.with_contains(&self.filter_text);
        }

        self.is_loading = true;
        self.needs_refresh = false;
        self.promise = Some(self.logs_client.query_logs(query, ctx));
    }

    /// Refresh the logs query.
    pub fn refresh(&mut self) {
        self.results = None;
        self.error = None;
        self.promise = None;
    }

    /// Render the pane header with filters.
    fn render_header(&mut self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);
        let muted_text = text_col.gamma_multiply(0.6);
        let accent = self.theme.accent_primary();

        // Header frame with premium styling - subtle accent tint for dark themes
        let header_bg = if self.theme.is_dark() {
            // Blend surface with a hint of accent for premium feel
            let base = self.theme.bg_surface();
            Color32::from_rgb(
                base.r().saturating_add((accent.r() as u16 * 3 / 100) as u8),
                base.g().saturating_add((accent.g() as u16 * 3 / 100) as u8),
                base.b().saturating_add((accent.b() as u16 * 3 / 100) as u8),
            )
        } else {
            self.theme.bg_surface()
        };

        let header_response = egui::Frame::new()
            .fill(header_bg)
            .inner_margin(egui::Margin::symmetric(PADDING as i8, 8))
            .show(ui, |ui| {
                ui.set_height(HEADER_HEIGHT);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;

                    // Pane icon with subtle glow effect
                    ui.label(
                        RichText::new(semantic_icons::file::TEXT)
                            .color(accent)
                            .size(16.0),
                    );

                    ui.label(RichText::new("Logs").color(text_col).size(13.0).strong());

                    ui.add_space(4.0);

                    // Level filter dropdown button
                    self.render_level_dropdown(ui, text_col, muted_text);

                    ui.add_space(4.0);

                    // Search input with icon
                    self.render_search_input(ui, text_col);

                    // Right-aligned items
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;

                        // Edit button (rightmost in RTL layout, so first)
                        let edit_response = ui.add(
                            egui::Button::new(
                                RichText::new(semantic_icons::action::EDIT)
                                    .color(muted_text)
                                    .size(14.0),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::splat(28.0)),
                        );

                        if edit_response
                            .on_hover_text("Edit query (e)")
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.edit_requested = true;
                        }

                        // Refresh button
                        let refresh_response = ui.add(
                            egui::Button::new(
                                RichText::new(semantic_icons::action::REFRESH)
                                    .color(muted_text)
                                    .size(14.0),
                            )
                            .fill(Color32::TRANSPARENT)
                            .min_size(Vec2::splat(28.0)),
                        );

                        if refresh_response
                            .on_hover_text("Refresh logs (r)")
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.refresh();
                        }

                        // History button (only show if we have history)
                        if !self.query_history.is_empty() {
                            self.render_history_dropdown(ui, text_col, muted_text);
                        }

                        // Row count badge
                        if let Some(ref response) = self.results {
                            let count = self.filtered_entries_count(response);
                            let total = response.entries.len();
                            let count_text = if count == total {
                                format!("{total} entries")
                            } else {
                                format!("{count} of {total}")
                            };

                            egui::Frame::new()
                                .fill(self.theme.bg_elevated())
                                .corner_radius(SMALL_CORNER_RADIUS)
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(count_text).color(muted_text).size(11.0),
                                    );
                                });
                        }
                    });
                });
            });

        // Premium accent line under header - subtle gradient effect
        let header_rect = header_response.response.rect;
        let painter = ui.painter();
        let accent_line_rect = egui::Rect::from_min_size(
            egui::pos2(header_rect.left(), header_rect.bottom()),
            egui::vec2(header_rect.width(), 1.0),
        );

        // Create a subtle gradient effect using the accent color
        let accent_alpha = if self.theme.is_dark() { 0.4 } else { 0.25 };
        painter.rect_filled(accent_line_rect, 0.0, accent.gamma_multiply(accent_alpha));
    }

    /// Render the level filter dropdown.
    fn render_level_dropdown(&mut self, ui: &mut egui::Ui, text_col: Color32, muted_text: Color32) {
        let level_label = self.filter_level.map(level_to_label).unwrap_or("All");
        let dropdown_level_color = self
            .filter_level
            .map(|l| level_color(l, self.theme))
            .unwrap_or(muted_text);

        let dropdown_response = ui.add(
            egui::Button::new(
                RichText::new(format!(
                    "{} {} {}",
                    self.filter_level
                        .map(level_icon)
                        .unwrap_or(semantic_icons::mode::VISUAL_LINE),
                    level_label,
                    semantic_icons::nav::EXPAND
                ))
                .color(dropdown_level_color)
                .size(12.0),
            )
            .fill(self.theme.bg_elevated())
            .corner_radius(SMALL_CORNER_RADIUS),
        );

        // Save rect before consuming response
        let dropdown_rect = dropdown_response.rect;
        let dropdown_hovered = dropdown_response.contains_pointer();

        if dropdown_response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.level_dropdown_open = !self.level_dropdown_open;
        }

        // Show dropdown menu
        if self.level_dropdown_open {
            let popup_id = egui::Id::new(format!("logs_level_popup_{}", self.id));
            let accent = self.theme.accent_primary();

            // Premium popup styling with theme-aware colors
            let popup_border = if self.theme.is_dark() {
                // Subtle accent-tinted border for dark themes
                let base = self.theme.border_subtle();
                Color32::from_rgb(
                    base.r()
                        .saturating_add((accent.r() as u16 * 10 / 100) as u8),
                    base.g()
                        .saturating_add((accent.g() as u16 * 10 / 100) as u8),
                    base.b()
                        .saturating_add((accent.b() as u16 * 10 / 100) as u8),
                )
            } else {
                self.theme.border_default()
            };

            let shadow_alpha = if self.theme.is_dark() { 60 } else { 30 };

            let area_response = egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(dropdown_rect.left_bottom() + egui::vec2(0.0, 4.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(self.theme.bg_elevated())
                        .stroke(egui::Stroke::new(1.0, popup_border))
                        .corner_radius(CORNER_RADIUS)
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 16,
                            spread: 0,
                            color: Color32::from_black_alpha(shadow_alpha),
                        })
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.set_min_width(120.0);
                            ui.spacing_mut().item_spacing.y = 2.0;

                            // All levels option
                            let all_selected = self.filter_level.is_none();
                            let all_response = ui.add(
                                egui::Button::new(
                                    RichText::new(format!(
                                        "{}  All Levels",
                                        semantic_icons::mode::VISUAL_LINE
                                    ))
                                    .color(if all_selected {
                                        self.theme.accent_primary()
                                    } else {
                                        text_col
                                    })
                                    .size(12.0),
                                )
                                .fill(if all_selected {
                                    self.theme.accent_primary().gamma_multiply(0.15)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(SMALL_CORNER_RADIUS)
                                .min_size(egui::vec2(ui.available_width(), 28.0)),
                            );

                            if all_response.clicked() {
                                self.filter_level = None;
                                self.level_dropdown_open = false;
                            }

                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);

                            // Individual level options
                            for level in [
                                LogLevel::Error,
                                LogLevel::Warn,
                                LogLevel::Info,
                                LogLevel::Debug,
                                LogLevel::Trace,
                            ] {
                                let is_selected = self.filter_level == Some(level);
                                let color = level_color(level, self.theme);

                                let level_response = ui.add(
                                    egui::Button::new(
                                        RichText::new(format!(
                                            "{}  {}",
                                            level_icon(level),
                                            level_to_label(level)
                                        ))
                                        .color(color)
                                        .size(12.0),
                                    )
                                    .fill(if is_selected {
                                        color.gamma_multiply(0.15)
                                    } else {
                                        Color32::TRANSPARENT
                                    })
                                    .corner_radius(SMALL_CORNER_RADIUS)
                                    .min_size(egui::vec2(ui.available_width(), 28.0)),
                                );

                                if level_response.clicked() {
                                    self.filter_level = Some(level);
                                    self.level_dropdown_open = false;
                                }
                            }
                        });
                });

            // Close if clicked outside
            if ui.input(|i| i.pointer.any_click())
                && !area_response.response.contains_pointer()
                && !dropdown_hovered
            {
                self.level_dropdown_open = false;
            }
        }
    }

    /// Render the query history dropdown.
    fn render_history_dropdown(
        &mut self,
        ui: &mut egui::Ui,
        text_col: Color32,
        muted_text: Color32,
    ) {
        let history_count = self.query_history.len();

        let dropdown_response = ui.add(
            egui::Button::new(
                RichText::new(format!(
                    "{} {} {}",
                    semantic_icons::action::HISTORY,
                    history_count,
                    semantic_icons::nav::EXPAND
                ))
                .color(muted_text)
                .size(12.0),
            )
            .fill(self.theme.bg_elevated())
            .corner_radius(SMALL_CORNER_RADIUS),
        );

        // Save rect before consuming response
        let dropdown_rect = dropdown_response.rect;
        let dropdown_hovered = dropdown_response.contains_pointer();

        if dropdown_response
            .on_hover_text("Query history")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.history_dropdown_open = !self.history_dropdown_open;
        }

        // Show dropdown menu
        if self.history_dropdown_open {
            let popup_id = egui::Id::new(format!("logs_history_popup_{}", self.id));
            let accent = self.theme.accent_primary();

            // Premium popup styling with theme-aware colors
            let popup_border = if self.theme.is_dark() {
                let base = self.theme.border_subtle();
                Color32::from_rgb(
                    base.r()
                        .saturating_add((accent.r() as u16 * 10 / 100) as u8),
                    base.g()
                        .saturating_add((accent.g() as u16 * 10 / 100) as u8),
                    base.b()
                        .saturating_add((accent.b() as u16 * 10 / 100) as u8),
                )
            } else {
                self.theme.border_default()
            };

            let shadow_alpha = if self.theme.is_dark() { 60 } else { 30 };

            // Clone history for use in closure
            let history_clone = self.query_history.clone();
            let mut selected_query: Option<String> = None;

            let area_response = egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(dropdown_rect.left_bottom() + egui::vec2(0.0, 4.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(self.theme.bg_elevated())
                        .stroke(egui::Stroke::new(1.0, popup_border))
                        .corner_radius(CORNER_RADIUS)
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 16,
                            spread: 0,
                            color: Color32::from_black_alpha(shadow_alpha),
                        })
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.set_min_width(300.0);
                            ui.set_max_width(400.0);
                            ui.spacing_mut().item_spacing.y = 2.0;

                            // Header
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Recent Queries")
                                        .color(muted_text)
                                        .size(10.0)
                                        .strong(),
                                );
                            });

                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);

                            // History entries (scrollable if many)
                            egui::ScrollArea::vertical()
                                .max_height(200.0)
                                .show(ui, |ui| {
                                    for (idx, query) in history_clone.iter().enumerate() {
                                        let is_current = query == &self.saved_query;

                                        // Truncate long queries for display
                                        let display_query = if query.len() > 50 {
                                            format!("{}…", &query[..47])
                                        } else {
                                            query.clone()
                                        };

                                        let item_response = ui.add(
                                            egui::Button::new(
                                                RichText::new(&display_query)
                                                    .color(if is_current {
                                                        self.theme.accent_primary()
                                                    } else {
                                                        text_col
                                                    })
                                                    .size(11.0)
                                                    .monospace(),
                                            )
                                            .fill(if is_current {
                                                self.theme.accent_primary().gamma_multiply(0.15)
                                            } else {
                                                Color32::TRANSPARENT
                                            })
                                            .corner_radius(SMALL_CORNER_RADIUS)
                                            .min_size(egui::vec2(ui.available_width(), 28.0)),
                                        );

                                        // Show full query on hover (truncated for display)
                                        let hover_text = if idx < 9 {
                                            format!("{}\n(Press {} to select)", query, idx + 1)
                                        } else {
                                            query.clone()
                                        };

                                        if item_response.on_hover_text(hover_text).clicked() {
                                            selected_query = Some(query.clone());
                                        }
                                    }
                                });
                        });
                });

            // Apply selection if made
            if let Some(query) = selected_query {
                self.saved_query = query;
                self.needs_refresh = true;
                self.results = None;
                self.promise = None;
                self.history_dropdown_open = false;
            }

            // Close if clicked outside
            if ui.input(|i| i.pointer.any_click())
                && !area_response.response.contains_pointer()
                && !dropdown_hovered
            {
                self.history_dropdown_open = false;
            }
        }
    }

    /// Render the search input field.
    fn render_search_input(&mut self, ui: &mut egui::Ui, text_col: Color32) {
        egui::Frame::new()
            .fill(self.theme.bg_elevated())
            .corner_radius(SMALL_CORNER_RADIUS)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    ui.label(
                        RichText::new(semantic_icons::status::SEARCH)
                            .color(text_col.gamma_multiply(0.5))
                            .size(12.0),
                    );

                    let filter_response = ui.add(
                        egui::TextEdit::singleline(&mut self.filter_text)
                            .hint_text("Filter logs...")
                            .desired_width(140.0)
                            .frame(false)
                            .font(egui::TextStyle::Small),
                    );

                    if filter_response.changed() {
                        // Trigger refresh on filter change
                        self.results = None;
                        self.promise = None;
                    }
                });
            });
    }

    /// Count entries that match the current level filter.
    fn filtered_entries_count(&self, response: &LogsResponse) -> usize {
        if self.filter_level.is_none() {
            return response.entries.len();
        }

        response
            .entries
            .iter()
            .filter(|e| self.filter_level.is_none() || e.level == self.filter_level)
            .count()
    }

    /// Render the main content area with log entries.
    fn render_content(&mut self, ui: &mut egui::Ui) {
        if self.is_loading {
            self.render_loading_skeleton(ui);
            return;
        }

        if let Some(ref error) = self.error.clone() {
            self.render_error(ui, error);
            return;
        }

        if let Some(response) = self.results.clone() {
            if response.entries.is_empty() {
                self.render_empty_state(ui);
            } else {
                self.render_logs_table(ui, &response);
            }
        } else {
            self.render_empty_state(ui);
        }
    }

    /// Render loading skeleton with shimmer effect (matching QueryPane style).
    fn render_loading_skeleton(&self, ui: &mut egui::Ui) {
        let time = ui.ctx().input(|i| i.time);
        let available = ui.available_size();

        // Premium theme-aware skeleton colors - blend base with subtle accent
        let base = self.theme.bg_elevated();
        let accent = self.theme.accent_primary();

        // Create skeleton base color with subtle accent tint
        let skeleton_base = if self.theme.is_dark() {
            Color32::from_rgb(
                base.r().saturating_add((accent.r() as u16 * 3 / 100) as u8),
                base.g().saturating_add((accent.g() as u16 * 3 / 100) as u8),
                base.b().saturating_add((accent.b() as u16 * 3 / 100) as u8),
            )
        } else {
            // Light themes: slightly darker base
            Color32::from_rgb(
                base.r().saturating_sub(8),
                base.g().saturating_sub(8),
                base.b().saturating_sub(6),
            )
        };

        // Shimmer color intensity varies by theme
        let shimmer_color = if self.theme.is_dark() {
            accent.gamma_multiply(0.35)
        } else {
            accent.gamma_multiply(0.20)
        };

        // Calculate shimmer position (sweeps left to right)
        let shimmer_progress = ((time * 0.8) % 2.0) as f32;
        let shimmer_width = available.x * 0.4;
        let shimmer_x = (shimmer_progress - 0.5) * (available.x + shimmer_width);

        // Allocate the full area
        let (full_rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
        let painter = ui.painter();

        // Table header skeleton
        let header_rect = egui::Rect::from_min_size(
            egui::pos2(full_rect.left() + PADDING, full_rect.top() + PADDING),
            egui::vec2(available.x - PADDING * 2.0, 20.0),
        );

        // Time column header
        painter.rect_filled(
            egui::Rect::from_min_size(header_rect.min, egui::vec2(60.0, 12.0)),
            3.0,
            skeleton_base.gamma_multiply(0.7),
        );

        // Level column header
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(header_rect.left() + 100.0, header_rect.top()),
                egui::vec2(40.0, 12.0),
            ),
            3.0,
            skeleton_base.gamma_multiply(0.7),
        );

        // Message column header
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(header_rect.left() + 180.0, header_rect.top()),
                egui::vec2(60.0, 12.0),
            ),
            3.0,
            skeleton_base.gamma_multiply(0.7),
        );

        // Row skeletons
        let num_rows = ((available.y - 60.0) / ROW_HEIGHT) as usize;
        for i in 0..num_rows.min(15) {
            let y = full_rect.top() + 48.0 + i as f32 * ROW_HEIGHT;

            // Alternating row background
            if i % 2 == 1 {
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(full_rect.left(), y),
                        egui::vec2(available.x, ROW_HEIGHT),
                    ),
                    0.0,
                    skeleton_base.gamma_multiply(0.3),
                );
            }

            // Timestamp skeleton
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(full_rect.left() + PADDING, y + 8.0),
                    egui::vec2(75.0, 12.0),
                ),
                3.0,
                skeleton_base,
            );

            // Level badge skeleton
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(full_rect.left() + PADDING + 100.0, y + 6.0),
                    egui::vec2(45.0, 16.0),
                ),
                SMALL_CORNER_RADIUS,
                skeleton_base,
            );

            // Message skeleton (varying widths)
            let msg_width = 150.0 + ((i * 47) % 250) as f32;
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(full_rect.left() + PADDING + 180.0, y + 8.0),
                    egui::vec2(msg_width.min(available.x - PADDING * 2.0 - 200.0), 12.0),
                ),
                3.0,
                skeleton_base,
            );
        }

        // Shimmer overlay
        let shimmer_rect = egui::Rect::from_min_size(
            egui::pos2(full_rect.left() + shimmer_x, full_rect.top()),
            egui::vec2(shimmer_width, available.y),
        );

        let clipped = shimmer_rect.intersect(full_rect);
        if clipped.width() > 0.0 {
            let segments = 10;
            let segment_width = clipped.width() / segments as f32;
            for i in 0..segments {
                let alpha = {
                    let t = i as f32 / segments as f32;
                    (-(t - 0.5).powi(2) * 8.0).exp()
                };
                let seg_rect = egui::Rect::from_min_size(
                    egui::pos2(clipped.left() + i as f32 * segment_width, clipped.top()),
                    egui::vec2(segment_width, clipped.height()),
                );
                painter.rect_filled(seg_rect, 0.0, shimmer_color.gamma_multiply(alpha));
            }
        }

        ui.ctx().request_repaint();
    }

    /// Render empty state.
    fn render_empty_state(&self, ui: &mut egui::Ui) {
        let text_col = text_color(self.theme);

        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);

                // Empty state icon
                ui.label(
                    RichText::new(semantic_icons::file::TEXT)
                        .color(text_col.gamma_multiply(0.3))
                        .size(48.0),
                );

                ui.add_space(16.0);

                ui.label(
                    RichText::new("No log entries")
                        .color(text_col.gamma_multiply(0.6))
                        .size(14.0),
                );

                ui.add_space(4.0);

                ui.label(
                    RichText::new("Adjust your time range or filters")
                        .color(text_col.gamma_multiply(0.4))
                        .size(12.0),
                );
            });
        });
    }

    /// Render error state.
    fn render_error(&self, ui: &mut egui::Ui, error: &str) {
        let text_col = text_color(self.theme);
        let error_color = self.theme.semantic_error();

        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);

                // Error icon with glow effect
                egui::Frame::new()
                    .fill(error_color.gamma_multiply(0.1))
                    .corner_radius(24.0)
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(semantic_icons::status::ERROR)
                                .color(error_color)
                                .size(32.0),
                        );
                    });

                ui.add_space(16.0);

                ui.label(
                    RichText::new("Failed to load logs")
                        .color(text_col)
                        .size(14.0)
                        .strong(),
                );

                ui.add_space(8.0);

                // Error message in a subtle frame
                egui::Frame::new()
                    .fill(self.theme.bg_elevated())
                    .corner_radius(SMALL_CORNER_RADIUS)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(error)
                                .color(text_col.gamma_multiply(0.7))
                                .size(11.0)
                                .monospace(),
                        );
                    });

                ui.add_space(16.0);

                // Retry button
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!("{} Retry", semantic_icons::action::REFRESH))
                                .size(12.0),
                        )
                        .corner_radius(SMALL_CORNER_RADIUS),
                    )
                    .clicked()
                {
                    // Note: Can't call refresh here as we have &self, handled via action pattern
                }
            });
        });
    }

    /// Render the logs table with premium styling.
    fn render_logs_table(&mut self, ui: &mut egui::Ui, response: &LogsResponse) {
        let text_col = text_color(self.theme);
        let accent = self.theme.accent_primary();

        // Premium header text color - use accent-tinted muted text
        let header_text = if self.theme.is_dark() {
            // Blend muted text with accent for premium feel
            let muted = text_col.gamma_multiply(0.5);
            Color32::from_rgb(
                muted
                    .r()
                    .saturating_add((accent.r() as u16 * 8 / 100) as u8),
                muted
                    .g()
                    .saturating_add((accent.g() as u16 * 8 / 100) as u8),
                muted
                    .b()
                    .saturating_add((accent.b() as u16 * 8 / 100) as u8),
            )
        } else {
            text_col.gamma_multiply(0.5)
        };

        let separator_color = self.theme.border_subtle();

        // Column layout - these are OFFSETS from the left edge of the content area
        // TIME column: 0 to 95px
        // LEVEL column: 95 to 160px (separator at 95, content starts at 107)
        // MESSAGE column: 160px onwards (separator at 160, content starts at 172)
        const TIME_COL_END: f32 = 95.0;
        const LEVEL_COL_END: f32 = 160.0;
        const COL_PADDING: f32 = 12.0;

        // Premium table header background - subtle accent tint
        let table_header_bg = if self.theme.is_dark() {
            let base = self.theme.bg_surface();
            Color32::from_rgb(
                base.r().saturating_add((accent.r() as u16 * 2 / 100) as u8),
                base.g().saturating_add((accent.g() as u16 * 2 / 100) as u8),
                base.b().saturating_add((accent.b() as u16 * 2 / 100) as u8),
            )
        } else {
            self.theme.bg_surface()
        };

        // Table header with bottom border
        let header_response = egui::Frame::new()
            .fill(table_header_bg)
            .inner_margin(egui::Margin::symmetric(PADDING as i8, 8))
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 18.0),
                    egui::Sense::hover(),
                );

                let painter = ui.painter();
                let base_x = rect.left();

                // TIME header
                painter.text(
                    egui::pos2(base_x, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "TIME",
                    egui::FontId::proportional(10.0),
                    header_text,
                );

                // Subtle vertical separator after TIME
                let sep1_x = base_x + TIME_COL_END;
                painter.line_segment(
                    [
                        egui::pos2(sep1_x, rect.top() + 2.0),
                        egui::pos2(sep1_x, rect.bottom() - 2.0),
                    ],
                    egui::Stroke::new(1.0, separator_color),
                );

                // LEVEL header - centered in column
                let level_col_center = sep1_x + (LEVEL_COL_END - TIME_COL_END) / 2.0;
                painter.text(
                    egui::pos2(level_col_center, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    "LEVEL",
                    egui::FontId::proportional(10.0),
                    header_text,
                );

                // Subtle vertical separator after LEVEL
                let sep2_x = base_x + LEVEL_COL_END;
                painter.line_segment(
                    [
                        egui::pos2(sep2_x, rect.top() + 2.0),
                        egui::pos2(sep2_x, rect.bottom() - 2.0),
                    ],
                    egui::Stroke::new(1.0, separator_color),
                );

                // MESSAGE header
                painter.text(
                    egui::pos2(sep2_x + COL_PADDING, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "MESSAGE",
                    egui::FontId::proportional(10.0),
                    header_text,
                );

                rect
            });

        // Premium accent line under table header
        let header_outer_rect = header_response.response.rect;
        let border_rect = egui::Rect::from_min_size(
            egui::pos2(header_outer_rect.left(), header_outer_rect.bottom()),
            egui::vec2(header_outer_rect.width(), 1.0),
        );
        let accent_border = if self.theme.is_dark() {
            accent.gamma_multiply(0.25)
        } else {
            separator_color
        };
        ui.painter().rect_filled(border_rect, 0.0, accent_border);

        // Reset hover state before rendering rows
        self.hovered_index = None;

        // Scrollable log entries
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                for (idx, entry) in response.entries.iter().enumerate() {
                    // Apply level filter
                    if let Some(filter_level) = self.filter_level {
                        if entry.level != Some(filter_level) {
                            continue;
                        }
                    }

                    let is_selected = self.selected_index == Some(idx);
                    let is_hovered = self.hovered_index == Some(idx);

                    // Premium row background with theme-aware styling
                    let bg_color = if is_selected {
                        // Selected: stronger accent background
                        if self.theme.is_dark() {
                            accent.gamma_multiply(0.18)
                        } else {
                            accent.gamma_multiply(0.12)
                        }
                    } else if is_hovered {
                        // Hover: subtle accent tint
                        if self.theme.is_dark() {
                            let hover = self.theme.bg_hover();
                            Color32::from_rgb(
                                hover
                                    .r()
                                    .saturating_add((accent.r() as u16 * 4 / 100) as u8),
                                hover
                                    .g()
                                    .saturating_add((accent.g() as u16 * 4 / 100) as u8),
                                hover
                                    .b()
                                    .saturating_add((accent.b() as u16 * 4 / 100) as u8),
                            )
                        } else {
                            self.theme.bg_hover()
                        }
                    } else if idx % 2 == 1 {
                        // Alternating rows: subtle zebra stripe
                        self.theme.bg_elevated().gamma_multiply(0.4)
                    } else {
                        Color32::TRANSPARENT
                    };

                    let (rect, row_response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), ROW_HEIGHT),
                        egui::Sense::click(),
                    );

                    // Update hover state
                    if row_response.hovered() {
                        self.hovered_index = Some(idx);
                    }

                    if row_response.clicked() {
                        self.selected_index = Some(idx);
                    }

                    // Draw row background with subtle left border for selected
                    let painter = ui.painter();
                    painter.rect_filled(rect, 0.0, bg_color);

                    if is_selected {
                        // Premium accent left border for selected row
                        let border_rect = egui::Rect::from_min_size(
                            rect.left_top(),
                            egui::vec2(3.0, rect.height()),
                        );
                        painter.rect_filled(border_rect, 0.0, accent);

                        // Optional: subtle glow effect for selected row (dark themes only)
                        if self.theme.is_dark() {
                            let glow_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.left() + 3.0, rect.top()),
                                egui::vec2(8.0, rect.height()),
                            );
                            painter.rect_filled(glow_rect, 0.0, accent.gamma_multiply(0.08));
                        }
                    }

                    // Draw row content - column layout must match header exactly
                    // These constants MUST match the header constants
                    const TIME_COL_END: f32 = 95.0;
                    const LEVEL_COL_END: f32 = 160.0;
                    const COL_PADDING: f32 = 12.0;

                    let base_x = rect.left() + PADDING;
                    let center_y = rect.center().y;
                    let separator_color = self.theme.border_subtle().gamma_multiply(0.5);

                    // Timestamp column (left-aligned)
                    let timestamp_str = format_timestamp_ns(entry.timestamp_ns);
                    let ts_galley = painter.layout_no_wrap(
                        timestamp_str,
                        egui::FontId::monospace(11.0),
                        text_col.gamma_multiply(0.7),
                    );
                    painter.galley(
                        egui::pos2(base_x, center_y - ts_galley.size().y / 2.0),
                        ts_galley,
                        text_col,
                    );

                    // Vertical separator after timestamp
                    let sep1_x = base_x + TIME_COL_END;
                    painter.line_segment(
                        [
                            egui::pos2(sep1_x, rect.top() + 4.0),
                            egui::pos2(sep1_x, rect.bottom() - 4.0),
                        ],
                        egui::Stroke::new(1.0, separator_color),
                    );

                    // Level badge column (centered in column) - premium theme-aware styling
                    if let Some(level) = entry.level {
                        let color = level_color(level, self.theme);
                        let level_text = level_to_short(level);

                        let badge_galley = painter.layout_no_wrap(
                            level_text.to_string(),
                            egui::FontId::monospace(9.0),
                            color,
                        );

                        // Center the badge in the LEVEL column
                        let level_col_center = sep1_x + (LEVEL_COL_END - TIME_COL_END) / 2.0;
                        let badge_width = badge_galley.size().x + 10.0;
                        let badge_height = badge_galley.size().y + 6.0;

                        let badge_rect = egui::Rect::from_center_size(
                            egui::pos2(level_col_center, center_y),
                            egui::vec2(badge_width, badge_height),
                        );

                        // Premium badge styling - different for light/dark themes
                        let badge_bg = if self.theme.is_dark() {
                            color.gamma_multiply(0.15)
                        } else {
                            color.gamma_multiply(0.10)
                        };

                        // Badge background with subtle border for light themes
                        painter.rect_filled(badge_rect, SMALL_CORNER_RADIUS, badge_bg);

                        // Add subtle border for light themes for better definition
                        if self.theme.is_light() {
                            painter.rect_stroke(
                                badge_rect,
                                SMALL_CORNER_RADIUS,
                                egui::Stroke::new(0.5, color.gamma_multiply(0.25)),
                                egui::StrokeKind::Inside,
                            );
                        }

                        // Badge text (centered)
                        painter.galley(
                            egui::pos2(
                                badge_rect.center().x - badge_galley.size().x / 2.0,
                                center_y - badge_galley.size().y / 2.0,
                            ),
                            badge_galley,
                            color,
                        );
                    }

                    // Vertical separator after level
                    let sep2_x = base_x + LEVEL_COL_END;
                    painter.line_segment(
                        [
                            egui::pos2(sep2_x, rect.top() + 4.0),
                            egui::pos2(sep2_x, rect.bottom() - 4.0),
                        ],
                        egui::Stroke::new(1.0, separator_color),
                    );

                    // Message column (left-aligned)
                    let msg_x = sep2_x + COL_PADDING;
                    let available_width = rect.right() - msg_x - PADDING;
                    let message = truncate_message(&entry.message, available_width, 11.0);

                    let msg_galley =
                        painter.layout_no_wrap(message, egui::FontId::monospace(11.0), text_col);
                    painter.galley(
                        egui::pos2(msg_x, center_y - msg_galley.size().y / 2.0),
                        msg_galley,
                        text_col,
                    );
                }
            });
    }

    /// Render the logs pane (styled like QueryPane).
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.poll_results(ui.ctx());

        egui::Frame::new()
            .fill(self.theme.bg_base())
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    self.render_header(ui);
                    self.render_content(ui);
                });
            });
    }
}

/// Implement Component trait so LogsPane can be used in the dashboard.
impl crate::components::Component for LogsPane {
    fn show(&mut self, ui: &mut egui::Ui) {
        LogsPane::show(self, ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    fn set_api_key(&mut self, _key: &str) {
        // Not needed for logs pane
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed for logs pane
    }

    fn label(&self) -> egui::RichText {
        egui::RichText::new(format!("{} {}", semantic_icons::file::TEXT, self.name))
    }

    fn description(&self) -> &str {
        "Log entries for metric correlation"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Get current Unix time in nanoseconds.
fn now_unix_ns() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        use web_time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }
}

/// Format a nanosecond timestamp as HH:MM:SS.mmm.
fn format_timestamp_ns(timestamp_ns: i64) -> String {
    let secs = timestamp_ns / 1_000_000_000;
    let millis = (timestamp_ns % 1_000_000_000) / 1_000_000;

    // Convert to time of day (simplified - just show time portion)
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;

    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

/// Get color for a log level using theme-aware semantic colors.
fn level_color(level: LogLevel, theme: AppTheme) -> Color32 {
    match level {
        LogLevel::Trace => theme.text_tertiary(),
        LogLevel::Debug => theme.text_secondary(),
        LogLevel::Info => theme.semantic_info(),
        LogLevel::Warn => theme.semantic_warning(),
        LogLevel::Error => theme.semantic_error(),
    }
}

/// Get short display text for a log level (for badges).
fn level_to_short(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "TRC",
        LogLevel::Debug => "DBG",
        LogLevel::Info => "INF",
        LogLevel::Warn => "WRN",
        LogLevel::Error => "ERR",
    }
}

/// Get a label for the log level.
fn level_to_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "Trace",
        LogLevel::Debug => "Debug",
        LogLevel::Info => "Info",
        LogLevel::Warn => "Warn",
        LogLevel::Error => "Error",
    }
}

/// Get an icon for the log level.
fn level_icon(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => semantic_icons::status::EMPTY,
        LogLevel::Debug => semantic_icons::status::INFO,
        LogLevel::Info => semantic_icons::status::SUCCESS,
        LogLevel::Warn => semantic_icons::status::WARNING,
        LogLevel::Error => semantic_icons::status::ERROR,
    }
}

/// Truncate a message to fit within a given width.
fn truncate_message(message: &str, max_width: f32, font_size: f32) -> String {
    // Rough estimate: ~6.5 pixels per character at size 11 for monospace
    let chars_per_pixel = font_size * 0.6;
    let max_chars = (max_width / chars_per_pixel) as usize;

    if message.len() <= max_chars {
        message.to_string()
    } else if max_chars > 3 {
        format!("{}...", &message[..max_chars.saturating_sub(3)])
    } else {
        message.chars().take(max_chars).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logs_pane_creation() {
        let start_ns = 1_609_459_200_000_000_000_i64;
        let end_ns = start_ns + 3_600_000_000_000_i64;
        let pane = LogsPane::new(start_ns, end_ns);

        assert_eq!(pane.start_ns, start_ns);
        assert_eq!(pane.end_ns, end_ns);
        assert!(pane.filter_text.is_empty());
        assert!(pane.filter_level.is_none());
        assert!(pane.results.is_none());
    }

    #[test]
    fn test_format_timestamp() {
        // 12:34:56.789
        let ts = 12 * 3600 * 1_000_000_000_i64
            + 34 * 60 * 1_000_000_000_i64
            + 56 * 1_000_000_000_i64
            + 789 * 1_000_000_i64;
        assert_eq!(format_timestamp_ns(ts), "12:34:56.789");
    }

    #[test]
    fn test_truncate_message() {
        assert_eq!(truncate_message("short", 100.0, 11.0), "short");
        // Long message should be truncated
        let long = "a".repeat(100);
        let truncated = truncate_message(&long, 50.0, 11.0);
        assert!(truncated.ends_with("..."));
        assert!(truncated.len() < long.len());
    }

    #[test]
    fn test_level_to_short() {
        assert_eq!(level_to_short(LogLevel::Error), "ERR");
        assert_eq!(level_to_short(LogLevel::Info), "INF");
        assert_eq!(level_to_short(LogLevel::Debug), "DBG");
    }

    #[test]
    fn test_theme_change() {
        use crate::components::Component;

        let start_ns = 1_609_459_200_000_000_000_i64;
        let end_ns = start_ns + 3_600_000_000_000_i64;
        let mut pane = LogsPane::new(start_ns, end_ns);

        // Default is Dark
        assert_eq!(pane.theme, AppTheme::Dark);

        // Change to Light
        pane.set_theme(AppTheme::Light);
        assert_eq!(pane.theme, AppTheme::Light);

        // Change to Nord
        pane.set_theme(AppTheme::Nord);
        assert_eq!(pane.theme, AppTheme::Nord);
    }

    #[test]
    fn test_level_colors_theme_aware() {
        // Verify level colors differ between themes
        let dark_error = level_color(LogLevel::Error, AppTheme::Dark);
        let light_error = level_color(LogLevel::Error, AppTheme::Light);

        // Dark and Light themes should produce different error colors
        assert_ne!(dark_error, light_error);

        // Verify all semantic colors are used
        let theme = AppTheme::Dark;
        assert_eq!(level_color(LogLevel::Error, theme), theme.semantic_error());
        assert_eq!(level_color(LogLevel::Warn, theme), theme.semantic_warning());
        assert_eq!(level_color(LogLevel::Info, theme), theme.semantic_info());
    }

    #[test]
    fn test_query_history_added_on_set() {
        let start_ns = 1_609_459_200_000_000_000_i64;
        let end_ns = start_ns + 3_600_000_000_000_i64;
        let mut pane = LogsPane::new(start_ns, end_ns);

        // History should start empty
        assert!(pane.query_history().is_empty());

        // Setting a query should add it to history
        pane.set_query("{app=\"nginx\"}");
        assert_eq!(pane.query_history().len(), 1);
        assert_eq!(pane.query_history()[0], "{app=\"nginx\"}");

        // Setting a different query should add to front
        pane.set_query("{job=\"varlogs\"}");
        assert_eq!(pane.query_history().len(), 2);
        assert_eq!(pane.query_history()[0], "{job=\"varlogs\"}");
        assert_eq!(pane.query_history()[1], "{app=\"nginx\"}");
    }

    #[test]
    fn test_query_history_deduplication() {
        let start_ns = 1_609_459_200_000_000_000_i64;
        let end_ns = start_ns + 3_600_000_000_000_i64;
        let mut pane = LogsPane::new(start_ns, end_ns);

        pane.set_query("{app=\"nginx\"}");
        pane.set_query("{job=\"varlogs\"}");
        pane.set_query("{app=\"nginx\"}"); // Duplicate

        // Should deduplicate and move to front
        assert_eq!(pane.query_history().len(), 2);
        assert_eq!(pane.query_history()[0], "{app=\"nginx\"}");
        assert_eq!(pane.query_history()[1], "{job=\"varlogs\"}");
    }

    #[test]
    fn test_query_history_empty_query_not_added() {
        let start_ns = 1_609_459_200_000_000_000_i64;
        let end_ns = start_ns + 3_600_000_000_000_i64;
        let mut pane = LogsPane::new(start_ns, end_ns);

        pane.set_query("");
        assert!(pane.query_history().is_empty());
    }

    #[test]
    fn test_query_history_max_size() {
        let start_ns = 1_609_459_200_000_000_000_i64;
        let end_ns = start_ns + 3_600_000_000_000_i64;
        let mut pane = LogsPane::new(start_ns, end_ns);

        // Add more than MAX_HISTORY_SIZE queries
        for i in 0..25 {
            pane.set_query(format!("{{app=\"test{i}\"}}"));
        }

        // Should be capped at MAX_HISTORY_SIZE
        assert_eq!(pane.query_history().len(), MAX_HISTORY_SIZE);
        // Most recent should be first
        assert_eq!(pane.query_history()[0], "{app=\"test24\"}");
    }

    #[test]
    fn test_query_history_clear() {
        let start_ns = 1_609_459_200_000_000_000_i64;
        let end_ns = start_ns + 3_600_000_000_000_i64;
        let mut pane = LogsPane::new(start_ns, end_ns);

        pane.set_query("{app=\"nginx\"}");
        pane.set_query("{job=\"varlogs\"}");
        assert_eq!(pane.query_history().len(), 2);

        pane.clear_history();
        assert!(pane.query_history().is_empty());
    }
}
