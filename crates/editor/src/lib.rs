#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub mod command;
pub mod components;
pub mod connection;
pub mod dashboard;
mod theme;
pub mod ui;
pub mod util;
pub mod wgpu;
pub mod workspace;
mod workspace_tabs;

pub use workspace_tabs::{TabBarAction, WorkspaceTab, WorkspaceTabBar};

pub use app::EnyaApp;
