//! Update checker service - polls GitHub Releases for new versions.
//!
//! Follows the same async HTTP + frame-polling pattern as `ConnectionManager`.
//! Checks for updates on startup and every 30 minutes thereafter.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Deserialize;

use enya_build_info::CrateVersion;

use crate::AsyncRuntime;
use crate::util::Instant;

/// How often to check for updates (30 minutes).
const CHECK_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Delay before the first check after startup (10 seconds).
const STARTUP_DELAY: Duration = Duration::from_secs(10);

/// GitHub API URL for the latest release.
const RELEASES_URL: &str = "https://api.github.com/repos/meldrumlabs/enya/releases/latest";

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// The new version string (e.g. "0.2.0").
    pub version: String,
    /// URL to the GitHub release page.
    pub release_url: String,
    /// Brief summary from the release body.
    pub release_notes: String,
    /// Download URL for the current platform's binary (if found).
    pub download_url: Option<String>,
}

/// Current status of the update checker.
#[derive(Debug, Clone, Default)]
pub enum UpdateStatus {
    /// Haven't checked yet.
    #[default]
    Unknown,
    /// A check is in progress.
    Checking,
    /// Current version is the latest.
    UpToDate,
    /// A newer version is available.
    Available(UpdateInfo),
    /// The check failed.
    Failed(String),
}

/// Result type for the pending update check.
type PendingCheckResult = Arc<Mutex<Option<Result<Option<UpdateInfo>, String>>>>;

/// Manages periodic update checks against GitHub Releases.
pub struct UpdateChecker {
    status: UpdateStatus,
    pending_result: PendingCheckResult,
    last_check: Option<Instant>,
    started_at: Instant,
    http_client: reqwest::Client,
    async_runtime: AsyncRuntime,
    dismissed_version: Option<String>,
    /// Whether update checking is enabled.
    enabled: bool,
    /// Whether an update download is in progress.
    downloading: bool,
    /// Result of an update download attempt.
    download_result: Arc<Mutex<Option<Result<(), String>>>>,
}

/// GitHub Release API response (subset of fields we need).
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

/// GitHub Release asset.
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

impl UpdateChecker {
    /// Create a new update checker.
    pub fn new(
        async_runtime: AsyncRuntime,
        dismissed_version: Option<String>,
        enabled: bool,
    ) -> Self {
        Self {
            status: UpdateStatus::Unknown,
            pending_result: Arc::new(Mutex::new(None)),
            last_check: None,
            started_at: Instant::now(),
            http_client: reqwest::Client::new(),
            async_runtime,
            dismissed_version,
            enabled,
            downloading: false,
            download_result: Arc::new(Mutex::new(None)),
        }
    }

    /// Set whether update checking is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Poll for update status changes. Call this each frame.
    ///
    /// Automatically triggers checks on startup (after delay) and periodically.
    pub fn poll(&mut self, ctx: &egui::Context) {
        if !self.enabled {
            return;
        }

        // Check for completed download
        if let Some(result) = self.download_result.lock().take() {
            self.downloading = false;
            match result {
                Ok(()) => {
                    self.restart_app(ctx);
                }
                Err(e) => {
                    log::error!("Update download failed: {e}");
                }
            }
        }

        // Check for completed update check
        if let Some(result) = self.pending_result.lock().take() {
            match result {
                Ok(Some(info)) => {
                    self.status = UpdateStatus::Available(info);
                }
                Ok(None) => {
                    self.status = UpdateStatus::UpToDate;
                }
                Err(e) => {
                    log::warn!("Update check failed: {e}");
                    self.status = UpdateStatus::Failed(e);
                }
            }
        }

        // Determine if we should trigger a new check
        let should_check = match self.last_check {
            None => {
                // First check: wait for startup delay
                self.started_at.elapsed() >= STARTUP_DELAY
            }
            Some(last) => last.elapsed() >= CHECK_INTERVAL,
        };

        if should_check && !matches!(self.status, UpdateStatus::Checking) {
            self.check(ctx);
        }
    }

    /// Returns the update info if a new version is available and not dismissed.
    pub fn available_update(&self) -> Option<&UpdateInfo> {
        if let UpdateStatus::Available(ref info) = self.status {
            if self.dismissed_version.as_deref() != Some(&info.version) {
                return Some(info);
            }
        }
        None
    }

    /// Dismiss the update notification for a specific version.
    pub fn dismiss(&mut self, version: String) {
        self.dismissed_version = Some(version);
    }

    /// Whether a download is currently in progress.
    pub fn is_downloading(&self) -> bool {
        self.downloading
    }

    /// Trigger the download-and-replace flow for the given update.
    pub fn download_and_update(&mut self, download_url: &str, ctx: &egui::Context) {
        if self.downloading {
            return;
        }
        self.downloading = true;

        let url = download_url.to_string();
        let client = self.http_client.clone();
        let result = Arc::clone(&self.download_result);
        let ctx = ctx.clone();

        self.async_runtime.spawn(async move {
            let download_result = Self::perform_download(&client, &url).await;
            *result.lock() = Some(download_result);
            ctx.request_repaint();
        });
    }

    /// Perform the update download and installation.
    ///
    /// On macOS: downloads the DMG, mounts it, copies the signed `.app` bundle
    /// using `ditto` (preserving code signatures), and atomically swaps it into place.
    ///
    /// On other platforms: downloads the binary and replaces the current executable.
    #[cfg(target_os = "macos")]
    async fn perform_download(client: &reqwest::Client, url: &str) -> Result<(), String> {
        let app_bundle = find_app_bundle()
            .ok_or("Not running from a .app bundle; cannot perform in-place update")?;

        let app_dir = app_bundle
            .parent()
            .ok_or("Cannot determine app parent directory")?;

        let app_name = app_bundle
            .file_name()
            .ok_or("Cannot determine app bundle name")?
            .to_string_lossy();

        // Stage in the same directory to guarantee same-filesystem atomic rename
        let staged_path = app_dir.join(".Enya-staged.app");
        let old_path = app_dir.join(format!("{app_name}.old"));
        let dmg_path = std::env::temp_dir().join("Enya-update.dmg");
        let mount_point = std::env::temp_dir().join("enya-update-mount");

        // Download the DMG
        log::info!("Downloading update from {url}");
        let response = client
            .get(url)
            .header("User-Agent", "enya-editor")
            .send()
            .await
            .map_err(|e| format!("Download failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Download HTTP {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read download: {e}"))?;

        {
            use std::io::Write;
            let mut file = std::fs::File::create(&dmg_path)
                .map_err(|e| format!("Failed to create DMG temp file: {e}"))?;
            file.write_all(&bytes)
                .map_err(|e| format!("Failed to write DMG: {e}"))?;
        }

        // Mount the DMG (nobrowse = no Finder sidebar, readonly = safe)
        log::info!("Mounting DMG");
        // Detach any stale mount from a previous failed attempt
        let _ = tokio::process::Command::new("hdiutil")
            .arg("detach")
            .arg(&mount_point)
            .output()
            .await;
        let _ = std::fs::create_dir_all(&mount_point);

        let mount_output = tokio::process::Command::new("hdiutil")
            .arg("attach")
            .arg("-nobrowse")
            .arg("-readonly")
            .arg("-mountpoint")
            .arg(&mount_point)
            .arg(&dmg_path)
            .output()
            .await
            .map_err(|e| format!("hdiutil attach failed: {e}"))?;

        if !mount_output.status.success() {
            let _ = std::fs::remove_file(&dmg_path);
            let stderr = String::from_utf8_lossy(&mount_output.stderr);
            return Err(format!("hdiutil attach failed: {stderr}"));
        }

        // Copy Enya.app from DMG using ditto (preserves code signatures + xattrs)
        log::info!("Copying app bundle from DMG");
        let source_app = mount_point.join("Enya.app");
        let _ = std::fs::remove_dir_all(&staged_path);

        let ditto_output = tokio::process::Command::new("ditto")
            .arg(&source_app)
            .arg(&staged_path)
            .output()
            .await
            .map_err(|e| format!("ditto failed: {e}"))?;

        // Unmount and clean up DMG regardless of ditto result
        let _ = tokio::process::Command::new("hdiutil")
            .arg("detach")
            .arg(&mount_point)
            .output()
            .await;
        let _ = std::fs::remove_file(&dmg_path);
        let _ = std::fs::remove_dir(&mount_point);

        if !ditto_output.status.success() {
            let _ = std::fs::remove_dir_all(&staged_path);
            let stderr = String::from_utf8_lossy(&ditto_output.stderr);
            return Err(format!("ditto failed: {stderr}"));
        }

        // Atomic bundle swap:
        // Rename current Enya.app → Enya.app.old (macOS allows renaming dirs
        // containing running binaries; the process keeps its open inode)
        log::info!("Swapping app bundles");
        let _ = std::fs::remove_dir_all(&old_path);

        std::fs::rename(&app_bundle, &old_path)
            .map_err(|e| format!("Failed to move current .app to .old: {e}"))?;

        if let Err(e) = std::fs::rename(&staged_path, &app_bundle) {
            // Rollback: restore the old bundle
            let _ = std::fs::rename(&old_path, &app_bundle);
            let _ = std::fs::remove_dir_all(&staged_path);
            return Err(format!("Failed to install staged .app: {e}"));
        }

        log::info!("Update installed successfully");
        Ok(())
    }

    /// Perform the actual binary download and replacement (non-macOS).
    #[cfg(not(target_os = "macos"))]
    async fn perform_download(client: &reqwest::Client, url: &str) -> Result<(), String> {
        use std::io::Write;

        let current_exe =
            std::env::current_exe().map_err(|e| format!("Failed to get current exe: {e}"))?;

        // Download the new binary
        let response = client
            .get(url)
            .header("Accept", "application/octet-stream")
            .header("User-Agent", "enya-editor")
            .send()
            .await
            .map_err(|e| format!("Download failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("Download HTTP {}", response.status()));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read download: {e}"))?;

        // Write to a temp file next to the current exe
        let temp_path = current_exe.with_extension("update");
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| format!("Failed to create temp file: {e}"))?;
        file.write_all(&bytes)
            .map_err(|e| format!("Failed to write temp file: {e}"))?;
        drop(file);

        // Make the temp file executable (Unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Failed to set permissions: {e}"))?;
        }

        // Replace the current exe: rename current to .old, rename temp to current
        let backup_path = current_exe.with_extension("old");
        let _ = std::fs::remove_file(&backup_path); // Remove any previous backup
        std::fs::rename(&current_exe, &backup_path)
            .map_err(|e| format!("Failed to backup current exe: {e}"))?;
        if let Err(e) = std::fs::rename(&temp_path, &current_exe) {
            // Try to restore backup
            let _ = std::fs::rename(&backup_path, &current_exe);
            return Err(format!("Failed to replace exe: {e}"));
        }

        Ok(())
    }

    /// Restart the application by spawning a new process and closing the current one.
    fn restart_app(&self, ctx: &egui::Context) {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Remove `Enya.app.old` left behind by a previous successful update.
    ///
    /// Called once on startup. Best-effort: logs on failure but does not panic.
    #[cfg(target_os = "macos")]
    pub fn cleanup_old_bundle() {
        if let Some(bundle) = find_app_bundle() {
            if let Some(parent) = bundle.parent() {
                let bundle_name = bundle
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Enya.app".to_string());
                let old_path = parent.join(format!("{bundle_name}.old"));
                if old_path.exists() {
                    log::info!("Removing leftover bundle from previous update: {old_path:?}");
                    if let Err(e) = std::fs::remove_dir_all(&old_path) {
                        log::warn!("Failed to remove old bundle {old_path:?}: {e}");
                    }
                }
                // Also clean up any failed staging
                let staged_path = parent.join(".Enya-staged.app");
                if staged_path.exists() {
                    log::info!("Removing leftover staged bundle: {staged_path:?}");
                    let _ = std::fs::remove_dir_all(&staged_path);
                }
            }
        }
    }

    /// Fire off an async update check.
    fn check(&mut self, ctx: &egui::Context) {
        self.status = UpdateStatus::Checking;
        self.last_check = Some(Instant::now());

        let pending = Arc::clone(&self.pending_result);
        let client = self.http_client.clone();
        let ctx = ctx.clone();

        self.async_runtime.spawn(async move {
            let result = Self::fetch_latest_release(&client).await;
            *pending.lock() = Some(result);
            ctx.request_repaint();
        });
    }

    /// Fetch the latest release from GitHub and compare versions.
    async fn fetch_latest_release(client: &reqwest::Client) -> Result<Option<UpdateInfo>, String> {
        let response = client
            .get(RELEASES_URL)
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "enya-editor")
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }

        let release: GitHubRelease = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {e}"))?;

        // Strip leading 'v' from tag name if present
        let remote_version_str = release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name);

        // Parse remote version and compare against the compiled-in build version
        let remote_version = parse_version(remote_version_str);
        let local_version = CrateVersion::LOCAL;

        if let Some(remote) = remote_version {
            if remote > local_version {
                let release_notes = release
                    .body
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(500)
                    .collect::<String>();

                // Find the download URL for the current platform
                let download_url = find_platform_asset(&release.assets);

                return Ok(Some(UpdateInfo {
                    version: remote_version_str.to_string(),
                    release_url: release.html_url,
                    release_notes,
                    download_url,
                }));
            }
        }

        Ok(None)
    }
}

/// Walk up from `current_exe()` to find the enclosing `.app` bundle directory.
///
/// Returns `None` when not running inside a macOS app bundle (e.g. `cargo run`).
#[cfg(target_os = "macos")]
fn find_app_bundle() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut path = exe.as_path();
    loop {
        path = path.parent()?;
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".app"))
        {
            return Some(path.to_path_buf());
        }
    }
}

/// Parse a version string at runtime into a `CrateVersion` for comparison.
///
/// Handles formats like "0.2.0", "1.0.0-alpha.1", "1.0.0-rc.2".
/// Pre-release metadata is stripped since `CrateVersion::Ord` only compares major.minor.patch.
fn parse_version(s: &str) -> Option<CrateVersion> {
    // Strip any pre-release suffix for comparison (e.g., "-alpha.1+dev")
    let base = s.split('-').next().unwrap_or(s);
    let mut parts = base.split('.');
    let major: u8 = parts.next()?.parse().ok()?;
    let minor: u8 = parts.next()?.parse().ok()?;
    let patch: u8 = parts.next()?.parse().ok()?;
    Some(CrateVersion::new(major, minor, patch))
}

/// Find the download asset URL matching the current platform.
#[cfg(target_os = "macos")]
fn find_platform_asset(assets: &[GitHubAsset]) -> Option<String> {
    // The release uploads a single universal DMG named "Enya.dmg".
    assets
        .iter()
        .find(|a| a.name.ends_with(".dmg"))
        .map(|a| a.browser_download_url.clone())
}

/// Find the download asset URL matching the current platform.
#[cfg(not(target_os = "macos"))]
fn find_platform_asset(assets: &[GitHubAsset]) -> Option<String> {
    let target = if cfg!(target_os = "linux") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-unknown-linux"
        } else {
            "x86_64-unknown-linux"
        }
    } else if cfg!(target_os = "windows") {
        "x86_64-pc-windows"
    } else {
        return None;
    };

    assets
        .iter()
        .find(|a| a.name.contains(target))
        .map(|a| a.browser_download_url.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("0.2.0"), Some(CrateVersion::new(0, 2, 0)));
        assert_eq!(parse_version("1.2.3"), Some(CrateVersion::new(1, 2, 3)));
        assert_eq!(
            parse_version("0.1.0-alpha.1+dev"),
            Some(CrateVersion::new(0, 1, 0))
        );
        assert_eq!(parse_version("invalid"), None);
    }

    #[test]
    fn test_version_comparison() {
        let v010 = CrateVersion::new(0, 1, 0);
        let v020 = CrateVersion::new(0, 2, 0);
        let v100 = CrateVersion::new(1, 0, 0);

        assert!(v020 > v010);
        assert!(v100 > v020);
        assert!(v010 <= v010); // equal, not newer
        assert!(v010 <= v020); // older, not newer
    }

    #[test]
    fn test_local_version_is_valid() {
        // CrateVersion::LOCAL is parsed at compile time from CARGO_PKG_VERSION
        let local = CrateVersion::LOCAL;
        // Should be a valid version (not panicking is the test)
        assert!(local.major < 255 || local.minor < 255 || local.patch < 255);
    }

    #[test]
    fn test_find_platform_asset() {
        let assets = vec![
            GitHubAsset {
                name: "Enya.dmg".to_string(),
                browser_download_url: "https://example.com/Enya.dmg".to_string(),
            },
            GitHubAsset {
                name: "enya-x86_64-unknown-linux-gnu.tar.gz".to_string(),
                browser_download_url: "https://example.com/linux".to_string(),
            },
            GitHubAsset {
                name: "checksums-macos.txt".to_string(),
                browser_download_url: "https://example.com/checksums".to_string(),
            },
        ];

        let result = find_platform_asset(&assets);
        #[cfg(target_os = "macos")]
        assert_eq!(result, Some("https://example.com/Enya.dmg".to_string()));
        #[cfg(target_os = "linux")]
        assert_eq!(result, Some("https://example.com/linux".to_string()));
        #[cfg(target_os = "windows")]
        assert!(result.is_none()); // no windows asset in test data
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_find_app_bundle_not_in_bundle() {
        // Running from `cargo test`, not inside a .app bundle
        let result = find_app_bundle();
        assert!(result.is_none());
    }
}
