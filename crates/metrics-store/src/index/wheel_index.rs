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
//!
//! The wheel thread owns:
//! - Per-series numeric/histogram wheels for aggregation queries
//! - A global bloom filter wheel for temporal tag existence checks

use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use enya_common::aggregators::DDSketchAggregator;
use rustc_hash::{FxBuildHasher, FxHashMap};
use tokio::sync::{mpsc, oneshot};
use uwheel::aggregator::bloom::BloomAggregator;
use uwheel::aggregator::sum::U64SumAggregator;
use uwheel::{Conf, Entry, HawConf, RwWheel, WheelRange};

use crate::SeriesId;

/// Hash type for tag terms in the bloom filter.
type TagTermHash = u64;

/// Bloom filter aggregator for tag term hashes.
///
/// Configuration:
/// - 65,536 bits (~8KB per wheel slot)
/// - 6 hash functions
/// - Custom seed for deterministic hashing
type TagBloomAggregator = BloomAggregator<TagTermHash, 65_536, 6, 0xE19A_B100_F1A7>;

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
    /// Record a tag term in the bloom filter.
    RecordTag { term_hash: TagTermHash, ts_ms: u64 },
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
    /// Check if a tag term may have been seen in the last N seconds.
    MayContainTag {
        term_hash: TagTermHash,
        seconds: u64,
        reply: oneshot::Sender<bool>,
    },
    /// Check if a tag term may have been seen in a time range.
    MayContainTagRange {
        term_hash: TagTermHash,
        start_ms: u64,
        end_ms: u64,
        reply: oneshot::Sender<bool>,
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
/// # Bloom Filter for Tag Existence
///
/// In addition to per-series numeric/histogram wheels, the index maintains a
/// global bloom filter wheel for tracking tag terms over time. This enables
/// fast probabilistic membership testing for time-range queries - if the bloom
/// filter says a tag wasn't seen, it definitely wasn't (no false negatives).
///
/// Tag terms should be in the format `{metric}#{key}:{value}`.
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
/// // Record tag terms for bloom filter
/// index.record_tag("cpu.usage#env:prod");
///
/// // Tick to advance watermark (call every 1 second)
/// index.tick().await;
///
/// // Query aggregates
/// let sum = index.query_sum(series_id, 60).await; // last 60 seconds
/// let p99 = index.query_percentile(latency_series_id, 60, 0.99).await;
///
/// // Check if a tag was seen in the last 60 seconds
/// let may_exist = index.may_contain_tag("cpu.usage#env:prod", 60).await;
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

    /// Inserts a value into the wheel for the given series at a specific timestamp.
    ///
    /// If no wheel exists for this series, one is created with the current watermark.
    /// Values with timestamps below the current watermark are dropped by µWheel.
    ///
    /// This method is fire-and-forget; it does not wait for the insert to complete.
    pub fn insert_at(&self, series_id: SeriesId, value: f64, kind: MetricKind, ts_ms: u64) {
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
        let _ = self.tx.send(Command::Tick {
            watermark_ms: now_ms,
        });
    }

    /// Advances all wheels to the specified watermark timestamp.
    ///
    /// This is useful for testing where you want to control time precisely.
    /// Advancing the watermark triggers aggregation of buffered values and makes them
    /// available for queries.
    ///
    /// This method is fire-and-forget; it does not wait for the tick to complete.
    pub fn tick_to(&self, watermark_ms: u64) {
        self.watermark_ms.store(watermark_ms, Ordering::Relaxed);
        let _ = self.tx.send(Command::Tick { watermark_ms });
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
    pub async fn query_percentile(&self, series_id: SeriesId, seconds: u64, p: f64) -> Option<f64> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send(Command::QueryPercentile {
            series_id,
            seconds,
            percentile: p,
            reply: reply_tx,
        });
        reply_rx.await.ok().flatten()
    }

    /// Records a tag term in the bloom filter at the current time.
    ///
    /// The term should be in the format `{metric}#{key}:{value}`.
    ///
    /// This method is fire-and-forget; it does not wait for the record to complete.
    pub fn record_tag(&self, term: &str) {
        let ts_ms = current_time_ms();
        let term_hash = hash_term(term);
        let _ = self.tx.send(Command::RecordTag { term_hash, ts_ms });
    }

    /// Records a tag term in the bloom filter at the specified timestamp.
    ///
    /// The term should be in the format `{metric}#{key}:{value}`.
    ///
    /// This method is fire-and-forget; it does not wait for the record to complete.
    pub fn record_tag_at(&self, term: &str, ts_ms: u64) {
        let term_hash = hash_term(term);
        let _ = self.tx.send(Command::RecordTag { term_hash, ts_ms });
    }

    /// Records a tag term in the bloom filter at the current time, without allocating.
    ///
    /// This is equivalent to `record_tag(&format!("{metric}#{key}:{value}"))` but
    /// avoids the intermediate string allocation by hashing the components directly.
    ///
    /// This method is fire-and-forget; it does not wait for the record to complete.
    pub fn record_tag_components(&self, metric: &str, key: &str, value: &str) {
        let ts_ms = current_time_ms();
        let term_hash = hash_term_components(metric, key, value);
        let _ = self.tx.send(Command::RecordTag { term_hash, ts_ms });
    }

    /// Records a tag term in the bloom filter at a specific timestamp, without allocating.
    ///
    /// This is equivalent to `record_tag_at(&format!("{metric}#{key}:{value}"), ts_ms)` but
    /// avoids the intermediate string allocation by hashing the components directly.
    ///
    /// This method is fire-and-forget; it does not wait for the record to complete.
    pub fn record_tag_components_at(&self, metric: &str, key: &str, value: &str, ts_ms: u64) {
        let term_hash = hash_term_components(metric, key, value);
        let _ = self.tx.send(Command::RecordTag { term_hash, ts_ms });
    }

    /// Checks if a tag term may have been seen in the last `seconds`.
    ///
    /// Returns `true` if the term was possibly seen (may be a false positive),
    /// or `false` if it was definitely not seen.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Check if "cpu.usage#env:prod" was seen in the last 60 seconds
    /// let may_exist = index.may_contain_tag("cpu.usage#env:prod", 60).await;
    /// if may_exist {
    ///     // Tag might exist, perform actual storage lookup
    /// } else {
    ///     // Tag definitely doesn't exist in this time range, skip lookup
    /// }
    /// ```
    pub async fn may_contain_tag(&self, term: &str, seconds: u64) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        let term_hash = hash_term(term);
        let _ = self.tx.send(Command::MayContainTag {
            term_hash,
            seconds,
            reply: reply_tx,
        });
        reply_rx.await.unwrap_or(false)
    }

    /// Checks if a tag term may have been seen in the specified time range.
    ///
    /// The range is `[start_ms, end_ms)` (start inclusive, end exclusive).
    ///
    /// Returns `true` if the term was possibly seen (may be a false positive),
    /// or `false` if it was definitely not seen.
    pub async fn may_contain_tag_range(&self, term: &str, start_ms: u64, end_ms: u64) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        let term_hash = hash_term(term);
        let _ = self.tx.send(Command::MayContainTagRange {
            term_hash,
            start_ms,
            end_ms,
            reply: reply_tx,
        });
        reply_rx.await.unwrap_or(false)
    }

    /// Checks if a tag term may have been seen in the last `seconds`, without allocating.
    ///
    /// This is equivalent to `may_contain_tag(&format!("{metric}#{key}:{value}"), seconds)`
    /// but uses the same component-based hashing as `record_tag_components`.
    ///
    /// Returns `true` if the term was possibly seen (may be a false positive),
    /// or `false` if it was definitely not seen.
    pub async fn may_contain_tag_components(
        &self,
        metric: &str,
        key: &str,
        value: &str,
        seconds: u64,
    ) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        let term_hash = hash_term_components(metric, key, value);
        let _ = self.tx.send(Command::MayContainTag {
            term_hash,
            seconds,
            reply: reply_tx,
        });
        reply_rx.await.unwrap_or(false)
    }

    /// Checks if a tag term may have been seen in the specified time range, without allocating.
    ///
    /// This is equivalent to `may_contain_tag_range(&format!("{metric}#{key}:{value}"), start_ms, end_ms)`
    /// but uses the same component-based hashing as `record_tag_components`.
    ///
    /// The range is `[start_ms, end_ms)` (start inclusive, end exclusive).
    ///
    /// Returns `true` if the term was possibly seen (may be a false positive),
    /// or `false` if it was definitely not seen.
    pub async fn may_contain_tag_range_components(
        &self,
        metric: &str,
        key: &str,
        value: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        let term_hash = hash_term_components(metric, key, value);
        let _ = self.tx.send(Command::MayContainTagRange {
            term_hash,
            start_ms,
            end_ms,
            reply: reply_tx,
        });
        reply_rx.await.unwrap_or(false)
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
#[allow(clippy::too_many_lines)] // Command dispatch requires handling all variants in one place
fn wheel_thread_main(mut rx: mpsc::UnboundedReceiver<Command>, initial_watermark: u64) {
    let mut wheels: FxHashMap<SeriesId, Wheel> = FxHashMap::default();
    let mut watermark = initial_watermark;

    // Create the global bloom filter wheel for tag existence checks
    let haw_conf = HawConf::default().with_watermark(initial_watermark);
    let conf = Conf::default().with_haw_conf(haw_conf);
    let mut bloom_wheel: RwWheel<TagBloomAggregator> = RwWheel::with_conf(conf);

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

            Command::RecordTag { term_hash, ts_ms } => {
                bloom_wheel.insert(Entry::new(term_hash, ts_ms));
            }

            Command::Tick { watermark_ms } => {
                watermark = watermark_ms;
                // Advance per-series wheels
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
                // Advance the bloom wheel
                bloom_wheel.advance_to(watermark_ms);
            }

            Command::QuerySum {
                series_id,
                seconds,
                reply,
            } => {
                let result = wheels.get(&series_id).and_then(|wheel| match wheel {
                    #[allow(clippy::cast_possible_wrap)] // seconds won't exceed i64::MAX
                    #[allow(clippy::cast_precision_loss)]
                    // acceptable for metrics display
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
                        let partial = w
                            .read()
                            .interval(uwheel::Duration::seconds(seconds as i64))?;
                        let sketch = partial.into_sketch();
                        sketch.quantile(percentile).ok().flatten()
                    }
                    Wheel::Sum(_) => None,
                });
                let _ = reply.send(result);
            }

            Command::MayContainTag {
                term_hash,
                seconds,
                reply,
            } => {
                #[allow(clippy::cast_possible_wrap)] // seconds won't exceed i64::MAX
                let result = bloom_wheel
                    .read()
                    .interval(uwheel::Duration::seconds(seconds as i64))
                    .is_some_and(|partial| partial.contains(&term_hash));
                let _ = reply.send(result);
            }

            Command::MayContainTagRange {
                term_hash,
                start_ms,
                end_ms,
                reply,
            } => {
                let result = bloom_wheel
                    .read()
                    .combine_range(WheelRange::new_unchecked(start_ms, end_ms))
                    .is_some_and(|partial| partial.contains(&term_hash));
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
                let series_size: usize = wheels
                    .values()
                    .map(|w| match w {
                        Wheel::Sum(w) => w.size_bytes(),
                        Wheel::Histogram(w) => w.size_bytes(),
                    })
                    .sum();
                let bloom_size = bloom_wheel.size_bytes();
                let _ = reply.send(series_size + bloom_size);
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
#[must_use]
#[allow(clippy::cast_possible_truncation)] // u128 millis won't overflow u64 for centuries
pub fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Hashes a tag term to a u64 for the bloom filter.
///
/// Uses `FxHasher` from `rustc-hash` which is fast and deterministic
/// (unlike `DefaultHasher` which can change between Rust versions).
///
/// This writes raw bytes to ensure compatibility with `hash_term_components`.
fn hash_term(term: &str) -> TagTermHash {
    use std::hash::Hasher;

    let mut hasher = FxBuildHasher.build_hasher();
    hasher.write(term.as_bytes());
    hasher.finish()
}

/// Hashes tag term components directly without allocating a string.
///
/// This writes the raw bytes of each component to the hasher, producing
/// the same hash as if the components were concatenated into a single string.
fn hash_term_components(metric: &str, key: &str, value: &str) -> TagTermHash {
    use std::hash::Hasher;

    let mut hasher = FxBuildHasher.build_hasher();
    hasher.write(metric.as_bytes());
    hasher.write(b"#");
    hasher.write(key.as_bytes());
    hasher.write(b":");
    hasher.write(value.as_bytes());
    hasher.finish()
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Small delay to let the wheel thread process commands.
    const PROCESS_DELAY: Duration = Duration::from_millis(10);

    #[tokio::test]
    async fn test_insert_and_query_sum() {
        let index = WheelIndex::new();
        let series_id = 1;
        let base_time = current_time_ms();

        // Insert some values at a specific time
        index.insert_at(series_id, 10.0, MetricKind::Sum, base_time + 1000);
        index.insert_at(series_id, 20.0, MetricKind::Sum, base_time + 1000);
        index.insert_at(series_id, 30.0, MetricKind::Sum, base_time + 1000);

        // Give the wheel thread time to process inserts
        tokio::time::sleep(PROCESS_DELAY).await;

        // Advance watermark past the insert timestamps
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Query should return sum
        let sum = index.query_sum(series_id, 60).await;
        assert_eq!(sum, Some(60.0));
    }

    #[tokio::test]
    async fn test_insert_and_query_histogram() {
        let index = WheelIndex::new();
        let series_id = 1;
        let base_time = current_time_ms();

        // Insert some latency values at a specific time
        for i in 1..=100 {
            index.insert_at(
                series_id,
                f64::from(i),
                MetricKind::Histogram,
                base_time + 1000,
            );
        }

        // Give the wheel thread time to process
        tokio::time::sleep(PROCESS_DELAY).await;

        // Advance watermark past the insert timestamps
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Query p50 should be around 50
        let p50 = index.query_percentile(series_id, 60, 0.5).await;
        assert!(p50.is_some(), "p50 query returned None");
        let p50_val = p50.expect("p50 should be Some");
        assert!(
            (45.0..=55.0).contains(&p50_val),
            "p50 should be around 50, got {p50_val}"
        );

        // Query p99 should be around 99
        let p99 = index.query_percentile(series_id, 60, 0.99).await;
        assert!(p99.is_some(), "p99 query returned None");
        let p99_val = p99.expect("p99 should be Some");
        assert!(
            (95.0..=100.0).contains(&p99_val),
            "p99 should be around 99, got {p99_val}"
        );
    }

    #[tokio::test]
    async fn test_wrong_metric_kind_returns_none() {
        let index = WheelIndex::new();
        let series_id = 1;
        let base_time = current_time_ms();

        // Insert as sum
        index.insert_at(series_id, 10.0, MetricKind::Sum, base_time + 1000);
        tokio::time::sleep(PROCESS_DELAY).await;
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Query as histogram should return None
        let p99 = index.query_percentile(series_id, 60, 0.99).await;
        assert!(p99.is_none());
    }

    #[tokio::test]
    async fn test_remove_wheel() {
        let index = WheelIndex::new();
        let series_id = 1;
        let base_time = current_time_ms();

        index.insert_at(series_id, 10.0, MetricKind::Sum, base_time + 1000);
        tokio::time::sleep(PROCESS_DELAY).await;

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
        let base_time = current_time_ms();

        let mut handles = vec![];

        // Spawn multiple tasks inserting concurrently at specific timestamp
        for i in 0..10 {
            let idx = index.clone();
            handles.push(tokio::spawn(async move {
                for j in 0..100 {
                    idx.insert_at(i, f64::from(j), MetricKind::Sum, base_time + 1000);
                }
            }));
        }

        // Wait for all inserts
        for h in handles {
            h.await.expect("task should complete");
        }

        // Give the wheel thread time to process
        tokio::time::sleep(PROCESS_DELAY).await;

        // Advance watermark past the insert timestamps
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

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
        let base_time = current_time_ms();

        // Insert different values into different series at a specific time
        index.insert_at(1, 100.0, MetricKind::Sum, base_time + 1000);
        index.insert_at(2, 200.0, MetricKind::Sum, base_time + 1000);
        index.insert_at(3, 300.0, MetricKind::Sum, base_time + 1000);

        tokio::time::sleep(PROCESS_DELAY).await;
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Each series should have its own isolated value
        assert_eq!(index.query_sum(1, 60).await, Some(100.0));
        assert_eq!(index.query_sum(2, 60).await, Some(200.0));
        assert_eq!(index.query_sum(3, 60).await, Some(300.0));
    }

    #[tokio::test]
    async fn test_histogram_multiple_percentiles() {
        let index = WheelIndex::new();
        let series_id = 1;
        let base_time = current_time_ms();

        // Insert values 1-1000 at a specific time
        for i in 1..=1000 {
            index.insert_at(
                series_id,
                f64::from(i),
                MetricKind::Histogram,
                base_time + 1000,
            );
        }

        tokio::time::sleep(PROCESS_DELAY).await;
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Test various percentiles
        let p10 = index
            .query_percentile(series_id, 60, 0.1)
            .await
            .expect("p10 should exist");
        let p50 = index
            .query_percentile(series_id, 60, 0.5)
            .await
            .expect("p50 should exist");
        let p90 = index
            .query_percentile(series_id, 60, 0.9)
            .await
            .expect("p90 should exist");
        let p99 = index
            .query_percentile(series_id, 60, 0.99)
            .await
            .expect("p99 should exist");

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
        let base_time = current_time_ms();

        // Empty index should have zero or minimal size
        let initial_size = index.size_bytes().await;

        // Add some data at a specific time
        for i in 0..10 {
            index.insert_at(i, 100.0, MetricKind::Sum, base_time + 1000);
        }

        tokio::time::sleep(PROCESS_DELAY).await;

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

        // Advance watermark using tick_to
        index.tick_to(initial_watermark + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        let new_watermark = index.watermark();
        assert!(
            new_watermark > initial_watermark,
            "watermark should advance: {initial_watermark} -> {new_watermark}"
        );
    }

    #[tokio::test]
    async fn test_is_empty() {
        let index = WheelIndex::new();
        let base_time = current_time_ms();

        assert!(index.is_empty().await, "new index should be empty");

        index.insert_at(1, 10.0, MetricKind::Sum, base_time + 1000);
        tokio::time::sleep(PROCESS_DELAY).await;

        assert!(
            !index.is_empty().await,
            "index should not be empty after insert"
        );

        index.remove(1).await;
        assert!(index.is_empty().await, "index should be empty after remove");
    }

    #[tokio::test]
    async fn test_mixed_metric_kinds() {
        let index = WheelIndex::new();
        let base_time = current_time_ms();

        // Insert sum metric
        index.insert_at(1, 100.0, MetricKind::Sum, base_time + 1000);
        // Insert histogram metric
        index.insert_at(2, 50.0, MetricKind::Histogram, base_time + 1000);

        tokio::time::sleep(PROCESS_DELAY).await;
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

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

    #[tokio::test]
    async fn test_bloom_record_and_query() {
        let index = WheelIndex::new();
        let base_time = current_time_ms();

        // Record some tag terms at specific times
        index.record_tag_at("cpu.usage#env:prod", base_time + 1000);
        index.record_tag_at("cpu.usage#service:db", base_time + 1000);
        index.record_tag_at("memory.used#env:prod", base_time + 1000);

        // Give the wheel thread time to process
        tokio::time::sleep(PROCESS_DELAY).await;

        // Advance watermark
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Recorded terms should be found
        assert!(index.may_contain_tag("cpu.usage#env:prod", 60).await);
        assert!(index.may_contain_tag("cpu.usage#service:db", 60).await);
        assert!(index.may_contain_tag("memory.used#env:prod", 60).await);

        // Non-recorded terms should NOT be found (no false negatives)
        assert!(!index.may_contain_tag("disk.io#env:staging", 60).await);
        assert!(!index.may_contain_tag("network.bytes#service:web", 60).await);
    }

    #[tokio::test]
    async fn test_bloom_time_range_query() {
        let index = WheelIndex::new();
        let base_time = current_time_ms();

        // Record at specific times
        index.record_tag_at("cpu.usage#env:prod", base_time + 1000);
        index.record_tag_at("memory.used#env:staging", base_time + 5000);

        // Give the wheel thread time to process
        tokio::time::sleep(PROCESS_DELAY).await;

        // Manually advance watermark past the records
        index.tick_to(base_time + 10_000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Query specific range that includes first record
        let found = index
            .may_contain_tag_range("cpu.usage#env:prod", base_time, base_time + 3000)
            .await;
        assert!(found, "should find cpu.usage#env:prod in [base, base+3s]");

        // Query specific range that includes second record
        let found = index
            .may_contain_tag_range(
                "memory.used#env:staging",
                base_time + 4000,
                base_time + 7000,
            )
            .await;
        assert!(
            found,
            "should find memory.used#env:staging in [base+4s, base+7s]"
        );
    }

    #[tokio::test]
    async fn test_bloom_concurrent_records() {
        use std::sync::Arc;

        let index = Arc::new(WheelIndex::new());
        let base_time = current_time_ms();

        let mut handles = vec![];

        // Spawn multiple tasks recording concurrently at specific timestamp
        for i in 0..10 {
            let idx = index.clone();
            handles.push(tokio::spawn(async move {
                for j in 0..100 {
                    let term = format!("metric{i}#tag:value{j}");
                    idx.record_tag_at(&term, base_time + 1000);
                }
            }));
        }

        // Wait for all records
        for h in handles {
            h.await.expect("task should complete");
        }

        // Give the wheel thread time to process
        tokio::time::sleep(PROCESS_DELAY).await;

        // Advance watermark
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Check some of the recorded terms
        assert!(index.may_contain_tag("metric0#tag:value0", 60).await);
        assert!(index.may_contain_tag("metric5#tag:value50", 60).await);
        assert!(index.may_contain_tag("metric9#tag:value99", 60).await);
    }

    #[test]
    fn test_hash_term_components_deterministic() {
        // Verify that hash_term_components produces consistent results
        // for the same input.

        let test_cases = [
            ("cpu.usage", "env", "prod"),
            ("memory.used", "host", "server1"),
            ("network.bytes", "service", "api"),
            ("disk.io", "region", "us-east-1"),
            ("", "key", "value"),    // empty metric
            ("metric", "", "value"), // empty key
            ("metric", "key", ""),   // empty value
            ("a", "b", "c"),         // short strings
            (
                "metric.with.dots",
                "key-with-dashes",
                "value_with_underscores",
            ),
        ];

        for (metric, key, value) in test_cases {
            let hash1 = hash_term_components(metric, key, value);
            let hash2 = hash_term_components(metric, key, value);

            assert_eq!(
                hash1, hash2,
                "hash_term_components should be deterministic for ({metric}, {key}, {value})"
            );
        }

        // Different inputs should produce different hashes (no collisions for these test cases)
        let hash_a = hash_term_components("cpu.usage", "env", "prod");
        let hash_b = hash_term_components("cpu.usage", "env", "staging");
        let hash_c = hash_term_components("memory.used", "env", "prod");

        assert_ne!(hash_a, hash_b, "different values should hash differently");
        assert_ne!(hash_a, hash_c, "different metrics should hash differently");
    }

    #[tokio::test]
    async fn test_record_tag_components_and_may_contain_tag_components() {
        // Verify that recording via record_tag_components can be queried
        // via may_contain_tag_components (both use component-based hashing).
        let index = WheelIndex::new();
        let base_time = current_time_ms();

        // Record using record_tag_at with component-style strings
        index.record_tag_at("cpu.usage#env:prod", base_time + 1000);
        index.record_tag_at("memory.used#host:server1", base_time + 1000);

        // Give the wheel thread time to process
        tokio::time::sleep(PROCESS_DELAY).await;

        // Advance watermark
        index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Query using may_contain_tag (should find the records)
        assert!(index.may_contain_tag("cpu.usage#env:prod", 60).await);
        assert!(index.may_contain_tag("memory.used#host:server1", 60).await);

        // Non-recorded terms should NOT be found
        assert!(!index.may_contain_tag("cpu.usage#env:staging", 60).await);
    }
}
