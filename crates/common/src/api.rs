//! Shared API types for agent-editor communication.
//!
//! These types support both JSON (serde) and bitcode serialization for
//! high-performance binary encoding between Rust applications.

use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// Nanosecond timestamp (same as metrics-store Timestamp)
pub type Timestamp = u128;

/// MIME type for bitcode binary format
pub const BITCODE_MIME: &str = "application/x-bitcode";

/// A single time bucket in a metrics query response.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct MetricsBucket {
    /// Start timestamp (nanoseconds)
    pub start: Timestamp,
    /// End timestamp (nanoseconds)
    pub end: Timestamp,
    /// Aggregated value for this bucket
    pub value: f64,
    /// Number of samples in this bucket
    pub count: usize,
}

/// A group of time buckets, typically representing a unique tag combination.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct MetricsGroup {
    /// Group identifier (e.g., "host:server1,env:prod")
    pub group: String,
    /// Time series buckets for this group
    pub buckets: Vec<MetricsBucket>,
}

/// Response from the `/api/metrics/query` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct QueryResponse {
    /// The metric name that was queried
    pub metric: String,
    /// The original query string
    pub query: String,
    /// Parsed aggregation function (e.g., "sum", "avg")
    pub parsed_agg: Option<String>,
    /// Parsed filter expression (e.g., "env:prod AND host:server1")
    pub parsed_filter: String,
    /// Parsed grouping clause (e.g., "by (host)")
    pub parsed_grouping: Option<String>,
    /// Parsed time range (e.g., "5m")
    pub parsed_time_range: Option<String>,
    /// Query start timestamp (nanoseconds)
    pub start: Option<Timestamp>,
    /// Query end timestamp (nanoseconds)
    pub end: Option<Timestamp>,
    /// Bucket granularity (nanoseconds)
    pub granularity_ns: u128,
    /// Result groups
    pub groups: Vec<MetricsGroup>,
}
