use std::fmt;

/// Aggregation function for metrics
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AggregationMode {
    /// No aggregation (raw values)
    #[default]
    None,
    /// Sum of values
    Sum,
    /// Average of values
    Avg,
    /// Minimum value
    Min,
    /// Maximum value
    Max,
    /// 50th percentile
    P50,
    /// 95th percentile
    P95,
    /// 99th percentile
    P99,
}

impl AggregationMode {
    /// Get the display label for this aggregation mode
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "raw",
            Self::Sum => "sum",
            Self::Avg => "avg",
            Self::Min => "min",
            Self::Max => "max",
            Self::P50 => "p50",
            Self::P95 => "p95",
            Self::P99 => "p99",
        }
    }

    /// Get all aggregation modes
    pub fn all() -> &'static [AggregationMode] {
        &[
            Self::None,
            Self::Sum,
            Self::Avg,
            Self::Min,
            Self::Max,
            Self::P50,
            Self::P95,
            Self::P99,
        ]
    }

    /// Cycle through percentile modes (p50 -> p95 -> p99 -> none)
    pub fn cycle_percentiles(&self) -> Self {
        match self {
            Self::P50 => Self::P95,
            Self::P95 => Self::P99,
            Self::P99 => Self::None,
            _ => Self::P50,
        }
    }

    /// Check if this is a percentile mode
    pub fn is_percentile(&self) -> bool {
        matches!(self, Self::P50 | Self::P95 | Self::P99)
    }
}

impl fmt::Display for AggregationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

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
    /// The aggregation function to apply
    pub aggregation: AggregationMode,
    /// The granularity for aggregation
    pub granularity: Granularity,
    /// Time range preset (from TimeRangePreset)
    pub time_range_label: String,
}

impl Default for QueryState {
    fn default() -> Self {
        Self {
            aggregation: AggregationMode::default(),
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

    /// Set aggregation to sum
    pub fn set_sum(&mut self) {
        self.aggregation = if self.aggregation == AggregationMode::Sum {
            AggregationMode::None
        } else {
            AggregationMode::Sum
        };
    }

    /// Set aggregation to avg
    pub fn set_avg(&mut self) {
        self.aggregation = if self.aggregation == AggregationMode::Avg {
            AggregationMode::None
        } else {
            AggregationMode::Avg
        };
    }

    /// Set aggregation to min
    pub fn set_min(&mut self) {
        self.aggregation = if self.aggregation == AggregationMode::Min {
            AggregationMode::None
        } else {
            AggregationMode::Min
        };
    }

    /// Set aggregation to max
    pub fn set_max(&mut self) {
        self.aggregation = if self.aggregation == AggregationMode::Max {
            AggregationMode::None
        } else {
            AggregationMode::Max
        };
    }

    /// Cycle percentiles (p50 -> p95 -> p99 -> off)
    pub fn cycle_percentiles(&mut self) {
        self.aggregation = self.aggregation.cycle_percentiles();
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
        let agg = if self.aggregation == AggregationMode::None {
            "raw".to_string()
        } else {
            self.aggregation.label().to_string()
        };
        format!("[{}] {} {}", agg, self.time_range_label, self.granularity)
    }
}
