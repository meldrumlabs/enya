//! Platform-specific integration (URL schemes, native APIs, etc.).

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{drain_pending_urls, init_url_handler};
