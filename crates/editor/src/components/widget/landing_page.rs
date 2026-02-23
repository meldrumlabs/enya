use egui::{Color32, RichText, Vec2};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::tinted_logo::TintedLogo;
use crate::ui::typography;
use crate::util::Instant;

/// Action returned by the landing page
#[derive(Debug, Clone, PartialEq)]
pub enum LandingPageAction {
    /// No action
    None,
    /// Open the interactive tutorial
    OpenTutorial,
    /// Open the settings overlay
    OpenSettings,
    /// Show about/info overlay
    ShowAbout,
    /// Show keyboard shortcuts (which-key)
    ShowShortcuts,
    /// Show plugins overlay
    OpenPlugins,
    /// Create a new project in the sidebar
    CreateProject,
    /// Dismiss the landing page and show the empty workspace
    NewWorkspace,
    /// Show native app info (WASM only)
    ShowNativeAppInfo,
}

/// Number of menu items in the landing page
const NUM_MENU_ITEMS: usize = 6;

/// Animation timing (in seconds)
mod animation {
    /// Characters per second for typewriter effect
    pub const CHARS_PER_SEC: f32 = 60.0;
    /// Cursor blink rate (blinks per second)
    pub const CURSOR_BLINK_RATE: f32 = 2.5;
    /// The cursor character
    pub const CURSOR: &str = "▌";
    /// When the logo appears
    pub const LOGO_START: f32 = 0.0;
    /// When the tagline starts typing
    pub const TAGLINE_START: f32 = 0.1;
    /// When the first menu item starts typing
    pub const MENU_START: f32 = 0.25;
    /// Delay between each menu item
    pub const MENU_STAGGER: f32 = 0.08;
    /// When the footer starts typing
    pub const FOOTER_START: f32 = 0.75;
}

/// Menu item type: (icon, label, shortcut, action_fn)
type MenuItem = (
    &'static str,
    &'static str,
    &'static str,
    fn() -> LandingPageAction,
);

/// The alpha-nvim inspired landing page component
pub struct LandingPage {
    theme: AppTheme,
    /// Currently selected menu item index
    selected_index: usize,
    /// Disable keyboard handling (when an overlay is open)
    keyboard_disabled: bool,
    /// Last known mouse position (to detect actual mouse movement)
    last_mouse_pos: Option<egui::Pos2>,
    /// Cached tinted logo texture
    tinted_logo: TintedLogo,
    /// When the landing page was first shown (for entrance animation)
    first_shown: Option<Instant>,
}

impl Default for LandingPage {
    fn default() -> Self {
        Self::new()
    }
}

impl LandingPage {
    pub fn new() -> Self {
        Self {
            theme: AppTheme::default(),
            selected_index: 0,
            keyboard_disabled: false,
            last_mouse_pos: None,
            tinted_logo: TintedLogo::new(),
            first_shown: None,
        }
    }

    /// Typewriter effect: returns the visible portion of text based on elapsed time
    fn typewriter<'a>(&self, text: &'a str, start_time: f32) -> &'a str {
        let elapsed = self
            .first_shown
            .map(|t| t.elapsed().as_secs_f32())
            .unwrap_or(0.0);
        let time_since_start = (elapsed - start_time).max(0.0);
        let char_count = (time_since_start * animation::CHARS_PER_SEC) as usize;

        // Find byte index for the nth character (Unicode-safe)
        text.char_indices()
            .nth(char_count)
            .map(|(i, _)| &text[..i])
            .unwrap_or(text)
    }

    /// Check if text is still being typed (not yet complete)
    fn is_typing(&self, text: &str, start_time: f32) -> bool {
        let elapsed = self
            .first_shown
            .map(|t| t.elapsed().as_secs_f32())
            .unwrap_or(0.0);
        let time_since_start = (elapsed - start_time).max(0.0);
        let char_count = (time_since_start * animation::CHARS_PER_SEC) as usize;
        char_count < text.chars().count()
    }

    /// Get the blinking cursor if it should be visible
    fn cursor(&self) -> &'static str {
        let elapsed = self
            .first_shown
            .map(|t| t.elapsed().as_secs_f32())
            .unwrap_or(0.0);
        // Blink on/off based on time
        let blink_cycle = elapsed * animation::CURSOR_BLINK_RATE;
        if blink_cycle.fract() < 0.5 {
            animation::CURSOR
        } else {
            ""
        }
    }

    /// Check if element has started appearing
    fn is_visible(&self, start_time: f32) -> bool {
        let elapsed = self
            .first_shown
            .map(|t| t.elapsed().as_secs_f32())
            .unwrap_or(0.0);
        elapsed >= start_time
    }

    /// Disable keyboard handling (call when an overlay is open over the landing page)
    pub fn set_keyboard_disabled(&mut self, disabled: bool) {
        self.keyboard_disabled = disabled;
    }

    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Show the landing page UI
    #[profiling::function]
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> LandingPageAction {
        // Initialize animation timer on first show
        if self.first_shown.is_none() {
            self.first_shown = Some(Instant::now());
        }

        // Request repaint for animations (typewriter + ambient glow)
        // Always repaint since ambient glow is continuously animated
        ctx.request_repaint();

        // Handle keyboard navigation
        let action = self.handle_keyboard(ctx);
        if action != LandingPageAction::None {
            return action;
        }
        let mut action = LandingPageAction::None;

        // Detect if mouse actually moved (to avoid hover overriding keyboard navigation)
        let current_mouse_pos = ctx.input(|i| i.pointer.hover_pos());
        let mouse_moved = match (self.last_mouse_pos, current_mouse_pos) {
            (Some(last), Some(current)) => (last - current).length() > 1.0,
            (None, Some(_)) => true, // First frame with mouse position
            _ => false,
        };
        self.last_mouse_pos = current_mouse_pos;

        let text_col = self.theme.text_primary();
        let accent_color = self.theme.accent_primary();
        let muted_color = self.theme.text_tertiary();

        // Responsive layout that scales to fit any screen without scrolling
        let available_height = ui.available_height();

        // Calculate the unscaled content height to determine required scale
        // Header: logo(160) + spacing(12) + tagline(~17) + spacing(8) + version(~15) = ~212
        //   WASM adds: spacing(8) + native_app_link(~15) = +23
        // Header spacing: 32
        // Menu: 6 items * (48 + 8) = 336
        // Footer spacing: 16
        // Footer: hints(~16) + spacing(12) + credits(~15) + spacing(4) + memorial(~14) = ~61
        // Margins: 32 (frame) + some padding
        // Total unscaled: ~689 (non-WASM), ~712 (WASM)
        #[cfg(target_arch = "wasm32")]
        const UNSCALED_CONTENT_HEIGHT: f32 = 720.0;
        #[cfg(not(target_arch = "wasm32"))]
        const UNSCALED_CONTENT_HEIGHT: f32 = 690.0;

        // Calculate scale to fit content with some breathing room (16px top + 16px bottom)
        let target_height = available_height - 32.0;
        let scale = (target_height / UNSCALED_CONTENT_HEIGHT).clamp(0.5, 1.0);

        // Scaled spacing values
        let header_spacing = 32.0 * scale;
        let footer_spacing = 16.0 * scale;

        // Actual content height after scaling
        let content_height = UNSCALED_CONTENT_HEIGHT * scale;

        // Center vertically with slight upward shift (35% from top)
        let top_padding = ((available_height - content_height) * 0.35).clamp(4.0, 80.0);

        egui::Frame {
            inner_margin: egui::Margin::same((16.0 * scale) as i8),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(top_padding);

                // === HEADER SECTION ===
                let header_action = self.show_header_scaled(ui, ctx, muted_color, scale);
                if header_action != LandingPageAction::None {
                    action = header_action;
                }

                ui.add_space(header_spacing);

                // === MENU BUTTONS (Vertical list) ===
                let menu_action =
                    self.show_menu_scaled(ui, text_col, accent_color, mouse_moved, scale);
                if menu_action != LandingPageAction::None {
                    action = menu_action;
                }

                ui.add_space(footer_spacing);

                // === FOOTER ===
                self.show_footer_scaled(ui, muted_color, scale);
            });
        });

        action
    }

    /// Show the header with logo and tagline (scaled version)
    /// Returns an action if the native app link was clicked (WASM only)
    fn show_header_scaled(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        muted_color: Color32,
        scale: f32,
    ) -> LandingPageAction {
        let logo_size = 160.0 * scale;

        // Logo appears instantly when its time comes
        // Theme already carries Custom variant if plugin theme is active
        if self.is_visible(animation::LOGO_START) {
            let texture = self.tinted_logo.get(ctx, self.theme);
            let logo = egui::Image::from_texture(egui::load::SizedTexture::from_handle(texture));
            ui.add(logo.max_width(logo_size).max_height(logo_size));
        } else {
            // Reserve space for logo before it appears
            ui.allocate_space(Vec2::splat(logo_size));
        }

        ui.add_space(12.0 * scale);

        // Tagline with typewriter + cursor
        let tagline = "A Builder's Best Friend";
        let tagline_start = animation::TAGLINE_START;
        let visible_tagline = self.typewriter(tagline, tagline_start);
        let tagline_cursor = if self.is_typing(tagline, tagline_start) {
            self.cursor()
        } else {
            ""
        };
        ui.label(
            RichText::new(format!("{visible_tagline}{tagline_cursor}"))
                .size(typography::LG * scale)
                .color(muted_color),
        );

        ui.add_space(8.0 * scale);

        // Version badge with typewriter + cursor
        let version = format!("Enya [ v{} ]", env!("CARGO_PKG_VERSION"));
        let version_start = tagline_start + 0.4;
        let visible_version = self.typewriter(&version, version_start);
        let version_cursor = if self.is_typing(&version, version_start) {
            self.cursor()
        } else {
            ""
        };
        ui.label(
            RichText::new(format!("{visible_version}{version_cursor}"))
                .size(typography::SM * scale)
                .color(muted_color.gamma_multiply(0.7)),
        );

        // On WASM, show a subtle native app notification below version
        #[cfg(target_arch = "wasm32")]
        {
            let accent = self.theme.accent_primary();
            ui.add_space(8.0 * scale);
            let wasm_text = format!(
                "{}  Download Native App for full features",
                semantic_icons::action::IMPORT
            );
            let wasm_start = version_start + 0.2;
            let visible_wasm = self.typewriter(&wasm_text, wasm_start);
            let wasm_cursor = if self.is_typing(&wasm_text, wasm_start) {
                self.cursor()
            } else {
                ""
            };
            let sense = if self.keyboard_disabled {
                egui::Sense::hover()
            } else {
                egui::Sense::click()
            };
            let response = ui.add(
                egui::Label::new(
                    RichText::new(format!("{}{}", visible_wasm, wasm_cursor))
                        .size(typography::SM * scale)
                        .color(accent.gamma_multiply(0.7)),
                )
                .sense(sense),
            );

            if !self.keyboard_disabled && response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            if !self.keyboard_disabled && response.clicked() {
                return LandingPageAction::ShowNativeAppInfo;
            }
        }

        LandingPageAction::None
    }

    /// Show the vertical menu buttons (scaled version)
    fn show_menu_scaled(
        &mut self,
        ui: &mut egui::Ui,
        text_col: Color32,
        accent_color: Color32,
        mouse_moved: bool,
        scale: f32,
    ) -> LandingPageAction {
        let mut action = LandingPageAction::None;

        // Menu items: (icon, label, shortcut, action)
        let menu_items: [MenuItem; NUM_MENU_ITEMS] = [
            (
                semantic_icons::file::FOLDER_PLUS,
                "Create project",
                "n",
                || LandingPageAction::CreateProject,
            ),
            (semantic_icons::nav::FORWARD, "Get started", "e", || {
                LandingPageAction::NewWorkspace
            }),
            (semantic_icons::diagnostic::HINT, "Tutorial", "t", || {
                LandingPageAction::OpenTutorial
            }),
            (semantic_icons::action::SETTINGS, "Settings", "s", || {
                LandingPageAction::OpenSettings
            }),
            (semantic_icons::action::TOOL, "Plugins", "p", || {
                LandingPageAction::OpenPlugins
            }),
            (semantic_icons::status::INFO, "About", "a", || {
                LandingPageAction::ShowAbout
            }),
        ];

        let button_width = 440.0 * scale;
        let item_height = 48.0 * scale;
        let item_spacing = 8.0 * scale;

        for (idx, (icon, label, shortcut, action_fn)) in menu_items.iter().enumerate() {
            let is_selected = self.selected_index == idx;

            // Staggered typewriter for each menu item
            let item_start = animation::MENU_START + (idx as f32 * animation::MENU_STAGGER);
            let visible_label = self.typewriter(label, item_start);
            let label_cursor = if self.is_typing(label, item_start) {
                self.cursor()
            } else {
                ""
            };
            let label_with_cursor = format!("{visible_label}{label_cursor}");

            // Only show item once it starts typing
            if !self.is_visible(item_start) {
                ui.allocate_space(Vec2::new(button_width, item_height));
                ui.add_space(item_spacing);
                continue;
            }

            let response = self.show_menu_item_scaled(
                ui,
                icon,
                &label_with_cursor,
                shortcut,
                text_col,
                accent_color,
                is_selected,
                button_width,
                item_height,
                scale,
            );

            if !self.keyboard_disabled && response.clicked() {
                action = action_fn();
            }

            // Only update selection on hover if mouse actually moved and no overlay is blocking
            if !self.keyboard_disabled && response.hovered() && !is_selected && mouse_moved {
                self.selected_index = idx;
            }

            // Small gap between items
            ui.add_space(item_spacing);
        }

        action
    }

    /// Show a single menu item button (scaled version)
    #[allow(clippy::too_many_arguments)]
    fn show_menu_item_scaled(
        &self,
        ui: &mut egui::Ui,
        icon: &str,
        label: &str,
        shortcut: &str,
        text_col: Color32,
        accent_color: Color32,
        is_selected: bool,
        width: f32,
        height: f32,
        scale: f32,
    ) -> egui::Response {
        // When keyboard is disabled (overlay open), don't accept clicks
        let sense = if self.keyboard_disabled {
            egui::Sense::hover()
        } else {
            egui::Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), sense);

        // Background on hover/select
        let bg_color = if is_selected {
            accent_color.gamma_multiply(0.12)
        } else if response.hovered() {
            text_col.gamma_multiply(0.05)
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 8.0 * scale, bg_color);
        }

        // Icon (left side)
        let icon_color = if is_selected || response.hovered() {
            accent_color
        } else {
            text_col.gamma_multiply(0.6)
        };

        ui.painter().text(
            egui::pos2(rect.min.x + 20.0 * scale, rect.center().y),
            egui::Align2::LEFT_CENTER,
            icon,
            egui::FontId::proportional(semantic_icons::SIZE_HEADER * scale),
            icon_color,
        );

        // Label (center-left)
        let label_color = if is_selected { accent_color } else { text_col };

        ui.painter().text(
            egui::pos2(rect.min.x + 56.0 * scale, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            typography::proportional(typography::XL * scale),
            label_color,
        );

        // Shortcut hint (right side) - plain monospace
        let shortcut_color = if is_selected || response.hovered() {
            accent_color.gamma_multiply(0.7)
        } else {
            text_col.gamma_multiply(0.4)
        };

        ui.painter().text(
            egui::pos2(rect.max.x - 20.0 * scale, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            shortcut,
            egui::FontId::monospace(typography::MD * scale),
            shortcut_color,
        );

        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// Show the footer with keyboard hints (scaled version)
    fn show_footer_scaled(&self, ui: &mut egui::Ui, muted_color: Color32, scale: f32) {
        // Keyboard hints with typewriter + cursor
        let hints = "j/k navigate  •  Enter select  •  : commands  •  ? help";
        let visible_hints = self.typewriter(hints, animation::FOOTER_START);
        let hints_cursor = if self.is_typing(hints, animation::FOOTER_START) {
            self.cursor()
        } else {
            ""
        };
        ui.label(
            RichText::new(format!("{visible_hints}{hints_cursor}"))
                .size(typography::MD * scale)
                .color(muted_color.gamma_multiply(0.7)),
        );

        ui.add_space(12.0 * scale);

        // Credits with typewriter + cursor
        let credits = "Crafted in Stockholm";
        let credits_start = animation::FOOTER_START + 0.8;
        let visible_credits = self.typewriter(credits, credits_start);
        let credits_cursor = if self.is_typing(credits, credits_start) {
            self.cursor()
        } else {
            ""
        };
        ui.label(
            RichText::new(format!("{visible_credits}{credits_cursor}"))
                .size(typography::SM * scale)
                .color(muted_color.gamma_multiply(0.5)),
        );

        ui.add_space(4.0 * scale);

        // Memorial with typewriter + cursor
        let memorial = "In memory of Enya \u{2014} the family dog";
        let memorial_start = credits_start + 0.8;
        let visible_memorial = self.typewriter(memorial, memorial_start);
        let memorial_cursor = if self.is_typing(memorial, memorial_start) {
            self.cursor()
        } else {
            ""
        };
        ui.label(
            RichText::new(format!("{visible_memorial}{memorial_cursor}"))
                .size(typography::XS * scale)
                .color(muted_color.gamma_multiply(0.35)),
        );
    }

    /// Handle keyboard navigation
    fn handle_keyboard(&mut self, ctx: &egui::Context) -> LandingPageAction {
        // Don't handle keys if keyboard is disabled (overlay is open)
        if self.keyboard_disabled {
            return LandingPageAction::None;
        }

        // Don't handle keys if a text field has focus
        if ctx.memory(|mem| mem.focused().is_some()) {
            return LandingPageAction::None;
        }

        let mut action = LandingPageAction::None;

        ctx.input_mut(|input| {
            // n - Create project
            if input.consume_key(egui::Modifiers::NONE, egui::Key::N) {
                action = LandingPageAction::CreateProject;
                return;
            }

            // e - Get started (open editor)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::E) {
                action = LandingPageAction::NewWorkspace;
                return;
            }

            // t - Tutorial
            if input.consume_key(egui::Modifiers::NONE, egui::Key::T) {
                action = LandingPageAction::OpenTutorial;
                return;
            }

            // s - Settings
            if input.consume_key(egui::Modifiers::NONE, egui::Key::S) {
                action = LandingPageAction::OpenSettings;
                return;
            }

            // ? - Shortcuts (check for '?' character in text input, or Shift+/)
            let has_question_mark = input
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "?"));
            if has_question_mark || input.consume_key(egui::Modifiers::SHIFT, egui::Key::Slash) {
                input
                    .events
                    .retain(|e| !matches!(e, egui::Event::Text(t) if t == "?"));
                action = LandingPageAction::ShowShortcuts;
                return;
            }

            // p - Plugins
            if input.consume_key(egui::Modifiers::NONE, egui::Key::P) {
                action = LandingPageAction::OpenPlugins;
                return;
            }

            // a - About
            if input.consume_key(egui::Modifiers::NONE, egui::Key::A) {
                action = LandingPageAction::ShowAbout;
                return;
            }

            // j/Down - Move down in menu
            if input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                self.selected_index = (self.selected_index + 1) % NUM_MENU_ITEMS;
                return;
            }

            // k/Up - Move up in menu
            if input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                self.selected_index = if self.selected_index == 0 {
                    NUM_MENU_ITEMS - 1
                } else {
                    self.selected_index - 1
                };
                return;
            }

            // Enter - Select current menu item
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                action = match self.selected_index {
                    0 => LandingPageAction::CreateProject,
                    1 => LandingPageAction::NewWorkspace,
                    2 => LandingPageAction::OpenTutorial,
                    3 => LandingPageAction::OpenSettings,
                    4 => LandingPageAction::OpenPlugins,
                    5 => LandingPageAction::ShowAbout,
                    _ => LandingPageAction::None,
                };
            }
        });

        action
    }
}
