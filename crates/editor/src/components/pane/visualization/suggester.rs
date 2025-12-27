//! Visualization type suggestion based on query result characteristics.
//!
//! This module provides heuristics for suggesting the most appropriate
//! visualization type based on Prometheus result type and data shape.

use super::VisualizationType;
use enya_client::{QueryResponse, ResultType};

/// Result characteristics used for visualization suggestion.
#[derive(Debug, Clone)]
pub struct ResultCharacteristics {
    /// Prometheus result type
    pub result_type: ResultType,
    /// Number of series/groups in the result
    pub series_count: usize,
    /// Total number of data points across all series
    pub point_count: usize,
    /// Whether values appear to be percentages (0-1 or 0-100 range)
    pub appears_percentage: bool,
    /// Average points per series
    pub avg_points_per_series: usize,
}

impl ResultCharacteristics {
    /// Extract characteristics from a `QueryResponse`.
    #[must_use]
    pub fn from_response(response: &QueryResponse) -> Self {
        let series_count = response.groups.len();
        let point_count: usize = response.groups.iter().map(|g| g.buckets.len()).sum();
        let avg_points_per_series = if series_count > 0 {
            point_count / series_count
        } else {
            0
        };

        // Check if values look like percentages
        let appears_percentage = check_percentage_range(response);

        Self {
            result_type: response.result_type,
            series_count,
            point_count,
            appears_percentage,
            avg_points_per_series,
        }
    }
}

/// Suggest the best visualization type based on result characteristics.
///
/// This function applies heuristics to suggest an appropriate visualization:
///
/// | Result Type    | Series | Points | Suggested        |
/// |----------------|--------|--------|------------------|
/// | Scalar/String  | -      | -      | Stat             |
/// | Vector         | 1      | 1      | Stat/Gauge       |
/// | Vector         | 2-10   | 1 each | BarChart         |
/// | Matrix         | 1      | few    | Stat/Sparkline   |
/// | Matrix         | 1      | many   | TimeSeries       |
/// | Matrix         | many   | many   | TimeSeries       |
///
/// Note: Heatmap and Flamegraph are NOT auto-suggested as they require
/// specific data formats.
#[must_use]
pub fn suggest_visualization(chars: &ResultCharacteristics) -> VisualizationType {
    match chars.result_type {
        ResultType::Scalar | ResultType::String => {
            // Single value -> Stat display
            VisualizationType::Stat
        }
        ResultType::Vector => {
            // Instant vector (single point per series)
            if chars.series_count == 1 {
                // Single series, single point -> Stat or Gauge
                if chars.appears_percentage {
                    VisualizationType::Gauge
                } else {
                    VisualizationType::Stat
                }
            } else {
                // Multiple series -> Bar chart for comparison
                VisualizationType::BarChart
            }
        }
        ResultType::Matrix => {
            // Range vector (time series data)
            if chars.series_count == 1 && chars.avg_points_per_series <= 3 {
                // Single series with very few points -> Stat
                if chars.appears_percentage {
                    VisualizationType::Gauge
                } else {
                    VisualizationType::Stat
                }
            } else if chars.series_count == 1 && chars.avg_points_per_series < 20 {
                // Single series with few points -> Sparkline
                VisualizationType::Sparkline
            } else {
                // Multiple time series or many points -> TimeSeries chart
                VisualizationType::TimeSeries
            }
        }
    }
}

/// Check if values appear to be in percentage range (0-1 or 0-100).
fn check_percentage_range(response: &QueryResponse) -> bool {
    let values: Vec<f64> = response
        .groups
        .iter()
        .flat_map(|g| g.buckets.iter().map(|b| b.value))
        .collect();

    if values.is_empty() {
        return false;
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    // Check for 0-1 range (ratios) or 0-100 range (percentages)
    (min >= 0.0 && max <= 1.0) || (min >= 0.0 && max <= 100.0 && max > 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use enya_client::{MetricsBucket, MetricsGroup};

    fn make_response(
        result_type: ResultType,
        groups: Vec<(usize, f64)>, // (num_points, value) per group
    ) -> QueryResponse {
        let groups = groups
            .into_iter()
            .enumerate()
            .map(|(i, (num_points, value))| MetricsGroup {
                group: format!("series-{i}"),
                buckets: (0..num_points)
                    .map(|j| MetricsBucket {
                        start: (j as u128) * 60_000_000_000,
                        end: ((j + 1) as u128) * 60_000_000_000,
                        value,
                        count: 1,
                    })
                    .collect(),
            })
            .collect();

        QueryResponse {
            metric: "test_metric".to_string(),
            query: "*".to_string(),
            parsed_agg: None,
            parsed_filter: String::new(),
            parsed_grouping: None,
            parsed_time_range: None,
            start: None,
            end: None,
            granularity_ns: 60_000_000_000,
            groups,
            result_type,
        }
    }

    #[test]
    fn test_scalar_suggests_stat() {
        let response = make_response(ResultType::Scalar, vec![(1, 42.0)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(suggest_visualization(&chars), VisualizationType::Stat);
    }

    #[test]
    fn test_string_suggests_stat() {
        let response = make_response(ResultType::String, vec![]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(suggest_visualization(&chars), VisualizationType::Stat);
    }

    #[test]
    fn test_vector_single_series_suggests_stat() {
        // Use a value > 100 to avoid triggering the percentage heuristic
        let response = make_response(ResultType::Vector, vec![(1, 500.0)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(suggest_visualization(&chars), VisualizationType::Stat);
    }

    #[test]
    fn test_vector_single_series_percentage_suggests_gauge() {
        let response = make_response(ResultType::Vector, vec![(1, 0.75)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(suggest_visualization(&chars), VisualizationType::Gauge);
    }

    #[test]
    fn test_vector_multiple_series_suggests_bar() {
        let response = make_response(ResultType::Vector, vec![(1, 10.0), (1, 20.0), (1, 30.0)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(suggest_visualization(&chars), VisualizationType::BarChart);
    }

    #[test]
    fn test_matrix_single_series_few_points_suggests_stat() {
        // Use a value > 100 to avoid triggering the percentage heuristic
        let response = make_response(ResultType::Matrix, vec![(2, 500.0)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(suggest_visualization(&chars), VisualizationType::Stat);
    }

    #[test]
    fn test_matrix_single_series_medium_points_suggests_sparkline() {
        let response = make_response(ResultType::Matrix, vec![(15, 42.0)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(suggest_visualization(&chars), VisualizationType::Sparkline);
    }

    #[test]
    fn test_matrix_single_series_many_points_suggests_timeseries() {
        let response = make_response(ResultType::Matrix, vec![(100, 42.0)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(suggest_visualization(&chars), VisualizationType::TimeSeries);
    }

    #[test]
    fn test_matrix_multiple_series_suggests_timeseries() {
        let response = make_response(ResultType::Matrix, vec![(50, 10.0), (50, 20.0), (50, 30.0)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(suggest_visualization(&chars), VisualizationType::TimeSeries);
    }

    #[test]
    fn test_percentage_detection_ratio() {
        // Values in 0-1 range
        let response = make_response(ResultType::Vector, vec![(1, 0.5)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert!(chars.appears_percentage);
    }

    #[test]
    fn test_percentage_detection_percent() {
        // Values in 0-100 range
        let response = make_response(ResultType::Vector, vec![(1, 75.0)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert!(chars.appears_percentage);
    }

    #[test]
    fn test_percentage_detection_large_values() {
        // Values > 100 are not percentages
        let response = make_response(ResultType::Vector, vec![(1, 500.0)]);
        let chars = ResultCharacteristics::from_response(&response);
        assert!(!chars.appears_percentage);
    }

    #[test]
    fn test_empty_response() {
        let response = make_response(ResultType::Matrix, vec![]);
        let chars = ResultCharacteristics::from_response(&response);
        assert_eq!(chars.series_count, 0);
        assert_eq!(chars.point_count, 0);
        assert!(!chars.appears_percentage);
        // Empty matrix still suggests TimeSeries (default for matrix)
        assert_eq!(suggest_visualization(&chars), VisualizationType::TimeSeries);
    }
}
