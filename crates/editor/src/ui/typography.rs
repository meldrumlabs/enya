//! Typography constants for the Enya editor.
//!
//! Departure Mono is optimized for 11px rendering.
//! This module provides a consistent type scale based on that recommendation.

use egui::FontId;

/// Base font size - Departure Mono's recommended size for optimal rendering
pub const BASE: f32 = 11.0;

/// Extra small text (tags, badges, hints)
pub const XS: f32 = 10.0;

/// Small text (secondary info, captions)
pub const SM: f32 = 11.0;

/// Medium text (body, default)
pub const MD: f32 = 12.0;

/// Large text (emphasized content)
pub const LG: f32 = 13.0;

/// Extra large text (subheadings)
pub const XL: f32 = 14.0;

/// Heading text (titles, headers)
pub const HEADING: f32 = 16.0;

/// Create a proportional FontId with the given size
#[inline]
pub fn proportional(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Create a monospace FontId with the given size
#[inline]
pub fn monospace(size: f32) -> FontId {
    FontId::monospace(size)
}

// Convenience functions for common font configurations

/// Default body text (11px proportional)
#[inline]
pub fn body() -> FontId {
    proportional(SM)
}

/// Default code/query text (11px monospace)
#[inline]
pub fn code() -> FontId {
    monospace(SM)
}

/// Large code/query text for editors (14px monospace)
#[inline]
pub fn code_lg() -> FontId {
    monospace(XL)
}

/// Small label text (10px proportional)
#[inline]
pub fn label() -> FontId {
    proportional(XS)
}

/// Heading text (16px proportional)
#[inline]
pub fn heading() -> FontId {
    proportional(HEADING)
}
