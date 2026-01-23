//! Widget components - smaller reusable UI elements.

pub mod agent_input_bar;
pub mod buffer;
pub mod landing_page;
pub mod notifications;
pub mod status_line;
pub mod team_menu;
pub mod team_status;
pub mod thinking_indicator;
pub mod time_range;

pub use agent_input_bar::{
    AgentInputBar, AgentInputBarResult, AgentInputState, ContextPane, QuickCommand,
};
pub use buffer::{Buffer, BufferAction, BufferMode};
pub use landing_page::{LandingPage, LandingPageAction};
pub use notifications::{Notification, NotificationLevel, NotificationManager};
pub use status_line::{InlineAgentInput, Sparkline, StatusLine, StatusLineResult, StatusMode};
pub use team_menu::{MemberPresence, TeamMember, TeamMenu, TeamMenuAction};
pub use team_status::{TeamStatusInfo, TeamStatusWidget, WsState};
pub use thinking_indicator::{ThinkingBanner, ThinkingIndicator, ThinkingStage};
pub use time_range::{TimeRange, TimeRangePreset, TimeRangeToolbar};
