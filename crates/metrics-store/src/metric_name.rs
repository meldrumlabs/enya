//! Metric name type with validation

use std::fmt;

/// A validated metric name.
///
/// Metric names may only contain: a-z, A-Z, 0-9, '.', '_'
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct MetricName<'a>(&'a str);

impl<'a> MetricName<'a> {
    /// Returns the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &'a str {
        self.0
    }
}

impl fmt::Debug for MetricName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MetricName({:?})", self.0)
    }
}

impl fmt::Display for MetricName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> TryFrom<&'a str> for MetricName<'a> {
    type Error = &'static str;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err("metric name cannot be empty");
        }

        for c in value.chars() {
            if !c.is_ascii_alphanumeric() && c != '.' && c != '_' {
                return Err("metric name may only contain: a-z, A-Z, 0-9, '.', '_'");
            }
        }

        Ok(Self(value))
    }
}

impl AsRef<str> for MetricName<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}
