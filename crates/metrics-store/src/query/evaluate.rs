//! Filter query evaluation against the metrics store.

use crate::SeriesId;
use crate::smap::SeriesMapping;
use crate::tag_index::TagIndex;
use enya_lang::Node;

/// Compute intersection of multiple sorted series ID vectors.
pub fn intersection(vecs: &[Vec<SeriesId>]) -> Vec<SeriesId> {
    if vecs.is_empty() || vecs.iter().any(Vec::is_empty) {
        return vec![];
    }

    let Some(first_vec) = vecs.first() else {
        return vec![];
    };

    let mut result = Vec::new();

    'outer: for &elem in first_vec {
        if vecs.iter().skip(1).any(|vec| !vec.contains(&elem)) {
            continue 'outer;
        }
        result.push(elem);
    }

    result
}

/// Compute union of multiple series ID vectors.
#[must_use]
pub fn union(vecs: &[Vec<SeriesId>]) -> Vec<SeriesId> {
    let mut result = vec![];

    for vec in vecs {
        result.extend(vec);
    }

    result.sort_unstable();
    result.dedup();

    result
}

/// Evaluate a filter expression to get matching series IDs.
pub async fn evaluate_filter(
    node: &Node<'_>,
    smap: &SeriesMapping,
    tag_index: &TagIndex,
    metric_name: &str,
) -> crate::Result<Vec<SeriesId>> {
    match node {
        Node::AllStar => tag_index.query_eq(metric_name).await,
        Node::Eq(leaf) => {
            tag_index
                .query_eq(&TagIndex::format_key(metric_name, leaf.key, leaf.value))
                .await
        }
        Node::Wildcard(leaf) => {
            tag_index
                .query_prefix(&TagIndex::format_key(metric_name, leaf.key, leaf.value))
                .await
        }
        Node::And(children) => {
            let mut ids = Vec::with_capacity(children.len());
            for child in children {
                ids.push(Box::pin(evaluate_filter(child, smap, tag_index, metric_name)).await?);
            }
            Ok(intersection(&ids))
        }
        Node::Or(children) => {
            let mut ids = Vec::with_capacity(children.len());
            for child in children {
                ids.push(Box::pin(evaluate_filter(child, smap, tag_index, metric_name)).await?);
            }
            Ok(union(&ids))
        }
        Node::Not(node) => {
            let mut all_ids = smap.list_all().await?;
            let excluded = Box::pin(evaluate_filter(node, smap, tag_index, metric_name)).await?;

            for id in excluded {
                all_ids.remove(&id);
            }

            let mut ids: Vec<_> = all_ids.into_iter().collect();
            ids.sort_unstable();
            Ok(ids)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_intersection() {
        assert_eq!(
            [1, 3],
            *intersection(&[vec![1, 2, 3, 4, 5], vec![1, 3, 5], vec![1, 3]]),
        );
    }

    #[test]
    fn test_union() {
        assert_eq!(
            [1, 2, 4, 8],
            *union(&[vec![1, 8], vec![1, 2], vec![1, 2, 4], vec![2, 4, 8]]),
        );
    }
}
