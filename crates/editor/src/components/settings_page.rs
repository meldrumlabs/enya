//! Full-page settings experience with sidebar navigation.
//!
//! Replaces the modal settings overlay with a full-page layout featuring
//! a left sidebar for category navigation and a spacious content area.

use egui::{Color32, FontFamily, FontId, Key, RichText, Vec2};
use egui_nerdfonts::regular;

use crate::components::overlay::style_picker::StyleTab;
use crate::components::util::{AiModel, AiProvider};
use crate::github_auth::AuthState;
use crate::ui::ActiveThemeColors;
use crate::ui::semantic_icons;
use crate::ui::settings_screen::EditorFont;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Result returned by the settings page each frame.
#[derive(Debug, Clone)]
pub enum SettingsPageResult {
    /// No action this frame.
    None,
    /// Navigate back to previous view (save on exit).
    GoBack,
    /// Settings were saved (on close).
    Saved {
        ai_provider: AiProvider,
        ai_model: Option<AiModel>,
        git_repo_url: String,
        default_prometheus_endpoint: String,
        default_loki_endpoint: String,
        default_flight_sql_endpoint: String,
    },
    /// Live preview of a builtin theme change.
    ThemePreview(AppTheme),
    /// Live preview of a custom theme change.
    CustomThemePreview(String),
    /// Live preview of a font change.
    FontPreview(EditorFont),
    /// Cancelled with restore of original theme/font (from Editor section).
    CancelledWithRestore {
        theme: AppTheme,
        custom_theme: Option<String>,
        font: EditorFont,
    },
    /// User clicked "Sign in with GitHub".
    GitHubSignIn,
    /// User clicked "Sign out".
    GitHubSignOut,
}

/// Settings category for sidebar navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Auth,
    Connections,
    Ai,
    ThemeFont,
}

impl SettingsCategory {
    fn all() -> &'static [Self] {
        &[Self::Auth, Self::Connections, Self::Ai, Self::ThemeFont]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Auth => "Auth",
            Self::Connections => "Connections",
            Self::Ai => "AI",
            Self::ThemeFont => "Theme & Font",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Auth => regular::MARK_GITHUB,
            Self::Connections => regular::CONNECTION,
            Self::Ai => regular::SPARKLE_FILL,
            Self::ThemeFont => regular::PALETTE,
        }
    }

    fn group_label(self) -> &'static str {
        match self {
            Self::Auth => "ACCOUNT",
            Self::Connections | Self::Ai => "CONFIGURATION",
            Self::ThemeFont => "EDITOR",
        }
    }
}

/// Which field is being text-edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditingField {
    GitRepoUrl,
    PrometheusEndpoint,
    LokiEndpoint,
    FlightSqlEndpoint,
}

/// Full-page settings component.
pub struct SettingsPage {
    is_open: bool,
    theme: AppTheme,
    // Sidebar state
    active_category: SettingsCategory,
    sidebar_focused: bool,
    // Per-category field state
    field_index: usize,
    editing_field: Option<EditingField>,
    // Working copies of settings
    ai_provider: AiProvider,
    ai_model: Option<AiModel>,
    ai_dropdown_open: bool,
    git_repo_url: String,
    default_prometheus_endpoint: String,
    default_loki_endpoint: String,
    default_flight_sql_endpoint: String,
    // Appearance state
    styling_theme: AppTheme,
    original_theme: AppTheme,
    styling_custom_theme: Option<String>,
    original_custom_theme: Option<String>,
    styling_font: EditorFont,
    original_font: EditorFont,
    custom_themes: Vec<(String, String, ActiveThemeColors)>,
    theme_index: usize,
    font_index: usize,
    focused_panel: StyleTab,
    panel_switch_anim: f32,
    scroll_to_theme: bool,
    scroll_to_font: bool,
    scroll_to_defaults: bool,
    // Pending result from appearance navigation (emitted next frame)
    pending_styling_result: Option<SettingsPageResult>,
    // GitHub auth state (passed in from EnyaApp each frame)
    github_auth_state: AuthState,
    // Cached avatar texture (created from bytes on first render)
    avatar_texture: Option<egui::TextureHandle>,
    // Whether keyboard Enter was pressed on the auth action button
    pending_account_action: bool,
}

impl Default for SettingsPage {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPage {
    pub fn new() -> Self {
        Self {
            is_open: false,
            theme: AppTheme::default(),
            active_category: SettingsCategory::Connections,
            sidebar_focused: false,
            field_index: 0,
            editing_field: None,
            ai_provider: AiProvider::default(),
            ai_model: None,
            ai_dropdown_open: false,
            git_repo_url: String::new(),
            default_prometheus_endpoint: String::new(),
            default_loki_endpoint: String::new(),
            default_flight_sql_endpoint: String::new(),
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
            github_auth_state: AuthState::SignedOut,
            avatar_texture: None,
            pending_account_action: false,
        }
    }

    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    pub fn set_github_auth_state(
        &mut self,
        state: AuthState,
        avatar_bytes: Option<&[u8]>,
        ctx: &egui::Context,
    ) {
        // If we have new avatar bytes and no texture yet, create the texture
        if let (Some(bytes), None) = (avatar_bytes, &self.avatar_texture) {
            if let Ok(dynamic_image) = image::load_from_memory(bytes) {
                let rgba = dynamic_image.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels: Vec<egui::Color32> = rgba
                    .pixels()
                    .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                    .collect();
                let color_image = egui::ColorImage::new(size, pixels);
                self.avatar_texture = Some(ctx.load_texture(
                    "github_avatar",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }

        // Clear texture on sign out
        if matches!(state, AuthState::SignedOut) {
            self.avatar_texture = None;
        }

        self.github_auth_state = state;
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Set the active settings category (e.g. to navigate to Auth after OAuth callback).
    pub fn set_active_category(&mut self, category: SettingsCategory) {
        self.active_category = category;
    }

    /// Open the settings page with current values.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        &mut self,
        ai_provider: AiProvider,
        ai_model: Option<AiModel>,
        git_repo_url: String,
        default_prometheus_endpoint: String,
        default_loki_endpoint: String,
        default_flight_sql_endpoint: String,
        current_theme: AppTheme,
        current_custom_theme: Option<String>,
        current_font: EditorFont,
        custom_themes: Vec<(String, String, ActiveThemeColors)>,
        github_auth_state: AuthState,
    ) {
        self.is_open = true;
        self.active_category = SettingsCategory::Auth;
        self.github_auth_state = github_auth_state;
        self.pending_account_action = false;
        self.sidebar_focused = false;
        self.field_index = 0;
        self.editing_field = None;
        self.ai_provider = ai_provider;
        self.ai_model = ai_model;
        self.git_repo_url = git_repo_url;
        self.ai_dropdown_open = false;
        self.default_prometheus_endpoint = default_prometheus_endpoint;
        self.default_loki_endpoint = default_loki_endpoint;
        self.default_flight_sql_endpoint = default_flight_sql_endpoint;
        // Appearance
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

    /// Number of navigable fields in the current category.
    fn field_count(&self) -> usize {
        match self.active_category {
            SettingsCategory::Auth => 1,        // Sign in/out button
            SettingsCategory::Connections => 4, // Prom, Loki, Flight SQL, Git URL
            SettingsCategory::Ai => 2,          // Provider, Model
            SettingsCategory::ThemeFont => 0,   // Panel-based navigation
        }
    }

    /// Total number of themes (builtin + custom).
    fn total_theme_count(&self) -> usize {
        AppTheme::all().len() + self.custom_themes.len()
    }

    /// Navigate theme index by delta, return preview result.
    fn navigate_theme(&mut self, delta: i32) -> SettingsPageResult {
        let total = self.total_theme_count();
        if total == 0 {
            return SettingsPageResult::None;
        }
        let new_idx = ((self.theme_index as i32 + delta).rem_euclid(total as i32)) as usize;
        self.theme_index = new_idx;
        self.scroll_to_theme = true;
        let builtin_count = AppTheme::all().len();
        if new_idx < builtin_count {
            self.styling_theme = AppTheme::all()[new_idx];
            self.styling_custom_theme = None;
            SettingsPageResult::ThemePreview(self.styling_theme)
        } else {
            let custom_idx = new_idx - builtin_count;
            let name = self.custom_themes[custom_idx].0.clone();
            self.styling_custom_theme = Some(name.clone());
            SettingsPageResult::CustomThemePreview(name)
        }
    }

    /// Navigate font index by delta, return preview result.
    fn navigate_font(&mut self, delta: i32) -> SettingsPageResult {
        let fonts = EditorFont::all();
        let total = fonts.len();
        if total == 0 {
            return SettingsPageResult::None;
        }
        let new_idx = ((self.font_index as i32 + delta).rem_euclid(total as i32)) as usize;
        self.font_index = new_idx;
        self.scroll_to_font = true;
        self.styling_font = fonts[new_idx];
        SettingsPageResult::FontPreview(self.styling_font)
    }

    /// Select a theme by index (e.g. from click).
    fn select_theme(&mut self, index: usize) -> SettingsPageResult {
        let builtin_count = AppTheme::all().len();
        self.theme_index = index;
        if index < builtin_count {
            self.styling_theme = AppTheme::all()[index];
            self.styling_custom_theme = None;
            SettingsPageResult::ThemePreview(self.styling_theme)
        } else {
            let custom_idx = index - builtin_count;
            let name = self.custom_themes[custom_idx].0.clone();
            self.styling_custom_theme = Some(name.clone());
            SettingsPageResult::CustomThemePreview(name)
        }
    }

    /// Select a font by index (e.g. from click).
    fn select_font(&mut self, index: usize) -> SettingsPageResult {
        let fonts = EditorFont::all();
        self.font_index = index;
        self.styling_font = fonts[index];
        SettingsPageResult::FontPreview(self.styling_font)
    }

    /// Build the saved result with all current values.
    fn build_saved(&self) -> SettingsPageResult {
        SettingsPageResult::Saved {
            ai_provider: self.ai_provider,
            ai_model: self.ai_model,
            git_repo_url: self.git_repo_url.clone(),
            default_prometheus_endpoint: self.default_prometheus_endpoint.clone(),
            default_loki_endpoint: self.default_loki_endpoint.clone(),
            default_flight_sql_endpoint: self.default_flight_sql_endpoint.clone(),
        }
    }

    /// Show the full-page settings. Returns a result each frame.
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> SettingsPageResult {
        let mut result = SettingsPageResult::None;

        // Emit pending styling result from previous frame
        if let Some(pending) = self.pending_styling_result.take() {
            result = pending;
        }

        // Handle keyboard input
        let kb_result = self.handle_keyboard(ctx);
        if !matches!(kb_result, SettingsPageResult::None) {
            result = kb_result;
        }

        let accent = self.theme.accent_primary();
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();
        let bg_base = self.theme.bg_base();
        let bg_surface = self.theme.bg_surface();
        let separator = self.theme.border_subtle();

        // Full-page layout with sidebar + content
        let sidebar_width = 220.0;

        // Sidebar panel
        egui::SidePanel::left("settings_sidebar")
            .exact_width(sidebar_width)
            .frame(
                egui::Frame::new()
                    .fill(bg_surface.gamma_multiply(0.5))
                    .inner_margin(egui::Margin::symmetric(0, 0))
                    .stroke(egui::Stroke::new(1.0, separator.gamma_multiply(0.3))),
            )
            .show_inside(ui, |ui| {
                self.render_sidebar(
                    ui,
                    accent,
                    text_primary,
                    text_secondary,
                    text_tertiary,
                    bg_base,
                );
            });

        // Content area
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(bg_base)
                    .inner_margin(egui::Margin::symmetric(0, 0)),
            )
            .show_inside(ui, |ui| {
                self.render_content(
                    ui,
                    ctx,
                    accent,
                    text_primary,
                    text_secondary,
                    text_tertiary,
                    &mut result,
                );
            });

        // Handle close
        if matches!(
            result,
            SettingsPageResult::GoBack
                | SettingsPageResult::Saved { .. }
                | SettingsPageResult::CancelledWithRestore { .. }
        ) {
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
            self.close();
        }

        result
    }

    /// Render the sidebar with category navigation.
    fn render_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        accent: Color32,
        text_primary: Color32,
        _text_secondary: Color32,
        text_tertiary: Color32,
        _bg_base: Color32,
    ) {
        ui.add_space(24.0);

        // Header
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.label(
                RichText::new("Settings")
                    .color(text_primary)
                    .size(16.0)
                    .strong(),
            );
        });

        ui.add_space(24.0);

        // Category groups
        let categories = SettingsCategory::all();
        let mut last_group = "";

        for (i, cat) in categories.iter().enumerate() {
            let group = cat.group_label();
            if group != last_group {
                if !last_group.is_empty() {
                    ui.add_space(16.0);
                }
                // Group label
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new(group)
                            .color(text_tertiary.gamma_multiply(0.5))
                            .font(typography::proportional(9.0))
                            .strong(),
                    );
                });
                ui.add_space(6.0);
                last_group = group;
            }

            let is_active = self.active_category == *cat;
            let is_sidebar_focused = self.sidebar_focused;

            // Category row
            let row_height = 34.0;
            let avail_width = ui.available_width();
            let margin = 12.0;
            let pill_width = avail_width - margin * 2.0;

            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(avail_width, row_height), egui::Sense::click());

            let pill_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + margin, rect.min.y),
                Vec2::new(pill_width, row_height),
            );

            // Active pill background
            if is_active {
                let pill_alpha = if is_sidebar_focused { 0.15 } else { 0.10 };
                ui.painter()
                    .rect_filled(pill_rect, 8.0, accent.gamma_multiply(pill_alpha));

                // Left accent strip when sidebar focused
                if is_sidebar_focused {
                    let strip =
                        egui::Rect::from_min_size(pill_rect.min, Vec2::new(3.0, row_height));
                    ui.painter().rect_filled(strip, 2.0, accent);
                }
            } else if response.hovered() {
                ui.painter()
                    .rect_filled(pill_rect, 8.0, text_primary.gamma_multiply(0.04));
            }

            // Icon
            let icon_color = if is_active { accent } else { text_tertiary };
            ui.painter().text(
                egui::pos2(pill_rect.min.x + 14.0, pill_rect.center().y),
                egui::Align2::LEFT_CENTER,
                cat.icon(),
                FontId::new(14.0, FontFamily::Proportional),
                icon_color,
            );

            // Label
            let label_color = if is_active {
                text_primary
            } else {
                text_tertiary
            };
            ui.painter().text(
                egui::pos2(pill_rect.min.x + 36.0, pill_rect.center().y),
                egui::Align2::LEFT_CENTER,
                cat.label(),
                typography::proportional(typography::SM),
                label_color,
            );

            // Number hint
            let num_color = if is_active {
                accent.gamma_multiply(0.4)
            } else {
                text_tertiary.gamma_multiply(0.3)
            };
            ui.painter().text(
                egui::pos2(pill_rect.max.x - 14.0, pill_rect.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("{}", i + 1),
                typography::monospace(typography::XS),
                num_color,
            );

            if response.clicked() {
                self.active_category = *cat;
                self.field_index = 0;
                self.editing_field = None;
                self.ai_dropdown_open = false;
                self.sidebar_focused = false;
            }

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        // Footer keyboard hint
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("esc")
                        .color(text_tertiary.gamma_multiply(0.35))
                        .font(typography::monospace(9.0)),
                );
                ui.label(
                    RichText::new("save & close")
                        .color(text_tertiary.gamma_multiply(0.25))
                        .font(typography::monospace(9.0)),
                );
            });
        });
    }

    /// Render the content area for the active category.
    #[allow(clippy::too_many_arguments)]
    fn render_content(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        accent: Color32,
        text_primary: Color32,
        text_secondary: Color32,
        text_tertiary: Color32,
        result: &mut SettingsPageResult,
    ) {
        // Theme & Font needs more width for side-by-side panels
        let max_content_width = match self.active_category {
            SettingsCategory::ThemeFont => 800.0,
            _ => 640.0,
        };

        // Center the content horizontally while preserving full vertical space
        let available = ui.available_rect_before_wrap();
        let side_margin = ((available.width() - max_content_width) / 2.0).max(32.0);
        let content_rect = egui::Rect::from_min_max(
            egui::pos2(available.min.x + side_margin, available.min.y),
            egui::pos2(
                (available.min.x + side_margin + max_content_width)
                    .min(available.max.x - side_margin),
                available.max.y,
            ),
        );

        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
        {
            let ui = &mut ui;
            ui.add_space(48.0);

            // Section title with back button
            let mut back_clicked = false;
            ui.horizontal(|ui| {
                // Back arrow
                let back_resp = ui.add(
                    egui::Label::new(
                        RichText::new(semantic_icons::nav::BACK)
                            .color(text_tertiary.gamma_multiply(0.5))
                            .size(16.0),
                    )
                    .sense(egui::Sense::click()),
                );
                if back_resp.clicked() {
                    back_clicked = true;
                }
                if back_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                ui.add_space(8.0);
                ui.label(
                    RichText::new(self.active_category.icon())
                        .color(accent)
                        .size(22.0),
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new(self.active_category.label())
                        .color(text_primary)
                        .size(20.0)
                        .strong(),
                );
            });
            if back_clicked {
                *result = self.build_saved();
            }

            // Section subtitle
            ui.add_space(8.0);
            let subtitle = match self.active_category {
                SettingsCategory::Auth => "Sign in to sync and share your work",
                SettingsCategory::Connections => "Default endpoints and data source configuration",
                SettingsCategory::Ai => "AI provider and model configuration",
                SettingsCategory::ThemeFont => "Choose your color scheme and editor typeface",
            };
            ui.label(
                RichText::new(subtitle)
                    .color(text_tertiary.gamma_multiply(0.6))
                    .font(typography::proportional(typography::SM)),
            );

            ui.add_space(28.0);

            // Separator
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, self.theme.border_subtle().gamma_multiply(0.3)),
            );
            ui.add_space(28.0);

            // Content sections — Theme & Font has its own inner scroll areas,
            // so only wrap Auth/Connections/AI in an outer scroll.
            match self.active_category {
                SettingsCategory::Auth | SettingsCategory::Connections | SettingsCategory::Ai => {
                    egui::ScrollArea::vertical()
                        .id_salt("settings_content_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            match self.active_category {
                                SettingsCategory::Auth => {
                                    self.show_auth_section(
                                        ui,
                                        ctx,
                                        accent,
                                        text_primary,
                                        text_tertiary,
                                        result,
                                    );
                                }
                                SettingsCategory::Connections => {
                                    self.show_general_section(
                                        ui,
                                        accent,
                                        text_primary,
                                        text_secondary,
                                        text_tertiary,
                                    );
                                }
                                SettingsCategory::Ai => {
                                    self.show_ai_section(
                                        ui,
                                        accent,
                                        text_primary,
                                        text_secondary,
                                        text_tertiary,
                                    );
                                }
                                _ => unreachable!(),
                            }
                            ui.add_space(64.0);
                        });
                }
                SettingsCategory::ThemeFont => {
                    self.show_appearance_section(
                        ui,
                        ctx,
                        accent,
                        text_primary,
                        text_tertiary,
                        result,
                    );
                }
            }
        }
    }

    // ── Auth Section ──────────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn show_auth_section(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        accent: Color32,
        text_primary: Color32,
        text_tertiary: Color32,
        result: &mut SettingsPageResult,
    ) {
        let card_bg = self.theme.bg_elevated().gamma_multiply(0.55);
        let card_border = self.theme.border_subtle().gamma_multiply(0.6);
        let is_focused = self.field_index == 0 && !self.sidebar_focused;

        // Check for keyboard-triggered action
        let kb_action = self.pending_account_action;
        self.pending_account_action = false;

        match &self.github_auth_state {
            AuthState::SignedOut | AuthState::Error(_) => {
                let error_msg = if let AuthState::Error(msg) = &self.github_auth_state {
                    Some(msg.clone())
                } else {
                    None
                };

                let stroke_color = if is_focused {
                    accent.gamma_multiply(0.5)
                } else {
                    card_border
                };

                egui::Frame::new()
                    .fill(card_bg)
                    .stroke(egui::Stroke::new(
                        if is_focused { 1.5 } else { 1.0 },
                        stroke_color,
                    ))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin {
                        left: 24,
                        right: 24,
                        top: 28,
                        bottom: 28,
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        // GitHub icon
                        ui.label(
                            RichText::new(regular::MARK_GITHUB)
                                .color(accent)
                                .size(28.0),
                        );

                        ui.add_space(16.0);

                        // Title
                        ui.label(
                            RichText::new("Sign in with GitHub")
                                .color(text_primary)
                                .size(16.0)
                                .strong(),
                        );

                        ui.add_space(8.0);

                        // Description
                        ui.label(
                            RichText::new(
                                "Connect your GitHub account to share snapshots\nand unlock premium features.",
                            )
                            .color(text_tertiary.gamma_multiply(0.6))
                            .font(typography::proportional(typography::SM)),
                        );

                        // Error message
                        if let Some(ref msg) = error_msg {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(msg)
                                    .color(self.theme.semantic_error())
                                    .font(typography::proportional(typography::SM)),
                            );
                        }

                        ui.add_space(20.0);

                        // Sign in button
                        let btn_height = 36.0;
                        let btn_width = 220.0;
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(btn_width, btn_height),
                            egui::Sense::click(),
                        );

                        let btn_fill = if response.hovered() {
                            accent.gamma_multiply(1.15)
                        } else {
                            accent
                        };
                        ui.painter()
                            .rect_filled(rect, 6.0, btn_fill);

                        // Button label
                        let btn_text = format!("{} Sign in with GitHub  \u{2192}", regular::MARK_GITHUB);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            btn_text,
                            typography::proportional(typography::SM),
                            Color32::WHITE,
                        );

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        if response.clicked() || kb_action {
                            *result = SettingsPageResult::GitHubSignIn;
                        }

                        // Keyboard hint when focused
                        if is_focused {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new("enter to sign in")
                                    .color(accent.gamma_multiply(0.35))
                                    .font(typography::monospace(9.0)),
                            );
                        }
                    });
            }

            AuthState::Authenticating => {
                egui::Frame::new()
                    .fill(card_bg)
                    .stroke(egui::Stroke::new(1.0, card_border))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin {
                        left: 24,
                        right: 24,
                        top: 28,
                        bottom: 28,
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        ui.add_space(8.0);

                        // GitHub icon
                        ui.label(
                            RichText::new(regular::MARK_GITHUB)
                                .color(accent)
                                .size(28.0),
                        );

                        ui.add_space(16.0);

                        ui.label(
                            RichText::new("Authorize in your browser")
                                .color(text_primary)
                                .font(typography::proportional(14.0))
                                .strong(),
                        );

                        ui.add_space(8.0);

                        ui.label(
                            RichText::new(
                                "A browser window has opened.\nSign in to GitHub and authorize Enya to continue.",
                            )
                            .color(text_tertiary.gamma_multiply(0.6))
                            .font(typography::proportional(typography::SM)),
                        );

                        ui.add_space(20.0);

                        // Animated waiting indicator
                        let elapsed = ui.input(|i| i.time) as f32;
                        let dots: String = (0..3)
                            .map(|i| {
                                let phase =
                                    (elapsed * 2.0 + i as f32 * 0.5).sin() * 0.5 + 0.5;
                                if phase > 0.3 { '.' } else { ' ' }
                            })
                            .collect();

                        ui.label(
                            RichText::new(format!("Waiting for authorization{dots}"))
                                .color(text_tertiary.gamma_multiply(0.6))
                                .font(typography::proportional(typography::SM)),
                        );

                        ctx.request_repaint_after(std::time::Duration::from_millis(500));
                    });
            }

            AuthState::SignedIn(creds) => {
                let creds = creds.clone();

                // User info card
                let stroke_color = if is_focused {
                    accent.gamma_multiply(0.5)
                } else {
                    card_border
                };

                egui::Frame::new()
                    .fill(card_bg)
                    .stroke(egui::Stroke::new(
                        if is_focused { 1.5 } else { 1.0 },
                        stroke_color,
                    ))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 20,
                        top: 20,
                        bottom: 20,
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        ui.horizontal(|ui| {
                            // Avatar circle
                            let avatar_size = 40.0;
                            let (rect, _response) = ui.allocate_exact_size(
                                Vec2::splat(avatar_size),
                                egui::Sense::hover(),
                            );

                            if let Some(texture) = &self.avatar_texture {
                                // Draw avatar image clipped to a circle
                                let mut mesh = egui::Mesh::with_texture(texture.id());

                                // Build a circle mesh with UV mapping
                                let center = rect.center();
                                let radius = avatar_size / 2.0;
                                let segments = 32;
                                // Center vertex
                                mesh.vertices.push(egui::epaint::Vertex {
                                    pos: center,
                                    uv: egui::pos2(0.5, 0.5),
                                    color: Color32::WHITE,
                                });
                                for i in 0..=segments {
                                    let angle = std::f32::consts::TAU * i as f32 / segments as f32;
                                    let (sin, cos) = angle.sin_cos();
                                    mesh.vertices.push(egui::epaint::Vertex {
                                        pos: center + egui::vec2(cos * radius, sin * radius),
                                        uv: egui::pos2(0.5 + cos * 0.5, 0.5 + sin * 0.5),
                                        color: Color32::WHITE,
                                    });
                                    if i > 0 {
                                        mesh.indices.push(0); // center
                                        mesh.indices.push(i);
                                        mesh.indices.push(i + 1);
                                    }
                                }

                                ui.painter().add(egui::Shape::mesh(mesh));
                            } else {
                                // Fallback: circle with initial letter
                                ui.painter().circle_filled(
                                    rect.center(),
                                    avatar_size / 2.0,
                                    accent.gamma_multiply(0.2),
                                );
                                let initial = creds
                                    .user
                                    .login
                                    .chars()
                                    .next()
                                    .unwrap_or('?')
                                    .to_uppercase()
                                    .to_string();
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    initial,
                                    FontId::new(18.0, FontFamily::Proportional),
                                    accent,
                                );
                            }

                            ui.add_space(12.0);

                            // Username and status
                            ui.vertical(|ui| {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(&creds.user.login)
                                        .color(text_primary)
                                        .font(typography::proportional(14.0))
                                        .strong(),
                                );
                                ui.add_space(2.0);
                                ui.horizontal(|ui| {
                                    // Green dot
                                    let (dot_rect, _) = ui.allocate_exact_size(
                                        Vec2::splat(8.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().circle_filled(
                                        dot_rect.center(),
                                        3.0,
                                        Color32::from_rgb(74, 222, 128),
                                    );
                                    ui.label(
                                        RichText::new("Connected via GitHub")
                                            .color(text_tertiary.gamma_multiply(0.6))
                                            .font(typography::proportional(typography::XS)),
                                    );
                                });
                            });
                        });
                    });

                ui.add_space(12.0);

                // Sign out button (muted)
                let btn_height = 32.0;
                let btn_width = 100.0;
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(btn_width, btn_height), egui::Sense::click());

                let signout_fill = if response.hovered() {
                    self.theme.bg_hover()
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter().rect(
                    rect,
                    6.0,
                    signout_fill,
                    egui::Stroke::new(1.0, text_tertiary.gamma_multiply(0.2)),
                    egui::epaint::StrokeKind::Outside,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Sign out",
                    typography::proportional(typography::SM),
                    text_tertiary.gamma_multiply(0.7),
                );

                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if response.clicked() || kb_action {
                    *result = SettingsPageResult::GitHubSignOut;
                }
            }
        }
    }

    // ── Connections Section ───────────────────────────────────────────────

    fn show_general_section(
        &mut self,
        ui: &mut egui::Ui,
        accent: Color32,
        text_primary: Color32,
        _text_secondary: Color32,
        text_tertiary: Color32,
    ) {
        let card_bg = self.theme.bg_elevated().gamma_multiply(0.55);
        let card_border = self.theme.border_subtle().gamma_multiply(0.6);
        let bg_hover = self.theme.bg_hover();

        // Connection rows: (index, icon, label, description, field, placeholder)
        let rows: &[(usize, &str, &str, &str, EditingField, &str)] = &[
            (
                0,
                regular::CHART_LINE,
                "Prometheus",
                "Metrics endpoint",
                EditingField::PrometheusEndpoint,
                "https://prometheus.example.com/api/v1",
            ),
            (
                1,
                regular::TEXT_SEARCH,
                "Loki",
                "Log query endpoint",
                EditingField::LokiEndpoint,
                "https://loki.example.com/loki/api/v1",
            ),
            (
                2,
                regular::DATABASE_2,
                "Flight SQL",
                "SQL query endpoint",
                EditingField::FlightSqlEndpoint,
                "grpc://localhost:50051",
            ),
            (
                3,
                regular::GIT_BRANCH,
                "Codebase",
                "Repository URL",
                EditingField::GitRepoUrl,
                "https://github.com/org/repo",
            ),
        ];

        for (i, (index, icon, label, desc, field, placeholder)) in rows.iter().enumerate() {
            if i > 0 {
                ui.add_space(8.0);
            }
            self.show_connection_row(
                ui,
                *index,
                icon,
                label,
                desc,
                *field,
                placeholder,
                card_bg,
                card_border,
                bg_hover,
                accent,
                text_primary,
                text_tertiary,
            );
        }
    }

    /// Render a single connection row as its own card.
    #[allow(clippy::too_many_arguments)]
    fn show_connection_row(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        icon: &str,
        label: &str,
        desc: &str,
        field: EditingField,
        placeholder: &str,
        card_bg: Color32,
        card_border: Color32,
        bg_hover: Color32,
        accent: Color32,
        text_primary: Color32,
        text_tertiary: Color32,
    ) {
        let is_focused =
            self.field_index == index && self.editing_field.is_none() && !self.sidebar_focused;
        let is_editing = self.editing_field == Some(field);
        let value = self.field_value(field);
        let is_configured = !value.is_empty();

        // Probe hover
        let row_height = if is_editing { 80.0 } else { 56.0 };
        let rect_estimate =
            egui::Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), row_height));
        let is_hovered = ui.rect_contains_pointer(rect_estimate);

        let fill = if is_focused {
            card_bg.gamma_multiply(1.6)
        } else if is_hovered {
            bg_hover.gamma_multiply(0.3)
        } else {
            card_bg
        };
        let stroke_color = if is_focused {
            accent.gamma_multiply(0.5)
        } else if is_hovered {
            card_border.gamma_multiply(1.5)
        } else {
            card_border
        };

        egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(
                if is_focused { 1.5 } else { 1.0 },
                stroke_color,
            ))
            .corner_radius(8.0)
            .inner_margin(egui::Margin {
                left: 14,
                right: 14,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                if is_editing {
                    // Edit mode
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon).color(accent).size(14.0));
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(label)
                                .color(accent)
                                .font(typography::proportional(typography::SM))
                                .strong(),
                        );
                    });
                    ui.add_space(6.0);

                    let bg = self.theme.bg_surface();
                    let avail_width = ui.available_width();
                    let text_to_edit = match self.editing_field {
                        Some(EditingField::GitRepoUrl) => &mut self.git_repo_url,
                        Some(EditingField::PrometheusEndpoint) => {
                            &mut self.default_prometheus_endpoint
                        }
                        Some(EditingField::LokiEndpoint) => &mut self.default_loki_endpoint,
                        Some(EditingField::FlightSqlEndpoint) => {
                            &mut self.default_flight_sql_endpoint
                        }
                        None => return,
                    };

                    egui::Frame::new()
                        .fill(bg)
                        .stroke(egui::Stroke::new(1.5, accent.gamma_multiply(0.6)))
                        .corner_radius(6.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.set_width(avail_width - 20.0);
                            let edit = egui::TextEdit::singleline(text_to_edit)
                                .desired_width(ui.available_width())
                                .font(typography::monospace(typography::SM))
                                .text_color(accent)
                                .hint_text(
                                    RichText::new(placeholder)
                                        .font(typography::monospace(typography::SM))
                                        .color(accent.gamma_multiply(0.25)),
                                )
                                .frame(false);
                            let response = ui.add(edit);
                            if !response.has_focus() {
                                response.request_focus();
                            }
                        });
                    ui.add_space(10.0);
                } else {
                    // Display mode — two-line row with icon
                    let avail_width = ui.available_width();
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(avail_width, 52.0), egui::Sense::click());

                    if response.clicked() {
                        self.field_index = index;
                        self.sidebar_focused = false;
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Focus accent strip
                    if is_focused {
                        let strip = egui::Rect::from_min_size(
                            egui::pos2(rect.min.x - 14.0, rect.min.y + 4.0),
                            Vec2::new(3.0, rect.height() - 8.0),
                        );
                        ui.painter().rect_filled(strip, 2.0, accent);
                    }

                    // Icon
                    let icon_color = if is_focused {
                        accent
                    } else if is_configured {
                        text_tertiary.gamma_multiply(0.7)
                    } else {
                        text_tertiary.gamma_multiply(0.3)
                    };
                    ui.painter().text(
                        egui::pos2(rect.min.x + 4.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        icon,
                        FontId::new(16.0, FontFamily::Proportional),
                        icon_color,
                    );

                    // Label + description
                    let text_x = rect.min.x + 28.0;
                    let top_y = rect.min.y + 15.0;
                    let label_color = if is_focused {
                        text_primary
                    } else {
                        text_tertiary.gamma_multiply(0.9)
                    };
                    ui.painter().text(
                        egui::pos2(text_x, top_y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        typography::proportional(typography::SM),
                        label_color,
                    );

                    // Description / status
                    let bottom_y = rect.min.y + 35.0;
                    let status_text = if is_configured { &value } else { desc };
                    let status_color = if is_configured {
                        text_primary.gamma_multiply(if is_focused { 0.8 } else { 0.5 })
                    } else {
                        text_tertiary.gamma_multiply(0.3)
                    };
                    let max_val_width = rect.max.x - text_x - if is_focused { 50.0 } else { 12.0 };
                    let galley = ui.painter().layout(
                        status_text.to_string(),
                        typography::monospace(typography::XS),
                        status_color,
                        max_val_width,
                    );
                    ui.painter().galley(
                        egui::pos2(text_x, bottom_y - galley.rect.height() / 2.0),
                        galley,
                        status_color,
                    );

                    // Status dot (right side)
                    let dot_x = rect.max.x - 8.0;
                    let dot_color = if is_configured {
                        Color32::from_rgb(74, 222, 128).gamma_multiply(0.6)
                    } else {
                        text_tertiary.gamma_multiply(0.15)
                    };
                    ui.painter()
                        .circle_filled(egui::pos2(dot_x, rect.center().y), 3.0, dot_color);

                    // Edit hint when focused
                    if is_focused {
                        ui.painter().text(
                            egui::pos2(rect.max.x - 20.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            "enter",
                            typography::monospace(8.0),
                            accent.gamma_multiply(0.35),
                        );
                    }
                }
            });
    }

    /// Get the current value for an editing field.
    fn field_value(&self, field: EditingField) -> String {
        match field {
            EditingField::PrometheusEndpoint => self.default_prometheus_endpoint.clone(),
            EditingField::LokiEndpoint => self.default_loki_endpoint.clone(),
            EditingField::FlightSqlEndpoint => self.default_flight_sql_endpoint.clone(),
            EditingField::GitRepoUrl => self.git_repo_url.clone(),
        }
    }

    // ── AI Section ───────────────────────────────────────────────────────

    fn show_ai_section(
        &mut self,
        ui: &mut egui::Ui,
        accent: Color32,
        text_primary: Color32,
        _text_secondary: Color32,
        text_tertiary: Color32,
    ) {
        let card_bg = self.theme.bg_elevated().gamma_multiply(0.3);
        let card_border = self.theme.border_subtle().gamma_multiply(0.4);
        let bg_hover = self.theme.bg_hover();

        Self::show_section_header(
            ui,
            "Configuration",
            "Select your AI provider and model for assistant features",
            text_tertiary,
        );

        egui::Frame::new()
            .fill(card_bg)
            .stroke(egui::Stroke::new(1.0, card_border))
            .corner_radius(8.0)
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 20,
                bottom: 20,
            })
            .show(ui, |ui| {
                // Provider
                let provider_focused =
                    self.field_index == 0 && self.editing_field.is_none() && !self.sidebar_focused;
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
                let model_focused =
                    self.field_index == 1 && self.editing_field.is_none() && !self.sidebar_focused;
                let model_expanded = model_focused && self.ai_dropdown_open;
                let current_model = self
                    .ai_model
                    .unwrap_or_else(|| AiModel::default_for(self.ai_provider));

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
                    let models = AiModel::for_provider(self.ai_provider);
                    for model in models {
                        let is_selected = current_model == *model;
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
                            self.ai_model = Some(*model);
                            self.ai_dropdown_open = false;
                        }
                    }
                } else {
                    let clicked = self.show_dropdown_value(
                        ui,
                        current_model.display_name(),
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

    // ── Editor / Appearance Section ────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn show_appearance_section(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        accent: Color32,
        text_primary: Color32,
        text_tertiary: Color32,
        result: &mut SettingsPageResult,
    ) {
        let bg_elevated = self.theme.bg_elevated();
        let bg_hover = self.theme.bg_hover();
        let border = self.theme.border_subtle().gamma_multiply(0.4);

        // Animate panel switch highlight
        if self.panel_switch_anim > 0.0 {
            self.panel_switch_anim =
                (self.panel_switch_anim - ctx.input(|i| i.stable_dt) * 3.0).max(0.0);
            ctx.request_repaint();
        }

        let anim_glow = self.panel_switch_anim * 0.3;
        let content_width = ui.available_width();
        let panel_width = ((content_width - 24.0) / 2.0).min(400.0);
        let panel_height = 400.0;

        // Side-by-side panels
        ui.horizontal(|ui| {
            // Theme panel (left)
            let theme_focused = self.focused_panel == StyleTab::Theme && !self.sidebar_focused;
            ui.allocate_ui(Vec2::new(panel_width, panel_height), |ui| {
                let fill = if theme_focused {
                    bg_elevated.gamma_multiply(0.6 + anim_glow)
                } else {
                    bg_elevated.gamma_multiply(0.3)
                };
                let stroke_width = if theme_focused { 2.0 } else { 1.0 };
                let stroke_color = if theme_focused {
                    accent.gamma_multiply(0.7 + anim_glow)
                } else {
                    border
                };

                egui::Frame::new()
                    .fill(fill)
                    .stroke(egui::Stroke::new(stroke_width, stroke_color))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 20,
                        bottom: 16,
                    })
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            self.render_theme_panel(
                                ui,
                                panel_width - 32.0,
                                panel_height,
                                accent,
                                text_primary,
                                text_tertiary,
                                bg_hover,
                                border,
                                result,
                            );
                        });
                    });
            });

            ui.add_space(12.0);

            // Font panel (right)
            let font_focused = self.focused_panel == StyleTab::Font && !self.sidebar_focused;
            ui.allocate_ui(Vec2::new(panel_width, panel_height), |ui| {
                let fill = if font_focused {
                    bg_elevated.gamma_multiply(0.6 + anim_glow)
                } else {
                    bg_elevated.gamma_multiply(0.3)
                };
                let stroke_width = if font_focused { 2.0 } else { 1.0 };
                let stroke_color = if font_focused {
                    accent.gamma_multiply(0.7 + anim_glow)
                } else {
                    border
                };

                egui::Frame::new()
                    .fill(fill)
                    .stroke(egui::Stroke::new(stroke_width, stroke_color))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 20,
                        bottom: 16,
                    })
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            self.render_font_panel(
                                ui,
                                panel_width - 32.0,
                                panel_height,
                                accent,
                                text_primary,
                                text_tertiary,
                                bg_hover,
                                result,
                            );
                        });
                    });
            });
        });
    }

    /// Renders the theme panel.
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
        border: Color32,
        result: &mut SettingsPageResult,
    ) {
        let is_focused = self.focused_panel == StyleTab::Theme && !self.sidebar_focused;
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
        ui.add_space(12.0);

        // Theme list
        egui::ScrollArea::vertical()
            .id_salt("settings_page_theme_scroll")
            .max_height(panel_height - 50.0)
            .auto_shrink([false, false])
            .animated(true)
            .show(ui, |ui| {
                for i in 0..theme_count {
                    // Separator before custom themes
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
                        border,
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
        border: Color32,
        result: &mut SettingsPageResult,
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

        if response.clicked() {
            self.theme_index = index;
            self.focused_panel = StyleTab::Theme;
            self.sidebar_focused = false;
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

        // Color palette bar
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
            egui::Stroke::new(1.0, border),
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

        // "current" indicator
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

    /// Renders the font panel.
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
        result: &mut SettingsPageResult,
    ) {
        let is_focused = self.focused_panel == StyleTab::Font && !self.sidebar_focused;
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
        ui.add_space(12.0);

        // Font list
        egui::ScrollArea::vertical()
            .id_salt("settings_page_font_scroll")
            .max_height(panel_height - 50.0)
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

                    if is_selected && self.scroll_to_font {
                        ui.scroll_to_rect(rect, Some(egui::Align::Center));
                        self.scroll_to_font = false;
                    }

                    if response.clicked() {
                        self.font_index = i;
                        self.focused_panel = StyleTab::Font;
                        self.sidebar_focused = false;
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

                    // Font preview
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

    // ── Shared UI Helpers ────────────────────────────────────────────────

    fn show_section_header(
        ui: &mut egui::Ui,
        label: &str,
        description: &str,
        text_tertiary: Color32,
    ) {
        ui.label(
            RichText::new(label.to_uppercase())
                .color(text_tertiary.gamma_multiply(0.6))
                .font(typography::proportional(typography::XS))
                .strong(),
        );
        ui.add_space(2.0);
        ui.label(
            RichText::new(description)
                .color(text_tertiary.gamma_multiply(0.35))
                .font(typography::proportional(typography::XS)),
        );
        ui.add_space(10.0);
    }

    fn show_field_divider(ui: &mut egui::Ui, separator: Color32) {
        ui.add_space(16.0);
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().top(),
            egui::Stroke::new(1.0, separator.gamma_multiply(0.3)),
        );
        ui.add_space(16.0);
    }

    /// Render the label for a dropdown field.
    #[allow(clippy::too_many_arguments)]
    fn show_dropdown_label(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        is_focused: bool,
        is_expanded: bool,
        accent: Color32,
        text_primary: Color32,
        text_tertiary: Color32,
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
        ui.add_space(8.0);
    }

    /// Render the collapsed value for a dropdown field. Returns true if clicked.
    #[allow(clippy::too_many_arguments)]
    fn show_dropdown_value(
        &self,
        ui: &mut egui::Ui,
        value: &str,
        is_focused: bool,
        accent: Color32,
        text_primary: Color32,
        _text_tertiary: Color32,
    ) -> bool {
        let input_height = 38.0;
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
        accent: Color32,
        text_primary: Color32,
        text_tertiary: Color32,
        bg_hover: Color32,
    ) -> egui::Response {
        let row_height = 34.0;
        let avail_width = ui.available_width();
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(avail_width, row_height), egui::Sense::click());

        let bg = if is_selected {
            accent.gamma_multiply(0.15)
        } else if response.hovered() {
            bg_hover.gamma_multiply(0.5)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 4.0, bg);

        if is_selected {
            ui.painter()
                .circle_filled(egui::pos2(rect.min.x + 10.0, rect.center().y), 3.0, accent);
        }

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

    // ── Keyboard Handling ────────────────────────────────────────────────

    /// Cycle the currently focused AI dropdown option by delta.
    fn cycle_ai_dropdown(&mut self, delta: i32) {
        match self.field_index {
            0 => {
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
                let models = AiModel::for_provider(self.ai_provider);
                let current = self
                    .ai_model
                    .unwrap_or_else(|| AiModel::default_for(self.ai_provider));
                let idx = models.iter().position(|m| *m == current).unwrap_or(0);
                let new_idx = ((idx as i32 + delta).rem_euclid(models.len() as i32)) as usize;
                self.ai_model = Some(models[new_idx]);
            }
            _ => {}
        }
    }

    /// Handle keyboard navigation and actions.
    fn handle_keyboard(&mut self, ctx: &egui::Context) -> SettingsPageResult {
        // If editing a text field, only handle Escape and Enter to stop editing
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
            return SettingsPageResult::None;
        }

        let mut result = SettingsPageResult::None;

        ctx.input_mut(|i| {
            // Escape — save and go back
            if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                result = self.build_saved();
                return;
            }

            // Number keys — jump to category directly
            let categories = SettingsCategory::all();
            for (idx, cat) in categories.iter().enumerate() {
                let key = match idx {
                    0 => Key::Num1,
                    1 => Key::Num2,
                    2 => Key::Num3,
                    3 => Key::Num4,
                    _ => continue,
                };
                if i.consume_key(egui::Modifiers::NONE, key) {
                    self.active_category = *cat;
                    self.field_index = 0;
                    self.editing_field = None;
                    self.ai_dropdown_open = false;
                    self.sidebar_focused = false;
                    return;
                }
            }

            // Tab / Shift+Tab — cycle categories
            if i.consume_key(egui::Modifiers::SHIFT, Key::Tab) {
                let cats = SettingsCategory::all();
                let idx = cats
                    .iter()
                    .position(|c| *c == self.active_category)
                    .unwrap_or(0);
                self.active_category = cats[if idx == 0 { cats.len() - 1 } else { idx - 1 }];
                self.field_index = 0;
                self.editing_field = None;
                self.ai_dropdown_open = false;
                self.sidebar_focused = false;
                return;
            }
            if i.consume_key(egui::Modifiers::NONE, Key::Tab) {
                let cats = SettingsCategory::all();
                let idx = cats
                    .iter()
                    .position(|c| *c == self.active_category)
                    .unwrap_or(0);
                self.active_category = cats[(idx + 1) % cats.len()];
                self.field_index = 0;
                self.editing_field = None;
                self.ai_dropdown_open = false;
                self.sidebar_focused = false;
                return;
            }

            // Sidebar-focused navigation
            if self.sidebar_focused {
                if i.consume_key(egui::Modifiers::NONE, Key::J)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                {
                    let cats = SettingsCategory::all();
                    let idx = cats
                        .iter()
                        .position(|c| *c == self.active_category)
                        .unwrap_or(0);
                    self.active_category = cats[(idx + 1) % cats.len()];
                    self.field_index = 0;
                    self.editing_field = None;
                    self.ai_dropdown_open = false;
                    return;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::K)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                {
                    let cats = SettingsCategory::all();
                    let idx = cats
                        .iter()
                        .position(|c| *c == self.active_category)
                        .unwrap_or(0);
                    self.active_category = cats[if idx == 0 { cats.len() - 1 } else { idx - 1 }];
                    self.field_index = 0;
                    self.editing_field = None;
                    self.ai_dropdown_open = false;
                    return;
                }
                if i.consume_key(egui::Modifiers::NONE, Key::L)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowRight)
                    || i.consume_key(egui::Modifiers::NONE, Key::Enter)
                {
                    self.sidebar_focused = false;
                    return;
                }
                return;
            }

            // h / ArrowLeft — focus sidebar (except in ThemeFont panel navigation)
            if self.active_category != SettingsCategory::ThemeFont
                && (i.consume_key(egui::Modifiers::NONE, Key::H)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft))
            {
                self.sidebar_focused = true;
                self.ai_dropdown_open = false;
                return;
            }

            // ThemeFont category uses panel-based navigation
            if self.active_category == SettingsCategory::ThemeFont {
                // h — switch to Theme panel or sidebar
                if i.consume_key(egui::Modifiers::NONE, Key::H)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowLeft)
                {
                    if self.focused_panel == StyleTab::Font {
                        self.focused_panel = StyleTab::Theme;
                        self.panel_switch_anim = 1.0;
                    } else {
                        self.sidebar_focused = true;
                    }
                    return;
                }

                // l — switch to Font panel
                if i.consume_key(egui::Modifiers::NONE, Key::L)
                    || i.consume_key(egui::Modifiers::NONE, Key::ArrowRight)
                {
                    if self.focused_panel != StyleTab::Font {
                        self.focused_panel = StyleTab::Font;
                        self.panel_switch_anim = 1.0;
                    }
                    return;
                }

                // j/Down — navigate down in focused panel
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

                // k/Up — navigate up in focused panel
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

                if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    return;
                }

                return;
            }

            // AI category with dropdown open
            if self.active_category == SettingsCategory::Ai && self.ai_dropdown_open {
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
                if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    self.ai_dropdown_open = false;
                    return;
                }
                return;
            }

            // j/Down — move down in content
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

            // k/Up — move up in content
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

            // l/Enter — open dropdown or activate text edit
            if i.consume_key(egui::Modifiers::NONE, Key::Enter)
                || i.consume_key(egui::Modifiers::NONE, Key::L)
            {
                match self.active_category {
                    SettingsCategory::Auth => {
                        self.pending_account_action = true;
                    }
                    SettingsCategory::Ai => {
                        self.ai_dropdown_open = true;
                    }
                    SettingsCategory::ThemeFont => {} // Handled above
                    SettingsCategory::Connections => {
                        let field = match self.field_index {
                            0 => Some(EditingField::PrometheusEndpoint),
                            1 => Some(EditingField::LokiEndpoint),
                            2 => Some(EditingField::FlightSqlEndpoint),
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
    fn test_new_page_starts_closed() {
        let page = SettingsPage::new();
        assert!(!page.is_open());
    }

    #[test]
    fn test_open_close() {
        let mut page = SettingsPage::new();
        page.open(
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
            AuthState::SignedOut,
        );
        assert!(page.is_open());
        page.close();
        assert!(!page.is_open());
    }

    #[test]
    fn test_field_count() {
        let mut page = SettingsPage::new();
        page.active_category = SettingsCategory::Auth;
        assert_eq!(page.field_count(), 1);
        page.active_category = SettingsCategory::Connections;
        assert_eq!(page.field_count(), 4);
        page.active_category = SettingsCategory::Ai;
        assert_eq!(page.field_count(), 2);
        page.active_category = SettingsCategory::ThemeFont;
        assert_eq!(page.field_count(), 0);
    }

    #[test]
    fn test_category_count() {
        let cats = SettingsCategory::all();
        assert_eq!(cats.len(), 4);
    }
}
