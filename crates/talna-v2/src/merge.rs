//! N-way merge iterator for combining multiple sorted streams

use crate::db::StreamItem;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Wrapper for heap ordering (by timestamp, descending)
struct HeapItem<I: Iterator<Item = crate::Result<StreamItem>>> {
    item: StreamItem,
    iter: I,
    iter_idx: usize,
}

impl<I: Iterator<Item = crate::Result<StreamItem>>> PartialEq for HeapItem<I> {
    fn eq(&self, other: &Self) -> bool {
        self.item.ts == other.item.ts
    }
}

impl<I: Iterator<Item = crate::Result<StreamItem>>> Eq for HeapItem<I> {}

impl<I: Iterator<Item = crate::Result<StreamItem>>> PartialOrd for HeapItem<I> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<I: Iterator<Item = crate::Result<StreamItem>>> Ord for HeapItem<I> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for max-heap (we want most recent first)
        other.item.ts.cmp(&self.item.ts)
    }
}

/// N-way merge iterator that combines multiple sorted streams
pub struct Merger<I: Iterator<Item = crate::Result<StreamItem>>> {
    heap: BinaryHeap<HeapItem<I>>,
    error: Option<crate::Error>,
}

impl<I: Iterator<Item = crate::Result<StreamItem>>> Merger<I> {
    /// Create a new merger from multiple iterators
    pub fn new(iters: Vec<I>) -> Self {
        let mut merger = Self {
            heap: BinaryHeap::with_capacity(iters.len()),
            error: None,
        };

        // Initialize heap with first item from each iterator
        for (idx, mut iter) in iters.into_iter().enumerate() {
            match iter.next() {
                Some(Ok(item)) => {
                    merger.heap.push(HeapItem {
                        item,
                        iter,
                        iter_idx: idx,
                    });
                }
                Some(Err(e)) => {
                    merger.error = Some(e);
                    return merger;
                }
                None => {
                    // Empty iterator, skip
                }
            }
        }

        merger
    }
}

impl<I: Iterator<Item = crate::Result<StreamItem>>> Iterator for Merger<I> {
    type Item = crate::Result<StreamItem>;

    fn next(&mut self) -> Option<Self::Item> {
        // Return any pending error
        if let Some(e) = self.error.take() {
            return Some(Err(e));
        }

        // Pop the next item from the heap
        let HeapItem {
            item,
            mut iter,
            iter_idx,
        } = self.heap.pop()?;

        // Try to get the next item from this iterator
        match iter.next() {
            Some(Ok(next_item)) => {
                self.heap.push(HeapItem {
                    item: next_item,
                    iter,
                    iter_idx,
                });
            }
            Some(Err(e)) => {
                self.error = Some(e);
            }
            None => {
                // Iterator exhausted, don't push back
            }
        }

        Some(Ok(item))
    }
}
