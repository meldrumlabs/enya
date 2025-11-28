#[cfg(feature = "high_precision")]
const MAX_PRECISE_AVG_SAMPLES: usize = 1usize << f64::MANTISSA_DIGITS;

#[cfg(not(feature = "high_precision"))]
const MAX_PRECISE_AVG_SAMPLES: usize = 1usize << f32::MANTISSA_DIGITS;

#[derive(Clone)]
pub struct Average;

#[inline]
fn len_as_value(len: usize) -> crate::Value {
    debug_assert!(
        len <= MAX_PRECISE_AVG_SAMPLES,
        "bucket accumulated more samples ({len}) than the active precision supports exactly"
    );

    #[allow(clippy::cast_precision_loss)]
    {
        len as crate::Value
    }
}

impl super::stream::Aggregation for Average {
    fn finish(bucket: &super::Bucket) -> crate::Value {
        bucket.value / len_as_value(bucket.len)
    }
}
