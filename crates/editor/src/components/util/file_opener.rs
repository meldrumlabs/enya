//! File opener popup for opening files in external applications.
//!
//! Provides a context menu popup that allows users to open files in external
//! editors, terminals, or file managers. Supports macOS and Linux with
//! automatic detection of installed applications.
//!
//! # Supported Applications
//!
//! - **Zed** - Modern code editor
//! - **VS Code** - Visual Studio Code
//! - **Cursor** - AI-powered code editor (VS Code fork)
//! - **Ghostty** - GPU-accelerated terminal
//! - **iTerm2** - Feature-rich terminal emulator
//! - **Finder/Files** - System file manager
//!
//! # Usage
//!
//! ```ignore
//! let mut popup = FileOpenerPopup::new();
//!
//! // Show popup at a position
//! if right_clicked {
//!     popup.open(mouse_pos, file_path);
//! }
//!
//! // Render and handle result
//! match popup.show(ui, theme) {
//!     FileOpenerResult::Selected(action) => {
//!         action.execute(&file_path).ok();
//!     }
//!     _ => {}
//! }
//! ```

use std::path::Path;

use egui::{Image, Key, RichText, Vec2};

use crate::ui::icons::{
    APP_CURSOR, APP_FINDER, APP_GHOSTTY, APP_ITERM2, APP_VSCODE, APP_ZED, Icon,
};
use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::finder_utils::OverlayStyle;

// ============================================================================
// External Application Types
// ============================================================================

/// An external application that can open files or directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalApp {
    /// Zed code editor
    Zed,
    /// Visual Studio Code
    VSCode,
    /// Cursor AI code editor (VS Code fork)
    Cursor,
    /// Ghostty terminal emulator
    Ghostty,
    /// iTerm2 terminal emulator
    ITerm2,
    /// macOS Finder / Linux file manager
    FileManager,
}

impl ExternalApp {
    /// Get the display name for this application.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Zed => "Zed",
            Self::VSCode => "VS Code",
            Self::Cursor => "Cursor",
            Self::Ghostty => "Ghostty",
            Self::ITerm2 => "iTerm2",
            Self::FileManager => {
                #[cfg(target_os = "macos")]
                {
                    "Finder"
                }
                #[cfg(target_os = "linux")]
                {
                    "Files"
                }
                #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                {
                    "File Manager"
                }
            }
        }
    }

    /// Get the icon for this application.
    pub fn icon(&self) -> &'static Icon {
        match self {
            Self::Zed => &APP_ZED,
            Self::VSCode => &APP_VSCODE,
            Self::Cursor => &APP_CURSOR,
            Self::Ghostty => &APP_GHOSTTY,
            Self::ITerm2 => &APP_ITERM2,
            Self::FileManager => &APP_FINDER,
        }
    }

    /// Get the action description for the menu.
    pub fn action_label(&self) -> String {
        match self {
            Self::Zed | Self::VSCode | Self::Cursor => format!("Open in {}", self.name()),
            Self::Ghostty | Self::ITerm2 => format!("Open in {}", self.name()),
            Self::FileManager => format!("Open in {}", self.name()),
        }
    }

    /// Check if this application is installed on the system.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_available(&self) -> bool {
        match self {
            Self::Zed => Self::check_app_or_command("Zed", "zed"),
            Self::VSCode => Self::check_app_or_command("Visual Studio Code", "code"),
            Self::Cursor => Self::check_app_or_command("Cursor", "cursor"),
            Self::Ghostty => Self::check_app_or_command("Ghostty", "ghostty"),
            Self::ITerm2 => Self::check_app_or_command("iTerm", "iterm2"),
            Self::FileManager => true, // Always available
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn is_available(&self) -> bool {
        false // External apps not available in WASM
    }

    /// Check if an app bundle exists (macOS) or command is in PATH.
    #[cfg(not(target_arch = "wasm32"))]
    fn check_app_or_command(app_name: &str, command: &str) -> bool {
        #[cfg(target_os = "macos")]
        {
            // Check for .app bundle first
            let app_path = format!("/Applications/{app_name}.app");
            if std::path::Path::new(&app_path).exists() {
                return true;
            }
        }

        // Fall back to checking if command exists in PATH
        std::process::Command::new("which")
            .arg(command)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Execute the open action for this application.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn execute(&self, path: &Path) -> Result<(), String> {
        log::debug!("ExternalApp::execute - app: {self:?}, path: {path:?}");

        // Check if file/directory exists before trying to open
        if !path.exists() {
            return Err(format!("File does not exist: {}", path.display()));
        }

        match self {
            Self::Zed => Self::open_app("Zed", "zed", path),
            Self::VSCode => Self::open_app("Visual Studio Code", "code", path),
            Self::Cursor => Self::open_app("Cursor", "cursor", path),
            Self::Ghostty => Self::open_terminal("Ghostty", "ghostty", path),
            Self::ITerm2 => Self::open_terminal("iTerm", "iterm2", path),
            Self::FileManager => Self::reveal_in_file_manager(path),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn execute(&self, _path: &Path) -> Result<(), String> {
        Err("External apps not available in web mode".into())
    }

    /// Open an editor app - uses `open -a` on macOS, CLI command on Linux
    #[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
    fn open_app(app_name: &str, _cli_cmd: &str, path: &Path) -> Result<(), String> {
        log::debug!("Opening {app_name} with path: {path:?}");
        std::process::Command::new("open")
            .args(["-a", app_name])
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open {app_name}: {e}"))?;
        Ok(())
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
    fn open_app(_app_name: &str, cli_cmd: &str, path: &Path) -> Result<(), String> {
        log::debug!("Opening {cli_cmd} with path: {path:?}");
        std::process::Command::new(cli_cmd)
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open {cli_cmd}: {e}"))?;
        Ok(())
    }

    #[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
    fn open_terminal(app_name: &str, _cli_cmd: &str, path: &Path) -> Result<(), String> {
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };

        // iTerm2 uses a different approach - AppleScript for setting working directory
        if app_name == "iTerm" {
            let script = format!(
                r#"tell application "iTerm"
                    create window with default profile
                    tell current session of current window
                        write text "cd '{}'"
                    end tell
                end tell"#,
                dir.display()
            );
            std::process::Command::new("osascript")
                .args(["-e", &script])
                .spawn()
                .map_err(|e| format!("Failed to open iTerm2: {e}"))?;
        } else {
            // Ghostty and other terminals
            std::process::Command::new("open")
                .args(["-a", app_name, "--args", "--working-directory"])
                .arg(dir)
                .spawn()
                .map_err(|e| format!("Failed to open {app_name}: {e}"))?;
        }
        Ok(())
    }

    #[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
    fn open_terminal(_app_name: &str, cli_cmd: &str, path: &Path) -> Result<(), String> {
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };

        std::process::Command::new(cli_cmd)
            .arg("--working-directory")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Failed to open {cli_cmd}: {e}"))?;
        Ok(())
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "macos"),
        not(target_os = "linux")
    ))]
    fn open_terminal(_app_name: &str, _cli_cmd: &str, _path: &Path) -> Result<(), String> {
        Err("Terminal opening not supported on this platform".into())
    }

    #[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
    fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
        // Use -R to reveal (select) the file in Finder
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to reveal in Finder: {e}"))?;
        Ok(())
    }

    #[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
    fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
        // Try nautilus first (GNOME), then xdg-open as fallback
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };

        if std::process::Command::new("nautilus")
            .arg("--select")
            .arg(path)
            .spawn()
            .is_ok()
        {
            return Ok(());
        }

        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
        Ok(())
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "macos"),
        not(target_os = "linux")
    ))]
    fn reveal_in_file_manager(_path: &Path) -> Result<(), String> {
        Err("File manager not supported on this platform".into())
    }
}

// ============================================================================
// File Opener Actions (includes non-app actions like Copy)
// ============================================================================

/// Actions available in the file opener popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOpenerAction {
    /// Open in an external application
    OpenIn(ExternalApp),
    /// Copy the full path to clipboard
    CopyPath,
    /// Copy the relative path to clipboard (if base path provided)
    CopyRelativePath,
}

impl FileOpenerAction {
    /// Get the display label for this action.
    pub fn label(&self) -> String {
        match self {
            Self::OpenIn(app) => app.action_label(),
            Self::CopyPath => "Copy path".to_string(),
            Self::CopyRelativePath => "Copy relative path".to_string(),
        }
    }
}

// ============================================================================
// File Opener Popup Result
// ============================================================================

/// Result of showing the file opener popup.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FileOpenerResult {
    /// No action taken
    #[default]
    None,
    /// An action was selected
    Selected(FileOpenerAction),
    /// Popup was closed without selection
    Closed,
}

// ============================================================================
// File Opener Popup
// ============================================================================

/// A popup menu for opening files in external applications.
pub struct FileOpenerPopup {
    /// Whether the popup is currently open
    is_open: bool,
    /// Position to show the popup
    position: egui::Pos2,
    /// The file path to operate on
    file_path: Option<std::path::PathBuf>,
    /// Base path for relative path calculation
    base_path: Option<std::path::PathBuf>,
    /// Currently selected item index
    selected_index: usize,
    /// Cached list of available apps (computed once when opened)
    available_apps: Vec<ExternalApp>,
    /// Current theme
    theme: AppTheme,
}

impl Default for FileOpenerPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl FileOpenerPopup {
    /// Create a new file opener popup.
    pub fn new() -> Self {
        Self {
            is_open: false,
            position: egui::Pos2::ZERO,
            file_path: None,
            base_path: None,
            selected_index: 0,
            available_apps: Vec::new(),
            theme: AppTheme::Dark,
        }
    }

    /// Set the theme.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Open the popup at the given position for a file path.
    pub fn open(&mut self, position: egui::Pos2, file_path: std::path::PathBuf) {
        self.open_with_base(position, file_path, None);
    }

    /// Open the popup with a base path for relative path calculation.
    pub fn open_with_base(
        &mut self,
        position: egui::Pos2,
        file_path: std::path::PathBuf,
        base_path: Option<std::path::PathBuf>,
    ) {
        self.is_open = true;
        self.position = position;
        self.file_path = Some(file_path);
        self.base_path = base_path;
        self.selected_index = 0;

        // Cache available apps
        self.available_apps = [
            ExternalApp::Zed,
            ExternalApp::VSCode,
            ExternalApp::Ghostty,
            ExternalApp::FileManager,
        ]
        .into_iter()
        .filter(|app| app.is_available())
        .collect();
    }

    /// Close the popup.
    /// Note: We don't clear file_path/base_path here so they remain accessible
    /// after the popup closes (for the action handler to use).
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Check if the popup is open.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Get the total number of items in the menu.
    fn item_count(&self) -> usize {
        // Apps + Copy actions (CopyRelativePath only if base_path is set)
        self.available_apps.len() + if self.base_path.is_some() { 2 } else { 1 }
    }

    /// Build the list of all actions.
    fn all_actions(&self) -> Vec<FileOpenerAction> {
        let mut actions: Vec<FileOpenerAction> = self
            .available_apps
            .iter()
            .map(|app| FileOpenerAction::OpenIn(*app))
            .collect();

        actions.push(FileOpenerAction::CopyPath);
        if self.base_path.is_some() {
            actions.push(FileOpenerAction::CopyRelativePath);
        }

        actions
    }

    /// Show the popup and return the result.
    pub fn show(&mut self, ctx: &egui::Context, theme: AppTheme) -> FileOpenerResult {
        if !self.is_open {
            return FileOpenerResult::None;
        }

        self.theme = theme;
        let mut result = FileOpenerResult::None;

        // Handle keyboard input
        ctx.input(|i| {
            if i.key_pressed(Key::Escape) {
                result = FileOpenerResult::Closed;
            } else if i.key_pressed(Key::Enter) {
                let actions = self.all_actions();
                if self.selected_index < actions.len() {
                    result = FileOpenerResult::Selected(actions[self.selected_index].clone());
                }
            } else if i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::J) {
                let count = self.item_count();
                if count > 0 {
                    self.selected_index = (self.selected_index + 1) % count;
                }
            } else if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::K) {
                let count = self.item_count();
                if count > 0 {
                    self.selected_index = (self.selected_index + count - 1) % count;
                }
            }
        });

        // Premium popup styling with frosted glass effect
        let style = OverlayStyle::frosted_glass(theme);
        let popup_width = 270.0;
        let corner_radius = 10.0;

        egui::Area::new(egui::Id::new("file_opener_popup"))
            .order(egui::Order::Foreground)
            .fixed_pos(self.position)
            .show(ctx, |ui| {
                // Premium frame with shadow and refined margins
                let frame = style
                    .frame()
                    .inner_margin(egui::Margin::symmetric(6, 8))
                    .corner_radius(corner_radius)
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(60),
                    });

                let frame_response = frame.show(ui, |ui| {
                    ui.set_width(popup_width);

                    // Header with subtle styling
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("Open with")
                                .size(typography::XS)
                                .color(theme.text_primary().gamma_multiply(0.5)),
                        );
                    });
                    ui.add_space(6.0);

                    let actions = self.all_actions();
                    let apps_count = self.available_apps.len();

                    for (idx, action) in actions.iter().enumerate() {
                        // Draw separator before copy actions
                        if idx == apps_count && apps_count > 0 {
                            ui.add_space(6.0);
                            let sep_rect = ui.available_rect_before_wrap();
                            let sep_y = sep_rect.top();
                            ui.painter().line_segment(
                                [
                                    egui::pos2(sep_rect.left() + 10.0, sep_y),
                                    egui::pos2(sep_rect.right() - 10.0, sep_y),
                                ],
                                egui::Stroke::new(1.0, theme.border_subtle().gamma_multiply(0.6)),
                            );
                            ui.add_space(6.0);
                        }

                        let is_selected = idx == self.selected_index;
                        let item_result =
                            self.render_menu_item(ui, action, is_selected, theme, popup_width);

                        if item_result.clicked() {
                            result = FileOpenerResult::Selected(action.clone());
                        }

                        if item_result.hovered() {
                            self.selected_index = idx;
                        }
                    }

                    ui.add_space(2.0);
                });

                // Draw premium top edge highlight
                if let Some(inner_highlight) = style.inner_highlight() {
                    let rect = frame_response.response.rect;
                    let highlight_rect = egui::Rect::from_min_size(
                        rect.left_top() + egui::vec2(1.0, 1.0),
                        egui::vec2(rect.width() - 2.0, 1.5),
                    );
                    ui.painter()
                        .rect_filled(highlight_rect, corner_radius - 1.0, inner_highlight);
                }
            });

        // Close popup if action was selected or cancelled
        if matches!(
            result,
            FileOpenerResult::Selected(_) | FileOpenerResult::Closed
        ) {
            self.close();
        }

        result
    }

    /// Render a single menu item with premium styling.
    fn render_menu_item(
        &self,
        ui: &mut egui::Ui,
        action: &FileOpenerAction,
        is_selected: bool,
        theme: AppTheme,
        popup_width: f32,
    ) -> egui::Response {
        let row_height = 32.0;
        let icon_size = 18.0;
        let left_padding = 10.0;
        let icon_label_gap = 10.0;
        let corner_radius = 6.0;

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(popup_width - 12.0, row_height),
            egui::Sense::click(),
        );

        let is_hovered = response.hovered();
        let accent_col = theme.accent_primary();
        let text_col = theme.text_primary();

        // Premium row styling
        if is_selected {
            // Selected: accent-tinted background with subtle glow
            let bg_color = accent_col.gamma_multiply(0.15);
            ui.painter().rect_filled(rect, corner_radius, bg_color);

            // Subtle glow border
            ui.painter().rect_stroke(
                rect.expand(0.5),
                corner_radius,
                egui::Stroke::new(1.0, accent_col.gamma_multiply(0.25)),
                egui::StrokeKind::Outside,
            );

            // Left accent bar
            let indicator_rect = egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height));
            ui.painter().rect_filled(indicator_rect, 2.0, accent_col);
        } else if is_hovered {
            // Hovered: subtle highlight
            let bg_color = text_col.gamma_multiply(0.06);
            ui.painter().rect_filled(rect, corner_radius, bg_color);

            // Very subtle border on hover
            ui.painter().rect_stroke(
                rect,
                corner_radius,
                egui::Stroke::new(0.5, text_col.gamma_multiply(0.08)),
                egui::StrokeKind::Inside,
            );
        }

        // Icon area - centered vertically with padding for PNG transparency
        let icon_display_size = icon_size;
        let icon_center_x = rect.left() + left_padding + icon_display_size / 2.0;
        let icon_center_y = rect.center().y;

        match action {
            FileOpenerAction::OpenIn(app) => {
                // Render app icon - use slightly larger size and center it
                let icon = app.icon();
                // Create rect centered at icon position
                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(icon_center_x, icon_center_y),
                    Vec2::splat(icon_display_size),
                );
                let image = Image::new(icon.as_image_source())
                    .fit_to_exact_size(Vec2::splat(icon_display_size));
                image.paint_at(ui, icon_rect);
            }
            FileOpenerAction::CopyPath | FileOpenerAction::CopyRelativePath => {
                // Render copy icon using nerd font - centered
                let icon_color = if is_selected {
                    accent_col
                } else if is_hovered {
                    text_col.gamma_multiply(0.7)
                } else {
                    text_col.gamma_multiply(0.5)
                };
                ui.painter().text(
                    egui::pos2(icon_center_x, icon_center_y),
                    egui::Align2::CENTER_CENTER,
                    semantic_icons::action::COPY,
                    typography::proportional(icon_display_size),
                    icon_color,
                );
            }
        }

        // Render label with proper positioning
        let label_x = rect.left() + left_padding + icon_display_size + icon_label_gap;
        let label_color = if is_selected {
            text_col
        } else if is_hovered {
            text_col.gamma_multiply(0.85)
        } else {
            text_col.gamma_multiply(0.7)
        };
        ui.painter().text(
            egui::pos2(label_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            action.label(),
            typography::proportional(typography::SM),
            label_color,
        );

        response
    }

    /// Get the file path if set.
    pub fn file_path(&self) -> Option<&std::path::Path> {
        self.file_path.as_deref()
    }

    /// Compute relative path from base.
    pub fn relative_path(&self) -> Option<std::path::PathBuf> {
        match (&self.file_path, &self.base_path) {
            (Some(file), Some(base)) => file.strip_prefix(base).ok().map(|p| p.to_path_buf()),
            _ => None,
        }
    }
}

// ============================================================================
// Inline File Opener Widget
// ============================================================================

/// An inline widget that shows app icons directly visible for opening files.
/// Unlike `FileOpenerPopup`, this renders clickable icons inline without a popup.
pub struct FileOpenerInline {
    /// Cached list of available apps (native only)
    #[cfg(not(target_arch = "wasm32"))]
    available_apps: Vec<ExternalApp>,
    /// Whether apps have been detected yet (native only)
    #[cfg(not(target_arch = "wasm32"))]
    apps_detected: bool,
}

impl Default for FileOpenerInline {
    fn default() -> Self {
        Self::new()
    }
}

impl FileOpenerInline {
    /// Create a new inline file opener.
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            available_apps: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            apps_detected: false,
        }
    }

    /// Ensure available apps are detected (only once).
    #[cfg(not(target_arch = "wasm32"))]
    fn ensure_apps_detected(&mut self) {
        if !self.apps_detected {
            self.available_apps = [
                ExternalApp::Zed,
                ExternalApp::VSCode,
                ExternalApp::Ghostty,
                ExternalApp::FileManager,
            ]
            .into_iter()
            .filter(|app| app.is_available())
            .collect();
            self.apps_detected = true;
        }
    }

    /// Show the inline widget and return selected action if any.
    /// `file_path` is the path to open, `base_path` is used for constructing full paths.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        theme: AppTheme,
        file_path: &std::path::Path,
        base_path: Option<&std::path::Path>,
    ) -> Option<FileOpenerAction> {
        self.ensure_apps_detected();

        let mut result = None;
        let icon_size = 16.0;
        let spacing = 4.0;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;

            for app in &self.available_apps {
                let icon = app.icon();
                let image = Image::new(icon.as_image_source())
                    .fit_to_exact_size(Vec2::splat(icon_size))
                    .sense(egui::Sense::click());

                let response = ui.add(image);

                if response.clicked() {
                    // Compute full path
                    let full_path = if let Some(base) = base_path {
                        base.join(file_path)
                    } else {
                        file_path.to_path_buf()
                    };
                    if let Err(e) = app.execute(&full_path) {
                        log::warn!("Failed to open in {}: {e}", app.name());
                    }
                    result = Some(FileOpenerAction::OpenIn(*app));
                }

                // Tooltip with app name
                response.on_hover_text(app.name());
            }

            // Optional copy button using a simpler icon
            let copy_btn = ui.add(
                egui::Button::new(
                    RichText::new(semantic_icons::action::COPY)
                        .size(icon_size - 2.0)
                        .color(theme.text_tertiary()),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            );

            if copy_btn.clicked() {
                let full_path = if let Some(base) = base_path {
                    base.join(file_path)
                } else {
                    file_path.to_path_buf()
                };
                ui.ctx().copy_text(full_path.display().to_string());
                result = Some(FileOpenerAction::CopyPath);
            }
            copy_btn.on_hover_text("Copy path");
        });

        result
    }

    /// WASM stub - does nothing in web mode.
    #[cfg(target_arch = "wasm32")]
    pub fn show(
        &mut self,
        _ui: &mut egui::Ui,
        _theme: AppTheme,
        _file_path: &std::path::Path,
        _base_path: Option<&std::path::Path>,
    ) -> Option<FileOpenerAction> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_app_names() {
        assert_eq!(ExternalApp::Zed.name(), "Zed");
        assert_eq!(ExternalApp::VSCode.name(), "VS Code");
        assert_eq!(ExternalApp::Ghostty.name(), "Ghostty");
    }

    #[test]
    fn test_action_labels() {
        assert_eq!(
            FileOpenerAction::OpenIn(ExternalApp::Zed).label(),
            "Open in Zed"
        );
        assert_eq!(FileOpenerAction::CopyPath.label(), "Copy path");
    }

    #[test]
    fn test_popup_default_closed() {
        let popup = FileOpenerPopup::new();
        assert!(!popup.is_open());
    }

    #[test]
    fn test_popup_open_close() {
        let mut popup = FileOpenerPopup::new();
        popup.open(egui::Pos2::ZERO, std::path::PathBuf::from("/test/path"));
        assert!(popup.is_open());

        popup.close();
        assert!(!popup.is_open());
    }

    #[test]
    fn test_file_path_accessor() {
        let mut popup = FileOpenerPopup::new();
        assert!(popup.file_path().is_none());

        popup.open(egui::Pos2::ZERO, std::path::PathBuf::from("/test/file.rs"));
        assert_eq!(
            popup.file_path(),
            Some(std::path::Path::new("/test/file.rs"))
        );

        // File path should remain accessible after close
        popup.close();
        assert_eq!(
            popup.file_path(),
            Some(std::path::Path::new("/test/file.rs"))
        );
    }

    #[test]
    fn test_relative_path_computation() {
        let mut popup = FileOpenerPopup::new();

        // Without base path, relative_path returns None
        popup.open(
            egui::Pos2::ZERO,
            std::path::PathBuf::from("/workspace/src/main.rs"),
        );
        assert!(popup.relative_path().is_none());

        // With base path, relative_path is computed
        popup.open_with_base(
            egui::Pos2::ZERO,
            std::path::PathBuf::from("/workspace/src/main.rs"),
            Some(std::path::PathBuf::from("/workspace")),
        );
        assert_eq!(
            popup.relative_path(),
            Some(std::path::PathBuf::from("src/main.rs"))
        );

        // If file is not under base, relative_path returns None
        popup.open_with_base(
            egui::Pos2::ZERO,
            std::path::PathBuf::from("/other/file.rs"),
            Some(std::path::PathBuf::from("/workspace")),
        );
        assert!(popup.relative_path().is_none());
    }

    #[test]
    fn test_all_actions_without_base_path() {
        let mut popup = FileOpenerPopup::new();
        popup.open(egui::Pos2::ZERO, std::path::PathBuf::from("/test/file.rs"));

        let actions = popup.all_actions();

        // Should have app actions + CopyPath (no CopyRelativePath without base)
        assert!(actions.contains(&FileOpenerAction::CopyPath));
        assert!(!actions.contains(&FileOpenerAction::CopyRelativePath));
    }

    #[test]
    fn test_all_actions_with_base_path() {
        let mut popup = FileOpenerPopup::new();
        popup.open_with_base(
            egui::Pos2::ZERO,
            std::path::PathBuf::from("/workspace/file.rs"),
            Some(std::path::PathBuf::from("/workspace")),
        );

        let actions = popup.all_actions();

        // Should have both copy actions when base path is set
        assert!(actions.contains(&FileOpenerAction::CopyPath));
        assert!(actions.contains(&FileOpenerAction::CopyRelativePath));
    }

    #[test]
    fn test_item_count_matches_actions() {
        let mut popup = FileOpenerPopup::new();
        popup.open(egui::Pos2::ZERO, std::path::PathBuf::from("/test/file.rs"));

        // item_count should match all_actions length
        assert_eq!(popup.item_count(), popup.all_actions().len());
    }

    #[test]
    fn test_selected_index_wraps() {
        let mut popup = FileOpenerPopup::new();
        popup.open(egui::Pos2::ZERO, std::path::PathBuf::from("/test/file.rs"));

        // Selection starts at 0
        assert_eq!(popup.selected_index, 0);
    }

    #[test]
    fn test_action_label_for_all_apps() {
        // Verify all apps have proper labels
        assert_eq!(
            FileOpenerAction::OpenIn(ExternalApp::VSCode).label(),
            "Open in VS Code"
        );
        assert_eq!(
            FileOpenerAction::OpenIn(ExternalApp::Ghostty).label(),
            "Open in Ghostty"
        );
        assert_eq!(
            FileOpenerAction::CopyRelativePath.label(),
            "Copy relative path"
        );
    }

    #[test]
    fn test_file_opener_result_default() {
        let result = FileOpenerResult::default();
        assert_eq!(result, FileOpenerResult::None);
    }
}
