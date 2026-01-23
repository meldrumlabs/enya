//! Plugins overlay component for viewing and managing installed plugins.
//!
//! Similar to LazyVim's plugin manager, this overlay displays all registered
//! plugins with their status, version, and capabilities.

use egui::{Color32, Key, RichText, ScrollArea};

use crate::ui::semantic_icons;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use crate::components::util::finder_utils::OverlayStyle;

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
    /// Closed
    Closed,
}

/// A modal overlay for viewing and managing plugins.
pub struct PluginsOverlay {
    /// Whether the overlay is open
    is_open: bool,
    /// Skip first frame of input (to avoid closing on same key that opened it)
    just_opened: bool,
    /// Current theme (supports Custom variant with plugin colors)
    theme: AppTheme,
    /// Plugins to display
    plugins: Vec<PluginDisplayInfo>,
    /// Currently selected plugin index
    selected_index: usize,
    /// Whether to show only enabled plugins
    show_enabled_only: bool,
}

impl PluginsOverlay {
    pub fn new() -> Self {
        Self {
            is_open: false,
            just_opened: false,
            theme: AppTheme::default(),
            plugins: Vec::new(),
            selected_index: 0,
            show_enabled_only: false,
        }
    }

    /// Set the theme (supports Custom variant with plugin colors).
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the plugins to display.
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
        if self.selected_index >= self.plugins.len() {
            self.selected_index = 0;
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

        let mut result = PluginsOverlayResult::None;

        // Handle keyboard input
        ctx.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, Key::Escape) {
                result = PluginsOverlayResult::Closed;
                return;
            }

            // j/Down - Move down
            if input.consume_key(egui::Modifiers::NONE, Key::J)
                || input.consume_key(egui::Modifiers::NONE, Key::ArrowDown)
            {
                let visible_count = self.visible_plugins().count();
                if visible_count > 0 {
                    self.selected_index = (self.selected_index + 1) % visible_count;
                }
                return;
            }

            // k/Up - Move up
            if input.consume_key(egui::Modifiers::NONE, Key::K)
                || input.consume_key(egui::Modifiers::NONE, Key::ArrowUp)
            {
                let visible_count = self.visible_plugins().count();
                if visible_count > 0 {
                    self.selected_index = if self.selected_index == 0 {
                        visible_count - 1
                    } else {
                        self.selected_index - 1
                    };
                }
                return;
            }

            // f - Toggle filter (show enabled only)
            if input.consume_key(egui::Modifiers::NONE, Key::F) {
                self.show_enabled_only = !self.show_enabled_only;
                self.selected_index = 0;
                return;
            }

            // o - Open plugin directory
            if input.consume_key(egui::Modifiers::NONE, Key::O) {
                result = PluginsOverlayResult::OpenPluginDirectory;
                return;
            }

            // Enter/Space - Toggle selected plugin
            if input.consume_key(egui::Modifiers::NONE, Key::Enter)
                || input.consume_key(egui::Modifiers::NONE, Key::Space)
            {
                if let Some(plugin) = self.visible_plugins().nth(self.selected_index) {
                    result = PluginsOverlayResult::TogglePlugin(plugin.name.clone());
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

                        // Stats on the right
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(16.0);
                            let enabled_count = self.plugins.iter().filter(|p| p.enabled).count();
                            let total_count = self.plugins.len();
                            ui.label(
                                RichText::new(format!("{enabled_count}/{total_count} enabled"))
                                    .color(muted_text)
                                    .font(typography::proportional(typography::MD)),
                            );
                        });
                    });
                    ui.add_space(12.0);

                    // Separator
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, separator_color),
                    );
                    ui.add_space(8.0);

                    // Filter indicator
                    if self.show_enabled_only {
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
                    let list_height = popup_max_height - 160.0;
                    ScrollArea::vertical()
                        .max_height(list_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(popup_width - 16.0);

                            let visible_plugins: Vec<_> = self.visible_plugins().cloned().collect();

                            if visible_plugins.is_empty() {
                                ui.add_space(20.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new(semantic_icons::empty::NO_ITEMS)
                                            .color(muted_text)
                                            .size(32.0),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("No plugins found")
                                            .color(muted_text)
                                            .font(typography::proportional(typography::LG)),
                                    );
                                });
                            } else {
                                for (idx, plugin) in visible_plugins.iter().enumerate() {
                                    let is_selected = idx == self.selected_index;
                                    Self::show_plugin_row(
                                        ui,
                                        plugin,
                                        is_selected,
                                        text_col,
                                        accent_color,
                                        accent_primary,
                                        muted_text,
                                        popup_width - 32.0,
                                    );
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

                    // Footer with keyboard hints
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        self.keyboard_hint(ui, "j/k", "navigate", muted_text, text_col);
                        ui.add_space(16.0);
                        self.keyboard_hint(ui, "f", "filter", muted_text, text_col);
                        ui.add_space(16.0);
                        self.keyboard_hint(ui, "o", "open dir", muted_text, text_col);
                        ui.add_space(16.0);
                        self.keyboard_hint(ui, "Esc", "close", muted_text, text_col);
                    });
                    ui.add_space(12.0);
                });
            });

        result
    }

    /// Get visible plugins based on filter.
    fn visible_plugins(&self) -> impl Iterator<Item = &PluginDisplayInfo> {
        self.plugins.iter().filter(|p| {
            if self.show_enabled_only {
                p.enabled
            } else {
                true
            }
        })
    }

    /// Show a single plugin row.
    #[allow(clippy::too_many_arguments)]
    fn show_plugin_row(
        ui: &mut egui::Ui,
        plugin: &PluginDisplayInfo,
        is_selected: bool,
        text_col: Color32,
        accent_color: Color32,
        accent_primary: Color32,
        muted_text: Color32,
        width: f32,
    ) {
        let row_height = 52.0;
        let (rect, _response) =
            ui.allocate_exact_size(egui::Vec2::new(width, row_height), egui::Sense::hover());

        // Background for selected item
        if is_selected {
            ui.painter()
                .rect_filled(rect, 6.0, accent_color.gamma_multiply(0.12));
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

        // Version badge
        let version_text = format!("v{}", plugin.version);
        ui.painter().text(
            egui::pos2(rect.min.x + 68.0 + 150.0, rect.center().y - 8.0),
            egui::Align2::LEFT_CENTER,
            &version_text,
            typography::monospace(typography::SM),
            muted_text.gamma_multiply(0.7),
        );

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
    }

    /// Show a keyboard hint.
    fn keyboard_hint(
        &self,
        ui: &mut egui::Ui,
        key: &str,
        desc: &str,
        muted_text: Color32,
        text_col: Color32,
    ) {
        ui.label(
            RichText::new(key)
                .color(text_col.gamma_multiply(0.8))
                .font(typography::monospace(typography::SM)),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(desc)
                .color(muted_text)
                .font(typography::proportional(typography::SM)),
        );
    }
}

impl Default for PluginsOverlay {
    fn default() -> Self {
        Self::new()
    }
}
