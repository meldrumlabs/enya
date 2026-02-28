//! Settings overlay - AI provider/model configuration, styling, connection, and codebase settings.

use egui::{Color32, FontFamily, FontId, Key, RichText, Vec2};
use egui_nerdfonts::regular;

use crate::components::util::finder_utils::{OverlayStyle, draw_backdrop};
use crate::components::util::{AiProvider, ProviderManifest};
use crate::ui::ActiveThemeColors;
use crate::ui::semantic_icons;
use crate::ui::settings_screen::EditorFont;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::style_picker::StyleTab;

/// Result returned by the settings overlay each frame.
#[derive(Debug, Clone)]
pub enum SettingsResult {
    /// Settings were saved.
    Saved {
        ai_provider: AiProvider,
        ai_model: Option<String>,
        git_repo_url: String,
        default_prometheus_endpoint: String,
        default_loki_endpoint: String,
        default_tempo_endpoint: String,
    },
    /// Overlay was cancelled (no changes).
    Cancelled,
    /// Live preview of a builtin theme change.
    ThemePreview(AppTheme),
    /// Live preview of a custom theme change.
    CustomThemePreview(String),
    /// Live preview of a font change.
    FontPreview(EditorFont),
    /// Cancelled with restore of original theme/font (from Styling tab).
    CancelledWithRestore {
        theme: AppTheme,
        custom_theme: Option<String>,
        font: EditorFont,
    },
    /// No action this frame.
    None,
}

/// Which tab is active in the settings overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Defaults,
    Ai,
    Styling,
}

impl SettingsTab {
    /// All available tabs.
    fn all() -> &'static [Self] {
        &[Self::Defaults, Self::Ai, Self::Styling]
    }

    /// Next tab in cycle.
    fn next(self) -> Self {
        let tabs = Self::all();
        let idx = tabs.iter().position(|t| *t == self).unwrap_or(0);
        tabs[(idx + 1) % tabs.len()]
    }

    /// Previous tab in cycle.
    fn prev(self) -> Self {
        let tabs = Self::all();
        let idx = tabs.iter().position(|t| *t == self).unwrap_or(0);
        tabs[if idx == 0 { tabs.len() - 1 } else { idx - 1 }]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Defaults => "Defaults",
            Self::Ai => "AI",
            Self::Styling => "Styling",
        }
    }
}

/// Which field is being text-edited (Enter to start, Escape/Enter to finish).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditingField {
    GitRepoUrl,
    PrometheusEndpoint,
    LokiEndpoint,
    TempoEndpoint,
}

/// Settings overlay with Defaults, AI, and Styling tabs.
pub struct SettingsOverlay {
    is_open: bool,
    theme: AppTheme,
    // Current tab
    tab: SettingsTab,
    // Field index within current tab (for j/k navigation)
    field_index: usize,
    // Text editing state
    editing_field: Option<EditingField>,
    // Working copies of settings (edited in-place, committed on save)
    ai_provider: AiProvider,
    ai_model: Option<String>,
    git_repo_url: String,
    // AI tab dropdown state (true = dropdown list is expanded)
    ai_dropdown_open: bool,
    default_prometheus_endpoint: String,
    default_loki_endpoint: String,
    default_tempo_endpoint: String,
    // Styling tab state
    styling_theme: AppTheme,
    original_theme: AppTheme,
    styling_custom_theme: Option<String>,
    original_custom_theme: Option<String>,
    styling_font: EditorFont,
    original_font: EditorFont,
    custom_themes: Vec<(String, String, ActiveThemeColors)>,
    theme_index: usize,
    font_index: usize,
    // Styling panel-based navigation (like style picker)
    focused_panel: StyleTab,
    panel_switch_anim: f32,
    scroll_to_theme: bool,
    scroll_to_font: bool,
    // Whether j/k navigation should scroll the Defaults tab
    scroll_to_defaults: bool,
    // Pending result from styling tab (emitted on next show() call)
    pending_styling_result: Option<SettingsResult>,
}

impl Default for SettingsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsOverlay {
    pub fn new() -> Self {
        Self {
            is_open: false,
            theme: AppTheme::default(),
            tab: SettingsTab::Defaults,
            field_index: 0,
            editing_field: None,
            ai_provider: AiProvider::default(),
            ai_model: None,
            git_repo_url: String::new(),
            ai_dropdown_open: false,
            default_prometheus_endpoint: String::new(),
            default_loki_endpoint: String::new(),
            default_tempo_endpoint: String::new(),
            styling_theme: AppTheme::default(),
            original_theme: AppTheme::default(),
            styling_custom_theme: None,
            original_custom_theme: None,
            styling_font: EditorFont::default(),
            original_font: EditorFont::default(),
            custom_themes: Vec::new(),
            theme_index: 0,
            font_index: 0,
            focused_panel: StyleTab::Theme,
            panel_switch_anim: 0.0,
            scroll_to_theme: false,
            scroll_to_font: false,
            scroll_to_defaults: false,
            pending_styling_result: None,
        }
    }

    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the overlay with current settings values.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        &mut self,
        ai_provider: AiProvider,
        ai_model: Option<String>,
        git_repo_url: String,
        default_prometheus_endpoint: String,
        default_loki_endpoint: String,
        default_tempo_endpoint: String,
        current_theme: AppTheme,
        current_custom_theme: Option<String>,
        current_font: EditorFont,
        custom_themes: Vec<(String, String, ActiveThemeColors)>,
    ) {
        self.is_open = true;
        self.tab = SettingsTab::Defaults;
        self.field_index = 0;
        self.editing_field = None;
        self.ai_provider = ai_provider;
        self.ai_model = ai_model;
        self.git_repo_url = git_repo_url;
        self.ai_dropdown_open = false;
        self.default_prometheus_endpoint = default_prometheus_endpoint;
        self.default_loki_endpoint = default_loki_endpoint;
        self.default_tempo_endpoint = default_tempo_endpoint;
        // Styling tab
        self.styling_theme = current_theme;
        self.original_theme = current_theme;
        self.styling_custom_theme = current_custom_theme.clone();
        self.original_custom_theme = current_custom_theme.clone();
        self.styling_font = current_font;
        self.original_font = current_font;
        self.custom_themes = custom_themes;
        self.focused_panel = StyleTab::Theme;
        self.panel_switch_anim = 0.0;
        self.scroll_to_theme = false;
        self.scroll_to_font = false;
        self.scroll_to_defaults = false;
        self.pending_styling_result = None;
        // Compute initial theme index
        let builtin_count = AppTheme::all().len();
        if let Some(ref name) = current_custom_theme {
            self.theme_index = builtin_count
                + self
                    .custom_themes
                    .iter()
                    .position(|(n, _, _)| n == name)
                    .unwrap_or(0);
        } else {
            self.theme_index = AppTheme::all()
                .iter()
                .position(|t| *t == current_theme)
                .unwrap_or(0);
        }
        // Compute initial font index
        self.font_index = EditorFont::all()
            .iter()
            .position(|f| *f == current_font)
            .unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.editing_field = None;
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Number of navigable fields in the current tab.
    fn field_count(&self) -> usize {
        match self.tab {
            SettingsTab::Defaults => 4, // Prom URL, Loki URL, Tempo URL, Git URL
            SettingsTab::Ai => 2,       // Provider, Model
            SettingsTab::Styling => 0,  // Panel-based navigation (not field-based)
        }
    }

    /// Total number of themes (builtin + custom).
    fn total_theme_count(&self) -> usize {
        AppTheme::all().len() + self.custom_themes.len()
    }

    /// Navigate theme index by delta, update state, return preview result.
    fn navigate_theme(&mut self, delta: i32) -> SettingsResult {
        let total = self.total_theme_count();
        if total == 0 {
            return SettingsResult::None;
        }
        let new_idx = ((self.theme_index as i32 + delta).rem_euclid(total as i32)) as usize;
        self.theme_index = new_idx;
        self.scroll_to_theme = true;
        let builtin_count = AppTheme::all().len();
        if new_idx < builtin_count {
            self.styling_theme = AppTheme::all()[new_idx];
            self.styling_custom_theme = None;
            SettingsResult::ThemePreview(self.styling_theme)
        } else {
            let custom_idx = new_idx - builtin_count;
            let name = self.custom_themes[custom_idx].0.clone();
            self.styling_custom_theme = Some(name.clone());
            SettingsResult::CustomThemePreview(name)
        }
    }

    /// Navigate font index by delta, update state, return preview result.
    fn navigate_font(&mut self, delta: i32) -> SettingsResult {
        let fonts = EditorFont::all();
        let total = fonts.len();
        if total == 0 {
            return SettingsResult::None;
        }
        let new_idx = ((self.font_index as i32 + delta).rem_euclid(total as i32)) as usize;
        self.font_index = new_idx;
        self.scroll_to_font = true;
        self.styling_font = fonts[new_idx];
        SettingsResult::FontPreview(self.styling_font)
    }

    /// Set theme index directly (e.g. from click), return preview result.
    fn select_theme(&mut self, index: usize) -> SettingsResult {
        let builtin_count = AppTheme::all().len();
        self.theme_index = index;
        if index < builtin_count {
            self.styling_theme = AppTheme::all()[index];
            self.styling_custom_theme = None;
            SettingsResult::ThemePreview(self.styling_theme)
        } else {
            let custom_idx = index - builtin_count;
            let name = self.custom_themes[custom_idx].0.clone();
            self.styling_custom_theme = Some(name.clone());
            SettingsResult::CustomThemePreview(name)
        }
    }

    /// Set font index directly (e.g. from click), return preview result.
    fn select_font(&mut self, index: usize) -> SettingsResult {
        let fonts = EditorFont::all();
        self.font_index = index;
        self.styling_font = fonts[index];
        SettingsResult::FontPreview(self.styling_font)
    }

    /// Build the saved result with all current values.
    fn build_saved(&self) -> SettingsResult {
        SettingsResult::Saved {
            ai_provider: self.ai_provider,
            ai_model: self.ai_model.clone(),
            git_repo_url: self.git_repo_url.clone(),
            default_prometheus_endpoint: self.default_prometheus_endpoint.clone(),
            default_loki_endpoint: self.default_loki_endpoint.clone(),
            default_tempo_endpoint: self.default_tempo_endpoint.clone(),
        }
    }

    /// Show the settings overlay. Returns a result each frame.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> SettingsResult {
        if !self.is_open {
            return SettingsResult::None;
        }

        let mut result = SettingsResult::None;

        // Emit pending styling result (from previous frame's keyboard handling)
        if let Some(pending) = self.pending_styling_result.take() {
            result = pending;
        }

        // Handle keyboard input
        let kb_result = self.handle_keyboard(ctx);
        match kb_result {
            SettingsResult::None => {}
            other => result = other,
        }

        // Calculate popup dimensions — consistent size across all tabs
        let popup_width = crate::util::overlay_width(ctx, 0.55, 620.0, 740.0);

        // Backdrop to block mouse events from reaching the landing page
        draw_backdrop(ctx, self.theme, "settings");

        egui::Area::new(egui::Id::new("settings_overlay"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .constrain_to(crate::util::overlay_content_rect(ctx))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let overlay_style = OverlayStyle::frosted_glass(self.theme);
                let accent = self.theme.accent_primary();
                let text_primary = self.theme.text_primary();
                let text_secondary = self.theme.text_secondary();
                let text_tertiary = self.theme.text_tertiary();
                let separator = self.theme.border_subtle();

                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Header
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(semantic_icons::action::SETTINGS)
                                .color(accent)
                                .size(20.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Settings")
                                .color(accent)
                                .size(18.0)
                                .strong(),
                        );
                    });
                    ui.add_space(12.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator),
                    );
                    ui.add_space(8.0);

                    // Tab bar with pill-shaped active indicator
                    let tabs = SettingsTab::all();
                    let mut tab_rects: Vec<(egui::Rect, bool)> = Vec::new();
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);

                        for (i, tab) in tabs.iter().enumerate() {
                            if i > 0 {
                                ui.add_space(8.0);
                            }
                            let selected = self.tab == *tab;
                            let tab_color = if selected { accent } else { text_tertiary };

                            // Render number + label together to measure pill rect
                            let start_pos = ui.cursor().min;
                            ui.add_space(8.0); // pill left padding
                            ui.label(
                                RichText::new(format!("{}", i + 1))
                                    .color(tab_color.gamma_multiply(0.5))
                                    .font(typography::monospace(typography::SM)),
                            );
                            ui.add_space(2.0);
                            let response = ui.add(
                                egui::Label::new(
                                    RichText::new(tab.label())
                                        .color(tab_color)
                                        .font(typography::proportional(typography::MD))
                                        .strong(),
                                )
                                .sense(egui::Sense::click()),
                            );
                            ui.add_space(8.0); // pill right padding
                            let end_pos = ui.cursor().min;

                            // Pill rect spans from start to end, full tab bar height
                            let pill_rect = egui::Rect::from_min_max(
                                egui::pos2(start_pos.x, response.rect.min.y - 4.0),
                                egui::pos2(end_pos.x, response.rect.max.y + 4.0),
                            );
                            tab_rects.push((pill_rect, selected));

                            if response.clicked() {
                                self.tab = *tab;
                                self.field_index = 0;
                                self.editing_field = None;
                            }
                        }
                    });

                    // Draw pill backgrounds (behind text, so draw at paint layer)
                    for &(rect, selected) in &tab_rects {
                        if selected {
                            ui.painter().rect_filled(
                                rect,
                                6.0,
                                accent.gamma_multiply(0.12),
                            );
                        }
                    }

                    ui.add_space(12.0);

                    // Content area — fixed height across all tabs
                    let content_width = popup_width - 32.0;
                    let content_height = 330.0;
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.vertical(|ui| {
                            ui.set_width(content_width);
                            ui.set_min_height(content_height);
                            ui.set_max_height(content_height);

                            match self.tab {
                                SettingsTab::Ai => {
                                    self.show_ai_tab(
                                        ui,
                                        accent,
                                        text_primary,
                                        text_secondary,
                                        text_tertiary,
                                    );
                                }
                                SettingsTab::Styling => {
                                    self.show_styling_tab(
                                        ui,
                                        ctx,
                                        accent,
                                        text_primary,
                                        text_tertiary,
                                        content_width,
                                        &overlay_style,
                                        &mut result,
                                    );
                                }
                                SettingsTab::Defaults => {
                                    self.show_defaults_tab(
                                        ui,
                                        accent,
                                        text_primary,
                                        text_secondary,
                                        text_tertiary,
                                    );
                                }
                            }
                        });
                    });

                    ui.add_space(16.0);

                    // Separator above footer
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator),
                    );
                    ui.add_space(10.0);

                    // Footer hints — context-dependent
                    ui.vertical_centered(|ui| {
                        let hint = if self.tab == SettingsTab::Styling {
                            "h/l switch panel \u{2022} j/k navigate \u{2022} Tab switch tab \u{2022} Esc close"
                        } else {
                            "j/k navigate \u{2022} Enter edit/cycle \u{2022} Tab switch tab \u{2022} Esc close"
                        };
                        ui.label(
                            RichText::new(hint)
                                .color(text_tertiary.gamma_multiply(0.7))
                                .size(typography::XS),
                        );
                    });
                    ui.add_space(10.0);
                });
            });

        // Close after rendering if needed
        if matches!(
            result,
            SettingsResult::Saved { .. }
                | SettingsResult::Cancelled
                | SettingsResult::CancelledWithRestore { .. }
        ) {
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
        }

        result
    }

    /// Render the AI settings tab.
    fn show_ai_tab(
        &mut self,
        ui: &mut egui::Ui,
        accent: egui::Color32,
        text_primary: egui::Color32,
        _text_secondary: egui::Color32,
        text_tertiary: egui::Color32,
    ) {
        let card_bg = self.theme.bg_elevated().gamma_multiply(0.3);
        let card_border = self.theme.border_subtle().gamma_multiply(0.4);
        let bg_hover = self.theme.bg_hover();

        Self::show_section_header(ui, "Configuration", text_tertiary);

        egui::Frame::new()
            .fill(card_bg)
            .stroke(egui::Stroke::new(1.0, card_border))
            .corner_radius(8.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                // Provider
                let provider_focused = self.field_index == 0 && self.editing_field.is_none();
                let provider_expanded = provider_focused && self.ai_dropdown_open;

                self.show_dropdown_label(
                    ui,
                    "Provider",
                    provider_focused,
                    provider_expanded,
                    accent,
                    text_primary,
                    text_tertiary,
                );

                if provider_expanded {
                    // Expanded dropdown: show all providers
                    let providers = AiProvider::all();
                    for provider in providers {
                        let is_selected = self.ai_provider == *provider;
                        let row_response = self.show_dropdown_option(
                            ui,
                            provider.display_name(),
                            is_selected,
                            accent,
                            text_primary,
                            text_tertiary,
                            bg_hover,
                        );
                        if row_response.clicked() {
                            self.ai_provider = *provider;
                            self.ai_model = None;
                            self.ai_dropdown_open = false;
                        }
                    }
                } else {
                    // Collapsed: show current value
                    let clicked = self.show_dropdown_value(
                        ui,
                        self.ai_provider.display_name(),
                        provider_focused,
                        accent,
                        text_primary,
                        text_tertiary,
                    );
                    if clicked {
                        self.field_index = 0;
                        self.ai_dropdown_open = true;
                    }
                }

                Self::show_field_divider(ui, card_border);

                // Model
                let model_focused = self.field_index == 1 && self.editing_field.is_none();
                let model_expanded = model_focused && self.ai_dropdown_open;
                let current_model_id = self.ai_model.clone().unwrap_or_else(|| {
                    ProviderManifest::default_model_id_for(self.ai_provider).unwrap_or_default()
                });
                let current_model_name = ProviderManifest::display_name_for(&current_model_id);

                self.show_dropdown_label(
                    ui,
                    "Model",
                    model_focused,
                    model_expanded,
                    accent,
                    text_primary,
                    text_tertiary,
                );

                if model_expanded {
                    // Expanded dropdown: show all models for current provider
                    let models = ProviderManifest::models_for(self.ai_provider);
                    for model in &models {
                        let is_selected = current_model_id == model.id;
                        let row_response = self.show_dropdown_option(
                            ui,
                            model.display_name(),
                            is_selected,
                            accent,
                            text_primary,
                            text_tertiary,
                            bg_hover,
                        );
                        if row_response.clicked() {
                            self.ai_model = Some(model.id.clone());
                            self.ai_dropdown_open = false;
                        }
                    }
                } else {
                    // Collapsed: show current value
                    let clicked = self.show_dropdown_value(
                        ui,
                        &current_model_name,
                        model_focused,
                        accent,
                        text_primary,
                        text_tertiary,
                    );
                    if clicked {
                        self.field_index = 1;
                        self.ai_dropdown_open = true;
                    }
                }
            });
    }

    /// Render the label for a dropdown field (with open/closed chevron).
    #[allow(clippy::too_many_arguments)]
    fn show_dropdown_label(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        is_focused: bool,
        is_expanded: bool,
        accent: egui::Color32,
        text_primary: egui::Color32,
        text_tertiary: egui::Color32,
    ) {
        ui.horizontal(|ui| {
            let label_color = if is_focused {
                text_primary
            } else {
                text_tertiary
            };
            ui.label(
                RichText::new(label)
                    .color(label_color)
                    .font(typography::proportional(typography::SM)),
            );
            if is_focused {
                let chevron = if is_expanded { "\u{25be}" } else { "\u{25b8}" };
                ui.label(
                    RichText::new(chevron)
                        .color(accent.gamma_multiply(0.5))
                        .font(typography::monospace(typography::XS)),
                );
            }
        });
        ui.add_space(3.0);
    }

    /// Render the collapsed value for a dropdown field. Returns true if clicked.
    #[allow(clippy::too_many_arguments)]
    fn show_dropdown_value(
        &self,
        ui: &mut egui::Ui,
        value: &str,
        is_focused: bool,
        accent: egui::Color32,
        text_primary: egui::Color32,
        _text_tertiary: egui::Color32,
    ) -> bool {
        let input_height = 30.0;
        let avail_width = ui.available_width();
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(avail_width, input_height), egui::Sense::click());

        let bg = if is_focused {
            self.theme.bg_surface()
        } else {
            self.theme.bg_surface().gamma_multiply(0.5)
        };
        let border_color = if is_focused {
            accent.gamma_multiply(0.6)
        } else {
            self.theme.border_subtle().gamma_multiply(0.4)
        };
        let border_width = if is_focused { 1.5 } else { 1.0 };

        ui.painter().rect(
            rect,
            6.0,
            bg,
            egui::Stroke::new(border_width, border_color),
            egui::StrokeKind::Inside,
        );

        ui.painter().text(
            egui::pos2(rect.min.x + 12.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            value,
            typography::monospace(typography::MD),
            text_primary.gamma_multiply(if is_focused { 1.0 } else { 0.8 }),
        );

        // Hint to open
        if is_focused {
            ui.painter().text(
                egui::pos2(rect.max.x - 12.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "l:open",
                typography::monospace(typography::XS),
                accent.gamma_multiply(0.4),
            );
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response.clicked()
    }

    /// Render a single option row in an expanded dropdown.
    #[allow(clippy::too_many_arguments)]
    fn show_dropdown_option(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        is_selected: bool,
        accent: egui::Color32,
        text_primary: egui::Color32,
        text_tertiary: egui::Color32,
        bg_hover: egui::Color32,
    ) -> egui::Response {
        let row_height = 28.0;
        let avail_width = ui.available_width();
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(avail_width, row_height), egui::Sense::click());

        // Background: selected = accent tint, hovered = hover color
        let bg = if is_selected {
            accent.gamma_multiply(0.15)
        } else if response.hovered() {
            bg_hover.gamma_multiply(0.5)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 4.0, bg);

        // Selection dot
        if is_selected {
            ui.painter()
                .circle_filled(egui::pos2(rect.min.x + 10.0, rect.center().y), 3.0, accent);
        }

        // Label
        let text_color = if is_selected {
            accent
        } else if response.hovered() {
            text_primary
        } else {
            text_tertiary
        };
        ui.painter().text(
            egui::pos2(rect.min.x + 22.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            typography::monospace(typography::MD),
            text_color,
        );

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        response
    }

    /// Render the Styling tab — side-by-side Theme and Font panels (matching Style Picker layout).
    #[allow(clippy::too_many_arguments)]
    fn show_styling_tab(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        accent: Color32,
        text: Color32,
        text_muted: Color32,
        content_width: f32,
        style: &OverlayStyle,
        result: &mut SettingsResult,
    ) {
        let bg_elevated = self.theme.bg_elevated();
        let bg_hover = self.theme.bg_hover();
        let panel_width = (content_width - 24.0) / 2.0;
        let panel_height = 330.0;

        // Animate panel switch highlight (decay over time)
        if self.panel_switch_anim > 0.0 {
            self.panel_switch_anim =
                (self.panel_switch_anim - ctx.input(|i| i.stable_dt) * 3.0).max(0.0);
            ctx.request_repaint();
        }

        let anim_glow = self.panel_switch_anim * 0.3;

        // Side-by-side panels
        ui.horizontal(|ui| {
            // Theme panel (left)
            let theme_focused = self.focused_panel == StyleTab::Theme;
            ui.allocate_ui(Vec2::new(panel_width, panel_height), |ui| {
                let fill = if theme_focused {
                    bg_elevated.gamma_multiply(0.6 + anim_glow)
                } else {
                    bg_elevated.gamma_multiply(0.5)
                };
                let stroke_width = if theme_focused { 2.0 } else { 1.0 };
                let stroke_color = if theme_focused {
                    accent.gamma_multiply(0.7 + anim_glow)
                } else {
                    style.border
                };

                egui::Frame::new()
                    .fill(fill)
                    .stroke(egui::Stroke::new(stroke_width, stroke_color))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            self.render_theme_panel(
                                ui,
                                panel_width - 24.0,
                                panel_height,
                                accent,
                                text,
                                text_muted,
                                bg_hover,
                                style,
                                result,
                            );
                        });
                    });
            });

            ui.add_space(8.0);

            // Font panel (right)
            let font_focused = self.focused_panel == StyleTab::Font;
            ui.allocate_ui(Vec2::new(panel_width, panel_height), |ui| {
                let fill = if font_focused {
                    bg_elevated.gamma_multiply(0.6 + anim_glow)
                } else {
                    bg_elevated.gamma_multiply(0.5)
                };
                let stroke_width = if font_focused { 2.0 } else { 1.0 };
                let stroke_color = if font_focused {
                    accent.gamma_multiply(0.7 + anim_glow)
                } else {
                    style.border
                };

                egui::Frame::new()
                    .fill(fill)
                    .stroke(egui::Stroke::new(stroke_width, stroke_color))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            self.render_font_panel(
                                ui,
                                panel_width - 24.0,
                                panel_height,
                                accent,
                                text,
                                text_muted,
                                bg_hover,
                                result,
                            );
                        });
                    });
            });
        });
    }

    /// Renders the theme panel (left side of Styling tab).
    #[allow(clippy::too_many_arguments)]
    fn render_theme_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel_width: f32,
        panel_height: f32,
        accent: Color32,
        text: Color32,
        text_muted: Color32,
        bg_hover: Color32,
        style: &OverlayStyle,
        result: &mut SettingsResult,
    ) {
        let is_focused = self.focused_panel == StyleTab::Theme;
        let row_height = 52.0;
        let builtin_count = AppTheme::all().len();
        let theme_count = self.total_theme_count();

        // Panel header
        ui.horizontal(|ui| {
            if is_focused {
                ui.label(RichText::new("●").color(accent).size(8.0));
                ui.add_space(4.0);
            }
            let header_color = if is_focused { accent } else { text };
            let icon_size = if is_focused { 18.0 } else { 16.0 };
            ui.label(
                RichText::new(regular::PALETTE)
                    .color(header_color)
                    .size(icon_size),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("Theme")
                    .color(header_color)
                    .font(typography::proportional(if is_focused {
                        typography::LG
                    } else {
                        typography::MD
                    }))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("({theme_count})"))
                    .color(if is_focused { accent } else { text_muted })
                    .font(typography::monospace(typography::XS)),
            );
        });
        ui.add_space(8.0);

        // Theme list in scroll area
        egui::ScrollArea::vertical()
            .id_salt("settings_theme_scroll")
            .max_height(panel_height - 40.0)
            .auto_shrink([false, false])
            .animated(true)
            .show(ui, |ui| {
                for i in 0..theme_count {
                    // Separator before custom themes section
                    if i == builtin_count && !self.custom_themes.is_empty() {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new("Custom Themes")
                                    .color(text_muted)
                                    .font(typography::proportional(typography::XS)),
                            );
                        });
                        ui.add_space(4.0);
                    }

                    self.render_theme_row(
                        ui,
                        panel_width,
                        row_height,
                        i,
                        accent,
                        text,
                        text_muted,
                        bg_hover,
                        style,
                        result,
                        is_focused,
                    );
                }
            });
    }

    /// Renders a single theme entry row.
    #[allow(clippy::too_many_arguments)]
    fn render_theme_row(
        &mut self,
        ui: &mut egui::Ui,
        panel_width: f32,
        row_height: f32,
        index: usize,
        accent: Color32,
        text: Color32,
        text_muted: Color32,
        bg_hover: Color32,
        style: &OverlayStyle,
        result: &mut SettingsResult,
        is_focused: bool,
    ) {
        let is_selected = index == self.theme_index;
        let builtin_count = AppTheme::all().len();

        // Get theme info
        let (display_name, preview_colors, chart_palette, is_custom) = if index < builtin_count {
            let theme = AppTheme::all()[index];
            let colors = [
                theme.bg_base(),
                theme.bg_elevated(),
                theme.accent_primary(),
                theme.text_primary(),
            ];
            (
                theme.name().to_string(),
                colors,
                Some(theme.chart_palette()),
                false,
            )
        } else {
            let custom_idx = index - builtin_count;
            let (_, display_name, colors) = &self.custom_themes[custom_idx];
            let preview = [
                colors.bg_base,
                colors.bg_elevated,
                colors.accent_primary,
                colors.text_primary,
            ];
            (
                display_name.clone(),
                preview,
                Some(colors.chart_palette),
                true,
            )
        };

        // Check if this is the original theme
        let is_original = if index < builtin_count {
            AppTheme::all()[index] == self.original_theme && self.original_custom_theme.is_none()
        } else {
            let custom_idx = index - builtin_count;
            self.original_custom_theme.as_deref() == Some(&self.custom_themes[custom_idx].0)
        };

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(panel_width, row_height), egui::Sense::click());

        // Scroll into view when navigating
        if is_selected && self.scroll_to_theme {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
            self.scroll_to_theme = false;
        }

        // Handle click — select and preview (don't close)
        if response.clicked() {
            self.theme_index = index;
            self.focused_panel = StyleTab::Theme;
            *result = self.select_theme(index);
        }

        // Hover/selection background
        if is_selected && is_focused {
            ui.painter()
                .rect_filled(rect, 6.0, accent.gamma_multiply(0.20));
            ui.painter().rect_filled(
                egui::Rect::from_min_size(rect.min, Vec2::new(3.0, row_height)),
                2.0,
                accent,
            );
        } else if is_selected {
            ui.painter()
                .rect_filled(rect, 6.0, text.gamma_multiply(0.08));
        } else if response.hovered() {
            ui.painter().rect_filled(rect, 6.0, bg_hover);
        }

        // Color palette bar (4 UI colors)
        let palette_x = rect.min.x + 8.0;
        let palette_y = rect.min.y + 10.0;
        let palette_width = 44.0;
        let palette_height = 14.0;
        let color_width = palette_width / 4.0;

        for (idx, color) in preview_colors.iter().enumerate() {
            let x = palette_x + (idx as f32) * color_width;
            let color_rect = egui::Rect::from_min_size(
                egui::pos2(x, palette_y),
                Vec2::new(color_width, palette_height),
            );
            let rounding = if idx == 0 {
                egui::CornerRadius {
                    nw: 3,
                    sw: 3,
                    ne: 0,
                    se: 0,
                }
            } else if idx == 3 {
                egui::CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: 3,
                    se: 3,
                }
            } else {
                egui::CornerRadius::ZERO
            };
            ui.painter().rect_filled(color_rect, rounding, *color);
        }

        // Border around palette
        let palette_rect = egui::Rect::from_min_size(
            egui::pos2(palette_x, palette_y),
            Vec2::new(palette_width, palette_height),
        );
        ui.painter().rect_stroke(
            palette_rect,
            3.0,
            egui::Stroke::new(1.0, style.border),
            egui::StrokeKind::Inside,
        );

        // Theme name
        let name_x = palette_x + palette_width + 10.0;
        ui.painter().text(
            egui::pos2(name_x, rect.min.y + 16.0),
            egui::Align2::LEFT_CENTER,
            &display_name,
            typography::proportional(typography::SM),
            if is_selected && is_focused {
                accent
            } else {
                text
            },
        );

        // Chart palette dots
        let dot_size = 5.0;
        let dot_spacing = 7.0;
        let dots_y = rect.min.y + 34.0;

        if let Some(chart_colors) = chart_palette {
            for (idx, color) in chart_colors.iter().enumerate() {
                let dot_x = palette_x + (idx as f32) * dot_spacing;
                let dot_center = egui::pos2(dot_x + dot_size / 2.0, dots_y);
                ui.painter()
                    .circle_filled(dot_center, dot_size / 2.0, *color);
            }

            let label = if is_custom { "plugin" } else { "chart" };
            let label_color = if is_custom {
                accent.gamma_multiply(0.7)
            } else {
                text_muted.gamma_multiply(0.6)
            };
            ui.painter().text(
                egui::pos2(palette_x + 8.0 * dot_spacing + 4.0, dots_y),
                egui::Align2::LEFT_CENTER,
                label,
                typography::monospace(9.0),
                label_color,
            );
        }

        // "current" indicator for original theme
        if is_original {
            ui.painter().text(
                egui::pos2(rect.max.x - 6.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "●",
                typography::monospace(typography::SM),
                accent,
            );
        }
    }

    /// Renders the font panel (right side of Styling tab).
    #[allow(clippy::too_many_arguments)]
    fn render_font_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel_width: f32,
        panel_height: f32,
        accent: Color32,
        text: Color32,
        text_muted: Color32,
        bg_hover: Color32,
        result: &mut SettingsResult,
    ) {
        let is_focused = self.focused_panel == StyleTab::Font;
        let fonts = EditorFont::all();
        let font_count = fonts.len();
        let row_height = 72.0;

        // Panel header
        ui.horizontal(|ui| {
            if is_focused {
                ui.label(RichText::new("●").color(accent).size(8.0));
                ui.add_space(4.0);
            }
            let header_color = if is_focused { accent } else { text };
            let icon_size = if is_focused { 18.0 } else { 16.0 };
            ui.label(
                RichText::new(regular::TEXT_BOX)
                    .color(header_color)
                    .size(icon_size),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("Font")
                    .color(header_color)
                    .font(typography::proportional(if is_focused {
                        typography::LG
                    } else {
                        typography::MD
                    }))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("({font_count})"))
                    .color(if is_focused { accent } else { text_muted })
                    .font(typography::monospace(typography::XS)),
            );
        });
        ui.add_space(8.0);

        // Font list in scroll area
        egui::ScrollArea::vertical()
            .id_salt("settings_font_scroll")
            .max_height(panel_height - 40.0)
            .auto_shrink([false, false])
            .animated(true)
            .show(ui, |ui| {
                for (i, font) in fonts.iter().enumerate() {
                    let is_selected = i == self.font_index;
                    let is_original_font = *font == self.original_font;

                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(panel_width, row_height),
                        egui::Sense::click(),
                    );

                    // Scroll into view when navigating
                    if is_selected && self.scroll_to_font {
                        ui.scroll_to_rect(rect, Some(egui::Align::Center));
                        self.scroll_to_font = false;
                    }

                    // Handle click — select and preview (don't close)
                    if response.clicked() {
                        self.font_index = i;
                        self.focused_panel = StyleTab::Font;
                        *result = self.select_font(i);
                    }

                    // Hover/selection background
                    if is_selected && is_focused {
                        ui.painter()
                            .rect_filled(rect, 6.0, accent.gamma_multiply(0.20));
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(rect.min, Vec2::new(3.0, row_height)),
                            2.0,
                            accent,
                        );
                    } else if is_selected {
                        ui.painter()
                            .rect_filled(rect, 6.0, text.gamma_multiply(0.08));
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, 6.0, bg_hover);
                    }

                    // Font name
                    let text_x = rect.min.x + 10.0;
                    let top_y = rect.min.y + 14.0;
                    ui.painter().text(
                        egui::pos2(text_x, top_y),
                        egui::Align2::LEFT_CENTER,
                        font.name(),
                        typography::proportional(typography::MD),
                        if is_selected && is_focused {
                            accent
                        } else {
                            text
                        },
                    );

                    // "current" indicator
                    if is_original_font {
                        ui.painter().text(
                            egui::pos2(rect.max.x - 6.0, top_y),
                            egui::Align2::RIGHT_CENTER,
                            "●",
                            typography::monospace(typography::SM),
                            accent,
                        );
                    }

                    // Font description
                    ui.painter().text(
                        egui::pos2(text_x, top_y + 16.0),
                        egui::Align2::LEFT_CENTER,
                        font.description(),
                        typography::proportional(typography::XS),
                        text_muted,
                    );

                    // Font preview — syntax-highlighted Rust code in the actual font
                    let preview_y = top_y + 34.0;
                    let preview_rect = egui::Rect::from_min_size(
                        egui::pos2(text_x, preview_y - 6.0),
                        Vec2::new(panel_width - 20.0, 20.0),
                    );
                    ui.painter()
                        .rect_filled(preview_rect, 4.0, text.gamma_multiply(0.05));

                    let font_family = FontFamily::Name(font.font_family_name().into());
                    let preview_font = FontId::new(typography::SM, font_family);

                    let x = text_x + 8.0;
                    let y = preview_y + 4.0;

                    let draw_token =
                        |ui: &egui::Ui, x: f32, text_str: &str, color: Color32| -> f32 {
                            let galley = ui.painter().layout_no_wrap(
                                text_str.to_string(),
                                preview_font.clone(),
                                color,
                            );
                            ui.painter()
                                .galley(egui::pos2(x, y - 6.0), galley.clone(), color);
                            x + galley.rect.width()
                        };

                    let keyword_color = accent;
                    let normal_color = text.gamma_multiply(0.7);
                    let type_color = accent.gamma_multiply(0.8);
                    let number_color = text.gamma_multiply(0.9);

                    let x = draw_token(ui, x, "let ", keyword_color);
                    let x = draw_token(ui, x, "x", normal_color);
                    let x = draw_token(ui, x, ": ", normal_color);
                    let x = draw_token(ui, x, "Option", type_color);
                    let x = draw_token(ui, x, "<", normal_color);
                    let x = draw_token(ui, x, "i32", type_color);
                    let x = draw_token(ui, x, "> = ", normal_color);
                    let x = draw_token(ui, x, "Some", type_color);
                    let x = draw_token(ui, x, "(", normal_color);
                    let x = draw_token(ui, x, "42", number_color);
                    let _ = draw_token(ui, x, ");", normal_color);
                }
            });
    }

    /// Render the Defaults tab — default connection and codebase settings for new workspaces.
    fn show_defaults_tab(
        &mut self,
        ui: &mut egui::Ui,
        accent: egui::Color32,
        text_primary: egui::Color32,
        _text_secondary: egui::Color32,
        text_tertiary: egui::Color32,
    ) {
        let card_bg = self.theme.bg_elevated().gamma_multiply(0.3);
        let card_border = self.theme.border_subtle().gamma_multiply(0.4);

        // Helper text
        ui.label(
            RichText::new("Pre-filled when creating new workspaces")
                .color(text_tertiary.gamma_multiply(0.4))
                .font(typography::proportional(typography::XS)),
        );
        ui.add_space(6.0);

        // Scrollable content area for all defaults sections
        egui::ScrollArea::vertical()
            .id_salt("settings_defaults_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Track focused field index for scroll-to
                let scroll_target = self.scroll_to_defaults;
                let focused = self.field_index;
                self.scroll_to_defaults = false;

                // Prometheus section
                Self::show_section_header(ui, "Prometheus", text_tertiary);

                let prom_resp = egui::Frame::new()
                    .fill(card_bg)
                    .stroke(egui::Stroke::new(1.0, card_border))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        let prom_endpoint_hint = "https://prometheus.example.com/api/v1";
                        if self.editing_field == Some(EditingField::PrometheusEndpoint) {
                            self.show_text_edit_row(
                                ui,
                                "Endpoint",
                                EditingField::PrometheusEndpoint,
                                prom_endpoint_hint,
                                accent,
                            );
                        } else {
                            self.show_field_row(
                                ui,
                                0,
                                "Endpoint",
                                &self.default_prometheus_endpoint.clone(),
                                prom_endpoint_hint,
                                accent,
                                text_primary,
                                text_tertiary,
                            );
                        }
                    });

                if scroll_target && focused == 0 {
                    ui.scroll_to_rect(prom_resp.response.rect, Some(egui::Align::Center));
                }

                ui.add_space(10.0);

                // Loki section
                Self::show_section_header(ui, "Loki", text_tertiary);

                let loki_resp = egui::Frame::new()
                    .fill(card_bg)
                    .stroke(egui::Stroke::new(1.0, card_border))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        let loki_endpoint_hint = "https://loki.example.com/loki/api/v1";
                        if self.editing_field == Some(EditingField::LokiEndpoint) {
                            self.show_text_edit_row(
                                ui,
                                "Endpoint",
                                EditingField::LokiEndpoint,
                                loki_endpoint_hint,
                                accent,
                            );
                        } else {
                            self.show_field_row(
                                ui,
                                1,
                                "Endpoint",
                                &self.default_loki_endpoint.clone(),
                                loki_endpoint_hint,
                                accent,
                                text_primary,
                                text_tertiary,
                            );
                        }
                    });

                if scroll_target && focused == 1 {
                    ui.scroll_to_rect(loki_resp.response.rect, Some(egui::Align::Center));
                }

                ui.add_space(10.0);

                // Tempo section
                Self::show_section_header(ui, "Tempo", text_tertiary);

                let tempo_resp = egui::Frame::new()
                    .fill(card_bg)
                    .stroke(egui::Stroke::new(1.0, card_border))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        let tempo_endpoint_hint = "https://tempo.example.com";
                        if self.editing_field == Some(EditingField::TempoEndpoint) {
                            self.show_text_edit_row(
                                ui,
                                "Endpoint",
                                EditingField::TempoEndpoint,
                                tempo_endpoint_hint,
                                accent,
                            );
                        } else {
                            self.show_field_row(
                                ui,
                                2,
                                "Endpoint",
                                &self.default_tempo_endpoint.clone(),
                                tempo_endpoint_hint,
                                accent,
                                text_primary,
                                text_tertiary,
                            );
                        }
                    });

                if scroll_target && focused == 2 {
                    ui.scroll_to_rect(tempo_resp.response.rect, Some(egui::Align::Center));
                }

                ui.add_space(10.0);

                // Codebase section
                Self::show_section_header(ui, "Codebase", text_tertiary);

                let code_resp = egui::Frame::new()
                    .fill(card_bg)
                    .stroke(egui::Stroke::new(1.0, card_border))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        let git_hint = "https://github.com/org/repo";
                        if self.editing_field == Some(EditingField::GitRepoUrl) {
                            self.show_text_edit_row(
                                ui,
                                "Git URL",
                                EditingField::GitRepoUrl,
                                git_hint,
                                accent,
                            );
                        } else {
                            self.show_field_row(
                                ui,
                                3,
                                "Git URL",
                                &self.git_repo_url.clone(),
                                git_hint,
                                accent,
                                text_primary,
                                text_tertiary,
                            );
                        }
                    });

                if scroll_target && focused == 3 {
                    ui.scroll_to_rect(code_resp.response.rect, Some(egui::Align::Center));
                }
            }); // end ScrollArea
    }

    /// Render a section header label (uppercase, muted).
    fn show_section_header(ui: &mut egui::Ui, label: &str, text_tertiary: egui::Color32) {
        ui.label(
            RichText::new(label.to_uppercase())
                .color(text_tertiary.gamma_multiply(0.6))
                .font(typography::proportional(typography::XS))
                .strong(),
        );
        ui.add_space(6.0);
    }

    /// Render a thin divider line between fields within a card.
    fn show_field_divider(ui: &mut egui::Ui, separator: egui::Color32) {
        ui.add_space(8.0);
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator.gamma_multiply(0.3)),
        );
        ui.add_space(8.0);
    }

    /// Show a read-only field row with label-above-input card style.
    #[allow(clippy::too_many_arguments)]
    fn show_field_row(
        &self,
        ui: &mut egui::Ui,
        index: usize,
        label: &str,
        value: &str,
        placeholder: &str,
        accent: egui::Color32,
        text_primary: egui::Color32,
        text_tertiary: egui::Color32,
    ) {
        let is_focused = self.field_index == index && self.editing_field.is_none();
        let input_height = 30.0;

        // Label with status dot
        ui.horizontal(|ui| {
            // Status dot: green if value is set, muted if empty
            let is_empty = value.is_empty();
            let dot_color = if is_empty {
                text_tertiary.gamma_multiply(0.2)
            } else {
                Color32::from_rgb(74, 222, 128).gamma_multiply(0.7) // green-400
            };
            let dot_rect = ui.allocate_exact_size(Vec2::new(6.0, 6.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(dot_rect.1.rect.center(), 3.0, dot_color);
            ui.add_space(4.0);
            ui.label(
                RichText::new(label)
                    .color(if is_focused {
                        text_primary
                    } else {
                        text_tertiary
                    })
                    .font(typography::proportional(typography::SM)),
            );
        });
        ui.add_space(3.0);

        // Input box
        let avail_width = ui.available_width();
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(avail_width, input_height), egui::Sense::hover());

        let bg = if is_focused {
            self.theme.bg_surface()
        } else {
            self.theme.bg_surface().gamma_multiply(0.5)
        };
        let border_color = if is_focused {
            accent.gamma_multiply(0.6)
        } else {
            self.theme.border_subtle().gamma_multiply(0.4)
        };
        let border_width = if is_focused { 1.5 } else { 1.0 };

        ui.painter().rect(
            rect,
            6.0,
            bg,
            egui::Stroke::new(border_width, border_color),
            egui::StrokeKind::Inside,
        );

        // Value or placeholder hint
        let is_empty = value.is_empty();
        let display_text = if is_empty { placeholder } else { value };
        let text_color = if is_empty {
            text_tertiary.gamma_multiply(0.3)
        } else {
            text_primary.gamma_multiply(if is_focused { 1.0 } else { 0.8 })
        };
        ui.painter().text(
            egui::pos2(rect.min.x + 12.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            display_text,
            typography::monospace(typography::MD),
            text_color,
        );

        // "Enter to edit" hint
        if is_focused {
            ui.painter().text(
                egui::pos2(rect.max.x - 12.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "l:edit",
                typography::monospace(typography::XS),
                accent.gamma_multiply(0.4),
            );
        }
    }

    /// Show an editable text field row with styled input container.
    fn show_text_edit_row(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        _field: EditingField,
        placeholder: &str,
        accent: egui::Color32,
    ) {
        let bg = self.theme.bg_surface();
        let avail_width = ui.available_width();

        // Label (accent-colored when editing)
        ui.label(
            RichText::new(label)
                .color(accent)
                .font(typography::proportional(typography::SM)),
        );
        ui.add_space(3.0);

        let text_to_edit = match self.editing_field {
            Some(EditingField::GitRepoUrl) => &mut self.git_repo_url,
            Some(EditingField::PrometheusEndpoint) => &mut self.default_prometheus_endpoint,
            Some(EditingField::LokiEndpoint) => &mut self.default_loki_endpoint,
            Some(EditingField::TempoEndpoint) => &mut self.default_tempo_endpoint,
            None => return,
        };

        // Styled input container with accent border
        egui::Frame::new()
            .fill(bg)
            .stroke(egui::Stroke::new(1.5, accent.gamma_multiply(0.6)))
            .corner_radius(6.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_width(avail_width - 20.0);

                let edit = egui::TextEdit::singleline(text_to_edit)
                    .desired_width(ui.available_width())
                    .font(typography::monospace(typography::MD))
                    .text_color(accent)
                    .hint_text(
                        RichText::new(placeholder)
                            .font(typography::monospace(typography::MD))
                            .color(accent.gamma_multiply(0.25)),
                    )
                    .frame(false);

                let response = ui.add(edit);

                if !response.has_focus() {
                    response.request_focus();
                }
            });
    }

    /// Cycle the currently focused AI dropdown option by delta.
    fn cycle_ai_dropdown(&mut self, delta: i32) {
        match self.field_index {
            0 => {
                // Provider dropdown
                let providers = AiProvider::all();
                let idx = providers
                    .iter()
                    .position(|p| *p == self.ai_provider)
                    .unwrap_or(0);
                let new_idx = ((idx as i32 + delta).rem_euclid(providers.len() as i32)) as usize;
                self.ai_provider = providers[new_idx];
                self.ai_model = None;
            }
            1 => {
                // Model dropdown
                let models = ProviderManifest::models_for(self.ai_provider);
                let current_id = self.ai_model.clone().unwrap_or_else(|| {
                    ProviderManifest::default_model_id_for(self.ai_provider).unwrap_or_default()
                });
                let idx = models.iter().position(|m| m.id == current_id).unwrap_or(0);
                let new_idx = ((idx as i32 + delta).rem_euclid(models.len() as i32)) as usize;
                self.ai_model = Some(models[new_idx].id.clone());
            }
            _ => {}
        }
    }

    /// Handle keyboard navigation and actions.
    fn handle_keyboard(&mut self, ctx: &egui::Context) -> SettingsResult {
        // If we're editing a text field, only handle Escape and Enter
        if self.editing_field.is_some() {
            let mut stop_editing = false;
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    stop_editing = true;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    stop_editing = true;
                }
            });
            if stop_editing {
                self.editing_field = None;
                ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            }
            return SettingsResult::None;
        }

        let mut result = SettingsResult::None;

        ctx.input_mut(|i| {
            // Escape - save and close (auto-save all settings)
            if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                result = self.build_saved();
                return;
            }

            // Tab / Shift+Tab - switch tabs
            if i.consume_key(egui::Modifiers::SHIFT, Key::Tab) {
                self.tab = self.tab.prev();
                self.field_index = 0;
                return;
            }
            if i.consume_key(egui::Modifiers::NONE, Key::Tab) {
                self.tab = self.tab.next();
                self.field_index = 0;
                return;
            }

            // Number keys - jump to tab directly
            let tabs = SettingsTab::all();
            for (idx, tab) in tabs.iter().enumerate() {
                let key = match idx {
                    0 => Key::Num1,
                    1 => Key::Num2,
                    2 => Key::Num3,
                    3 => Key::Num4,
                    _ => continue,
                };
                if i.consume_key(egui::Modifiers::NONE, key) {
                    self.tab = *tab;
                    self.field_index = 0;
                    return;
                }
            }

            // Styling tab uses panel-based navigation (h/l switch panels, j/k navigate within)
            if self.tab == SettingsTab::Styling {
                // h - switch to Theme panel (left)
                if i.consume_key(egui::Modifiers::NONE, Key::H)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft)
                {
                    if self.focused_panel != StyleTab::Theme {
                        self.focused_panel = StyleTab::Theme;
                        self.panel_switch_anim = 1.0;
                    }
                    return;
                }

                // l - switch to Font panel (right)
                if i.consume_key(egui::Modifiers::NONE, Key::L)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowRight)
                {
                    if self.focused_panel != StyleTab::Font {
                        self.focused_panel = StyleTab::Font;
                        self.panel_switch_anim = 1.0;
                    }
                    return;
                }

                // j/Down - navigate down in focused panel
                if i.consume_key(egui::Modifiers::NONE, Key::J)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                {
                    let preview = match self.focused_panel {
                        StyleTab::Theme => self.navigate_theme(1),
                        StyleTab::Font => self.navigate_font(1),
                    };
                    self.pending_styling_result = Some(preview);
                    return;
                }

                // k/Up - navigate up in focused panel
                if i.consume_key(egui::Modifiers::NONE, Key::K)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                {
                    let preview = match self.focused_panel {
                        StyleTab::Theme => self.navigate_theme(-1),
                        StyleTab::Font => self.navigate_font(-1),
                    };
                    self.pending_styling_result = Some(preview);
                    return;
                }

                // Enter does nothing special on Styling tab (live preview already active)
                if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    return;
                }

                return;
            }

            // AI tab with dropdown open: j/k navigate options, Escape/h close dropdown
            if self.tab == SettingsTab::Ai && self.ai_dropdown_open {
                if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    self.ai_dropdown_open = false;
                    return;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::H) {
                    self.ai_dropdown_open = false;
                    return;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::J)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                {
                    self.cycle_ai_dropdown(1);
                    return;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::K)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                {
                    self.cycle_ai_dropdown(-1);
                    return;
                }
                // Enter - select current and close
                if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    self.ai_dropdown_open = false;
                    return;
                }
                return;
            }

            // j/Down - move down
            if i.consume_key(egui::Modifiers::NONE, Key::J)
                || i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
            {
                let count = self.field_count();
                if count > 0 {
                    self.field_index = (self.field_index + 1) % count;
                    self.ai_dropdown_open = false;
                    self.scroll_to_defaults = true;
                }
                return;
            }

            // k/Up - move up
            if i.consume_key(egui::Modifiers::NONE, Key::K)
                || i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
            {
                let count = self.field_count();
                if count > 0 {
                    self.field_index = if self.field_index == 0 {
                        count - 1
                    } else {
                        self.field_index - 1
                    };
                    self.ai_dropdown_open = false;
                    self.scroll_to_defaults = true;
                }
                return;
            }

            // l/Enter - open dropdown (AI tab) or activate text edit (Defaults tab)
            if i.consume_key(egui::Modifiers::NONE, Key::Enter)
                || i.consume_key(egui::Modifiers::NONE, Key::L)
            {
                match self.tab {
                    SettingsTab::Ai => {
                        self.ai_dropdown_open = true;
                    }
                    SettingsTab::Styling => {} // Handled above
                    SettingsTab::Defaults => {
                        let field = match self.field_index {
                            0 => Some(EditingField::PrometheusEndpoint),
                            1 => Some(EditingField::LokiEndpoint),
                            2 => Some(EditingField::TempoEndpoint),
                            3 => Some(EditingField::GitRepoUrl),
                            _ => None,
                        };
                        self.editing_field = field;
                    }
                }
            }
        });

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_overlay_is_closed() {
        let overlay = SettingsOverlay::new();
        assert!(!overlay.is_open());
    }

    #[test]
    fn test_open_close() {
        let mut overlay = SettingsOverlay::new();
        overlay.open(
            AiProvider::Claude,
            None,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            AppTheme::default(),
            None,
            EditorFont::default(),
            Vec::new(),
        );
        assert!(overlay.is_open());
        overlay.close();
        assert!(!overlay.is_open());
    }

    #[test]
    fn test_field_count() {
        let mut overlay = SettingsOverlay::new();
        overlay.tab = SettingsTab::Defaults;
        assert_eq!(overlay.field_count(), 4); // Prom URL, Loki URL, Tempo URL, Git URL
        overlay.tab = SettingsTab::Ai;
        assert_eq!(overlay.field_count(), 2); // Provider, Model
        overlay.tab = SettingsTab::Styling;
        assert_eq!(overlay.field_count(), 0); // Panel-based
    }

    #[test]
    fn test_tab_cycling() {
        let tabs = SettingsTab::all();
        assert!(!tabs.is_empty());

        let ai = SettingsTab::Ai;
        let next = ai.next();
        if tabs.len() > 1 {
            assert_ne!(next, ai);
        }
    }
}
