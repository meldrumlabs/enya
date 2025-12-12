//! Over-time aggregation functions
//!
//! These functions aggregate values within a time range window:
//! - `avg_over_time` - Average of all values in the time range
//! - `sum_over_time` - Sum of all values in the time range
//! - `min_over_time` - Minimum value in the time range
//! - `max_over_time` - Maximum value in the time range
//! - `count_over_time` - Count of values in the time range

use super::{Bucket, stream::Aggregation};
use crate::Value;

/// Average over time - computes the mean of all values in the time range
pub struct AvgOverTime;

impl Aggregation for AvgOverTime {
    #[allow(clippy::cast_precision_loss)]
    fn finish(bucket: &Bucket) -> Value {
        if bucket.len == 0 {
            return 0.0;
        }
        bucket.value / (bucket.len as f64)
    }
}

/// Sum over time - computes the sum of all values in the time range
pub struct SumOverTime;

impl Aggregation for SumOverTime {
    // Default implementation: init = value, transform = add, finish = identity
}

/// Min over time - finds the minimum value in the time range
pub struct MinOverTime;

impl Aggregation for MinOverTime {
    fn transform(accu: Value, x: Value) -> Value {
        accu.min(x)
    }
}

/// Max over time - finds the maximum value in the time range
pub struct MaxOverTime;

impl Aggregation for MaxOverTime {
    fn transform(accu: Value, x: Value) -> Value {
        accu.max(x)
    }
}

/// Count over time - counts the number of values in the time range
pub struct CountOverTime;

impl Aggregation for CountOverTime {
    fn init(_value: Value) -> Value {
        1.0
    }

    fn transform(accu: Value, _x: Value) -> Value {
        accu + 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avg_over_time_finish() {
        let bucket = Bucket {
            start: 0,
            end: 100,
            value: 30.0,
            len: 3,
        };
        assert!((AvgOverTime::finish(&bucket) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_avg_over_time_empty() {
        let bucket = Bucket {
            start: 0,
            end: 0,
            value: 0.0,
            len: 0,
        };
        assert!((AvgOverTime::finish(&bucket) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sum_over_time() {
        // Sum uses default implementation which just adds values
        let result = SumOverTime::transform(10.0, 5.0);
        assert!((result - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_min_over_time() {
        assert!((MinOverTime::transform(10.0, 5.0) - 5.0).abs() < f64::EPSILON);
        assert!((MinOverTime::transform(5.0, 10.0) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_max_over_time() {
        assert!((MaxOverTime::transform(10.0, 5.0) - 10.0).abs() < f64::EPSILON);
        assert!((MaxOverTime::transform(5.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_count_over_time() {
        let initial = CountOverTime::init(42.0);
        assert!((initial - 1.0).abs() < f64::EPSILON);

        let after_one = CountOverTime::transform(initial, 100.0);
        assert!((after_one - 2.0).abs() < f64::EPSILON);

        let after_two = CountOverTime::transform(after_one, 200.0);
        assert!((after_two - 3.0).abs() < f64::EPSILON);
    }
}
