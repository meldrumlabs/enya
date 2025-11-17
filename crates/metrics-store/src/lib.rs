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
pub struct MetricsStore {
    inner: talna::Database,

    git_ver: Option<String>,
    git_date: Option<String>,
}

impl MetricsStore {
    pub fn new(git_ver: Option<String>, git_date: Option<String>) -> Self {
        unimplemented!();
    }
    pub fn ingest(&self) {}
}
