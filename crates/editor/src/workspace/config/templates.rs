//! Default workspace templates.
//!
//! This module contains the built-in workspace templates that ship with the editor,
//! including example workspaces and the demo workspace for offline use.

/// Default example workspace that ships with the app
pub const DEFAULT_WORKSPACE_TOML: &str = r#"[workspace]
name = "example"
description = "Example workspace demonstrating Enya features with i3-style layout"

[view]
theme = "dark"
metrics_panel = true
inspector = false

[time]
preset = "1h"

[[panes]]
query = "env:prod AND service:api"
name = "API Latency"
tag = "Critical"
aggregation = "p95"
granularity = "1m"

[[panes]]
query = "env:prod AND name:request_count"
name = "Request Rate"
aggregation = "sum"
granularity = "1m"

[[panes]]
query = "env:prod AND name:error_rate"
name = "Error Rate"
tag = "Critical"
aggregation = "avg"
granularity = "5m"

# i3-style layout: API Latency on left (2/3), Request Rate and Error Rate stacked on right (1/3)
# +---------------------+-----------+
# |                     | Request   |
# |   API Latency (0)   | Rate (1)  |
# |                     +-----------+
# |                     | Error     |
# |                     | Rate (2)  |
# +---------------------+-----------+
[layout]
type = "horizontal"
shares = [2.0, 1.0]
children = [
    0,
    { type = "vertical", children = [1, 2] }
]
"#;

/// Complex viewport workspace with deeply nested i3-style layout
pub const COMPLEX_VIEWPORT_TOML: &str = r#"[workspace]
name = "viewport"
description = "Production viewport with complex nested i3 layout"

[view]
theme = "dark"
metrics_panel = false
inspector = false

[time]
preset = "1h"

# Pane 0: Primary metrics overview
[[panes]]
query = "env:prod"
name = "Overview"
aggregation = "count"
granularity = "1m"

# Pane 1: API latency (p99)
[[panes]]
query = "env:prod AND service:api AND name:latency"
name = "API p99"
tag = "Critical"
aggregation = "p99"
granularity = "1m"

# Pane 2: API latency (p50)
[[panes]]
query = "env:prod AND service:api AND name:latency"
name = "API p50"
aggregation = "p50"
granularity = "1m"

# Pane 3: Database queries
[[panes]]
query = "env:prod AND service:database"
name = "DB Queries"
aggregation = "sum"
granularity = "1m"

# Pane 4: Cache hit rate
[[panes]]
query = "env:prod AND service:cache AND name:hit_rate"
name = "Cache Hits"
aggregation = "avg"
granularity = "5m"

# Pane 5: Error rate
[[panes]]
query = "env:prod AND name:error"
name = "Errors"
tag = "Critical"
aggregation = "sum"
granularity = "1m"

# Pane 6: Memory usage
[[panes]]
query = "env:prod AND name:memory"
name = "Memory"
aggregation = "avg"
granularity = "5m"

# Pane 7: CPU usage
[[panes]]
query = "env:prod AND name:cpu"
name = "CPU"
aggregation = "avg"
granularity = "5m"

# Complex nested layout:
# +-------------------+-------------------+
# |                   |   API p99 (1)     |
# |   Overview (0)    +-------------------+
# |                   |   API p50 (2)     |
# +-------------------+-------------------+
# | DB (3) | Cache(4) | Errors | Mem | CPU|
# |        |          |  (5)   | (6) | (7)|
# +--------+----------+--------+-----+----+
#
# Top row: Overview on left (1/2), API metrics stacked on right (1/2)
# Bottom row: 5 panels in horizontal split
[layout]
type = "vertical"
shares = [2.0, 1.0]
children = [
    { type = "horizontal", shares = [1.0, 1.0], children = [
        0,
        { type = "vertical", children = [1, 2] }
    ]},
    { type = "horizontal", shares = [1.5, 1.5, 1.0, 1.0, 1.0], children = [3, 4, 5, 6, 7] }
]
"#;

/// Demo workspace with sample queries that work without a backend connection.
/// Uses realistic Prometheus metric names from the DemoMetricsClient catalog.
pub const DEMO_WORKSPACE_TOML: &str = r#"[workspace]
name = "demo"
description = "Interactive demo with sample data - no backend required"

# Empty endpoint means demo mode (synthetic data)
[connection]
endpoint = ""

[view]
theme = "dark"
metrics_panel = false
inspector = false

[time]
preset = "1h"

# Time Series: HTTP request rate by method (rate of counter)
[[panes]]
query = "sum(rate(http_requests_total[5m])) by (method)"
name = "HTTP Request Rate"
visualization = "time_series"
granularity = "1m"

# Stat: Active database connections (gauge - current value)
[[panes]]
query = "sum(db_connections_active) by (pool)"
name = "DB Connections"
visualization = "stat"
granularity = "1m"

# Time Series: Request latency p99 (histogram quantile)
[[panes]]
query = "histogram_quantile(0.99, rate(http_request_duration_seconds[5m]))"
name = "Request Latency (p99)"
visualization = "time_series"
granularity = "1m"

# Gauge: Application queue depth (gauge - current value)
[[panes]]
query = "sum(app_queue_depth) by (queue)"
name = "Queue Depth"
visualization = "gauge"
granularity = "1m"

# Layout: 2x2 grid
# +----------------------+------------------+
# | HTTP Request Rate(0) | DB Connections(1)|
# +----------------------+------------------+
# | Request Latency(2)   | Queue Depth (3)  |
# +----------------------+------------------+
[layout]
type = "vertical"
children = [
    { type = "horizontal", children = [0, 1] },
    { type = "horizontal", children = [2, 3] }
]
"#;
