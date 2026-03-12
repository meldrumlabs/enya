//! Enya daemon/infrastructure configuration.
//!
//! Separate from workspace configuration, [`Config`] holds settings for the
//! Enya daemon: datasource endpoints, server bind settings, etc.
//!
//! Stored at `~/.enya/config.toml` by default.

use serde::{Deserialize, Serialize};

/// A datasource endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Datasource {
    /// Endpoint URL (e.g. "http://localhost:9090").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,

    /// API key / bearer token (optional).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,
}

impl Datasource {
    pub fn is_empty(&self) -> bool {
        self.url.is_empty() && self.api_key.is_empty()
    }
}

/// Datasource connections.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Datasources {
    /// Prometheus endpoint.
    #[serde(default, skip_serializing_if = "Datasource::is_empty")]
    pub prometheus: Datasource,

    /// Loki endpoint.
    #[serde(default, skip_serializing_if = "Datasource::is_empty")]
    pub loki: Datasource,

    /// Tempo endpoint.
    #[serde(default, skip_serializing_if = "Datasource::is_empty")]
    pub tempo: Datasource,
}

/// Server bind configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Address to bind to.
    #[serde(default = "default_bind")]
    pub bind: String,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            port: default_port(),
            bind: default_bind(),
        }
    }
}

fn default_port() -> u16 {
    3030
}

fn default_bind() -> String {
    "127.0.0.1".to_string()
}

/// OTLP receiver configuration.
///
/// When enabled, Enya accepts OpenTelemetry data directly via OTLP HTTP
/// endpoints (`/v1/traces`, `/v1/logs`, `/v1/metrics`), storing it in memory.
/// This lets developers point their OTel SDKs at Enya without needing separate
/// Prometheus/Loki/Tempo infrastructure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpReceiver {
    /// Whether the OTLP receiver is enabled (default: false).
    #[serde(default)]
    pub enabled: bool,

    /// Maximum number of traces to store in memory (default: 1000).
    #[serde(default = "default_max_traces")]
    pub max_traces: usize,

    /// Maximum number of log entries to store in memory (default: 50000).
    #[serde(default = "default_max_log_entries")]
    pub max_log_entries: usize,

    /// Maximum number of data points per metric time series (default: 10000).
    #[serde(default = "default_max_metric_data_points")]
    pub max_metric_data_points: usize,
}

impl Default for OtlpReceiver {
    fn default() -> Self {
        Self {
            enabled: false,
            max_traces: default_max_traces(),
            max_log_entries: default_max_log_entries(),
            max_metric_data_points: default_max_metric_data_points(),
        }
    }
}

impl OtlpReceiver {
    /// Returns true if this config is all defaults (for skip_serializing_if).
    pub fn is_default(&self) -> bool {
        !self.enabled
            && self.max_traces == default_max_traces()
            && self.max_log_entries == default_max_log_entries()
            && self.max_metric_data_points == default_max_metric_data_points()
    }
}

fn default_max_traces() -> usize {
    1000
}

fn default_max_log_entries() -> usize {
    50_000
}

fn default_max_metric_data_points() -> usize {
    10_000
}

/// Top-level Enya daemon configuration.
///
/// Stored at `~/.enya/config.toml`. Separate from workspace files
/// which live at `~/.enya/workspaces/*.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Datasource endpoints.
    #[serde(default)]
    pub datasources: Datasources,

    /// Server bind settings.
    #[serde(default)]
    pub server: Server,

    /// OTLP receiver settings.
    #[serde(default, skip_serializing_if = "OtlpReceiver::is_default")]
    pub otlp: OtlpReceiver,
}

#[cfg(not(target_arch = "wasm32"))]
impl Config {
    /// Load from a TOML file path.
    pub fn load(path: &std::path::Path) -> Result<Self, crate::workspace::WorkspaceError> {
        let content =
            std::fs::read_to_string(path).map_err(crate::workspace::WorkspaceError::Io)?;
        toml::from_str(&content).map_err(crate::workspace::WorkspaceError::Parse)
    }

    /// Load from `~/.enya/config.toml`, or return defaults if the file doesn't exist.
    pub fn load_or_default() -> Self {
        let path = crate::dir::config_path();
        if path.exists() {
            Self::load(&path).unwrap_or_default()
        } else {
            Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let config = Config::default();
        assert_eq!(config.server.port, 3030);
        assert_eq!(config.server.bind, "127.0.0.1");
        assert!(config.datasources.prometheus.is_empty());
        assert!(config.datasources.loki.is_empty());
        assert!(config.datasources.tempo.is_empty());
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
[datasources.prometheus]
url = "http://prometheus:9090"
api_key = "secret"

[datasources.loki]
url = "http://loki:3100"

[server]
port = 8080
bind = "0.0.0.0"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.datasources.prometheus.url, "http://prometheus:9090");
        assert_eq!(config.datasources.prometheus.api_key, "secret");
        assert_eq!(config.datasources.loki.url, "http://loki:3100");
        assert!(config.datasources.tempo.is_empty());
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.bind, "0.0.0.0");
    }

    #[test]
    fn test_empty_toml() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.server.port, 3030);
        assert_eq!(config.server.bind, "127.0.0.1");
    }

    #[test]
    fn test_otlp_config() {
        let toml = r#"
[otlp]
enabled = true
max_traces = 2000
max_log_entries = 100000
max_metric_data_points = 20000
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.otlp.enabled);
        assert_eq!(config.otlp.max_traces, 2000);
        assert_eq!(config.otlp.max_log_entries, 100_000);
        assert_eq!(config.otlp.max_metric_data_points, 20_000);
    }

    #[test]
    fn test_otlp_defaults() {
        let config = Config::default();
        assert!(!config.otlp.enabled);
        assert_eq!(config.otlp.max_traces, 1000);
        assert_eq!(config.otlp.max_log_entries, 50_000);
        assert_eq!(config.otlp.max_metric_data_points, 10_000);
        assert!(config.otlp.is_default());
    }

    #[test]
    fn test_otlp_skip_serializing_when_default() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(!toml_str.contains("otlp"));
    }

    #[test]
    fn test_roundtrip() {
        let config = Config {
            datasources: Datasources {
                prometheus: Datasource {
                    url: "http://prometheus:9090".to_string(),
                    api_key: String::new(),
                },
                ..Default::default()
            },
            server: Server::default(),
            otlp: OtlpReceiver::default(),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.datasources.prometheus.url, "http://prometheus:9090");
        assert!(parsed.datasources.prometheus.api_key.is_empty());
    }

    #[test]
    fn test_skip_empty_fields() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        // Empty datasources should be omitted
        assert!(!toml_str.contains("prometheus"));
        assert!(!toml_str.contains("loki"));
        assert!(!toml_str.contains("tempo"));
    }
}
