//! Workspace configuration types, re-exported from `enya-config`.
//!
//! This module re-exports all workspace configuration types from the
//! `enya-config` crate and adds editor-specific extension traits
//! for converting between config strings and editor enum types.

pub use enya_config::*;

use crate::components::{Granularity, QueryState, TimeRangePreset, VisualizationType};
use crate::ui::theme::AppTheme;

// =============================================================================
// Extension traits for editor-specific conversions
// =============================================================================

/// Extension methods for `PaneConfig` that require editor types.
pub trait PaneConfigExt {
    /// Set granularity from the editor's `Granularity` enum
    fn with_granularity(self, gran: Granularity) -> Self;
    /// Set visualization from the editor's `VisualizationType` enum
    fn with_visualization(self, viz: VisualizationType) -> Self;
    /// Convert granularity string to editor's `Granularity` enum
    fn granularity_value(&self) -> Granularity;
    /// Convert visualization string to editor's `VisualizationType` enum
    fn visualization_type(&self) -> VisualizationType;
    /// Convert to editor's `QueryState`
    fn to_query_state(&self, time_preset: &str) -> QueryState;
}

impl PaneConfigExt for PaneConfig {
    fn with_granularity(mut self, gran: Granularity) -> Self {
        self.granularity = gran.label().to_string();
        self
    }

    fn with_visualization(mut self, viz: VisualizationType) -> Self {
        self.visualization = viz.as_str().to_string();
        self
    }

    fn granularity_value(&self) -> Granularity {
        match self.granularity.to_lowercase().as_str() {
            "1m" => Granularity::OneMinute,
            "5m" => Granularity::FiveMinutes,
            "15m" => Granularity::FifteenMinutes,
            "1h" => Granularity::OneHour,
            "6h" => Granularity::SixHours,
            "1d" => Granularity::OneDay,
            _ => Granularity::FiveMinutes,
        }
    }

    fn visualization_type(&self) -> VisualizationType {
        VisualizationType::parse(&self.visualization)
    }

    fn to_query_state(&self, time_preset: &str) -> QueryState {
        QueryState {
            granularity: self.granularity_value(),
            time_range_label: time_preset.to_string(),
        }
    }
}

/// Extension methods for `TimeConfig` that require editor types.
pub trait TimeConfigExt {
    /// Convert preset string to editor's `TimeRangePreset` enum
    fn to_preset(&self) -> TimeRangePreset;
}

impl TimeConfigExt for TimeConfig {
    fn to_preset(&self) -> TimeRangePreset {
        match self.preset.to_lowercase().as_str() {
            "5m" => TimeRangePreset::Last5Minutes,
            "15m" => TimeRangePreset::Last15Minutes,
            "30m" => TimeRangePreset::Last30Minutes,
            "1h" => TimeRangePreset::Last1Hour,
            "6h" => TimeRangePreset::Last6Hours,
            "24h" => TimeRangePreset::Last24Hours,
            "7d" => TimeRangePreset::Last7Days,
            _ => TimeRangePreset::Last15Minutes,
        }
    }
}

/// Extension methods for `ViewConfig` that require editor types.
pub trait ViewConfigExt {
    /// Convert theme string to editor's `AppTheme` enum
    fn app_theme(&self) -> AppTheme;
}

impl ViewConfigExt for ViewConfig {
    fn app_theme(&self) -> AppTheme {
        AppTheme::parse(&self.theme).unwrap_or_default()
    }
}

// =============================================================================
// Free functions for constructors that depend on editor types
// =============================================================================

/// Create a `PaneConfig` from a query and `QueryState`
pub fn pane_from_query_state(
    query: &str,
    name: &str,
    tag: &str,
    description: &str,
    state: &QueryState,
) -> PaneConfig {
    PaneConfig {
        query: query.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        tag: tag.to_string(),
        unit: String::new(),
        granularity: state.granularity.label().to_string(),
        visualization: "time_series".to_string(),
    }
}

/// Create a `PaneConfig` from a query, `QueryState`, and `VisualizationType`
pub fn pane_from_query_state_with_viz(
    query: &str,
    name: &str,
    tag: &str,
    description: &str,
    state: &QueryState,
    viz_type: VisualizationType,
) -> PaneConfig {
    PaneConfig {
        query: query.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        tag: tag.to_string(),
        unit: String::new(),
        granularity: state.granularity.label().to_string(),
        visualization: viz_type.as_str().to_string(),
    }
}

/// Create a `TimeConfig` from a `TimeRangePreset`
pub fn time_config_from_preset(preset: TimeRangePreset) -> TimeConfig {
    TimeConfig {
        preset: preset.label().to_string(),
        refresh: String::new(),
    }
}

/// Create a `TimeConfig` from a `TimeRangePreset` with a `RefreshInterval`
pub fn time_config_from_preset_with_refresh(
    preset: TimeRangePreset,
    refresh: RefreshInterval,
) -> TimeConfig {
    TimeConfig {
        preset: preset.label().to_string(),
        refresh: refresh.to_string(),
    }
}

// =============================================================================
// Tests for extension traits
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_workspace() {
        let toml = r#"
[workspace]
name = "prod-dashboard"
description = "Production monitoring"
version = 1

[view]
theme = "light"
zen_mode = false

[time]
preset = "1h"

[[panes]]
query = "avg(env:prod AND service:api) by (service)"
name = "API Requests"
granularity = "1m"

[[panes]]
query = "sum(env:prod AND name:error_rate) by (name)"
granularity = "5m"
"#;
        let ws = WorkspaceConfig::from_toml(toml).unwrap();
        assert_eq!(ws.workspace.name, "prod-dashboard");
        assert_eq!(ws.view.theme, "light");
        assert_eq!(ws.view.app_theme(), AppTheme::Light);
        assert_eq!(ws.time.preset, "1h");
        assert_eq!(ws.panes.len(), 2);
        assert_eq!(
            ws.panes[0].query,
            "avg(env:prod AND service:api) by (service)"
        );
        assert_eq!(ws.panes[0].name, "API Requests");
        assert_eq!(ws.panes[0].granularity, "1m");
        assert_eq!(ws.panes[1].granularity, "5m");
    }

    #[test]
    fn test_roundtrip() {
        let mut ws = WorkspaceConfig::new("test");
        ws.workspace.description = "Test workspace".to_string();
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("avg(env:prod) by (service)")
                .with_name("Production")
                .with_granularity(Granularity::OneMinute),
        );

        let toml = ws.to_toml().unwrap();
        let parsed = WorkspaceConfig::from_toml(&toml).unwrap();

        assert_eq!(parsed.workspace.name, "test");
        assert_eq!(parsed.workspace.description, "Test workspace");
        assert_eq!(parsed.view.theme, "light");
        assert_eq!(parsed.time.preset, "1h");
        assert_eq!(parsed.panes.len(), 1);
        assert_eq!(parsed.panes[0].name, "Production");
    }

    #[test]
    fn test_base64_encoding_with_panes() {
        let mut ws = WorkspaceConfig::new("dashboard");
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("sum(env:prod AND service:api) by (service)")
                .with_name("API Latency")
                .with_tag("Critical")
                .with_granularity(Granularity::OneMinute),
        );
        ws.add_pane(PaneConfig::new("env:prod AND name:error_rate"));

        let encoded = ws.to_base64().unwrap();
        assert!(encoded.starts_with('p'));

        let decoded = WorkspaceConfig::from_base64(&encoded).unwrap();
        assert_eq!(decoded.workspace.name, "dashboard");
        assert_eq!(decoded.view.theme, "light");
        assert_eq!(decoded.time.preset, "1h");
        assert_eq!(decoded.panes.len(), 2);
        assert_eq!(
            decoded.panes[0].query,
            "sum(env:prod AND service:api) by (service)"
        );
        assert_eq!(decoded.panes[0].name, "API Latency");
        assert_eq!(decoded.panes[0].tag, "Critical");
        assert_eq!(decoded.panes[0].granularity, "1m");
        assert_eq!(decoded.panes[1].query, "env:prod AND name:error_rate");
    }

    #[test]
    fn test_single_pane_encoding() {
        let mut ws = WorkspaceConfig::new("dashboard");
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("sum(env:prod AND service:api) by (service)")
                .with_name("API Latency")
                .with_granularity(Granularity::OneMinute),
        );

        let pane_encoded = ws.pane_to_base64(0).unwrap();
        let ws_encoded = ws.to_base64().unwrap();

        assert!(pane_encoded.starts_with('q'));
        assert!(
            pane_encoded.len() < ws_encoded.len(),
            "single pane ({}) should be shorter than workspace ({})",
            pane_encoded.len(),
            ws_encoded.len()
        );

        let decoded = WorkspaceConfig::from_base64(&pane_encoded).unwrap();
        assert_eq!(decoded.workspace.name, "shared");
        assert_eq!(decoded.view.theme, "light");
        assert_eq!(decoded.time.preset, "1h");
        assert_eq!(decoded.panes.len(), 1);
        assert_eq!(
            decoded.panes[0].query,
            "sum(env:prod AND service:api) by (service)"
        );
        assert_eq!(decoded.panes[0].name, "API Latency");
        assert_eq!(decoded.panes[0].granularity, "1m");
    }

    #[test]
    fn test_pane_config_conversions() {
        let pane = PaneConfig {
            query: "sum(*) by (service)".to_string(),
            name: "Test".to_string(),
            description: String::new(),
            tag: "Critical".to_string(),
            granularity: "15m".to_string(),
            visualization: "time_series".to_string(),
            unit: String::new(),
        };

        assert_eq!(pane.granularity_value(), Granularity::FifteenMinutes);
        assert_eq!(pane.tag, "Critical");

        let state = pane.to_query_state("1h");
        assert_eq!(state.granularity, Granularity::FifteenMinutes);
        assert_eq!(state.time_range_label, "1h");
    }

    #[test]
    fn test_time_config_presets() {
        let cases = [
            ("5m", TimeRangePreset::Last5Minutes),
            ("15m", TimeRangePreset::Last15Minutes),
            ("1h", TimeRangePreset::Last1Hour),
            ("24h", TimeRangePreset::Last24Hours),
            ("7d", TimeRangePreset::Last7Days),
        ];

        for (input, expected) in cases {
            let config = TimeConfig {
                preset: input.to_string(),
                refresh: String::new(),
            };
            assert_eq!(config.to_preset(), expected);
        }
    }

    #[test]
    fn test_time_config_from_preset() {
        let config = time_config_from_preset(TimeRangePreset::Last1Hour);
        assert_eq!(config.preset, "1h");

        let config = time_config_from_preset(TimeRangePreset::Last7Days);
        assert_eq!(config.preset, "7d");
    }

    #[test]
    fn test_time_config_to_preset_all() {
        let cases = [
            ("5m", TimeRangePreset::Last5Minutes),
            ("15m", TimeRangePreset::Last15Minutes),
            ("30m", TimeRangePreset::Last30Minutes),
            ("1h", TimeRangePreset::Last1Hour),
            ("6h", TimeRangePreset::Last6Hours),
            ("24h", TimeRangePreset::Last24Hours),
            ("7d", TimeRangePreset::Last7Days),
            ("invalid", TimeRangePreset::Last15Minutes),
        ];

        for (input, expected) in cases {
            let config = TimeConfig {
                preset: input.to_string(),
                refresh: String::new(),
            };
            assert_eq!(config.to_preset(), expected, "Failed for input: {input}");
        }
    }

    #[test]
    fn test_pane_config_builder() {
        let pane = PaneConfig::new("sum(*) by (host)")
            .with_name("Host Metrics")
            .with_tag("Important")
            .with_granularity(Granularity::OneHour)
            .with_visualization(VisualizationType::Stat);

        assert_eq!(pane.query, "sum(*) by (host)");
        assert_eq!(pane.name, "Host Metrics");
        assert_eq!(pane.tag, "Important");
        assert_eq!(pane.granularity, "1h");
        assert_eq!(pane.visualization, "stat");
    }

    #[test]
    fn test_pane_config_granularity_value_all() {
        let cases = [
            ("1m", Granularity::OneMinute),
            ("5m", Granularity::FiveMinutes),
            ("15m", Granularity::FifteenMinutes),
            ("1h", Granularity::OneHour),
            ("6h", Granularity::SixHours),
            ("1d", Granularity::OneDay),
            ("invalid", Granularity::FiveMinutes),
        ];

        for (input, expected) in cases {
            let pane = PaneConfig {
                query: "test".to_string(),
                name: String::new(),
                description: String::new(),
                tag: String::new(),
                granularity: input.to_string(),
                visualization: "time_series".to_string(),
                unit: String::new(),
            };
            assert_eq!(pane.granularity_value(), expected, "Failed for: {input}");
        }
    }

    #[test]
    fn test_pane_config_from_query_state_with_viz() {
        let state = QueryState {
            granularity: Granularity::OneMinute,
            time_range_label: "1h".to_string(),
        };
        let pane = pane_from_query_state_with_viz(
            "sum(*)",
            "Test",
            "MyTag",
            "Test description",
            &state,
            VisualizationType::Stat,
        );

        assert_eq!(pane.query, "sum(*)");
        assert_eq!(pane.name, "Test");
        assert_eq!(pane.description, "Test description");
        assert_eq!(pane.tag, "MyTag");
        assert_eq!(pane.granularity, "1m");
        assert_eq!(pane.visualization, "stat");
    }

    #[test]
    fn test_view_config_app_theme() {
        let mut config = ViewConfig::default();
        assert_eq!(config.app_theme(), AppTheme::Dark);

        config.theme = "light".to_string();
        assert_eq!(config.app_theme(), AppTheme::Light);

        config.theme = "LIGHT".to_string();
        assert_eq!(config.app_theme(), AppTheme::Light);

        config.theme = "midnight".to_string();
        assert_eq!(config.app_theme(), AppTheme::Midnight);

        config.theme = "invalid".to_string();
        assert_eq!(config.app_theme(), AppTheme::Dark);
    }

    #[test]
    fn test_snapshot_full_workspace_toml() {
        let mut ws = WorkspaceConfig::new("full-dashboard");
        ws.workspace.description = "A comprehensive monitoring dashboard".to_string();
        ws.metrics = MetricsConfig::with_endpoint("https://prometheus.example.com");
        ws.logs = LogsConfig::with_endpoint("https://loki.example.com")
            .with_default_query("{app=\"api\"}");
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("sum(env:prod AND service:api) by (service)")
                .with_name("API Latency")
                .with_tag("Critical")
                .with_granularity(Granularity::OneMinute),
        );
        ws.add_pane(
            PaneConfig::new("avg(env:prod AND name:cpu_usage) by (host)")
                .with_name("CPU Usage")
                .with_granularity(Granularity::FiveMinutes)
                .with_visualization(VisualizationType::Stat),
        );

        let toml = ws.to_toml().unwrap();
        insta::assert_snapshot!(toml, @r#"
        [workspace]
        name = "full-dashboard"
        description = "A comprehensive monitoring dashboard"

        [metrics]
        endpoint = "https://prometheus.example.com"

        [logs]
        endpoint = "https://loki.example.com"
        default_query = '{app="api"}'

        [view]
        theme = "light"

        [time]
        preset = "1h"

        [[panes]]
        query = "sum(env:prod AND service:api) by (service)"
        name = "API Latency"
        tag = "Critical"
        granularity = "1m"

        [[panes]]
        query = "avg(env:prod AND name:cpu_usage) by (host)"
        name = "CPU Usage"
        visualization = "stat"
        "#);
    }

    #[test]
    fn test_snapshot_pane_config_toml() {
        let pane = PaneConfig::new("sum(*) by (host)")
            .with_name("Host Metrics")
            .with_tag("Critical")
            .with_granularity(Granularity::OneHour)
            .with_visualization(VisualizationType::Gauge);

        insta::assert_toml_snapshot!(pane, @r#"
        query = 'sum(*) by (host)'
        name = 'Host Metrics'
        tag = 'Critical'
        granularity = '1h'
        visualization = 'gauge'
        "#);
    }

    #[test]
    fn test_snapshot_base64_encoding_stability() {
        let mut ws = WorkspaceConfig::new("shared");
        ws.view.theme = "light".to_string();
        ws.time.preset = "1h".to_string();
        ws.add_pane(
            PaneConfig::new("sum(env:prod) by (service)")
                .with_name("Production")
                .with_granularity(Granularity::FiveMinutes),
        );

        let encoded = ws.to_base64().unwrap();
        insta::assert_snapshot!(encoded, @"pNAAAAPAlBnNoYXJlZAsBGnN1bShlbnY6cHJvZCkgYnkgKHNlcnZpY2UpAQpQcm9kdWN0aW9uAAEAAA");
    }
}
