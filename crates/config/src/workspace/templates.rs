//! Default workspace templates.
//!
//! This module contains the built-in workspace templates that ship with the editor,
//! including tutorial workspaces that showcase different features and the atlas
//! workspace for real-backend codebase integration.
//!
//! Tutorial workspaces use demo-mode metric names (see `enya_client::demo`) so
//! they render realistic data without a real Prometheus backend.

/// Golden Signals workspace — the four pillars of SRE monitoring.
/// Showcases sections with different layouts and all major visualization types.
pub const GOLDEN_SIGNALS_TOML: &str = r#"[workspace]
name = "golden-signals"
description = "The four golden signals of SRE: latency, traffic, errors, saturation"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

# Latency — how long requests take
[[sections]]
name = "Latency"
layout = "horizontal"
shares = [2.0, 1.0]

[[sections.panes]]
query = "http_request_duration_seconds"
name = "Request Latency"
description = "Response time distribution across all endpoints"
tag = "Critical"
unit = "ms"
visualization = "time_series"
granularity = "1m"

[[sections.panes]]
query = "http_request_duration_seconds"
name = "Median Latency"
unit = "ms"
visualization = "stat"
granularity = "1m"

# Traffic — how much demand is being placed on the system
[[sections]]
name = "Traffic"
layout = "horizontal"

[[sections.panes]]
query = "http_requests_total"
name = "Request Rate"
description = "HTTP requests per second by method"
unit = "req/s"
visualization = "time_series"
granularity = "1m"

[[sections.panes]]
query = "http_requests_total"
name = "Rate by Endpoint"
description = "Request distribution across API endpoints"
visualization = "bar_chart"
granularity = "1m"

# Errors — the rate of failed requests
[[sections]]
name = "Errors"
layout = "horizontal"
shares = [1.0, 1.0, 1.0]

[[sections.panes]]
query = "http_requests_total"
name = "Error Rate"
description = "Percentage of requests returning 5xx status codes"
tag = "Critical"
unit = "%"
visualization = "time_series"
granularity = "1m"

[[sections.panes]]
query = "http_requests_total"
name = "5xx Errors"
tag = "Critical"
visualization = "stat"
granularity = "1m"

[[sections.panes]]
query = "http_requests_total"
name = "4xx Errors"
tag = "Warning"
visualization = "stat"
granularity = "1m"

# Saturation — how full the system is
[[sections]]
name = "Saturation"
layout = "grid"
columns = 3

[[sections.panes]]
query = "node_cpu_seconds_total"
name = "CPU Usage"
description = "Average CPU utilization across all nodes"
unit = "%"
visualization = "gauge"
granularity = "1m"

[[sections.panes]]
query = "node_memory_bytes"
name = "Memory Usage"
unit = "%"
visualization = "gauge"
granularity = "1m"

[[sections.panes]]
query = "db_connections_active"
name = "DB Pool"
description = "Database connection pool utilization"
unit = "%"
visualization = "gauge"
granularity = "1m"
"#;

/// Incident response workspace — cross-signal investigation during an outage.
/// Showcases stat panels, sparklines, and multi-section layouts.
pub const INCIDENT_RESPONSE_TOML: &str = r#"[workspace]
name = "incident-response"
description = "Cross-signal investigation workspace for incident response"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

# Key indicators to assess blast radius
[[sections]]
name = "Impact Assessment"
layout = "grid"
columns = 4

[[sections.panes]]
query = "http_requests_total"
name = "Error Rate"
tag = "Critical"
unit = "err/s"
visualization = "stat"
granularity = "1m"

[[sections.panes]]
query = "http_request_duration_seconds"
name = "p99 Latency"
tag = "Critical"
unit = "ms"
visualization = "stat"
granularity = "1m"

[[sections.panes]]
query = "http_requests_total"
name = "Throughput"
unit = "req/s"
visualization = "stat"
granularity = "1m"

[[sections.panes]]
query = "http_requests_in_flight"
name = "In-Flight Requests"
visualization = "stat"
granularity = "1m"

# Timeline of the incident
[[sections]]
name = "Error Timeline"
layout = "horizontal"

[[sections.panes]]
query = "http_requests_total"
name = "Errors by Endpoint"
description = "Identify which endpoints are failing"
tag = "Critical"
visualization = "time_series"
granularity = "1m"

[[sections.panes]]
query = "http_request_duration_seconds"
name = "Latency by Endpoint"
description = "Check for latency spikes correlating with errors"
unit = "ms"
visualization = "time_series"
granularity = "1m"

# Resource pressure during the incident
[[sections]]
name = "Resource Pressure"
layout = "horizontal"
shares = [1.0, 1.0, 1.0]

[[sections.panes]]
query = "node_cpu_seconds_total"
name = "CPU"
unit = "%"
visualization = "sparkline"
granularity = "1m"

[[sections.panes]]
query = "node_memory_bytes"
name = "Memory"
unit = "%"
visualization = "sparkline"
granularity = "1m"

[[sections.panes]]
query = "db_connections_active"
name = "DB Connections"
visualization = "sparkline"
granularity = "1m"
"#;

/// Service overview workspace — deep-dive into a single service.
/// Showcases all 6 visualization types in a single workspace.
pub const SERVICE_OVERVIEW_TOML: &str = r#"[workspace]
name = "service-overview"
description = "Single-service deep-dive showcasing every visualization type"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

# KPI summary at the top
[[sections]]
name = "Key Metrics"
layout = "grid"
columns = 4

[[sections.panes]]
query = "http_requests_total"
name = "Request Rate"
unit = "req/s"
visualization = "stat"
granularity = "1m"

[[sections.panes]]
query = "http_request_duration_seconds"
name = "p99 Latency"
unit = "ms"
visualization = "stat"
granularity = "1m"

[[sections.panes]]
query = "http_requests_total"
name = "Error Rate"
tag = "Critical"
unit = "%"
visualization = "stat"
granularity = "1m"

[[sections.panes]]
query = "node_cpu_seconds_total"
name = "CPU Usage"
unit = "%"
visualization = "gauge"
granularity = "1m"

# Request trends — detailed time series
[[sections]]
name = "Request Trends"
layout = "horizontal"

[[sections.panes]]
query = "http_requests_total"
name = "Traffic by Method"
description = "GET, POST, PUT, DELETE request rates over time"
visualization = "time_series"
granularity = "1m"

[[sections.panes]]
query = "http_request_duration_seconds"
name = "Latency Heatmap"
description = "Request latency distribution over time"
visualization = "heatmap"
granularity = "1m"

# Endpoint breakdown
[[sections]]
name = "Endpoint Breakdown"
layout = "horizontal"

[[sections.panes]]
query = "http_requests_total"
name = "Traffic by Endpoint"
description = "Which endpoints receive the most traffic"
visualization = "bar_chart"
granularity = "5m"

[[sections.panes]]
query = "http_request_duration_seconds"
name = "Latency by Endpoint"
description = "Compact latency trends per endpoint"
visualization = "sparkline"
granularity = "1m"
"#;

/// Infrastructure workspace — system-level monitoring with live auto-refresh.
/// Showcases gauges, sparklines, and grid layout for compact dashboards.
pub const INFRASTRUCTURE_TOML: &str = r#"[workspace]
name = "infrastructure"
description = "System-level monitoring: CPU, memory, disk, network"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

# System health gauges
[[sections]]
name = "System Health"
layout = "grid"
columns = 4

[[sections.panes]]
query = "node_cpu_seconds_total"
name = "CPU"
description = "Average CPU utilization across all nodes"
unit = "%"
visualization = "gauge"
granularity = "1m"

[[sections.panes]]
query = "node_memory_bytes"
name = "Memory"
description = "System memory utilization"
unit = "%"
visualization = "gauge"
granularity = "1m"

[[sections.panes]]
query = "node_disk_read_bytes_total"
name = "Disk Read"
description = "Disk read throughput"
unit = "MB/s"
visualization = "stat"
granularity = "1m"

[[sections.panes]]
query = "node_network_receive_bytes_total"
name = "Network In"
unit = "MB/s"
visualization = "stat"
granularity = "1m"

# Resource trends over time
[[sections]]
name = "Resource Trends"
layout = "horizontal"

[[sections.panes]]
query = "node_cpu_seconds_total"
name = "CPU per Node"
description = "CPU utilization broken down by host"
unit = "%"
visualization = "time_series"
granularity = "1m"

[[sections.panes]]
query = "node_memory_bytes"
name = "Memory Usage"
description = "Memory utilization over time"
unit = "MB"
visualization = "time_series"
granularity = "1m"

# Database and cache infrastructure
[[sections]]
name = "Database & Cache"
layout = "grid"
columns = 3

[[sections.panes]]
query = "db_connections_active"
name = "DB Connections"
description = "Active connections per pool"
visualization = "sparkline"
granularity = "1m"

[[sections.panes]]
query = "db_query_duration_seconds"
name = "Avg Query Time"
unit = "ms"
visualization = "stat"
granularity = "1m"

[[sections.panes]]
query = "app_queue_depth"
name = "Queue Depth"
description = "Messages waiting to be processed"
visualization = "sparkline"
granularity = "1m"
"#;

/// Multi-service comparison workspace — comparing services side by side.
/// Showcases bar charts and tabs layout for comparing across services.
pub const MULTI_SERVICE_TOML: &str = r#"[workspace]
name = "multi-service"
description = "Compare request rates, latencies, and errors across services"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

# Service comparison overview
[[sections]]
name = "Service Comparison"
layout = "horizontal"

[[sections.panes]]
query = "http_requests_in_flight"
name = "In-Flight by Service"
description = "Active requests compared across all services"
unit = "req"
visualization = "bar_chart"
granularity = "5m"

[[sections.panes]]
query = "http_request_duration_seconds"
name = "Latency by Service"
description = "Tail latency compared across services"
unit = "ms"
visualization = "bar_chart"
granularity = "5m"

# Per-service detail — tabs for switching between services
[[sections]]
name = "Service Traffic"
layout = "tabs"

[[sections.panes]]
query = "http_requests_total"
name = "API Gateway"
visualization = "time_series"
granularity = "1m"

[[sections.panes]]
query = "tokio_tasks_spawned_total"
name = "Async Tasks"
visualization = "time_series"
granularity = "1m"

[[sections.panes]]
query = "app_cache_hits_total"
name = "Cache Performance"
visualization = "time_series"
granularity = "1m"

# Application health
[[sections]]
name = "Application Health"
layout = "horizontal"
shares = [2.0, 1.0]

[[sections.panes]]
query = "app_active_users"
name = "Active Users"
visualization = "time_series"
granularity = "1m"

[[sections.panes]]
query = "app_cache_misses_total"
name = "Cache Misses"
tag = "Warning"
visualization = "bar_chart"
granularity = "5m"
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
