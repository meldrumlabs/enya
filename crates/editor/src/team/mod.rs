//! Team collaboration module.
//!
//! This module provides all team collaboration features:
//!
//! - **state**: Team state management, connection handling, member presence
//! - **chat**: Channels, threads, messages, and chat UI
//! - **ui**: Team menu and status widgets
//!
//! # Feature Flag
//!
//! This entire module is behind the `teams` feature flag. Enable it with:
//! ```bash
//! cargo build -p enya-editor --features teams
//! ```
//!
//! # Layout
//!
//! ```text
//! team/
//! ├── mod.rs          # This file - module root and re-exports
//! ├── state.rs        # TeamState, TeamConfig
//! ├── chat/           # Chat system
//! │   ├── mod.rs
//! │   ├── channel.rs
//! │   ├── channels_panel.rs
//! │   ├── chat_view.rs
//! │   ├── message.rs
//! │   ├── state.rs
//! │   ├── theme_helpers.rs
//! │   └── thread.rs
//! └── ui/             # Team UI widgets
//!     ├── mod.rs
//!     ├── team_menu.rs
//!     └── team_status.rs
//! ```

pub mod chat;
pub mod state;
pub mod ui;

// Re-export main types at module root for convenience
pub use state::{TeamConfig, TeamState};

// Re-export chat types
pub use chat::{
    BarData, Channel, ChannelId, ChannelKind, ChannelsPanel, ChannelsPanelAction, ChatMessage,
    ChatMessageAuthor, ChatState, ChatView, ChatViewAction, ChatViewMode, EmbeddedChart,
    InlineBarChart, InlineChart, InlineGauge, InlineStat, InlineTable, InlineVisualization,
    Mention, MentionKind, MessageId, StatTrend, Thread, ThreadId, ThreadStatus,
};
// Re-export pane info types from pane module for backwards compatibility
pub use crate::components::pane::{CommitInfo, PaneInfo, PaneVisualization};

// Re-export UI types
pub use ui::{
    MemberPresence, TeamMember, TeamMenu, TeamMenuAction, TeamStatusInfo, TeamStatusWidget, WsState,
};
