# Integration Tests

Integration tests for enya crates using [testcontainers](https://rust.testcontainers.org/).

## Requirements

- Docker must be running

## Running

```bash
# Run all integration tests
just it-test

# Or directly with cargo
cargo nextest run -p enya-integration-tests --run-ignored ignored-only
```

## Prometheus Client Tests

Tests that verify `PrometheusClient` correctly communicates with a real Prometheus instance:

| Test | What it verifies |
|------|------------------|
| `test_health_check` | `/api/v1/status/buildinfo` returns valid backend info |
| `test_backend_type` | Client reports `"prometheus"` as backend type |
| `test_fetch_label_names` | `/api/v1/labels` endpoint works |
| `test_fetch_metric_names_empty` | `/api/v1/label/__name__/values` endpoint works |
| `test_query_nonexistent_metric` | Query for non-existent metric returns empty (not error) |
| `test_query_prometheus_internal_metrics` | Query for `up` metric returns self-scrape data |
| `test_query_with_time_range` | Time-range queries work correctly |
| `test_fetch_label_values_for_job` | `/api/v1/label/{name}/values` returns expected values |
| `test_fetch_metric_labels` | `/api/v1/series` returns job/instance labels for `up` |
| `test_promql_aggregation_query` | `sum(up)` aggregation works |
| `test_promql_rate_query` | `rate()` function queries don't error |

These test the full HTTP round-trip: URL construction, request encoding, response parsing, and error handling against a real Prometheus API.
