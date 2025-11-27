use talna::Value;

#[inline]
pub fn value_as_f64(value: Value) -> f64 {
    #[cfg(feature = "high_precision")]
    {
        value
    }
    #[cfg(not(feature = "high_precision"))]
    {
        value as f64
    }
}
