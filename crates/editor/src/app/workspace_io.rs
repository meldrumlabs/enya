//! Workspace I/O operations.
//!
//! This module handles saving, loading, listing, and sharing workspaces.
//! On native platforms, workspaces are stored as TOML files in the
//! `.enya/workspaces` directory. On WASM, workspaces are encoded as
//! base64 URL parameters.

use crate::components::{Notification, NotificationLevel};

/// The canonical base URL for the web editor, used in all share links.
const EDITOR_BASE_URL: &str = "https://enya.build/editor";

use super::EnyaApp;

impl EnyaApp {
    /// Get the workspace directory path
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn workspace_dir() -> std::path::PathBuf {
        enya_config::workspace_dir()
    }

    /// List available workspace files from the workspace directory
    #[cfg(not(target_arch = "wasm32"))]
    pub fn list_available_workspaces() -> Vec<(String, Option<String>)> {
        enya_config::list_workspaces()
    }

    /// List available workspace files (WASM stub - returns empty)
    #[cfg(target_arch = "wasm32")]
    pub fn list_available_workspaces() -> Vec<(String, Option<String>)> {
        Vec::new()
    }

    /// Ensure the default example workspaces exist
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn ensure_default_workspace() {
        use crate::workspace::{
            ATLAS_WORKSPACE_TOML, COMPLEX_VIEWPORT_TOML, DEFAULT_WORKSPACE_TOML,
            DEMO_WORKSPACE_TOML,
        };

        let dir = Self::workspace_dir();

        // Create example workspace if it doesn't exist
        let example_path = dir.join("example.toml");
        if !example_path.exists() {
            if let Err(e) = std::fs::write(&example_path, DEFAULT_WORKSPACE_TOML) {
                log::warn!("Failed to create default workspace: {e}");
            } else {
                log::info!("Created default workspace: {}", example_path.display());
            }
        }

        // Create complex viewport workspace if it doesn't exist
        let viewport_path = dir.join("viewport.toml");
        if !viewport_path.exists() {
            if let Err(e) = std::fs::write(&viewport_path, COMPLEX_VIEWPORT_TOML) {
                log::warn!("Failed to create viewport workspace: {e}");
            } else {
                log::info!("Created viewport workspace: {}", viewport_path.display());
            }
        }

        // Create demo workspace if it doesn't exist
        let demo_path = dir.join("demo.toml");
        if !demo_path.exists() {
            if let Err(e) = std::fs::write(&demo_path, DEMO_WORKSPACE_TOML) {
                log::warn!("Failed to create demo workspace: {e}");
            } else {
                log::info!("Created demo workspace: {}", demo_path.display());
            }
        }

        // Create atlas workspace if it doesn't exist
        let atlas_path = dir.join("atlas.toml");
        if !atlas_path.exists() {
            if let Err(e) = std::fs::write(&atlas_path, ATLAS_WORKSPACE_TOML) {
                log::warn!("Failed to create atlas workspace: {e}");
            } else {
                log::info!("Created atlas workspace: {}", atlas_path.display());
            }
        }
    }

    /// Get the base URL for share links.
    /// On WASM, uses the current page origin/path so self-hosted deployments work.
    /// On native, falls back to the public editor URL.
    #[cfg(target_arch = "wasm32")]
    fn share_base_url() -> String {
        let base_url = web_sys::window()
            .and_then(|w| w.location().href().ok())
            .unwrap_or_else(|| EDITOR_BASE_URL.to_string());
        // Strip any existing query string
        base_url.split('?').next().unwrap_or(&base_url).to_string()
    }

    /// Save the current workspace to a file
    pub(super) fn save_workspace(&mut self, name: Option<&str>) {
        let workspace_name = name.unwrap_or("default");

        let workspace_config = self.workspace.to_workspace_config(workspace_name, None);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = Self::workspace_dir();
            let path = dir.join(format!("{workspace_name}.toml"));

            match workspace_config.save(&path) {
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
            // On web, encode to base64 and copy URL to clipboard
            match workspace_config.to_base64() {
                Ok(encoded) => {
                    let base = Self::share_base_url();
                    let full_url = format!("{base}?workspace={encoded}");

                    // Copy to clipboard
                    if let Err(e) = Self::copy_to_clipboard_wasm(&full_url) {
                        log::error!("Failed to copy to clipboard: {e}");
                        self.notifications.notify(Notification::new(
                            format!("Failed to copy URL: {e}"),
                            NotificationLevel::Error,
                        ));
                        return;
                    }

                    log::info!("Workspace URL: {full_url}");
                    self.notifications.notify(Notification::new(
                        format!("Workspace '{workspace_name}' URL copied to clipboard!"),
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
    pub(super) fn load_workspace(&mut self, name: &str) {
        use crate::workspace::WorkspaceConfig;

        #[cfg(not(target_arch = "wasm32"))]
        {
            let dir = Self::workspace_dir();
            let path = dir.join(format!("{name}.toml"));

            match WorkspaceConfig::load(&path) {
                Ok(workspace_config) => {
                    if let Err(e) = workspace_config.validate() {
                        self.notifications.notify(Notification::new(
                            format!("Invalid workspace: {e}"),
                            NotificationLevel::Error,
                        ));
                        return;
                    }

                    let connection = self.workspace.load_workspace_config(&workspace_config);

                    // TODO: Apply connection settings when endpoint tracking is implemented
                    if let Some(conn) = connection {
                        if !conn.endpoint.is_empty() {
                            log::info!("Workspace specifies endpoint: {}", conn.endpoint);
                        }
                    }

                    // Add to recent workspaces
                    self.state.settings.add_recent_workspace(
                        name.to_string(),
                        workspace_config.workspace.description.clone(),
                    );

                    log::info!("Workspace loaded: {name}");
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
                Ok(WorkspaceConfig::default_example())
            } else if name == "demo" {
                // Load built-in demo workspace (synthetic data)
                Ok(WorkspaceConfig::default_demo())
            } else if name == "dashboard" {
                // Load built-in complex dashboard workspace
                WorkspaceConfig::from_toml(crate::workspace::COMPLEX_VIEWPORT_TOML)
            } else {
                // Try to decode from base64 (for shared URLs)
                WorkspaceConfig::from_base64(name)
            };

            match workspace_result {
                Ok(workspace_config) => {
                    if let Err(e) = workspace_config.validate() {
                        self.notifications.notify(Notification::new(
                            format!("Invalid workspace: {e}"),
                            NotificationLevel::Error,
                        ));
                        return;
                    }

                    let connection = self.workspace.load_workspace_config(&workspace_config);

                    // TODO: Apply connection settings when endpoint tracking is implemented
                    if let Some(conn) = connection {
                        if !conn.endpoint.is_empty() {
                            log::info!("Workspace specifies endpoint: {}", conn.endpoint);
                        }
                    }

                    // Add to recent workspaces
                    self.state.settings.add_recent_workspace(
                        workspace_config.workspace.name.clone(),
                        workspace_config.workspace.description.clone(),
                    );
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

    /// Build a share URL for the current workspace (config-only).
    /// Returns the URL string or None on error (with notification).
    pub(super) fn build_share_workspace_url(&mut self) -> Option<String> {
        let workspace_config = self.workspace.to_workspace_config("shared", None);
        match workspace_config.to_base64() {
            Ok(encoded) => Some(Self::build_share_url("workspace", &encoded)),
            Err(e) => {
                log::error!("Failed to encode workspace: {e}");
                self.notifications.notify(Notification::new(
                    format!("Failed to encode workspace: {e}"),
                    NotificationLevel::Error,
                ));
                None
            }
        }
    }

    /// Build a share URL for a single pane (config-only).
    /// Returns the URL string or None on error (with notification).
    pub(super) fn build_share_pane_url(&mut self, pane_index: usize) -> Option<String> {
        let workspace_config = self.workspace.to_workspace_config("shared", None);
        match workspace_config.pane_to_base64(pane_index) {
            Ok(encoded) => Some(Self::build_share_url("pane", &encoded)),
            Err(e) => {
                log::error!("Failed to encode pane: {e}");
                self.notifications.notify(Notification::new(
                    format!("Failed to encode pane: {e}"),
                    NotificationLevel::Error,
                ));
                None
            }
        }
    }

    /// Build a snapshot share URL for the workspace (config + data).
    /// Returns the URL string or None on error (with notification).
    pub(super) fn build_snapshot_workspace_url(&mut self) -> Option<String> {
        let workspace_config = self.workspace.to_workspace_config("snapshot", None);
        let pane_data = self.workspace.extract_all_snapshot_data();
        let captured_at = crate::util::now_unix_secs() as u64;

        match workspace_config.snapshot_to_base64(&pane_data, captured_at) {
            Ok(encoded) => Some(Self::build_share_url("workspace", &encoded)),
            Err(e) => {
                log::error!("Failed to encode snapshot: {e}");
                self.notifications.notify(Notification::new(
                    format!("Failed to encode snapshot: {e}"),
                    NotificationLevel::Error,
                ));
                None
            }
        }
    }

    /// Build a snapshot share URL for a single pane (config + data).
    /// Returns the URL string or None on error (with notification).
    pub(super) fn build_snapshot_pane_url(&mut self, pane_index: usize) -> Option<String> {
        let workspace_config = self.workspace.to_workspace_config("snapshot", None);
        let pane_data = self.workspace.extract_all_snapshot_data();
        let captured_at = crate::util::now_unix_secs() as u64;

        let data = match pane_data.get(pane_index) {
            Some(d) => d,
            None => {
                self.notifications.notify(Notification::new(
                    "No data to snapshot".to_string(),
                    NotificationLevel::Error,
                ));
                return None;
            }
        };

        match workspace_config.snapshot_pane_to_base64(pane_index, data, captured_at) {
            Ok(encoded) => Some(Self::build_share_url("pane", &encoded)),
            Err(e) => {
                log::error!("Failed to encode pane snapshot: {e}");
                self.notifications.notify(Notification::new(
                    format!("Failed to encode pane snapshot: {e}"),
                    NotificationLevel::Error,
                ));
                None
            }
        }
    }

    /// Build a snapshot share URL for selected panes (config + data).
    /// Returns the URL string or None on error (with notification).
    pub(super) fn build_snapshot_selected_url(&mut self, pane_indices: &[usize]) -> Option<String> {
        let workspace_config = self
            .workspace
            .to_workspace_config_for_panes("snapshot", pane_indices);
        let pane_data = self.workspace.extract_snapshot_data_for_panes(pane_indices);
        let captured_at = crate::util::now_unix_secs() as u64;

        match workspace_config.snapshot_to_base64(&pane_data, captured_at) {
            Ok(encoded) => Some(Self::build_share_url("workspace", &encoded)),
            Err(e) => {
                log::error!("Failed to encode selected panes snapshot: {e}");
                self.notifications.notify(Notification::new(
                    format!("Failed to encode snapshot: {e}"),
                    NotificationLevel::Error,
                ));
                None
            }
        }
    }

    /// Build a config-only share URL for selected panes (no embedded data).
    /// Returns the URL string or None on error (with notification).
    pub(super) fn build_share_selected_url(&mut self, pane_indices: &[usize]) -> Option<String> {
        let workspace_config = self
            .workspace
            .to_workspace_config_for_panes("shared", pane_indices);
        match workspace_config.to_base64() {
            Ok(encoded) => Some(Self::build_share_url("workspace", &encoded)),
            Err(e) => {
                log::error!("Failed to encode selected panes: {e}");
                self.notifications.notify(Notification::new(
                    format!("Failed to encode panes: {e}"),
                    NotificationLevel::Error,
                ));
                None
            }
        }
    }

    /// Build a full share URL from a query parameter name and encoded value.
    fn build_share_url(param: &str, encoded: &str) -> String {
        #[cfg(target_arch = "wasm32")]
        {
            let base = Self::share_base_url();
            format!("{base}?{param}={encoded}")
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            format!("{EDITOR_BASE_URL}?{param}={encoded}")
        }
    }

    /// Copy a share URL to clipboard and show a notification.
    /// Uses `ctx.copy_text()` which integrates with egui's clipboard handling.
    pub(super) fn copy_share_url(&mut self, ctx: &egui::Context, url: &str, success_msg: &str) {
        ctx.copy_text(url.to_string());

        let url_len = url.len();
        if url_len > 15_000 {
            self.notifications.notify(Notification::new(
                format!(
                    "{success_msg} ({:.1}KB) - may be too long for some browsers",
                    url_len as f64 / 1024.0
                ),
                NotificationLevel::Warn,
            ));
        } else {
            self.notifications.notify(Notification::new(
                format!("{success_msg}!"),
                NotificationLevel::Success,
            ));
        }
    }

    /// Copy text to clipboard (WASM only)
    #[cfg(target_arch = "wasm32")]
    pub(super) fn copy_to_clipboard_wasm(text: &str) -> Result<(), String> {
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
    pub(super) fn get_url_workspace_param() -> Option<String> {
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
    pub(super) fn get_url_pane_param() -> Option<String> {
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
    pub(super) fn list_workspaces(&mut self) {
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
