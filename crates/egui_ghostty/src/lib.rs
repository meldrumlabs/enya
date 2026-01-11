//! Terminal emulator widget for egui using ghostty's VT library.
//!
//! This crate provides a terminal emulator that can be embedded in egui applications.
//! It uses ghostty's virtual terminal library for accurate terminal emulation and
//! portable-pty for cross-platform PTY support.
//!
//! # Example
//!
//! ```ignore
//! use egui_ghostty::{TerminalWidget, TerminalConfig};
//!
//! let config = TerminalConfig::default();
//! let mut terminal = TerminalWidget::new(config, "/bin/zsh").unwrap();
//!
//! // In your egui render loop:
//! terminal.show(ui);
//! ```

mod colors;
mod config;
mod input;
mod session;
mod widget;

pub use colors::{AnsiColor, ColorScheme};
pub use config::TerminalConfig;
pub use session::TerminalSession;
pub use widget::{TerminalResponse, TerminalWidget};

/// Re-export ghostty types that users might need.
pub use ghostty_vt::{KeyModifiers, Rgb};
