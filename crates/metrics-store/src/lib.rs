//! Ideas for the Metrics Store path

/// Internal tags
/// "git_ver" => "939201309"
/// "git_timestamp" => "2025-10-01 23:10:00"
///
/// "query" => "type_a_query"
/// "server" => "primary"
///
/// value latency
///
///
/// Then we can investigate perf issues per git commit or range of commits.
/// group_by (git_ver, query, server) -> visualize latencies per group.
use talna::{Database, MetricName, Result as TalnaResult, TagSet, Value};

/// Tag used to store the Git commit hash that produced the metrics.
pub const GIT_VERSION_TAG_KEY: &str = "git_ver";
/// Tag used to store the Git commit timestamp (in RFC3339 format).
pub const GIT_TIMESTAMP_TAG_KEY: &str = "git_timestamp";

#[derive(Clone)]
pub struct MetricsStore {
    inner: Database,
    git_ver: Option<String>,
    git_date: Option<String>,
    default_tags: Vec<(Box<str>, Box<str>)>,
}

impl MetricsStore {
    /// Creates a new metrics store that wraps a [`talna::Database`].
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
    pub fn ingest<'a>(
        &self,
        metric: MetricName<'a>,
        value: Value,
        tags: &TagSet<'a>,
    ) -> TalnaResult<()> {
        if self.default_tags.is_empty() {
            return self.inner.write(metric, value, tags);
        }

        let mut merged_tags = Vec::with_capacity(tags.len() + self.default_tags.len());

        for (key, value) in &self.default_tags {
            if tags.iter().any(|(candidate, _)| *candidate == key.as_ref()) {
                continue;
            }

            merged_tags.push((key.as_ref(), value.as_ref()));
        }

        merged_tags.extend(tags.iter().copied());

        self.inner.write(metric, value, &merged_tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talna::{MetricName, tagset};
    use tempfile::TempDir;

    fn temp_store(git_ver: Option<&str>, git_date: Option<&str>) -> (TempDir, MetricsStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = Database::builder().open(dir.path()).expect("database");

        let store = MetricsStore::new(db, git_ver.map(str::to_owned), git_date.map(str::to_owned));

        (dir, store)
    }

    fn metric() -> MetricName<'static> {
        MetricName::try_from("latency").expect("valid metric name")
    }

    #[test]
    fn exposes_git_metadata_as_default_tags() {
        let (_dir, store) = temp_store(Some("abc123"), Some("2024-06-11T12:00:00Z"));

        let defaults: Vec<_> = store.default_tags().collect();
        assert!(defaults.contains(&("git_ver", "abc123")));
        assert!(defaults.contains(&("git_timestamp", "2024-06-11T12:00:00Z")));
    }

    #[test]
    fn ingest_appends_default_tags() {
        let (_dir, mut store) = temp_store(Some("abc123"), None);
        store.register_default_tag("query", "type_a");
        let metric = metric();

        store
            .ingest(metric, 42.0, tagset!("env" => "test"))
            .expect("ingest");

        let results = store
            .database()
            .count(metric, "env")
            .filter("git_ver:abc123 AND query:type_a")
            .build()
            .expect("build query")
            .collect()
            .expect("collect results");

        assert_eq!(results.len(), 1);
        assert!(results.contains_key("test"));
        let buckets = &results["test"];
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].len, 1);
    }

    #[test]
    fn explicit_tag_overrides_default() {
        let (_dir, store) = temp_store(Some("abc123"), None);
        let metric = metric();

        store
            .ingest(metric, 1.0, tagset!("env" => "test", "git_ver" => "custom"))
            .expect("ingest");

        let results = store
            .database()
            .count(metric, "env")
            .filter("git_ver:abc123")
            .build()
            .expect("build query")
            .collect()
            .expect("collect results");

        assert!(results.is_empty());
    }
}
