//! Min aggregation

use super::stream::Aggregation;
use crate::Value;

/// Min aggregation - finds the minimum value in a bucket
pub struct Min;

impl Aggregation for Min {
    fn transform(accu: Value, x: Value) -> Value {
        accu.min(x)
    }
}
