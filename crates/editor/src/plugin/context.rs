//! Plugin context providing access to editor services.
//!
//! This module provides the `EditorPluginHost` which implements `enya_plugin::PluginHost`,
//! allowing plugins to interact with the editor through a controlled interface.

use std::sync::Arc;

use enya_plugin::{
    BoxFuture, HttpError, HttpResponse, LogLevel, NotificationLevel, PluginHost, Theme,
};
use rustc_hash::FxHashMap;

use crate::AsyncRuntime;
use crate::command::{CommandSender, UICommandSender};
use crate::ui::theme::AppTheme;

/// Reference-counted plugin context for shared access.
pub type PluginContextRef = Arc<enya_plugin::PluginContext>;

/// Create a plugin context from an EditorPluginHost.
pub type PluginContext = enya_plugin::PluginContext;

/// Editor implementation of the PluginHost trait.
///
/// This adapter bridges the enya-plugin system with the editor's internal
/// services (command sender, async runtime, theme, etc.).
pub struct EditorPluginHost {
    /// Channel for sending UI commands
    command_sender: CommandSender,
    /// Async runtime for spawning background tasks
    async_runtime: AsyncRuntime,
    /// Current theme
    theme: AppTheme,
    /// Editor version
    editor_version: &'static str,
    /// Whether the editor is running in WASM
    is_wasm: bool,
}

impl EditorPluginHost {
    /// Create a new editor plugin host.
    pub fn new(
        command_sender: CommandSender,
        async_runtime: AsyncRuntime,
        theme: AppTheme,
    ) -> Self {
        Self {
            command_sender,
            async_runtime,
            theme,
            editor_version: env!("CARGO_PKG_VERSION"),
            #[cfg(target_arch = "wasm32")]
            is_wasm: true,
            #[cfg(not(target_arch = "wasm32"))]
            is_wasm: false,
        }
    }

    /// Get the command sender for sending UI commands.
    pub fn command_sender(&self) -> &CommandSender {
        &self.command_sender
    }

    /// Get the async runtime for spawning background tasks.
    pub fn async_runtime(&self) -> &AsyncRuntime {
        &self.async_runtime
    }

    /// Update the current theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Get the current app theme.
    pub fn app_theme(&self) -> AppTheme {
        self.theme
    }
}

impl PluginHost for EditorPluginHost {
    fn notify(&self, level: NotificationLevel, message: &str) {
        use crate::command::UICommand;
        let level_str = match level {
            NotificationLevel::Info => "info",
            NotificationLevel::Warning => "warn",
            NotificationLevel::Error => "error",
        };
        self.command_sender.send_ui(UICommand::Notify {
            level: level_str.to_string(),
            message: message.to_string(),
        });
    }

    fn request_repaint(&self) {
        use crate::command::UICommand;
        self.command_sender.send_ui(UICommand::Repaint);
    }

    fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Debug => log::debug!("[plugin] {message}"),
            LogLevel::Info => log::info!("[plugin] {message}"),
            LogLevel::Warn => log::warn!("[plugin] {message}"),
            LogLevel::Error => log::error!("[plugin] {message}"),
        }
    }

    fn version(&self) -> &'static str {
        self.editor_version
    }

    fn is_wasm(&self) -> bool {
        self.is_wasm
    }

    fn theme(&self) -> Theme {
        match self.theme {
            AppTheme::Light | AppTheme::Stockholm => Theme::Light,
            // All dark-ish themes map to Dark
            _ => Theme::Dark,
        }
    }

    fn theme_name(&self) -> &'static str {
        match self.theme {
            AppTheme::Custom(_) => "custom",
            AppTheme::Dark => "dark",
            AppTheme::Light => "light",
            AppTheme::Midnight => "midnight",
            AppTheme::Nord => "nord",
            AppTheme::Catppuccin => "catppuccin",
            AppTheme::Ayu => "ayu-dark",
            AppTheme::Bergman => "bergman",
            AppTheme::Aurora => "aurora",
            AppTheme::Stockholm => "stockholm",
            AppTheme::Graphite => "graphite",
            AppTheme::Ink => "ink",
            AppTheme::Midsommar => "midsommar",
            AppTheme::Skargard => "skargard",
        }
    }

    fn clipboard_write(&self, text: &str) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match arboard::Clipboard::new() {
                Ok(mut clipboard) => clipboard.set_text(text).is_ok(),
                Err(e) => {
                    log::warn!("Failed to access clipboard: {e}");
                    false
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = text;
            false // Clipboard not supported in WASM (would need web-sys integration)
        }
    }

    fn clipboard_read(&self) -> Option<String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            arboard::Clipboard::new()
                .ok()
                .and_then(|mut c| c.get_text().ok())
        }
        #[cfg(target_arch = "wasm32")]
        {
            None // Clipboard not supported in WASM
        }
    }

    fn spawn(&self, future: BoxFuture<()>) {
        #[cfg(not(target_arch = "wasm32"))]
        self.async_runtime.spawn(future);
        #[cfg(target_arch = "wasm32")]
        {
            let _ = future; // Unused on WASM
        }
    }

    fn http_get(
        &self,
        url: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut request = ureq::get(url);
            for (key, value) in headers {
                request = request.header(key.as_str(), value.as_str());
            }

            match request.call() {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let mut response_headers = FxHashMap::default();
                    for (name, value) in response.headers().iter() {
                        if let Ok(v) = value.to_str() {
                            response_headers.insert(name.to_string(), v.to_string());
                        }
                    }
                    match response.into_body().read_to_string() {
                        Ok(body) => Ok(HttpResponse {
                            status,
                            body,
                            headers: response_headers,
                        }),
                        Err(e) => Err(HttpError {
                            message: format!("Failed to read response body: {e}"),
                        }),
                    }
                }
                Err(e) => Err(HttpError {
                    message: format!("HTTP GET failed: {e}"),
                }),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (url, headers);
            Err(HttpError {
                message: "HTTP requests not supported in WASM".to_string(),
            })
        }
    }

    fn http_post(
        &self,
        url: &str,
        body: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut request = ureq::post(url);
            for (key, value) in headers {
                request = request.header(key.as_str(), value.as_str());
            }
            // Default to application/json if not specified
            if !headers.contains_key("Content-Type") && !headers.contains_key("content-type") {
                request = request.header("Content-Type", "application/json");
            }

            match request.send(body) {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let mut response_headers = FxHashMap::default();
                    for (name, value) in response.headers().iter() {
                        if let Ok(v) = value.to_str() {
                            response_headers.insert(name.to_string(), v.to_string());
                        }
                    }
                    match response.into_body().read_to_string() {
                        Ok(resp_body) => Ok(HttpResponse {
                            status,
                            body: resp_body,
                            headers: response_headers,
                        }),
                        Err(e) => Err(HttpError {
                            message: format!("Failed to read response body: {e}"),
                        }),
                    }
                }
                Err(e) => Err(HttpError {
                    message: format!("HTTP POST failed: {e}"),
                }),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (url, body, headers);
            Err(HttpError {
                message: "HTTP requests not supported in WASM".to_string(),
            })
        }
    }

    // ==================== Pane Management ====================

    fn add_query_pane(&self, query: &str, title: Option<&str>) {
        use crate::command::UICommand;
        self.command_sender.send_ui(UICommand::PluginAddQueryPane {
            query: query.to_string(),
            title: title.map(|t| t.to_string()),
        });
    }

    fn add_logs_pane(&self) {
        use crate::command::UICommand;
        self.command_sender.send_ui(UICommand::PluginAddLogsPane);
    }

    fn add_tracing_pane(&self, trace_id: Option<&str>) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginAddTracingPane {
                trace_id: trace_id.map(|t| t.to_string()),
            });
    }

    fn add_terminal_pane(&self) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginAddTerminalPane);
    }

    fn add_sql_pane(&self) {
        use crate::command::UICommand;
        self.command_sender.send_ui(UICommand::PluginAddSqlPane);
    }

    fn close_focused_pane(&self) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginCloseFocusedPane);
    }

    fn focus_pane(&self, direction: &str) {
        use crate::command::UICommand;
        self.command_sender.send_ui(UICommand::PluginFocusPane {
            direction: direction.to_string(),
        });
    }

    // ==================== Time Range ====================

    fn set_time_range_preset(&self, preset: &str) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginSetTimeRangePreset {
                preset: preset.to_string(),
            });
    }

    fn set_time_range_absolute(&self, start_secs: f64, end_secs: f64) {
        use crate::command::UICommand;
        // Convert seconds to milliseconds for hashable storage in UICommand
        let start_ms = (start_secs * 1000.0) as i64;
        let end_ms = (end_secs * 1000.0) as i64;
        self.command_sender
            .send_ui(UICommand::PluginSetTimeRangeAbsolute { start_ms, end_ms });
    }

    fn get_time_range(&self) -> (f64, f64) {
        // Note: Time range is managed by the workspace, and we can't synchronously
        // query it from here. This returns a cached/default value.
        // For accurate time range, plugins should use the async pattern or
        // the time range will be provided in callbacks.
        // Default to last 5 minutes from now.
        let now = crate::util::now_unix_secs_f64();
        let start = now - 300.0; // 5 minutes ago
        (start, now)
    }

    // ==================== Custom Panes ====================

    fn register_custom_table_pane(&self, config: enya_plugin::CustomTableConfig) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginRegisterCustomTablePane { config });
    }

    fn add_custom_table_pane(&self, pane_type: &str) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginAddCustomTablePane {
                pane_type: pane_type.to_string(),
            });
    }

    fn update_custom_table_data(&self, pane_id: usize, data: enya_plugin::CustomTableData) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginUpdateCustomTableData { pane_id, data });
    }

    fn update_custom_table_data_by_type(
        &self,
        pane_type: &str,
        data: enya_plugin::CustomTableData,
    ) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginUpdateCustomTableDataByType {
                pane_type: pane_type.to_string(),
                data,
            });
    }

    // ==================== Custom Chart Panes ====================

    fn register_custom_chart_pane(&self, config: enya_plugin::CustomChartConfig) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginRegisterCustomChartPane { config });
    }

    fn add_custom_chart_pane(&self, pane_type: &str) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginAddCustomChartPane {
                pane_type: pane_type.to_string(),
            });
    }

    fn update_custom_chart_data_by_type(
        &self,
        pane_type: &str,
        data: enya_plugin::CustomChartData,
    ) {
        use crate::command::{ChartSeriesHashable, UICommand};
        self.command_sender
            .send_ui(UICommand::PluginUpdateCustomChartDataByType {
                pane_type: pane_type.to_string(),
                series: data
                    .series
                    .iter()
                    .map(ChartSeriesHashable::from_plugin)
                    .collect(),
                error: data.error,
            });
    }

    // ==================== Custom Stat Panes ====================

    fn register_stat_pane(&self, config: enya_plugin::StatPaneConfig) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginRegisterCustomStatPane { config });
    }

    fn add_stat_pane(&self, pane_type: &str) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginAddCustomStatPane {
                pane_type: pane_type.to_string(),
            });
    }

    fn update_stat_data_by_type(&self, pane_type: &str, data: enya_plugin::StatPaneData) {
        use crate::command::{StatDataHashable, UICommand};
        self.command_sender
            .send_ui(UICommand::PluginUpdateCustomStatDataByType {
                pane_type: pane_type.to_string(),
                data: StatDataHashable::from_plugin(&data),
            });
    }

    // ==================== Custom Gauge Panes ====================

    fn register_gauge_pane(&self, config: enya_plugin::GaugePaneConfig) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginRegisterCustomGaugePane { config });
    }

    fn add_gauge_pane(&self, pane_type: &str) {
        use crate::command::UICommand;
        self.command_sender
            .send_ui(UICommand::PluginAddCustomGaugePane {
                pane_type: pane_type.to_string(),
            });
    }

    fn update_gauge_data_by_type(&self, pane_type: &str, data: enya_plugin::GaugePaneData) {
        use crate::command::{GaugeDataHashable, UICommand};
        self.command_sender
            .send_ui(UICommand::PluginUpdateCustomGaugeDataByType {
                pane_type: pane_type.to_string(),
                data: GaugeDataHashable::from_plugin(&data),
            });
    }
}
