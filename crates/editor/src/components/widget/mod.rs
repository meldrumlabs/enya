//! Widget components - smaller reusable UI elements.

pub mod agent_input_bar;
pub mod buffer;
pub mod landing_page;
pub mod notifications;
pub mod status_line;
pub mod time_range;

pub use agent_input_bar::{
    AgentInputBar, AgentInputBarResult, AgentInputState, ContextPane, QuickCommand,
};
pub use buffer::{Buffer, BufferAction, BufferMode};
pub use landing_page::{LandingPage, LandingPageAction};
pub use notifications::{Notification, NotificationLevel, NotificationManager};
pub use status_line::{Sparkline, StatusLine, StatusMode};
pub use time_range::{TimeRange, TimeRangePreset, TimeRangeToolbar};
