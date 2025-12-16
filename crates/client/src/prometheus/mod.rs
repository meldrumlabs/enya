//! Prometheus backend implementation.
//!
//! This module provides a [`PrometheusClient`] that translates enya-lang queries
//! to PromQL and executes them against a Prometheus HTTP API.

pub mod client;
pub mod response;
pub mod translate;

pub use client::PrometheusClient;
