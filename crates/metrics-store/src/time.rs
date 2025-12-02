//! Time utilities

use crate::Timestamp;

/// Returns the current timestamp in nanoseconds since Unix epoch.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn timestamp() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}
