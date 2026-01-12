//! Grafana Tempo backend implementation.
//!
//! This module provides the [`TempoClient`] implementation for querying
//! distributed traces from Grafana Tempo.

pub mod client;
pub mod response;
pub mod types;

pub use client::{TempoClient, demo_search_results, demo_trace};
