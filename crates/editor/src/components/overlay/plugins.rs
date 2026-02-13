//! Plugins overlay component for viewing and managing installed plugins.
//!
//! Similar to LazyVim's plugin manager, this overlay displays all registered
//! plugins with their status, version, and capabilities. It also supports
//! browsing and installing community plugins from a remote registry.

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use egui::{Color32, Key, RichText, ScrollArea};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::{OverlayStyle, render_keyboard_hint_pill};
#[cfg(not(target_arch = "wasm32"))]
use crate::components::util::{FileOpenerAction, FileOpenerPopup, FileOpenerResult};
#[cfg(not(target_arch = "wasm32"))]
use crate::ui::icons::APP_GHOSTTY;

/// Information about a plugin to display in the overlay.
#[derive(Debug, Clone)]
pub struct PluginDisplayInfo {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Whether the plugin is enabled
    pub enabled: bool,
    /// Plugin source type
    pub source: PluginSource,
    /// Number of commands provided
    pub command_count: usize,
    /// Number of keybindings provided
    pub keybinding_count: usize,
}

/// Information about a community plugin available for installation.
#[derive(Debug, Clone)]
pub struct CommunityPluginInfo {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author
    pub author: String,
    /// Filename of the plugin
    pub file: String,
    /// Whether this plugin is already installed
    pub installed: bool,
}

/// Current tab in the plugins overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PluginTab {
    /// Installed plugins
    #[default]
    Installed,
    /// Available community plugins
    Available,
}

/// Source type of a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginSource {
    /// TOML config plugin
    Config,
    /// Lua script plugin
    Lua,
}

impl PluginSource {
    /// Get display name for the source type.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Lua => "lua",
        }
    }

    /// Get icon for the source type.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Config => semantic_icons::file::CONFIG,
            Self::Lua => semantic_icons::language::LUA,
        }
    }
}

/// Result from showing the plugins overlay.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginsOverlayResult {
    /// No action
    None,
    /// Toggle plugin enabled state
    TogglePlugin(String),
    /// Open plugin directory
    OpenPluginDirectory,
    /// Install a community plugin (name, file)
    InstallPlugin(String, String),
    /// Remove an installed plugin (name)
    RemovePlugin(String),
    /// Refresh available plugins from remote
    RefreshAvailable,
    /// Closed
    Closed,
}

/// Braille spinner frames for installation animation.
#[cfg(not(target_arch = "wasm32"))]
const BRAILLE_SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// A modal overlay for viewing and managing plugins.
pub struct PluginsOverlay {
    /// Whether the overlay is open
    is_open: bool,
    /// Skip first frame of input (to avoid closing on same key that opened it)
    just_opened: bool,
    /// Current theme (supports Custom variant with plugin colors)
    theme: AppTheme,
    /// Installed plugins to display
    plugins: Vec<PluginDisplayInfo>,
    /// Available community plugins
    available_plugins: Vec<CommunityPluginInfo>,
    /// Currently selected plugin index
    selected_index: usize,
    /// Target selection index for smooth animation
    target_index: usize,
    /// Animation progress (0.0 to 1.0)
    selection_anim_progress: f32,
    /// Whether to show only enabled plugins (for Installed tab)
    show_enabled_only: bool,
    /// Current tab
    current_tab: PluginTab,
    /// Whether we're currently loading available plugins
    loading_available: bool,
    /// Plugin currently being installed (if any)
    installing_plugin: Option<String>,
    /// Whether we need to auto-refresh available plugins on next show
    needs_auto_refresh: bool,
    /// Plugin pending removal confirmation (name)
    pending_remove: Option<String>,
    /// Search filter text
    search_filter: String,
    /// Whether search input is focused
    search_focused: bool,
    /// Whether 'g' was pressed (for gg navigation)
    g_pressed: bool,
    /// File opener popup for opening plugin directory in external apps
    #[cfg(not(target_arch = "wasm32"))]
    file_opener: FileOpenerPopup,
    /// Whether file opener popup should open (set when 'o' is pressed)
    #[cfg(not(target_arch = "wasm32"))]
    pending_open_file_opener: bool,
    /// Cached plugin directory path
    #[cfg(not(target_arch = "wasm32"))]
    plugin_directory: Option<PathBuf>,
}

impl PluginsOverlay {
    pub fn new() -> Self {
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            plugins: Vec::new(),
            available_plugins: Vec::new(),
            selected_index: 0,
            target_index: 0,
            selection_anim_progress: 1.0,
            show_enabled_only: false,
            current_tab: PluginTab::Installed,
            loading_available: false,
            installing_plugin: None,
            needs_auto_refresh: true, // Refresh on first open
            pending_remove: None,
            search_filter: String::new(),
            search_focused: false,
            g_pressed: false,
            #[cfg(not(target_arch = "wasm32"))]
            file_opener: FileOpenerPopup::new(),
            #[cfg(not(target_arch = "wasm32"))]
            pending_open_file_opener: false,
            #[cfg(not(target_arch = "wasm32"))]
            plugin_directory: dirs::home_dir()
                .map(|d| d.join(".config").join("enya").join("plugins")),
        }
    }

    /// Set the theme (supports Custom variant with plugin colors).
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the installed plugins to display.
    pub fn set_plugins(&mut self, plugins: Vec<PluginDisplayInfo>) {
        self.plugins = plugins;
        // Sort: enabled first, then by source (Lua > Config), then alphabetically
        self.plugins.sort_by(|a, b| {
            // First, enabled plugins come first
            match (a.enabled, b.enabled) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            // Then sort by source type: Lua > Config
            match (a.source, b.source) {
                (PluginSource::Lua, PluginSource::Config) => return std::cmp::Ordering::Less,
                (PluginSource::Config, PluginSource::Lua) => return std::cmp::Ordering::Greater,
                _ => {}
            }
            // Finally, sort alphabetically by name
            a.name.cmp(&b.name)
        });
        if self.current_tab == PluginTab::Installed && self.selected_index >= self.plugins.len() {
            self.selected_index = 0;
        }
        // Update installed status for available plugins
        self.update_installed_status();
        // Clear installing state when plugins are refreshed
        self.installing_plugin = None;
    }

    /// Set the available community plugins.
    pub fn set_available_plugins(&mut self, plugins: Vec<CommunityPluginInfo>) {
        self.available_plugins = plugins;
        self.loading_available = false;
        // Clear auto-refresh flag since we now have data
        self.needs_auto_refresh = false;
        // Clear installing state when plugins are refreshed
        self.installing_plugin = None;
        // Sort alphabetically by name
        self.available_plugins.sort_by(|a, b| a.name.cmp(&b.name));
        // Update installed status
        self.update_installed_status();
        if self.current_tab == PluginTab::Available
            && self.selected_index >= self.available_plugins.len()
        {
            self.selected_index = 0;
        }
    }

    /// Mark that we're loading available plugins.
    pub fn set_loading_available(&mut self, loading: bool) {
        self.loading_available = loading;
    }

    /// Set the plugin currently being installed.
    pub fn set_installing_plugin(&mut self, name: Option<String>) {
        self.installing_plugin = name;
    }

    /// Update installed status for available plugins based on installed plugins.
    fn update_installed_status(&mut self) {
        let installed_names: rustc_hash::FxHashSet<_> =
            self.plugins.iter().map(|p| &p.name).collect();
        for plugin in &mut self.available_plugins {
            plugin.installed = installed_names.contains(&plugin.name);
        }
    }

    /// Open the overlay.
    pub fn open(&mut self) {
        self.is_open = true;
        self.just_opened = true;
        self.selected_index = 0;
    }

    /// Close the overlay.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Check if the overlay is open.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Show the overlay.
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> PluginsOverlayResult {
        if !self.is_open {
            return PluginsOverlayResult::None;
        }

        // Skip input on first frame
        if self.just_opened {
            self.just_opened = false;
            ctx.request_repaint();
            return PluginsOverlayResult::None;
        }

        // Auto-refresh available plugins if needed
        if self.needs_auto_refresh && !self.loading_available {
            self.needs_auto_refresh = false;
            self.loading_available = true;
            return PluginsOverlayResult::RefreshAvailable;
        }

        let mut result = PluginsOverlayResult::None;

        // Update selection animation
        if self.selected_index != self.target_index {
            self.selection_anim_progress += ctx.input(|i| i.stable_dt) * 8.0;
            if self.selection_anim_progress >= 1.0 {
                self.selection_anim_progress = 1.0;
                self.selected_index = self.target_index;
            }
            ctx.request_repaint();
        }

        // Handle keyboard input
        let current_tab = self.current_tab;
        let has_pending_remove = self.pending_remove.is_some();
        let search_focused = self.search_focused;

        ctx.input_mut(|input| {
            // Handle confirmation dialog inputs first
            if has_pending_remove {
                if input.consume_key(egui::Modifiers::NONE, Key::Y) {
                    // Confirm removal
                    if let Some(name) = self.pending_remove.take() {
                        result = PluginsOverlayResult::RemovePlugin(name);
                    }
                    return;
                }
                if input.consume_key(egui::Modifiers::NONE, Key::N)
                    || input.consume_key(egui::Modifiers::NONE, Key::Escape)
                {
                    // Cancel removal
                    self.pending_remove = None;
                    return;
                }
                // Ignore other keys while confirmation is shown
                return;
            }

            // Handle search input when focused
            if search_focused {
                if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                    self.search_focused = false;
                    self.search_filter.clear();
                    return;
                }
                if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                    self.search_focused = false;
                    return;
                }
                if input.consume_key(egui::Modifiers::NONE, Key::Backspace) {
                    self.search_filter.pop();
                    self.selected_index = 0;
                    self.target_index = 0;
                    return;
                }
                // Capture typed characters
                for event in &input.events {
                    if let egui::Event::Text(text) = event {
                        self.search_filter.push_str(text);
                        self.selected_index = 0;
                        self.target_index = 0;
                    }
                }
                return;
            }

            // Normal keyboard handling
            if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                result = PluginsOverlayResult::Closed;
                return;
            }

            // / - Focus search
            if input.consume_key(egui::Modifiers::NONE, Key::Slash) {
                self.search_focused = true;
                return;
            }

            // Tab / Shift+Tab - Switch tabs
            if input.consume_key(egui::Modifiers::NONE, Key::Tab) {
                self.current_tab = match self.current_tab {
                    PluginTab::Installed => PluginTab::Available,
                    PluginTab::Available => PluginTab::Installed,
                };
                self.selected_index = 0;
                self.target_index = 0;
                self.search_filter.clear();
                return;
            }

            // 1 - Installed tab
            if input.consume_key(egui::Modifiers::NONE, Key::Num1) {
                self.current_tab = PluginTab::Installed;
                self.selected_index = 0;
                self.target_index = 0;
                self.search_filter.clear();
                return;
            }

            // 2 - Available tab
            if input.consume_key(egui::Modifiers::NONE, Key::Num2) {
                self.current_tab = PluginTab::Available;
                self.selected_index = 0;
                self.target_index = 0;
                self.search_filter.clear();
                return;
            }

            // G - Jump to last item (Shift+g)
            if input.consume_key(egui::Modifiers::SHIFT, Key::G) {
                let visible_count = self.filtered_count(current_tab);
                if visible_count > 0 {
                    self.target_index = visible_count - 1;
                    self.selection_anim_progress = 0.0;
                }
                self.g_pressed = false;
                return;
            }

            // g - First press of gg sequence
            if input.consume_key(egui::Modifiers::NONE, Key::G) {
                if self.g_pressed {
                    // Second g - jump to first
                    self.target_index = 0;
                    self.selection_anim_progress = 0.0;
                    self.g_pressed = false;
                } else {
                    self.g_pressed = true;
                }
                return;
            } else {
                // Reset g_pressed if any other key is pressed
                self.g_pressed = false;
            }

            // j/Down - Move down
            if input.consume_key(egui::Modifiers::NONE, Key::J)
                || input.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
            {
                let visible_count = self.filtered_count(current_tab);
                if visible_count > 0 {
                    self.target_index = (self.target_index + 1) % visible_count;
                    self.selection_anim_progress = 0.0;
                }
                return;
            }

            // k/Up - Move up
            if input.consume_key(egui::Modifiers::NONE, Key::K)
                || input.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
            {
                let visible_count = self.filtered_count(current_tab);
                if visible_count > 0 {
                    self.target_index = if self.target_index == 0 {
                        visible_count - 1
                    } else {
                        self.target_index - 1
                    };
                    self.selection_anim_progress = 0.0;
                }
                return;
            }

            // f - Toggle filter (show enabled only) - only for Installed tab
            if input.consume_key(egui::Modifiers::NONE, Key::F) {
                if current_tab == PluginTab::Installed {
                    self.show_enabled_only = !self.show_enabled_only;
                    self.selected_index = 0;
                    self.target_index = 0;
                }
                return;
            }

            // r - Refresh available plugins
            if input.consume_key(egui::Modifiers::NONE, Key::R) {
                if current_tab == PluginTab::Available {
                    result = PluginsOverlayResult::RefreshAvailable;
                }
                return;
            }

            // o - Open plugin directory (triggers file opener popup on native)
            if input.consume_key(egui::Modifiers::NONE, Key::O) {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.pending_open_file_opener = true;
                }
                #[cfg(target_arch = "wasm32")]
                {
                    result = PluginsOverlayResult::OpenPluginDirectory;
                }
                return;
            }

            // Enter - Toggle selected plugin (Installed tab only)
            if input.consume_key(egui::Modifiers::NONE, Key::Enter) {
                if current_tab == PluginTab::Installed {
                    if let Some(plugin) = self.filtered_plugins(current_tab).nth(self.target_index)
                    {
                        result = PluginsOverlayResult::TogglePlugin(plugin.name.clone());
                    }
                }
                return;
            }

            // i - Install selected plugin (Available tab only)
            if input.consume_key(egui::Modifiers::NONE, Key::I)
                && current_tab == PluginTab::Available
            {
                if let Some(plugin) = self.filtered_available_plugins().nth(self.target_index) {
                    if !plugin.installed {
                        result = PluginsOverlayResult::InstallPlugin(
                            plugin.name.clone(),
                            plugin.file.clone(),
                        );
                    }
                }
                return;
            }

            // x - Remove selected plugin (Installed tab only) - shows confirmation
            if input.consume_key(egui::Modifiers::NONE, Key::X)
                && current_tab == PluginTab::Installed
            {
                let name = self
                    .filtered_plugins(current_tab)
                    .nth(self.target_index)
                    .map(|p| p.name.clone());
                if let Some(name) = name {
                    self.pending_remove = Some(name);
                }
            }
        });

        if result == PluginsOverlayResult::Closed {
            self.close();
            return result;
        }

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let popup_width = (screen_rect.width() * 0.6).clamp(500.0, 800.0);
        let popup_max_height = (screen_rect.height() * 0.7).min(600.0);

        // Pre-calculate colors for closure access (Custom variant handles plugin colors internally)
        let overlay_style = OverlayStyle::frosted_glass(self.theme);
        let separator_color = self.theme.border_subtle();
        let muted_text = self.theme.text_primary().gamma_multiply(0.6);
        let accent_color = self.theme.accent_hover();
        let text_col = self.theme.text_primary();
        #[cfg(not(target_arch = "wasm32"))]
        let accent_primary = self.theme.accent_primary();

        egui::Area::new(egui::Id::new("plugins_overlay_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(popup_width);
                    ui.set_max_height(popup_max_height);

                    // Header section
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(semantic_icons::action::TOOL)
                                .color(accent_color)
                                .size(20.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Plugins")
                                .color(accent_color)
                                .size(18.0)
                                .strong(),
                        );

                        // Right side: Open button and stats
                        // Use 52px spacing to align with [config] labels in rows
                        // (rows are popup_width-32 wide with 20px internal right padding = 52px from edge)
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(52.0);

                            // Stats
                            match self.current_tab {
                                PluginTab::Installed => {
                                    let enabled_count =
                                        self.plugins.iter().filter(|p| p.enabled).count();
                                    let total_count = self.plugins.len();
                                    ui.label(
                                        RichText::new(format!(
                                            "{enabled_count}/{total_count} enabled"
                                        ))
                                        .color(muted_text)
                                        .font(typography::proportional(typography::MD)),
                                    );
                                }
                                PluginTab::Available => {
                                    let available_count = self
                                        .available_plugins
                                        .iter()
                                        .filter(|p| !p.installed)
                                        .count();
                                    ui.label(
                                        RichText::new(format!("{available_count} available"))
                                            .color(muted_text)
                                            .font(typography::proportional(typography::MD)),
                                    );
                                }
                            }

                            ui.add_space(12.0);

                            // "Open" dropdown button for plugins directory (native only)
                            #[cfg(not(target_arch = "wasm32"))]
                            if let Some(ref dir) = self.plugin_directory {
                                let btn = ui.add(
                                    egui::Button::image_and_text(
                                        egui::Image::new(APP_GHOSTTY.as_image_source())
                                            .fit_to_exact_size(egui::vec2(14.0, 14.0)),
                                        RichText::new(format!(
                                            "Open {}",
                                            egui_nerdfonts::regular::CHEVRON_DOWN
                                        ))
                                        .size(typography::SM)
                                        .color(self.theme.text_secondary()),
                                    )
                                    .fill(self.theme.bg_elevated())
                                    .stroke(egui::Stroke::new(1.0, self.theme.border_subtle()))
                                    .corner_radius(4.0),
                                );

                                // Open popup on button click or 'o' key press
                                if btn.clicked() || self.pending_open_file_opener {
                                    self.pending_open_file_opener = false;
                                    let popup_pos = btn.rect.left_bottom();
                                    self.file_opener.open(popup_pos, dir.clone());
                                }
                            }
                        });
                    });
                    ui.add_space(8.0);

                    // Tab bar with underline indicators
                    let tab_bar_response = ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        let tab_installed_color = if self.current_tab == PluginTab::Installed {
                            accent_color
                        } else {
                            muted_text
                        };
                        let tab_available_color = if self.current_tab == PluginTab::Available {
                            accent_color
                        } else {
                            muted_text
                        };

                        // Track tab positions for underline
                        let installed_start = ui.cursor().left();
                        ui.label(
                            RichText::new("1")
                                .color(tab_installed_color.gamma_multiply(0.6))
                                .font(typography::monospace(typography::SM)),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new("Installed")
                                .color(tab_installed_color)
                                .font(typography::proportional(typography::MD))
                                .strong(),
                        );
                        let installed_end = ui.cursor().left();

                        ui.add_space(24.0);

                        let available_start = ui.cursor().left();
                        ui.label(
                            RichText::new("2")
                                .color(tab_available_color.gamma_multiply(0.6))
                                .font(typography::monospace(typography::SM)),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new("Available")
                                .color(tab_available_color)
                                .font(typography::proportional(typography::MD))
                                .strong(),
                        );
                        let available_end = ui.cursor().left();

                        if self.loading_available && self.current_tab == PluginTab::Available {
                            ui.add_space(8.0);
                            ui.spinner();
                        }

                        // Return tab positions
                        (
                            installed_start,
                            installed_end,
                            available_start,
                            available_end,
                        )
                    });
                    let (installed_start, installed_end, available_start, available_end) =
                        tab_bar_response.inner;
                    let tab_bottom = tab_bar_response.response.rect.bottom();

                    // Draw underline for active tab
                    let underline_y = tab_bottom + 4.0;
                    let (underline_start, underline_end) = match self.current_tab {
                        PluginTab::Installed => (installed_start, installed_end),
                        PluginTab::Available => (available_start, available_end),
                    };
                    ui.painter().hline(
                        underline_start..=underline_end,
                        underline_y,
                        egui::Stroke::new(2.0, accent_color),
                    );

                    ui.add_space(12.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(8.0);

                    // Search input or filter indicator
                    if self.search_focused || !self.search_filter.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(semantic_icons::action::SEARCH)
                                    .color(accent_color)
                                    .size(14.0),
                            );
                            ui.add_space(4.0);
                            let search_text = if self.search_filter.is_empty() {
                                "Type to search...".to_string()
                            } else {
                                self.search_filter.clone()
                            };
                            let search_color = if self.search_filter.is_empty() {
                                muted_text.gamma_multiply(0.7)
                            } else {
                                text_col
                            };
                            ui.label(
                                RichText::new(&search_text)
                                    .color(search_color)
                                    .font(typography::proportional(typography::MD)),
                            );
                            if self.search_focused {
                                // Show blinking cursor
                                let blink = (ui.input(|i| i.time) * 2.0) as i32 % 2 == 0;
                                if blink {
                                    ui.label(
                                        RichText::new("|")
                                            .color(accent_color)
                                            .font(typography::proportional(typography::MD)),
                                    );
                                }
                                ui.ctx().request_repaint();
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(16.0);
                                    ui.label(
                                        RichText::new("Esc to clear")
                                            .color(muted_text.gamma_multiply(0.6))
                                            .font(typography::proportional(typography::XS)),
                                    );
                                },
                            );
                        });
                        ui.add_space(4.0);
                    } else if self.current_tab == PluginTab::Installed && self.show_enabled_only {
                        // Filter indicator (only for Installed tab)
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(format!(
                                    "{}  Showing enabled only",
                                    semantic_icons::action::FILTER
                                ))
                                .color(accent_color.gamma_multiply(0.8))
                                .font(typography::proportional(typography::SM)),
                            );
                        });
                        ui.add_space(4.0);
                    }

                    // Plugin list
                    let list_height = popup_max_height - 180.0;

                    ScrollArea::vertical()
                        .max_height(list_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(popup_width - 16.0);

                            // Show native-only notice on WASM
                            #[cfg(target_arch = "wasm32")]
                            {
                                ui.add_space(list_height * 0.3);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new(egui_nerdfonts::regular::DESKTOP)
                                            .color(muted_text)
                                            .size(32.0),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("Native app required")
                                            .color(text_col)
                                            .font(typography::proportional(typography::LG)),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(
                                            "Plugins are only available in the native app",
                                        )
                                        .color(muted_text)
                                        .font(typography::proportional(typography::SM)),
                                    );
                                });
                            }

                            #[cfg(not(target_arch = "wasm32"))]
                            match self.current_tab {
                                PluginTab::Installed => {
                                    let filtered_plugins: Vec<_> = self
                                        .filtered_plugins(PluginTab::Installed)
                                        .cloned()
                                        .collect();

                                    if filtered_plugins.is_empty() {
                                        ui.add_space(20.0);
                                        ui.vertical_centered(|ui| {
                                            ui.label(
                                                RichText::new(semantic_icons::empty::NO_ITEMS)
                                                    .color(muted_text)
                                                    .size(32.0),
                                            );
                                            ui.add_space(8.0);
                                            let message = if !self.search_filter.is_empty() {
                                                "No matching plugins"
                                            } else {
                                                "No plugins found"
                                            };
                                            ui.label(
                                                RichText::new(message)
                                                    .color(muted_text)
                                                    .font(typography::proportional(typography::LG)),
                                            );
                                        });
                                    } else {
                                        // Build map of available versions for update detection
                                        let available_versions: rustc_hash::FxHashMap<_, _> = self
                                            .available_plugins
                                            .iter()
                                            .map(|p| (p.name.as_str(), p.version.as_str()))
                                            .collect();

                                        for (idx, plugin) in filtered_plugins.iter().enumerate() {
                                            let is_selected = idx == self.target_index;
                                            // Check if update is available
                                            let has_update = available_versions
                                                .get(plugin.name.as_str())
                                                .is_some_and(|av| *av != plugin.version);

                                            let response = Self::show_plugin_row(
                                                ui,
                                                plugin,
                                                is_selected,
                                                has_update,
                                                text_col,
                                                accent_color,
                                                accent_primary,
                                                muted_text,
                                                popup_width - 32.0,
                                            );

                                            // Scroll to selected item
                                            if is_selected {
                                                response.scroll_to_me(Some(egui::Align::Center));
                                            }
                                        }
                                    }
                                }
                                PluginTab::Available => {
                                    if self.loading_available {
                                        ui.add_space(40.0);
                                        ui.vertical_centered(|ui| {
                                            ui.spinner();
                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new("Loading plugins...")
                                                    .color(muted_text)
                                                    .font(typography::proportional(typography::MD)),
                                            );
                                        });
                                    } else if self.available_plugins.is_empty() {
                                        ui.add_space(20.0);
                                        ui.vertical_centered(|ui| {
                                            ui.label(
                                                RichText::new(semantic_icons::status::INFO)
                                                    .color(muted_text)
                                                    .size(32.0),
                                            );
                                            ui.add_space(8.0);
                                            ui.label(
                                                RichText::new("No community plugins available")
                                                    .color(muted_text)
                                                    .font(typography::proportional(typography::LG)),
                                            );
                                            ui.add_space(4.0);
                                            ui.label(
                                                RichText::new("Press 'r' to refresh")
                                                    .color(muted_text.gamma_multiply(0.7))
                                                    .font(typography::proportional(typography::SM)),
                                            );
                                        });
                                    } else {
                                        let filtered_available: Vec<_> =
                                            self.filtered_available_plugins().cloned().collect();

                                        if filtered_available.is_empty()
                                            && !self.search_filter.is_empty()
                                        {
                                            ui.add_space(20.0);
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    RichText::new(semantic_icons::empty::NO_ITEMS)
                                                        .color(muted_text)
                                                        .size(32.0),
                                                );
                                                ui.add_space(8.0);
                                                ui.label(
                                                    RichText::new("No matching plugins")
                                                        .color(muted_text)
                                                        .font(typography::proportional(
                                                            typography::LG,
                                                        )),
                                                );
                                            });
                                        } else {
                                            // Get current time for spinner animation
                                            let time = ui.input(|i| i.time);
                                            for (idx, plugin) in
                                                filtered_available.iter().enumerate()
                                            {
                                                let is_selected = idx == self.target_index;
                                                let is_installing = self
                                                    .installing_plugin
                                                    .as_ref()
                                                    .is_some_and(|name| name == &plugin.name);
                                                let response = Self::show_available_plugin_row(
                                                    ui,
                                                    plugin,
                                                    is_selected,
                                                    is_installing,
                                                    time,
                                                    text_col,
                                                    accent_color,
                                                    accent_primary,
                                                    muted_text,
                                                    popup_width - 32.0,
                                                );

                                                // Scroll to selected item
                                                if is_selected {
                                                    response
                                                        .scroll_to_me(Some(egui::Align::Center));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        });

                    ui.add_space(8.0);

                    // Separator above footer
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(8.0);

                    // Footer with keyboard hints (compact spacing)
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        render_keyboard_hint_pill(ui, "Tab", "tab", muted_text, text_col);
                        ui.add_space(8.0);
                        render_keyboard_hint_pill(ui, "j/k", "nav", muted_text, text_col);
                        ui.add_space(8.0);
                        render_keyboard_hint_pill(ui, "/", "search", muted_text, text_col);
                        ui.add_space(8.0);
                        match self.current_tab {
                            PluginTab::Installed => {
                                render_keyboard_hint_pill(ui, "f", "filter", muted_text, text_col);
                                ui.add_space(8.0);
                                render_keyboard_hint_pill(ui, "x", "remove", muted_text, text_col);
                            }
                            PluginTab::Available => {
                                render_keyboard_hint_pill(ui, "r", "refresh", muted_text, text_col);
                                ui.add_space(8.0);
                                render_keyboard_hint_pill(ui, "i", "install", muted_text, text_col);
                            }
                        }
                        ui.add_space(8.0);
                        render_keyboard_hint_pill(ui, "o", "open", muted_text, text_col);
                        ui.add_space(8.0);
                        render_keyboard_hint_pill(ui, "Esc", "close", muted_text, text_col);
                    });
                    ui.add_space(12.0);
                });
            });

        // File opener popup for opening plugin directory in external apps
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Open popup if pending flag is set
            if self.pending_open_file_opener {
                self.pending_open_file_opener = false;
                if let Some(ref plugin_dir) = self.plugin_directory {
                    // Position near center of screen (popup will appear below this point)
                    let popup_pos = egui::pos2(
                        screen_rect.center().x - 100.0,
                        screen_rect.center().y - 50.0,
                    );
                    self.file_opener.open(popup_pos, plugin_dir.clone());
                }
            }

            // Show file opener popup and handle result
            match self.file_opener.show(ctx, self.theme) {
                FileOpenerResult::Selected(action) => {
                    if let Some(ref plugin_dir) = self.plugin_directory {
                        match action {
                            FileOpenerAction::OpenIn(app) => {
                                if let Err(e) = app.execute(plugin_dir) {
                                    log::warn!(
                                        "Failed to open plugin directory in {}: {e}",
                                        app.name()
                                    );
                                }
                            }
                            FileOpenerAction::CopyPath => {
                                ctx.copy_text(plugin_dir.display().to_string());
                            }
                            FileOpenerAction::CopyRelativePath => {
                                // Not applicable for plugin directory
                            }
                        }
                    }
                }
                FileOpenerResult::Closed | FileOpenerResult::None => {}
            }
        }

        // Confirmation dialog as separate centered overlay
        if let Some(plugin_name) = &self.pending_remove {
            let plugin_name = plugin_name.clone();

            // Full screen semi-transparent backdrop
            egui::Area::new(egui::Id::new("plugins_confirm_backdrop"))
                .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ctx, |ui| {
                    let screen = ctx.available_rect();
                    ui.painter()
                        .rect_filled(screen, 0.0, Color32::from_black_alpha(180));
                });

            // Centered dialog box
            egui::Area::new(egui::Id::new("plugins_confirm_dialog"))
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let dialog_width = 320.0;
                    let dialog_height = 120.0;

                    egui::Frame::new()
                        .fill(self.theme.bg_elevated())
                        .corner_radius(8.0)
                        .stroke(egui::Stroke::new(1.0, accent_color.gamma_multiply(0.5)))
                        .inner_margin(20.0)
                        .show(ui, |ui| {
                            ui.set_width(dialog_width - 40.0);
                            ui.set_height(dialog_height - 40.0);

                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new(format!("Remove \"{plugin_name}\"?"))
                                        .color(text_col)
                                        .font(typography::proportional(typography::LG)),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("This will delete the plugin file.")
                                        .color(muted_text)
                                        .font(typography::proportional(typography::SM)),
                                );
                                ui.add_space(16.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(40.0);
                                    ui.label(
                                        RichText::new("[y] confirm")
                                            .color(accent_color)
                                            .font(typography::monospace(typography::SM)),
                                    );
                                    ui.add_space(24.0);
                                    ui.label(
                                        RichText::new("[n] cancel")
                                            .color(muted_text)
                                            .font(typography::monospace(typography::SM)),
                                    );
                                });
                            });
                        });
                });
        }

        result
    }

    /// Get filtered plugins based on search filter and enabled filter.
    fn filtered_plugins(
        &self,
        tab: PluginTab,
    ) -> Box<dyn Iterator<Item = &PluginDisplayInfo> + '_> {
        if tab != PluginTab::Installed {
            return Box::new(std::iter::empty());
        }
        let filter = self.search_filter.to_lowercase();
        Box::new(self.plugins.iter().filter(move |p| {
            let matches_enabled = !self.show_enabled_only || p.enabled;
            let matches_search = filter.is_empty()
                || p.name.to_lowercase().contains(&filter)
                || p.description.to_lowercase().contains(&filter);
            matches_enabled && matches_search
        }))
    }

    /// Get filtered available plugins based on search filter.
    fn filtered_available_plugins(&self) -> impl Iterator<Item = &CommunityPluginInfo> {
        let filter = self.search_filter.to_lowercase();
        self.available_plugins.iter().filter(move |p| {
            filter.is_empty()
                || p.name.to_lowercase().contains(&filter)
                || p.description.to_lowercase().contains(&filter)
        })
    }

    /// Get count of filtered items for current tab.
    fn filtered_count(&self, tab: PluginTab) -> usize {
        match tab {
            PluginTab::Installed => self.filtered_plugins(tab).count(),
            PluginTab::Available => self.filtered_available_plugins().count(),
        }
    }

    /// Show a single plugin row. Returns the response for scroll handling.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::too_many_arguments)]
    fn show_plugin_row(
        ui: &mut egui::Ui,
        plugin: &PluginDisplayInfo,
        is_selected: bool,
        has_update: bool,
        text_col: Color32,
        accent_color: Color32,
        accent_primary: Color32,
        muted_text: Color32,
        width: f32,
    ) -> egui::Response {
        let row_height = 52.0;
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::new(width, row_height), egui::Sense::hover());

        // Background and left accent bar for selected item
        if is_selected {
            ui.painter()
                .rect_filled(rect, 6.0, accent_color.gamma_multiply(0.10));
            // Left accent bar
            let bar_rect = egui::Rect::from_min_size(rect.min, egui::Vec2::new(3.0, row_height));
            ui.painter().rect_filled(bar_rect, 2.0, accent_color);
        }

        // Status indicator (enabled/disabled)
        let status_color = if plugin.enabled {
            accent_primary
        } else {
            muted_text.gamma_multiply(0.5)
        };
        let status_icon = if plugin.enabled {
            semantic_icons::status::SUCCESS
        } else {
            semantic_icons::status::EMPTY
        };

        // Draw status icon
        ui.painter().text(
            egui::pos2(rect.min.x + 20.0, rect.center().y - 8.0),
            egui::Align2::LEFT_CENTER,
            status_icon,
            egui::FontId::proportional(14.0),
            status_color,
        );

        // Source type icon
        let source_icon_color = if is_selected {
            accent_color
        } else {
            muted_text
        };
        ui.painter().text(
            egui::pos2(rect.min.x + 44.0, rect.center().y - 8.0),
            egui::Align2::LEFT_CENTER,
            plugin.source.icon(),
            egui::FontId::proportional(14.0),
            source_icon_color,
        );

        // Plugin name
        let name_color = if is_selected {
            accent_color
        } else if plugin.enabled {
            text_col
        } else {
            muted_text
        };
        ui.painter().text(
            egui::pos2(rect.min.x + 68.0, rect.center().y - 8.0),
            egui::Align2::LEFT_CENTER,
            &plugin.name,
            typography::proportional(typography::LG),
            name_color,
        );

        // Version badge with update indicator
        let version_text = format!("v{}", plugin.version);
        let version_x = rect.min.x + 68.0 + 150.0;
        ui.painter().text(
            egui::pos2(version_x, rect.center().y - 8.0),
            egui::Align2::LEFT_CENTER,
            &version_text,
            typography::monospace(typography::SM),
            muted_text.gamma_multiply(0.7),
        );

        // Update available badge
        if has_update {
            let update_x = version_x + 50.0;
            // Draw pill background
            let pill_rect = egui::Rect::from_min_size(
                egui::pos2(update_x, rect.center().y - 16.0),
                egui::Vec2::new(52.0, 16.0),
            );
            ui.painter()
                .rect_filled(pill_rect, 4.0, accent_primary.gamma_multiply(0.2));
            ui.painter().text(
                pill_rect.center(),
                egui::Align2::CENTER_CENTER,
                "UPDATE",
                typography::monospace(typography::XS),
                accent_primary,
            );
        }

        // Source type label
        let source_label = format!("[{}]", plugin.source.display_name());
        ui.painter().text(
            egui::pos2(rect.max.x - 20.0, rect.center().y - 8.0),
            egui::Align2::RIGHT_CENTER,
            &source_label,
            typography::monospace(typography::SM),
            muted_text.gamma_multiply(0.6),
        );

        // Description (second line)
        let desc = if plugin.description.len() > 60 {
            format!("{}...", &plugin.description[..57])
        } else {
            plugin.description.clone()
        };
        ui.painter().text(
            egui::pos2(rect.min.x + 68.0, rect.center().y + 10.0),
            egui::Align2::LEFT_CENTER,
            &desc,
            typography::proportional(typography::SM),
            muted_text,
        );

        // Commands/keybindings count
        if plugin.command_count > 0 || plugin.keybinding_count > 0 {
            let stats = format!("{}cmd {}key", plugin.command_count, plugin.keybinding_count);
            ui.painter().text(
                egui::pos2(rect.max.x - 20.0, rect.center().y + 10.0),
                egui::Align2::RIGHT_CENTER,
                &stats,
                typography::monospace(typography::XS),
                muted_text.gamma_multiply(0.5),
            );
        }

        response
    }

    /// Show a single available plugin row. Returns the response for scroll handling.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(clippy::too_many_arguments)]
    fn show_available_plugin_row(
        ui: &mut egui::Ui,
        plugin: &CommunityPluginInfo,
        is_selected: bool,
        is_installing: bool,
        time: f64,
        text_col: Color32,
        accent_color: Color32,
        accent_primary: Color32,
        muted_text: Color32,
        width: f32,
    ) -> egui::Response {
        let row_height = 52.0;
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::new(width, row_height), egui::Sense::hover());

        // Background and left accent bar for selected item
        if is_selected {
            ui.painter()
                .rect_filled(rect, 6.0, accent_color.gamma_multiply(0.10));
            // Left accent bar
            let bar_rect = egui::Rect::from_min_size(rect.min, egui::Vec2::new(3.0, row_height));
            ui.painter().rect_filled(bar_rect, 2.0, accent_color);
        }

        // Status indicator (installing spinner, installed, or not installed)
        if is_installing {
            // Show braille spinner animation
            let frame_idx = ((time * 10.0) as usize) % BRAILLE_SPINNER.len();
            let spinner_char = BRAILLE_SPINNER[frame_idx];
            ui.painter().text(
                egui::pos2(rect.min.x + 20.0, rect.center().y - 8.0),
                egui::Align2::LEFT_CENTER,
                spinner_char.to_string(),
                egui::FontId::monospace(14.0),
                accent_color,
            );
            // Request repaint for animation
            ui.ctx().request_repaint();
        } else {
            let (status_color, status_icon) = if plugin.installed {
                (accent_primary, semantic_icons::status::SUCCESS)
            } else {
                (muted_text.gamma_multiply(0.5), semantic_icons::action::ADD)
            };

            // Draw status icon
            ui.painter().text(
                egui::pos2(rect.min.x + 20.0, rect.center().y - 8.0),
                egui::Align2::LEFT_CENTER,
                status_icon,
                egui::FontId::proportional(14.0),
                status_color,
            );
        }

        // Lua icon (all community plugins are Lua)
        let source_icon_color = if is_selected {
            accent_color
        } else {
            muted_text
        };
        ui.painter().text(
            egui::pos2(rect.min.x + 44.0, rect.center().y - 8.0),
            egui::Align2::LEFT_CENTER,
            semantic_icons::language::LUA,
            egui::FontId::proportional(14.0),
            source_icon_color,
        );

        // Plugin name
        let name_color = if is_selected {
            accent_color
        } else if plugin.installed {
            text_col.gamma_multiply(0.7)
        } else {
            text_col
        };
        ui.painter().text(
            egui::pos2(rect.min.x + 68.0, rect.center().y - 8.0),
            egui::Align2::LEFT_CENTER,
            &plugin.name,
            typography::proportional(typography::LG),
            name_color,
        );

        // Version badge
        let version_text = format!("v{}", plugin.version);
        ui.painter().text(
            egui::pos2(rect.min.x + 68.0 + 150.0, rect.center().y - 8.0),
            egui::Align2::LEFT_CENTER,
            &version_text,
            typography::monospace(typography::SM),
            muted_text.gamma_multiply(0.7),
        );

        // Description (second line)
        let desc = if plugin.description.len() > 60 {
            format!("{}...", &plugin.description[..57])
        } else {
            plugin.description.clone()
        };
        ui.painter().text(
            egui::pos2(rect.min.x + 68.0, rect.center().y + 10.0),
            egui::Align2::LEFT_CENTER,
            &desc,
            typography::proportional(typography::SM),
            muted_text,
        );

        // Status label on second line (installing, installed, or author)
        let (status_label, status_label_color) = if is_installing {
            ("installing...".to_string(), accent_color)
        } else if plugin.installed {
            ("[installed]".to_string(), muted_text.gamma_multiply(0.6))
        } else {
            (
                format!("by {}", plugin.author),
                muted_text.gamma_multiply(0.6),
            )
        };
        ui.painter().text(
            egui::pos2(rect.max.x - 20.0, rect.center().y + 10.0),
            egui::Align2::RIGHT_CENTER,
            &status_label,
            typography::monospace(typography::SM),
            status_label_color,
        );

        response
    }
}

impl Default for PluginsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plugins() -> Vec<PluginDisplayInfo> {
        vec![
            PluginDisplayInfo {
                name: "tokyo-night".into(),
                version: "1.0.0".into(),
                description: "Tokyo Night color theme".into(),
                enabled: true,
                source: PluginSource::Lua,
                command_count: 0,
                keybinding_count: 0,
            },
            PluginDisplayInfo {
                name: "vim-mode".into(),
                version: "2.0.0".into(),
                description: "Vim keybindings".into(),
                enabled: true,
                source: PluginSource::Lua,
                command_count: 5,
                keybinding_count: 20,
            },
            PluginDisplayInfo {
                name: "disabled-plugin".into(),
                version: "1.0.0".into(),
                description: "A disabled plugin".into(),
                enabled: false,
                source: PluginSource::Config,
                command_count: 0,
                keybinding_count: 0,
            },
            PluginDisplayInfo {
                name: "metrics-helper".into(),
                version: "0.5.0".into(),
                description: "Helper for metrics".into(),
                enabled: false,
                source: PluginSource::Lua,
                command_count: 2,
                keybinding_count: 1,
            },
        ]
    }

    fn sample_available_plugins() -> Vec<CommunityPluginInfo> {
        vec![
            CommunityPluginInfo {
                name: "tokyo-night".into(),
                version: "1.1.0".into(), // Newer version available
                description: "Tokyo Night color theme".into(),
                author: "author1".into(),
                file: "tokyo-night.lua".into(),
                installed: false,
            },
            CommunityPluginInfo {
                name: "gruvbox".into(),
                version: "1.0.0".into(),
                description: "Gruvbox color theme".into(),
                author: "author2".into(),
                file: "gruvbox.lua".into(),
                installed: false,
            },
            CommunityPluginInfo {
                name: "catppuccin".into(),
                version: "2.0.0".into(),
                description: "Catppuccin theme".into(),
                author: "author3".into(),
                file: "catppuccin.lua".into(),
                installed: false,
            },
        ]
    }

    #[test]
    fn test_plugins_overlay_default_closed() {
        let overlay = PluginsOverlay::new();
        assert!(!overlay.is_open());
    }

    #[test]
    fn test_plugins_overlay_open_close() {
        let mut overlay = PluginsOverlay::new();
        overlay.open();
        assert!(overlay.is_open());

        overlay.close();
        assert!(!overlay.is_open());
    }

    #[test]
    fn test_filtered_plugins_no_filter() {
        let mut overlay = PluginsOverlay::new();
        overlay.set_plugins(sample_plugins());

        let count = overlay.filtered_plugins(PluginTab::Installed).count();
        // All 4 plugins (sorted by enabled, then source, then name)
        assert_eq!(count, 4);
    }

    #[test]
    fn test_filtered_plugins_enabled_only() {
        let mut overlay = PluginsOverlay::new();
        overlay.set_plugins(sample_plugins());
        overlay.show_enabled_only = true;

        let count = overlay.filtered_plugins(PluginTab::Installed).count();
        // Only enabled plugins
        assert_eq!(count, 2);

        let names: Vec<_> = overlay
            .filtered_plugins(PluginTab::Installed)
            .map(|p| &p.name)
            .collect();
        assert!(names.contains(&&"tokyo-night".to_string()));
        assert!(names.contains(&&"vim-mode".to_string()));
    }

    #[test]
    fn test_filtered_plugins_search() {
        let mut overlay = PluginsOverlay::new();
        overlay.set_plugins(sample_plugins());
        overlay.search_filter = "vim".into();

        let filtered: Vec<_> = overlay.filtered_plugins(PluginTab::Installed).collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "vim-mode");
    }

    #[test]
    fn test_filtered_plugins_search_by_description() {
        let mut overlay = PluginsOverlay::new();
        overlay.set_plugins(sample_plugins());
        overlay.search_filter = "keybindings".into();

        let filtered: Vec<_> = overlay.filtered_plugins(PluginTab::Installed).collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "vim-mode");
    }

    #[test]
    fn test_filtered_plugins_case_insensitive() {
        let mut overlay = PluginsOverlay::new();
        overlay.set_plugins(sample_plugins());
        overlay.search_filter = "TOKYO".into();

        let filtered: Vec<_> = overlay.filtered_plugins(PluginTab::Installed).collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "tokyo-night");
    }

    #[test]
    fn test_filtered_available_plugins() {
        let mut overlay = PluginsOverlay::new();
        overlay.set_available_plugins(sample_available_plugins());
        overlay.search_filter = "gruvbox".into();

        let filtered: Vec<_> = overlay.filtered_available_plugins().collect();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "gruvbox");
    }

    #[test]
    fn test_update_installed_status() {
        let mut overlay = PluginsOverlay::new();
        overlay.set_plugins(sample_plugins()); // tokyo-night is installed
        overlay.set_available_plugins(sample_available_plugins());

        // tokyo-night should be marked as installed in available list
        let tokyo_night = overlay
            .available_plugins
            .iter()
            .find(|p| p.name == "tokyo-night")
            .unwrap();
        assert!(tokyo_night.installed);

        // gruvbox should not be marked as installed
        let gruvbox = overlay
            .available_plugins
            .iter()
            .find(|p| p.name == "gruvbox")
            .unwrap();
        assert!(!gruvbox.installed);
    }

    #[test]
    fn test_filtered_count_installed() {
        let mut overlay = PluginsOverlay::new();
        overlay.set_plugins(sample_plugins());

        assert_eq!(overlay.filtered_count(PluginTab::Installed), 4);

        overlay.show_enabled_only = true;
        assert_eq!(overlay.filtered_count(PluginTab::Installed), 2);
    }

    #[test]
    fn test_filtered_count_available() {
        let mut overlay = PluginsOverlay::new();
        overlay.set_available_plugins(sample_available_plugins());

        assert_eq!(overlay.filtered_count(PluginTab::Available), 3);

        overlay.search_filter = "cat".into();
        assert_eq!(overlay.filtered_count(PluginTab::Available), 1);
    }

    #[test]
    fn test_plugins_sorted_by_enabled_then_source() {
        let mut overlay = PluginsOverlay::new();
        let plugins = vec![
            PluginDisplayInfo {
                name: "z-disabled-lua".into(),
                version: "1.0.0".into(),
                description: "".into(),
                enabled: false,
                source: PluginSource::Lua,
                command_count: 0,
                keybinding_count: 0,
            },
            PluginDisplayInfo {
                name: "a-enabled-config".into(),
                version: "1.0.0".into(),
                description: "".into(),
                enabled: true,
                source: PluginSource::Config,
                command_count: 0,
                keybinding_count: 0,
            },
            PluginDisplayInfo {
                name: "b-enabled-lua".into(),
                version: "1.0.0".into(),
                description: "".into(),
                enabled: true,
                source: PluginSource::Lua,
                command_count: 0,
                keybinding_count: 0,
            },
        ];
        overlay.set_plugins(plugins);

        let names: Vec<_> = overlay
            .filtered_plugins(PluginTab::Installed)
            .map(|p| p.name.as_str())
            .collect();

        // Order: enabled Lua first, then enabled Config, then disabled
        assert_eq!(
            names,
            vec!["b-enabled-lua", "a-enabled-config", "z-disabled-lua"]
        );
    }

    #[test]
    fn test_plugin_source_display() {
        assert_eq!(PluginSource::Config.display_name(), "config");
        assert_eq!(PluginSource::Lua.display_name(), "lua");
    }

    #[test]
    fn test_plugins_overlay_result_variants() {
        // Test that all variants are distinct
        assert_ne!(PluginsOverlayResult::None, PluginsOverlayResult::Closed);
        assert_ne!(
            PluginsOverlayResult::TogglePlugin("test".into()),
            PluginsOverlayResult::None
        );
        assert_eq!(
            PluginsOverlayResult::InstallPlugin("a".into(), "b".into()),
            PluginsOverlayResult::InstallPlugin("a".into(), "b".into())
        );
    }

    #[test]
    fn test_set_installing_plugin() {
        let mut overlay = PluginsOverlay::new();
        assert!(overlay.installing_plugin.is_none());

        overlay.set_installing_plugin(Some("test-plugin".into()));
        assert_eq!(overlay.installing_plugin, Some("test-plugin".into()));

        overlay.set_installing_plugin(None);
        assert!(overlay.installing_plugin.is_none());
    }

    #[test]
    fn test_loading_available_state() {
        let mut overlay = PluginsOverlay::new();
        assert!(!overlay.loading_available);

        overlay.set_loading_available(true);
        assert!(overlay.loading_available);

        overlay.set_loading_available(false);
        assert!(!overlay.loading_available);
    }
}
