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

pub struct MetricsStore {
    inner: Database,
    git_ver: Option<String>,
    git_date: Option<String>,
}

impl MetricsStore {
    /// Creates a new metrics store that wraps a [`talna::Database`].
    #[must_use]
    pub fn new(inner: Database, git_ver: Option<String>, git_date: Option<String>) -> Self {
        Self {
            inner,
            git_ver,
            git_date,
        }
    }

    /// Returns the Git metadata associated with this store.
    #[must_use]
    pub fn git_info(&self) -> (&Option<String>, &Option<String>) {
        (&self.git_ver, &self.git_date)
    }

    /// Writes a new data point into the underlying database.
    pub fn ingest<'a>(
        &self,
        metric: MetricName<'a>,
        value: Value,
        tags: &TagSet<'a>,
    ) -> TalnaResult<()> {
        self.inner.write(metric, value, tags)
    }
}
