# Claude Code Guidelines for Enya

## CI Validation

**Always run `just ci` before committing changes** to ensure all checks pass:

```bash
just ci
```

This runs:
- `cargo fmt --all -- --check` - Code formatting check
- `cargo clippy --all-targets --all-features -- -D warnings` - Linting with warnings as errors
- `cargo machete` - Unused dependency detection
- `cargo nextest run --all-features` - All tests
- `cargo check -p enya-editor --target wasm32-unknown-unknown` - WASM build check

## Key Lint Rules

The `metrics-store` crate has strict linting:
- `#![deny(clippy::unwrap_used)]` - No `.unwrap()` calls
- `#![warn(clippy::indexing_slicing)]` - Prefer `.get()` over direct indexing
- `#![warn(clippy::pedantic)]` - Strict style checks

When adding code that triggers these lints intentionally (e.g., indexing after bounds check), use targeted `#[allow(...)]` attributes with a safety comment.

## Testing

Run tests with:
```bash
cargo nextest run --all-features
```

Or for a specific package:
```bash
cargo test --package enya-metrics-store
```
