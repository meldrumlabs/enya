//! Git-related types for correlating metrics with code changes.

/// A marker representing a git commit at a specific point in time.
///
/// Used to annotate time-series charts with commit information,
/// allowing correlation between metric changes and code changes.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitMarker {
    /// Git commit hash (full or abbreviated)
    pub hash: String,
    /// Timestamp of the commit in seconds (Unix epoch)
    pub timestamp: f64,
    /// Commit message (first line / subject)
    pub message: String,
}

impl CommitMarker {
    pub fn new(hash: impl Into<String>, timestamp: f64, message: impl Into<String>) -> Self {
        Self {
            hash: hash.into(),
            timestamp,
            message: message.into(),
        }
    }

    /// Get abbreviated hash (first 7 characters)
    #[must_use]
    pub fn short_hash(&self) -> &str {
        if self.hash.len() > 7 {
            &self.hash[..7]
        } else {
            &self.hash
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_marker_new() {
        let marker = CommitMarker::new("abc123def456", 1700000000.0, "Initial commit");
        assert_eq!(marker.hash, "abc123def456");
        assert!((marker.timestamp - 1700000000.0).abs() < f64::EPSILON);
        assert_eq!(marker.message, "Initial commit");
    }

    #[test]
    fn test_short_hash_long() {
        let marker = CommitMarker::new("abc123def456789", 0.0, "test");
        assert_eq!(marker.short_hash(), "abc123d");
    }

    #[test]
    fn test_short_hash_short() {
        let marker = CommitMarker::new("abc", 0.0, "test");
        assert_eq!(marker.short_hash(), "abc");
    }

    #[test]
    fn test_short_hash_exact_seven() {
        let marker = CommitMarker::new("abc1234", 0.0, "test");
        assert_eq!(marker.short_hash(), "abc1234");
    }
}
