// Ref: https://github.com/rerun-io/rerun/blob/5949a229032660911b0c49f67c002dde23a714f4/crates/viewer/re_ui/src/command.rs#L490

use egui::{Key, KeyboardShortcut, Modifiers, os::OperatingSystem};

use crate::ui::{colors::text_color, icons::EXTERNAL_LINK, theme::AppTheme};

/// Interface for sending [`UICommand`] messages.
pub trait UICommandSender {
    fn send_ui(&self, command: UICommand);
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum UICommand {
    Home,
    Dashboard,
    OpenExampleDashboard(usize),
    Help,
    ConnectionStatus(bool),
    /// Set a specific theme
    Theme(AppTheme),
    /// Cycle to the next theme
    NextTheme,
    OpenFuzzyFinder,
    OpenCommandPalette,
    /// Show a notification to the user (from plugins)
    Notify {
        level: String,
        message: String,
    },
    /// Request a UI repaint (from plugins)
    Repaint,

    // ==================== Plugin Pane Commands ====================
    /// Add a query pane from a plugin
    PluginAddQueryPane {
        query: String,
        title: Option<String>,
    },
    /// Add a logs pane from a plugin
    PluginAddLogsPane,
    /// Add a tracing pane from a plugin
    PluginAddTracingPane {
        trace_id: Option<String>,
    },
    /// Add a terminal pane from a plugin
    PluginAddTerminalPane,
    /// Add a SQL pane from a plugin
    PluginAddSqlPane,
    /// Close the focused pane from a plugin
    PluginCloseFocusedPane,
    /// Focus pane in a direction from a plugin
    PluginFocusPane {
        direction: String,
    },

    // ==================== Plugin Time Range Commands ====================
    /// Set time range preset from a plugin (e.g., "5m", "1h", "24h")
    PluginSetTimeRangePreset {
        preset: String,
    },
    /// Set absolute time range from a plugin (milliseconds since Unix epoch)
    PluginSetTimeRangeAbsolute {
        start_ms: i64,
        end_ms: i64,
    },

    // ==================== Plugin Custom Pane Commands ====================
    /// Register a custom table pane type from a plugin
    PluginRegisterCustomTablePane {
        config: enya_plugin::CustomTableConfig,
    },
    /// Add an instance of a custom table pane
    PluginAddCustomTablePane {
        pane_type: String,
    },
    /// Update data for a custom table pane by ID
    PluginUpdateCustomTableData {
        pane_id: usize,
        data: enya_plugin::CustomTableData,
    },
    /// Update data for all custom table panes of a type
    PluginUpdateCustomTableDataByType {
        pane_type: String,
        data: enya_plugin::CustomTableData,
    },

    // ==================== Plugin Custom Chart Pane Commands ====================
    /// Register a custom chart pane type from a plugin
    PluginRegisterCustomChartPane {
        config: enya_plugin::CustomChartConfig,
    },
    /// Add an instance of a custom chart pane
    PluginAddCustomChartPane {
        pane_type: String,
    },
    /// Update data for all custom chart panes of a type
    /// Note: timestamps stored as milliseconds (i64), values scaled by 1_000_000 (i64)
    PluginUpdateCustomChartDataByType {
        pane_type: String,
        /// Series data in hashable form: Vec<(name, tags, points)>
        /// where points are Vec<(timestamp_ms, value_scaled)>
        series: Vec<ChartSeriesHashable>,
        error: Option<String>,
    },

    // ==================== Plugin Custom Stat Pane Commands ====================
    /// Register a custom stat pane type from a plugin
    PluginRegisterCustomStatPane {
        config: enya_plugin::StatPaneConfig,
    },
    /// Add an instance of a custom stat pane
    PluginAddCustomStatPane {
        pane_type: String,
    },
    /// Update data for all custom stat panes of a type
    PluginUpdateCustomStatDataByType {
        pane_type: String,
        data: StatDataHashable,
    },

    // ==================== Plugin Custom Gauge Pane Commands ====================
    /// Register a custom gauge pane type from a plugin
    PluginRegisterCustomGaugePane {
        config: enya_plugin::GaugePaneConfig,
    },
    /// Add an instance of a custom gauge pane
    PluginAddCustomGaugePane {
        pane_type: String,
    },
    /// Update data for all custom gauge panes of a type
    PluginUpdateCustomGaugeDataByType {
        pane_type: String,
        data: GaugeDataHashable,
    },
}

/// Hashable representation of a chart series for UICommand
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChartSeriesHashable {
    pub name: String,
    pub tags: std::collections::BTreeMap<String, String>,
    /// Points as (timestamp_ms, value * 1_000_000)
    pub points: Vec<(i64, i64)>,
}

impl ChartSeriesHashable {
    /// Convert from plugin ChartSeries to hashable form
    pub fn from_plugin(series: &enya_plugin::ChartSeries) -> Self {
        Self {
            name: series.name.clone(),
            tags: series.tags.clone(),
            points: series
                .points
                .iter()
                .map(|p| {
                    let timestamp_ms = (p.timestamp * 1000.0) as i64;
                    let value_scaled = (p.value * 1_000_000.0) as i64;
                    (timestamp_ms, value_scaled)
                })
                .collect(),
        }
    }

    /// Convert back to plugin ChartSeries
    pub fn to_plugin(&self) -> enya_plugin::ChartSeries {
        let mut series = enya_plugin::ChartSeries::new(&self.name);
        series.tags = self.tags.clone();
        series.points = self
            .points
            .iter()
            .map(|(ts_ms, val_scaled)| {
                enya_plugin::ChartDataPoint::new(
                    *ts_ms as f64 / 1000.0,
                    *val_scaled as f64 / 1_000_000.0,
                )
            })
            .collect();
        series
    }
}

/// Hashable representation of a threshold for UICommand
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThresholdHashable {
    /// Value scaled by 1_000_000
    pub value_scaled: i64,
    pub color: String,
    pub label: Option<String>,
}

impl ThresholdHashable {
    /// Convert from plugin ThresholdConfig to hashable form
    pub fn from_plugin(thresh: &enya_plugin::ThresholdConfig) -> Self {
        Self {
            value_scaled: (thresh.value * 1_000_000.0) as i64,
            color: thresh.color.clone(),
            label: thresh.label.clone(),
        }
    }

    /// Convert back to plugin ThresholdConfig
    pub fn to_plugin(&self) -> enya_plugin::ThresholdConfig {
        let mut thresh =
            enya_plugin::ThresholdConfig::new(self.value_scaled as f64 / 1_000_000.0, &self.color);
        if let Some(ref lbl) = self.label {
            thresh = thresh.with_label(lbl.clone());
        }
        thresh
    }
}

/// Hashable representation of stat pane data for UICommand
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StatDataHashable {
    /// Value scaled by 1_000_000
    pub value_scaled: i64,
    /// Sparkline values scaled by 1_000_000
    pub sparkline: Vec<i64>,
    /// Change value scaled by 1_000_000
    pub change_value_scaled: Option<i64>,
    pub change_period: Option<String>,
    pub thresholds: Vec<ThresholdHashable>,
    pub error: Option<String>,
}

impl StatDataHashable {
    /// Convert from plugin StatPaneData to hashable form
    pub fn from_plugin(data: &enya_plugin::StatPaneData) -> Self {
        Self {
            value_scaled: (data.value * 1_000_000.0) as i64,
            sparkline: data
                .sparkline
                .iter()
                .map(|v| (*v * 1_000_000.0) as i64)
                .collect(),
            change_value_scaled: data.change_value.map(|v| (v * 1_000_000.0) as i64),
            change_period: data.change_period.clone(),
            thresholds: data
                .thresholds
                .iter()
                .map(ThresholdHashable::from_plugin)
                .collect(),
            error: data.error.clone(),
        }
    }

    /// Convert back to plugin StatPaneData
    pub fn to_plugin(&self) -> enya_plugin::StatPaneData {
        if let Some(ref err) = self.error {
            return enya_plugin::StatPaneData::with_error(err.clone());
        }

        let mut data =
            enya_plugin::StatPaneData::with_value(self.value_scaled as f64 / 1_000_000.0);
        data.sparkline = self
            .sparkline
            .iter()
            .map(|v| *v as f64 / 1_000_000.0)
            .collect();
        data.change_value = self.change_value_scaled.map(|v| v as f64 / 1_000_000.0);
        data.change_period = self.change_period.clone();
        data.thresholds = self.thresholds.iter().map(|t| t.to_plugin()).collect();
        data
    }
}

/// Hashable representation of gauge pane data for UICommand
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GaugeDataHashable {
    /// Value scaled by 1_000_000
    pub value_scaled: i64,
    pub thresholds: Vec<ThresholdHashable>,
    pub error: Option<String>,
}

impl GaugeDataHashable {
    /// Convert from plugin GaugePaneData to hashable form
    pub fn from_plugin(data: &enya_plugin::GaugePaneData) -> Self {
        Self {
            value_scaled: (data.value * 1_000_000.0) as i64,
            thresholds: data
                .thresholds
                .iter()
                .map(ThresholdHashable::from_plugin)
                .collect(),
            error: data.error.clone(),
        }
    }

    /// Convert back to plugin GaugePaneData
    pub fn to_plugin(&self) -> enya_plugin::GaugePaneData {
        if let Some(ref err) = self.error {
            return enya_plugin::GaugePaneData::with_error(err.clone());
        }

        let mut data =
            enya_plugin::GaugePaneData::with_value(self.value_scaled as f64 / 1_000_000.0);
        data.thresholds = self.thresholds.iter().map(|t| t.to_plugin()).collect();
        data
    }
}

impl UICommand {
    /// Returns all command variants (for iteration).
    /// Note: OpenExampleDashboard uses index 0 as placeholder.
    /// Plugin commands are not included here as they are programmatic only.
    fn all() -> impl Iterator<Item = Self> {
        [
            Self::Home,
            Self::Dashboard,
            Self::OpenExampleDashboard(0),
            Self::Help,
            Self::ConnectionStatus(false),
            Self::Theme(AppTheme::default()),
            Self::NextTheme,
            Self::OpenFuzzyFinder,
            Self::OpenCommandPalette,
            Self::Notify {
                level: String::new(),
                message: String::new(),
            },
            Self::Repaint,
        ]
        .into_iter()
    }

    pub fn text(&self) -> &'static str {
        self.text_and_tooltip().0
    }

    pub fn tooltip(&self) -> &'static str {
        self.text_and_tooltip().1
    }

    pub fn text_and_tooltip(&self) -> (&'static str, &'static str) {
        match self {
            Self::Home => ("Home", "Open Welcome Screen"),
            Self::Help => ("Help", "Get help with any Playground issues"),
            Self::Dashboard => ("Dashboard", "Open Enya Dashboard"),
            Self::OpenExampleDashboard(_) => ("...", "Create an Enya dashboard"),
            Self::Theme(_) => ("...", "..."),
            Self::NextTheme => ("Next Theme", "Cycle to the next theme"),
            Self::ConnectionStatus(_) => ("", ""),
            Self::OpenFuzzyFinder => ("Search...", "Open fuzzy finder to search metrics"),
            Self::OpenCommandPalette => ("Command Palette", "Open command palette"),
            Self::Notify { .. } => ("", ""),
            Self::Repaint => ("", ""),
            // Plugin commands (programmatic only)
            Self::PluginAddQueryPane { .. } => ("", ""),
            Self::PluginAddLogsPane => ("", ""),
            Self::PluginAddTracingPane { .. } => ("", ""),
            Self::PluginAddTerminalPane => ("", ""),
            Self::PluginAddSqlPane => ("", ""),
            Self::PluginCloseFocusedPane => ("", ""),
            Self::PluginFocusPane { .. } => ("", ""),
            Self::PluginSetTimeRangePreset { .. } => ("", ""),
            Self::PluginSetTimeRangeAbsolute { .. } => ("", ""),
            // Custom table pane commands (programmatic only)
            Self::PluginRegisterCustomTablePane { .. } => ("", ""),
            Self::PluginAddCustomTablePane { .. } => ("", ""),
            Self::PluginUpdateCustomTableData { .. } => ("", ""),
            Self::PluginUpdateCustomTableDataByType { .. } => ("", ""),
            // Custom chart pane commands (programmatic only)
            Self::PluginRegisterCustomChartPane { .. } => ("", ""),
            Self::PluginAddCustomChartPane { .. } => ("", ""),
            Self::PluginUpdateCustomChartDataByType { .. } => ("", ""),
            // Custom stat pane commands (programmatic only)
            Self::PluginRegisterCustomStatPane { .. } => ("", ""),
            Self::PluginAddCustomStatPane { .. } => ("", ""),
            Self::PluginUpdateCustomStatDataByType { .. } => ("", ""),
            // Custom gauge pane commands (programmatic only)
            Self::PluginRegisterCustomGaugePane { .. } => ("", ""),
            Self::PluginAddCustomGaugePane { .. } => ("", ""),
            Self::PluginUpdateCustomGaugeDataByType { .. } => ("", ""),
        }
    }

    /// Show this command as a menu-button.
    ///
    /// If clicked, enqueue the command.
    pub fn menu_button_ui(
        self,
        ui: &mut egui::Ui,
        theme: AppTheme,
        command_sender: &impl UICommandSender,
    ) -> egui::Response {
        let button = self.menu_button(ui.ctx(), theme);
        let mut response = ui.add(button).on_hover_text(self.tooltip());

        if self.is_link() {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }

        if response.clicked() {
            command_sender.send_ui(self);
            ui.close();
        }

        response
    }

    pub fn is_link(&self) -> bool {
        matches!(self, Self::Help)
    }

    pub fn icon(&self) -> Option<&'static crate::ui::icons::Icon> {
        match self {
            Self::Help => Some(&EXTERNAL_LINK),
            _ => None,
        }
    }

    #[must_use = "Returns the Command that was triggered by some keyboard shortcut"]
    pub fn listen_for_kb_shortcut(egui_ctx: &egui::Context) -> Option<Self> {
        // Handle ':' for command palette BEFORE checking focus.
        // This allows ':' to work even when widgets have focus (e.g., SQL plan viewer),
        // as long as no widget actually consumed the key. The consume_shortcut will
        // fail if a TextEdit or other input widget already processed the keystroke.
        let colon_shortcut = KeyboardShortcut::new(Modifiers::NONE, Key::Colon);
        let command_palette_triggered =
            egui_ctx.input_mut(|input| input.consume_shortcut(&colon_shortcut));
        if command_palette_triggered {
            return Some(Self::OpenCommandPalette);
        }

        let anything_has_focus = egui_ctx.memory(|mem| mem.focused().is_some());
        if anything_has_focus {
            return None; // e.g. we're typing in a TextField
        }

        let mut commands: Vec<(KeyboardShortcut, Self)> = Self::all()
            .flat_map(|cmd| {
                let shortcuts = cmd.kb_shortcuts(egui_ctx.os());
                shortcuts
                    .into_iter()
                    .map(move |kb_shortcut| (kb_shortcut, cmd.clone()))
            })
            .collect();

        // If the user pressed `Cmd-Shift-S` then egui will match that
        // with both `Cmd-Shift-S` and `Cmd-S`.
        // The reason is that `Shift` (and `Alt`) are sometimes required to produce certain keys,
        // such as `+` (`Shift =` on an american keyboard).
        // The result of this is that we must check for `Cmd-Shift-S` before `Cmd-S`, etc.
        // So we order the commands here so that the commands with `Shift` and `Alt` in them
        // are checked first.
        commands.sort_by_key(|(kb_shortcut, _cmd)| {
            let num_shift_alts =
                kb_shortcut.modifiers.shift as i32 + kb_shortcut.modifiers.alt as i32;
            -num_shift_alts // most first
        });

        egui_ctx.input_mut(|input| {
            for (kb_shortcut, command) in commands {
                if input.consume_shortcut(&kb_shortcut) {
                    return Some(command);
                }
            }
            None
        })
    }

    pub fn menu_button(&self, egui_ctx: &egui::Context, theme: AppTheme) -> egui::Button<'static> {
        let mut button = if let Some(icon) = self.icon() {
            egui::Button::image_and_text(icon.as_image().tint(text_color(theme)), self.text())
        } else {
            egui::Button::new(self.text())
        };

        if let Some(shortcut_text) = self.formatted_kb_shortcut(egui_ctx) {
            button = button.shortcut_text(shortcut_text);
        }

        button
    }

    /// All keyboard shortcuts, with the primary first.
    pub fn kb_shortcuts(&self, _os: OperatingSystem) -> Vec<KeyboardShortcut> {
        fn key(key: Key) -> KeyboardShortcut {
            KeyboardShortcut::new(Modifiers::NONE, key)
        }

        match self {
            Self::Home => vec![],
            Self::Help => vec![], // Help accessed via ? on landing page or :help command
            Self::Dashboard => vec![key(Key::D)],
            Self::Theme(_) => vec![],
            Self::NextTheme => vec![], // Use :theme command
            Self::OpenExampleDashboard(_) => vec![],
            Self::ConnectionStatus(_) => vec![],
            Self::OpenFuzzyFinder => vec![], // Space+m leader key sequence
            // ':' key (colon) - no modifiers needed since it's already the shifted key
            Self::OpenCommandPalette => vec![key(Key::Colon)],
            Self::Notify { .. } => vec![], // Programmatic only
            Self::Repaint => vec![],       // Programmatic only
            // Plugin commands (programmatic only)
            Self::PluginAddQueryPane { .. } => vec![],
            Self::PluginAddLogsPane => vec![],
            Self::PluginAddTracingPane { .. } => vec![],
            Self::PluginAddTerminalPane => vec![],
            Self::PluginAddSqlPane => vec![],
            Self::PluginCloseFocusedPane => vec![],
            Self::PluginFocusPane { .. } => vec![],
            Self::PluginSetTimeRangePreset { .. } => vec![],
            Self::PluginSetTimeRangeAbsolute { .. } => vec![],
            // Custom table pane commands (programmatic only)
            Self::PluginRegisterCustomTablePane { .. } => vec![],
            Self::PluginAddCustomTablePane { .. } => vec![],
            Self::PluginUpdateCustomTableData { .. } => vec![],
            Self::PluginUpdateCustomTableDataByType { .. } => vec![],
            // Custom chart pane commands (programmatic only)
            Self::PluginRegisterCustomChartPane { .. } => vec![],
            Self::PluginAddCustomChartPane { .. } => vec![],
            Self::PluginUpdateCustomChartDataByType { .. } => vec![],
            // Custom stat pane commands (programmatic only)
            Self::PluginRegisterCustomStatPane { .. } => vec![],
            Self::PluginAddCustomStatPane { .. } => vec![],
            Self::PluginUpdateCustomStatDataByType { .. } => vec![],
            // Custom gauge pane commands (programmatic only)
            Self::PluginRegisterCustomGaugePane { .. } => vec![],
            Self::PluginAddCustomGaugePane { .. } => vec![],
            Self::PluginUpdateCustomGaugeDataByType { .. } => vec![],
        }
    }

    /// Primary keyboard shortcut
    fn primary_kb_shortcut(&self, os: OperatingSystem) -> Option<KeyboardShortcut> {
        self.kb_shortcuts(os).first().copied()
    }

    /// Return the keyboard shortcut for this command, nicely formatted
    pub fn formatted_kb_shortcut(&self, egui_ctx: &egui::Context) -> Option<String> {
        // Note: we only show the primary shortcut to the user.
        // The fallbacks are there for people who have muscle memory for the other shortcuts.
        self.primary_kb_shortcut(egui_ctx.os())
            .map(|shortcut| egui_ctx.format_shortcut(&shortcut))
    }

    /// Add e.g. " (Ctrl+F11)" as a suffix
    pub fn format_shortcut_tooltip_suffix(&self, egui_ctx: &egui::Context) -> String {
        if let Some(shortcut_text) = self.formatted_kb_shortcut(egui_ctx) {
            format!(" ({shortcut_text})")
        } else {
            Default::default()
        }
    }
    pub fn tooltip_with_shortcut(&self, egui_ctx: &egui::Context) -> String {
        format!(
            "{}{}",
            self.tooltip(),
            self.format_shortcut_tooltip_suffix(egui_ctx)
        )
    }
}

/// Sender that queues up the execution of commands.
#[derive(Clone)]
pub struct CommandSender {
    //system_sender: std::sync::mpsc::Sender<SystemCommand>,
    ui_sender: std::sync::mpsc::Sender<UICommand>,
}

// Creates a new command channel.
pub fn command_channel() -> (CommandSender, CommandReceiver) {
    let (ui_sender, ui_receiver) = std::sync::mpsc::channel();
    (CommandSender { ui_sender }, CommandReceiver { ui_receiver })
}

/// Receiver for the [`CommandSender`]
pub struct CommandReceiver {
    ui_receiver: std::sync::mpsc::Receiver<UICommand>,
}

impl CommandReceiver {
    /// Receive a [`UICommand`] to be executed if any is queued.
    pub fn recv_ui(&self) -> Option<UICommand> {
        // The only way this can fail (other than being empty)
        // is if the sender has been dropped.
        self.ui_receiver.try_recv().ok()
    }
}

impl UICommandSender for CommandSender {
    /// Send a command to be executed.
    fn send_ui(&self, command: UICommand) {
        // The only way this can fail is if the receiver has been dropped.
        self.ui_sender.send(command).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== UICommand Tests ====================

    #[test]
    fn test_ui_command_all_returns_all_variants() {
        let commands: Vec<UICommand> = UICommand::all().collect();
        assert_eq!(commands.len(), 11);
        assert!(commands.contains(&UICommand::Home));
        assert!(commands.contains(&UICommand::Dashboard));
        assert!(commands.contains(&UICommand::Help));
        assert!(commands.contains(&UICommand::NextTheme));
        assert!(commands.contains(&UICommand::OpenFuzzyFinder));
        assert!(commands.contains(&UICommand::OpenCommandPalette));
        assert!(commands.contains(&UICommand::Repaint));
    }

    #[test]
    fn test_ui_command_text_returns_expected_values() {
        assert_eq!(UICommand::Home.text(), "Home");
        assert_eq!(UICommand::Help.text(), "Help");
        assert_eq!(UICommand::Dashboard.text(), "Dashboard");
        assert_eq!(UICommand::NextTheme.text(), "Next Theme");
        assert_eq!(UICommand::OpenFuzzyFinder.text(), "Search...");
        assert_eq!(UICommand::OpenCommandPalette.text(), "Command Palette");
    }

    #[test]
    fn test_ui_command_tooltip_returns_expected_values() {
        assert_eq!(UICommand::Home.tooltip(), "Open Welcome Screen");
        assert_eq!(
            UICommand::Help.tooltip(),
            "Get help with any Playground issues"
        );
        assert_eq!(UICommand::Dashboard.tooltip(), "Open Enya Dashboard");
        assert_eq!(UICommand::NextTheme.tooltip(), "Cycle to the next theme");
        assert_eq!(
            UICommand::OpenFuzzyFinder.tooltip(),
            "Open fuzzy finder to search metrics"
        );
        assert_eq!(
            UICommand::OpenCommandPalette.tooltip(),
            "Open command palette"
        );
    }

    #[test]
    fn test_ui_command_text_and_tooltip_consistency() {
        for cmd in UICommand::all() {
            let (text, tooltip) = cmd.text_and_tooltip();
            assert_eq!(cmd.text(), text);
            assert_eq!(cmd.tooltip(), tooltip);
        }
    }

    #[test]
    fn test_ui_command_is_link() {
        assert!(UICommand::Help.is_link());
        assert!(!UICommand::Home.is_link());
        assert!(!UICommand::Dashboard.is_link());
        assert!(!UICommand::NextTheme.is_link());
        assert!(!UICommand::OpenFuzzyFinder.is_link());
        assert!(!UICommand::OpenCommandPalette.is_link());
    }

    #[test]
    fn test_ui_command_icon() {
        assert!(UICommand::Help.icon().is_some());
        assert!(UICommand::Home.icon().is_none());
        assert!(UICommand::Dashboard.icon().is_none());
        assert!(UICommand::NextTheme.icon().is_none());
        assert!(UICommand::OpenFuzzyFinder.icon().is_none());
        assert!(UICommand::OpenCommandPalette.icon().is_none());
    }

    #[test]
    fn test_ui_command_kb_shortcuts_dashboard() {
        let shortcuts = UICommand::Dashboard.kb_shortcuts(OperatingSystem::Mac);
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].logical_key, Key::D);
        assert_eq!(shortcuts[0].modifiers, Modifiers::NONE);
    }

    #[test]
    fn test_ui_command_kb_shortcuts_command_palette() {
        let shortcuts = UICommand::OpenCommandPalette.kb_shortcuts(OperatingSystem::Mac);
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].logical_key, Key::Colon);
        assert_eq!(shortcuts[0].modifiers, Modifiers::NONE);
    }

    #[test]
    fn test_ui_command_kb_shortcuts_empty_for_some_commands() {
        assert!(
            UICommand::Home
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::Help
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::NextTheme
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::OpenFuzzyFinder
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::Theme(AppTheme::Dark)
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::OpenExampleDashboard(0)
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
        assert!(
            UICommand::ConnectionStatus(false)
                .kb_shortcuts(OperatingSystem::Mac)
                .is_empty()
        );
    }

    #[test]
    fn test_ui_command_kb_shortcuts_consistent_across_os() {
        let mac_shortcuts = UICommand::Dashboard.kb_shortcuts(OperatingSystem::Mac);
        let windows_shortcuts = UICommand::Dashboard.kb_shortcuts(OperatingSystem::Windows);
        let linux_shortcuts = UICommand::Dashboard.kb_shortcuts(OperatingSystem::Nix);

        assert_eq!(mac_shortcuts, windows_shortcuts);
        assert_eq!(mac_shortcuts, linux_shortcuts);
    }

    #[test]
    fn test_ui_command_primary_kb_shortcut() {
        assert!(
            UICommand::Dashboard
                .primary_kb_shortcut(OperatingSystem::Mac)
                .is_some()
        );
        assert!(
            UICommand::OpenCommandPalette
                .primary_kb_shortcut(OperatingSystem::Mac)
                .is_some()
        );
        assert!(
            UICommand::Home
                .primary_kb_shortcut(OperatingSystem::Mac)
                .is_none()
        );
        assert!(
            UICommand::Help
                .primary_kb_shortcut(OperatingSystem::Mac)
                .is_none()
        );
    }

    #[test]
    fn test_ui_command_equality() {
        assert_eq!(UICommand::Home, UICommand::Home);
        assert_eq!(UICommand::Help, UICommand::Help);
        assert_eq!(UICommand::Dashboard, UICommand::Dashboard);
        assert_ne!(UICommand::Home, UICommand::Help);
        assert_ne!(UICommand::Dashboard, UICommand::Home);
    }

    #[test]
    fn test_ui_command_theme_variants() {
        let dark = UICommand::Theme(AppTheme::Dark);
        let light = UICommand::Theme(AppTheme::Light);
        assert_ne!(dark, light);
        assert_eq!(dark.text(), "...");
        assert_eq!(light.text(), "...");
    }

    #[test]
    fn test_ui_command_open_example_dashboard_with_index() {
        let cmd0 = UICommand::OpenExampleDashboard(0);
        let cmd1 = UICommand::OpenExampleDashboard(1);
        let cmd5 = UICommand::OpenExampleDashboard(5);
        assert_ne!(cmd0, cmd1);
        assert_ne!(cmd1, cmd5);
        assert_eq!(cmd0.text(), "...");
        assert_eq!(cmd1.text(), "...");
    }

    #[test]
    fn test_ui_command_connection_status_variants() {
        let connected = UICommand::ConnectionStatus(true);
        let disconnected = UICommand::ConnectionStatus(false);
        assert_ne!(connected, disconnected);
        assert_eq!(connected.text(), "");
        assert_eq!(disconnected.text(), "");
    }

    #[test]
    fn test_ui_command_clone() {
        let cmd = UICommand::Dashboard;
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_ui_command_clone_with_data() {
        let cmd = UICommand::Notify {
            level: "info".to_string(),
            message: "test".to_string(),
        };
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    #[test]
    fn test_ui_command_debug() {
        let debug_str = format!("{:?}", UICommand::Home);
        assert!(debug_str.contains("Home"));
    }

    #[test]
    fn test_ui_command_hash() {
        use rustc_hash::FxHashSet;
        let mut set = FxHashSet::default();
        set.insert(UICommand::Home);
        set.insert(UICommand::Dashboard);
        set.insert(UICommand::Home); // duplicate
        assert_eq!(set.len(), 2);
    }

    // ==================== CommandSender/Receiver Tests ====================

    #[test]
    fn test_command_channel_creation() {
        let (sender, receiver) = command_channel();
        // Channel should be created successfully
        drop(sender);
        drop(receiver);
    }

    #[test]
    fn test_command_channel_send_and_receive() {
        let (sender, receiver) = command_channel();
        sender.send_ui(UICommand::Home);
        let received = receiver.recv_ui();
        assert_eq!(received, Some(UICommand::Home));
    }

    #[test]
    fn test_command_channel_multiple_messages() {
        let (sender, receiver) = command_channel();
        sender.send_ui(UICommand::Home);
        sender.send_ui(UICommand::Dashboard);
        sender.send_ui(UICommand::Help);

        assert_eq!(receiver.recv_ui(), Some(UICommand::Home));
        assert_eq!(receiver.recv_ui(), Some(UICommand::Dashboard));
        assert_eq!(receiver.recv_ui(), Some(UICommand::Help));
    }

    #[test]
    fn test_command_channel_empty_returns_none() {
        let (_sender, receiver) = command_channel();
        assert_eq!(receiver.recv_ui(), None);
    }

    #[test]
    fn test_command_channel_recv_after_all_consumed() {
        let (sender, receiver) = command_channel();
        sender.send_ui(UICommand::Home);
        let _ = receiver.recv_ui();
        assert_eq!(receiver.recv_ui(), None);
    }

    #[test]
    fn test_command_sender_clone() {
        let (sender, receiver) = command_channel();
        let sender_clone = sender.clone();

        sender.send_ui(UICommand::Home);
        sender_clone.send_ui(UICommand::Dashboard);

        assert_eq!(receiver.recv_ui(), Some(UICommand::Home));
        assert_eq!(receiver.recv_ui(), Some(UICommand::Dashboard));
    }

    #[test]
    fn test_command_sender_dropped_receiver_does_not_panic() {
        let (sender, receiver) = command_channel();
        drop(receiver);
        // Should not panic, just silently fail
        sender.send_ui(UICommand::Home);
    }

    #[test]
    fn test_command_receiver_dropped_sender_returns_none() {
        let (sender, receiver) = command_channel();
        drop(sender);
        assert_eq!(receiver.recv_ui(), None);
    }

    #[test]
    fn test_ui_command_sender_trait() {
        struct MockSender {
            commands: std::cell::RefCell<Vec<UICommand>>,
        }

        impl UICommandSender for MockSender {
            fn send_ui(&self, command: UICommand) {
                self.commands.borrow_mut().push(command);
            }
        }

        let mock = MockSender {
            commands: std::cell::RefCell::new(Vec::new()),
        };
        mock.send_ui(UICommand::Home);
        mock.send_ui(UICommand::Dashboard);

        let commands = mock.commands.borrow();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], UICommand::Home);
        assert_eq!(commands[1], UICommand::Dashboard);
    }

    #[test]
    fn test_command_channel_fifo_order() {
        let (sender, receiver) = command_channel();

        // Send in specific order
        sender.send_ui(UICommand::OpenExampleDashboard(1));
        sender.send_ui(UICommand::OpenExampleDashboard(2));
        sender.send_ui(UICommand::OpenExampleDashboard(3));

        // Receive in same order (FIFO)
        assert_eq!(receiver.recv_ui(), Some(UICommand::OpenExampleDashboard(1)));
        assert_eq!(receiver.recv_ui(), Some(UICommand::OpenExampleDashboard(2)));
        assert_eq!(receiver.recv_ui(), Some(UICommand::OpenExampleDashboard(3)));
    }
}
