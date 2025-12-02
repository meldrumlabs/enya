//! Wheel-based aggregate index using µWheel.
//!
//! µWheel's `RwWheel` is `!Send` and `!Sync` by design (it uses `Rc<RefCell<>>`
//! internally for performance). This module provides a thread-safe wrapper that
//! confines all wheel operations to a single dedicated thread.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────┐    channel     ┌──────────────────┐
//! │  Async callers   │ ─────────────► │  Wheel thread    │
//! │  (any thread)    │                │  (single thread) │
//! │                  │ ◄───────────── │  owns all wheels │
//! └──────────────────┘   oneshot      └──────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use enya_common::aggregators::DDSketchAggregator;
use tokio::sync::{mpsc, oneshot};
use uwheel::aggregator::sum::U64SumAggregator;
use uwheel::{Conf, Entry, HawConf, RwWheel};

use crate::SeriesId;

/// The kind of metric being tracked, which determines the wheel type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricKind {
    /// Counter or gauge - uses sum aggregation.
    Sum,
    /// Histogram/latency - uses `DDSketch` for percentile queries.
    Histogram,
}

/// Commands sent to the wheel thread.
enum Command {
    /// Insert a value into a wheel.
    Insert {
        series_id: SeriesId,
        value: f64,
        kind: MetricKind,
        ts_ms: u64,
    },
    /// Advance all wheels to the given watermark.
    Tick { watermark_ms: u64 },
    /// Query sum over last N seconds.
    QuerySum {
        series_id: SeriesId,
        seconds: u64,
        reply: oneshot::Sender<Option<f64>>,
    },
    /// Query percentile over last N seconds.
    QueryPercentile {
        series_id: SeriesId,
        seconds: u64,
        percentile: f64,
        reply: oneshot::Sender<Option<f64>>,
    },
    /// Remove a wheel.
    Remove {
        series_id: SeriesId,
        reply: oneshot::Sender<bool>,
    },
    /// Get the number of wheels.
    Len { reply: oneshot::Sender<usize> },
    /// Get total size in bytes.
    SizeBytes { reply: oneshot::Sender<usize> },
    /// Shutdown the wheel thread.
    Shutdown,
}

/// A wheel for a specific metric type.
enum Wheel {
    /// Sum aggregator wheel (for counters/gauges).
    Sum(RwWheel<U64SumAggregator>),
    /// `DDSketch` aggregator wheel (for histograms/latencies).
    Histogram(RwWheel<DDSketchAggregator>),
}

/// Wheel-based aggregate index for fast time-range queries.
///
/// This index maintains pre-computed aggregates using µWheel's hierarchical
/// wheel structure. It uses wall-clock time as its watermark, with a background
/// ticker calling [`tick`](Self::tick) every second to advance all wheels.
///
/// # Thread Safety
///
/// `WheelIndex` is `Send + Sync` and can be shared across threads. Internally,
/// all wheel operations are serialized through a dedicated thread that owns the
/// actual wheel data structures.
///
/// # Example
///
/// ```ignore
/// use enya_metrics_store::index::{WheelIndex, MetricKind};
///
/// let index = WheelIndex::new();
///
/// // Insert values
/// index.insert(series_id, 42.0, MetricKind::Sum).await;
/// index.insert(latency_series_id, 150.0, MetricKind::Histogram).await;
///
/// // Tick to advance watermark (call every 1 second)
/// index.tick().await;
///
/// // Query aggregates
/// let sum = index.query_sum(series_id, 60).await; // last 60 seconds
/// let p99 = index.query_percentile(latency_series_id, 60, 0.99).await;
/// ```
pub struct WheelIndex {
    /// Channel to send commands to the wheel thread.
    tx: mpsc::UnboundedSender<Command>,
    /// Current watermark in milliseconds since epoch (cached for fast access).
    watermark_ms: AtomicU64,
    /// Handle to the wheel thread (for cleanup).
    _thread_handle: JoinHandle<()>,
}

impl WheelIndex {
    /// Creates a new wheel index with the current system time as the initial watermark.
    #[must_use]
    pub fn new() -> Self {
        let now_ms = current_time_ms();
        let (tx, rx) = mpsc::unbounded_channel();

        let thread_handle = thread::spawn(move || {
            wheel_thread_main(rx, now_ms);
        });

        Self {
            tx,
            watermark_ms: AtomicU64::new(now_ms),
            _thread_handle: thread_handle,
        }
    }

    /// Returns the current watermark in milliseconds since epoch.
    #[must_use]
    pub fn watermark(&self) -> u64 {
        self.watermark_ms.load(Ordering::Relaxed)
    }

    /// Returns the number of active wheels.
    pub async fn len(&self) -> usize {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(Command::Len { reply: reply_tx });
        reply_rx.await.unwrap_or(0)
    }

    /// Returns true if there are no active wheels.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Inserts a value into the wheel for the given series.
    ///
    /// If no wheel exists for this series, one is created with the current watermark.
    /// Values with timestamps below the current watermark are dropped by µWheel.
    ///
    /// This method is fire-and-forget; it does not wait for the insert to complete.
    pub fn insert(&self, series_id: SeriesId, value: f64, kind: MetricKind) {
        let ts_ms = current_time_ms();
        let _ = self.tx.send(Command::Insert {
            series_id,
            value,
            kind,
            ts_ms,
        });
    }

    /// Advances all wheels to the current system time.
    ///
    /// This should be called periodically (e.g., every 1 second) from a background task.
    /// Advancing the watermark triggers aggregation of buffered values and makes them
    /// available for queries.
    ///
    /// This method is fire-and-forget; it does not wait for the tick to complete.
    pub fn tick(&self) {
        let now_ms = current_time_ms();
        self.watermark_ms.store(now_ms, Ordering::Relaxed);
        let _ = self.tx.send(Command::Tick { watermark_ms: now_ms });
    }

    /// Queries the sum aggregate over the last `seconds` for the given series.
    ///
    /// Returns `None` if no wheel exists for this series or if the wheel is not
    /// a sum aggregator.
    pub async fn query_sum(&self, series_id: SeriesId, seconds: u64) -> Option<f64> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(Command::QuerySum {
            series_id,
            seconds,
            reply: reply_tx,
        });
        reply_rx.await.ok().flatten()
    }

    /// Queries a percentile over the last `seconds` for the given series.
    ///
    /// The percentile `p` should be in the range `[0.0, 1.0]` (e.g., 0.99 for p99).
    ///
    /// Returns `None` if no wheel exists, the wheel is not a histogram,
    /// or the percentile cannot be computed (e.g., empty sketch).
    pub async fn query_percentile(
        &self,
        series_id: SeriesId,
        seconds: u64,
        p: f64,
    ) -> Option<f64> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(Command::QueryPercentile {
            series_id,
            seconds,
            percentile: p,
            reply: reply_tx,
        });
        reply_rx.await.ok().flatten()
    }

    /// Removes the wheel for the given series, returning true if it existed.
    pub async fn remove(&self, series_id: SeriesId) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(Command::Remove {
            series_id,
            reply: reply_tx,
        });
        reply_rx.await.unwrap_or(false)
    }

    /// Returns the approximate memory usage of all wheels in bytes.
    pub async fn size_bytes(&self) -> usize {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(Command::SizeBytes { reply: reply_tx });
        reply_rx.await.unwrap_or(0)
    }

    /// Shuts down the wheel thread gracefully.
    ///
    /// After calling this method, all other methods will fail silently.
    pub fn shutdown(&self) {
        let _ = self.tx.send(Command::Shutdown);
    }
}

impl Default for WheelIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WheelIndex {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Main loop for the wheel thread.
fn wheel_thread_main(mut rx: mpsc::UnboundedReceiver<Command>, initial_watermark: u64) {
    let mut wheels: HashMap<SeriesId, Wheel> = HashMap::new();
    let mut watermark = initial_watermark;

    while let Some(cmd) = rx.blocking_recv() {
        match cmd {
            Command::Insert {
                series_id,
                value,
                kind,
                ts_ms,
            } => {
                let wheel = wheels
                    .entry(series_id)
                    .or_insert_with(|| create_wheel(kind, watermark));

                match wheel {
                    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                    Wheel::Sum(w) => w.insert(Entry::new(value as u64, ts_ms)),
                    Wheel::Histogram(w) => w.insert(Entry::new(value, ts_ms)),
                }
            }

            Command::Tick { watermark_ms } => {
                watermark = watermark_ms;
                for wheel in wheels.values_mut() {
                    match wheel {
                        Wheel::Sum(w) => {
                            w.advance_to(watermark_ms);
                        }
                        Wheel::Histogram(w) => {
                            w.advance_to(watermark_ms);
                        }
                    }
                }
            }

            Command::QuerySum {
                series_id,
                seconds,
                reply,
            } => {
                let result = wheels.get(&series_id).and_then(|wheel| match wheel {
                    #[allow(clippy::cast_possible_wrap)] // seconds won't exceed i64::MAX
                    #[allow(clippy::cast_precision_loss)] // acceptable for metrics display
                    Wheel::Sum(w) => w
                        .read()
                        .interval(uwheel::Duration::seconds(seconds as i64))
                        .map(|v| v as f64),
                    Wheel::Histogram(_) => None,
                });
                let _ = reply.send(result);
            }

            Command::QueryPercentile {
                series_id,
                seconds,
                percentile,
                reply,
            } => {
                let result = wheels.get(&series_id).and_then(|wheel| match wheel {
                    Wheel::Histogram(w) => {
                        #[allow(clippy::cast_possible_wrap)] // seconds won't exceed i64::MAX
                        let partial = w.read().interval(uwheel::Duration::seconds(seconds as i64))?;
                        let sketch = partial.into_sketch();
                        sketch.quantile(percentile).ok().flatten()
                    }
                    Wheel::Sum(_) => None,
                });
                let _ = reply.send(result);
            }

            Command::Remove { series_id, reply } => {
                let existed = wheels.remove(&series_id).is_some();
                let _ = reply.send(existed);
            }

            Command::Len { reply } => {
                let _ = reply.send(wheels.len());
            }

            Command::SizeBytes { reply } => {
                let size: usize = wheels
                    .values()
                    .map(|w| match w {
                        Wheel::Sum(w) => w.size_bytes(),
                        Wheel::Histogram(w) => w.size_bytes(),
                    })
                    .sum();
                let _ = reply.send(size);
            }

            Command::Shutdown => {
                break;
            }
        }
    }
}

/// Creates a new wheel of the appropriate type.
fn create_wheel(kind: MetricKind, watermark: u64) -> Wheel {
    let haw_conf = HawConf::default().with_watermark(watermark);
    let conf = Conf::default().with_haw_conf(haw_conf);

    match kind {
        MetricKind::Sum => Wheel::Sum(RwWheel::with_conf(conf)),
        MetricKind::Histogram => Wheel::Histogram(RwWheel::with_conf(conf)),
    }
}

/// Returns the current system time in milliseconds since the Unix epoch.
#[allow(clippy::cast_possible_truncation)] // u128 millis won't overflow u64 for centuries
fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Spawns a background tokio task that ticks the wheel index every second.
///
/// Returns a handle to the spawned task.
pub fn spawn_ticker(
    wheel_index: std::sync::Arc<WheelIndex>,
) -> tokio::task::JoinHandle<std::convert::Infallible> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            wheel_index.tick();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insert_and_query_sum() {
        let index = WheelIndex::new();
        let series_id = 1;

        // Tick first to establish a baseline watermark
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Wait at least 1 second so inserts happen in the next second bucket
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Insert some values (these will be above the watermark)
        index.insert(series_id, 10.0, MetricKind::Sum);
        index.insert(series_id, 20.0, MetricKind::Sum);
        index.insert(series_id, 30.0, MetricKind::Sum);

        // Give the wheel thread time to process inserts
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Wait another second and tick to advance watermark past the insert timestamps
        tokio::time::sleep(Duration::from_secs(1)).await;
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Query should return sum
        let sum = index.query_sum(series_id, 60).await;
        assert_eq!(sum, Some(60.0));
    }

    #[tokio::test]
    async fn test_insert_and_query_histogram() {
        let index = WheelIndex::new();
        let series_id = 1;

        // Tick first to establish a baseline watermark
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Wait at least 1 second so inserts happen in the next second bucket
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Insert some latency values
        for i in 1..=100 {
            index.insert(series_id, i as f64, MetricKind::Histogram);
        }

        // Give the wheel thread time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Wait another second and tick to advance watermark past the insert timestamps
        tokio::time::sleep(Duration::from_secs(1)).await;
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Query p50 should be around 50
        let p50 = index.query_percentile(series_id, 60, 0.5).await;
        assert!(p50.is_some(), "p50 query returned None");
        let p50_val = p50.unwrap();
        assert!(
            (45.0..=55.0).contains(&p50_val),
            "p50 should be around 50, got {p50_val}"
        );

        // Query p99 should be around 99
        let p99 = index.query_percentile(series_id, 60, 0.99).await;
        assert!(p99.is_some(), "p99 query returned None");
        let p99_val = p99.unwrap();
        assert!(
            (95.0..=100.0).contains(&p99_val),
            "p99 should be around 99, got {p99_val}"
        );
    }

    #[tokio::test]
    async fn test_wrong_metric_kind_returns_none() {
        let index = WheelIndex::new();
        let series_id = 1;

        // Insert as sum
        index.insert(series_id, 10.0, MetricKind::Sum);
        tokio::time::sleep(Duration::from_millis(50)).await;
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Query as histogram should return None
        let p99 = index.query_percentile(series_id, 60, 0.99).await;
        assert!(p99.is_none());
    }

    #[tokio::test]
    async fn test_remove_wheel() {
        let index = WheelIndex::new();
        let series_id = 1;

        index.insert(series_id, 10.0, MetricKind::Sum);
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(index.len().await, 1);

        let removed = index.remove(series_id).await;
        assert!(removed);
        assert_eq!(index.len().await, 0);

        // Removing again should return false
        let removed = index.remove(series_id).await;
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_concurrent_inserts() {
        use std::sync::Arc;

        let index = Arc::new(WheelIndex::new());

        // Tick first to establish a baseline watermark
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Wait at least 1 second so inserts happen in the next second bucket
        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut handles = vec![];

        // Spawn multiple tasks inserting concurrently
        for i in 0..10 {
            let idx = index.clone();
            handles.push(tokio::spawn(async move {
                for j in 0..100 {
                    idx.insert(i, j as f64, MetricKind::Sum);
                }
            }));
        }

        // Wait for all inserts
        for h in handles {
            h.await.unwrap();
        }

        // Give the wheel thread time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Wait another second and tick to advance watermark past the insert timestamps
        tokio::time::sleep(Duration::from_secs(1)).await;
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Should have 10 series
        assert_eq!(index.len().await, 10);

        // Each series should have sum of 0+1+...+99 = 4950
        for i in 0..10 {
            let sum = index.query_sum(i, 60).await;
            assert_eq!(sum, Some(4950.0), "series {i} should have sum 4950");
        }
    }

    #[tokio::test]
    async fn test_query_nonexistent_series() {
        let index = WheelIndex::new();

        // Query a series that was never inserted
        let sum = index.query_sum(999, 60).await;
        assert!(sum.is_none(), "nonexistent series should return None");

        let p99 = index.query_percentile(999, 60, 0.99).await;
        assert!(p99.is_none(), "nonexistent series should return None");
    }

    #[tokio::test]
    async fn test_multiple_series_isolation() {
        let index = WheelIndex::new();

        // Tick to establish baseline
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Insert different values into different series
        index.insert(1, 100.0, MetricKind::Sum);
        index.insert(2, 200.0, MetricKind::Sum);
        index.insert(3, 300.0, MetricKind::Sum);

        tokio::time::sleep(Duration::from_millis(50)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Each series should have its own isolated value
        assert_eq!(index.query_sum(1, 60).await, Some(100.0));
        assert_eq!(index.query_sum(2, 60).await, Some(200.0));
        assert_eq!(index.query_sum(3, 60).await, Some(300.0));
    }

    #[tokio::test]
    async fn test_histogram_multiple_percentiles() {
        let index = WheelIndex::new();
        let series_id = 1;

        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Insert values 1-1000
        for i in 1..=1000 {
            index.insert(series_id, i as f64, MetricKind::Histogram);
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Test various percentiles
        let p10 = index.query_percentile(series_id, 60, 0.1).await.unwrap();
        let p50 = index.query_percentile(series_id, 60, 0.5).await.unwrap();
        let p90 = index.query_percentile(series_id, 60, 0.9).await.unwrap();
        let p99 = index.query_percentile(series_id, 60, 0.99).await.unwrap();

        // Verify ordering: p10 < p50 < p90 < p99
        assert!(p10 < p50, "p10 ({p10}) should be less than p50 ({p50})");
        assert!(p50 < p90, "p50 ({p50}) should be less than p90 ({p90})");
        assert!(p90 < p99, "p90 ({p90}) should be less than p99 ({p99})");

        // Verify approximate values (with DDSketch error margin)
        assert!(
            (90.0..=110.0).contains(&p10),
            "p10 should be around 100, got {p10}"
        );
        assert!(
            (450.0..=550.0).contains(&p50),
            "p50 should be around 500, got {p50}"
        );
        assert!(
            (850.0..=950.0).contains(&p90),
            "p90 should be around 900, got {p90}"
        );
    }

    #[tokio::test]
    async fn test_size_bytes() {
        let index = WheelIndex::new();

        // Empty index should have zero or minimal size
        let initial_size = index.size_bytes().await;

        // Add some data
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;

        for i in 0..10 {
            index.insert(i, 100.0, MetricKind::Sum);
        }

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Size should have increased
        let new_size = index.size_bytes().await;
        assert!(
            new_size > initial_size,
            "size should increase after inserts: {initial_size} -> {new_size}"
        );
    }

    #[tokio::test]
    async fn test_watermark_advances() {
        let index = WheelIndex::new();

        let initial_watermark = index.watermark();
        assert!(initial_watermark > 0, "initial watermark should be set");

        // Wait and tick
        tokio::time::sleep(Duration::from_secs(1)).await;
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let new_watermark = index.watermark();
        assert!(
            new_watermark > initial_watermark,
            "watermark should advance: {initial_watermark} -> {new_watermark}"
        );
    }

    #[tokio::test]
    async fn test_is_empty() {
        let index = WheelIndex::new();

        assert!(index.is_empty().await, "new index should be empty");

        index.insert(1, 10.0, MetricKind::Sum);
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(!index.is_empty().await, "index should not be empty after insert");

        index.remove(1).await;
        assert!(index.is_empty().await, "index should be empty after remove");
    }

    #[tokio::test]
    async fn test_mixed_metric_kinds() {
        let index = WheelIndex::new();

        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Insert sum metric
        index.insert(1, 100.0, MetricKind::Sum);
        // Insert histogram metric
        index.insert(2, 50.0, MetricKind::Histogram);

        tokio::time::sleep(Duration::from_millis(50)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        index.tick();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Sum queries work on sum series
        assert_eq!(index.query_sum(1, 60).await, Some(100.0));
        // Sum queries don't work on histogram series
        assert!(index.query_sum(2, 60).await.is_none());

        // Percentile queries work on histogram series
        assert!(index.query_percentile(2, 60, 0.5).await.is_some());
        // Percentile queries don't work on sum series
        assert!(index.query_percentile(1, 60, 0.5).await.is_none());
    }

    #[tokio::test]
    async fn test_default_impl() {
        let index = WheelIndex::default();
        assert!(index.is_empty().await);
        assert!(index.watermark() > 0);
    }
}
