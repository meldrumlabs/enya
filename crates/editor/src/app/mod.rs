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
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::ManifestFetcher;
use crate::components::{
    Notification, NotificationLevel, NotificationManager, SettingsPage, SettingsPageResult,
    Sparkline, StatusLine, StatusMode,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::{UpdateBanner, UpdateBannerAction};
use crate::connection::ConnectionManager;
use crate::github_auth::GitHubAuthManager;
use crate::plugin::{EditorPluginHost, PluginContextRef, PluginRegistry, PluginSharedStateRef};
use crate::ui::theme::AppTheme;
use crate::ui::welcome_screen::welcome_section_ui;
use crate::ui::{CustomThemeStore, ResolvedCustomTheme};
#[cfg(not(target_arch = "wasm32"))]
use crate::update_checker::UpdateChecker;
use crate::workspace::{Workspace, WorkspaceAction};

use state::EditorMetrics;

/// The core App
pub struct EnyaApp {
    pub(super) state: AppState,

    /// The workspace (pane layout, modals, etc.)
    pub(super) workspace: Workspace,

    // Agent connection manager
    connection: ConnectionManager,

    // Update checker for new version notifications (native only)
    #[cfg(not(target_arch = "wasm32"))]
    update_checker: UpdateChecker,

    // Provider manifest fetcher for hot-updating model lists (native only)
    #[cfg(not(target_arch = "wasm32"))]
    manifest_fetcher: ManifestFetcher,

    // Channels for ui commands
    pub command_sender: CommandSender,
    pub command_receiver: CommandReceiver,

    // Status line component
    status_line: StatusLine,

    // Notification manager
    pub(super) notifications: NotificationManager,

    // Internal editor metrics (frame times, etc.)
    editor_metrics: EditorMetrics,

    // Async runtime for spawning background tasks (snapshot uploads, AI agent, etc.)
    async_runtime: AsyncRuntime,

    // HTTP client for snapshot uploads to blob server
    snapshot_http_client: reqwest::Client,
    // Pending snapshot upload result (URL or error)
    pending_snapshot_upload: std::sync::Arc<parking_lot::Mutex<Option<Result<String, String>>>>,
    // Pending snapshot load result (decoded WorkspaceConfig or error)
    pending_snapshot_load:
        std::sync::Arc<parking_lot::Mutex<Option<Result<enya_config::WorkspaceConfig, String>>>>,

    // Pending screenshot path (used when screenshot event arrives)
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) pending_screenshot_path: Option<String>,

    // Track window fullscreen state for toggle behavior
    #[cfg(not(target_arch = "wasm32"))]
    is_fullscreen: bool,

    // Whether we've checked URL for workspace parameter (WASM only)
    #[cfg(target_arch = "wasm32")]
    checked_url_workspace: bool,

    // Startup workspace to load on first frame (native only, set via CLI --workspace flag)
    #[cfg(not(target_arch = "wasm32"))]
    startup_workspace: Option<String>,

    // Plugin system (registry manages plugins, context provides editor services)
    #[allow(dead_code)] // Will be used when plugin commands are dispatched
    plugin_registry: PluginRegistry,
    #[allow(dead_code)] // Will be used when plugins interact with editor
    plugin_context: PluginContextRef,
    /// Shared state for plugins (focused pane info, etc.)
    plugin_shared_state: PluginSharedStateRef,

    // Custom themes from plugins
    custom_themes: CustomThemeStore,

    // Currently active resolved custom theme (for rendering)
    resolved_custom_theme: Option<ResolvedCustomTheme>,

    // Full-page settings
    settings_page: SettingsPage,

    // GitHub authentication manager
    github_auth: GitHubAuthManager,

    // Delay plugin installation by one frame to allow spinner to render
    #[cfg(not(target_arch = "wasm32"))]
    install_plugin_ready: bool,
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

        // Scale up the UI on WASM to compensate for browser rendering making content appear small
        #[cfg(target_arch = "wasm32")]
        cc.egui_ctx
            .set_zoom_factor(crate::ui::typography::WASM_ZOOM_FACTOR);

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

        // Apply saved AI provider/model to agent panel
        workspace.set_agent_provider_and_model(
            state.settings.ai_provider,
            state.settings.ai_model.clone(),
        );

        // Apply saved git sync interval
        #[cfg(not(target_arch = "wasm32"))]
        workspace.set_git_sync_interval(state.settings.git_sync_interval.to_secs());

        // Initialize plugin system
        let plugin_shared_state = EditorPluginHost::create_shared_state();
        let plugin_host = EditorPluginHost::new(
            command_sender.clone(),
            async_runtime.clone(),
            state.theme,
            plugin_shared_state.clone(),
        );
        let plugin_host_ref: Arc<dyn crate::plugin::PluginHost> = Arc::new(plugin_host);
        let plugin_context: PluginContextRef =
            Arc::new(enya_plugin::PluginContext::new(plugin_host_ref.clone()));

        // Create plugin registry
        let mut plugin_registry = PluginRegistry::new();

        // Collect plugin errors to surface in diagnostics pane
        let mut plugin_errors: Vec<String> = Vec::new();

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
                            let msg = format!("Failed to register plugin '{name}': {e}");
                            log::warn!("{msg}");
                            plugin_errors.push(msg);
                        } else {
                            log::info!("Loaded plugin: {name}");
                        }
                    }
                    Err(e) => {
                        let msg = format!("Failed to load plugin: {e}");
                        log::warn!("{msg}");
                        plugin_errors.push(msg);
                    }
                }
            }

            // Load Lua script plugins
            for result in loader.load_all_lua() {
                match result {
                    Ok(plugin) => {
                        let name = plugin.name().to_string();
                        if let Err(e) = plugin_registry.register(plugin, true) {
                            let msg = format!("Failed to register Lua plugin '{name}': {e}");
                            log::warn!("{msg}");
                            plugin_errors.push(msg);
                        } else {
                            log::info!("Loaded Lua plugin: {name}");
                        }
                    }
                    Err(e) => {
                        let msg = format!("Failed to load Lua plugin: {e}");
                        log::warn!("{msg}");
                        plugin_errors.push(msg);
                    }
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
                let msg = format!("Failed to initialize plugin {id:?}: {e}");
                log::warn!("{msg}");
                plugin_errors.push(msg);
            } else if let Err(e) = plugin_registry.activate_plugin(id) {
                let msg = format!("Failed to activate plugin {id:?}: {e}");
                log::warn!("{msg}");
                plugin_errors.push(msg);
            }
        }

        // Surface plugin errors in diagnostics pane
        for error_msg in plugin_errors {
            use crate::components::overlay::diagnostics::{Diagnostic, DiagnosticSource};
            let diagnostic = Diagnostic::warning(error_msg).with_source(DiagnosticSource::Plugin);
            workspace.add_diagnostic(diagnostic);
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

        // Collect and register custom table pane types from plugins
        for config in plugin_registry.all_custom_table_panes() {
            workspace.register_custom_table_pane(config);
        }

        // Collect and register custom chart pane types from plugins
        for config in plugin_registry.all_custom_chart_panes() {
            workspace.register_custom_chart_pane(config);
        }

        // Collect and register custom stat pane types from plugins
        for config in plugin_registry.all_custom_stat_panes() {
            workspace.register_custom_stat_pane(config);
        }

        // Collect and register custom gauge pane types from plugins
        for config in plugin_registry.all_custom_gauge_panes() {
            workspace.register_custom_gauge_pane(config);
        }

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

        #[cfg(not(target_arch = "wasm32"))]
        let dismissed_update_version = state.settings.dismissed_update_version.clone();
        #[cfg(not(target_arch = "wasm32"))]
        let check_for_updates = state.settings.check_for_updates;

        #[cfg_attr(not(target_arch = "wasm32"), allow(unused_mut))]
        let mut github_auth = GitHubAuthManager::restore(
            state.settings.github_credentials.clone(),
            async_runtime.clone(),
            reqwest::Client::new(),
        );

        // On WASM, check if we arrived back from a GitHub OAuth redirect.
        // If so, navigate directly to the settings Auth page.
        #[cfg(target_arch = "wasm32")]
        let auth_callback_detected = github_auth.check_auth_callback(&cc.egui_ctx);
        #[cfg(not(target_arch = "wasm32"))]
        let auth_callback_detected = false;

        if auth_callback_detected {
            state.ui_state = UIState::Settings;
        }

        let mut settings_page = SettingsPage::new();
        if auth_callback_detected {
            settings_page
                .set_active_category(crate::components::settings_page::SettingsCategory::Profile);
        }

        Self {
            state,
            workspace,
            command_sender,
            command_receiver,
            connection: ConnectionManager::new(async_runtime.clone()),
            #[cfg(not(target_arch = "wasm32"))]
            update_checker: UpdateChecker::new(
                async_runtime.clone(),
                dismissed_update_version,
                check_for_updates,
            ),
            #[cfg(not(target_arch = "wasm32"))]
            manifest_fetcher: ManifestFetcher::new(async_runtime.clone()),
            status_line: StatusLine::new(),
            notifications: NotificationManager::new(),
            editor_metrics: EditorMetrics::default(),
            async_runtime,
            snapshot_http_client: reqwest::Client::new(),
            pending_snapshot_upload: std::sync::Arc::new(parking_lot::Mutex::new(None)),
            pending_snapshot_load: std::sync::Arc::new(parking_lot::Mutex::new(None)),
            #[cfg(not(target_arch = "wasm32"))]
            pending_screenshot_path: None,
            #[cfg(not(target_arch = "wasm32"))]
            is_fullscreen: false,
            #[cfg(target_arch = "wasm32")]
            checked_url_workspace: false,
            // Plugin system
            plugin_registry,
            plugin_context,
            plugin_shared_state,
            custom_themes,
            resolved_custom_theme: None,
            settings_page,
            github_auth,
            #[cfg(not(target_arch = "wasm32"))]
            install_plugin_ready: false,
            #[cfg(not(target_arch = "wasm32"))]
            startup_workspace: None,
        }
    }

    /// Set a workspace to load on the first frame (native only).
    ///
    /// Used by the CLI's `enya --workspace <name>` to open the editor with a specific workspace.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_startup_workspace(&mut self, name: String) {
        self.startup_workspace = Some(name);
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

    /// Get the effective theme (custom from plugin or builtin from settings).
    ///
    /// Use this method instead of `self.state.theme` when rendering UI components
    /// to ensure custom plugin themes are properly applied.
    fn effective_theme(&self) -> AppTheme {
        if let Some(ref custom) = self.resolved_custom_theme {
            AppTheme::Custom(crate::ui::ActiveThemeColors::from_custom(custom))
        } else {
            self.state.theme
        }
    }

    // Paints the bottom panel aka footer (lualine-style status bar)
    fn show_bottom_panel(&mut self, ctx: &egui::Context) {
        // Hide status line on landing page - it's part of the workspace UI, not the landing page
        if self.state.ui_state == UIState::Dashboard && self.workspace.is_landing_page() {
            return;
        }

        // Update status line state with effective theme (custom plugin theme if active)
        self.status_line.set_theme(self.effective_theme());

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
                } else if self.workspace.is_snapshot() {
                    StatusMode::Snapshot
                } else {
                    StatusMode::Normal
                }
            }
            UIState::Home => StatusMode::Home,
            UIState::Settings => StatusMode::Settings,
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

        let is_agent_mode = self.workspace.is_agent_mode();
        let effective_theme = self.effective_theme();

        egui::TopBottomPanel::bottom("bottom_panel")
            .resizable(false)
            .show(ctx, |ui| {
                // Status line with embedded agent input when in agent mode
                if is_agent_mode {
                    self.status_line.show_with_extra_content(ui, |ui| {
                        // Render the agent input bar inline within the status line
                        self.workspace
                            .show_agent_input_bar_inline(ui, ctx, effective_theme);
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
            UIState::Settings => self.draw_settings(ctx),
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

            // ==================== Plugin Pane Commands ====================
            UICommand::PluginAddQueryPane { query, title } => {
                self.workspace.add_query_pane(&query, title.as_deref());
                egui_ctx.request_repaint();
            }
            UICommand::PluginAddLogsPane => {
                self.workspace.add_logs_pane_from_plugin();
                egui_ctx.request_repaint();
            }
            UICommand::PluginAddTracingPane { trace_id } => {
                self.workspace.add_tracing_pane(trace_id.as_deref());
                egui_ctx.request_repaint();
            }
            UICommand::PluginAddTerminalPane => {
                self.workspace.add_terminal_pane();
                egui_ctx.request_repaint();
            }
            UICommand::PluginAddSqlPane => {
                self.workspace.add_sql_pane();
                egui_ctx.request_repaint();
            }
            UICommand::PluginCloseFocusedPane => {
                self.workspace.close_focused_pane();
                egui_ctx.request_repaint();
            }
            UICommand::PluginFocusPane { direction } => {
                self.workspace.focus_pane_in_direction(&direction);
                egui_ctx.request_repaint();
            }

            // ==================== Plugin Time Range Commands ====================
            UICommand::PluginSetTimeRangePreset { preset } => {
                self.workspace.set_time_range_preset_from_plugin(&preset);
                egui_ctx.request_repaint();
            }
            UICommand::PluginSetTimeRangeAbsolute { start_ms, end_ms } => {
                // Convert milliseconds back to seconds for the workspace API
                let start_secs = start_ms as f64 / 1000.0;
                let end_secs = end_ms as f64 / 1000.0;
                self.workspace
                    .set_time_range_absolute_from_plugin(start_secs, end_secs);
                egui_ctx.request_repaint();
            }

            // ==================== Plugin Custom Pane Commands ====================
            UICommand::PluginRegisterCustomTablePane { config } => {
                self.workspace.register_custom_table_pane(config);
            }
            UICommand::PluginAddCustomTablePane { pane_type } => {
                self.workspace.add_custom_table_pane(&pane_type);
                egui_ctx.request_repaint();
            }
            UICommand::PluginUpdateCustomTableData { pane_id, data } => {
                self.workspace.update_custom_table_data(pane_id, data);
                egui_ctx.request_repaint();
            }
            UICommand::PluginUpdateCustomTableDataByType { pane_type, data } => {
                self.workspace
                    .update_custom_table_data_by_type(&pane_type, data);
                egui_ctx.request_repaint();
            }

            // ==================== Plugin Custom Chart Pane Commands ====================
            UICommand::PluginRegisterCustomChartPane { config } => {
                self.workspace.register_custom_chart_pane(config);
            }
            UICommand::PluginAddCustomChartPane { pane_type } => {
                self.workspace.add_custom_chart_pane(&pane_type);
                egui_ctx.request_repaint();
            }
            UICommand::PluginUpdateCustomChartDataByType {
                pane_type,
                series,
                error,
            } => {
                // Convert hashable series back to plugin format
                let chart_data = if let Some(err) = error {
                    enya_plugin::CustomChartData::with_error(err)
                } else {
                    let plugin_series: Vec<enya_plugin::ChartSeries> =
                        series.iter().map(|s| s.to_plugin()).collect();
                    enya_plugin::CustomChartData::with_series(plugin_series)
                };
                self.workspace
                    .update_custom_chart_data_by_type(&pane_type, chart_data);
                egui_ctx.request_repaint();
            }

            // ==================== Plugin Custom Stat Pane Commands ====================
            UICommand::PluginRegisterCustomStatPane { config } => {
                self.workspace.register_custom_stat_pane(config);
            }
            UICommand::PluginAddCustomStatPane { pane_type } => {
                self.workspace.add_custom_stat_pane(&pane_type);
                egui_ctx.request_repaint();
            }
            UICommand::PluginUpdateCustomStatDataByType { pane_type, data } => {
                // Convert hashable data back to plugin format
                let stat_data = data.to_plugin();
                self.workspace
                    .update_custom_stat_data_by_type(&pane_type, stat_data);
                egui_ctx.request_repaint();
            }

            // ==================== Plugin Custom Gauge Pane Commands ====================
            UICommand::PluginRegisterCustomGaugePane { config } => {
                self.workspace.register_custom_gauge_pane(config);
            }
            UICommand::PluginAddCustomGaugePane { pane_type } => {
                self.workspace.add_custom_gauge_pane(&pane_type);
                egui_ctx.request_repaint();
            }
            UICommand::PluginUpdateCustomGaugeDataByType { pane_type, data } => {
                // Convert hashable data back to plugin format
                let gauge_data = data.to_plugin();
                self.workspace
                    .update_custom_gauge_data_by_type(&pane_type, gauge_data);
                egui_ctx.request_repaint();
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::InstallCommunityPlugin { name, file } => {
                self.install_community_plugin(&name, &file);
            }
            #[cfg(not(target_arch = "wasm32"))]
            UICommand::RefreshCommunityPlugins => {
                self.refresh_community_plugins();
            }
            #[cfg(target_arch = "wasm32")]
            UICommand::InstallCommunityPlugin { .. } | UICommand::RefreshCommunityPlugins => {
                // Community plugins not supported on WASM
            }
        }
    }

    #[inline]
    fn draw_home(&mut self, ctx: &egui::Context) {
        let theme = self.effective_theme();
        egui::CentralPanel::default().show(ctx, |ui| {
            welcome_section_ui(ui, theme);
        });
    }

    fn draw_settings(&mut self, ctx: &egui::Context) {
        self.settings_page.set_theme(self.effective_theme());
        self.settings_page.set_github_auth_state(
            self.github_auth.state().clone(),
            self.github_auth.avatar_bytes(),
            ctx,
        );
        self.github_auth.poll(ctx);

        let mut page_result = SettingsPageResult::None;

        egui::CentralPanel::default().show(ctx, |ui| {
            page_result = self.settings_page.show(ui, ctx);
        });

        // Persist credentials if auth just completed
        if let Some(creds) = self.github_auth.credentials() {
            if self
                .state
                .settings
                .github_credentials
                .as_ref()
                .map(|c| &c.access_token)
                != Some(&creds.access_token)
            {
                self.state.settings.github_credentials = Some(creds.clone());
            }
        }

        match page_result {
            SettingsPageResult::None => {}
            SettingsPageResult::GitHubSignIn => {
                self.github_auth.start_sign_in(ctx);
            }
            SettingsPageResult::GitHubSignOut => {
                self.github_auth.sign_out();
                self.state.settings.github_credentials = None;
            }
            SettingsPageResult::GoBack | SettingsPageResult::Saved { .. } => {
                // Save settings if provided
                if let SettingsPageResult::Saved {
                    ai_provider,
                    ai_model,
                    git_repo_url,
                    default_prometheus_endpoint,
                    default_loki_endpoint,
                    default_flight_sql_endpoint,
                    default_workspace,
                    timezone,
                    default_time_range,
                    startup_page,
                    check_for_updates,
                    notify_new_models,
                    git_sync_interval,
                } = page_result
                {
                    self.state.settings.ai_provider = ai_provider;
                    self.state.settings.ai_model = ai_model.clone();
                    self.state.settings.git_repo_url = git_repo_url;
                    self.state.settings.default_prometheus_endpoint = default_prometheus_endpoint;
                    self.state.settings.default_loki_endpoint = default_loki_endpoint;
                    self.state.settings.default_flight_sql_endpoint = default_flight_sql_endpoint;
                    self.state.settings.default_workspace = default_workspace;
                    self.state.settings.timezone = timezone;
                    self.state.settings.default_time_range = default_time_range;
                    self.state.settings.startup_page = startup_page;
                    self.state.settings.check_for_updates = check_for_updates;
                    self.state.settings.notify_new_models = notify_new_models;
                    self.state.settings.git_sync_interval = git_sync_interval;
                    #[cfg(not(target_arch = "wasm32"))]
                    self.update_checker.set_enabled(check_for_updates);
                    // Propagate provider/model to agent panel
                    self.workspace
                        .set_agent_provider_and_model(ai_provider, ai_model);
                    // Propagate git sync interval to codebase manager
                    #[cfg(not(target_arch = "wasm32"))]
                    self.workspace
                        .set_git_sync_interval(git_sync_interval.to_secs());
                }
                self.state.ui_state = self.state.previous_ui_state;
            }
            SettingsPageResult::ThemePreview(theme) => {
                // Clear custom theme and preview builtin
                self.state.custom_theme = None;
                self.resolved_custom_theme = None;
                self.command_sender.send_ui(UICommand::Theme(theme));
            }
            SettingsPageResult::CustomThemePreview(name) => {
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
            }
            SettingsPageResult::FontPreview(font) => {
                self.state.settings.font = font;
                fonts::setup_fonts(ctx, font);
            }
            SettingsPageResult::CancelledWithRestore {
                theme,
                custom_theme,
                font,
            } => {
                if let Some(name) = custom_theme {
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
                } else {
                    self.state.custom_theme = None;
                    self.resolved_custom_theme = None;
                    self.command_sender.send_ui(UICommand::Theme(theme));
                }
                self.state.settings.font = font;
                fonts::setup_fonts(ctx, font);
                self.state.ui_state = self.state.previous_ui_state;
            }
        }
    }

    #[profiling::function]
    fn draw_workspace(&mut self, ctx: &egui::Context) {
        // On native, load startup workspace on first frame if specified via CLI
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ws_name) = self.startup_workspace.take() {
            self.load_workspace(&ws_name);
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
            } else if let Some(snapshot_id) = Self::get_url_snapshot_param() {
                // Blob snapshot: async fetch from server, then decode and load
                self.fetch_snapshot(ctx, &snapshot_id);
            }
        }

        let mut workspace_action = WorkspaceAction::None;

        egui::CentralPanel::default().show(ctx, |ui| {
            // Update active theme colors (from custom or builtin theme)
            self.workspace
                .set_active_colors(self.effective_theme().active_colors());

            workspace_action = self.workspace.show(ui, ctx, &self.state);

            // Poll for pane interactions (e.g., chart drilldown clicks)
            self.workspace.poll_pane_interactions();

            // Poll for community plugin actions (native only)
            #[cfg(not(target_arch = "wasm32"))]
            {
                if self.workspace.take_pending_refresh_plugins() {
                    self.refresh_community_plugins();
                }
                // Delay plugin installation by one frame to allow spinner to render
                if self.workspace.has_pending_install_plugin() {
                    if self.install_plugin_ready {
                        // Second frame: actually install
                        if let Some((name, file)) = self.workspace.take_pending_install_plugin() {
                            self.install_community_plugin(&name, &file);
                        }
                        self.install_plugin_ready = false;
                    } else {
                        // First frame: just set ready flag, let UI render spinner
                        self.install_plugin_ready = true;
                    }
                }
                // Handle plugin removal
                if self.workspace.has_pending_remove_plugin() {
                    if let Some(name) = self.workspace.take_pending_remove_plugin() {
                        self.remove_plugin(&name);
                    }
                }
            }
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
                // Context-aware: snapshot if panes have data, config-only otherwise
                let url = if self.workspace.has_pane_data() {
                    self.build_snapshot_workspace_url()
                } else {
                    self.build_share_workspace_url()
                };
                if let Some(url) = url {
                    self.copy_share_url(ctx, &url, "Snapshot URL copied to clipboard");
                }
            }
            WorkspaceAction::SharePane(pane_index) => {
                // Context-aware: snapshot if panes have data, config-only otherwise
                let url = if self.workspace.has_pane_data() {
                    self.build_snapshot_pane_url(pane_index)
                } else {
                    self.build_share_pane_url(pane_index)
                };
                if let Some(url) = url {
                    self.copy_share_url(ctx, &url, "Pane snapshot URL copied to clipboard");
                }
            }
            WorkspaceAction::ShareSelectedPanes(indices) => {
                let count = indices.len();
                // Context-aware: snapshot if selected panes have data, config-only otherwise
                let url = if self.workspace.has_pane_data_for_indices(&indices) {
                    self.build_snapshot_selected_url(&indices)
                } else {
                    self.build_share_selected_url(&indices)
                };
                if let Some(url) = url {
                    self.copy_share_url(
                        ctx,
                        &url,
                        &format!("{count} panes snapshot URL copied to clipboard"),
                    );
                }
            }
            WorkspaceAction::ShareLiveWorkspace => {
                if let Some(url) = self.build_share_workspace_url() {
                    self.copy_share_url(ctx, &url, "Workspace URL copied to clipboard");
                }
            }
            WorkspaceAction::ShareLivePane(pane_index) => {
                if let Some(url) = self.build_share_pane_url(pane_index) {
                    self.copy_share_url(ctx, &url, "Pane URL copied to clipboard");
                }
            }
            WorkspaceAction::UploadSnapshot => {
                self.upload_snapshot(ctx);
            }
            WorkspaceAction::OpenSnapshot(id) => {
                self.fetch_snapshot(ctx, &id);
            }
            WorkspaceAction::QuitApp => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            WorkspaceAction::OpenAnnotationEditor => {
                // Open annotation editor on the focused pane
                self.workspace.open_annotation_editor();
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
            WorkspaceAction::SaveSettings {
                ai_provider,
                ai_model,
                git_repo_url,
                default_prometheus_endpoint,
                default_loki_endpoint,
                default_flight_sql_endpoint,
            } => {
                self.state.settings.ai_provider = ai_provider;
                self.state.settings.ai_model = ai_model.clone();
                self.state.settings.git_repo_url = git_repo_url;
                self.state.settings.default_prometheus_endpoint = default_prometheus_endpoint;
                self.state.settings.default_loki_endpoint = default_loki_endpoint;
                self.state.settings.default_flight_sql_endpoint = default_flight_sql_endpoint;
                // Propagate provider/model to agent panel
                self.workspace
                    .set_agent_provider_and_model(ai_provider, ai_model);
            }
            WorkspaceAction::OpenSettings => {
                // Collect custom themes for the settings page
                let custom_theme_list: Vec<(String, String, crate::ui::ActiveThemeColors)> = self
                    .plugin_registry
                    .all_themes()
                    .into_iter()
                    .map(|t| {
                        let resolved =
                            crate::ui::custom_theme::ResolvedCustomTheme::from_definition(&t);
                        let colors = crate::ui::ActiveThemeColors::from_custom(&resolved);
                        (t.name, t.display_name, colors)
                    })
                    .collect();

                self.state.previous_ui_state = self.state.ui_state;
                self.state.ui_state = UIState::Settings;
                let available_workspaces: Vec<String> = Self::list_available_workspaces()
                    .into_iter()
                    .map(|(name, _)| name)
                    .collect();

                self.settings_page.open(
                    self.state.settings.ai_provider,
                    self.state.settings.ai_model.clone(),
                    self.state.settings.git_repo_url.clone(),
                    self.state.settings.default_prometheus_endpoint.clone(),
                    self.state.settings.default_loki_endpoint.clone(),
                    self.state.settings.default_flight_sql_endpoint.clone(),
                    self.effective_theme(),
                    self.state.custom_theme.clone(),
                    self.state.settings.font,
                    custom_theme_list,
                    self.github_auth.state().clone(),
                    self.state.settings.default_workspace.clone(),
                    available_workspaces,
                    self.state.settings.timezone,
                    self.state.settings.default_time_range,
                    self.state.settings.startup_page,
                    self.state.settings.check_for_updates,
                    self.state.settings.notify_new_models,
                    self.state.settings.git_sync_interval,
                );
            }
        }
    }

    fn open_command_palette(&mut self) {
        self.workspace.open_command_palette();
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

    /// Poll plugin panes for auto-refresh based on their refresh intervals.
    ///
    /// This checks which plugin pane types need to be refreshed and triggers
    /// their refresh callbacks.
    fn poll_plugin_pane_refreshes(&mut self) {
        // Get all refreshable pane types from the plugin registry
        let refreshable = self.plugin_registry.all_refreshable_pane_types();
        if refreshable.is_empty() {
            return;
        }

        // Check which pane types need to be refreshed
        let pending = self.workspace.get_pending_plugin_refreshes(&refreshable);
        if pending.is_empty() {
            return;
        }

        // Trigger refresh for each pending pane type
        for pane_type in pending {
            if self.plugin_registry.trigger_pane_refresh(&pane_type) {
                // Mark as refreshed after successful trigger
                self.workspace.mark_plugin_pane_refreshed(&pane_type);
                log::debug!("Auto-refreshed plugin pane type '{pane_type}'");
            }
        }
    }

    /// Update plugin shared state with current focused pane information.
    ///
    /// This is called each frame to keep the plugin system informed about
    /// which pane is currently focused, enabling features like "share to Slack".
    fn update_plugin_shared_state(&self) {
        let focused_pane_info = self.workspace.get_focused_pane_info();
        let mut state = self.plugin_shared_state.write();
        state.focused_pane = focused_pane_info;
    }

    // ==================== Community Plugin Methods ====================

    /// Refresh the installed plugins list and commands from the registry.
    ///
    /// Call this after installing/updating a plugin to update the plugins overlay
    /// and command palette with new commands.
    #[cfg(not(target_arch = "wasm32"))]
    fn refresh_installed_plugins(&mut self) {
        // Refresh plugin commands for the command palette
        let plugin_commands: Vec<crate::components::DynamicCommand> = self
            .plugin_registry
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
        self.workspace.set_plugin_commands(plugin_commands);

        // Refresh installed plugins list for the plugins overlay
        let plugins_info: Vec<crate::components::PluginDisplayInfo> = self
            .plugin_registry
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
                let commands = self.plugin_registry.commands_for_plugin(info.id);
                let keybindings = self.plugin_registry.keybindings_for_plugin(info.id);

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
        self.workspace.set_plugins(plugins_info);
    }

    /// Refresh the list of available community plugins from the remote registry.
    #[cfg(not(target_arch = "wasm32"))]
    fn refresh_community_plugins(&mut self) {
        use crate::components::overlay::plugins::CommunityPluginInfo;

        let plugins_url = std::env::var("ENYA_PLUGINS_URL").unwrap_or_else(|_| {
            "https://raw.githubusercontent.com/meldrumlabs/enya/main/plugins".to_string()
        });

        self.workspace.set_plugins_loading(true);

        let index_url = format!("{plugins_url}/index.toml");

        match ureq::get(&index_url).call() {
            Ok(response) => match response.into_body().read_to_string() {
                Ok(body) => match body.parse::<toml::Table>() {
                    Ok(table) => {
                        let mut plugins = Vec::new();
                        if let Some(toml::Value::Array(plugin_array)) = table.get("plugins") {
                            for plugin in plugin_array {
                                if let toml::Value::Table(p) = plugin {
                                    let name = p
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let version = p
                                        .get("version")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("0.0.0")
                                        .to_string();
                                    let description = p
                                        .get("description")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let author = p
                                        .get("author")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let file = p
                                        .get("file")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    if !name.is_empty() && !file.is_empty() {
                                        plugins.push(CommunityPluginInfo {
                                            name,
                                            version,
                                            description,
                                            author,
                                            file,
                                            installed: false,
                                        });
                                    }
                                }
                            }
                        }
                        self.workspace.set_available_plugins(plugins);
                        log::info!("Refreshed community plugins list");
                    }
                    Err(e) => {
                        log::warn!("Failed to parse community plugins index: {e}");
                        self.workspace.set_plugins_loading(false);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read community plugins index: {e}");
                    self.workspace.set_plugins_loading(false);
                }
            },
            Err(e) => {
                log::warn!("Failed to fetch community plugins index: {e}");
                self.workspace.set_plugins_loading(false);
            }
        }
    }

    /// Remove an installed plugin by unregistering it and deleting the file.
    #[cfg(not(target_arch = "wasm32"))]
    fn remove_plugin(&mut self, name: &str) {
        use crate::components::Notification;
        use crate::components::NotificationLevel;

        // Unregister the plugin from the registry
        if let Err(e) = self.plugin_registry.unregister_plugin_by_name(name) {
            log::warn!("Failed to unregister plugin '{name}': {e}");
        }

        // Find and delete the plugin file
        let Some(home_dir) = dirs::home_dir() else {
            self.notifications.notify(Notification::new(
                "Failed to remove plugin: could not find home directory",
                NotificationLevel::Error,
            ));
            return;
        };

        let plugins_dir = home_dir.join(".config").join("enya").join("plugins");

        // Look for .lua file with plugin name
        let plugin_file = plugins_dir.join(format!("{name}.lua"));
        if plugin_file.exists() {
            if let Err(e) = std::fs::remove_file(&plugin_file) {
                self.notifications.notify(Notification::new(
                    format!("Failed to delete plugin file: {e}"),
                    NotificationLevel::Error,
                ));
                return;
            }
        }

        // Refresh the installed plugins list
        self.refresh_installed_plugins();

        self.notifications.notify(Notification::new(
            format!("Plugin '{name}' removed"),
            NotificationLevel::Success,
        ));
        log::info!("Removed plugin: {name}");
    }

    /// Install a community plugin by downloading it to the local plugins directory.
    #[cfg(not(target_arch = "wasm32"))]
    fn install_community_plugin(&mut self, name: &str, file: &str) {
        use crate::components::Notification;
        use crate::components::NotificationLevel;
        use std::io::Write;

        let plugins_url = std::env::var("ENYA_PLUGINS_URL").unwrap_or_else(|_| {
            "https://raw.githubusercontent.com/meldrumlabs/enya/main/plugins".to_string()
        });

        let plugin_url = format!("{plugins_url}/{file}");

        let Some(home_dir) = dirs::home_dir() else {
            self.notifications.notify(Notification::new(
                "Failed to install plugin: could not find home directory",
                NotificationLevel::Error,
            ));
            return;
        };

        let plugins_dir = home_dir.join(".config").join("enya").join("plugins");

        if let Err(e) = std::fs::create_dir_all(&plugins_dir) {
            self.notifications.notify(Notification::new(
                format!("Failed to create plugins directory: {e}"),
                NotificationLevel::Error,
            ));
            return;
        }

        let plugin_path = plugins_dir.join(file);

        match ureq::get(&plugin_url).call() {
            Ok(response) => match response.into_body().read_to_string() {
                Ok(content) => match std::fs::File::create(&plugin_path) {
                    Ok(mut file_handle) => {
                        if let Err(e) = file_handle.write_all(content.as_bytes()) {
                            self.notifications.notify(Notification::new(
                                format!("Failed to write plugin file: {e}"),
                                NotificationLevel::Error,
                            ));
                            return;
                        }

                        // Check if plugin is already registered (update vs fresh install)
                        let already_registered = self.plugin_registry.info_by_name(name).is_some();

                        // For updates, unregister the old version first for hot-reload
                        if already_registered {
                            if let Err(e) = self.plugin_registry.unregister_plugin_by_name(name) {
                                log::warn!("Failed to unregister old plugin '{name}': {e}");
                            }
                        }

                        // Load and activate the plugin (works for both fresh install and update)
                        match crate::plugin::LuaPlugin::load(&plugin_path) {
                            Ok(plugin) => match self.plugin_registry.register(plugin, true) {
                                Ok(id) => {
                                    if let Err(e) = self.plugin_registry.init_plugin(id) {
                                        log::warn!("Failed to initialize plugin '{name}': {e}");
                                        self.notifications.notify(Notification::new(
                                            format!(
                                                "Installed plugin '{name}'. Restart Enya to activate."
                                            ),
                                            NotificationLevel::Success,
                                        ));
                                    } else if let Err(e) = self.plugin_registry.activate_plugin(id)
                                    {
                                        log::warn!("Failed to activate plugin '{name}': {e}");
                                        self.notifications.notify(Notification::new(
                                            format!(
                                                "Installed plugin '{name}'. Restart Enya to activate."
                                            ),
                                            NotificationLevel::Success,
                                        ));
                                    } else {
                                        let action = if already_registered {
                                            "updated"
                                        } else {
                                            "installed"
                                        };
                                        self.notifications.notify(Notification::new(
                                            format!("Plugin '{name}' {action} and activated!"),
                                            NotificationLevel::Success,
                                        ));
                                        log::info!(
                                            "{} and activated community plugin: {name}",
                                            if already_registered {
                                                "Updated"
                                            } else {
                                                "Installed"
                                            }
                                        );
                                        // Refresh installed plugins list to update the UI
                                        self.refresh_installed_plugins();
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Failed to register plugin '{name}': {e}");
                                    self.notifications.notify(Notification::new(
                                        format!(
                                            "Installed plugin '{name}'. Restart Enya to activate."
                                        ),
                                        NotificationLevel::Success,
                                    ));
                                }
                            },
                            Err(e) => {
                                log::warn!("Failed to load plugin '{name}': {e}");
                                self.notifications.notify(Notification::new(
                                    format!("Installed plugin '{name}'. Restart Enya to activate."),
                                    NotificationLevel::Success,
                                ));
                            }
                        }

                        log::info!("Installed community plugin: {name} to {plugin_path:?}");
                        self.refresh_community_plugins();
                    }
                    Err(e) => {
                        self.notifications.notify(Notification::new(
                            format!("Failed to create plugin file: {e}"),
                            NotificationLevel::Error,
                        ));
                    }
                },
                Err(e) => {
                    self.notifications.notify(Notification::new(
                        format!("Failed to read plugin content: {e}"),
                        NotificationLevel::Error,
                    ));
                }
            },
            Err(e) => {
                self.notifications.notify(Notification::new(
                    format!("Failed to download plugin: {e}"),
                    NotificationLevel::Error,
                ));
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

        // Poll snapshot uploads for completed results
        self.poll_snapshot_upload(ctx);

        // Poll snapshot loads for completed fetches
        self.poll_snapshot_load(ctx);

        // Poll plugin pane refreshes (auto-refresh based on intervals)
        self.poll_plugin_pane_refreshes();

        // Update plugin shared state with current focused pane info
        self.update_plugin_shared_state();

        // Custom titlebar with window controls and drag area
        // Replaces native macOS titlebar for seamless theme integration
        #[cfg(not(target_arch = "wasm32"))]
        {
            let theme = self.effective_theme();
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

        // Draw notifications (on top of everything) with effective theme
        self.notifications.set_theme(self.effective_theme());
        self.notifications.show(ctx);

        // Poll for remote provider manifest updates (native only)
        #[cfg(not(target_arch = "wasm32"))]
        self.manifest_fetcher.poll(ctx);

        // Poll for update availability and show banner if needed (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.update_checker.poll(ctx);
            if let Some(update_info) = self.update_checker.available_update().cloned() {
                let mut banner = UpdateBanner::new(
                    update_info.version.clone(),
                    update_info.release_url.clone(),
                    update_info.release_notes.clone(),
                    update_info.download_url.is_some(),
                );
                banner.set_theme(self.effective_theme());
                banner.set_downloading(self.update_checker.is_downloading());
                match banner.show(ctx) {
                    UpdateBannerAction::SeeChanges(url) => {
                        ctx.open_url(egui::OpenUrl::new_tab(&url));
                    }
                    UpdateBannerAction::Restart => {
                        if let Some(ref url) = update_info.download_url {
                            self.update_checker.download_and_update(url, ctx);
                        }
                    }
                    UpdateBannerAction::Dismissed(version) => {
                        self.state.settings.dismissed_update_version = Some(version.clone());
                        self.update_checker.dismiss(version);
                    }
                    UpdateBannerAction::None => {}
                }
            }
        }

        // Check for possible key board shortcut triggers
        self.check_keyboard_shortcuts(ctx);

        // Run any pending ui commands which updates internal data before the next frame
        self.run_pending_ui_commands(ctx);
    }
}
