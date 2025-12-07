# enya-lang

Query language for Enya time series database.

## Syntax

```
env:prod                              # exact match
service:db.*                          # wildcard/prefix match
env:prod AND service:db               # AND
env:prod OR env:staging               # OR
!env:prod                             # NOT
(env:prod OR env:staging) AND service:db  # grouping
*                                     # match all
```

## Usage

```rust
use enya_lang::parse_filter_query;

let ast = parse_filter_query("env:prod AND service:db.*")?;
```
