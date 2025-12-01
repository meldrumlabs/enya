//! Sum aggregation

use super::stream::Aggregation;

/// Sum aggregation - adds all values in a bucket
pub struct Sum;

impl Aggregation for Sum {}
