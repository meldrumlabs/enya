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

/// Main application entry point and event loop.
mod app;

/// Command system for vim-style `:command` interactions.
pub mod command;

/// UI components organized by category: panes, overlays, widgets, and utilities.
pub mod components;

/// Backend connection management for Prometheus and demo data sources.
pub mod connection;

/// Theme definitions for light and dark mode styling.
mod theme;

/// UI primitives: colors, typography, icons, and design tokens.
pub mod ui;

/// General utilities including WASM-compatible time handling.
pub mod util;

/// GPU-accelerated rendering for high-density visualizations like heatmaps.
pub mod wgpu;

/// Workspace runtime (pane layout) and configuration (serialization).
pub mod workspace;

/// Tab bar for managing multiple workspace tabs.
mod workspace_tabs;

pub use workspace_tabs::{TabBarAction, WorkspaceTab, WorkspaceTabBar};

pub use app::EnyaApp;
