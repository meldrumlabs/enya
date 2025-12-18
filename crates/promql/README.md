# enya-promql

PromQL parser and autocomplete for the Enya metrics editor.

## Features

- Context-aware autocomplete analysis
- Query validation using promql-parser
- Syntax suggestions for PromQL constructs

## Usage

```rust
use enya_promql::{analyze, syntax_suggestions, validate};

// Validate a query
let result = validate("rate(http_requests_total[5m])");
assert!(result.is_valid);

// Get completion context at cursor position
let query = "sum(http_requests_total{";
let cursor = query.len();
let ctx = analyze(query, cursor);

// Get syntax suggestions for the context
let suggestions = syntax_suggestions(&ctx);
```
