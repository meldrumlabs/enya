//! Streaming aggregator

use super::{Bucket, builder::Builder};
use crate::{Value, db::StreamItem};
use futures::Stream;
use pin_project_lite::pin_project;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Defines an aggregation.
///
/// - `transform` defines what to do with each value (default: Add)
/// - `finish` can transform the result value (default: Identity)
pub trait Aggregation {
    /// Initialize a bucket with the first value
    fn init(value: Value) -> Value {
        value
    }

    /// Transform/accumulate a value into the bucket
    fn transform(accu: Value, x: Value) -> Value {
        accu + x
    }

    /// Finalize the bucket value
    fn finish(bucket: &Bucket) -> Value {
        bucket.value
    }
}

pin_project! {
    /// A streaming aggregator
    ///
    /// Takes in an async stream of data points and emits aggregated buckets.
    pub struct Aggregator<A, S>
    where
        A: Aggregation,
        S: Stream<Item = crate::Result<StreamItem>>,
    {
        bucket_width: crate::Timestamp,
        bucket: Bucket,
        #[pin]
        stream: S,
        done: bool,
        phantom: PhantomData<A>,
    }
}

impl<A, S> Aggregator<A, S>
where
    A: Aggregation,
    S: Stream<Item = crate::Result<StreamItem>>,
{
    /// Create a new aggregator
    pub fn new(builder: &Builder<'_, A>, stream: S) -> Self {
        Self {
            bucket_width: builder.bucket_width,
            bucket: Bucket::default(),
            stream,
            done: false,
            phantom: PhantomData,
        }
    }
}

impl<A, S> Stream for Aggregator<A, S>
where
    A: Aggregation,
    S: Stream<Item = crate::Result<StreamItem>>,
{
    type Item = crate::Result<Bucket>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if *this.done {
            return Poll::Ready(None);
        }

        loop {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(data_point))) => {
                    if this.bucket.len == 0 {
                        // Initialize bucket
                        this.bucket.len = 1;
                        this.bucket.start = data_point.ts;
                        this.bucket.end = data_point.ts;
                        this.bucket.value = A::init(data_point.value);
                        continue;
                    }

                    if (this.bucket.end - data_point.ts) <= *this.bucket_width {
                        // Add to bucket
                        this.bucket.len += 1;
                        this.bucket.value = A::transform(this.bucket.value, data_point.value);
                        this.bucket.start = data_point.ts;
                    } else {
                        // Return bucket, and initialize new bucket with current data point
                        let mut bucket = std::mem::take(this.bucket);
                        bucket.value = A::finish(&bucket);

                        // Initialize new bucket with current data point
                        this.bucket.len = 1;
                        this.bucket.start = data_point.ts;
                        this.bucket.end = data_point.ts;
                        this.bucket.value = A::init(data_point.value);

                        return Poll::Ready(Some(Ok(bucket)));
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    *this.done = true;
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(None) => {
                    // Stream exhausted
                    *this.done = true;
                    if this.bucket.len > 0 {
                        // Return last bucket
                        let mut bucket = std::mem::take(this.bucket);
                        bucket.value = A::finish(&bucket);
                        return Poll::Ready(Some(Ok(bucket)));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
    }
}
