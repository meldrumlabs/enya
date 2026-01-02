# Contributing to Enya

All contributions are appreciated! Whether it's fixing a typo, refactoring existing code, or adding a new feature.

If you're unsure where to start, have a look at the open [GitHub issues](https://github.com/meldrumlabs/enya/issues).

## Getting Started

Fork the repository and create a feature branch:

```bash
git clone git@github.com:<your-username>/enya.git
cd enya
git checkout -b my-feature
git remote add upstream git@github.com:meldrumlabs/enya.git
```

Install [just](https://github.com/casey/just) command runner:

```bash
# macOS
brew install just

# Arch Linux
pacman -S just

# Other platforms: https://github.com/casey/just#installation
```

Install development tools:

```bash
just install
```

## Development Workflow

Use the Justfile helpers for reproducibility:

```bash
just fmt          # Format code
just clippy       # Run linter
just test         # Run tests
just ci           # Full CI check (lint + machete + test + check-wasm)
just check-wasm   # Verify WASM build
```

Before submitting a PR, run `just ci` to catch issues locally.

## Coding Style

- Rust 2024 edition, minimum Rust 1.88
- Format with `cargo fmt --all`
- Keep `cargo clippy` clean (warnings are denied in CI)
- Follow idiomatic Rust naming (`snake_case` modules, `CamelCase` types)
- No unsafe code unless explicitly documented

## Pull Request Guidelines

We use a squash-and-merge strategy, so all commits will be consolidated into one.

When opening a PR:

- **Explain** what your PR introduces and why
- **Keep changes focused** — avoid bundling unrelated features
- **Reference issues** when applicable (e.g., "closes #123")
- **Include test output** or manual verification notes
- **Add screenshots/gifs** for UI-visible changes

## Testing

- Place unit tests beside their implementation files
- Use descriptive test names like `handles_out_of_order_samples`
- Gate feature-specific tests with `#[cfg(feature = "...")]`
- Run `just check-wasm` for editor changes

## Security & Dependencies

- Run `just deny` before PRs to check for vulnerable or unlicensed dependencies
- Use `just machete` to detect unused crates
- Never commit secrets or credentials

## Editor Changes

When modifying `enya-editor`, update the changelog at `crates/editor/CHANGELOG.md` under the `[Unreleased]` section following the [Keep a Changelog](https://keepachangelog.com/) format.

## Questions?

Open an issue or start a [discussion](https://github.com/meldrumlabs/enya/discussions).
