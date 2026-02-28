//! Style tab enum shared by settings overlay and settings page.

/// Which panel is currently focused (Theme or Font)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StyleTab {
    #[default]
    Theme,
    Font,
}
