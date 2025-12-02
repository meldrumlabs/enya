//! Grouped aggregation result

use super::{Bucket, stream::Aggregation, stream::Aggregator};
use crate::db::StreamItem;
use futures::{Stream, TryStreamExt};

/// A grouped aggregation result
///
/// Contains aggregators for each group, allowing lazy evaluation.
pub struct GroupedAggregation<A, S>(pub(crate) crate::HashMap<String, Aggregator<A, S>>)
where
    A: Aggregation,
    S: Stream<Item = crate::Result<StreamItem>>;

impl<A, S> GroupedAggregation<A, S>
where
    A: Aggregation,
    S: Stream<Item = crate::Result<StreamItem>> + Unpin,
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

    /// Collect all buckets from all groups asynchronously
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred during aggregation.
    pub async fn collect(self) -> crate::Result<crate::HashMap<String, Vec<Bucket>>> {
        let mut result = crate::HashMap::default();

        for (group, aggregator) in self.0 {
            let buckets: Vec<_> = aggregator.try_collect().await?;
            result.insert(group, buckets);
        }

        Ok(result)
    }
}

impl<A, S> IntoIterator for GroupedAggregation<A, S>
where
    A: Aggregation,
    S: Stream<Item = crate::Result<StreamItem>>,
{
    type Item = (String, Aggregator<A, S>);
    type IntoIter = std::collections::hash_map::IntoIter<String, Aggregator<A, S>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
