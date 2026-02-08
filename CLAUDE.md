# Claude Code Guidelines for Enya

## CI Validation

**Always run `just ci` before committing changes** to ensure all checks pass:

```bash
just ci
```

This runs:
- `cargo fmt --all -- --check` - Code formatting check
- `cargo clippy --all-targets -- -D warnings` - Linting with warnings as errors
- `cargo machete` - Unused dependency detection
- `cargo nextest run` - All tests
- `cargo check -p enya-editor --target wasm32-unknown-unknown` - WASM build check

Note: We intentionally don't use `--all-features` because the puffin and tracy profiling backends are mutually exclusive.

## Editor Changelog

When making changes to the `enya-editor` crate, **update the changelog** at `crates/editor/CHANGELOG.md` with a summary of changes under the `[Unreleased]` section. Follow the [Keep a Changelog](https://keepachangelog.com/) format:

- **Added** - New features
- **Changed** - Changes to existing functionality
- **Fixed** - Bug fixes
- **Removed** - Removed features

## Enya Commands Documentation

When adding, modifying, or removing **Enya commands** (the `enya-command` blocks that AI agents can execute), **always update the command documentation** at `crates/ai/COMMANDS.md`. This includes:

- Adding new commands to the `AgentCommand` enum in `agent_context.rs`
- Changing command parameters or behavior
- Updating command preferences or usage guidelines

The documentation should stay in sync with:
- `crates/editor/src/components/overlay/agent_context.rs` - Command definitions and parser
- `crates/editor/src/workspace/mod.rs` - Command execution handlers

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

## Custom Theme Support

The editor supports custom themes from Lua plugins. To ensure custom themes are applied everywhere:

### In `Workspace` code
Use `self.theme()` instead of `app_state.theme`:

```rust
// ✅ Correct - use self.theme()
self.my_overlay.set_theme(self.theme());
let accent_color = self.theme().accent_primary();

// ❌ Wrong - misses custom plugin themes!
self.my_overlay.set_theme(app_state.theme);
```

### In `EnyaApp` code (app/mod.rs)
Use `self.effective_theme()` instead of `self.state.theme`:

```rust
// ✅ Correct - use self.effective_theme()
self.status_line.set_theme(self.effective_theme());
self.notifications.set_theme(self.effective_theme());

// For ActiveThemeColors:
let colors = self.effective_theme().active_colors();

// ❌ Wrong - misses custom plugin themes!
self.status_line.set_theme(self.state.theme);
```

### Key methods
- `Workspace::theme()` - cached effective theme for workspace rendering
- `EnyaApp::effective_theme()` - computes effective theme (custom if active, otherwise builtin)
- `AppTheme::active_colors()` - extracts `ActiveThemeColors` from any `AppTheme`

This pattern ensures that when users activate a custom theme plugin (like Tokyo Night), all UI components display with the correct theme colors.
