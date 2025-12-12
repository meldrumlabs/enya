use std::fmt;

/// Granularity for metric aggregation windows
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Granularity {
    /// 1 minute
    OneMinute,
    /// 5 minutes
    #[default]
    FiveMinutes,
    /// 15 minutes
    FifteenMinutes,
    /// 1 hour
    OneHour,
    /// 6 hours
    SixHours,
    /// 1 day
    OneDay,
}

impl Granularity {
    /// Get the duration in seconds
    pub fn seconds(&self) -> u64 {
        match self {
            Self::OneMinute => 60,
            Self::FiveMinutes => 5 * 60,
            Self::FifteenMinutes => 15 * 60,
            Self::OneHour => 60 * 60,
            Self::SixHours => 6 * 60 * 60,
            Self::OneDay => 24 * 60 * 60,
        }
    }

    /// Get the display label
    pub fn label(&self) -> &'static str {
        match self {
            Self::OneMinute => "1m",
            Self::FiveMinutes => "5m",
            Self::FifteenMinutes => "15m",
            Self::OneHour => "1h",
            Self::SixHours => "6h",
            Self::OneDay => "1d",
        }
    }

    /// Get all granularities
    pub fn all() -> &'static [Granularity] {
        &[
            Self::OneMinute,
            Self::FiveMinutes,
            Self::FifteenMinutes,
            Self::OneHour,
            Self::SixHours,
            Self::OneDay,
        ]
    }

    /// Cycle to next granularity
    pub fn cycle_next(&self) -> Self {
        match self {
            Self::OneMinute => Self::FiveMinutes,
            Self::FiveMinutes => Self::FifteenMinutes,
            Self::FifteenMinutes => Self::OneHour,
            Self::OneHour => Self::SixHours,
            Self::SixHours => Self::OneDay,
            Self::OneDay => Self::OneMinute,
        }
    }

    /// Cycle to previous granularity
    pub fn cycle_prev(&self) -> Self {
        match self {
            Self::OneMinute => Self::OneDay,
            Self::FiveMinutes => Self::OneMinute,
            Self::FifteenMinutes => Self::FiveMinutes,
            Self::OneHour => Self::FifteenMinutes,
            Self::SixHours => Self::OneHour,
            Self::OneDay => Self::SixHours,
        }
    }
}

impl fmt::Display for Granularity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Complete query state for a buffer (view preferences)
#[derive(Debug, Clone, PartialEq)]
pub struct QueryState {
    /// The granularity for aggregation
    pub granularity: Granularity,
    /// Time range preset (from TimeRangePreset)
    pub time_range_label: String,
}

impl Default for QueryState {
    fn default() -> Self {
        Self {
            granularity: Granularity::default(),
            time_range_label: "15m".to_string(),
        }
    }
}

impl QueryState {
    /// Create a new query state
    pub fn new() -> Self {
        Self::default()
    }

    /// Cycle granularity forward
    pub fn cycle_granularity(&mut self) {
        self.granularity = self.granularity.cycle_next();
    }

    /// Cycle granularity backward
    pub fn cycle_granularity_back(&mut self) {
        self.granularity = self.granularity.cycle_prev();
    }

    /// Format the status line
    pub fn format_status(&self) -> String {
        format!("{} {}", self.time_range_label, self.granularity)
    }
}
