//! Chart annotations for team collaboration.
//!
//! Annotations allow team members to pin comments to specific points or time ranges
//! on charts. Each annotation includes:
//! - A target (point or time range)
//! - A message/comment
//! - Author information
//! - Creation timestamp
//!
//! Annotations are rendered as markers on the chart and can be hovered for details.

use egui::Color32;
use egui_nerdfonts::regular;
#[cfg(feature = "teams")]
use enya_team_api::UserId;

use crate::ui::theme::AppTheme;

/// Unique identifier for an annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnnotationId(pub u64);

impl AnnotationId {
    /// Generate a new unique annotation ID.
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for AnnotationId {
    fn default() -> Self {
        Self::new()
    }
}

/// The target location for an annotation on a chart.
#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationTarget {
    /// A specific point in time (vertical line).
    Point {
        /// Timestamp in seconds (Unix epoch).
        timestamp: f64,
    },
    /// A time range (highlighted region).
    Range {
        /// Start timestamp in seconds.
        start: f64,
        /// End timestamp in seconds.
        end: f64,
    },
    /// A specific data point (timestamp + value).
    DataPoint {
        /// Timestamp in seconds.
        timestamp: f64,
        /// Y-axis value.
        value: f64,
    },
}

impl AnnotationTarget {
    /// Get the primary timestamp for this target (for sorting/navigation).
    pub fn primary_timestamp(&self) -> f64 {
        match self {
            Self::Point { timestamp } => *timestamp,
            Self::Range { start, .. } => *start,
            Self::DataPoint { timestamp, .. } => *timestamp,
        }
    }

    /// Check if a timestamp falls within this target's range.
    pub fn contains_timestamp(&self, t: f64, threshold: f64) -> bool {
        match self {
            Self::Point { timestamp } => (t - timestamp).abs() < threshold,
            Self::Range { start, end } => t >= *start && t <= *end,
            Self::DataPoint { timestamp, .. } => (t - timestamp).abs() < threshold,
        }
    }
}

/// Priority level for an annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotationPriority {
    /// Normal annotation (default).
    #[default]
    Normal,
    /// Important annotation (highlighted).
    Important,
    /// Critical annotation (alert-style).
    Critical,
}

impl AnnotationPriority {
    /// Get the display color for this priority using the theme.
    pub fn color_for_theme(&self, theme: AppTheme) -> Color32 {
        match self {
            Self::Normal => theme.annotation_normal(),
            Self::Important => theme.annotation_important(),
            Self::Critical => theme.annotation_critical(),
        }
    }

    /// Get the display color for this priority (uses default dark theme colors).
    /// Prefer `color_for_theme()` when a theme is available.
    pub fn color(&self) -> Color32 {
        self.color_for_theme(AppTheme::Dark)
    }

    /// Get the icon for this priority.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Normal => regular::COMMENT,
            Self::Important => regular::BOOKMARK,
            Self::Critical => regular::ALERT,
        }
    }

    /// Get the label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Note",
            Self::Important => "Important",
            Self::Critical => "Critical",
        }
    }

    /// Cycle to the next priority.
    pub fn next(&self) -> Self {
        match self {
            Self::Normal => Self::Important,
            Self::Important => Self::Critical,
            Self::Critical => Self::Normal,
        }
    }
}

/// Author information for an annotation.
#[derive(Debug, Clone)]
pub struct AnnotationAuthor {
    /// User ID (from team API, requires `teams` feature).
    #[cfg(feature = "teams")]
    pub user_id: Option<UserId>,
    /// Display name.
    pub display_name: String,
}

impl Default for AnnotationAuthor {
    fn default() -> Self {
        Self {
            #[cfg(feature = "teams")]
            user_id: None,
            display_name: "Anonymous".to_string(),
        }
    }
}

impl AnnotationAuthor {
    /// Create an author with just a display name (for local/demo mode).
    pub fn local(name: impl Into<String>) -> Self {
        Self {
            #[cfg(feature = "teams")]
            user_id: None,
            display_name: name.into(),
        }
    }

    /// Create an author from team user info (requires `teams` feature).
    #[cfg(feature = "teams")]
    pub fn from_user(user_id: UserId, display_name: impl Into<String>) -> Self {
        Self {
            user_id: Some(user_id),
            display_name: display_name.into(),
        }
    }
}

/// A chart annotation with message and metadata.
#[derive(Debug, Clone)]
pub struct Annotation {
    /// Unique identifier.
    pub id: AnnotationId,
    /// Where on the chart this annotation is attached.
    pub target: AnnotationTarget,
    /// The annotation message/comment.
    pub message: String,
    /// Who created this annotation.
    pub author: AnnotationAuthor,
    /// When this annotation was created (Unix timestamp in seconds).
    pub created_at: f64,
    /// Priority level.
    pub priority: AnnotationPriority,
    /// Whether this annotation is resolved/closed.
    pub resolved: bool,
}

impl Annotation {
    /// Create a new annotation at a point in time.
    pub fn at_point(timestamp: f64, message: impl Into<String>) -> Self {
        Self {
            id: AnnotationId::new(),
            target: AnnotationTarget::Point { timestamp },
            message: message.into(),
            author: AnnotationAuthor::default(),
            created_at: now_unix_secs(),
            priority: AnnotationPriority::Normal,
            resolved: false,
        }
    }

    /// Create a new annotation for a time range.
    pub fn at_range(start: f64, end: f64, message: impl Into<String>) -> Self {
        Self {
            id: AnnotationId::new(),
            target: AnnotationTarget::Range { start, end },
            message: message.into(),
            author: AnnotationAuthor::default(),
            created_at: now_unix_secs(),
            priority: AnnotationPriority::Normal,
            resolved: false,
        }
    }

    /// Create a new annotation at a specific data point.
    pub fn at_data_point(timestamp: f64, value: f64, message: impl Into<String>) -> Self {
        Self {
            id: AnnotationId::new(),
            target: AnnotationTarget::DataPoint { timestamp, value },
            message: message.into(),
            author: AnnotationAuthor::default(),
            created_at: now_unix_secs(),
            priority: AnnotationPriority::Normal,
            resolved: false,
        }
    }

    /// Set the author.
    pub fn with_author(mut self, author: AnnotationAuthor) -> Self {
        self.author = author;
        self
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: AnnotationPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Mark as resolved.
    pub fn resolve(&mut self) {
        self.resolved = true;
    }

    /// Get the display color based on priority and resolved state, using the theme.
    pub fn color_for_theme(&self, theme: AppTheme) -> Color32 {
        if self.resolved {
            theme.annotation_resolved()
        } else {
            self.priority.color_for_theme(theme)
        }
    }

    /// Get the display color based on priority and resolved state.
    /// Prefer `color_for_theme()` when a theme is available.
    pub fn color(&self) -> Color32 {
        self.color_for_theme(AppTheme::Dark)
    }

    /// Get the primary timestamp for sorting.
    pub fn timestamp(&self) -> f64 {
        self.target.primary_timestamp()
    }

    /// Get a short preview of the message (for tooltips/labels).
    pub fn message_preview(&self, max_len: usize) -> &str {
        if self.message.len() <= max_len {
            &self.message
        } else {
            // Find a good break point
            let preview = &self.message[..max_len];
            if let Some(space_idx) = preview.rfind(' ') {
                &self.message[..space_idx]
            } else {
                preview
            }
        }
    }
}

/// Get current Unix timestamp in seconds (WASM-compatible).
fn now_unix_secs() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        use web_time::SystemTime;
        SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotation_id_uniqueness() {
        let id1 = AnnotationId::new();
        let id2 = AnnotationId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_annotation_target_contains() {
        let point = AnnotationTarget::Point { timestamp: 100.0 };
        assert!(point.contains_timestamp(100.5, 1.0));
        assert!(!point.contains_timestamp(102.0, 1.0));

        let range = AnnotationTarget::Range {
            start: 100.0,
            end: 200.0,
        };
        assert!(range.contains_timestamp(150.0, 1.0));
        assert!(!range.contains_timestamp(250.0, 1.0));
    }

    #[test]
    fn test_annotation_creation() {
        let ann = Annotation::at_point(1000.0, "Test message")
            .with_author(AnnotationAuthor::local("Alice"))
            .with_priority(AnnotationPriority::Important);

        assert_eq!(ann.message, "Test message");
        assert_eq!(ann.author.display_name, "Alice");
        assert_eq!(ann.priority, AnnotationPriority::Important);
        assert!(!ann.resolved);
    }

    #[test]
    fn test_priority_cycle() {
        assert_eq!(
            AnnotationPriority::Normal.next(),
            AnnotationPriority::Important
        );
        assert_eq!(
            AnnotationPriority::Important.next(),
            AnnotationPriority::Critical
        );
        assert_eq!(
            AnnotationPriority::Critical.next(),
            AnnotationPriority::Normal
        );
    }

    #[test]
    fn test_message_preview() {
        let ann = Annotation::at_point(0.0, "This is a longer message for testing");
        assert_eq!(ann.message_preview(20), "This is a longer");
        assert_eq!(
            ann.message_preview(100),
            "This is a longer message for testing"
        );
    }
}
