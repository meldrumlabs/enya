//! Loki log backend client.
//!
//! Implements the [`LogsClient`] trait for Grafana Loki.

mod client;
pub mod response;

pub use client::LokiClient;
