//! Integration tests for enya crates.
//!
//! This crate contains integration tests that require external dependencies
//! like Prometheus running in testcontainers.

#[cfg(test)]
mod cloud;
#[cfg(test)]
mod prometheus;
