use rustc_hash::FxHashMap;

use crate::types::{
    BoxFuture, CustomChartConfig, CustomChartData, CustomTableConfig, CustomTableData,
    FocusedPaneInfo, GaugePaneConfig, GaugePaneData, HttpError, HttpResponse, LogLevel,
    NotificationLevel, PluginHost, StatPaneConfig, StatPaneData, Theme,
};

/// A headless implementation of `PluginHost` for CLI use.
///
/// Provides real implementations for non-UI methods (HTTP, logging, notifications)
/// and no-ops for UI-specific methods (panes, time range, custom visualizations).
pub struct HeadlessPluginHost;

impl PluginHost for HeadlessPluginHost {
    fn notify(&self, level: NotificationLevel, message: &str) {
        let prefix = match level {
            NotificationLevel::Info => "info",
            NotificationLevel::Warning => "warn",
            NotificationLevel::Error => "error",
        };
        println!("[{prefix}] {message}");
    }

    fn request_repaint(&self) {
        // No-op: no UI to repaint
    }

    fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Debug => log::debug!("{message}"),
            LogLevel::Info => log::info!("{message}"),
            LogLevel::Warn => log::warn!("{message}"),
            LogLevel::Error => log::error!("{message}"),
        }
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn is_wasm(&self) -> bool {
        false
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn theme_name(&self) -> &'static str {
        "dark"
    }

    fn clipboard_write(&self, _text: &str) -> bool {
        false
    }

    fn clipboard_read(&self) -> Option<String> {
        None
    }

    fn spawn(&self, _future: BoxFuture<()>) {
        log::warn!("spawn() not available in headless mode");
    }

    fn http_get(
        &self,
        url: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError> {
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

    fn http_post(
        &self,
        url: &str,
        body: &str,
        headers: &FxHashMap<String, String>,
    ) -> Result<HttpResponse, HttpError> {
        let mut request = ureq::post(url);
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }
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

    // Pane management — no-ops in headless mode

    fn add_query_pane(&self, _query: &str, _title: Option<&str>) {}
    fn add_logs_pane(&self) {}
    fn add_tracing_pane(&self, _trace_id: Option<&str>) {}
    fn add_terminal_pane(&self) {}
    fn add_sql_pane(&self) {}
    fn close_focused_pane(&self) {}
    fn focus_pane(&self, _direction: &str) {}

    // Time range — no-ops / defaults

    fn set_time_range_preset(&self, _preset: &str) {}
    fn set_time_range_absolute(&self, _start_secs: f64, _end_secs: f64) {}
    fn get_time_range(&self) -> (f64, f64) {
        (0.0, 0.0)
    }

    // Custom panes — no-ops

    fn register_custom_table_pane(&self, _config: CustomTableConfig) {}
    fn add_custom_table_pane(&self, _pane_type: &str) {}
    fn update_custom_table_data(&self, _pane_id: usize, _data: CustomTableData) {}
    fn update_custom_table_data_by_type(&self, _pane_type: &str, _data: CustomTableData) {}

    fn register_custom_chart_pane(&self, _config: CustomChartConfig) {}
    fn add_custom_chart_pane(&self, _pane_type: &str) {}
    fn update_custom_chart_data_by_type(&self, _pane_type: &str, _data: CustomChartData) {}

    fn register_stat_pane(&self, _config: StatPaneConfig) {}
    fn add_stat_pane(&self, _pane_type: &str) {}
    fn update_stat_data_by_type(&self, _pane_type: &str, _data: StatPaneData) {}

    fn register_gauge_pane(&self, _config: GaugePaneConfig) {}
    fn add_gauge_pane(&self, _pane_type: &str) {}
    fn update_gauge_data_by_type(&self, _pane_type: &str, _data: GaugePaneData) {}

    fn get_focused_pane_info(&self) -> Option<FocusedPaneInfo> {
        None
    }
}
