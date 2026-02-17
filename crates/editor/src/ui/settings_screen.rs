/// Available editor fonts
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EditorFont {
    /// Maple Mono - clean, modern monospace font
    MapleMono,
    /// Departure Mono - distinctive retro-style monospace font
    #[default]
    DepartureMono,
    /// JetBrains Mono - designed by JetBrains for developers
    JetBrainsMono,
    /// Iosevka - narrow, highly customizable monospace font
    Iosevka,
}

impl EditorFont {
    /// Human-readable font name
    pub fn name(&self) -> &'static str {
        match self {
            Self::MapleMono => "Maple Mono",
            Self::DepartureMono => "Departure Mono",
            Self::JetBrainsMono => "JetBrains Mono",
            Self::Iosevka => "Iosevka",
        }
    }

    /// Internal font family name used in egui
    pub fn font_family_name(&self) -> &'static str {
        match self {
            Self::MapleMono => "maple_mono",
            Self::DepartureMono => "departure_mono",
            Self::JetBrainsMono => "jetbrains_mono",
            Self::Iosevka => "iosevka",
        }
    }

    /// Returns all available fonts
    pub fn all() -> &'static [EditorFont] {
        &[
            Self::DepartureMono,
            Self::MapleMono,
            Self::JetBrainsMono,
            Self::Iosevka,
        ]
    }

    /// Description of the font
    pub fn description(&self) -> &'static str {
        match self {
            Self::MapleMono => "Clean, modern monospace with ligatures",
            Self::DepartureMono => "Distinctive retro-style monospace",
            Self::JetBrainsMono => "Developer-focused, great for code",
            Self::Iosevka => "Narrow, highly customizable",
        }
    }
}

use crate::components::util::{AiModel, AiProvider};
use crate::ui::theme::AppTheme;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AppSettings {
    /// API key for backend services (kept for future use)
    #[serde(default)]
    pub api_key: String,
    /// Recent plots that were opened (metric name, timestamp)
    #[serde(default)]
    pub recent_plots: Vec<RecentPlotEntry>,
    /// Recent workspaces that were accessed
    #[serde(default)]
    pub recent_workspaces: Vec<WorkspaceEntry>,
    /// Currently selected editor font
    #[serde(default)]
    pub font: EditorFont,
    /// Current UI theme (user preference, not per-workspace)
    #[serde(default)]
    pub theme: AppTheme,
    /// Selected AI provider
    #[serde(default)]
    pub ai_provider: AiProvider,
    /// Selected AI model (None = use provider default)
    #[serde(default)]
    pub ai_model: Option<AiModel>,
    /// Anthropic API key (for Claude provider)
    #[serde(default)]
    pub anthropic_api_key: String,
    /// OpenAI API key (for Codex provider)
    #[serde(default)]
    pub openai_api_key: String,
    /// Git repository URL for codebase integration
    #[serde(default)]
    pub git_repo_url: String,
    /// Default Prometheus endpoint for new workspaces
    #[serde(default)]
    pub default_prometheus_endpoint: String,
    /// Default Loki endpoint for new workspaces
    #[serde(default)]
    pub default_loki_endpoint: String,
    /// Default Arrow Flight SQL endpoint for new workspaces
    #[serde(default)]
    pub default_flight_sql_endpoint: String,
    /// Version string of the last dismissed update notification
    #[serde(default)]
    pub dismissed_update_version: Option<String>,
    /// GitHub authentication credentials (optional)
    #[serde(default)]
    pub github_credentials: Option<crate::github_auth::GitHubCredentials>,
    /// Default workspace to open on startup (None = last used)
    #[serde(default)]
    pub default_workspace: Option<String>,
    /// Timezone preference for chart axes and time displays
    #[serde(default)]
    pub timezone: TimezonePreference,
    /// Default time range preset for new panes
    #[serde(default)]
    pub default_time_range: crate::components::widget::time_range::TimeRangePreset,
    /// What to show on startup
    #[serde(default)]
    pub startup_page: StartupPage,
    /// Whether to check for new versions automatically
    #[serde(default = "default_true")]
    pub check_for_updates: bool,
}

fn default_true() -> bool {
    true
}

/// User preference for timezone display throughout the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TimezonePreference {
    /// Use the system's local timezone (default).
    #[default]
    Local,
    /// Use UTC for all time displays.
    Utc,
}

/// What to show when the app starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum StartupPage {
    /// Show the landing / home page (default).
    #[default]
    LandingPage,
    /// Resume the last opened workspace.
    LastWorkspace,
}

/// Entry for a recently opened plot/chart
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RecentPlotEntry {
    /// Display name for the plot
    pub name: String,
    /// The metric name or query identifier
    pub metric_name: String,
    /// Unix timestamp of when it was last opened
    pub timestamp: i64,
    /// Whether this is a custom query (vs a metric)
    #[serde(default)]
    pub is_query: bool,
}

/// Entry for a recently accessed workspace
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceEntry {
    /// Display name for the workspace
    pub name: String,
    /// Description or path of the workspace
    pub description: String,
    /// Unix timestamp of when it was last accessed
    pub timestamp: i64,
}

impl AppSettings {
    /// Maximum number of recent plots to keep
    pub const MAX_RECENT_PLOTS: usize = 10;
    /// Maximum number of recent workspaces to keep
    pub const MAX_RECENT_WORKSPACES: usize = 8;

    /// Add a recent plot entry, updating timestamp if it already exists
    pub fn add_recent_plot(&mut self, name: String, metric_name: String, is_query: bool) {
        let timestamp = crate::util::now_unix_secs();

        // Remove existing entry with same metric_name
        self.recent_plots.retain(|p| p.metric_name != metric_name);

        // Add new entry at the front
        self.recent_plots.insert(
            0,
            RecentPlotEntry {
                name,
                metric_name,
                timestamp,
                is_query,
            },
        );

        // Trim to max size
        self.recent_plots.truncate(Self::MAX_RECENT_PLOTS);
    }

    /// Add a recent workspace entry, updating timestamp if it already exists
    pub fn add_recent_workspace(&mut self, name: String, description: String) {
        let timestamp = crate::util::now_unix_secs();

        // Remove existing entry with same name
        self.recent_workspaces.retain(|w| w.name != name);

        // Add new entry at the front
        self.recent_workspaces.insert(
            0,
            WorkspaceEntry {
                name,
                description,
                timestamp,
            },
        );

        // Trim to max size
        self.recent_workspaces.truncate(Self::MAX_RECENT_WORKSPACES);
    }

    /// Ensure the demo workspace is in recent workspaces (for new users)
    pub fn ensure_demo_workspace(&mut self) {
        // Check if demo workspace is already in recent workspaces
        if self.recent_workspaces.iter().any(|w| w.name == "demo") {
            return;
        }

        // Add demo workspace at the end (so it doesn't take priority over user's recent)
        // but will appear for new users
        self.recent_workspaces.push(WorkspaceEntry {
            name: "demo".to_string(),
            description: "Interactive demo with sample data".to_string(),
            timestamp: 0, // Old timestamp so it sorts last
        });

        // Don't truncate here - we want demo to stay even if at max
    }
}
