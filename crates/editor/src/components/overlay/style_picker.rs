//! Style Picker - A unified overlay for theme and font selection.
//!
//! This module provides a visual style picker that allows users to browse
//! themes and fonts side-by-side with live preview. Themes on the left,
//! fonts on the right, so you can see the immediate impact of font changes.

use egui::{Color32, FontFamily, FontId, Key, RichText, Vec2};
use egui_nerdfonts::regular;

use crate::ui::ActiveThemeColors;
use crate::ui::settings_screen::EditorFont;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;

/// Which panel is currently focused in the style picker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StyleTab {
    #[default]
    Theme,
    Font,
}

/// Result of the style picker interaction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StylePickerResult {
    /// User selected a builtin theme (Enter on theme panel)
    ThemeSelected(AppTheme),
    /// User selected a custom theme by name
    CustomThemeSelected(String),
    /// User cancelled - return to original theme/custom_theme and font
    /// (AppTheme, Option<custom_theme_name>, EditorFont)
    Cancelled(AppTheme, Option<String>, EditorFont),
    /// User is previewing a builtin theme (live update while navigating)
    ThemePreview(AppTheme),
    /// User is previewing a custom theme by name
    CustomThemePreview(String),
    /// User selected a font (Enter on font panel)
    FontSelected(EditorFont),
    /// User is previewing a font (live update while navigating)
    FontPreview(EditorFont),
    /// No action this frame
    None,
}

/// A theme entry in the picker (either builtin or custom)
#[derive(Debug, Clone)]
pub enum ThemeEntry {
    /// Builtin theme from AppTheme enum
    Builtin(AppTheme),
    /// Custom theme from plugins (name, display_name, resolved colors for preview)
    Custom {
        name: String,
        display_name: String,
        colors: ActiveThemeColors,
    },
}

/// Interactive style picker overlay with side-by-side Theme and Font panels
pub struct StylePicker {
    /// Whether the picker is currently visible
    open: bool,
    /// Current focused panel (Theme or Font)
    focused_panel: StyleTab,
    /// The builtin theme that was active when the picker opened (for cancel restore)
    original_theme: AppTheme,
    /// The custom theme name that was active when the picker opened (for cancel restore)
    original_custom_theme: Option<String>,
    /// The font that was active when the picker opened
    original_font: EditorFont,
    /// Currently selected theme index (into combined builtin + custom list)
    theme_index: usize,
    /// Currently selected font index
    font_index: usize,
    /// Last preview theme (to detect changes) - None means builtin, Some(name) means custom
    last_theme_preview: Option<ThemeEntry>,
    /// Last preview font (to detect changes)
    last_font_preview: Option<EditorFont>,
    /// Animation progress for panel switch highlight (0.0 to 1.0)
    panel_switch_anim: f32,
    /// Whether to scroll to the selected theme (set when navigating)
    scroll_to_theme: bool,
    /// Whether to scroll to the selected font (set when navigating)
    scroll_to_font: bool,
    /// Custom themes from plugins (name, display_name, colors)
    custom_themes: Vec<(String, String, ActiveThemeColors)>,
}

impl Default for StylePicker {
    fn default() -> Self {
        Self::new()
    }
}

impl StylePicker {
    /// Creates a new style picker.
    pub fn new() -> Self {
        Self {
            open: false,
            focused_panel: StyleTab::Theme,
            original_theme: AppTheme::default(),
            original_custom_theme: None,
            original_font: EditorFont::default(),
            theme_index: 0,
            font_index: 0,
            last_theme_preview: None,
            last_font_preview: None,
            panel_switch_anim: 0.0,
            scroll_to_theme: false,
            scroll_to_font: false,
            custom_themes: Vec::new(),
        }
    }

    /// Set the custom themes available from plugins.
    /// Each tuple is (name, display_name, resolved colors).
    pub fn set_custom_themes(&mut self, themes: Vec<(String, String, ActiveThemeColors)>) {
        self.custom_themes = themes;
    }

    /// Get the combined list of theme entries (builtins first, then custom).
    fn theme_entries(&self) -> Vec<ThemeEntry> {
        let mut entries: Vec<ThemeEntry> = AppTheme::all()
            .iter()
            .map(|t| ThemeEntry::Builtin(*t))
            .collect();
        for (name, display_name, colors) in &self.custom_themes {
            entries.push(ThemeEntry::Custom {
                name: name.clone(),
                display_name: display_name.clone(),
                colors: *colors,
            });
        }
        entries
    }

    /// Returns true if the picker is currently visible.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the style picker with current theme and font.
    /// If custom_theme is Some, that custom theme is currently selected.
    pub fn open_with_custom(
        &mut self,
        current_theme: AppTheme,
        custom_theme: Option<&str>,
        current_font: EditorFont,
    ) {
        self.open = true;
        self.original_theme = current_theme;
        self.original_custom_theme = custom_theme.map(|s| s.to_string());
        self.original_font = current_font;
        self.last_font_preview = Some(current_font);

        // Find the current theme index
        let builtin_count = AppTheme::all().len();
        if let Some(custom_name) = custom_theme {
            // Currently using a custom theme - find its index
            if let Some(idx) = self
                .custom_themes
                .iter()
                .position(|(name, _, _)| name == custom_name)
            {
                self.theme_index = builtin_count + idx;
                self.last_theme_preview = Some(ThemeEntry::Custom {
                    name: custom_name.to_string(),
                    display_name: self.custom_themes[idx].1.clone(),
                    colors: self.custom_themes[idx].2,
                });
            } else {
                // Custom theme not found, fall back to builtin
                self.theme_index = AppTheme::all()
                    .iter()
                    .position(|t| *t == current_theme)
                    .unwrap_or(0);
                self.last_theme_preview = Some(ThemeEntry::Builtin(current_theme));
            }
        } else {
            // Using a builtin theme
            self.theme_index = AppTheme::all()
                .iter()
                .position(|t| *t == current_theme)
                .unwrap_or(0);
            self.last_theme_preview = Some(ThemeEntry::Builtin(current_theme));
        }

        let fonts = EditorFont::all();
        self.font_index = fonts.iter().position(|f| *f == current_font).unwrap_or(0);
    }

    /// Opens the style picker with current theme and font (no custom theme).
    pub fn open(&mut self, current_theme: AppTheme, current_font: EditorFont) {
        self.open_with_custom(current_theme, None, current_font);
    }

    /// Opens directly to the theme panel
    pub fn open_theme(&mut self, current_theme: AppTheme, current_font: EditorFont) {
        self.focused_panel = StyleTab::Theme;
        self.open(current_theme, current_font);
    }

    /// Opens directly to the font panel
    pub fn open_font(&mut self, current_theme: AppTheme, current_font: EditorFont) {
        self.focused_panel = StyleTab::Font;
        self.open(current_theme, current_font);
    }

    /// Closes the style picker.
    pub fn close(&mut self) {
        self.open = false;
        self.last_theme_preview = None;
        self.last_font_preview = None;
    }

    /// Shows the style picker overlay.
    /// Shows the style picker overlay.
    #[profiling::function]
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        current_theme: AppTheme,
        current_font: EditorFont,
    ) -> StylePickerResult {
        if !self.open {
            return StylePickerResult::None;
        }

        // Extract colors from theme (Custom variant handles plugin colors internally)
        let style = OverlayStyle::frosted_glass(current_theme);
        let accent = current_theme.accent_primary();
        let text = current_theme.text_primary();
        let text_muted = current_theme.text_tertiary();
        let bg_hover = current_theme.bg_hover();
        let bg_elevated = current_theme.bg_elevated();

        let mut result = StylePickerResult::None;
        let mut needs_repaint = false;

        let theme_entries = self.theme_entries();
        let fonts = EditorFont::all();
        let theme_count = theme_entries.len();
        let font_count = fonts.len();

        // Clamp indices
        if theme_count == 0 {
            self.theme_index = 0;
        } else if self.theme_index >= theme_count {
            self.theme_index = theme_count - 1;
        }

        if font_count == 0 {
            self.font_index = 0;
        } else if self.font_index >= font_count {
            self.font_index = font_count - 1;
        }

        // Track if picker is closing (to clear egui focus after input handling)
        let mut should_clear_focus = false;

        // Handle keyboard input - use consume_key to prevent other components from handling
        ctx.input_mut(|i| {
            // Escape to cancel - restore both original theme and font
            if i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                self.close();
                result = StylePickerResult::Cancelled(
                    self.original_theme,
                    self.original_custom_theme.clone(),
                    self.original_font,
                );
                should_clear_focus = true;
                return;
            }

            // Tab to switch panels
            if i.consume_key(egui::Modifiers::NONE, Key::Tab) {
                let new_panel = match self.focused_panel {
                    StyleTab::Theme => StyleTab::Font,
                    StyleTab::Font => StyleTab::Theme,
                };
                if new_panel != self.focused_panel {
                    self.focused_panel = new_panel;
                    self.panel_switch_anim = 1.0; // Trigger animation
                }
            }

            // h/l (vim) to switch panels - h = left (Theme), l = right (Font)
            if i.consume_key(egui::Modifiers::NONE, Key::H) && self.focused_panel != StyleTab::Theme
            {
                self.focused_panel = StyleTab::Theme;
                self.panel_switch_anim = 1.0; // Trigger animation
            }
            if i.consume_key(egui::Modifiers::NONE, Key::L) && self.focused_panel != StyleTab::Font
            {
                self.focused_panel = StyleTab::Font;
                self.panel_switch_anim = 1.0; // Trigger animation
            }

            // Enter to confirm selection in focused panel
            if i.consume_key(egui::Modifiers::NONE, Key::Enter) {
                match self.focused_panel {
                    StyleTab::Theme if theme_count > 0 => {
                        match &theme_entries[self.theme_index] {
                            ThemeEntry::Builtin(theme) => {
                                let theme = *theme;
                                self.close();
                                result = StylePickerResult::ThemeSelected(theme);
                            }
                            ThemeEntry::Custom { name, .. } => {
                                let name = name.clone();
                                self.close();
                                result = StylePickerResult::CustomThemeSelected(name);
                            }
                        }
                        should_clear_focus = true;
                    }
                    StyleTab::Font if font_count > 0 => {
                        let selected = fonts[self.font_index];
                        self.close();
                        result = StylePickerResult::FontSelected(selected);
                        should_clear_focus = true;
                    }
                    _ => {}
                }
                return;
            }

            // Navigation in focused panel - j/k and arrow keys
            let (count, index, is_theme) = match self.focused_panel {
                StyleTab::Theme => (theme_count, &mut self.theme_index, true),
                StyleTab::Font => (font_count, &mut self.font_index, false),
            };

            if count > 0 {
                let old_index = *index;
                // Down: j, ArrowDown, Ctrl+N
                if i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
                    || i.consume_key(egui::Modifiers::NONE, Key::J)
                    || i.consume_key(egui::Modifiers::CTRL, Key::N)
                {
                    *index = (*index + 1) % count;
                }
                // Up: k, ArrowUp, Ctrl+P
                if i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
                    || i.consume_key(egui::Modifiers::NONE, Key::K)
                    || i.consume_key(egui::Modifiers::CTRL, Key::P)
                {
                    *index = index.checked_sub(1).unwrap_or(count - 1);
                }
                // Request repaint and scroll for animation if index changed
                if *index != old_index {
                    needs_repaint = true;
                    if is_theme {
                        self.scroll_to_theme = true;
                    } else {
                        self.scroll_to_font = true;
                    }
                }
            }
        });

        // Clear egui focus when picker closes so vim keys work immediately
        if should_clear_focus {
            ctx.memory_mut(|mem| mem.surrender_focus(egui::Id::NULL));
        }

        // Request repaint for scroll animation
        if needs_repaint {
            ctx.request_repaint();
        }

        // If already returning a result, skip rendering
        if result != StylePickerResult::None {
            return result;
        }

        // Check if theme preview changed - apply immediately
        let current_entry = &theme_entries[self.theme_index];
        let preview_changed = match (&self.last_theme_preview, current_entry) {
            (Some(ThemeEntry::Builtin(prev)), ThemeEntry::Builtin(curr)) => prev != curr,
            (
                Some(ThemeEntry::Custom { name: prev, .. }),
                ThemeEntry::Custom { name: curr, .. },
            ) => prev != curr,
            (None, _) => true,
            _ => true, // Different types = changed
        };
        if preview_changed {
            self.last_theme_preview = Some(current_entry.clone());
            match current_entry {
                ThemeEntry::Builtin(theme) => {
                    result = StylePickerResult::ThemePreview(*theme);
                }
                ThemeEntry::Custom { name, .. } => {
                    result = StylePickerResult::CustomThemePreview(name.clone());
                }
            }
        }

        // Check if font preview changed - apply immediately
        let current_font_preview = fonts[self.font_index];
        if self.last_font_preview != Some(current_font_preview) {
            self.last_font_preview = Some(current_font_preview);
            // Font preview takes priority if both changed in same frame
            result = StylePickerResult::FontPreview(current_font_preview);
        }

        // Render the overlay — use overlay_content_rect so it centers within the content area
        // (accounting for sidebar)
        let available_rect = crate::util::overlay_content_rect(ctx);
        let overlay_width = 700.0_f32.min(available_rect.width() - 40.0);
        let overlay_max_height = 480.0_f32.min(available_rect.height() - 80.0);
        let panel_width = (overlay_width - 56.0) / 2.0;
        let list_height = overlay_max_height - 140.0;

        // Backdrop - use Tooltip order to appear above other overlays
        egui::Area::new(egui::Id::new("style_picker_backdrop"))
            .fixed_pos(available_rect.min)
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                ui.painter()
                    .rect_filled(available_rect, 0.0, Color32::from_black_alpha(160));
            });

        // Main overlay - use Tooltip order to be the uppermost overlay
        egui::Area::new(egui::Id::new("style_picker"))
            .fixed_pos(egui::pos2(
                (available_rect.width() - overlay_width) / 2.0 + available_rect.min.x,
                available_rect.min.y + 60.0,
            ))
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(style.bg)
                    .stroke(egui::Stroke::new(1.0, style.border))
                    .corner_radius(12.0)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: Color32::from_black_alpha(80),
                    })
                    .show(ui, |ui| {
                        ui.set_width(overlay_width);
                        ui.set_max_height(overlay_max_height);

                        ui.vertical(|ui| {
                            // Header
                            ui.add_space(16.0);
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(regular::PAINTBRUSH).color(accent).size(20.0),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Style")
                                        .color(text)
                                        .font(typography::proportional(typography::XL))
                                        .strong(),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Theme & Font")
                                        .color(text_muted)
                                        .font(typography::proportional(typography::MD)),
                                );
                            });
                            ui.add_space(12.0);

                            // Divider
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                let rect = ui.available_rect_before_wrap();
                                ui.painter().hline(
                                    rect.min.x..=rect.min.x + overlay_width - 32.0,
                                    rect.min.y,
                                    egui::Stroke::new(1.0, style.border),
                                );
                            });
                            ui.add_space(12.0);

                            // Animate panel switch highlight (decay over time)
                            if self.panel_switch_anim > 0.0 {
                                self.panel_switch_anim = (self.panel_switch_anim
                                    - ctx.input(|i| i.stable_dt) * 3.0)
                                    .max(0.0);
                                ctx.request_repaint();
                            }

                            // Side-by-side panels
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);

                                // Calculate animation glow for focused panel
                                let anim_glow = self.panel_switch_anim * 0.3;

                                // Theme panel (left)
                                let theme_focused = self.focused_panel == StyleTab::Theme;
                                ui.allocate_ui(Vec2::new(panel_width, list_height + 50.0), |ui| {
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
                                                    list_height,
                                                    &theme_entries,
                                                    accent,
                                                    text,
                                                    text_muted,
                                                    bg_hover,
                                                    &style,
                                                    &mut result,
                                                );
                                            });
                                        });
                                });

                                ui.add_space(8.0);

                                // Font panel (right)
                                let font_focused = self.focused_panel == StyleTab::Font;
                                ui.allocate_ui(Vec2::new(panel_width, list_height + 50.0), |ui| {
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
                                                    list_height,
                                                    fonts,
                                                    accent,
                                                    text,
                                                    text_muted,
                                                    bg_hover,
                                                    current_font,
                                                    &mut result,
                                                );
                                            });
                                        });
                                });

                                ui.add_space(16.0);
                            });

                            ui.add_space(12.0);

                            // Footer with key cap styled hints
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);

                                // Helper to draw a key cap
                                let draw_keycap = |ui: &mut egui::Ui, key: &str, label: &str| {
                                    // Key cap background
                                    let key_size = Vec2::new(18.0, 16.0);
                                    let (key_rect, _) =
                                        ui.allocate_exact_size(key_size, egui::Sense::hover());
                                    ui.painter().rect_filled(
                                        key_rect,
                                        3.0,
                                        text.gamma_multiply(0.12),
                                    );
                                    ui.painter().rect_stroke(
                                        key_rect,
                                        3.0,
                                        egui::Stroke::new(1.0, text.gamma_multiply(0.25)),
                                        egui::StrokeKind::Inside,
                                    );
                                    ui.painter().text(
                                        key_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        key,
                                        typography::monospace(typography::XS),
                                        text.gamma_multiply(0.7),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(label)
                                            .color(text_muted)
                                            .font(typography::proportional(typography::XS)),
                                    );
                                };

                                draw_keycap(ui, "h", "");
                                draw_keycap(ui, "l", "switch");
                                ui.add_space(12.0);
                                draw_keycap(ui, "j", "");
                                draw_keycap(ui, "k", "navigate");
                                ui.add_space(12.0);
                                draw_keycap(ui, "⏎", "select");
                                ui.add_space(12.0);
                                draw_keycap(ui, "⎋", "cancel");
                            });
                            ui.add_space(12.0);
                        });
                    });
            });

        result
    }

    /// Renders the theme panel (left side)
    #[allow(clippy::too_many_arguments)]
    fn render_theme_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel_width: f32,
        panel_height: f32,
        theme_entries: &[ThemeEntry],
        accent: Color32,
        text: Color32,
        text_muted: Color32,
        bg_hover: Color32,
        style: &OverlayStyle,
        result: &mut StylePickerResult,
    ) {
        let is_focused = self.focused_panel == StyleTab::Theme;
        let row_height = 52.0; // Slightly taller to fit chart palette dots
        let builtin_count = AppTheme::all().len();

        // Panel header with icon, title, count, and active indicator
        ui.horizontal(|ui| {
            // Active indicator dot
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
                RichText::new(format!("({})", theme_entries.len()))
                    .color(if is_focused { accent } else { text_muted })
                    .font(typography::monospace(typography::XS)),
            );
        });
        ui.add_space(8.0);

        // Theme list in scroll area
        egui::ScrollArea::vertical()
            .id_salt("theme_scroll")
            .max_height(panel_height - 30.0)
            .auto_shrink([false, true])
            .animated(true)
            .show(ui, |ui| {
                for (i, entry) in theme_entries.iter().enumerate() {
                    // Add separator before custom themes section
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

                    self.render_theme_entry_row(
                        ui,
                        panel_width,
                        row_height,
                        i,
                        entry,
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

    /// Renders a single theme entry row (builtin or custom)
    #[allow(clippy::too_many_arguments)]
    fn render_theme_entry_row(
        &mut self,
        ui: &mut egui::Ui,
        panel_width: f32,
        row_height: f32,
        index: usize,
        entry: &ThemeEntry,
        accent: Color32,
        text: Color32,
        text_muted: Color32,
        bg_hover: Color32,
        style: &OverlayStyle,
        result: &mut StylePickerResult,
        is_focused: bool,
    ) {
        let is_selected = index == self.theme_index;

        // Get theme info based on entry type
        let is_system = matches!(entry, ThemeEntry::Builtin(AppTheme::System));
        let (display_name, preview_colors, chart_palette, is_custom) = match entry {
            ThemeEntry::Builtin(theme) => {
                if *theme == AppTheme::System {
                    // System theme: show split Dark/Light preview
                    let colors = [
                        AppTheme::Dark.bg_base(),
                        AppTheme::Light.bg_base(),
                        AppTheme::Dark.accent_primary(),
                        AppTheme::Light.text_primary(),
                    ];
                    (
                        theme.name().to_string(),
                        colors,
                        Some(AppTheme::Dark.chart_palette()),
                        false,
                    )
                } else {
                    // For builtin themes, create preview colors from the theme
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
                }
            }
            ThemeEntry::Custom {
                display_name,
                colors,
                ..
            } => {
                // For custom themes, use the resolved colors
                let preview = [
                    colors.bg_base,
                    colors.bg_elevated,
                    colors.accent_primary,
                    colors.text_primary,
                ];
                // Custom themes also have chart_palette in ActiveThemeColors
                (
                    display_name.clone(),
                    preview,
                    Some(colors.chart_palette),
                    true,
                )
            }
        };

        let is_original = match entry {
            ThemeEntry::Builtin(t) => {
                *t == self.original_theme && self.original_custom_theme.is_none()
            }
            ThemeEntry::Custom { name, .. } => self.original_custom_theme.as_deref() == Some(name),
        };

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(panel_width, row_height), egui::Sense::click());

        // Scroll into view when navigating to this item
        if is_selected && self.scroll_to_theme {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
            self.scroll_to_theme = false;
        }

        // Handle click
        if response.clicked() {
            self.theme_index = index;
            self.focused_panel = StyleTab::Theme;
            self.close();
            match entry {
                ThemeEntry::Builtin(theme) => {
                    *result = StylePickerResult::ThemeSelected(*theme);
                }
                ThemeEntry::Custom { name, .. } => {
                    *result = StylePickerResult::CustomThemeSelected(name.clone());
                }
            }
            return;
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

        // Color palette bar (UI colors) - use preview_colors
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

        // "follows OS" subtitle for System theme
        if is_system {
            let name_galley = ui.painter().layout_no_wrap(
                display_name.clone(),
                typography::proportional(typography::SM),
                text,
            );
            ui.painter().text(
                egui::pos2(name_x + name_galley.size().x + 6.0, rect.min.y + 16.0),
                egui::Align2::LEFT_CENTER,
                "follows OS",
                typography::monospace(9.0),
                text_muted.gamma_multiply(0.6),
            );
        }

        // Show chart palette dots for all themes, or plugin badge for custom
        let dot_size = 5.0;
        let dot_spacing = 7.0;
        let dots_y = rect.min.y + 34.0;

        if let Some(chart_colors) = chart_palette {
            // Chart palette preview dots (8 colors)
            for (idx, color) in chart_colors.iter().enumerate() {
                let dot_x = palette_x + (idx as f32) * dot_spacing;
                let dot_center = egui::pos2(dot_x + dot_size / 2.0, dots_y);
                ui.painter()
                    .circle_filled(dot_center, dot_size / 2.0, *color);
            }

            // Label: "chart" for builtins, "plugin" for custom
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

    /// Renders the font panel (right side)
    #[allow(clippy::too_many_arguments)]
    fn render_font_panel(
        &mut self,
        ui: &mut egui::Ui,
        panel_width: f32,
        panel_height: f32,
        fonts: &[EditorFont],
        accent: Color32,
        text: Color32,
        text_muted: Color32,
        bg_hover: Color32,
        current_font: EditorFont,
        result: &mut StylePickerResult,
    ) {
        let is_focused = self.focused_panel == StyleTab::Font;
        let row_height = 72.0; // Taller for preview

        // Panel header with icon, title, count, and active indicator
        ui.horizontal(|ui| {
            // Active indicator dot
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
                RichText::new(format!("({})", fonts.len()))
                    .color(if is_focused { accent } else { text_muted })
                    .font(typography::monospace(typography::XS)),
            );
        });
        ui.add_space(8.0);

        // Font list in scroll area
        egui::ScrollArea::vertical()
            .id_salt("font_scroll")
            .max_height(panel_height - 30.0)
            .auto_shrink([false, true])
            .animated(true)
            .show(ui, |ui| {
                for (i, font) in fonts.iter().enumerate() {
                    let is_selected = i == self.font_index;
                    let is_current_font = *font == current_font;

                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(panel_width, row_height),
                        egui::Sense::click(),
                    );

                    // Scroll into view when navigating to this item (not every frame)
                    if is_selected && self.scroll_to_font {
                        ui.scroll_to_rect(rect, Some(egui::Align::Center));
                        self.scroll_to_font = false;
                    }

                    // Handle click
                    if response.clicked() {
                        self.font_index = i;
                        self.focused_panel = StyleTab::Font;
                        let selected = fonts[i];
                        self.close();
                        *result = StylePickerResult::FontSelected(selected);
                        return;
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
                    if is_current_font {
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

                    // Font preview - sample text with background, using the actual font
                    let preview_y = top_y + 34.0;
                    let preview_rect = egui::Rect::from_min_size(
                        egui::pos2(text_x, preview_y - 6.0),
                        Vec2::new(panel_width - 20.0, 20.0),
                    );
                    ui.painter()
                        .rect_filled(preview_rect, 4.0, text.gamma_multiply(0.05));

                    // Create a FontId using this specific font's family
                    let font_family = FontFamily::Name(font.font_family_name().into());
                    let preview_font = FontId::new(typography::SM, font_family);

                    // Draw Rust code preview with syntax highlighting colors
                    // `let result: Option<i32> = Some(42);`
                    let x = text_x + 8.0;
                    let y = preview_y + 4.0;

                    // Helper to draw text and return new x position
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

                    // Rust syntax: `let result: Option<i32> = Some(42);`
                    let keyword_color = accent; // Keywords in accent
                    let normal_color = text.gamma_multiply(0.7);
                    let type_color = accent.gamma_multiply(0.8); // Types slightly muted
                    let number_color = text.gamma_multiply(0.9); // Numbers bright

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
}
