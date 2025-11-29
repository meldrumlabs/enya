use egui::RichText;
use egui::Theme;
use egui::Visuals;
use enya_build_info::BuildInfo;
use enya_build_info::build_info;

use crate::command::CommandReceiver;
use crate::command::CommandSender;
use crate::command::UICommand;
use crate::command::UICommandSender;
use crate::command::command_channel;
use crate::dashboard::Dashboard;
use crate::theme::AppTheme;
use crate::theme::light;
use crate::ui::colors::text_color;
use crate::ui::design::black_theme;
use crate::ui::settings_screen::AppSettings;
use crate::ui::settings_screen::show_settings_ui;
use crate::ui::welcome_screen::welcome_section_ui;

/// The core App
pub struct EnyaApp {
    state: AppState,

    dashboard: Option<Dashboard>,

    is_connected: bool,

    build_info: BuildInfo,

    // Channels for ui commands
    pub command_sender: CommandSender,
    pub command_receiver: CommandReceiver,
}

// Serializable state that can be persiste
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    pub(crate) settings: AppSettings,
    /// Current active Theme
    pub(crate) theme: AppTheme,
    pub(crate) ui_state: UIState,
    pub(crate) prev_ui_state: UIState,
    #[serde(skip)]
    pub(crate) active_dashboard: Dashboard,
}

impl AppState {
    /// Returns the current App theme visuals
    fn visuals(&self) -> Visuals {
        match self.theme {
            AppTheme::Light => light(),
            AppTheme::Dark => black_theme(),
        }
    }
    /// Returns the current previous UIState
    fn ui_state(&self) -> &UIState {
        &self.ui_state
    }
    /// Returns the previous UIState
    fn prev_ui_state(&self) -> &UIState {
        &self.prev_ui_state
    }
}

/// Which current state the UI is in
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub enum UIState {
    Settings,
    Dashboard,
    #[default]
    Home,
}

impl Default for EnyaApp {
    fn default() -> Self {
        let (command_sender, command_receiver) = command_channel();
        Self {
            dashboard: None,
            command_sender,
            command_receiver,
            state: AppState::default(),
            is_connected: false,
            build_info: build_info!(),
        }
    }
}

impl EnyaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Thin);

        cc.egui_ctx.set_fonts(fonts);

        replace_fonts(&cc.egui_ctx);

        let mut app = Self::default();

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            let state: AppState = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app.state = state;
        }

        match cc.egui_ctx.theme() {
            Theme::Light => app.state.theme = AppTheme::Light,
            Theme::Dark => app.state.theme = AppTheme::Dark,
        }

        app
    }

    // This paints the top panel aka header
    fn show_top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.horizontal(|ui| {
                    self.menu_button_ui(ui);

                    ui.add_space(12.0);

                    ui.separator();
                    egui::warn_if_debug_build(ui);
                });
            });
        });
    }

    // Paints the menu button at the header top left
    pub fn menu_button_ui(&mut self, ui: &mut egui::Ui) {
        pub fn small_icon_size() -> egui::Vec2 {
            egui::Vec2::splat(24.0)
        }

        let icon = crate::ui::icons::ICON_COLOR;
        let image = icon.as_image().fit_to_exact_size(small_icon_size());

        ui.menu_image_button(image, |ui| {
            self.menu_ui(ui);
        });
    }

    // List of commands under the menu button
    fn menu_ui(&mut self, ui: &mut egui::Ui) {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        let theme = self.state.theme;

        // Open Home
        UICommand::Home.menu_button_ui(ui, theme, &self.command_sender);

        // Open dashboard
        UICommand::Open.menu_button_ui(ui, theme, &self.command_sender);

        // Settings
        UICommand::Settings.menu_button_ui(ui, theme, &self.command_sender);

        ui.add_space(12.0);
        // Get Help
        UICommand::Help.menu_button_ui(ui, theme, &self.command_sender);
    }

    fn check_keyboard_shortcuts(&self, egui_ctx: &egui::Context) {
        if let Some(cmd) = UICommand::listen_for_kb_shortcut(egui_ctx) {
            self.command_sender.send_ui(cmd);
        }
    }

    // Paints the bottom panel aka footer
    fn show_bottom_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            let color = text_color(self.state.theme);
            ui.horizontal(|ui| {
                ui.separator();
                ui.label(
                    RichText::new(egui_phosphor::regular::NETWORK_X)
                        .color(color)
                        .strong(),
                );
                ui.separator();
                if self.is_connected {
                    ui.label(RichText::new("CONNECTED").color(color).strong());
                } else if ui
                    .label(RichText::new("NOT CONNECTED").color(color).strong())
                    .on_hover_and_drag_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    // Make settings pop up
                    self.command_sender.send_ui(UICommand::Settings);
                }
                ui.separator();
            });
        });
    }

    // This draws the main panel
    #[inline]
    fn show_main_content(&mut self, ctx: &egui::Context) {
        match self.state.ui_state() {
            UIState::Settings => self.draw_settings(ctx),
            UIState::Dashboard => self.draw_dashboard(ctx),
            UIState::Home => self.draw_home(ctx),
        }
    }

    // Receive UI Commands and handle them
    fn run_pending_ui_commands(&mut self, egui_ctx: &egui::Context) {
        while let Some(cmd) = self.command_receiver.recv_ui() {
            self.run_ui_command(egui_ctx, cmd);
        }
    }
    // updates UI state both current and previous
    fn run_ui_command(&mut self, egui_ctx: &egui::Context, cmd: UICommand) {
        let ui_state = self.state.ui_state();
        let prev_ui_state = self.state.prev_ui_state();
        match cmd {
            UICommand::Home => {
                self.state.ui_state = UIState::Home;
                self.state.prev_ui_state = UIState::Home;
            }
            UICommand::Settings => {
                if let UIState::Settings = prev_ui_state {
                    self.state.prev_ui_state = UIState::Dashboard;
                } else {
                    self.state.prev_ui_state = *ui_state;
                }

                self.state.ui_state = UIState::Settings;
            }
            UICommand::CloseSettings => {
                let old = *ui_state;
                self.state.ui_state = UIState::Dashboard;
                self.state.prev_ui_state = old;
            }
            UICommand::Open => {
                self.state.prev_ui_state = self.state.ui_state;
                self.state.ui_state = UIState::Dashboard;
            }

            UICommand::Help => {
                egui_ctx.open_url(egui::output::OpenUrl {
                    url: "https://enya.dev/contact".to_owned(),
                    new_tab: true,
                });
            }
            UICommand::OpenExampleDashboard(_id) => {
                self.state.prev_ui_state = self.state.ui_state;
                self.state.ui_state = UIState::Dashboard;
            }
            UICommand::Theme(theme) => {
                self.state.theme = theme;
                egui_ctx.set_visuals(self.state.visuals());
                egui_ctx.request_repaint();
            }
            UICommand::ToggleTheme => {
                let new_theme = match self.state.theme {
                    AppTheme::Light => AppTheme::Dark,
                    AppTheme::Dark => AppTheme::Light,
                };
                self.state.theme = new_theme;
                egui_ctx.set_visuals(self.state.visuals());
                egui_ctx.request_repaint();
            }

            UICommand::ConnectionStatus(connected) => {
                self.is_connected = connected;
                // trigger repaint to illustrate the connection status
                egui_ctx.request_repaint();
            }
        }
    }

    #[inline]
    fn draw_settings(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            show_settings_ui(ui, self.build_info, &mut self.state, &self.command_sender);
        });
    }
    #[inline]
    fn draw_home(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            welcome_section_ui(ui, &self.state, &self.command_sender);
        });
    }

    fn draw_dashboard(&mut self, ctx: &egui::Context) {
        if self.dashboard.is_none() {
            self.dashboard = Some(Dashboard::example(self.state.settings.api_key.clone()));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Safe since we initialized the example_dashboard
            self.dashboard.as_mut().unwrap().show(ui, &self.state);
        });
    }
}

impl eframe::App for EnyaApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Set theme for the context
        ctx.set_visuals(self.state.visuals());

        // Draw header panel
        self.show_top_panel(ctx);

        // Draw main content
        self.show_main_content(ctx);

        // Draw bottom panel with connection info etc.
        self.show_bottom_panel(ctx);

        // Check for possible key board shortcut triggers
        self.check_keyboard_shortcuts(ctx);

        // Run any pending ui commands which updates internal data before the next frame
        self.run_pending_ui_commands(ctx);
    }
}

fn replace_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "depature_mono".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/DepartureMono-Regular.otf")),
    );

    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "depature_mono".to_owned());

    // Put my font as last fallback for monospace:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("depature_mono".to_owned());

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}
