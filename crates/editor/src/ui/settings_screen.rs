/// Available editor fonts
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EditorFont {
    /// Maple Mono - clean, modern monospace font
    MapleMono,
    /// Departure Mono - distinctive retro-style monospace font
    DepartureMono,
    /// JetBrains Mono - designed by JetBrains for developers
    JetBrainsMono,
    /// Iosevka - narrow, highly customizable monospace font
    Iosevka,
    /// Geist Mono - modern monospace by Vercel
    #[default]
    GeistMono,
}

impl EditorFont {
    /// Human-readable font name
    pub fn name(&self) -> &'static str {
        match self {
            Self::MapleMono => "Maple Mono",
            Self::DepartureMono => "Departure Mono",
            Self::JetBrainsMono => "JetBrains Mono",
            Self::Iosevka => "Iosevka",
            Self::GeistMono => "Geist Mono",
        }
    }

    /// Internal font family name used in egui
    pub fn font_family_name(&self) -> &'static str {
        match self {
            Self::MapleMono => "maple_mono",
            Self::DepartureMono => "departure_mono",
            Self::JetBrainsMono => "jetbrains_mono",
            Self::Iosevka => "iosevka",
            Self::GeistMono => "geist_mono",
        }
    }

    /// Returns all available fonts
    pub fn all() -> &'static [EditorFont] {
        &[
            Self::GeistMono,
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
            Self::GeistMono => "Modern, clean monospace by Vercel",
        }
    }
}

use crate::components::util::AiProvider;
use crate::ui::theme::AppTheme;

/// A named Arrow Flight SQL connection endpoint.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FlightSqlConnection {
    /// User-facing label (e.g., "prod", "staging", "local").
    pub name: String,
    /// Flight SQL endpoint URL (e.g., "grpc://localhost:50051").
    pub endpoint: String,
}

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
    /// Selected AI model ID (None = use provider default).
    /// Stores the API model ID string (e.g. "claude-sonnet-4-5-20250514").
    /// Custom deserializer handles migration from legacy enum variant names.
    #[serde(default, deserialize_with = "deserialize_ai_model")]
    pub ai_model: Option<String>,
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
    /// [DEPRECATED] Legacy single Flight SQL endpoint — kept for migration only.
    #[serde(default, skip_serializing)]
    pub default_flight_sql_endpoint: String,
    /// Named Arrow Flight SQL connections (persisted between sessions).
    #[serde(default)]
    pub flight_sql_connections: Vec<FlightSqlConnection>,
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
    /// Whether to notify when new AI models become available
    #[serde(default = "default_true")]
    pub notify_new_models: bool,
    /// How often to automatically fetch new commits from the remote repository
    #[serde(default)]
    pub git_sync_interval: GitSyncInterval,
}

impl AppSettings {
    /// Migrate legacy settings fields to their new equivalents.
    ///
    /// Call once after deserializing from storage.
    pub fn migrate(&mut self) {
        // Migrate single Flight SQL endpoint to the new connections list
        if self.flight_sql_connections.is_empty() && !self.default_flight_sql_endpoint.is_empty() {
            self.flight_sql_connections.push(FlightSqlConnection {
                name: "default".to_string(),
                endpoint: self.default_flight_sql_endpoint.clone(),
            });
            self.default_flight_sql_endpoint.clear();
        }
    }
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

/// How often to automatically fetch new commits from the remote repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum GitSyncInterval {
    /// Disable automatic git fetch.
    Off,
    /// Fetch every 1 minute.
    OneMinute,
    /// Fetch every 5 minutes (default).
    #[default]
    FiveMinutes,
    /// Fetch every 15 minutes.
    FifteenMinutes,
    /// Fetch every 30 minutes.
    ThirtyMinutes,
}

impl GitSyncInterval {
    /// Returns the interval in seconds, or 0 if disabled.
    pub fn to_secs(self) -> u64 {
        match self {
            Self::Off => 0,
            Self::OneMinute => 60,
            Self::FiveMinutes => 300,
            Self::FifteenMinutes => 900,
            Self::ThirtyMinutes => 1800,
        }
    }

    /// Returns a display label for this interval.
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::OneMinute => "1m",
            Self::FiveMinutes => "5m",
            Self::FifteenMinutes => "15m",
            Self::ThirtyMinutes => "30m",
        }
    }

    /// All variants in display order.
    pub fn all() -> &'static [Self] {
        &[
            Self::Off,
            Self::OneMinute,
            Self::FiveMinutes,
            Self::FifteenMinutes,
            Self::ThirtyMinutes,
        ]
    }
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

/// Custom deserializer for `ai_model` that handles both:
/// - New format: `Some("claude-sonnet-4-5-20250514")` (string model ID)
/// - Legacy format: `Some(ClaudeSonnet45)` (RON enum variant from old `AiModel` enum)
fn deserialize_ai_model<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use crate::components::util::migrate_legacy_model_name;
    use serde::de;

    struct OptModelVisitor;

    impl<'de> de::Visitor<'de> for OptModelVisitor {
        type Value = Option<String>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("None, a model ID string, or a legacy AiModel variant")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D2: serde::Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
            d.deserialize_any(ModelValueVisitor).map(Some)
        }

        fn visit_enum<A: de::EnumAccess<'de>>(self, data: A) -> Result<Self::Value, A::Error> {
            let (variant, va) = de::EnumAccess::variant::<String>(data)?;
            match variant.as_str() {
                "None" => {
                    de::VariantAccess::unit_variant(va)?;
                    Ok(None)
                }
                "Some" => {
                    let inner = de::VariantAccess::newtype_variant::<ModelValue>(va)?;
                    Ok(Some(inner.0))
                }
                name => {
                    de::VariantAccess::unit_variant(va)?;
                    Ok(Some(migrate_legacy_model_name(name).to_string()))
                }
            }
        }
    }

    struct ModelValueVisitor;

    impl<'de> de::Visitor<'de> for ModelValueVisitor {
        type Value = String;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a model ID string or legacy enum variant")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(migrate_legacy_model_name(v).to_string())
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(migrate_legacy_model_name(&v).to_string())
        }

        fn visit_enum<A: de::EnumAccess<'de>>(self, data: A) -> Result<Self::Value, A::Error> {
            let (variant, va) = de::EnumAccess::variant::<String>(data)?;
            de::VariantAccess::unit_variant(va)?;
            Ok(migrate_legacy_model_name(&variant).to_string())
        }
    }

    struct ModelValue(String);

    impl<'de> serde::Deserialize<'de> for ModelValue {
        fn deserialize<D2: serde::Deserializer<'de>>(d: D2) -> Result<Self, D2::Error> {
            d.deserialize_any(ModelValueVisitor).map(ModelValue)
        }
    }

    deserializer.deserialize_option(OptModelVisitor)
}
