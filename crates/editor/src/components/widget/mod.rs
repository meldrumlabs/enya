//! Widget components - smaller reusable UI elements.

pub mod agent_input_bar;
pub mod buffer;
pub mod landing_page;
pub mod notifications;
pub mod status_line;
pub mod thinking_indicator;
pub mod time_range;
#[cfg(not(target_arch = "wasm32"))]
pub mod update_banner;

pub use agent_input_bar::{
    AgentInputBar, AgentInputBarResult, AgentInputState, ContextPane, QuickCommand,
};
pub use buffer::{Buffer, BufferAction, BufferMode};
pub use landing_page::{LandingPage, LandingPageAction};
pub use notifications::{Notification, NotificationLevel, NotificationManager};
pub use status_line::{InlineAgentInput, Sparkline, StatusLine, StatusLineResult, StatusMode};
pub use thinking_indicator::{ThinkingBanner, ThinkingIndicator, ThinkingStage};
pub use time_range::{TimeRange, TimeRangePreset, TimeRangeToolbar};
#[cfg(not(target_arch = "wasm32"))]
pub use update_banner::{UpdateBanner, UpdateBannerAction};
