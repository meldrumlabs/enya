# Repository Guidelines

This guide keeps contributors aligned with the current enya workspace. Scan it before opening a PR so every change fits the conventions.

## Project Structure & Module Organization

The workspace in `Cargo.toml` centers on `crates/*`: `enya` (CLI binary), `editor` (egui/eframe UI), `agent` (JSON-RPC session and HTTP server), `ai` (LLM provider integration), `client` (Prometheus/Loki backends), `analyzer` (git repo analysis and syntax highlighting), `promql`/`logql` (query parsers), `search` (Tantivy full-text search), `datafusion` (SQL engine), `plugin` (Lua plugin system), `config` (shared configuration), `headless` (headless API), `build-info`/`build-tools` (build utilities), and `egui_ghostty`/`ghostty_vt`/`ghostty_vt_sys` (terminal emulator). Static media live in `assets/`. Unit tests sit beside their modules, and cross-crate integration tests belong in `crates/integration-tests/` so `cargo nextest` discovers them automatically.

## Build, Test, and Development Commands

Use the Justfile helpers for reproducibility:

```bash
just fmt          # cargo fmt --all
just clippy       # cargo clippy --all-targets -D warnings
just test         # cargo nextest run (respects features=...)
just ci           # Enables all features, runs lint+machete+test+check-wasm
just check-wasm   # `enya-editor` against wasm32-unknown-unknown
```

Run `just install` one time to fetch `cargo-nextest`, `cargo-machete`, and `cargo-deny`.

## Coding Style & Naming Conventions

The workspace targets Rust 2024 edition (`rust-version = 1.88`). Always format with `cargo fmt --all` and keep `cargo clippy` clean; warnings are denied in CI. Follow idiomatic Rust naming (`snake_case` modules, `CamelCase` types, `SCREAMING_SNAKE_CASE` consts). Prefer `tracing` spans over direct logging, and respect the project’s “unsafe forbidden” posture unless a crate documents an exception.

## Testing Guidelines

Author fast unit tests near implementation files and heavier flows inside `tests/`. Default to `cargo nextest run` (or `just test`) so flaky tests get isolated. Use descriptive names like `handles_out_of_order_samples` and gate feature-specific suites with `#[cfg(feature = "...")]`. For the UI, pair `just check-wasm` with WebAssembly smoke tests before publishing.

## Commit & Pull Request Guidelines

Recent history favors short, imperative commits (“add cache to ci”, “remove toml fmt”). Keep commits scoped to a single concern and reference GitHub issues with `Refs #123` when relevant. PRs should include: (1) a one-paragraph summary of the change, (2) explicit test output or manual verification notes, (3) screenshots or gifs for UI-visible updates, and (4) mention of compatibility considerations (feature flags, WASM builds). Request reviews from domain owners of the touched crates.

## Security & Dependency Hygiene

Run `just deny` before PRs to surface vulnerable or unlicensed dependencies (`deny.toml` holds the policy). Add `just machete` when trimming unused crates. Never commit secrets; configuration samples should live under `examples/` or as `.example` files ignored by git.
