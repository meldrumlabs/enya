/// Available editor fonts
#[derive(Clone, Default, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EditorFont {
    /// Maple Mono - clean, modern monospace font
    MapleMono,
    /// Departure Mono - distinctive retro-style monospace font
    DepartureMono,
    /// JetBrains Mono - designed by JetBrains for developers
    JetBrainsMono,
    /// Iosevka - narrow, highly customizable monospace font
    Iosevka,
    /// Geist Mono - modern, clean monospace font
    #[default]
    GeistMono,
    /// User-loaded custom font
    Custom {
        /// Display name for the font
        name: String,
        /// Absolute path to the font file
        path: String,
    },
}

impl EditorFont {
    /// Human-readable font name
    pub fn name(&self) -> String {
        match self {
            Self::MapleMono => "Maple Mono".into(),
            Self::DepartureMono => "Departure Mono".into(),
            Self::JetBrainsMono => "JetBrains Mono".into(),
            Self::Iosevka => "Iosevka".into(),
            Self::GeistMono => "Geist Mono".into(),
            Self::Custom { name, .. } => name.clone(),
        }
    }

    /// Internal font family name used in egui
    pub fn font_family_name(&self) -> String {
        match self {
            Self::MapleMono => "maple_mono".into(),
            Self::DepartureMono => "departure_mono".into(),
            Self::JetBrainsMono => "jetbrains_mono".into(),
            Self::Iosevka => "iosevka".into(),
            Self::GeistMono => "geist_mono".into(),
            Self::Custom { name, .. } => format!("custom_{name}"),
        }
    }

    /// Returns all built-in fonts
    pub fn all_builtins() -> &'static [EditorFont] {
        &[
            Self::GeistMono,
            Self::DepartureMono,
            Self::MapleMono,
            Self::JetBrainsMono,
            Self::Iosevka,
        ]
    }

    /// Returns all available fonts (built-ins only; UI should merge with custom fonts)
    pub fn all() -> &'static [EditorFont] {
        Self::all_builtins()
    }

    /// Description of the font
    pub fn description(&self) -> String {
        match self {
            Self::MapleMono => "Clean, modern monospace with ligatures".into(),
            Self::DepartureMono => "Distinctive retro-style monospace".into(),
            Self::JetBrainsMono => "Developer-focused, great for code".into(),
            Self::Iosevka => "Narrow, highly customizable".into(),
            Self::GeistMono => "Modern, clean monospace font".into(),
            Self::Custom { path, .. } => {
                let ext = std::path::Path::new(path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_uppercase())
                    .unwrap_or_else(|| "CUSTOM".into());
                format!("Custom · {ext}")
            }
        }
    }

    /// Returns true if this is a user-loaded custom font
    pub fn is_custom(&self) -> bool {
        matches!(self, Self::Custom { .. })
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
    /// User-loaded custom fonts: (display_name, absolute_path)
    #[serde(default)]
    pub custom_fonts: Vec<(String, String)>,
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
    /// Default Tempo endpoint for new workspaces
    #[serde(default)]
    pub default_tempo_endpoint: String,
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
    pub github_credentials: Option<crate::git::auth::GitHubCredentials>,
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
    /// Legacy field — projects are now filesystem-based. Kept for serde compat.
    #[serde(default, skip_serializing)]
    _projects: Vec<serde::de::IgnoredAny>,
    /// Port for the embedded OTLP HTTP receiver (default: 4318).
    /// Changes take effect on next app launch.
    #[serde(default = "default_otlp_port")]
    pub otlp_port: u16,
}

fn default_otlp_port() -> u16 {
    4318
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
    /// Project this workspace belongs to
    #[serde(default)]
    pub project: String,
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

    /// Add a recent workspace entry, updating timestamp if it already exists.
    pub fn add_recent_workspace(&mut self, name: String, description: String, project: String) {
        let timestamp = crate::util::now_unix_secs();

        // Remove existing entry with same name in same project
        self.recent_workspaces
            .retain(|w| !(w.name == name && w.project == project));

        // Add new entry at the front
        self.recent_workspaces.insert(
            0,
            WorkspaceEntry {
                name,
                description,
                timestamp,
                project,
            },
        );

        // Trim to max size
        self.recent_workspaces.truncate(Self::MAX_RECENT_WORKSPACES);
    }

    /// Remove recent workspace entries whose `.toml` files no longer exist on disk.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn prune_stale_workspaces(&mut self) {
        let before = self.recent_workspaces.len();
        self.recent_workspaces
            .retain(|w| enya_config::resolve_project_workspace_path(&w.project, &w.name).exists());
        let pruned = before - self.recent_workspaces.len();

        if pruned > 0 {
            log::info!("Pruned {pruned} stale workspace(s) from recent list");
        }
    }

    /// Canonical list of built-in tutorial workspaces.
    /// Names must match the files written by `ensure_default_workspace()`.
    pub const TUTORIAL_WORKSPACES: &[(&str, &str)] = &[
        ("quick-start", "The 4 golden signals at a glance"),
        ("infra", "CPU, memory, and system health"),
        ("logs-and-traces", "Explore logs and distributed traces"),
    ];

    /// Ensure tutorial workspaces are in recent_workspaces so they appear in the sidebar.
    pub fn ensure_tutorial_project(&mut self) {
        // On WASM, always rebuild from scratch to guarantee a clean demo
        // experience regardless of any persisted data in localStorage.
        #[cfg(target_arch = "wasm32")]
        {
            let tutorial_names: Vec<String> = Self::TUTORIAL_WORKSPACES
                .iter()
                .map(|&(n, _)| n.to_string())
                .collect();
            self.recent_workspaces
                .retain(|w| tutorial_names.contains(&w.name));
        }

        // Ensure all tutorial workspaces are in recent_workspaces with correct project
        for &(name, desc) in Self::TUTORIAL_WORKSPACES {
            if let Some(existing) = self.recent_workspaces.iter_mut().find(|w| w.name == name) {
                // Fix project field on entries from before the project migration
                existing.project = "Tutorial".to_string();
            } else {
                self.recent_workspaces.push(WorkspaceEntry {
                    name: name.to_string(),
                    description: desc.to_string(),
                    timestamp: 0,
                    project: "Tutorial".to_string(),
                });
            }
        }
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
