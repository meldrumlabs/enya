//! Workspace I/O operations.
//!
//! This module handles saving, loading, listing, and sharing workspaces.
//! On native platforms, workspaces are stored as TOML files in project
//! directories under `.enya/projects/{project}/workspaces/`. On WASM,
//! workspaces are encoded as base64 URL parameters.

use crate::components::{Notification, NotificationLevel};

/// The canonical base URL for the web editor, used in all share links.
const EDITOR_BASE_URL: &str = "https://enya.build/editor";

use super::EnyaApp;

impl EnyaApp {
    /// Generate the next unique "untitled-N" workspace name within a project.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn next_untitled_workspace_name(&self, project: &str) -> String {
        let existing: rustc_hash::FxHashSet<String> = enya_config::list_project_workspaces(project)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        for i in 1u32.. {
            let candidate = format!("untitled-{i}");
            if !existing.contains(&candidate) {
                return candidate;
            }
        }
        "untitled".to_string()
    }

    /// Ensure the built-in tutorial and example workspaces exist on disk.
    ///
    /// Tutorial workspaces are always overwritten so template updates take
    /// effect without users having to manually delete old files.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn ensure_default_workspace() {
        use crate::workspace::{GOLDEN_SIGNALS_TOML, INFRASTRUCTURE_TOML, MULTI_SERVICE_TOML};

        let dir = enya_config::project_workspace_dir("Tutorial");

        // Tutorial workspaces — always overwritten for template updates
        let tutorials: &[(&str, &str)] = &[
            ("quick-start", GOLDEN_SIGNALS_TOML),
            ("infra", INFRASTRUCTURE_TOML),
            ("logs-and-traces", MULTI_SERVICE_TOML),
        ];
        for &(name, toml) in tutorials {
            let path = dir.join(format!("{name}.toml"));
            if let Err(e) = std::fs::write(&path, toml) {
                log::warn!("Failed to write {name} workspace: {e}");
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
    pub(super) fn save_workspace(&mut self, name: Option<&str>, project: Option<&str>) {
        let workspace_name = name.unwrap_or("default");
        let loaded_project = self.workspace.loaded_project().map(|s| s.to_string());
        let project_name = match project.or(loaded_project.as_deref()) {
            Some(p) => p,
            None => {
                log::error!("Cannot save workspace without a project");
                self.notifications.notify(Notification::new(
                    "Cannot save workspace: no project specified".to_string(),
                    NotificationLevel::Error,
                ));
                return;
            }
        };

        let workspace_config = self.workspace.to_workspace_config(workspace_name, None);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = enya_config::resolve_project_workspace_path(project_name, workspace_name);

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
            let _ = project_name;
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

                    log::debug!("Workspace URL: {full_url}");
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
    pub(super) fn load_workspace(&mut self, name: &str, project: Option<&str>) {
        use crate::workspace::WorkspaceConfig;

        // Skip reload if this workspace is already loaded — prevents losing
        // in-memory state (e.g. PR review pane) when the sidebar previews
        // the same workspace via j/k navigation.
        if self.workspace.loaded_name() == Some(name) && self.workspace.loaded_project() == project
        {
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let project_name = match project {
                Some(p) => p,
                None => {
                    log::error!("Cannot load workspace '{name}' without a project");
                    self.notifications.notify(Notification::new(
                        format!("Cannot load workspace '{name}': no project specified"),
                        NotificationLevel::Error,
                    ));
                    return;
                }
            };
            let path = enya_config::resolve_project_workspace_path(project_name, name);

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
                            log::debug!("Workspace specifies endpoint: {}", conn.endpoint);
                        }
                    }

                    // Track the loaded workspace name and project
                    self.workspace.set_loaded_name(Some(name.to_string()));
                    self.workspace
                        .set_loaded_project(Some(project_name.to_string()));

                    // Add to recent workspaces
                    self.state.settings.add_recent_workspace(
                        name.to_string(),
                        workspace_config.workspace.description.clone(),
                        project_name.to_string(),
                    );

                    log::info!("Workspace loaded: {name} (project: {project_name})");
                }
                Err(e) => {
                    log::error!("Failed to load workspace '{name}': {e}");
                    // Remove the stale entry so we don't keep trying to load it
                    self.state
                        .settings
                        .recent_workspaces
                        .retain(|w| !(w.name == name && w.project == project_name));
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = project;
            // On web, first check for built-in workspaces, then try base64
            let workspace_result = if name == "quick-start" {
                WorkspaceConfig::from_toml(crate::workspace::GOLDEN_SIGNALS_TOML)
            } else if name == "infra" {
                WorkspaceConfig::from_toml(crate::workspace::INFRASTRUCTURE_TOML)
            } else if name == "logs-and-traces" {
                WorkspaceConfig::from_toml(crate::workspace::MULTI_SERVICE_TOML)
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
                            log::debug!("Workspace specifies endpoint: {}", conn.endpoint);
                        }
                    }

                    // Track the loaded workspace name
                    self.workspace
                        .set_loaded_name(Some(workspace_config.workspace.name.clone()));

                    // Add to recent workspaces
                    self.state.settings.add_recent_workspace(
                        workspace_config.workspace.name.clone(),
                        workspace_config.workspace.description.clone(),
                        "Tutorial".to_string(),
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

        // Auto-focus project sidebar when workspace has no panes
        if !self.workspace.has_panes() && self.project_sidebar.is_visible() {
            self.project_sidebar.focus();
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

    /// Resolve the snapshot server base URL.
    ///
    /// Production R2 worker at api.enya.build (both native and WASM).
    fn snapshot_server_url() -> &'static str {
        "https://api.enya.build"
    }

    /// Upload a full snapshot (workspace + data + conversation) to the blob server.
    ///
    /// Requires GitHub authentication — the access token is sent as a Bearer
    /// token and validated by the Worker against GitHub's API.
    pub(super) fn upload_snapshot(&mut self, ctx: &egui::Context, title: Option<&str>) {
        // Require sign-in before uploading
        let token = match self.github_auth.credentials() {
            Some(creds) => creds.access_token.clone(),
            None => {
                self.notifications.notify(Notification::new(
                    "Sign in to share snapshots. Go to Settings → Profile to connect GitHub."
                        .to_string(),
                    NotificationLevel::Error,
                ));
                return;
            }
        };

        // Gather data (synchronous)
        let existing_title = self.workspace.snapshot_title();
        let ws_name = title.or(existing_title.as_deref()).unwrap_or("snapshot");
        let ws_config = self.workspace.to_workspace_config(ws_name, None);
        let pane_data = self.workspace.extract_all_snapshot_data();
        let captured_at = crate::util::now_unix_secs() as u64;
        let conversation = self.workspace.agent_panel().extract_snapshot_conversation();
        let sql_pane_data = self.workspace.extract_sql_snapshot_data();

        // Encode (synchronous — postcard + LZ4 is fast)
        let bytes = match enya_config::workspace::snapshot::encode_snapshot(
            &ws_config,
            &pane_data,
            captured_at,
            conversation.as_ref(),
            sql_pane_data.as_ref(),
        ) {
            Ok(b) => b,
            Err(e) => {
                self.notifications.notify(Notification::new(
                    format!("Failed to encode snapshot: {e}"),
                    NotificationLevel::Error,
                ));
                return;
            }
        };

        let pending = std::sync::Arc::clone(&self.pending_snapshot_upload);
        let client = self.snapshot_http_client.clone();
        let ctx = ctx.clone();
        let server_url = Self::snapshot_server_url();
        let share_base = Self::build_share_url("snapshot", "PLACEHOLDER");
        // Strip the placeholder to get just the base + param prefix
        let share_base = share_base.replace("PLACEHOLDER", "");

        // Async upload
        self.async_runtime.spawn(async move {
            let url = format!("{server_url}/snapshot");
            let result = match client
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .body(bytes)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<serde_json::Value>().await {
                        Ok(json) => json["id"]
                            .as_str()
                            .map(|id| format!("{share_base}{id}"))
                            .ok_or_else(|| "No ID in response".to_string()),
                        Err(e) => Err(format!("Invalid response: {e}")),
                    }
                }
                Ok(resp) => {
                    let body = resp.text().await.unwrap_or_default();
                    Err(format!("Server error: {body}"))
                }
                Err(e) => Err(format!("Upload failed: {e}")),
            };
            *pending.lock() = Some(result);
            ctx.request_repaint();
        });
    }

    /// Poll for completed snapshot upload and copy URL to clipboard.
    pub(super) fn poll_snapshot_upload(&mut self, ctx: &egui::Context) {
        if let Some(result) = self.pending_snapshot_upload.lock().take() {
            match result {
                Ok(url) => {
                    ctx.copy_text(url.clone());
                    self.notifications.notify(Notification::new(
                        "Snapshot URL copied to clipboard!".to_string(),
                        NotificationLevel::Success,
                    ));
                }
                Err(e) => {
                    self.notifications.notify(Notification::new(
                        format!("Snapshot upload failed: {e}"),
                        NotificationLevel::Error,
                    ));
                }
            }
        }
    }

    /// Fetch a snapshot blob from the server by ID, decode it, and put the result in pending.
    pub(super) fn fetch_snapshot(&mut self, ctx: &egui::Context, id: &str) {
        let pending = std::sync::Arc::clone(&self.pending_snapshot_load);
        let client = self.snapshot_http_client.clone();
        let ctx = ctx.clone();
        let url = format!("{}/snapshot/{}", Self::snapshot_server_url(), id);

        self.async_runtime.spawn(async move {
            let result = match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                    Ok(bytes) => enya_config::workspace::snapshot::decode_snapshot(&bytes)
                        .map(|s| s.workspace)
                        .map_err(|e| format!("Decode failed: {e}")),
                    Err(e) => Err(format!("Read failed: {e}")),
                },
                Ok(resp) => {
                    let body = resp.text().await.unwrap_or_default();
                    Err(format!("Server error: {body}"))
                }
                Err(e) => Err(format!("Fetch failed: {e}")),
            };
            *pending.lock() = Some(result);
            ctx.request_repaint();
        });
    }

    /// Poll for completed snapshot load and apply the workspace config.
    pub(super) fn poll_snapshot_load(&mut self, _ctx: &egui::Context) {
        if let Some(result) = self.pending_snapshot_load.lock().take() {
            match result {
                Ok(workspace_config) => {
                    let connection = self.workspace.load_workspace_config(&workspace_config);
                    if let Some(conn) = connection {
                        if !conn.endpoint.is_empty() {
                            log::debug!("Snapshot workspace specifies endpoint: {}", conn.endpoint);
                        }
                    }
                    self.notifications.notify(Notification::new(
                        "Snapshot loaded!".to_string(),
                        NotificationLevel::Success,
                    ));
                }
                Err(e) => {
                    self.notifications.notify(Notification::new(
                        format!("Failed to load snapshot: {e}"),
                        NotificationLevel::Error,
                    ));
                }
            }
        }
    }

    /// Get snapshot ID parameter from URL (WASM only)
    /// Returns the snapshot ID if ?snapshot=... is present
    #[cfg(target_arch = "wasm32")]
    pub(super) fn get_url_snapshot_param() -> Option<String> {
        let window = web_sys::window()?;
        let location = window.location();
        let search = location.search().ok()?;

        if search.starts_with('?') {
            for param in search[1..].split('&') {
                if let Some(value) = param.strip_prefix("snapshot=") {
                    if !value.is_empty() {
                        log::debug!("Found snapshot parameter in URL");
                        return Some(value.to_string());
                    }
                }
            }
        }

        None
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
                        log::debug!("Found workspace parameter in URL");
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
                        log::debug!("Found pane parameter in URL");
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
            let projects = enya_config::list_projects();
            if projects.is_empty() {
                self.notifications.notify(Notification::new(
                    "No projects found".to_string(),
                    NotificationLevel::Info,
                ));
            } else {
                let mut parts = Vec::new();
                for project in &projects {
                    let workspaces = enya_config::list_project_workspaces(project);
                    let ws_names: Vec<_> = workspaces.iter().map(|(n, _)| n.as_str()).collect();
                    parts.push(format!("{project}: {}", ws_names.join(", ")));
                }
                let list = parts.join(" | ");
                log::debug!("Available workspaces: {list}");
                self.notifications.notify(Notification::new(
                    format!("Workspaces: {list}"),
                    NotificationLevel::Info,
                ));
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
