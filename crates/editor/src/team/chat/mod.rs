//! Team chat and channels module.
//!
//! This module provides the data model and UI components for team collaboration
//! through chat channels and threads. Key features:
//!
//! - Hierarchical channels (like Slack/Zed)
//! - Threads for focused discussions
//! - @mentions for users, agents, and charts
//! - AI agent integration in conversations
//!
//! # Layout: Threads-First (Layout E)
//!
//! The channels panel uses a threads-first approach that surfaces active
//! conversations at the top, ideal for incident response workflows:
//!
//! ```text
//! ┌──────────────────┐
//! │ ACTIVE THREADS   │
//! │ 🔥 P99 incident  │
//! │    #incidents    │
//! │    3 replies     │
//! ├──────────────────┤
//! │ CHANNELS         │
//! │ # general        │
//! │ # incidents   2  │
//! ├──────────────────┤
//! │ TEAM             │
//! │ ● Alice ● You    │
//! └──────────────────┘
//! ```

pub mod channel;
pub mod channels_panel;
pub mod chat_view;
pub mod message;
pub mod state;
pub mod theme_helpers;
pub mod thread;

pub use channel::{Channel, ChannelId, ChannelKind};
pub use channels_panel::{ChannelsPanel, ChannelsPanelAction};
pub use chat_view::{
    BarData, ChatView, ChatViewAction, ChatViewMode, EmbeddedChart, InlineBarChart, InlineChart,
    InlineGauge, InlineStat, InlineTable, InlineVisualization, StatTrend,
};
// Re-export pane info types from pane module for backwards compatibility
pub use crate::components::pane::{CommitInfo, PaneInfo, PaneVisualization};
pub use message::{ChatMessage, ChatMessageAuthor, Mention, MentionKind, MessageId};
pub use state::ChatState;
// Re-export ChatColors from ui module for backwards compatibility
pub use crate::ui::ChatColors;
pub use thread::{Thread, ThreadId, ThreadStatus};
