//! Team UI widgets.
//!
//! This module contains UI components for team collaboration:
//!
//! - `TeamMenu`: Team member list with presence indicators
//! - `TeamStatusWidget`: Connection status indicator for the status bar

pub mod team_menu;
pub mod team_status;

pub use team_menu::{MemberPresence, TeamMember, TeamMenu, TeamMenuAction};
pub use team_status::{TeamStatusInfo, TeamStatusWidget, WsState};
