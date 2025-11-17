#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub mod command;
pub mod components;
pub mod dashboard;
mod theme;
pub mod ui;
pub mod util;

pub use app::EnyaApp;
