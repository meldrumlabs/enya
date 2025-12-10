use std::time::Duration;

/// Default directory for storing enya data.
pub const DEFAULT_DATA_DIR: &str = "/tmp/enya";

/// Default interval for collecting task monitor metrics.
pub const DEFAULT_TASK_METRICS_INTERVAL: Duration = Duration::from_secs(30);

/// Options for task monitor metrics collection.
#[derive(Debug, Clone)]
pub struct TaskMetricsOptions {
    /// Whether task metrics collection is enabled.
    pub enabled: bool,
    /// Interval between metric collections.
    pub interval: Duration,
}

impl Default for TaskMetricsOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: DEFAULT_TASK_METRICS_INTERVAL,
        }
    }
}

impl TaskMetricsOptions {
    /// Creates task metrics options with collection enabled.
    #[must_use]
    pub fn enabled() -> Self {
        Self::default()
    }

    /// Creates task metrics options with collection disabled.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            interval: DEFAULT_TASK_METRICS_INTERVAL,
        }
    }

    /// Sets the collection interval.
    #[must_use]
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }
}

/// Options for configuring Enya.
pub struct Options {
    /// Directory where metrics and logs are stored.
    data_dir: String,
    /// Options for task monitor metrics collection.
    task_metrics: TaskMetricsOptions,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            data_dir: DEFAULT_DATA_DIR.to_string(),
            task_metrics: TaskMetricsOptions::default(),
        }
    }
}

impl Options {
    /// Returns the directory used to store metrics and logs.
    #[must_use]
    pub fn data_dir(&self) -> &str {
        &self.data_dir
    }

    /// Returns the task metrics options.
    #[must_use]
    pub fn task_metrics(&self) -> &TaskMetricsOptions {
        &self.task_metrics
    }

    /// Sets the task metrics options.
    #[must_use]
    pub fn with_task_metrics(mut self, options: TaskMetricsOptions) -> Self {
        self.task_metrics = options;
        self
    }

    /// Sets the data directory.
    #[must_use]
    pub fn with_data_dir(mut self, data_dir: impl Into<String>) -> Self {
        self.data_dir = data_dir.into();
        self
    }
}
