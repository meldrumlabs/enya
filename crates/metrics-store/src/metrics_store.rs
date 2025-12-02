//! `MetricsStore` wrapper for the time series database
//!
//! Provides convenient default tag management for metrics ingestion.

use crate::{Database, MetricName, Result, TagSet, Value};

/// Tag used to store the Git commit hash that produced the metrics.
pub const GIT_VERSION_TAG_KEY: &str = "git_ver";
/// Tag used to store the Git commit timestamp (in RFC3339 format).
pub const GIT_TIMESTAMP_TAG_KEY: &str = "git_timestamp";

/// A wrapper around [`Database`] that manages default tags.
///
/// Default tags are automatically appended to every write operation,
/// making it easy to attach metadata like git version and timestamp
/// to all metrics.
#[derive(Clone)]
pub struct MetricsStore {
    inner: Database,
    git_ver: Option<String>,
    git_date: Option<String>,
    default_tags: Vec<(Box<str>, Box<str>)>,
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
        };

        if let Some(ref git_ver) = store.git_ver {
            store.register_default_tag(GIT_VERSION_TAG_KEY, git_ver.clone());
        }

        if let Some(ref git_date) = store.git_date {
            store.register_default_tag(GIT_TIMESTAMP_TAG_KEY, git_date.clone());
        }

        store
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
    /// # Errors
    ///
    /// Returns error if an I/O error occurred.
    pub async fn ingest<'a>(
        &self,
        metric: MetricName<'a>,
        value: Value,
        tags: &TagSet<'a>,
    ) -> Result<()> {
        if self.default_tags.is_empty() {
            return self.inner.write(metric, value, tags).await;
        }

        let mut merged_tags = Vec::with_capacity(tags.len() + self.default_tags.len());

        for (key, value) in &self.default_tags {
            if tags.iter().any(|(candidate, _)| *candidate == key.as_ref()) {
                continue;
            }

            merged_tags.push((key.as_ref(), value.as_ref()));
        }

        merged_tags.extend(tags.iter().copied());

        self.inner.write(metric, value, &merged_tags).await
    }
}
