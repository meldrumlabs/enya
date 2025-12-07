//! `MetricsStore` wrapper for the time series database
//!
//! Provides convenient default tag management for metrics ingestion.

use crate::index::{MetricKind, WheelIndex};
use crate::{Database, MetricName, Result, SeriesId, TagSet, Value};
use std::sync::Arc;

/// Tag used to store the Git commit hash that produced the metrics.
pub const GIT_VERSION_TAG_KEY: &str = "git_ver";
/// Tag used to store the Git commit timestamp (in RFC3339 format).
pub const GIT_TIMESTAMP_TAG_KEY: &str = "git_timestamp";

/// Configuration for a metric's wheel index behavior.
#[derive(Debug, Clone)]
pub struct MetricConfig {
    /// The kind of aggregation to use for this metric.
    pub kind: MetricKind,
}

impl MetricConfig {
    /// Creates a new metric config for sum aggregation (counters/gauges).
    #[must_use]
    pub const fn sum() -> Self {
        Self {
            kind: MetricKind::Sum,
        }
    }

    /// Creates a new metric config for histogram aggregation (latencies/distributions).
    #[must_use]
    pub const fn histogram() -> Self {
        Self {
            kind: MetricKind::Histogram,
        }
    }
}

/// A wrapper around [`Database`] that manages default tags and wheel indexing.
///
/// Default tags are automatically appended to every write operation,
/// making it easy to attach metadata like git version and timestamp
/// to all metrics.
///
/// When a [`WheelIndex`] is configured, data is dual-written to both the
/// persistent storage and the in-memory wheel index for fast real-time queries.
#[derive(Clone)]
pub struct MetricsStore {
    inner: Database,
    git_ver: Option<String>,
    git_date: Option<String>,
    default_tags: Vec<(Box<str>, Box<str>)>,
    /// Optional wheel index for pre-computed aggregates.
    wheel_index: Option<Arc<WheelIndex>>,
    /// Metric configurations for determining wheel index behavior.
    metric_configs: Arc<parking_lot::RwLock<crate::HashMap<String, MetricConfig>>>,
}

impl MetricsStore {
    /// Creates a new metrics store that wraps a [`Database`].
    #[must_use]
    pub fn new(inner: Database, git_ver: Option<String>, git_date: Option<String>) -> Self {
        let mut store = Self {
            inner,
            git_ver,
            git_date,
            default_tags: Vec::new(),
            wheel_index: None,
            metric_configs: Arc::new(parking_lot::RwLock::new(crate::HashMap::default())),
        };

        if let Some(ref git_ver) = store.git_ver {
            store.register_default_tag(GIT_VERSION_TAG_KEY, git_ver.clone());
        }

        if let Some(ref git_date) = store.git_date {
            store.register_default_tag(GIT_TIMESTAMP_TAG_KEY, git_date.clone());
        }

        store
    }

    /// Returns a new [`MetricsStore`] with a wheel index for pre-computed aggregates.
    ///
    /// When enabled, all ingested data is dual-written to both the persistent
    /// storage and the in-memory wheel index, enabling fast real-time queries.
    #[must_use]
    pub fn with_wheel_index(mut self, wheel_index: Arc<WheelIndex>) -> Self {
        self.wheel_index = Some(wheel_index);
        self
    }

    /// Returns the wheel index if configured.
    #[must_use]
    pub fn wheel_index(&self) -> Option<&Arc<WheelIndex>> {
        self.wheel_index.as_ref()
    }

    /// Registers a metric configuration.
    ///
    /// This determines how the metric is aggregated in the wheel index.
    /// Metrics without an explicit configuration default to [`MetricKind::Sum`].
    pub fn register_metric(&self, metric_name: &str, config: MetricConfig) {
        self.metric_configs
            .write()
            .insert(metric_name.to_string(), config);
    }

    /// Returns the metric kind for a given metric name.
    ///
    /// Returns [`MetricKind::Sum`] if no explicit configuration is registered.
    #[must_use]
    pub fn metric_kind(&self, metric_name: &str) -> MetricKind {
        self.metric_configs
            .read()
            .get(metric_name)
            .map_or(MetricKind::Sum, |c| c.kind)
    }

    /// Returns the Git metadata associated with this store.
    #[must_use]
    pub fn git_info(&self) -> (&Option<String>, &Option<String>) {
        (&self.git_ver, &self.git_date)
    }

    /// Returns the underlying [`Database`].
    #[must_use]
    pub fn database(&self) -> &Database {
        &self.inner
    }

    /// Returns an iterator over the default tags that will be attached to every write.
    pub fn default_tags(&self) -> impl Iterator<Item = (&str, &str)> {
        self.default_tags
            .iter()
            .map(|(key, value)| (key.as_ref(), value.as_ref()))
    }

    /// Registers a new default tag or overwrites an existing key.
    pub fn register_default_tag<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        let key = key.into();
        let value = value.into();

        if let Some(existing) = self
            .default_tags
            .iter_mut()
            .find(|(candidate, _)| candidate.as_ref() == key)
        {
            *existing = (key.into_boxed_str(), value.into_boxed_str());
        } else {
            self.default_tags
                .push((key.into_boxed_str(), value.into_boxed_str()));
        }
    }

    /// Returns a new [`MetricsStore`] with an additional default tag applied.
    #[must_use]
    pub fn with_default_tag<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.register_default_tag(key, value);
        self
    }

    /// Writes a new data point into the underlying database.
    ///
    /// User-provided tags take precedence over any default tags registered on the store.
    ///
    /// If a wheel index is configured, the data is also written to the wheel index
    /// for fast real-time queries.
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    pub async fn ingest<'a>(
        &self,
        metric: MetricName<'a>,
        value: Value,
        tags: &TagSet<'a>,
    ) -> Result<()> {
        // Build the effective tag set (user tags + default tags)
        let effective_tags: Vec<(&str, &str)> = if self.default_tags.is_empty() {
            tags.to_vec()
        } else {
            let mut merged = Vec::with_capacity(tags.len() + self.default_tags.len());

            // Add default tags that aren't overridden by user tags
            for (key, val) in &self.default_tags {
                if !tags.iter().any(|(k, _)| *k == key.as_ref()) {
                    merged.push((key.as_ref(), val.as_ref()));
                }
            }

            merged.extend(tags.iter().copied());
            merged
        };

        let series_id = self
            .inner
            .write_at(metric, crate::timestamp(), value, &effective_tags)
            .await?;

        // Dual-write to wheel index if configured
        if let Some(ref wheel_index) = self.wheel_index {
            let kind = self.metric_kind(metric.as_str());
            wheel_index.insert(series_id, value, kind);

            // Record user-provided tag terms in the bloom filter for fast existence checks
            // We only index user tags, not internal default tags (like git_ver, git_timestamp)
            // Format: {metric}#{key}:{value} - hashed directly to avoid string allocation
            let metric_str = metric.as_str();
            for (key, val) in tags {
                wheel_index.record_tag_components(metric_str, key, val);
            }
        }

        Ok(())
    }

    /// Ingests a metric data point at a specific timestamp (for testing).
    ///
    /// Like `ingest`, but allows specifying the timestamp for the wheel index.
    /// This is primarily useful for testing where wall-clock time cannot be used.
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    #[cfg(test)]
    pub async fn ingest_at<'a>(
        &self,
        metric: MetricName<'a>,
        value: Value,
        tags: &TagSet<'a>,
        ts_ms: u64,
    ) -> Result<()> {
        // Build the effective tag set (user tags + default tags)
        let effective_tags: Vec<(&str, &str)> = if self.default_tags.is_empty() {
            tags.to_vec()
        } else {
            let mut merged = Vec::with_capacity(tags.len() + self.default_tags.len());

            // Add default tags that aren't overridden by user tags
            for (key, val) in &self.default_tags {
                if !tags.iter().any(|(k, _)| *k == key.as_ref()) {
                    merged.push((key.as_ref(), val.as_ref()));
                }
            }

            merged.extend(tags.iter().copied());
            merged
        };

        let series_id = self
            .inner
            .write_at(metric, crate::timestamp(), value, &effective_tags)
            .await?;

        // Dual-write to wheel index if configured
        if let Some(ref wheel_index) = self.wheel_index {
            let kind = self.metric_kind(metric.as_str());
            wheel_index.insert_at(series_id, value, kind, ts_ms);

            // Record user-provided tag terms in the bloom filter for fast existence checks
            let metric_str = metric.as_str();
            for (key, val) in tags {
                wheel_index.record_tag_components_at(metric_str, key, val, ts_ms);
            }
        }

        Ok(())
    }

    /// Queries the sum aggregate over the last `seconds` for the given series.
    ///
    /// This is a fast query that uses the wheel index if available.
    /// Returns `None` if the wheel index is not configured, the series doesn't exist,
    /// or the series is not configured as a sum metric.
    pub async fn query_sum(&self, series_id: SeriesId, seconds: u64) -> Option<f64> {
        self.wheel_index
            .as_ref()?
            .query_sum(series_id, seconds)
            .await
    }

    /// Queries a percentile over the last `seconds` for the given series.
    ///
    /// This is a fast query that uses the wheel index if available.
    /// The percentile `p` should be in the range `[0.0, 1.0]` (e.g., 0.99 for p99).
    ///
    /// Returns `None` if the wheel index is not configured, the series doesn't exist,
    /// or the series is not configured as a histogram metric.
    pub async fn query_percentile(
        &self,
        series_id: SeriesId,
        seconds: u64,
        percentile: f64,
    ) -> Option<f64> {
        self.wheel_index
            .as_ref()?
            .query_percentile(series_id, seconds, percentile)
            .await
    }

    /// Queries the sum aggregate by metric name and tags.
    ///
    /// This looks up the series ID for the given metric and tags, then queries
    /// the wheel index for the sum over the last `seconds`.
    ///
    /// Returns `None` if:
    /// - The wheel index is not configured
    /// - No series exists for this metric/tags combination
    /// - The series is not configured as a sum metric
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred during series lookup.
    pub async fn query_sum_by_name<'a>(
        &self,
        metric: MetricName<'a>,
        tags: &TagSet<'a>,
        seconds: u64,
    ) -> Result<Option<f64>> {
        let Some(wheel_index) = &self.wheel_index else {
            return Ok(None);
        };

        let Some(series_id) = self.inner.get_series_id(metric, tags).await? else {
            return Ok(None);
        };

        Ok(wheel_index.query_sum(series_id, seconds).await)
    }

    /// Queries a percentile by metric name and tags.
    ///
    /// This looks up the series ID for the given metric and tags, then queries
    /// the wheel index for the percentile over the last `seconds`.
    ///
    /// The percentile `p` should be in the range `[0.0, 1.0]` (e.g., 0.99 for p99).
    ///
    /// Returns `None` if:
    /// - The wheel index is not configured
    /// - No series exists for this metric/tags combination
    /// - The series is not configured as a histogram metric
    ///
    /// # Errors
    ///
    /// Returns error if an I/O error occurred during series lookup.
    pub async fn query_percentile_by_name<'a>(
        &self,
        metric: MetricName<'a>,
        tags: &TagSet<'a>,
        seconds: u64,
        percentile: f64,
    ) -> Result<Option<f64>> {
        let Some(wheel_index) = &self.wheel_index else {
            return Ok(None);
        };

        let Some(series_id) = self.inner.get_series_id(metric, tags).await? else {
            return Ok(None);
        };

        Ok(wheel_index
            .query_percentile(series_id, seconds, percentile)
            .await)
    }

    /// Checks if a tag term may have been seen in the last `seconds`.
    ///
    /// The term should be in the format `{metric}#{key}:{value}`.
    ///
    /// Returns `true` if the term was possibly seen (may be a false positive),
    /// or `false` if it was definitely not seen. Returns `false` if no wheel
    /// index is configured.
    ///
    /// This is useful for quickly filtering queries before performing expensive
    /// storage lookups - if the bloom filter says a tag wasn't seen, we can
    /// skip the lookup entirely.
    pub async fn may_contain_tag(&self, term: &str, seconds: u64) -> bool {
        match &self.wheel_index {
            Some(wheel_index) => wheel_index.may_contain_tag(term, seconds).await,
            None => false,
        }
    }

    /// Checks if a tag term may have been seen in the specified time range.
    ///
    /// The term should be in the format `{metric}#{key}:{value}`.
    /// The range is `[start_ms, end_ms)` (start inclusive, end exclusive).
    ///
    /// Returns `true` if the term was possibly seen (may be a false positive),
    /// or `false` if it was definitely not seen. Returns `false` if no wheel
    /// index is configured.
    pub async fn may_contain_tag_range(&self, term: &str, start_ms: u64, end_ms: u64) -> bool {
        match &self.wheel_index {
            Some(wheel_index) => {
                wheel_index
                    .may_contain_tag_range(term, start_ms, end_ms)
                    .await
            }
            None => false,
        }
    }

    /// Checks if a tag term may have been seen in the last `seconds`, without allocating.
    ///
    /// This is the allocation-free version that takes metric, key, and value separately.
    /// Use this method when querying tags that were recorded via `ingest()`.
    ///
    /// Returns `true` if the term was possibly seen (may be a false positive),
    /// or `false` if it was definitely not seen. Returns `false` if no wheel
    /// index is configured.
    pub async fn may_contain_tag_components(
        &self,
        metric: &str,
        key: &str,
        value: &str,
        seconds: u64,
    ) -> bool {
        match &self.wheel_index {
            Some(wheel_index) => {
                wheel_index
                    .may_contain_tag_components(metric, key, value, seconds)
                    .await
            }
            None => false,
        }
    }

    /// Checks if a tag term may have been seen in the specified time range, without allocating.
    ///
    /// This is the allocation-free version that takes metric, key, and value separately.
    /// Use this method when querying tags that were recorded via `ingest()`.
    ///
    /// The range is `[start_ms, end_ms)` (start inclusive, end exclusive).
    ///
    /// Returns `true` if the term was possibly seen (may be a false positive),
    /// or `false` if it was definitely not seen. Returns `false` if no wheel
    /// index is configured.
    pub async fn may_contain_tag_range_components(
        &self,
        metric: &str,
        key: &str,
        value: &str,
        start_ms: u64,
        end_ms: u64,
    ) -> bool {
        match &self.wheel_index {
            Some(wheel_index) => {
                wheel_index
                    .may_contain_tag_range_components(metric, key, value, start_ms, end_ms)
                    .await
            }
            None => false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::index::current_time_ms;
    use slatedb::object_store::memory::InMemory;
    use std::time::Duration;

    /// Small delay to let the wheel thread process commands.
    const PROCESS_DELAY: Duration = Duration::from_millis(10);

    async fn create_test_store() -> MetricsStore {
        let object_store = Arc::new(InMemory::new());

        // Use fast flush interval for tests
        let db = crate::Database::builder()
            .with_flush_interval(Duration::from_millis(100))
            .open(object_store, "/db")
            .await
            .unwrap();

        let wheel_index = Arc::new(WheelIndex::new());
        MetricsStore::new(db, None, None).with_wheel_index(wheel_index)
    }

    #[tokio::test]
    async fn test_metrics_store_with_wheel_index_sum() {
        let store = create_test_store().await;
        let wheel_index = store.wheel_index().unwrap();
        let base_time = current_time_ms();

        // Ingest some counter values at a specific time
        let metric = MetricName::try_from("test.counter").unwrap();
        let tags: &[(&str, &str)] = &[("host", "h1")];
        store
            .ingest_at(metric, 10.0, tags, base_time + 1000)
            .await
            .unwrap();
        store
            .ingest_at(metric, 20.0, tags, base_time + 1000)
            .await
            .unwrap();
        store
            .ingest_at(metric, 30.0, tags, base_time + 1000)
            .await
            .unwrap();

        // Give the wheel thread time to process
        tokio::time::sleep(PROCESS_DELAY).await;

        // Advance watermark past the insert timestamps
        wheel_index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Verify the wheel index has at least one wheel
        assert!(!wheel_index.is_empty().await);

        // Query by metric name and tags (ergonomic API)
        let sum = store.query_sum_by_name(metric, tags, 60).await.unwrap();
        assert_eq!(sum, Some(60.0));

        // Close the database
        store.database().close().await.unwrap();
    }

    #[tokio::test]
    async fn test_metrics_store_with_wheel_index_histogram() {
        let store = create_test_store().await;
        let wheel_index = store.wheel_index().unwrap();
        let base_time = current_time_ms();

        // Register the metric as a histogram
        store.register_metric("test.latency", MetricConfig::histogram());

        // Ingest first value via full path to establish series in database
        let metric = MetricName::try_from("test.latency").unwrap();
        let tags: &[(&str, &str)] = &[("service", "api")];
        store
            .ingest_at(metric, 1.0, tags, base_time + 1000)
            .await
            .unwrap();

        // Get the series_id that was created
        let series_id = store
            .database()
            .get_series_id(metric, tags)
            .await
            .unwrap()
            .expect("series should exist after ingest");

        // Insert remaining values directly to wheel index (fast, bypasses db writes)
        for i in 2..=100 {
            wheel_index.insert_at(
                series_id,
                f64::from(i),
                MetricKind::Histogram,
                base_time + 1000,
            );
        }

        // Give the wheel thread time to process
        tokio::time::sleep(PROCESS_DELAY).await;

        // Advance watermark past the insert timestamps
        wheel_index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Verify the wheel index has at least one wheel
        assert!(!wheel_index.is_empty().await);

        // Query by metric name and tags (ergonomic API)
        let p50 = store
            .query_percentile_by_name(metric, tags, 60, 0.5)
            .await
            .unwrap();
        assert!(p50.is_some(), "p50 query returned None");
        let p50_val = p50.expect("p50 should be Some");
        assert!(
            (45.0..=55.0).contains(&p50_val),
            "p50 should be around 50, got {p50_val}"
        );

        // Close the database
        store.database().close().await.unwrap();
    }

    #[tokio::test]
    async fn test_metrics_store_metric_kind_default() {
        let store = create_test_store().await;

        // Default should be Sum
        assert_eq!(store.metric_kind("unknown.metric"), MetricKind::Sum);

        // Register as histogram
        store.register_metric("my.latency", MetricConfig::histogram());
        assert_eq!(store.metric_kind("my.latency"), MetricKind::Histogram);

        // Register as sum
        store.register_metric("my.counter", MetricConfig::sum());
        assert_eq!(store.metric_kind("my.counter"), MetricKind::Sum);

        // Close the database to avoid errors
        store.database().close().await.unwrap();
    }

    #[tokio::test]
    async fn test_metrics_store_without_wheel_index() {
        let object_store = Arc::new(InMemory::new());

        let db = crate::Database::builder()
            .with_flush_interval(Duration::from_millis(10))
            .open(object_store, "/db")
            .await
            .unwrap();

        // Create store without wheel index
        let store = MetricsStore::new(db, None, None);

        // wheel_index() should return None
        assert!(store.wheel_index().is_none());

        // Ingestion should still work
        let metric = MetricName::try_from("test.metric").unwrap();
        store
            .ingest(metric, 42.0, &[("env", "test")])
            .await
            .unwrap();

        // Query methods should return None
        assert!(store.query_sum(1, 60).await.is_none());
        assert!(store.query_percentile(1, 60, 0.5).await.is_none());

        // Close the database
        store.database().close().await.unwrap();
    }

    #[tokio::test]
    async fn test_metrics_store_bloom_filter_on_ingest() {
        let store = create_test_store().await;
        let wheel_index = store.wheel_index().unwrap();
        let base_time = current_time_ms();

        // Ingest some metrics with tags at a specific time
        let metric = MetricName::try_from("cpu.usage").unwrap();
        store
            .ingest_at(
                metric,
                50.0,
                &[("host", "server1"), ("env", "prod")],
                base_time + 1000,
            )
            .await
            .unwrap();
        store
            .ingest_at(
                metric,
                60.0,
                &[("host", "server2"), ("env", "staging")],
                base_time + 1000,
            )
            .await
            .unwrap();

        // Give the wheel thread time to process
        tokio::time::sleep(PROCESS_DELAY).await;

        // Advance watermark past the insert timestamps
        wheel_index.tick_to(base_time + 5000);
        tokio::time::sleep(PROCESS_DELAY).await;

        // Bloom filter should contain the ingested tag terms
        // Use may_contain_tag_components since ingest uses record_tag_components
        assert!(
            store
                .may_contain_tag_components("cpu.usage", "host", "server1", 60)
                .await
        );
        assert!(
            store
                .may_contain_tag_components("cpu.usage", "env", "prod", 60)
                .await
        );
        assert!(
            store
                .may_contain_tag_components("cpu.usage", "host", "server2", 60)
                .await
        );
        assert!(
            store
                .may_contain_tag_components("cpu.usage", "env", "staging", 60)
                .await
        );

        // Tags that were never ingested should NOT be found (no false negatives)
        assert!(
            !store
                .may_contain_tag_components("cpu.usage", "host", "server3", 60)
                .await
        );
        assert!(
            !store
                .may_contain_tag_components("memory.used", "host", "server1", 60)
                .await
        );

        // Close the database
        store.database().close().await.unwrap();
    }
}
