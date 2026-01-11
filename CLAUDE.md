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

## Testing

Run tests with:
```bash
cargo nextest run --all-features
```

## Editor Changelog

When making changes to the `enya-editor` crate, **update the changelog** at `crates/editor/CHANGELOG.md` with a summary of changes under the `[Unreleased]` section. Follow the [Keep a Changelog](https://keepachangelog.com/) format:

- **Added** - New features
- **Changed** - Changes to existing functionality
- **Fixed** - Bug fixes
- **Removed** - Removed features

## WASM Compatibility

The editor and client crates must compile for WASM (`wasm32-unknown-unknown`). When working with time:

- **Never use `std::time::Instant`** directly - it freezes/panics in WASM browsers
- **Never use `std::time::SystemTime`** directly - it panics in WASM browsers
- For the editor: use `crate::util::Instant` (re-exports `web_time::Instant` on WASM)
- For the editor: use `crate::util::now_unix_secs()` for Unix timestamps
- For the client: use `enya_client::now_unix_secs()` which handles both platforms

Example for Instant:
```rust
// In the editor crate, use the util module:
use crate::util::Instant;

let start = Instant::now();
// ... do work ...
let elapsed = start.elapsed();
```

Example for SystemTime (if needed directly):
```rust
#[cfg(target_arch = "wasm32")]
use web_time::SystemTime;
#[cfg(not(target_arch = "wasm32"))]
use std::time::SystemTime;
```
