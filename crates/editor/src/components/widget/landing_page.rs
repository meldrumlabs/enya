use egui::{Color32, RichText, Vec2};

use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::tinted_logo::TintedLogo;
use crate::ui::typography;

/// Action returned by the landing page
#[derive(Debug, Clone, PartialEq)]
pub enum LandingPageAction {
    /// No action
    None,
    /// Open the workspace finder
    OpenWorkspaceFinder,
    /// Create a new workspace
    CreateWorkspace,
    /// Open the interactive tutorial
    OpenTutorial,
    /// Open the documentation website
    OpenDocs,
    /// Show about/info overlay
    ShowAbout,
    /// Show keyboard shortcuts (which-key)
    ShowShortcuts,
}

/// Number of menu items in the landing page
const NUM_MENU_ITEMS: usize = 6;

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
        }
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

        let text_col = text_color(self.theme);
        let accent_color = self.accent_color();
        let muted_color = text_col.gamma_multiply(0.5);

        // Responsive layout that scales to fit any screen without scrolling
        let available_height = ui.available_height();

        // Calculate the unscaled content height to determine required scale
        // Header: logo(160) + spacing(12) + title(42) + spacing(6) + tagline(20) + spacing(8) + version(14) = 262
        // Header spacing: 32
        // Menu: 6 items * (48 + 8) = 336
        // Footer spacing: 16
        // Footer: hints(16) + spacing(12) + credits(12) = 40
        // Margins: 32 (frame) + some padding
        // Total unscaled: ~720
        const UNSCALED_CONTENT_HEIGHT: f32 = 720.0;

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
                self.show_header_scaled(ui, ctx, muted_color, scale);

                ui.add_space(header_spacing);

                // === MENU BUTTONS (Vertical list) ===
                action = self.show_menu_scaled(ui, text_col, accent_color, mouse_moved, scale);

                ui.add_space(footer_spacing);

                // === FOOTER ===
                self.show_footer_scaled(ui, muted_color, scale);
            });
        });

        action
    }

    /// Show the header with logo and title (scaled version)
    fn show_header_scaled(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        muted_color: Color32,
        scale: f32,
    ) {
        let accent = self.accent_color();
        let logo_size = 160.0 * scale;
        let title_size = 42.0 * scale;

        // Get the overlay-blended tinted logo (cached per theme)
        let texture = self.tinted_logo.get(ctx, self.theme);
        let logo = egui::Image::from_texture(egui::load::SizedTexture::from_handle(texture));
        ui.add(logo.max_width(logo_size).max_height(logo_size));

        ui.add_space(12.0 * scale);

        // App name in theme accent color
        ui.heading(
            RichText::new("ENYA")
                .strong()
                .size(title_size)
                .color(accent),
        );

        ui.add_space(6.0 * scale);

        // Tagline
        ui.label(
            RichText::new("A Builder's Best Friend")
                .size(typography::LG * scale)
                .color(muted_color),
        );

        ui.add_space(8.0 * scale);

        // Version badge - ASCII box style: [ v0.1.0 ]
        let version = format!("[ v{} ]", env!("CARGO_PKG_VERSION"));
        ui.label(
            RichText::new(version)
                .size(typography::SM * scale)
                .color(muted_color.gamma_multiply(0.7)),
        );
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
                semantic_icons::file::FOLDER_OPEN,
                "Find workspace",
                "w",
                || LandingPageAction::OpenWorkspaceFinder,
            ),
            (semantic_icons::action::ADD, "Create workspace", "n", || {
                LandingPageAction::CreateWorkspace
            }),
            (semantic_icons::diagnostic::HINT, "Tutorial", "t", || {
                LandingPageAction::OpenTutorial
            }),
            (semantic_icons::file::TEXT, "Docs", "d", || {
                LandingPageAction::OpenDocs
            }),
            (semantic_icons::keyboard::KEYBOARD, "Shortcuts", "?", || {
                LandingPageAction::ShowShortcuts
            }),
            (semantic_icons::status::INFO, "About", "i", || {
                LandingPageAction::ShowAbout
            }),
        ];

        let button_width = 440.0 * scale;
        let item_height = 48.0 * scale;
        let item_spacing = 8.0 * scale;

        for (idx, (icon, label, shortcut, action_fn)) in menu_items.iter().enumerate() {
            let is_selected = self.selected_index == idx;

            let response = self.show_menu_item_scaled(
                ui,
                icon,
                label,
                shortcut,
                text_col,
                accent_color,
                is_selected,
                button_width,
                item_height,
                scale,
            );

            if response.clicked() {
                action = action_fn();
            }

            // Only update selection on hover if mouse actually moved
            if response.hovered() && !is_selected && mouse_moved {
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
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());

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

        // Shortcut hint (right side)
        let shortcut_color = if is_selected || response.hovered() {
            accent_color.gamma_multiply(0.7)
        } else {
            text_col.gamma_multiply(0.4)
        };

        ui.painter().text(
            egui::pos2(rect.max.x - 20.0 * scale, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            shortcut,
            typography::proportional(typography::LG * scale),
            shortcut_color,
        );

        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// Show the footer with keyboard hints (scaled version)
    fn show_footer_scaled(&self, ui: &mut egui::Ui, muted_color: Color32, scale: f32) {
        // Keyboard hints
        ui.label(
            RichText::new("j/k navigate  •  Enter select  •  : commands")
                .size(typography::MD * scale)
                .color(muted_color.gamma_multiply(0.7)),
        );

        ui.add_space(12.0 * scale);

        // Credits
        ui.label(
            RichText::new("Developed by Meldrum Labs")
                .size(typography::SM * scale)
                .color(muted_color.gamma_multiply(0.5)),
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
            // w - Find workspace
            if input.consume_key(egui::Modifiers::NONE, egui::Key::W) {
                action = LandingPageAction::OpenWorkspaceFinder;
                return;
            }

            // n - Create workspace
            if input.consume_key(egui::Modifiers::NONE, egui::Key::N) {
                action = LandingPageAction::CreateWorkspace;
                return;
            }

            // t - Tutorial
            if input.consume_key(egui::Modifiers::NONE, egui::Key::T) {
                action = LandingPageAction::OpenTutorial;
                return;
            }

            // d - Docs
            if input.consume_key(egui::Modifiers::NONE, egui::Key::D) {
                action = LandingPageAction::OpenDocs;
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

            // i - About
            if input.consume_key(egui::Modifiers::NONE, egui::Key::I) {
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
                    0 => LandingPageAction::OpenWorkspaceFinder,
                    1 => LandingPageAction::CreateWorkspace,
                    2 => LandingPageAction::OpenTutorial,
                    3 => LandingPageAction::OpenDocs,
                    4 => LandingPageAction::ShowShortcuts,
                    5 => LandingPageAction::ShowAbout,
                    _ => LandingPageAction::None,
                };
            }
        });

        action
    }

    /// Get the accent color based on theme (Enya's emerald brand color)
    fn accent_color(&self) -> Color32 {
        self.theme.accent_primary()
    }
}
