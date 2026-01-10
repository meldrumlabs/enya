//! The core EnyaApp and application lifecycle.
//!
//! This module contains the main application struct and its eframe::App
//! implementation. Submodules handle specific concerns:
//!
//! - `state` - AppState, UIState, and EditorMetrics types
//! - `workspace_io` - Save/load/share workspace operations
//! - `screenshot` - Screenshot capture and saving
//! - `fonts` - Font configuration

mod fonts;
mod screenshot;
mod state;
mod workspace_io;

pub use state::{AppState, UIState};

use egui::Theme;

use crate::AsyncRuntime;
use crate::command::{CommandReceiver, CommandSender, UICommand, UICommandSender, command_channel};
use crate::components::{
    Notification, NotificationLevel, NotificationManager, Sparkline, StatusLine, StatusMode,
};
use crate::connection::ConnectionManager;
use crate::team::TeamState;
use crate::ui::theme::AppTheme;
use crate::ui::welcome_screen::welcome_section_ui;
use crate::workspace::{TabBarAction, WorkspaceAction, WorkspaceTabBar};

use state::EditorMetrics;

/// The core App
pub struct EnyaApp {
    pub(super) state: AppState,

    /// Workspace tab bar for managing multiple workspaces
    pub(super) workspace_tabs: WorkspaceTabBar,

    // Agent connection manager
    connection: ConnectionManager,

    // Channels for ui commands
    pub command_sender: CommandSender,
    pub command_receiver: CommandReceiver,

    // Status line component
    status_line: StatusLine,

    // Notification manager
    pub(super) notifications: NotificationManager,

    // Internal editor metrics (frame times, etc.)
    editor_metrics: EditorMetrics,

    // Async runtime for spawning background tasks (AI agent, etc.)
    #[allow(dead_code)] // Will be used by AI agent integration
    async_runtime: AsyncRuntime,

    // Pending screenshot path (used when screenshot event arrives)
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) pending_screenshot_path: Option<String>,

    // Track window fullscreen state for toggle behavior
    #[cfg(not(target_arch = "wasm32"))]
    is_fullscreen: bool,

    // Whether we've checked URL for workspace parameter (WASM only)
    #[cfg(target_arch = "wasm32")]
    checked_url_workspace: bool,

    // Team collaboration state (disabled by default, can be enabled with TeamConfig)
    team_state: TeamState,
}

impl EnyaApp {
    pub fn new(cc: &eframe::CreationContext<'_>, async_runtime: AsyncRuntime) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let (command_sender, command_receiver) = command_channel();

        // Load previous app state FIRST (before font setup).
        // Note that you must enable the `persistence` feature for this to work.
        let mut state: AppState = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();

        // Set up fonts with user's preferred font from saved settings
        fonts::setup_fonts(&cc.egui_ctx, state.settings.font);

        // Always start with Dashboard (ignore persisted ui_state)
        state.ui_state = UIState::Dashboard;

        // Ensure the default example workspace exists on first run
        #[cfg(not(target_arch = "wasm32"))]
        Self::ensure_default_workspace();

        // Ensure demo workspace is in recent workspaces (for new users)
        state.settings.ensure_demo_workspace();

        // Use theme from settings (user preference), falling back to system theme only on first run
        if state.settings.theme == AppTheme::default() && state.theme == AppTheme::default() {
            // First run: use system theme
            match cc.egui_ctx.theme() {
                Theme::Light => state.settings.theme = AppTheme::Light,
                Theme::Dark => state.settings.theme = AppTheme::Dark,
            }
        }
        // Sync state.theme from settings.theme (settings is the source of truth)
        state.theme = state.settings.theme;

        // Initialize workspace tabs with async runtime
        let mut workspace_tabs = WorkspaceTabBar::new(async_runtime.clone());
        workspace_tabs.set_theme(state.theme);

        Self {
            state,
            workspace_tabs,
            command_sender,
            command_receiver,
            connection: ConnectionManager::new(async_runtime.clone()),
            status_line: StatusLine::new(),
            notifications: NotificationManager::new(),
            editor_metrics: EditorMetrics::default(),
            async_runtime,
            #[cfg(not(target_arch = "wasm32"))]
            pending_screenshot_path: None,
            #[cfg(not(target_arch = "wasm32"))]
            is_fullscreen: false,
            #[cfg(target_arch = "wasm32")]
            checked_url_workspace: false,
            // Team state starts disabled - can be enabled later via connect()
            team_state: TeamState::default(),
        }
    }

    fn check_keyboard_shortcuts(&self, egui_ctx: &egui::Context) {
        // Skip global shortcuts when multi-buffer editing is capturing input
        if let Some(tab) = self.workspace_tabs.active_tab() {
            if tab.workspace.is_multi_buffer_input_mode() {
                return;
            }
        }
        if let Some(cmd) = UICommand::listen_for_kb_shortcut(egui_ctx) {
            self.command_sender.send_ui(cmd);
        }
    }

    /// Handle workspace tab navigation keyboard shortcuts at the app level
    fn check_tab_navigation(&mut self, ctx: &egui::Context) {
        // Don't handle if something has focus (text input, etc.)
        if ctx.memory(|mem| mem.focused().is_some()) {
            return;
        }

        ctx.input_mut(|input| {
            // Shift+N - next workspace tab
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::N) {
                self.workspace_tabs.next_tab();
            }
            // Shift+P - previous workspace tab
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::P) {
                self.workspace_tabs.prev_tab();
            }
            // Shift+T - create new workspace tab (like :tabnew)
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::T) {
                self.workspace_tabs.new_tab();
            }
            // Shift+X - close current workspace tab (like :tabclose)
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::X) {
                self.workspace_tabs
                    .close_tab(self.workspace_tabs.active_index());
            }
        });
    }

    // Paints the bottom panel aka footer (lualine-style status bar)
    fn show_bottom_panel(&mut self, ctx: &egui::Context) {
        // Hide status line on landing page - it's part of the workspace UI, not the landing page
        if let Some(tab) = self.workspace_tabs.active_tab() {
            if tab.workspace.is_landing_page() {
                return;
            }
        }

        // Update status line state
        self.status_line.set_theme(self.state.theme);

        // Set team status (only shows when connected)
        self.status_line
            .set_team_status(self.team_state.status_info());

        // Set mode based on current UI state
        // Note: Zen/Fullscreen are display preferences, not modes - user stays in Normal mode
        let mode = match self.state.ui_state {
            UIState::Dashboard => {
                if let Some(tab) = self.workspace_tabs.active_tab() {
                    let workspace = &tab.workspace;
                    if workspace.is_command_palette_open() {
                        StatusMode::Command
                    } else if workspace.is_metrics_finder_open() {
                        StatusMode::Search
                    } else if workspace.is_agent_mode() {
                        StatusMode::Agent
                    } else if workspace.is_visual_multi_mode() {
                        StatusMode::VisualMulti
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

        // Set open tabs count from workspace
        if let Some(tab) = self.workspace_tabs.active_tab() {
            let workspace = &tab.workspace;
            self.status_line.set_open_tabs(workspace.open_tabs_count());
            self.status_line
                .set_selected_metric(workspace.selected_metric());
            self.status_line
                .set_viewport_info(workspace.viewport_info());
            // Set multi-buffer status if in visual-multi mode
            let multi_buffer_status = workspace.multi_buffer_status_text();
            self.status_line
                .set_extra_status(if multi_buffer_status.is_empty() {
                    None
                } else {
                    Some(multi_buffer_status)
                });
            // Set diagnostics count
            let (errors, warnings, infos) = workspace.diagnostics_count_by_level();
            self.status_line
                .set_diagnostics_count(errors, warnings, infos);
            // Set connection status based on Prometheus health check
            self.status_line.set_connected(workspace.is_online());
            // Set display preference badges
            self.status_line.set_zen_mode(workspace.is_zen_mode());
            self.status_line.set_fullscreen(workspace.is_fullscreen());
            // Set codebase status (Cloning..., Indexing..., Ready, Error)
            // Only show when not on landing page - user expects status after entering workspace
            if workspace.is_landing_page() {
                self.status_line.set_codebase_status(None);
            } else {
                self.status_line
                    .set_codebase_status(workspace.codebase_status_info());
            }
        } else {
            // No active tab - show offline
            self.status_line.set_connected(false);
            self.status_line.set_codebase_status(None);
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

        let theme = self.state.theme;
        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(false)
            .show(ctx, |ui| {
                // Show agent input bar above status line (if in agent mode)
                if let Some(tab) = self.workspace_tabs.active_tab_mut() {
                    tab.workspace.show_agent_input_bar(ui, ctx, theme);
                }

                // Show viewport filter bar above status line (if filter is open)
                if let Some(tab) = self.workspace_tabs.active_tab_mut() {
                    tab.workspace.show_viewport_filter_bar(ui);
                }

                // Status line at the bottom
                self.status_line.show(ui);
            });
    }

    // This draws the main panel
    #[inline]
    #[profiling::function]
    fn show_main_content(&mut self, ctx: &egui::Context) {
        match self.state.ui_state() {
            UIState::Dashboard => self.draw_workspace(ctx),
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
                self.state.settings.theme = theme; // Persist to settings
                egui_ctx.set_visuals(self.state.visuals());
                egui_ctx.request_repaint();
            }
            UICommand::NextTheme => {
                self.state.theme.next();
                self.state.settings.theme = self.state.theme; // Persist to settings
                egui_ctx.set_visuals(self.state.visuals());
                egui_ctx.request_repaint();
            }

            UICommand::ConnectionStatus(_connected) => {
                // External connection status updates are currently unused.
                // Connection status is now managed by ConnectionManager.
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

    #[profiling::function]
    fn draw_workspace(&mut self, ctx: &egui::Context) {
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

        // Update workspace tabs theme
        self.workspace_tabs.set_theme(self.state.theme);

        // Render workspace tab bar (hide when single tab on landing page, or in zen mode)
        let should_hide_tabs = (self.workspace_tabs.tab_count() == 1
            && self
                .workspace_tabs
                .active_tab()
                .is_some_and(|tab| tab.workspace.is_landing_page()))
            || self
                .workspace_tabs
                .active_tab()
                .is_some_and(|tab| tab.workspace.is_zen_mode());
        if !should_hide_tabs {
            let tab_action = self.workspace_tabs.show(ctx);
            self.handle_tab_bar_action(tab_action);
        }

        let mut workspace_action = WorkspaceAction::None;

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(tab) = self.workspace_tabs.active_tab_mut() {
                // Update workspace team status and members before rendering
                tab.workspace.set_team_status(self.team_state.status_info());
                tab.workspace
                    .set_team_members(self.team_state.members().to_vec());
                // Pass chat state reference when team mode is active
                let chat_state = if self.team_state.is_connected() {
                    tab.workspace.show_channels_panel();
                    Some(self.team_state.chat_state())
                } else {
                    tab.workspace.hide_channels_panel();
                    None
                };
                workspace_action = tab.workspace.show(ui, ctx, &self.state, chat_state);
            }
        });

        // Handle actions from the viewport (e.g., from command palette)
        self.handle_workspace_action(ctx, workspace_action);
    }

    /// Handle actions from the workspace tab bar
    fn handle_tab_bar_action(&mut self, action: TabBarAction) {
        match action {
            TabBarAction::None => {}
            TabBarAction::SwitchToTab(idx) => {
                self.workspace_tabs.switch_to_tab(idx);
            }
            TabBarAction::CloseTab(idx) => {
                self.workspace_tabs.close_tab(idx);
            }
            TabBarAction::NewTab => {
                // On native: open the workspace creator overlay
                // On WASM: directly create a new tab (overlay not supported)
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(tab) = self.workspace_tabs.active_tab_mut() {
                    tab.workspace.open_workspace_creator();
                }
                #[cfg(target_arch = "wasm32")]
                self.workspace_tabs.new_tab();
            }
        }
    }

    fn handle_workspace_action(&mut self, ctx: &egui::Context, action: WorkspaceAction) {
        match action {
            WorkspaceAction::None => {}
            WorkspaceAction::SetTheme(theme) => {
                self.command_sender.send_ui(UICommand::Theme(theme));
            }
            WorkspaceAction::NextTheme => {
                self.command_sender.send_ui(UICommand::NextTheme);
            }
            WorkspaceAction::SetFont(font) => {
                // Update the setting (will be persisted automatically via save())
                self.state.settings.font = font;
                // Apply the font change immediately
                fonts::setup_fonts(ctx, font);
            }
            WorkspaceAction::SetThemeAndFont(theme, font) => {
                // Restore both theme and font (used when cancelling style picker)
                self.command_sender.send_ui(UICommand::Theme(theme));
                self.state.settings.font = font;
                fonts::setup_fonts(ctx, font);
            }
            WorkspaceAction::Notify { level, message } => {
                let notification_level = match level.to_lowercase().as_str() {
                    "success" | "ok" => NotificationLevel::Success,
                    "warn" | "warning" => NotificationLevel::Warn,
                    "error" | "err" => NotificationLevel::Error,
                    _ => NotificationLevel::Info,
                };
                self.notifications
                    .notify(Notification::new(message, notification_level));
            }
            WorkspaceAction::TrackRecentPlot {
                name,
                metric_name,
                is_query,
            } => {
                self.state
                    .settings
                    .add_recent_plot(name, metric_name, is_query);
            }
            WorkspaceAction::TakeScreenshot(path) => {
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
            WorkspaceAction::SaveWorkspace(name) => {
                self.save_workspace(name.as_deref());
            }
            WorkspaceAction::LoadWorkspace(name) => {
                self.load_workspace(&name);
            }
            WorkspaceAction::ListWorkspaces => {
                self.list_workspaces();
            }
            WorkspaceAction::ShareWorkspace => {
                self.share_workspace();
            }
            WorkspaceAction::SharePane(pane_index) => {
                self.share_pane(pane_index);
            }
            WorkspaceAction::QuitApp => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            WorkspaceAction::NewWorkspaceTab(name) => {
                if let Some(name) = name {
                    self.workspace_tabs.new_tab_with_name(name);
                } else {
                    self.workspace_tabs.new_tab();
                }
            }
            WorkspaceAction::CloseWorkspaceTab => {
                self.workspace_tabs
                    .close_tab(self.workspace_tabs.active_index());
            }
            WorkspaceAction::NextWorkspaceTab => {
                self.workspace_tabs.next_tab();
            }
            WorkspaceAction::PrevWorkspaceTab => {
                self.workspace_tabs.prev_tab();
            }
            WorkspaceAction::RenameCurrentWorkspace(name) => {
                self.workspace_tabs.rename_active_tab(name);
            }
            WorkspaceAction::RenameAndSaveWorkspace(name) => {
                self.workspace_tabs.rename_active_tab(name.clone());
                self.save_workspace(Some(&name));
            }
            WorkspaceAction::ToggleTeamDemo => {
                self.team_state.toggle_demo_mode();
                let msg = if self.team_state.is_demo() {
                    "Team demo mode enabled"
                } else {
                    "Team demo mode disabled"
                };
                self.notifications
                    .notify(Notification::new(msg, NotificationLevel::Info));
            }
            WorkspaceAction::TeamConnect { url, token } => {
                // Set the async runtime before connecting (native only)
                #[cfg(not(target_arch = "wasm32"))]
                self.team_state
                    .set_async_runtime(self.async_runtime.clone());

                self.team_state.connect(&url, &token, ctx);
                self.notifications.notify(Notification::new(
                    format!("Connecting to team server: {url}"),
                    NotificationLevel::Info,
                ));
            }
            WorkspaceAction::TeamDisconnect => {
                self.team_state.disconnect();
                self.notifications.notify(Notification::new(
                    "Disconnected from team server",
                    NotificationLevel::Info,
                ));
            }
            WorkspaceAction::OpenAnnotationEditor => {
                // Open annotation editor on the focused pane
                if let Some(tab) = self.workspace_tabs.active_tab_mut() {
                    tab.workspace.open_annotation_editor();
                }
            }
            WorkspaceAction::SendChatMessage {
                text,
                chart,
                visualization,
                thread_id,
            } => {
                // Add message to team_state.chat_state (the authoritative source)
                use crate::chat::ChatMessage;
                use enya_team_api::UserId;

                let user_id = UserId::new_v4(); // TODO: Get from team_state
                let mut message = ChatMessage::from_user(user_id, "You", &text);
                if let Some(chart) = chart {
                    message = message.with_inline_chart(chart);
                }
                if let Some(viz) = visualization {
                    message = message.with_visualization(viz);
                }
                if let Some(tid) = thread_id {
                    message.thread_id = Some(tid);
                }

                self.team_state.chat_state_mut().add_message(message);
            }
            WorkspaceAction::CreateChannel { name } => {
                self.team_state.create_channel(&name, ctx);
                self.notifications.notify(Notification::new(
                    format!("Creating channel: {name}"),
                    NotificationLevel::Info,
                ));
            }
            WorkspaceAction::CreateThread { channel_id, title } => {
                self.team_state.create_thread(channel_id, &title, ctx);
                self.notifications.notify(Notification::new(
                    format!("Creating thread: {title}"),
                    NotificationLevel::Info,
                ));
            }
            WorkspaceAction::SearchChatCommits { query } => {
                // Search commits in the codebase index and provide to chat
                self.search_chat_commits(&query);
            }
            WorkspaceAction::OpenDiffViewer { hash, diff } => {
                // Open the diff viewer pane with the commit diff
                self.open_diff_viewer(&hash, &diff);
            }
        }
    }

    fn open_metrics_finder(&mut self) {
        if let Some(tab) = self.workspace_tabs.active_tab_mut() {
            tab.workspace.open_metrics_finder();
        }
    }

    fn open_command_palette(&mut self) {
        if let Some(tab) = self.workspace_tabs.active_tab_mut() {
            tab.workspace.open_command_palette();
        }
    }

    /// Search commits for # autocomplete in chat.
    #[cfg(not(target_arch = "wasm32"))]
    fn search_chat_commits(&mut self, query: &str) {
        use crate::chat::CommitInfo;

        if let Some(tab) = self.workspace_tabs.active_tab_mut() {
            // Search codebase for commits
            let results = tab
                .workspace
                .search_codebase(query, Some("commits"), Some(10));

            // Convert to CommitInfo for the chat
            let commits: Vec<CommitInfo> = results
                .into_iter()
                .filter_map(|r| {
                    if let crate::codebase::SearchResultKind::Commit {
                        hash,
                        timestamp,
                        diff,
                    } = r.kind
                    {
                        Some(CommitInfo {
                            short_hash: if hash.len() >= 7 {
                                hash[..7].to_string()
                            } else {
                                hash.clone()
                            },
                            full_hash: hash,
                            message: r.name,
                            timestamp,
                            diff,
                        })
                    } else {
                        None
                    }
                })
                .collect();

            // Provide commits to the channels panel
            tab.workspace.set_chat_commits(commits);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn search_chat_commits(&mut self, _query: &str) {
        // Commit search is not available in WASM builds
    }

    /// Open the diff viewer with a commit diff.
    fn open_diff_viewer(&mut self, hash: &str, diff: &str) {
        if let Some(tab) = self.workspace_tabs.active_tab_mut() {
            tab.workspace.open_diff_viewer_with_content(hash, diff);
        }
    }

    /// Poll the connection manager for completed health checks
    fn poll_connection(&mut self) {
        if let Some(result) = self.connection.poll() {
            match result {
                Ok(health) => {
                    let short_hash = if health.git_hash.len() >= 7 {
                        &health.git_hash[..7]
                    } else {
                        &health.git_hash
                    };
                    self.notifications.notify(Notification::new(
                        format!("Connected to agent v{} ({})", health.version, short_hash),
                        NotificationLevel::Success,
                    ));
                }
                Err(e) => {
                    self.notifications.notify(Notification::new(
                        format!("Connection failed: {e}"),
                        NotificationLevel::Error,
                    ));
                }
            }
        }
    }
}

impl eframe::App for EnyaApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    #[profiling::function]
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        profiling::finish_frame!();

        // Record frame time for editor metrics sparkline
        self.editor_metrics.record_frame();

        // Set theme for the context
        ctx.set_visuals(self.state.visuals());

        // Handle screenshot events
        self.handle_screenshot_events(ctx);

        // Poll connection manager for completed health checks
        self.poll_connection();

        // Poll team state for events (presence changes, mentions, etc.)
        self.team_state.poll(ctx);

        // Handle workspace tab navigation shortcuts at app level
        self.check_tab_navigation(ctx);

        // Custom titlebar with window controls and drag area
        // Replaces native macOS titlebar for seamless theme integration
        #[cfg(not(target_arch = "wasm32"))]
        {
            let theme = self.state.theme;
            let titlebar_height = 32.0;

            egui::TopBottomPanel::top("custom_titlebar")
                .exact_height(titlebar_height)
                .frame(egui::Frame::NONE.fill(theme.bg_base()))
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add_space(8.0);

                        // Traffic light buttons with theme-appropriate colors
                        let button_size = 12.0;
                        let button_spacing = 8.0;

                        // Close button (red)
                        let close_color = egui::Color32::from_rgb(255, 95, 87);
                        let (close_rect, close_response) = ui.allocate_exact_size(
                            egui::vec2(button_size, button_size),
                            egui::Sense::click(),
                        );
                        let close_color = if close_response.hovered() {
                            close_color
                        } else {
                            close_color.gamma_multiply(0.7)
                        };
                        ui.painter().circle_filled(
                            close_rect.center(),
                            button_size / 2.0,
                            close_color,
                        );
                        if close_response.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }

                        ui.add_space(button_spacing);

                        // Minimize button (yellow)
                        let minimize_color = egui::Color32::from_rgb(255, 189, 46);
                        let (min_rect, min_response) = ui.allocate_exact_size(
                            egui::vec2(button_size, button_size),
                            egui::Sense::click(),
                        );
                        let minimize_color = if min_response.hovered() {
                            minimize_color
                        } else {
                            minimize_color.gamma_multiply(0.7)
                        };
                        ui.painter().circle_filled(
                            min_rect.center(),
                            button_size / 2.0,
                            minimize_color,
                        );
                        if min_response.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }

                        ui.add_space(button_spacing);

                        // Fullscreen button (uses theme accent)
                        let fullscreen_color = theme.accent_primary();
                        let (fs_rect, fs_response) = ui.allocate_exact_size(
                            egui::vec2(button_size, button_size),
                            egui::Sense::click(),
                        );
                        let fullscreen_color = if fs_response.hovered() {
                            theme.accent_hover()
                        } else {
                            fullscreen_color.gamma_multiply(0.7)
                        };
                        ui.painter().circle_filled(
                            fs_rect.center(),
                            button_size / 2.0,
                            fullscreen_color,
                        );
                        if fs_response.clicked() {
                            self.is_fullscreen = !self.is_fullscreen;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                                self.is_fullscreen,
                            ));
                        }

                        // Rest of titlebar is drag area
                        let remaining = ui.available_rect_before_wrap();
                        let drag_response =
                            ui.allocate_rect(remaining, egui::Sense::click_and_drag());
                        if drag_response.dragged() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                        // Double-click to maximize
                        if drag_response.double_clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                        }
                    });
                });
        }

        // Draw bottom panel with connection info etc.
        // IMPORTANT: Must be drawn BEFORE main content so CentralPanel knows to reserve space
        self.show_bottom_panel(ctx);

        // Draw main content
        self.show_main_content(ctx);

        // Draw notifications (on top of everything)
        self.notifications.set_theme(self.state.theme);
        self.notifications.show(ctx);

        // Check for possible key board shortcut triggers
        self.check_keyboard_shortcuts(ctx);

        // Run any pending ui commands which updates internal data before the next frame
        self.run_pending_ui_commands(ctx);
    }
}
