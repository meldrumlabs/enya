# Enya Cloud Metrics

This document describes all Prometheus metrics exposed by the Enya Cloud backend at the `/metrics` endpoint.

## Endpoint

```
GET /metrics
```

Returns metrics in Prometheus exposition format. Compatible with Prometheus, Grafana, and other monitoring tools.

## Metrics Reference

### HTTP Metrics

These metrics are automatically collected for all API requests via middleware.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `http_request_duration_seconds` | Histogram | `method`, `path`, `status` | Request latency in seconds |
| `http_requests_total` | Counter | `method`, `path`, `status` | Total number of HTTP requests |

**Note:** Path labels are normalized to reduce cardinality. UUIDs are replaced with `{id}` (e.g., `/teams/{id}/members`).

### Database Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `enya_db_query_duration_seconds` | Histogram | `operation` | Query execution time in seconds |
| `enya_db_queries_total` | Counter | `operation` | Total number of database queries |
| `enya_db_errors_total` | Counter | `operation` | Total number of database errors |
| `enya_db_pool_connections_active` | Gauge | - | Current active database connections |
| `enya_db_pool_connections_idle` | Gauge | - | Current idle database connections |

### Authentication Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `enya_auth_success_total` | Counter | `provider` | Successful authentications (e.g., `github`, `dev`) |
| `enya_auth_failure_total` | Counter | `provider`, `reason` | Failed authentications |

### Team Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `enya_teams_created_total` | Counter | - | Total teams created |
| `enya_team_joins_total` | Counter | - | Total users joining teams |
| `enya_members_removed_total` | Counter | - | Total members removed by admins |
| `enya_members_left_total` | Counter | - | Total members leaving voluntarily |
| `enya_role_changes_total` | Counter | - | Total role changes (admin/member) |

### Invitation Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `enya_invitations_sent_total` | Counter | `type` | Invitations created (`email` or `magic_link`) |
| `enya_invitations_accepted_total` | Counter | - | Invitations accepted |
| `enya_invitations_revoked_total` | Counter | - | Invitations revoked by admins |

### Collaboration Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `enya_messages_sent_total` | Counter | - | Total messages sent |
| `enya_annotations_created_total` | Counter | - | Total annotations created |
| `enya_annotations_deleted_total` | Counter | - | Total annotations deleted |
| `enya_threads_created_total` | Counter | - | Total threads created |
| `enya_threads_resolved_total` | Counter | - | Total threads resolved |
| `enya_channels_created_total` | Counter | - | Total channels created |

### WebSocket Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `enya_websocket_connections` | Gauge | - | Current active WebSocket connections |
| `enya_websocket_connections_total` | Counter | - | Total WebSocket connections established |
| `enya_websocket_disconnections_total` | Counter | - | Total WebSocket disconnections |
| `enya_realtime_events_total` | Counter | `type` | Real-time events broadcast by type |

### Per-Team Usage Metrics

These metrics support usage-based billing and per-team analytics.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `enya_team_api_calls_total` | Counter | `team_id` | API calls per team |

### Error Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `enya_errors_total` | Counter | `type` | Errors by type (e.g., `auth`, `validation`, `permission`) |
| `enya_api_errors_total` | Counter | `type`, `endpoint` | API errors by type and endpoint |

## Example Prometheus Queries

### Request Rate
```promql
rate(http_requests_total[5m])
```

### Request Latency (p95)
```promql
histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))
```

### Error Rate
```promql
sum(rate(http_requests_total{status=~"5.."}[5m])) / sum(rate(http_requests_total[5m]))
```

### Active Teams (by activity)
```promql
count(increase(enya_team_api_calls_total[24h]) > 0)
```

### Messages Per Minute
```promql
rate(enya_messages_sent_total[1m]) * 60
```

### WebSocket Connection Health
```promql
enya_websocket_connections
```

### Database Query Latency (p99)
```promql
histogram_quantile(0.99, rate(enya_db_query_duration_seconds_bucket[5m]))
```

## Grafana Dashboard

A sample Grafana dashboard is available at `dashboards/enya-cloud.json` (coming soon).

Recommended panels:
- Request rate and latency heatmap
- Error rate over time
- Active users and teams
- WebSocket connections
- Database connection pool usage
- Top endpoints by traffic

## Alerting Rules

Example Prometheus alerting rules:

```yaml
groups:
  - name: enya-cloud
    rules:
      - alert: HighErrorRate
        expr: sum(rate(http_requests_total{status=~"5.."}[5m])) / sum(rate(http_requests_total[5m])) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate detected"

      - alert: HighLatency
        expr: histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m])) > 1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High request latency (p95 > 1s)"

      - alert: DatabaseConnectionPoolExhausted
        expr: enya_db_pool_connections_idle == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Database connection pool exhausted"
```

## Self-Hosted Deployment

For self-hosted deployments, configure your Prometheus to scrape the metrics endpoint:

```yaml
scrape_configs:
  - job_name: 'enya-cloud'
    static_configs:
      - targets: ['enya-cloud:8080']
    metrics_path: '/metrics'
```
