//! Application state types.
//!
//! This module contains the serializable `AppState` struct and related types
//! including `UIState` for tracking the current view and `EditorMetrics`
//! for frame time tracking.

use egui::Visuals;

use crate::ui::settings_screen::AppSettings;
use crate::ui::theme::AppTheme;
use crate::util::Instant;

/// Tracks internal editor metrics for the status line sparkline
pub(super) struct EditorMetrics {
    /// Recent frame times in milliseconds
    frame_times: std::collections::VecDeque<f64>,
    /// Last frame timestamp
    last_frame: Option<Instant>,
}

impl Default for EditorMetrics {
    fn default() -> Self {
        Self {
            frame_times: std::collections::VecDeque::with_capacity(15),
            last_frame: None,
        }
    }
}

impl EditorMetrics {
    /// Record a new frame and return the frame time in ms
    pub fn record_frame(&mut self) -> f64 {
        let now = Instant::now();
        let frame_time = if let Some(last) = self.last_frame {
            now.duration_since(last).as_secs_f64() * 1000.0
        } else {
            16.67 // Default ~60fps assumption for first frame
        };
        self.last_frame = Some(now);

        // Keep last 15 frame times
        if self.frame_times.len() >= 15 {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(frame_time);

        frame_time
    }

    /// Get the frame times for sparkline display
    pub fn frame_times(&self) -> Vec<f64> {
        self.frame_times.iter().copied().collect()
    }

    /// Get current FPS (based on recent frame time)
    pub fn fps(&self) -> f64 {
        if let Some(&last_time) = self.frame_times.back() {
            if last_time > 0.0 {
                return 1000.0 / last_time;
            }
        }
        60.0
    }
}

/// Serializable state that can be persisted
#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppState {
    pub(crate) settings: AppSettings,
    /// Current active builtin theme
    pub(crate) theme: AppTheme,
    /// Custom theme name (from plugins). When Some, overrides the builtin theme.
    pub(crate) custom_theme: Option<String>,
    pub(crate) ui_state: UIState,
}

impl AppState {
    /// Returns the current App theme visuals
    pub fn visuals(&self) -> Visuals {
        self.theme.visuals()
    }

    /// Returns the current UIState
    pub fn ui_state(&self) -> &UIState {
        &self.ui_state
    }

    /// Returns the current builtin theme.
    pub fn builtin_theme(&self) -> AppTheme {
        self.theme
    }

    /// Returns the custom theme name if one is active.
    pub fn custom_theme(&self) -> Option<&str> {
        self.custom_theme.as_deref()
    }

    /// Returns true if a custom (plugin) theme is active.
    pub fn is_custom_theme(&self) -> bool {
        self.custom_theme.is_some()
    }

    /// Set a builtin theme, clearing any custom theme.
    pub fn set_builtin_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
        self.custom_theme = None;
    }

    /// Set a custom theme by name.
    pub fn set_custom_theme(&mut self, name: String) {
        self.custom_theme = Some(name);
    }
}

/// Which current state the UI is in
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub enum UIState {
    #[default]
    Dashboard,
    Home,
}
