use egui::Color32;
use icons::Icon;

pub mod active_theme;
pub mod chat_colors;
pub mod colors;
pub mod custom_theme;
pub mod design;
pub mod icons;
pub mod palette;
pub mod semantic_icons;
pub mod settings_screen;
pub mod theme;
pub mod tinted_logo;
pub mod typography;
pub mod welcome_screen;

pub use active_theme::ActiveThemeColors;
pub use chat_colors::ChatColors;
pub use custom_theme::{CustomThemeStore, ResolvedCustomTheme};
pub use theme::AppTheme;

pub trait UiExt {
    fn ui(&self) -> &egui::Ui;
    fn ui_mut(&mut self) -> &mut egui::Ui;

    fn bullet(&mut self, color: Color32) {
        let ui = self.ui_mut();
        static DIAMETER: f32 = 6.0;
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(DIAMETER, DIAMETER), egui::Sense::hover());

        ui.painter().add(egui::epaint::CircleShape {
            center: rect.center(),
            radius: DIAMETER / 2.0,
            fill: color,
            stroke: egui::Stroke::NONE,
        });
    }

    fn small_icon_button(&mut self, icon: &Icon) -> egui::Response {
        let widget = self.small_icon_button_widget(icon);
        self.ui_mut().add(widget)
    }

    fn small_icon_button_widget<'a>(&self, icon: &'a Icon) -> egui::Button<'a> {
        egui::Button::image(
            icon.as_image()
                .tint(self.ui().visuals().widgets.inactive.fg_stroke.color),
        )
    }
}

impl UiExt for egui::Ui {
    #[inline]
    fn ui(&self) -> &egui::Ui {
        self
    }

    #[inline]
    fn ui_mut(&mut self) -> &mut egui::Ui {
        self
    }
}
