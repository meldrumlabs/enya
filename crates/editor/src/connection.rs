//! Agent connection management for the Enya editor.
//!
//! Handles connecting to an Enya agent's REST API via health checks.

use parking_lot::Mutex;
use serde::Deserialize;
use std::sync::Arc;

/// Health response from the agent's `/api/health` endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentHealth {
    pub msg: String,
    pub version: String,
    pub git_hash: String,
    pub git_branch: String,
    pub built_at: String,
    pub build_summary: String,
    #[serde(default)]
    pub metrics_git_version: Option<String>,
    #[serde(default)]
    pub metrics_git_timestamp: Option<String>,
}

/// Connection status to an agent endpoint.
#[derive(Debug, Clone, Default)]
pub enum ConnectionStatus {
    /// Not connected to any agent.
    #[default]
    Disconnected,
    /// Currently attempting to connect.
    Connecting { endpoint: String },
    /// Successfully connected to an agent.
    Connected {
        endpoint: String,
        health: AgentHealth,
    },
    /// Connection attempt failed.
    Failed { endpoint: String, error: String },
}

impl ConnectionStatus {
    /// Returns the endpoint if connected or connecting.
    pub fn endpoint(&self) -> Option<&str> {
        match self {
            ConnectionStatus::Disconnected => None,
            ConnectionStatus::Connecting { endpoint }
            | ConnectionStatus::Connected { endpoint, .. }
            | ConnectionStatus::Failed { endpoint, .. } => Some(endpoint),
        }
    }

    /// Returns true if currently connected.
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionStatus::Connected { .. })
    }

    /// Returns true if a connection attempt is in progress.
    pub fn is_connecting(&self) -> bool {
        matches!(self, ConnectionStatus::Connecting { .. })
    }
}

/// Error that can occur during connection.
#[derive(Debug, Clone)]
pub struct ConnectionError {
    pub message: String,
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Result type for async health check operations.
pub type HealthCheckResult = Result<AgentHealth, ConnectionError>;

/// Manages connection state and async health check requests.
pub struct ConnectionManager {
    /// Current connection status.
    status: ConnectionStatus,
    /// Pending health check result (set by async callback).
    pending_result: Arc<Mutex<Option<(String, HealthCheckResult)>>>,
    /// Last successfully connected endpoint (for reconnection).
    last_endpoint: Option<String>,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            pending_result: Arc::new(Mutex::new(None)),
            last_endpoint: None,
        }
    }

    /// Get the current connection status.
    pub fn status(&self) -> &ConnectionStatus {
        &self.status
    }

    /// Get the last successfully connected endpoint.
    pub fn last_endpoint(&self) -> Option<&str> {
        self.last_endpoint.as_deref()
    }

    /// Initiate a connection to the given endpoint.
    ///
    /// This fires off an async HTTP request to `/api/health`.
    /// Call `poll()` each frame to check for completion.
    pub fn connect(&mut self, endpoint: &str, ctx: &egui::Context) {
        let endpoint = normalize_endpoint(endpoint);

        // Don't reconnect if already connected to the same endpoint
        if let ConnectionStatus::Connected {
            endpoint: current, ..
        } = &self.status
        {
            if current == &endpoint {
                return;
            }
        }

        // Don't start a new connection if one is already in progress
        if let ConnectionStatus::Connecting {
            endpoint: current, ..
        } = &self.status
        {
            if current == &endpoint {
                return;
            }
        }

        self.status = ConnectionStatus::Connecting {
            endpoint: endpoint.clone(),
        };

        let url = format!("{endpoint}/api/health");
        let pending = Arc::clone(&self.pending_result);
        let endpoint_clone = endpoint.clone();
        let ctx = ctx.clone();

        ehttp::fetch(ehttp::Request::get(&url), move |response| {
            let result = match response {
                Ok(response) => {
                    if response.ok {
                        match serde_json::from_slice::<AgentHealth>(&response.bytes) {
                            Ok(health) => Ok(health),
                            Err(e) => Err(ConnectionError {
                                message: format!("Invalid response: {e}"),
                            }),
                        }
                    } else {
                        Err(ConnectionError {
                            message: format!("HTTP {}: {}", response.status, response.status_text),
                        })
                    }
                }
                Err(_) => Err(ConnectionError {
                    message: "Failed to connect".to_string(),
                }),
            };

            *pending.lock() = Some((endpoint_clone, result));
            ctx.request_repaint();
        });
    }

    /// Poll for completion of pending health check.
    ///
    /// Returns `Some(result)` if a health check just completed, `None` otherwise.
    /// Updates internal status accordingly.
    pub fn poll(&mut self) -> Option<HealthCheckResult> {
        let pending = self.pending_result.lock().take();

        if let Some((endpoint, result)) = pending {
            match &result {
                Ok(health) => {
                    self.last_endpoint = Some(endpoint.clone());
                    self.status = ConnectionStatus::Connected {
                        endpoint,
                        health: health.clone(),
                    };
                }
                Err(e) => {
                    self.status = ConnectionStatus::Failed {
                        endpoint,
                        error: e.message.clone(),
                    };
                }
            }
            return Some(result);
        }

        None
    }
}

/// Normalize an endpoint URL (ensure no trailing slash, add scheme if missing).
fn normalize_endpoint(endpoint: &str) -> String {
    let endpoint = endpoint.trim();
    let endpoint = endpoint.trim_end_matches('/');

    // Add http:// if no scheme present
    if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
        format!("http://{endpoint}")
    } else {
        endpoint.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_endpoint() {
        assert_eq!(
            normalize_endpoint("localhost:3000"),
            "http://localhost:3000"
        );
        assert_eq!(
            normalize_endpoint("http://localhost:3000/"),
            "http://localhost:3000"
        );
        assert_eq!(
            normalize_endpoint("https://api.example.com"),
            "https://api.example.com"
        );
        assert_eq!(
            normalize_endpoint("  localhost:8080  "),
            "http://localhost:8080"
        );
    }

    #[test]
    fn test_connection_status_helpers() {
        let status = ConnectionStatus::Disconnected;
        assert!(!status.is_connected());
        assert!(!status.is_connecting());
        assert!(status.endpoint().is_none());

        let status = ConnectionStatus::Connecting {
            endpoint: "http://localhost:3000".to_string(),
        };
        assert!(!status.is_connected());
        assert!(status.is_connecting());
        assert_eq!(status.endpoint(), Some("http://localhost:3000"));

        let status = ConnectionStatus::Connected {
            endpoint: "http://localhost:3000".to_string(),
            health: AgentHealth {
                msg: "ok".to_string(),
                version: "1.0.0".to_string(),
                git_hash: "abc123".to_string(),
                git_branch: "main".to_string(),
                built_at: "2024-01-01".to_string(),
                build_summary: "test".to_string(),
                metrics_git_version: None,
                metrics_git_timestamp: None,
            },
        };
        assert!(status.is_connected());
        assert!(!status.is_connecting());
    }
}
