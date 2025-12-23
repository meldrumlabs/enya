# enya-index

Metrics instrumentation indexer for source code repositories.

This crate scans codebases to discover metric instrumentation points and alert rule definitions, building an in-memory index for fast lookups.

## Features

### Metric Scanning

Discovers metrics defined in source code using tree-sitter parsing:

| Language | Library | Patterns |
|----------|---------|----------|
| Rust | [metrics-rs](https://docs.rs/metrics) | `counter!()`, `gauge!()`, `histogram!()` |

For each metric, the scanner extracts:
- Metric name (e.g., `http_requests_total`)
- Metric kind (counter, gauge, histogram)
- Label keys (e.g., `["method", "status"]`)
- File location (path, line, column)
- Function context (containing function and impl type)

### Alert Scanning

Discovers Prometheus alerting rules in YAML files:

```yaml
groups:
  - name: example
    rules:
      - alert: HighErrorRate
        expr: rate(errors_total[5m]) > 0.1
        labels:
          severity: critical
        annotations:
          message: "Error rate is high"
```

For each alert, the scanner extracts:
- Alert name
- PromQL expression
- Primary metric name (extracted from expression)
- Severity and message
- File location

### Lookup Features

- **Exact matching**: Find metrics by exact name
- **Suffix matching**: Find `http_requests_total` when querying `myapp_http_requests_total` (handles runtime metric prefixes)
- **Fuzzy search**: Case-insensitive substring matching
- **Alert lookup**: Find alerts that reference a specific metric

## Architecture

```
crates/index/
├── src/
│   ├── lib.rs          # Public API exports
│   ├── index.rs        # CodebaseIndex - builds and queries the index
│   ├── parser.rs       # Tree-sitter parsing utilities
│   ├── repo.rs         # Git operations (clone, fetch)
│   └── scanner/
│       ├── mod.rs      # Scanner trait and registry
│       ├── rust.rs     # Rust metrics-rs scanner
│       └── yaml.rs     # YAML alert rule scanner
```

### Scanner Trait

Add support for new languages by implementing the `Scanner` trait:

```rust
pub trait Scanner: Send + Sync {
    /// File extensions this scanner handles (e.g., `["rs"]`).
    fn extensions(&self) -> &[&str];

    /// Scan a source file for metric instrumentation points.
    fn scan_file(&self, path: &Path) -> Result<Vec<MetricInstrumentation>, ParseError>;
}
```

## Usage

```rust
use enya_index::{CodebaseIndex, build_index_with_progress, IndexProgress};
use std::path::Path;

// Build an index from a local repository
let progress = IndexProgress::new();
let index = build_index_with_progress(
    "https://github.com/org/repo.git",
    Path::new("/path/to/repo"),
    &progress,
)?;

// Find metrics by name (supports suffix matching for prefixed metrics)
let metrics = index.find_by_name("myapp_http_requests_total");

// Search for metrics containing a substring
let results = index.search("requests");

// Find alerts referencing a metric
let alerts = index.find_alerts_by_metric("http_requests_total");
```

## Excluded Directories

The scanner automatically excludes:
- `target/` (Rust build output)
- `.git/`
- `vendor/`
- `node_modules/`
