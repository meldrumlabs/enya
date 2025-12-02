//! Grouped aggregation result

use super::{Bucket, stream::Aggregation, stream::Aggregator};
use crate::db::StreamItem;

/// A grouped aggregation result
///
/// Contains aggregators for each group, allowing lazy evaluation.
pub struct GroupedAggregation<'a, A, I>(pub(crate) crate::HashMap<String, Aggregator<'a, A, I>>)
where
    A: Aggregation,
    I: Iterator<Item = crate::Result<StreamItem>>;

impl<A, I> GroupedAggregation<'_, A, I>
where
    A: Aggregation,
    I: Iterator<Item = crate::Result<StreamItem>>,
{
    /// Returns the number of groups
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if there are no groups
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Check if a group exists
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Collect all buckets from all groups
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred during aggregation.
    pub fn collect(self) -> crate::Result<crate::HashMap<String, Vec<Bucket>>> {
        self.0
            .into_iter()
            .map(|(group, aggregator)| {
                let buckets: crate::Result<Vec<_>> = aggregator.collect();
                buckets.map(|b| (group, b))
            })
            .collect()
    }
}

impl<'a, A, I> IntoIterator for GroupedAggregation<'a, A, I>
where
    A: Aggregation,
    I: Iterator<Item = crate::Result<StreamItem>>,
{
    type Item = (String, Aggregator<'a, A, I>);
    type IntoIter = std::collections::hash_map::IntoIter<String, Aggregator<'a, A, I>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
