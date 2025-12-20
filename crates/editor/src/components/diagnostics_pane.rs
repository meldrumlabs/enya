//! Diagnostics pane component - LSP-style diagnostics for the Enya editor.
//!
//! Displays query validation errors, warnings, and hints in a dedicated pane.
//! Can be added to the viewport like any other component.

use std::any::Any;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use egui::{Color32, Key, RichText, ScrollArea, Ui};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::semantic_icons;
use crate::util::Instant;

use super::Component;
use super::finder_utils::OverlayStyle;

/// Unique ID counter for diagnostics
static NEXT_DIAGNOSTIC_ID: AtomicU64 = AtomicU64::new(1);

/// Unique ID counter for panes
static NEXT_PANE_ID: AtomicUsize = AtomicUsize::new(5000);

/// Diagnostic severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticLevel {
    /// Error - query won't execute
    Error,
    /// Warning - query runs but may have issues
    Warning,
    /// Info - informational hints
    Info,
    /// Hint - suggestions for improvement
    Hint,
}

impl DiagnosticLevel {
    /// Get the icon for this diagnostic level
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Error => semantic_icons::diagnostic::ERROR,
            Self::Warning => semantic_icons::diagnostic::WARNING,
            Self::Info => semantic_icons::diagnostic::INFO,
            Self::Hint => semantic_icons::diagnostic::HINT,
        }
    }

    /// Get the accent color for this diagnostic level
    pub fn color(&self, theme: AppTheme) -> Color32 {
        match self {
            Self::Error => match theme {
                AppTheme::Light => Color32::from_rgb(220, 38, 38),
                AppTheme::Dark => Color32::from_rgb(248, 113, 113),
            },
            Self::Warning => match theme {
                AppTheme::Light => Color32::from_rgb(217, 119, 6),
                AppTheme::Dark => Color32::from_rgb(251, 191, 36),
            },
            Self::Info => match theme {
                AppTheme::Light => Color32::from_rgb(37, 99, 235),
                AppTheme::Dark => Color32::from_rgb(96, 165, 250),
            },
            Self::Hint => match theme {
                AppTheme::Light => Color32::from_rgb(22, 163, 74),
                AppTheme::Dark => Color32::from_rgb(74, 222, 128),
            },
        }
    }

    /// Get the background color for this diagnostic level
    pub fn bg_color(&self, theme: AppTheme) -> Color32 {
        match self {
            Self::Error => match theme {
                AppTheme::Light => Color32::from_rgb(254, 242, 242),
                AppTheme::Dark => Color32::from_rgb(51, 28, 28),
            },
            Self::Warning => match theme {
                AppTheme::Light => Color32::from_rgb(255, 251, 235),
                AppTheme::Dark => Color32::from_rgb(54, 47, 22),
            },
            Self::Info => match theme {
                AppTheme::Light => Color32::from_rgb(239, 246, 255),
                AppTheme::Dark => Color32::from_rgb(30, 41, 59),
            },
            Self::Hint => match theme {
                AppTheme::Light => Color32::from_rgb(240, 253, 244),
                AppTheme::Dark => Color32::from_rgb(20, 51, 36),
            },
        }
    }

    /// Get the label for this diagnostic level
    pub fn label(&self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Info => "Info",
            Self::Hint => "Hint",
        }
    }
}

/// Source of a diagnostic
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSource {
    /// Query syntax error (from parser)
    QuerySyntax,
    /// Query semantic validation (e.g., unknown tag keys)
    QueryValidation,
    /// Data connection error
    DataConnection,
    /// Performance hint
    Performance,
    /// Import operation (e.g., Grafana dashboard import)
    Import,
    /// Unknown/other source
    Unknown,
}

impl DiagnosticSource {
    /// Get the label for this diagnostic source
    pub fn label(&self) -> &'static str {
        match self {
            Self::QuerySyntax => "syntax",
            Self::QueryValidation => "validation",
            Self::DataConnection => "connection",
            Self::Performance => "performance",
            Self::Import => "import",
            Self::Unknown => "unknown",
        }
    }
}

/// A single diagnostic message
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Unique identifier
    pub id: u64,
    /// Severity level
    pub level: DiagnosticLevel,
    /// The diagnostic message
    pub message: String,
    /// Source of the diagnostic
    pub source: DiagnosticSource,
    /// Optional line number (1-indexed)
    pub line: Option<usize>,
    /// Optional column number (1-indexed)
    pub column: Option<usize>,
    /// Optional error code
    pub code: Option<String>,
    /// Related pane ID (if this diagnostic is for a specific pane)
    pub related_pane_id: Option<usize>,
    /// Related pane name (for display)
    pub related_pane_name: Option<String>,
    /// When the diagnostic was created
    pub timestamp: Instant,
    /// Whether this diagnostic has a suggested fix
    pub fixable: bool,
}

impl Diagnostic {
    /// Create a new diagnostic
    pub fn new(level: DiagnosticLevel, message: impl Into<String>) -> Self {
        Self {
            id: NEXT_DIAGNOSTIC_ID.fetch_add(1, Ordering::Relaxed),
            level,
            message: message.into(),
            source: DiagnosticSource::Unknown,
            line: None,
            column: None,
            code: None,
            related_pane_id: None,
            related_pane_name: None,
            timestamp: Instant::now(),
            fixable: false,
        }
    }

    /// Create an error diagnostic
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Error, message)
    }

    /// Create a warning diagnostic
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Warning, message)
    }

    /// Create an info diagnostic
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Info, message)
    }

    /// Create a hint diagnostic
    pub fn hint(message: impl Into<String>) -> Self {
        Self::new(DiagnosticLevel::Hint, message)
    }

    /// Set the source
    pub fn with_source(mut self, source: DiagnosticSource) -> Self {
        self.source = source;
        self
    }

    /// Set the line number
    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the column number
    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    /// Set the error code
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Set the related pane
    pub fn with_pane(mut self, pane_id: usize, pane_name: impl Into<String>) -> Self {
        self.related_pane_id = Some(pane_id);
        self.related_pane_name = Some(pane_name.into());
        self
    }

    /// Mark as fixable
    pub fn with_fix(mut self) -> Self {
        self.fixable = true;
        self
    }
}

/// Actions that can result from diagnostics pane interaction
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticsPaneAction {
    /// No action
    None,
    /// Jump to the source pane for a diagnostic
    JumpToPane(usize),
    /// Clear all diagnostics
    Clear,
}

/// Filter for which diagnostics to show
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagnosticsFilter {
    /// Show all diagnostics
    #[default]
    All,
    /// Show only errors
    Errors,
    /// Show only warnings
    Warnings,
    /// Show errors and warnings
    ErrorsAndWarnings,
}

impl DiagnosticsFilter {
    /// Check if a diagnostic matches this filter
    pub fn matches(&self, level: DiagnosticLevel) -> bool {
        match self {
            Self::All => true,
            Self::Errors => level == DiagnosticLevel::Error,
            Self::Warnings => level == DiagnosticLevel::Warning,
            Self::ErrorsAndWarnings => {
                level == DiagnosticLevel::Error || level == DiagnosticLevel::Warning
            }
        }
    }

    /// Cycle to the next filter
    pub fn cycle(&self) -> Self {
        match self {
            Self::All => Self::Errors,
            Self::Errors => Self::Warnings,
            Self::Warnings => Self::ErrorsAndWarnings,
            Self::ErrorsAndWarnings => Self::All,
        }
    }

    /// Get the label for this filter
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Errors => "Errors",
            Self::Warnings => "Warnings",
            Self::ErrorsAndWarnings => "Errors & Warnings",
        }
    }
}

/// Diagnostics pane component
pub struct DiagnosticsPane {
    /// Unique pane identifier
    id: usize,
    /// Current theme
    theme: AppTheme,
    /// List of diagnostics
    diagnostics: Vec<Diagnostic>,
    /// Currently selected diagnostic ID
    selected_id: Option<u64>,
    /// Filter for which diagnostics to show
    filter: DiagnosticsFilter,
    /// Whether to auto-scroll to new diagnostics (planned for future use)
    #[allow(dead_code)]
    auto_scroll: bool,
    /// Whether the overlay is open
    is_open: bool,
    /// Whether we just opened (to prevent immediate close)
    just_opened: bool,
}

impl Default for DiagnosticsPane {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticsPane {
    /// Create a new diagnostics pane
    pub fn new() -> Self {
        Self {
            id: NEXT_PANE_ID.fetch_add(1, Ordering::Relaxed),
            theme: AppTheme::default(),
            diagnostics: Vec::new(),
            selected_id: None,
            filter: DiagnosticsFilter::default(),
            auto_scroll: true,
            is_open: false,
            just_opened: false,
        }
    }

    /// Create a new diagnostics pane with initial diagnostics
    pub fn with_diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            id: NEXT_PANE_ID.fetch_add(1, Ordering::Relaxed),
            theme: AppTheme::default(),
            diagnostics,
            selected_id: None,
            filter: DiagnosticsFilter::default(),
            auto_scroll: true,
            is_open: false,
            just_opened: false,
        }
    }

    /// Add a diagnostic
    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Add multiple diagnostics
    pub fn add_all(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        self.diagnostics.extend(diagnostics);
    }

    /// Clear all diagnostics
    pub fn clear(&mut self) {
        self.diagnostics.clear();
        self.selected_id = None;
    }

    /// Clear diagnostics for a specific pane
    pub fn clear_for_pane(&mut self, pane_id: usize) {
        self.diagnostics
            .retain(|d| d.related_pane_id != Some(pane_id));
        // Clear selection if it was for this pane
        if let Some(selected) = self.selected_id {
            if !self.diagnostics.iter().any(|d| d.id == selected) {
                self.selected_id = None;
            }
        }
    }

    /// Get the count of diagnostics by level
    pub fn count_by_level(&self) -> (usize, usize, usize, usize) {
        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;
        let mut hints = 0;

        for d in &self.diagnostics {
            match d.level {
                DiagnosticLevel::Error => errors += 1,
                DiagnosticLevel::Warning => warnings += 1,
                DiagnosticLevel::Info => infos += 1,
                DiagnosticLevel::Hint => hints += 1,
            }
        }

        (errors, warnings, infos, hints)
    }

    /// Get the total count of diagnostics
    pub fn count(&self) -> usize {
        self.diagnostics.len()
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.level == DiagnosticLevel::Error)
    }

    /// Get all diagnostics (cloned)
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.diagnostics.clone()
    }

    /// Set the filter
    pub fn set_filter(&mut self, filter: DiagnosticsFilter) {
        self.filter = filter;
    }

    /// Cycle to the next filter
    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.cycle();
    }

    /// Get filtered diagnostics
    fn filtered_diagnostics(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| self.filter.matches(d.level))
            .collect()
    }

    /// Select the next diagnostic
    pub fn select_next(&mut self) {
        let filtered = self.filtered_diagnostics();
        if filtered.is_empty() {
            self.selected_id = None;
            return;
        }

        let current_idx = self
            .selected_id
            .and_then(|id| filtered.iter().position(|d| d.id == id));

        let next_idx = match current_idx {
            Some(idx) => (idx + 1) % filtered.len(),
            None => 0,
        };

        self.selected_id = filtered.get(next_idx).map(|d| d.id);
    }

    /// Select the previous diagnostic
    pub fn select_prev(&mut self) {
        let filtered = self.filtered_diagnostics();
        if filtered.is_empty() {
            self.selected_id = None;
            return;
        }

        let current_idx = self
            .selected_id
            .and_then(|id| filtered.iter().position(|d| d.id == id));

        let prev_idx = match current_idx {
            Some(idx) => {
                if idx == 0 {
                    filtered.len() - 1
                } else {
                    idx - 1
                }
            }
            None => filtered.len() - 1,
        };

        self.selected_id = filtered.get(prev_idx).map(|d| d.id);
    }

    /// Get the currently selected diagnostic's related pane ID
    pub fn selected_pane_id(&self) -> Option<usize> {
        self.selected_id.and_then(|id| {
            self.diagnostics
                .iter()
                .find(|d| d.id == id)
                .and_then(|d| d.related_pane_id)
        })
    }

    /// Open the diagnostics overlay
    pub fn open(&mut self) {
        self.is_open = true;
        self.just_opened = true;
    }

    /// Close the diagnostics overlay
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Toggle the diagnostics overlay
    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    /// Check if the overlay is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Show the diagnostics as a floating overlay
    /// Returns the action if any (JumpToPane, Clear)
    pub fn show_overlay(&mut self, ctx: &egui::Context) -> DiagnosticsPaneAction {
        if !self.is_open {
            return DiagnosticsPaneAction::None;
        }

        let mut action = DiagnosticsPaneAction::None;
        let mut should_close = false;

        // Skip input handling on the first frame after opening
        if self.just_opened {
            self.just_opened = false;
        } else {
            // Handle keyboard input
            ctx.input_mut(|input| {
                // Escape or Space+d to close
                if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    should_close = true;
                }
                // j/k for navigation
                if input.consume_key(egui::Modifiers::NONE, Key::J) {
                    self.select_next();
                }
                if input.consume_key(egui::Modifiers::NONE, Key::K) {
                    self.select_prev();
                }
                // f to cycle filter
                if input.consume_key(egui::Modifiers::NONE, Key::F) {
                    self.cycle_filter();
                }
                // c to clear
                if input.consume_key(egui::Modifiers::NONE, Key::C) {
                    self.clear();
                    action = DiagnosticsPaneAction::Clear;
                }
                // Enter to jump to selected diagnostic's pane
                if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    if let Some(pane_id) = self.selected_pane_id() {
                        action = DiagnosticsPaneAction::JumpToPane(pane_id);
                        should_close = true;
                    }
                }
            });
        }

        if should_close {
            self.close();
            return action;
        }

        // Calculate popup dimensions (match metrics/workspace finder sizes)
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.70).clamp(600.0, 850.0);
        let popup_max_height = (screen_rect.height() * 0.75).min(650.0);

        let text_col = text_color(self.theme);
        let (errors, warnings, infos, hints) = self.count_by_level();

        // Use shared overlay style
        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let separator_color = match self.theme {
            AppTheme::Light => palette::light_border::SUBTLE,
            AppTheme::Dark => palette::border::SUBTLE,
        };
        let muted_text = text_col.gamma_multiply(0.6);
        let key_bg = match self.theme {
            AppTheme::Light => Color32::from_rgba_unmultiplied(240, 240, 240, 200),
            AppTheme::Dark => Color32::from_rgba_unmultiplied(40, 40, 40, 200),
        };

        egui::Area::new(egui::Id::new("diagnostics_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                        ui.set_width(popup_width);
                        ui.set_max_height(popup_max_height);

                        // Header section
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(semantic_icons::diagnostic::INFO)
                                    .color(muted_text)
                                    .size(20.0),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("Diagnostics")
                                    .color(text_col)
                                    .size(18.0)
                                    .strong(),
                            );

                            ui.add_space(16.0);

                            // Count badges
                            if errors > 0 {
                                let color = DiagnosticLevel::Error.color(self.theme);
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        semantic_icons::diagnostic::ERROR,
                                        errors
                                    ))
                                    .color(color)
                                    .size(13.0),
                                );
                            }
                            if warnings > 0 {
                                let color = DiagnosticLevel::Warning.color(self.theme);
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        semantic_icons::diagnostic::WARNING,
                                        warnings
                                    ))
                                    .color(color)
                                    .size(13.0),
                                );
                            }
                            if infos > 0 {
                                let color = DiagnosticLevel::Info.color(self.theme);
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        semantic_icons::diagnostic::INFO,
                                        infos
                                    ))
                                    .color(color)
                                    .size(13.0),
                                );
                            }
                            if hints > 0 {
                                let color = DiagnosticLevel::Hint.color(self.theme);
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        semantic_icons::diagnostic::HINT,
                                        hints
                                    ))
                                    .color(color)
                                    .size(13.0),
                                );
                            }

                            // Filter indicator on the right
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(16.0);
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {}",
                                            semantic_icons::action::FILTER,
                                            self.filter.label()
                                        ))
                                        .color(muted_text)
                                        .size(11.0),
                                    );
                                },
                            );
                        });
                        ui.add_space(8.0);

                        // Keyboard hints
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            Self::render_key_hint(ui, "j/k", "navigate", key_bg, muted_text);
                            ui.add_space(8.0);
                            Self::render_key_hint(ui, "Enter", "jump", key_bg, muted_text);
                            ui.add_space(8.0);
                            Self::render_key_hint(ui, "f", "filter", key_bg, muted_text);
                            ui.add_space(8.0);
                            Self::render_key_hint(ui, "c", "clear", key_bg, muted_text);
                            ui.add_space(8.0);
                            Self::render_key_hint(ui, "Esc", "close", key_bg, muted_text);
                        });
                        ui.add_space(12.0);

                        // Separator below header
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().top(),
                            egui::Stroke::new(1.0, separator_color),
                        );
                        ui.add_space(8.0);

                        // Content area with diagnostics list
                        let filtered: Vec<Diagnostic> = self
                            .diagnostics
                            .iter()
                            .filter(|d| self.filter.matches(d.level))
                            .cloned()
                            .collect();

                        if filtered.is_empty() {
                            // Empty state
                            ui.vertical_centered(|ui| {
                                ui.add_space(40.0);
                                ui.label(
                                    RichText::new(semantic_icons::status::SUCCESS)
                                        .color(DiagnosticLevel::Hint.color(self.theme))
                                        .size(40.0),
                                );
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new("No diagnostics")
                                        .color(muted_text)
                                        .size(14.0),
                                );
                                ui.add_space(40.0);
                            });
                        } else {
                            let selected_id = self.selected_id;
                            let theme = self.theme;
                            let mut clicked_diagnostic: Option<(u64, Option<usize>)> = None;

                            ScrollArea::vertical()
                                .auto_shrink([false, false])
                                .max_height(popup_max_height - 120.0)
                                .show(ui, |ui| {
                                    ui.add_space(4.0);
                                    for diagnostic in &filtered {
                                        let is_selected = selected_id == Some(diagnostic.id);
                                        let bg = if is_selected {
                                            diagnostic.level.bg_color(theme)
                                        } else {
                                            Color32::TRANSPARENT
                                        };

                                        ui.horizontal(|ui| {
                                            ui.add_space(12.0);
                                            egui::Frame::new()
                                                .fill(bg)
                                                .corner_radius(4.0)
                                                .inner_margin(egui::vec2(8.0, 6.0))
                                                .show(ui, |ui| {
                                                    ui.set_width(popup_width - 40.0);
                                                    let response = ui
                                                        .horizontal(|ui| {
                                                            // Level icon
                                                            ui.label(
                                                                RichText::new(diagnostic.level.icon())
                                                                    .color(
                                                                        diagnostic.level.color(theme),
                                                                    )
                                                                    .size(14.0),
                                                            );

                                                            ui.add_space(8.0);

                                                            // Message and details
                                                            ui.vertical(|ui| {
                                                                ui.horizontal(|ui| {
                                                                    ui.label(
                                                                        RichText::new(
                                                                            &diagnostic.message,
                                                                        )
                                                                        .color(text_col)
                                                                        .size(12.0),
                                                                    );

                                                                    // Source badge
                                                                    ui.label(
                                                                        RichText::new(format!(
                                                                            "[{}]",
                                                                            diagnostic.source.label()
                                                                        ))
                                                                        .color(
                                                                            text_col
                                                                                .gamma_multiply(0.4),
                                                                        )
                                                                        .size(10.0),
                                                                    );
                                                                });

                                                                // Pane name and location
                                                                if diagnostic
                                                                    .related_pane_name
                                                                    .is_some()
                                                                    || diagnostic.line.is_some()
                                                                {
                                                                    ui.horizontal(|ui| {
                                                                        if let Some(pane_name) =
                                                                            &diagnostic
                                                                                .related_pane_name
                                                                        {
                                                                            ui.label(
                                                                                RichText::new(
                                                                                    pane_name,
                                                                                )
                                                                                .color(
                                                                                    text_col
                                                                                        .gamma_multiply(
                                                                                            0.5,
                                                                                        ),
                                                                                )
                                                                                .size(10.0),
                                                                            );
                                                                        }
                                                                        if let Some(line) =
                                                                            diagnostic.line
                                                                        {
                                                                            let loc = if let Some(
                                                                                col,
                                                                            ) =
                                                                                diagnostic.column
                                                                            {
                                                                                format!(
                                                                                    ":{line}:{col}"
                                                                                )
                                                                            } else {
                                                                                format!(":{line}")
                                                                            };
                                                                            ui.label(
                                                                                RichText::new(loc)
                                                                                    .color(
                                                                                        text_col
                                                                                            .gamma_multiply(
                                                                                                0.5,
                                                                                            ),
                                                                                    )
                                                                                    .size(10.0),
                                                                            );
                                                                        }
                                                                    });
                                                                }
                                                            });
                                                        })
                                                        .response;

                                                    // Handle click to select and jump
                                                    if response
                                                        .interact(egui::Sense::click())
                                                        .clicked()
                                                    {
                                                        clicked_diagnostic = Some((
                                                            diagnostic.id,
                                                            diagnostic.related_pane_id,
                                                        ));
                                                    }
                                                });
                                        });
                                        ui.add_space(2.0);
                                    }
                                    ui.add_space(8.0);
                                });

                            // Apply click state changes after rendering
                            if let Some((diag_id, pane_id)) = clicked_diagnostic {
                                self.selected_id = Some(diag_id);
                                if let Some(pane) = pane_id {
                                    action = DiagnosticsPaneAction::JumpToPane(pane);
                                    self.close();
                                }
                            }
                        }
                    });
            });

        action
    }

    /// Render a keyboard hint badge
    fn render_key_hint(ui: &mut Ui, key: &str, desc: &str, key_bg: Color32, muted_text: Color32) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            egui::Frame::new()
                .fill(key_bg)
                .corner_radius(3.0)
                .inner_margin(egui::vec2(4.0, 2.0))
                .show(ui, |ui| {
                    ui.label(RichText::new(key).color(muted_text).size(10.0).strong());
                });
            ui.label(RichText::new(desc).color(muted_text).size(10.0));
        });
    }

    /// Render the diagnostics pane
    pub fn show_with_action(&mut self, ui: &mut Ui) -> DiagnosticsPaneAction {
        let mut action = DiagnosticsPaneAction::None;
        let text_col = text_color(self.theme);
        let (errors, warnings, infos, hints) = self.count_by_level();

        // Header
        ui.horizontal(|ui| {
            // Title with icon
            ui.label(
                RichText::new(format!("{} Diagnostics", semantic_icons::diagnostic::INFO))
                    .color(text_col)
                    .strong()
                    .size(14.0),
            );

            ui.add_space(8.0);

            // Count badges
            if errors > 0 {
                let color = DiagnosticLevel::Error.color(self.theme);
                ui.label(
                    RichText::new(format!("{} {errors}", semantic_icons::diagnostic::ERROR))
                        .color(color)
                        .size(12.0),
                );
            }
            if warnings > 0 {
                let color = DiagnosticLevel::Warning.color(self.theme);
                ui.label(
                    RichText::new(format!(
                        "{} {warnings}",
                        semantic_icons::diagnostic::WARNING
                    ))
                    .color(color)
                    .size(12.0),
                );
            }
            if infos > 0 {
                let color = DiagnosticLevel::Info.color(self.theme);
                ui.label(
                    RichText::new(format!("{} {infos}", semantic_icons::diagnostic::INFO))
                        .color(color)
                        .size(12.0),
                );
            }
            if hints > 0 {
                let color = DiagnosticLevel::Hint.color(self.theme);
                ui.label(
                    RichText::new(format!("{} {hints}", semantic_icons::diagnostic::HINT))
                        .color(color)
                        .size(12.0),
                );
            }

            // Right side controls
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Clear button
                if ui
                    .small_button(RichText::new(semantic_icons::action::DELETE).size(12.0))
                    .on_hover_text("Clear all diagnostics")
                    .clicked()
                {
                    action = DiagnosticsPaneAction::Clear;
                }

                // Filter button
                if ui
                    .small_button(
                        RichText::new(format!(
                            "{} {}",
                            semantic_icons::action::FILTER,
                            self.filter.label()
                        ))
                        .size(11.0),
                    )
                    .on_hover_text("Cycle filter")
                    .clicked()
                {
                    self.cycle_filter();
                }
            });
        });

        ui.separator();

        // Diagnostics list - clone filtered diagnostics to avoid borrow issues
        let filtered: Vec<Diagnostic> = self
            .diagnostics
            .iter()
            .filter(|d| self.filter.matches(d.level))
            .cloned()
            .collect();
        let selected_id = self.selected_id;
        let theme = self.theme;

        if filtered.is_empty() {
            // Empty state
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(
                    RichText::new(semantic_icons::status::SUCCESS)
                        .color(DiagnosticLevel::Hint.color(theme))
                        .size(32.0),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("No diagnostics")
                        .color(text_col.gamma_multiply(0.6))
                        .size(13.0),
                );
            });
        } else {
            // Track clicks to update state after rendering
            let mut clicked_diagnostic: Option<(u64, Option<usize>)> = None;

            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for diagnostic in &filtered {
                        let is_selected = selected_id == Some(diagnostic.id);
                        let bg_color = if is_selected {
                            diagnostic.level.bg_color(theme)
                        } else {
                            Color32::TRANSPARENT
                        };

                        egui::Frame::new()
                            .fill(bg_color)
                            .corner_radius(4.0)
                            .inner_margin(egui::vec2(8.0, 4.0))
                            .show(ui, |ui| {
                                let response = ui
                                    .horizontal(|ui| {
                                        // Level icon
                                        ui.label(
                                            RichText::new(diagnostic.level.icon())
                                                .color(diagnostic.level.color(theme))
                                                .size(14.0),
                                        );

                                        ui.add_space(4.0);

                                        // Message
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(&diagnostic.message)
                                                        .color(text_col)
                                                        .size(12.0),
                                                );

                                                // Source badge
                                                ui.label(
                                                    RichText::new(format!(
                                                        "[{}]",
                                                        diagnostic.source.label()
                                                    ))
                                                    .color(text_col.gamma_multiply(0.4))
                                                    .size(10.0),
                                                );
                                            });

                                            // Pane name and location
                                            if diagnostic.related_pane_name.is_some()
                                                || diagnostic.line.is_some()
                                            {
                                                ui.horizontal(|ui| {
                                                    if let Some(pane_name) =
                                                        &diagnostic.related_pane_name
                                                    {
                                                        ui.label(
                                                            RichText::new(pane_name)
                                                                .color(text_col.gamma_multiply(0.5))
                                                                .size(10.0),
                                                        );
                                                    }
                                                    if let Some(line) = diagnostic.line {
                                                        let loc =
                                                            if let Some(col) = diagnostic.column {
                                                                format!(":{line}:{col}")
                                                            } else {
                                                                format!(":{line}")
                                                            };
                                                        ui.label(
                                                            RichText::new(loc)
                                                                .color(text_col.gamma_multiply(0.5))
                                                                .size(10.0),
                                                        );
                                                    }
                                                });
                                            }
                                        });
                                    })
                                    .response;

                                // Handle click to select and jump
                                if response.interact(egui::Sense::click()).clicked() {
                                    clicked_diagnostic =
                                        Some((diagnostic.id, diagnostic.related_pane_id));
                                }
                            });

                        ui.add_space(2.0);
                    }
                });

            // Apply click state changes after rendering
            if let Some((diag_id, pane_id)) = clicked_diagnostic {
                self.selected_id = Some(diag_id);
                if let Some(pane) = pane_id {
                    action = DiagnosticsPaneAction::JumpToPane(pane);
                }
            }
        }

        action
    }
}

impl Component for DiagnosticsPane {
    fn show(&mut self, ui: &mut Ui) {
        // Ignore action in Component::show - use show_with_action for action handling
        self.show_with_action(ui);
    }

    fn id(&self) -> usize {
        self.id
    }

    fn name(&self) -> String {
        "Diagnostics".to_string()
    }

    fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    fn set_api_key(&mut self, _key: &str) {
        // Not needed
    }

    fn set_staging_api_key(&mut self, _key: &str) {
        // Not needed
    }

    fn label(&self) -> RichText {
        let (errors, warnings, _, _) = self.count_by_level();
        let icon = if errors > 0 {
            semantic_icons::diagnostic::ERROR
        } else if warnings > 0 {
            semantic_icons::diagnostic::WARNING
        } else {
            semantic_icons::diagnostic::INFO
        };

        let count = self.count();
        if count > 0 {
            RichText::new(format!("{icon} Diagnostics ({count})"))
        } else {
            RichText::new(format!("{icon} Diagnostics"))
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_creation() {
        let diag = Diagnostic::error("Test error")
            .with_source(DiagnosticSource::QuerySyntax)
            .with_line(1)
            .with_column(5);

        assert_eq!(diag.level, DiagnosticLevel::Error);
        assert_eq!(diag.message, "Test error");
        assert_eq!(diag.source, DiagnosticSource::QuerySyntax);
        assert_eq!(diag.line, Some(1));
        assert_eq!(diag.column, Some(5));
    }

    #[test]
    fn test_diagnostics_filter() {
        let filter = DiagnosticsFilter::Errors;
        assert!(filter.matches(DiagnosticLevel::Error));
        assert!(!filter.matches(DiagnosticLevel::Warning));
        assert!(!filter.matches(DiagnosticLevel::Info));
    }

    #[test]
    fn test_diagnostics_pane_counts() {
        let mut pane = DiagnosticsPane::new();
        pane.add(Diagnostic::error("Error 1"));
        pane.add(Diagnostic::error("Error 2"));
        pane.add(Diagnostic::warning("Warning 1"));
        pane.add(Diagnostic::info("Info 1"));

        let (errors, warnings, infos, hints) = pane.count_by_level();
        assert_eq!(errors, 2);
        assert_eq!(warnings, 1);
        assert_eq!(infos, 1);
        assert_eq!(hints, 0);
        assert!(pane.has_errors());
    }

    #[test]
    fn test_clear_for_pane() {
        let mut pane = DiagnosticsPane::new();
        pane.add(Diagnostic::error("Error 1").with_pane(100, "Pane A"));
        pane.add(Diagnostic::error("Error 2").with_pane(200, "Pane B"));
        pane.add(Diagnostic::warning("Warning 1").with_pane(100, "Pane A"));

        assert_eq!(pane.count(), 3);
        pane.clear_for_pane(100);
        assert_eq!(pane.count(), 1);
    }
}
