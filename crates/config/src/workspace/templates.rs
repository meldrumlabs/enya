//! Default workspace templates.
//!
//! This module contains the built-in workspace templates that ship with the editor,
//! including tutorial workspaces that showcase different features.
//!
//! Tutorial workspaces use demo-mode metric names (see `enya_client::demo`) so
//! they render realistic data without a real Prometheus backend.

/// Quick Start workspace — a friendly intro to the four golden signals.
pub const GOLDEN_SIGNALS_TOML: &str = r#"[workspace]
name = "quick-start"
description = "The 4 golden signals at a glance"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

[[panes]]
query = "http_request_duration_seconds"
name = "Latency Heatmap"
description = "Request latency distribution over time"
tag = "Critical"
visualization = "heatmap"
granularity = "1m"

[[panes]]
query = "http_requests_total"
name = "Traffic"
description = "HTTP requests per second by method"
unit = "req/s"
visualization = "time_series"
granularity = "1m"

[[panes]]
query = "http_requests_total"
name = "Errors"
description = "Percentage of requests returning 5xx status codes"
tag = "Critical"
unit = "%"
visualization = "stat"
granularity = "1m"

[[panes]]
query = "node_cpu_seconds_total"
name = "Saturation"
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

/// Infra workspace — system-level health at a glance.
pub const INFRASTRUCTURE_TOML: &str = r#"[workspace]
name = "infra"
description = "CPU, memory, and system health"

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
query = "node_cpu_seconds_total by_node"
name = "CPU per Node"
description = "CPU utilization broken down by host"
unit = "%"
visualization = "time_series"
granularity = "1m"

[[panes]]
query = "http_requests_total by region"
name = "Traffic by Region"
description = "Request distribution across regions"
unit = "req/s"
visualization = "pie_chart"
granularity = "1m"

[layout]
type = "vertical"
children = [
  { type = "horizontal", children = [0, 1] },
  { type = "horizontal", children = [2, 3] },
]
"#;

/// Logs & Traces workspace — explore logs and distributed traces.
pub const MULTI_SERVICE_TOML: &str = r#"[workspace]
name = "logs-and-traces"
description = "Explore logs and distributed traces"

[view]
theme = "dark"

[time]
preset = "1h"
refresh = "30s"

[[panes]]
query = "http_requests_total"
name = "Request Rate"
unit = "req/s"
visualization = "time_series"
granularity = "1m"

[[panes]]
query = "http_request_duration_seconds"
name = "p99 Latency"
tag = "Critical"
unit = "ms"
visualization = "stat"
granularity = "1m"

[[panes]]
query = ""
name = "Logs"
visualization = "logs"

[[panes]]
query = ""
name = "Traces"
visualization = "tracing"

[layout]
type = "vertical"
children = [
  { type = "horizontal", children = [0, 1] },
  { type = "horizontal", children = [2, 3] },
]
"#;
