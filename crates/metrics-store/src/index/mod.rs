//! Wheel-based aggregate index for fast time-range queries.
//!
//! This module provides a [`WheelIndex`] that maintains pre-computed aggregates
//! using µWheel's hierarchical wheel structure. It supports both numeric aggregations
//! (sum, avg, min, max) and histogram aggregations (`DDSketch` for percentiles).
//!
//! The index also includes a global bloom filter wheel for probabilistic temporal
//! tag existence checks - useful for quickly filtering queries before expensive
//! storage lookups.
//!
//! # Architecture
//!
//! The index uses wall-clock time as its watermark. A background task should call
//! [`WheelIndex::tick`] every second to advance all wheels and commit aggregates
//! to the read path.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     MetricsStore                            │
//! ├─────────────────────────────────────────────────────────────┤
//! │   Raw Data (SlateDB)          │   WheelIndex (uwheel)       │
//! │   ─────────────────────────   │   ─────────────────────────  │
//! │   d:{series_id}{ts} → value   │   wheels: HashMap<SeriesId,  │
//! │   (full resolution)           │            Wheel>            │
//! │                               │   - Per-series wheels        │
//! │                               │   - Auto-rollup to hourly,   │
//! │                               │     daily granularity        │
//! │                               │                               │
//! │                               │   bloom_wheel: RwWheel       │
//! │                               │   ─────────────────────────   │
//! │                               │   - Bloom filter wheel for   │
//! │                               │     temporal tag existence   │
//! │                               │   - Fast query filtering     │
//! └─────────────────────────────────────────────────────────────┘
//! ```

mod wheel_index;

pub use wheel_index::{MetricKind, WheelIndex, spawn_ticker};
