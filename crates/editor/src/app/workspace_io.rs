//! Workspace I/O operations.
//!
//! This module handles saving, loading, listing, and sharing workspaces.
//! On native platforms, workspaces are stored as TOML files in the
//! `.enya/workspaces` directory. On WASM, workspaces are encoded as
//! base64 URL parameters.

use crate::components::{Notification, NotificationLevel};

use super::EnyaApp;

impl EnyaApp {
    /// Get the workspace directory path
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn workspace_dir() -> std::path::PathBuf {
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

    /// List available workspace files from the workspace directory
    #[cfg(not(target_arch = "wasm32"))]
    pub fn list_available_workspaces() -> Vec<(String, Option<String>)> {
        use crate::workspace::WorkspaceConfig;

        let dir = Self::workspace_dir();
        let mut workspaces = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "toml") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        // Try to load workspace to get description
                        let description = std::fs::read_to_string(&path)
                            .ok()
                            .and_then(|content| WorkspaceConfig::from_toml(&content).ok())
                            .and_then(|ws| {
                                if ws.workspace.description.is_empty() {
                                    None
                                } else {
                                    Some(ws.workspace.description)
                                }
                            });
                        workspaces.push((name.to_string(), description));
                    }
                }
            }
        }

        // Sort alphabetically by name
        workspaces.sort_by(|a, b| a.0.cmp(&b.0));
        workspaces
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
                    // Build the full URL
                    let full_url = {
                        let base_url = web_sys::window()
                            .and_then(|w| w.location().href().ok())
                            .unwrap_or_else(|| "https://enya.build/editor".to_string());

                        // Remove any existing query string
                        let base_url = base_url.split('?').next().unwrap_or(&base_url);
                        format!("{base_url}?workspace={encoded}")
                    };

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

    /// Share the current workspace as a URL (copies to clipboard)
    pub(super) fn share_workspace(&mut self) {
        let workspace_config = self.workspace.to_workspace_config("shared", None);

        match workspace_config.to_base64() {
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
    pub(super) fn share_pane(&mut self, pane_index: usize) {
        let workspace_config = self.workspace.to_workspace_config("shared", None);

        match workspace_config.pane_to_base64(pane_index) {
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
