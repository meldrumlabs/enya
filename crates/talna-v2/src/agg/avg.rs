//! Average aggregation

use super::{Bucket, stream::Aggregation};
use crate::Value;

/// Maximum number of samples for precise average calculation
const MAX_PRECISE_AVG_SAMPLES: usize = 1_000_000;

/// Average aggregation - computes the mean of values in a bucket
pub struct Average;

impl Aggregation for Average {
    #[allow(clippy::cast_precision_loss)]
    fn finish(bucket: &Bucket) -> Value {
        if bucket.len == 0 {
            return 0.0;
        }

        // Guard against precision loss with many samples
        if bucket.len > MAX_PRECISE_AVG_SAMPLES {
            log::warn!(
                "Averaging {} samples may result in precision loss",
                bucket.len
            );
        }

        #[cfg(feature = "high_precision")]
        {
            bucket.value / (bucket.len as f64)
        }
        #[cfg(not(feature = "high_precision"))]
        {
            bucket.value / (bucket.len as f32)
        }
    }
}
