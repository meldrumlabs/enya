//! Max aggregation

use super::stream::Aggregation;
use crate::Value;

/// Max aggregation - finds the maximum value in a bucket
pub struct Max;

impl Aggregation for Max {
    fn transform(accu: Value, x: Value) -> Value {
        accu.max(x)
    }
}
