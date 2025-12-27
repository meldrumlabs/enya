//! Prometheus backend implementation.
//!
//! This module provides a [`PrometheusClient`] that executes PromQL queries
//! against a Prometheus HTTP API.

pub mod client;
pub mod response;

pub use client::PrometheusClient;
