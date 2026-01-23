//! Plugin-defined pane types.
//!
//! These panes are created by Lua plugins and render custom visualizations.

pub mod chart_pane;
pub mod gauge_pane;
pub mod stat_pane;
pub mod table_pane;

pub use chart_pane::PluginChartPane;
pub use gauge_pane::PluginGaugePane;
pub use stat_pane::PluginStatPane;
pub use table_pane::PluginTablePane;
