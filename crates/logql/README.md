# enya-logql

Lightweight LogQL parser and autocomplete for [Enya](https://github.com/meldrumlabs/enya).

## Features

- **Lexer**: Scans LogQL input to track nesting and cursor context
- **Completion**: Context-aware suggestions for LogQL syntax
- **Validation**: Basic query validation

## LogQL Syntax Support

### Stream Selectors
```logql
{app="nginx", env="prod"}
```

### Line Filters
- `|=` - Line contains
- `!=` - Line does not contain
- `|~` - Line matches regex
- `!~` - Line does not match regex

### Label Filters
```logql
{app="nginx"} | json | level="error"
```

### Parsers
- `json` - JSON parser
- `logfmt` - Logfmt parser
- `pattern` - Pattern parser
- `regexp` - Regex parser
- `unpack` - Unpack parser

### Range Aggregations
- `rate()` - Rate of log entries per second
- `count_over_time()` - Count of log entries
- `bytes_rate()` - Rate of bytes per second
- `bytes_over_time()` - Total bytes

### Aggregations
- `sum`, `avg`, `min`, `max`, `count`
- `stddev`, `stdvar`
- `bottomk`, `topk`

## Usage

```rust
use enya_logql::{analyze, syntax_suggestions, Context};

// Analyze cursor context
let ctx = analyze("{app=\"nginx\"} |= ", 18);

// Get suggestions for current context
let suggestions: Vec<&str> = syntax_suggestions(&ctx).collect();
```

## License

MIT OR Apache-2.0
