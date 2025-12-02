//! N-way merge for combining multiple sorted async streams

use crate::db::{DataPointStream, StreamItem};
use futures::Stream;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Wrapper for heap ordering (by timestamp, descending)
struct HeapItem {
    item: StreamItem,
    stream: DataPointStream,
    stream_idx: usize,
}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.item.ts == other.item.ts
    }
}

impl Eq for HeapItem {}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for max-heap (we want most recent first)
        other.item.ts.cmp(&self.item.ts)
    }
}

/// State for the async merger
enum MergerState {
    /// Currently initializing streams
    Initializing {
        pending_streams: Vec<DataPointStream>,
    },
    /// Actively merging
    Merging,
    /// Fetching next item from a stream after yielding one
    FetchingNext {
        stream: DataPointStream,
        stream_idx: usize,
    },
    /// Completed or errored
    Done,
}

/// N-way merge stream that combines multiple sorted async streams
pub struct Merger {
    heap: BinaryHeap<HeapItem>,
    state: MergerState,
    error: Option<crate::Error>,
}

impl Merger {
    /// Create a new merger from multiple async streams
    pub fn new(streams: Vec<DataPointStream>) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(streams.len()),
            state: MergerState::Initializing {
                pending_streams: streams,
            },
            error: None,
        }
    }
}

impl Stream for Merger {
    type Item = crate::Result<StreamItem>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Return any pending error
            if let Some(e) = self.error.take() {
                self.state = MergerState::Done;
                return Poll::Ready(Some(Err(e)));
            }

            match std::mem::replace(&mut self.state, MergerState::Done) {
                MergerState::Initializing {
                    mut pending_streams,
                } => {
                    // Initialize heap by polling each stream for its first item
                    // We always process index 0, since swap_remove moves elements
                    while !pending_streams.is_empty() {
                        // SAFETY: We just checked the vector is non-empty
                        #[allow(clippy::indexing_slicing)]
                        let stream = &mut pending_streams[0];
                        match stream.as_mut().poll_next(cx) {
                            Poll::Ready(Some(Ok(item))) => {
                                // Got first item, add to heap
                                let stream = pending_streams.swap_remove(0);
                                let stream_idx = self.heap.len();
                                self.heap.push(HeapItem {
                                    item,
                                    stream,
                                    stream_idx,
                                });
                            }
                            Poll::Ready(Some(Err(e))) => {
                                self.state = MergerState::Done;
                                return Poll::Ready(Some(Err(e)));
                            }
                            Poll::Ready(None) => {
                                // Stream is empty, remove and drop it
                                drop(pending_streams.swap_remove(0));
                            }
                            Poll::Pending => {
                                // Stream not ready, save state and return pending
                                self.state = MergerState::Initializing { pending_streams };
                                return Poll::Pending;
                            }
                        }
                    }

                    // All streams initialized, transition to merging
                    self.state = MergerState::Merging;
                }

                MergerState::Merging => {
                    // Pop the next item from the heap
                    let Some(HeapItem {
                        item,
                        stream,
                        stream_idx,
                    }) = self.heap.pop()
                    else {
                        // Heap is empty, we're done
                        self.state = MergerState::Done;
                        return Poll::Ready(None);
                    };

                    // We'll fetch the next item from this stream on the next poll
                    self.state = MergerState::FetchingNext { stream, stream_idx };
                    return Poll::Ready(Some(Ok(item)));
                }

                MergerState::FetchingNext {
                    mut stream,
                    stream_idx,
                } => {
                    // Try to get the next item from the stream we just yielded from
                    match stream.as_mut().poll_next(cx) {
                        Poll::Ready(Some(Ok(next_item))) => {
                            self.heap.push(HeapItem {
                                item: next_item,
                                stream,
                                stream_idx,
                            });
                            self.state = MergerState::Merging;
                        }
                        Poll::Ready(Some(Err(e))) => {
                            self.error = Some(e);
                            self.state = MergerState::Merging;
                        }
                        Poll::Ready(None) => {
                            // Stream exhausted, don't push back, continue merging
                            self.state = MergerState::Merging;
                        }
                        Poll::Pending => {
                            // Stream not ready, save state and return pending
                            self.state = MergerState::FetchingNext { stream, stream_idx };
                            return Poll::Pending;
                        }
                    }
                }

                MergerState::Done => {
                    return Poll::Ready(None);
                }
            }
        }
    }
}
