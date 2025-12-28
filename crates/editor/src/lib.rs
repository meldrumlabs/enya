//! Enya Editor - A vim-inspired metrics visualization editor.
//!
//! This crate provides the core editor UI built on [egui](https://github.com/emilk/egui),
//! featuring a tile-based workspace layout, PromQL query editing with autocompletion,
//! and multiple visualization types for time-series data.
//!
//! # Architecture
//!
//! The editor is organized around these key concepts:
//!
//! - **Workspace**: A tile-based layout of query panes, similar to vim splits
//! - **Components**: Reusable UI elements (panes, overlays, widgets, utilities)
//! - **Commands**: Vim-style command palette with `:command` syntax

#![warn(clippy::all, rust_2018_idioms)]

/// Async runtime abstraction for cross-platform async execution.
mod async_runtime;
pub use async_runtime::AsyncRuntime;

/// Codebase integration: git repo management and metrics-rs instrumentation discovery.
#[cfg(not(target_arch = "wasm32"))]
pub mod codebase;

/// Main application entry point and event loop.
mod app;

/// Command system for vim-style `:command` interactions.
pub mod command;

/// UI components organized by category: panes, overlays, widgets, and utilities.
pub mod components;

/// Backend connection management for Prometheus and demo data sources.
pub mod connection;

/// UI primitives: colors, typography, icons, design tokens, and theme definitions.
pub mod ui;

/// General utilities including WASM-compatible time handling.
pub mod util;

/// Workspace runtime (pane layout), configuration (serialization), and tab management.
pub mod workspace;

pub use workspace::{TabBarAction, WorkspaceTab, WorkspaceTabBar};

pub use app::EnyaApp;
