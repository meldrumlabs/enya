//! Count aggregation

use super::{Bucket, stream::Aggregation};
use crate::Value;

/// Count aggregation - counts data points in a bucket
pub struct Count;

impl Aggregation for Count {
    fn init(_value: Value) -> Value {
        1.0
    }

    fn transform(accu: Value, _x: Value) -> Value {
        accu + 1.0
    }

    fn finish(bucket: &Bucket) -> Value {
        bucket.value
    }
}
