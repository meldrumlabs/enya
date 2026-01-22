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

use std::sync::Arc;

use egui::Theme;

use crate::AsyncRuntime;
use crate::command::{CommandReceiver, CommandSender, UICommand, UICommandSender, command_channel};
use crate::components::{
    Notification, NotificationLevel, NotificationManager, Sparkline, StatusLine, StatusMode,
};
use crate::connection::ConnectionManager;
use crate::plugin::{EditorPluginHost, PluginContextRef, PluginRegistry};
use crate::team::TeamState;
use crate::ui::theme::AppTheme;
use crate::ui::welcome_screen::welcome_section_ui;
use crate::ui::{CustomThemeStore, ResolvedCustomTheme};
use crate::workspace::{Workspace, WorkspaceAction};

use state::EditorMetrics;

/// The core App
pub struct EnyaApp {
    pub(super) state: AppState,

    /// The workspace (pane layout, modals, etc.)
    pub(super) workspace: Workspace,

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

    // Plugin system (registry manages plugins, context provides editor services)
    #[allow(dead_code)] // Will be used when plugin commands are dispatched
    plugin_registry: PluginRegistry,
    #[allow(dead_code)] // Will be used when plugins interact with editor
    plugin_context: PluginContextRef,

    // Custom themes from plugins
    custom_themes: CustomThemeStore,

    // Currently active resolved custom theme (for rendering)
    resolved_custom_theme: Option<ResolvedCustomTheme>,
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

        // Initialize workspace with async runtime
        let mut workspace = Workspace::new(async_runtime.clone());

        // Initialize plugin system
        let plugin_host =
            EditorPluginHost::new(command_sender.clone(), async_runtime.clone(), state.theme);
        let plugin_host_ref: Arc<dyn crate::plugin::PluginHost> = Arc::new(plugin_host);
        let plugin_context: PluginContextRef =
            Arc::new(enya_plugin::PluginContext::new(plugin_host_ref.clone()));

        // Create plugin registry
        let mut plugin_registry = PluginRegistry::new();

        // Load external plugins from ~/.config/enya/plugins/ (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            use crate::plugin::{Plugin, PluginLoader};
            let loader = PluginLoader::new();

            // Load TOML config plugins
            for result in loader.load_all() {
                match result {
                    Ok(plugin) => {
                        let name = plugin.manifest().plugin.name.clone();
                        if let Err(e) = plugin_registry.register(plugin, true) {
                            log::warn!("Failed to register plugin '{name}': {e}");
                        } else {
                            log::info!("Loaded plugin: {name}");
                        }
                    }
                    Err(e) => log::warn!("Failed to load plugin: {e}"),
                }
            }

            // Load Lua script plugins
            for result in loader.load_all_lua() {
                match result {
                    Ok(plugin) => {
                        let name = plugin.name().to_string();
                        if let Err(e) = plugin_registry.register(plugin, true) {
                            log::warn!("Failed to register Lua plugin '{name}': {e}");
                        } else {
                            log::info!("Loaded Lua plugin: {name}");
                        }
                    }
                    Err(e) => log::warn!("Failed to load Lua plugin: {e}"),
                }
            }
        }

        // Initialize the registry with the plugin context
        plugin_registry.init(enya_plugin::PluginContext::new(plugin_host_ref));

        // Initialize and activate all plugins
        let plugin_ids: Vec<_> = plugin_registry
            .list_plugins()
            .iter()
            .map(|p| p.id)
            .collect();
        for id in plugin_ids {
            if let Err(e) = plugin_registry.init_plugin(id) {
                log::warn!("Failed to initialize plugin {id:?}: {e}");
            } else if let Err(e) = plugin_registry.activate_plugin(id) {
                log::warn!("Failed to activate plugin {id:?}: {e}");
            }
        }

        // Collect custom themes from plugins
        let mut custom_themes = CustomThemeStore::new();
        let custom_theme_list: Vec<(String, String, crate::ui::ActiveThemeColors)> =
            plugin_registry
                .all_themes()
                .into_iter()
                .map(|t| {
                    custom_themes.register(t.clone());
                    // Resolve the theme to get colors for the style picker preview
                    let resolved =
                        crate::ui::custom_theme::ResolvedCustomTheme::from_definition(&t);
                    let colors = crate::ui::ActiveThemeColors::from_custom(&resolved);
                    (t.name, t.display_name, colors)
                })
                .collect();
        workspace.set_custom_themes(custom_theme_list);

        // Collect plugin commands and pass to command palette
        let plugin_commands: Vec<crate::components::DynamicCommand> = plugin_registry
            .all_commands()
            .into_iter()
            .map(|(info, cmd)| crate::components::DynamicCommand {
                name: cmd.name.clone(),
                aliases: cmd.aliases.clone(),
                description: cmd.description.clone(),
                accepts_args: cmd.accepts_args,
                source: info.name.clone(),
            })
            .collect();
        workspace.set_plugin_commands(plugin_commands);

        // Collect plugin info for the plugins overlay
        let plugins_info: Vec<crate::components::PluginDisplayInfo> = plugin_registry
            .list_plugins()
            .iter()
            .map(|info| {
                // Determine the source type based on plugin characteristics
                let source = if info.name.ends_with(".lua") || info.description.contains("Lua") {
                    crate::components::PluginSource::Lua
                } else {
                    crate::components::PluginSource::Config
                };

                // Get commands and keybindings for this plugin
                let commands = plugin_registry.commands_for_plugin(info.id);
                let keybindings = plugin_registry.keybindings_for_plugin(info.id);

                crate::components::PluginDisplayInfo {
                    name: info.name.clone(),
                    version: info.version.clone(),
                    description: info.description.clone(),
                    enabled: info.state == enya_plugin::PluginState::Active,
                    source,
                    command_count: commands.len(),
                    keybinding_count: keybindings.len(),
                }
            })
            .collect();
        workspace.set_plugins(plugins_info);

        Self {
            state,
            workspace,
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
            // Plugin system
            plugin_registry,
            plugin_context,
            custom_themes,
            resolved_custom_theme: None,
        }
    }

    fn check_keyboard_shortcuts(&self, egui_ctx: &egui::Context) {
        // Skip global shortcuts when multi-buffer editing is capturing input
        if self.workspace.is_multi_buffer_input_mode() {
            return;
        }
        if let Some(cmd) = UICommand::listen_for_kb_shortcut(egui_ctx) {
            self.command_sender.send_ui(cmd);
        }
    }

    /// Get the egui Visuals for the current theme (builtin or custom)
    fn current_visuals(&self) -> egui::Visuals {
        if let Some(ref custom) = self.resolved_custom_theme {
            crate::ui::design::custom_theme_visuals(custom)
        } else {
            self.state.visuals()
        }
    }

    // Paints the bottom panel aka footer (lualine-style status bar)
    fn show_bottom_panel(&mut self, ctx: &egui::Context) {
        // Hide status line on landing page - it's part of the workspace UI, not the landing page
        if self.workspace.is_landing_page() {
            return;
        }

        // Update status line state
        self.status_line.set_theme(self.state.theme);

        // Set active theme colors (for custom theme support)
        // Set team status (only shows when connected)
        self.status_line
            .set_team_status(self.team_state.status_info());

        // Set mode based on current UI state
        // Note: Zen/Fullscreen are display preferences, not modes - user stays in Normal mode
        let mode = match self.state.ui_state {
            UIState::Dashboard => {
                if self.workspace.is_command_palette_open() {
                    StatusMode::Command
                } else if self.workspace.is_unified_finder_open() {
                    StatusMode::Search
                } else if self.workspace.is_agent_mode() {
                    StatusMode::Agent
                } else if self.workspace.is_visual_multi_mode() {
                    StatusMode::VisualMulti
                } else {
                    StatusMode::Normal
                }
            }
            UIState::Home => StatusMode::Home,
        };
        self.status_line.set_mode(mode);

        // Set agent provider name (shown as mode badge when in Agent mode)
        if mode == StatusMode::Agent {
            self.status_line
                .set_agent_provider_name(Some(self.workspace.agent_provider_name()));
        } else {
            self.status_line.set_agent_provider_name(None);
        }

        // Set open tabs count from workspace
        self.status_line
            .set_open_tabs(self.workspace.open_tabs_count());
        self.status_line
            .set_selected_metric(self.workspace.selected_metric());
        self.status_line
            .set_viewport_info(self.workspace.viewport_info());
        // Set multi-buffer status if in visual-multi mode
        let multi_buffer_status = self.workspace.multi_buffer_status_text();
        self.status_line
            .set_extra_status(if multi_buffer_status.is_empty() {
                None
            } else {
                Some(multi_buffer_status)
            });
        // Set diagnostics count
        let (errors, warnings, infos) = self.workspace.diagnostics_count_by_level();
        self.status_line
            .set_diagnostics_count(errors, warnings, infos);
        // Set connection status based on Prometheus health check
        self.status_line.set_connected(self.workspace.is_online());
        // Set display preference badges
        self.status_line.set_zen_mode(self.workspace.is_zen_mode());
        self.status_line
            .set_fullscreen(self.workspace.is_fullscreen());
        // Set codebase status (Cloning..., Indexing..., Ready, Error)
        // Only show when not on landing page - user expects status after entering workspace
        if self.workspace.is_landing_page() {
            self.status_line.set_codebase_status(None);
        } else {
            self.status_line
                .set_codebase_status(self.workspace.codebase_status_info());
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
        let is_agent_mode = self.workspace.is_agent_mode();

        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(false)
            .show(ctx, |ui| {
                // Note: viewport filter is now inline in the top toolbar

                // Status line with embedded agent input when in agent mode
                if is_agent_mode {
                    self.status_line.show_with_extra_content(ui, |ui| {
                        // Render the agent input bar inline within the status line
                        self.workspace.show_agent_input_bar_inline(ui, ctx, theme);
                    });
                } else {
                    self.status_line.show(ui);
                }
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
                egui_ctx.set_visuals(self.current_visuals());
                egui_ctx.request_repaint();
            }
            UICommand::NextTheme => {
                self.state.theme.next();
                self.state.settings.theme = self.state.theme; // Persist to settings
                egui_ctx.set_visuals(self.current_visuals());
                egui_ctx.request_repaint();
            }

            UICommand::ConnectionStatus(_connected) => {
                // External connection status updates are currently unused.
                // Connection status is now managed by ConnectionManager.
                egui_ctx.request_repaint();
            }

            UICommand::OpenFuzzyFinder => {
                self.workspace.open_unified_finder();
            }

            UICommand::OpenCommandPalette => {
                self.open_command_palette();
            }

            UICommand::Notify { level, message } => {
                // Log the notification from plugins
                match level.as_str() {
                    "error" => log::error!("[plugin] {message}"),
                    "warn" | "warning" => log::warn!("[plugin] {message}"),
                    "info" => log::info!("[plugin] {message}"),
                    _ => log::debug!("[plugin] {message}"),
                }
                // TODO: Show visual notification in UI
            }

            UICommand::Repaint => {
                egui_ctx.request_repaint();
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

        let mut workspace_action = WorkspaceAction::None;

        egui::CentralPanel::default().show(ctx, |ui| {
            // Update active theme colors (from custom or builtin theme)
            let active_colors = if let Some(ref custom) = self.resolved_custom_theme {
                crate::ui::ActiveThemeColors::from_custom(custom)
            } else {
                crate::ui::ActiveThemeColors::from_builtin(self.state.theme)
            };
            self.workspace.set_active_colors(active_colors);

            // Update workspace team status and members before rendering
            self.workspace
                .set_team_status(self.team_state.status_info());
            self.workspace
                .set_team_members(self.team_state.members().to_vec());
            // Pass chat state reference when team mode is active
            let chat_state = if self.team_state.is_connected() {
                self.workspace.show_channels_panel();
                Some(self.team_state.chat_state())
            } else {
                self.workspace.hide_channels_panel();
                None
            };
            workspace_action = self.workspace.show(ui, ctx, &self.state, chat_state);

            // Poll for pane interactions (e.g., chart drilldown clicks)
            self.workspace.poll_pane_interactions();
        });

        // Handle actions from the viewport (e.g., from command palette)
        self.handle_workspace_action(ctx, workspace_action);
    }

    fn handle_workspace_action(&mut self, ctx: &egui::Context, action: WorkspaceAction) {
        match action {
            WorkspaceAction::None => {}
            WorkspaceAction::SetTheme(theme) => {
                // Clear custom theme and set builtin theme
                self.state.custom_theme = None;
                self.resolved_custom_theme = None;
                self.command_sender.send_ui(UICommand::Theme(theme));
            }
            WorkspaceAction::SetCustomTheme(name) => {
                // Resolve the custom theme
                if let Some(def) = self.custom_themes.get(&name) {
                    let resolved = ResolvedCustomTheme::from_definition(def);
                    log::info!(
                        "[theme] Resolved custom theme: {} (base: {})",
                        resolved.display_name,
                        if resolved.is_dark { "dark" } else { "light" }
                    );
                    // Set the base theme (dark or light) for fallback colors
                    let base = if resolved.is_dark {
                        AppTheme::Dark
                    } else {
                        AppTheme::Light
                    };
                    self.command_sender.send_ui(UICommand::Theme(base));
                    self.resolved_custom_theme = Some(resolved);
                }
                self.state.set_custom_theme(name.clone());
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
                self.state.custom_theme = None;
                self.resolved_custom_theme = None;
                self.command_sender.send_ui(UICommand::Theme(theme));
                self.state.settings.font = font;
                fonts::setup_fonts(ctx, font);
            }
            WorkspaceAction::SetCustomThemeAndFont(name, font) => {
                // Restore custom theme and font (used when cancelling style picker)
                if let Some(def) = self.custom_themes.get(&name) {
                    let resolved = ResolvedCustomTheme::from_definition(def);
                    let base = if resolved.is_dark {
                        AppTheme::Dark
                    } else {
                        AppTheme::Light
                    };
                    self.command_sender.send_ui(UICommand::Theme(base));
                    self.resolved_custom_theme = Some(resolved);
                }
                self.state.set_custom_theme(name);
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
                self.workspace.open_annotation_editor();
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
            WorkspaceAction::OpenDiffViewer {
                hash,
                message,
                diff,
            } => {
                // Open the diff viewer with the commit message as title
                self.open_diff_viewer(&hash, &message, &diff);
            }
            WorkspaceAction::PluginCommand { command, args } => {
                // Dispatch to plugin registry
                if !self.plugin_registry.execute_command(&command, &args) {
                    // No plugin handled the command
                    self.notifications.notify(Notification::new(
                        format!("Unknown command: {command}"),
                        NotificationLevel::Error,
                    ));
                }
            }
        }
    }

    fn open_command_palette(&mut self) {
        self.workspace.open_command_palette();
    }

    /// Search commits for # autocomplete in chat.
    #[cfg(not(target_arch = "wasm32"))]
    fn search_chat_commits(&mut self, query: &str) {
        use crate::chat::CommitInfo;

        // Search codebase for commits
        let results = self
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
        self.workspace.set_chat_commits(commits);
    }

    #[cfg(target_arch = "wasm32")]
    fn search_chat_commits(&mut self, _query: &str) {
        // Commit search is not available in WASM builds
    }

    /// Open the diff viewer with a commit diff.
    fn open_diff_viewer(&mut self, hash: &str, message: &str, diff: &str) {
        self.workspace
            .open_diff_viewer_with_content(hash, message, diff);
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

        // Set theme for the context (use custom theme colors if active)
        ctx.set_visuals(self.current_visuals());

        // Handle screenshot events
        self.handle_screenshot_events(ctx);

        // Poll connection manager for completed health checks
        self.poll_connection();

        // Poll team state for events (presence changes, mentions, etc.)
        self.team_state.poll(ctx);

        // Custom titlebar with window controls and drag area
        // Replaces native macOS titlebar for seamless theme integration
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Use custom theme if active, otherwise fall back to builtin theme
            let theme = if let Some(ref custom) = self.resolved_custom_theme {
                AppTheme::Custom(crate::ui::ActiveThemeColors::from_custom(custom))
            } else {
                self.state.theme
            };
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
