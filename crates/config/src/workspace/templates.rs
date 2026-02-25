//! Default workspace templates.
//!
//! This module contains the built-in workspace templates that ship with the editor,
//! including tutorial workspaces that showcase different features and the atlas
//! workspace for real-backend codebase integration.
//!
//! Tutorial workspaces use demo-mode metric names (see `enya_client::demo`) so
//! they render realistic data without a real Prometheus backend.

/// Golden Signals workspace — the four pillars of SRE monitoring.
pub const GOLDEN_SIGNALS_TOML: &str = r#"[workspace]
name = "golden-signals"
description = "The four golden signals of SRE: latency, traffic, errors, saturation"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

[[panes]]
query = "http_request_duration_seconds"
name = "Request Latency"
description = "Response time distribution across all endpoints"
tag = "Critical"
unit = "ms"
visualization = "time_series"
granularity = "1m"

[[panes]]
query = "http_requests_total"
name = "Request Rate"
description = "HTTP requests per second by method"
unit = "req/s"
visualization = "time_series"
granularity = "1m"

[[panes]]
query = "http_requests_total"
name = "Error Rate"
description = "Percentage of requests returning 5xx status codes"
tag = "Critical"
unit = "%"
visualization = "stat"
granularity = "1m"

[[panes]]
query = "node_cpu_seconds_total"
name = "CPU Usage"
description = "Average CPU utilization across all nodes"
unit = "%"
visualization = "gauge"
granularity = "1m"

[layout]
type = "vertical"
children = [
  { type = "horizontal", children = [0, 1] },
  { type = "horizontal", children = [2, 3] },
]
"#;

/// Incident response workspace — cross-signal investigation during an outage.
pub const INCIDENT_RESPONSE_TOML: &str = r#"[workspace]
name = "incident-response"
description = "Cross-signal investigation workspace for incident response"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

[[panes]]
query = "http_requests_total"
name = "Error Rate"
tag = "Critical"
unit = "err/s"
visualization = "stat"
granularity = "1m"

[[panes]]
query = "http_request_duration_seconds"
name = "p99 Latency"
tag = "Critical"
unit = "ms"
visualization = "stat"
granularity = "1m"

[[panes]]
query = "http_requests_total"
name = "Errors by Endpoint"
description = "Identify which endpoints are failing"
tag = "Critical"
visualization = "time_series"
granularity = "1m"

[[panes]]
query = "node_cpu_seconds_total"
name = "CPU"
unit = "%"
visualization = "sparkline"
granularity = "1m"

[layout]
type = "vertical"
children = [
  { type = "horizontal", children = [0, 1] },
  { type = "horizontal", children = [2, 3] },
]
"#;

/// Service overview workspace — deep-dive into a single service.
pub const SERVICE_OVERVIEW_TOML: &str = r#"[workspace]
name = "service-overview"
description = "Single-service deep-dive showcasing multiple visualization types"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

[[panes]]
query = "http_requests_total"
name = "Request Rate"
unit = "req/s"
visualization = "stat"
granularity = "1m"

[[panes]]
query = "http_request_duration_seconds"
name = "p99 Latency"
unit = "ms"
visualization = "stat"
granularity = "1m"

[[panes]]
query = "http_requests_total"
name = "Traffic by Method"
description = "GET, POST, PUT, DELETE request rates over time"
visualization = "time_series"
granularity = "1m"

[[panes]]
query = "http_request_duration_seconds"
name = "Latency Heatmap"
description = "Request latency distribution over time"
visualization = "heatmap"
granularity = "1m"

[layout]
type = "vertical"
children = [
  { type = "horizontal", children = [0, 1] },
  { type = "horizontal", children = [2, 3] },
]
"#;

/// Infrastructure workspace — system-level monitoring with live auto-refresh.
pub const INFRASTRUCTURE_TOML: &str = r#"[workspace]
name = "infrastructure"
description = "System-level monitoring: CPU, memory, disk, network"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

[[panes]]
query = "node_cpu_seconds_total"
name = "CPU"
description = "Average CPU utilization across all nodes"
unit = "%"
visualization = "gauge"
granularity = "1m"

[[panes]]
query = "node_memory_bytes"
name = "Memory"
description = "System memory utilization"
unit = "%"
visualization = "gauge"
granularity = "1m"

[[panes]]
query = "node_cpu_seconds_total"
name = "CPU per Node"
description = "CPU utilization broken down by host"
unit = "%"
visualization = "time_series"
granularity = "1m"

[[panes]]
query = "node_memory_bytes"
name = "Memory Usage"
description = "Memory utilization over time"
unit = "MB"
visualization = "time_series"
granularity = "1m"

[layout]
type = "vertical"
children = [
  { type = "horizontal", children = [0, 1] },
  { type = "horizontal", children = [2, 3] },
]
"#;

/// Multi-service comparison workspace — comparing services side by side.
pub const MULTI_SERVICE_TOML: &str = r#"[workspace]
name = "multi-service"
description = "Compare request rates, latencies, and errors across services"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

[[panes]]
query = "http_requests_in_flight"
name = "In-Flight by Service"
description = "Active requests compared across all services"
unit = "req"
visualization = "bar_chart"
granularity = "5m"

[[panes]]
query = "http_request_duration_seconds"
name = "Latency by Service"
description = "Tail latency compared across services"
unit = "ms"
visualization = "bar_chart"
granularity = "5m"

[[panes]]
query = "http_requests_total"
name = "API Gateway"
visualization = "time_series"
granularity = "1m"

[[panes]]
query = "app_active_users"
name = "Active Users"
visualization = "time_series"
granularity = "1m"

[layout]
type = "vertical"
children = [
  { type = "horizontal", children = [0, 1] },
  { type = "horizontal", children = [2, 3] },
]
"#;

/// Atlas workspace for Polygon's rust-app-atlas repository.
/// Demonstrates codebase integration with alert rules and metric definitions.
pub const ATLAS_WORKSPACE_TOML: &str = r#"[workspace]
name = "atlas"
description = "Atlas observability dashboard with codebase integration"
endpoint = "http://localhost:9090"

[git]
url = "git@github.com:polygon-io/rust-app-atlas.git"
branch = "main"
language = "rust"

[view]
theme = "dark"

[time]
preset = "1h"

# Atlas Live Consumer metrics
[[panes]]
query = "sum(rate(atlas_live_consumer_errors_total[5m])) by (status)"
name = "Live Consumer Errors"
tag = "Critical"
visualization = "time_series"
granularity = "1m"

# Layout: Single pane for now
[layout]
type = "horizontal"
children = [0]
"#;
