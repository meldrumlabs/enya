//! Centralized ID generation utilities.
//!
//! Provides type-safe, unique ID generation for components, eliminating scattered
//! static atomics throughout the codebase.

use std::sync::atomic::{AtomicU64, Ordering};

/// Global ID generator for all component IDs.
///
/// Uses a single atomic counter to ensure globally unique IDs across all
/// component types. The high bit space (u64) ensures we won't exhaust IDs.
static GLOBAL_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique ID for any component.
///
/// Returns a monotonically increasing u64 that is guaranteed to be unique
/// within the lifetime of the application.
///
/// # Examples
///
/// ```
/// use enya_editor::components::id_generator::next_id;
///
/// let id1 = next_id();
/// let id2 = next_id();
/// assert_ne!(id1, id2);
/// ```
#[inline]
pub fn next_id() -> u64 {
    GLOBAL_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Generate a unique ID and return it as usize.
///
/// Convenience function for components that use usize IDs.
#[inline]
pub fn next_id_usize() -> usize {
    next_id() as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let id1 = next_id();
        let id2 = next_id();
        let id3 = next_id();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn ids_are_monotonic() {
        let id1 = next_id();
        let id2 = next_id();
        assert!(id2 > id1);
    }
}
