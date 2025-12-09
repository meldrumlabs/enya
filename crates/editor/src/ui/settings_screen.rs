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
}
