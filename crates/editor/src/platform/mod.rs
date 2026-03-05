//! Platform-specific integrations (macOS vibrancy, URL schemes, etc.)

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "macos")]
mod shell_path;

#[cfg(target_os = "macos")]
mod url_handler;

#[cfg(target_os = "macos")]
pub use shell_path::resolve_shell_environment;

#[cfg(target_os = "macos")]
pub use url_handler::{drain_pending_urls, init_url_handler};
