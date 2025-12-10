use egui::Theme;
use egui::Visuals;

use crate::command::CommandReceiver;
use crate::command::CommandSender;
use crate::command::UICommand;
use crate::command::UICommandSender;
use crate::command::command_channel;
use crate::components::{NotificationManager, Sparkline, StatusLine, StatusMode};
use crate::dashboard::{Dashboard, DashboardAction};
use crate::theme::AppTheme;
use crate::theme::light;
use crate::ui::design::black_theme;
use crate::ui::settings_screen::AppSettings;
use crate::ui::welcome_screen::welcome_section_ui;
use crate::util::Instant;

/// Tracks internal editor metrics for the status line sparkline
struct EditorMetrics {
    /// Recent frame times in milliseconds
    frame_times: std::collections::VecDeque<f64>,
    /// Last frame timestamp
    last_frame: Option<Instant>,
}

impl Default for EditorMetrics {
    fn default() -> Self {
        Self {
            frame_times: std::collections::VecDeque::with_capacity(15),
            last_frame: None,
        }
    }
}

impl EditorMetrics {
    /// Record a new frame and return the frame time in ms
    fn record_frame(&mut self) -> f64 {
        let now = Instant::now();
        let frame_time = if let Some(last) = self.last_frame {
            now.duration_since(last).as_secs_f64() * 1000.0
        } else {
            16.67 // Default ~60fps assumption for first frame
        };
        self.last_frame = Some(now);

        // Keep last 15 frame times
        if self.frame_times.len() >= 15 {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(frame_time);

        frame_time
    }

    /// Get the frame times for sparkline display
    fn frame_times(&self) -> Vec<f64> {
        self.frame_times.iter().copied().collect()
    }

    /// Get current FPS (based on recent frame time)
    fn fps(&self) -> f64 {
        if let Some(&last_time) = self.frame_times.back() {
            if last_time > 0.0 {
                return 1000.0 / last_time;
            }
        }
        60.0
    }
}

/// The core App
pub struct EnyaApp {
    state: AppState,

    dashboard: Option<Dashboard>,

    is_connected: bool,

    // Channels for ui commands
    pub command_sender: CommandSender,
    pub command_receiver: CommandReceiver,

    // Status line component
    status_line: StatusLine,

    // Notification manager
    notifications: NotificationManager,

    // Internal editor metrics (frame times, etc.)
    editor_metrics: EditorMetrics,

    // Pending screenshot path (used when screenshot event arrives)
    #[cfg(not(target_arch = "wasm32"))]
    pending_screenshot_path: Option<String>,

    // Whether we've checked URL for workspace parameter (WASM only)
    #[cfg(target_arch = "wasm32")]
    checked_url_workspace: bool,
}

// Serializable state that can be persisted
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    pub(crate) settings: AppSettings,
    /// Current active Theme
    pub(crate) theme: AppTheme,
    pub(crate) ui_state: UIState,
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
    /// Returns the current UIState
    fn ui_state(&self) -> &UIState {
        &self.ui_state
    }
}

/// Which current state the UI is in
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub enum UIState {
    #[default]
    Dashboard,
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
            status_line: StatusLine::new(),
            notifications: NotificationManager::new(),
            editor_metrics: EditorMetrics::default(),
            #[cfg(not(target_arch = "wasm32"))]
            pending_screenshot_path: None,
            #[cfg(target_arch = "wasm32")]
            checked_url_workspace: false,
        }
    }
}

impl EnyaApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Set up fonts with both DepartureMono and Phosphor icons
        setup_fonts(&cc.egui_ctx);

        let mut app = Self::default();

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            let state: AppState = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
            app.state = state;
        }

        // Always start with Dashboard (ignore persisted ui_state)
        app.state.ui_state = UIState::Dashboard;

        // Ensure the default example workspace exists on first run
        #[cfg(not(target_arch = "wasm32"))]
        Self::ensure_default_workspace();

        match cc.egui_ctx.theme() {
            Theme::Light => app.state.theme = AppTheme::Light,
            Theme::Dark => app.state.theme = AppTheme::Dark,
        }

        app
    }

    fn check_keyboard_shortcuts(&self, egui_ctx: &egui::Context) {
        if let Some(cmd) = UICommand::listen_for_kb_shortcut(egui_ctx) {
            self.command_sender.send_ui(cmd);
        }
    }

    // Paints the bottom panel aka footer (lualine-style status bar)
    fn show_bottom_panel(&mut self, ctx: &egui::Context) {
        // Update status line state
        self.status_line.set_theme(self.state.theme);
        self.status_line.set_connected(self.is_connected);

        // Set mode based on current UI state
        let mode = match self.state.ui_state {
            UIState::Dashboard => {
                // Check if command palette or fuzzy finder is open, or zen/fullscreen mode is active
                if let Some(ref dashboard) = self.dashboard {
                    if dashboard.is_command_palette_open() {
                        StatusMode::Command
                    } else if dashboard.is_metrics_finder_open() {
                        StatusMode::Search
                    } else if dashboard.is_fullscreen() {
                        StatusMode::Fullscreen
                    } else if dashboard.is_zen_mode() {
                        StatusMode::Zen
                    } else {
                        StatusMode::Normal
                    }
                } else {
                    StatusMode::Normal
                }
            }
            UIState::Home => StatusMode::Home,
        };
        self.status_line.set_mode(mode);

        // Set open tabs count from dashboard
        if let Some(ref dashboard) = self.dashboard {
            self.status_line.set_open_tabs(dashboard.open_tabs_count());
            self.status_line
                .set_selected_metric(dashboard.selected_metric());
            self.status_line
                .set_viewport_info(dashboard.viewport_info());
        }

        // Update sparkline with editor frame time metrics
        let frame_times = self.editor_metrics.frame_times();
        if !frame_times.is_empty() {
            let fps = self.editor_metrics.fps();
            let mut sparkline = Sparkline::new(format!("{fps:.0} fps")).with_unit("ms");
            for value in frame_times {
                sparkline.push(value);
            }
            self.status_line.set_sparkline(Some(sparkline));
        }

        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(false)
            .show(ctx, |ui| {
                self.status_line.show(ui);
            });
    }

    // This draws the main panel
    #[inline]
    fn show_main_content(&mut self, ctx: &egui::Context) {
        match self.state.ui_state() {
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
    // updates UI state
    fn run_ui_command(&mut self, egui_ctx: &egui::Context, cmd: UICommand) {
        match cmd {
            UICommand::Home => {
                self.state.ui_state = UIState::Home;
            }
            UICommand::Dashboard => {
                self.state.ui_state = UIState::Dashboard;
            }

            UICommand::Help => {
                egui_ctx.open_url(egui::output::OpenUrl {
                    url: "https://enya.dev/contact".to_owned(),
                    new_tab: true,
                });
            }
            UICommand::OpenExampleDashboard(_id) => {
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

            UICommand::OpenFuzzyFinder => {
                self.open_metrics_finder();
            }

            UICommand::OpenCommandPalette => {
                self.open_command_palette();
            }
        }
    }

    #[inline]
    fn draw_home(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            welcome_section_ui(ui, &self.state);
        });
    }

    fn draw_dashboard(&mut self, ctx: &egui::Context) {
        if self.dashboard.is_none() {
            self.dashboard = Some(Dashboard::example(self.state.settings.api_key.clone()));
        }

        // On WASM, check for workspace or pane parameter in URL on first frame
        #[cfg(target_arch = "wasm32")]
        if !self.checked_url_workspace {
            self.checked_url_workspace = true;
            // Check for full workspace first, then single pane
            if let Some(workspace_param) = Self::get_url_workspace_param() {
                self.load_workspace(&workspace_param);
            } else if let Some(pane_param) = Self::get_url_pane_param() {
                // Single pane uses the same decoder (returns a single-pane workspace)
                self.load_workspace(&pane_param);
            }
        }

        let mut dashboard_action = DashboardAction::None;

        egui::CentralPanel::default().show(ctx, |ui| {
            // Safe since we initialized the example_dashboard
            if let Some(dashboard) = self.dashboard.as_mut() {
                dashboard_action = dashboard.show(ui, ctx, &self.state);
            }
        });

        // Handle actions from the dashboard (e.g., from command palette)
        self.handle_dashboard_action(ctx, dashboard_action);
    }

    fn handle_dashboard_action(&mut self, ctx: &egui::Context, action: DashboardAction) {
        match action {
            DashboardAction::None => {}
            DashboardAction::ToggleTheme => {
                self.command_sender.send_ui(UICommand::ToggleTheme);
            }
            DashboardAction::SetTheme(theme) => {
                self.command_sender.send_ui(UICommand::Theme(theme));
            }
            DashboardAction::ShowHelp => {
                ctx.open_url(egui::output::OpenUrl {
                    url: "https://enya.dev/contact".to_owned(),
                    new_tab: true,
                });
            }
            DashboardAction::Notify { level, message } => {
                use crate::components::{Notification, NotificationLevel};
                let notification_level = match level.to_lowercase().as_str() {
                    "success" | "ok" => NotificationLevel::Success,
                    "warn" | "warning" => NotificationLevel::Warn,
                    "error" | "err" => NotificationLevel::Error,
                    _ => NotificationLevel::Info,
                };
                self.notifications
                    .notify(Notification::new(message, notification_level));
            }
            DashboardAction::TrackRecentPlot {
                name,
                metric_name,
                is_query,
            } => {
                self.state
                    .settings
                    .add_recent_plot(name, metric_name, is_query);
            }
            DashboardAction::TakeScreenshot(path) => {
                // Store the custom path for when the screenshot event arrives
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.pending_screenshot_path = path;
                }
                #[cfg(target_arch = "wasm32")]
                let _ = path; // Path is ignored on WASM (browser handles download location)

                // Request a screenshot from egui
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            DashboardAction::SaveWorkspace(name) => {
                self.save_workspace(name.as_deref());
            }
            DashboardAction::LoadWorkspace(name) => {
                self.load_workspace(&name);
            }
            DashboardAction::ListWorkspaces => {
                self.list_workspaces();
            }
            DashboardAction::ShareWorkspace => {
                self.share_workspace();
            }
            DashboardAction::SharePane(pane_index) => {
                self.share_pane(pane_index);
            }
            DashboardAction::QuitApp => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn open_metrics_finder(&mut self) {
        if let Some(dashboard) = self.dashboard.as_mut() {
            dashboard.open_metrics_finder();
        }
    }

    fn open_command_palette(&mut self) {
        if let Some(dashboard) = self.dashboard.as_mut() {
            dashboard.open_command_palette();
        }
    }

    /// Handle screenshot events from egui
    fn handle_screenshot_events(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    self.save_screenshot(image);
                }
            }
        });
    }

    /// Save a screenshot image to disk
    fn save_screenshot(&mut self, image: &std::sync::Arc<egui::ColorImage>) {
        use crate::components::{Notification, NotificationLevel};
        use crate::util::now_unix_secs;

        // Generate filename with timestamp (works on both native and WASM)
        let timestamp = now_unix_secs();
        let filename = format!("enya_screenshot_{timestamp}.png");

        // Get the save path (custom path or default to Pictures directory)
        #[cfg(not(target_arch = "wasm32"))]
        let save_path = {
            if let Some(custom_path) = self.pending_screenshot_path.take() {
                let path = std::path::PathBuf::from(&custom_path);
                // If it's a directory, append the filename
                if path.is_dir() {
                    path.join(&filename)
                } else {
                    // Use as-is (user specified full path with filename)
                    path
                }
            } else {
                // Default: save to Pictures or home directory
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let pictures_dir = std::path::PathBuf::from(&home).join("Pictures");
                if pictures_dir.exists() {
                    pictures_dir.join(&filename)
                } else {
                    std::path::PathBuf::from(&home).join(&filename)
                }
            }
        };

        // Convert ColorImage to image buffer
        let width = image.width() as u32;
        let height = image.height() as u32;
        let pixels: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
            .collect();

        // Save the image
        match image::RgbaImage::from_raw(width, height, pixels) {
            Some(img_buffer) => {
                #[cfg(not(target_arch = "wasm32"))]
                match img_buffer.save(&save_path) {
                    Ok(()) => {
                        log::info!("Screenshot saved to: {}", save_path.display());
                        self.notifications.notify(Notification::new(
                            format!("Screenshot saved: {}", save_path.display()),
                            NotificationLevel::Success,
                        ));
                    }
                    Err(e) => {
                        log::error!("Failed to save screenshot: {e}");
                        self.notifications.notify(Notification::new(
                            format!("Failed to save screenshot: {e}"),
                            NotificationLevel::Error,
                        ));
                    }
                }

                #[cfg(target_arch = "wasm32")]
                {
                    // For WASM, trigger a browser download
                    match Self::trigger_browser_download(&filename, &img_buffer) {
                        Ok(()) => {
                            log::info!("Screenshot download triggered: {filename}");
                            self.notifications.notify(Notification::new(
                                format!("Screenshot downloading: {filename}"),
                                NotificationLevel::Success,
                            ));
                        }
                        Err(e) => {
                            log::error!("Failed to trigger download: {e}");
                            self.notifications.notify(Notification::new(
                                format!("Failed to download screenshot: {e}"),
                                NotificationLevel::Error,
                            ));
                        }
                    }
                }
            }
            None => {
                log::error!("Failed to create image buffer from screenshot");
                self.notifications.notify(Notification::new(
                    "Failed to create screenshot image".to_string(),
                    NotificationLevel::Error,
                ));
            }
        }
    }

    /// Trigger a browser download for the screenshot (WASM only)
    #[cfg(target_arch = "wasm32")]
    fn trigger_browser_download(
        filename: &str,
        img_buffer: &image::RgbaImage,
    ) -> Result<(), String> {
        use std::io::Cursor;
        use wasm_bindgen::JsCast;

        // Encode the image as PNG
        let mut png_data = Vec::new();
        img_buffer
            .write_to(&mut Cursor::new(&mut png_data), image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG: {e}"))?;

        // Create a Blob from the PNG data
        let uint8_array = js_sys::Uint8Array::from(png_data.as_slice());
        let array = js_sys::Array::new();
        array.push(&uint8_array);

        let options = web_sys::BlobPropertyBag::new();
        options.set_type("image/png");

        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&array, &options)
            .map_err(|e| format!("Failed to create Blob: {e:?}"))?;

        // Create an object URL for the blob
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|e| format!("Failed to create object URL: {e:?}"))?;

        // Create a temporary anchor element and trigger download
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        let anchor: web_sys::HtmlAnchorElement = document
            .create_element("a")
            .map_err(|e| format!("Failed to create element: {e:?}"))?
            .dyn_into()
            .map_err(|_| "Failed to cast to anchor")?;

        anchor.set_href(&url);
        anchor.set_download(filename);
        anchor.click();

        // Clean up the object URL
        let _ = web_sys::Url::revoke_object_url(&url);

        Ok(())
    }

    // =========================================================================
    // Workspace management
    // =========================================================================

    /// Get the workspace directory path
    #[cfg(not(target_arch = "wasm32"))]
    fn workspace_dir() -> std::path::PathBuf {
        // Look for .enya/workspaces in current directory or home
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let enya_dir = cwd.join(".enya").join("workspaces");
        if enya_dir.exists() || std::fs::create_dir_all(&enya_dir).is_ok() {
            return enya_dir;
        }

        // Fallback to home directory
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let home_enya = std::path::PathBuf::from(&home)
            .join(".enya")
            .join("workspaces");
        let _ = std::fs::create_dir_all(&home_enya);
        home_enya
    }

    /// Ensure the default example workspace exists
    #[cfg(not(target_arch = "wasm32"))]
    fn ensure_default_workspace() {
        use crate::workspace::DEFAULT_WORKSPACE_TOML;

        let dir = Self::workspace_dir();
        let example_path = dir.join("example.toml");

        // Only create if it doesn't exist
        if !example_path.exists() {
            if let Err(e) = std::fs::write(&example_path, DEFAULT_WORKSPACE_TOML) {
                log::warn!("Failed to create default workspace: {e}");
            } else {
                log::info!("Created default workspace: {}", example_path.display());
            }
        }
    }

    /// Save the current workspace to a file
    fn save_workspace(&mut self, name: Option<&str>) {
        use crate::components::{Notification, NotificationLevel};

        let workspace_name = name.unwrap_or("default");

        // Get workspace from dashboard
        // TODO: Pass actual endpoint when endpoint tracking is implemented
        let Some(dashboard) = self.dashboard.as_ref() else {
            self.notifications.notify(Notification::new(
                "Dashboard not initialized".to_string(),
                NotificationLevel::Error,
            ));
            return;
        };
        let workspace = dashboard.to_workspace(workspace_name, self.state.theme, None);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = Self::workspace_dir();
            let path = dir.join(format!("{workspace_name}.toml"));

            match workspace.save(&path) {
                Ok(()) => {
                    log::info!("Workspace saved to: {}", path.display());
                    self.notifications.notify(Notification::new(
                        format!("Workspace saved: {workspace_name}"),
                        NotificationLevel::Success,
                    ));
                }
                Err(e) => {
                    log::error!("Failed to save workspace: {e}");
                    self.notifications.notify(Notification::new(
                        format!("Failed to save workspace: {e}"),
                        NotificationLevel::Error,
                    ));
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // On web, encode to base64 and show the URL
            match workspace.to_base64() {
                Ok(encoded) => {
                    let url = format!("?workspace={encoded}");
                    log::info!("Workspace encoded for URL: {url}");
                    self.notifications.notify(Notification::new(
                        format!(
                            "Workspace '{workspace_name}' ready to share (see console for URL)"
                        ),
                        NotificationLevel::Success,
                    ));
                }
                Err(e) => {
                    self.notifications.notify(Notification::new(
                        format!("Failed to encode workspace: {e}"),
                        NotificationLevel::Error,
                    ));
                }
            }
        }
    }

    /// Load a workspace from a file
    fn load_workspace(&mut self, name: &str) {
        use crate::components::{Notification, NotificationLevel};
        use crate::workspace::Workspace;

        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = Self::workspace_dir();
            let path = dir.join(format!("{name}.toml"));

            match Workspace::load(&path) {
                Ok(workspace) => {
                    if let Err(e) = workspace.validate() {
                        self.notifications.notify(Notification::new(
                            format!("Invalid workspace: {e}"),
                            NotificationLevel::Error,
                        ));
                        return;
                    }

                    if let Some(dashboard) = self.dashboard.as_mut() {
                        let connection =
                            dashboard.load_workspace(&workspace, &mut self.state.theme);

                        // TODO: Apply connection settings when endpoint tracking is implemented
                        if let Some(conn) = connection {
                            if !conn.endpoint.is_empty() {
                                log::info!("Workspace specifies endpoint: {}", conn.endpoint);
                            }
                        }

                        // Add to recent workspaces
                        self.state.settings.add_recent_workspace(
                            name.to_string(),
                            workspace.workspace.description.clone(),
                        );

                        log::info!("Workspace loaded: {name}");
                        self.notifications.notify(Notification::new(
                            format!("Workspace loaded: {name}"),
                            NotificationLevel::Success,
                        ));
                    } else {
                        self.notifications.notify(Notification::new(
                            "Dashboard not initialized".to_string(),
                            NotificationLevel::Error,
                        ));
                    }
                }
                Err(e) => {
                    log::error!("Failed to load workspace '{name}': {e}");
                    self.notifications.notify(Notification::new(
                        format!("Failed to load workspace '{name}': {e}"),
                        NotificationLevel::Error,
                    ));
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            // On web, first check for built-in workspaces, then try base64
            let workspace_result = if name == "example" {
                // Load built-in example workspace
                Ok(Workspace::default_example())
            } else {
                // Try to decode from base64 (for shared URLs)
                Workspace::from_base64(name)
            };

            match workspace_result {
                Ok(workspace) => {
                    if let Err(e) = workspace.validate() {
                        self.notifications.notify(Notification::new(
                            format!("Invalid workspace: {e}"),
                            NotificationLevel::Error,
                        ));
                        return;
                    }

                    if let Some(dashboard) = self.dashboard.as_mut() {
                        let connection =
                            dashboard.load_workspace(&workspace, &mut self.state.theme);

                        // TODO: Apply connection settings when endpoint tracking is implemented
                        if let Some(conn) = connection {
                            if !conn.endpoint.is_empty() {
                                log::info!("Workspace specifies endpoint: {}", conn.endpoint);
                            }
                        }

                        // Add to recent workspaces
                        self.state.settings.add_recent_workspace(
                            workspace.workspace.name.clone(),
                            workspace.workspace.description.clone(),
                        );

                        self.notifications.notify(Notification::new(
                            format!("Workspace loaded: {}", workspace.workspace.name),
                            NotificationLevel::Success,
                        ));
                    } else {
                        self.notifications.notify(Notification::new(
                            "Dashboard not initialized".to_string(),
                            NotificationLevel::Error,
                        ));
                    }
                }
                Err(e) => {
                    self.notifications.notify(Notification::new(
                        format!("Failed to load workspace: {e}"),
                        NotificationLevel::Error,
                    ));
                }
            }
        }
    }

    /// Share the current workspace as a URL (copies to clipboard)
    fn share_workspace(&mut self) {
        use crate::components::{Notification, NotificationLevel};

        // Get workspace from dashboard
        let Some(dashboard) = self.dashboard.as_ref() else {
            self.notifications.notify(Notification::new(
                "Dashboard not initialized".to_string(),
                NotificationLevel::Error,
            ));
            return;
        };

        let workspace = dashboard.to_workspace("shared", self.state.theme, None);

        match workspace.to_base64() {
            Ok(encoded) => {
                // Build the full URL
                #[cfg(target_arch = "wasm32")]
                let full_url = {
                    // Get the current page URL and append the workspace parameter
                    let base_url = web_sys::window()
                        .and_then(|w| w.location().href().ok())
                        .unwrap_or_else(|| "https://enya.build/editor".to_string());

                    // Remove any existing query string
                    let base_url = base_url.split('?').next().unwrap_or(&base_url);
                    format!("{base_url}?workspace={encoded}")
                };

                #[cfg(not(target_arch = "wasm32"))]
                let full_url = format!("https://enya.build/editor?workspace={encoded}");

                // Copy to clipboard
                #[cfg(target_arch = "wasm32")]
                {
                    if let Err(e) = Self::copy_to_clipboard_wasm(&full_url) {
                        log::error!("Failed to copy to clipboard: {e}");
                        self.notifications.notify(Notification::new(
                            format!("Failed to copy URL: {e}"),
                            NotificationLevel::Error,
                        ));
                        return;
                    }
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    // On native, just log the URL (clipboard support would need additional deps)
                    log::info!("Share URL: {full_url}");
                }

                self.notifications.notify(Notification::new(
                    "Workspace URL copied to clipboard!".to_string(),
                    NotificationLevel::Success,
                ));
            }
            Err(e) => {
                log::error!("Failed to encode workspace: {e}");
                self.notifications.notify(Notification::new(
                    format!("Failed to encode workspace: {e}"),
                    NotificationLevel::Error,
                ));
            }
        }
    }

    /// Share a single pane as a URL (copies to clipboard)
    fn share_pane(&mut self, pane_index: usize) {
        use crate::components::{Notification, NotificationLevel};

        // Get workspace from dashboard
        let Some(dashboard) = self.dashboard.as_ref() else {
            self.notifications.notify(Notification::new(
                "Dashboard not initialized".to_string(),
                NotificationLevel::Error,
            ));
            return;
        };

        let workspace = dashboard.to_workspace("shared", self.state.theme, None);

        match workspace.pane_to_base64(pane_index) {
            Ok(encoded) => {
                // Build the full URL
                #[cfg(target_arch = "wasm32")]
                let full_url = {
                    // Get the current page URL and append the pane parameter
                    let base_url = web_sys::window()
                        .and_then(|w| w.location().href().ok())
                        .unwrap_or_else(|| "https://enya.build/editor".to_string());

                    // Remove any existing query string
                    let base_url = base_url.split('?').next().unwrap_or(&base_url);
                    format!("{base_url}?pane={encoded}")
                };

                #[cfg(not(target_arch = "wasm32"))]
                let full_url = format!("https://enya.build/editor?pane={encoded}");

                // Copy to clipboard
                #[cfg(target_arch = "wasm32")]
                {
                    if let Err(e) = Self::copy_to_clipboard_wasm(&full_url) {
                        log::error!("Failed to copy to clipboard: {e}");
                        self.notifications.notify(Notification::new(
                            format!("Failed to copy URL: {e}"),
                            NotificationLevel::Error,
                        ));
                        return;
                    }
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    // On native, just log the URL (clipboard support would need additional deps)
                    log::info!("Share URL: {full_url}");
                }

                self.notifications.notify(Notification::new(
                    "Pane URL copied to clipboard!".to_string(),
                    NotificationLevel::Success,
                ));
            }
            Err(e) => {
                log::error!("Failed to encode pane: {e}");
                self.notifications.notify(Notification::new(
                    format!("Failed to encode pane: {e}"),
                    NotificationLevel::Error,
                ));
            }
        }
    }

    /// Copy text to clipboard (WASM only)
    #[cfg(target_arch = "wasm32")]
    fn copy_to_clipboard_wasm(text: &str) -> Result<(), String> {
        let window = web_sys::window().ok_or("No window")?;
        let navigator = window.navigator();
        let clipboard = navigator.clipboard();

        // Use the clipboard API to write text
        let text = text.to_string();
        let promise = clipboard.write_text(&text);

        // We don't need to await the promise - just fire and forget
        // The clipboard write happens asynchronously
        let _ = promise;

        Ok(())
    }

    /// Get workspace parameter from URL (WASM only)
    /// Returns the base64-encoded workspace if ?workspace=... is present
    #[cfg(target_arch = "wasm32")]
    fn get_url_workspace_param() -> Option<String> {
        let window = web_sys::window()?;
        let location = window.location();
        let search = location.search().ok()?;

        // Parse query string for workspace parameter
        // Format: ?workspace=base64encodedtoml
        if search.starts_with('?') {
            for param in search[1..].split('&') {
                if let Some(value) = param.strip_prefix("workspace=") {
                    if !value.is_empty() {
                        log::info!("Found workspace parameter in URL");
                        return Some(value.to_string());
                    }
                }
            }
        }

        None
    }

    /// Get pane parameter from URL (WASM only)
    /// Returns the base64-encoded single pane if ?pane=... is present
    #[cfg(target_arch = "wasm32")]
    fn get_url_pane_param() -> Option<String> {
        let window = web_sys::window()?;
        let location = window.location();
        let search = location.search().ok()?;

        // Parse query string for pane parameter
        // Format: ?pane=base64encodedsingle
        if search.starts_with('?') {
            for param in search[1..].split('&') {
                if let Some(value) = param.strip_prefix("pane=") {
                    if !value.is_empty() {
                        log::info!("Found pane parameter in URL");
                        return Some(value.to_string());
                    }
                }
            }
        }

        None
    }

    /// List available workspaces
    fn list_workspaces(&mut self) {
        use crate::components::{Notification, NotificationLevel};

        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = Self::workspace_dir();
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let workspaces: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .filter_map(|e| {
                            let path = e.path();
                            if path.extension().is_some_and(|ext| ext == "toml") {
                                path.file_stem()
                                    .and_then(|s| s.to_str())
                                    .map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                        .collect();

                    if workspaces.is_empty() {
                        self.notifications.notify(Notification::new(
                            format!("No workspaces found in {}", dir.display()),
                            NotificationLevel::Info,
                        ));
                    } else {
                        let list = workspaces.join(", ");
                        log::info!("Available workspaces: {list}");
                        self.notifications.notify(Notification::new(
                            format!("Workspaces: {list}"),
                            NotificationLevel::Info,
                        ));
                    }
                }
                Err(e) => {
                    self.notifications.notify(Notification::new(
                        format!("Failed to list workspaces: {e}"),
                        NotificationLevel::Error,
                    ));
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.notifications.notify(Notification::new(
                "Workspace listing not available on web. Use :source <base64> to load.".to_string(),
                NotificationLevel::Info,
            ));
        }
    }
}

impl eframe::App for EnyaApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Record frame time for editor metrics sparkline
        self.editor_metrics.record_frame();

        // Set theme for the context
        ctx.set_visuals(self.state.visuals());

        // Handle screenshot events
        self.handle_screenshot_events(ctx);

        // Custom title bar drag area (since native title bar is hidden)
        // The entire top 32px area acts as a drag region for moving the window
        #[cfg(not(target_arch = "wasm32"))]
        {
            let title_bar_height = 32.0;
            let screen_width = ctx.content_rect().width();
            let title_bar_rect = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(screen_width, title_bar_height),
            );
            // Check if pointer is in the title bar area and being dragged
            let pointer_in_title_bar = ctx.input(|i| {
                i.pointer
                    .interact_pos()
                    .is_some_and(|pos| title_bar_rect.contains(pos))
            });
            let is_dragging = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
            if pointer_in_title_bar && is_dragging {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }

        // No header panel - neovim-style UI uses only status bar at bottom

        // Draw main content
        self.show_main_content(ctx);

        // Draw bottom panel with connection info etc.
        self.show_bottom_panel(ctx);

        // Draw notifications (on top of everything)
        self.notifications.set_theme(self.state.theme);
        self.notifications.show(ctx);

        // Check for possible key board shortcut triggers
        self.check_keyboard_shortcuts(ctx);

        // Run any pending ui commands which updates internal data before the next frame
        self.run_pending_ui_commands(ctx);
    }
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Add DepartureMono font
    fonts.font_data.insert(
        "departure_mono".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/DepartureMono-Regular.otf"))
            .into(),
    );

    // Add Nerd Fonts icons
    egui_nerdfonts::add_to_fonts(&mut fonts, egui_nerdfonts::Variant::Regular);

    // Put DepartureMono first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "departure_mono".to_owned());

    // Put DepartureMono first (highest priority) for monospace too:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "departure_mono".to_owned());

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}
