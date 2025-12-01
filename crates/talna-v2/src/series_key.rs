//! Series key formatting utilities

use crate::{MetricName, TagSet};

/// Utility for formatting series keys.
///
/// A series key uniquely identifies a time series by combining its metric name
/// with its sorted tag set.
///
/// Format: `{metric_name}#{key1:value1;key2:value2;...}`
pub struct SeriesKey;

impl SeriesKey {
    /// Formats a series key from the given metric name and tag set.
    ///
    /// Tags are sorted alphabetically by key to ensure consistent key generation.
    #[must_use]
    pub fn format(metric: MetricName, tags: &TagSet) -> String {
        let mut sorted_tags: Vec<_> = tags.iter().collect();
        sorted_tags.sort_by_key(|(k, _)| *k);

        let tag_capacity = sorted_tags
            .iter()
            .map(|(k, v)| k.len() + 1 + v.len() + 1)
            .sum::<usize>();

        let mut key = String::with_capacity(metric.as_str().len() + 1 + tag_capacity);
        key.push_str(metric.as_str());
        key.push('#');

        Self::join_tags_into(&mut key, &sorted_tags);

        key
    }

    /// Allocate a string with capacity for the sorted tags.
    #[must_use]
    pub fn allocate_string_for_tags(tags: &TagSet, extra_capacity: usize) -> String {
        let capacity = tags
            .iter()
            .map(|(k, v)| k.len() + 1 + v.len() + 1)
            .sum::<usize>()
            + extra_capacity;
        String::with_capacity(capacity)
    }

    /// Join tags into an existing string.
    /// Tags should be pre-sorted.
    pub fn join_tags(s: &mut String, tags: &TagSet) {
        let mut sorted_tags: Vec<_> = tags.iter().collect();
        sorted_tags.sort_by_key(|(k, _)| *k);
        Self::join_tags_into(s, &sorted_tags);
    }

    fn join_tags_into(s: &mut String, sorted_tags: &[&(&str, &str)]) {
        let last_idx = sorted_tags.len().saturating_sub(1);
        for (i, (k, v)) in sorted_tags.iter().enumerate() {
            s.push_str(k);
            s.push(':');
            s.push_str(v);
            if i < last_idx {
                s.push(';');
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tagset;

    #[test]
    fn test_series_key_format() {
        let metric = MetricName::try_from("cpu.total").unwrap();
        let tags = tagset!(
            "env" => "prod",
            "host" => "h-1",
            "service" => "db",
        );

        let key = SeriesKey::format(metric, tags);
        // Tags are sorted alphabetically
        assert_eq!(key, "cpu.total#env:prod;host:h-1;service:db");
    }

    #[test]
    fn test_series_key_empty_tags() {
        let metric = MetricName::try_from("cpu.total").unwrap();
        let tags: &TagSet = &[];

        let key = SeriesKey::format(metric, tags);
        assert_eq!(key, "cpu.total#");
    }
}
