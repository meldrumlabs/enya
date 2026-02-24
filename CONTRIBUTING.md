# Contributing to Enya

All contributions are appreciated! Whether it's fixing a typo, refactoring existing code, or adding a new feature.

If you're unsure where to start, have a look at the open [GitHub issues](https://github.com/meldrumlabs/enya/issues).

## Requirements

- [Rust](https://rustup.rs/) 1.88+
- [just](https://github.com/casey/just) command runner

Optional:

- [Zig](https://ziglang.org/) toolchain — required for the `terminal` feature (Ghostty)
- [Docker](https://www.docker.com/) — required for integration tests

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

# Other platforms: https://github.com/casey/just#installation
```

Install development tools:

```bash
just install
```

## Development Workflow

Use the Justfile helpers for reproducibility:

| Command | Description |
|---------|-------------|
| `just install` | Install dev tools and initialize submodules |
| `just run` | Build and run the editor |
| `just build` | Build the editor |
| `just fmt` | Format code |
| `just clippy` | Run linter |
| `just test` | Run tests with nextest |
| `just ci` | Full CI check (fmt, clippy, machete, tests, WASM) |
| `just check-wasm` | Verify WASM build compiles |
| `just machete` | Detect unused dependencies |
| `just deny` | Check dependency licenses and vulnerabilities |
| `just it-test` | Run integration tests (requires Docker) |

Before submitting a PR, run `just ci` to catch issues locally.

## Feature Flags

All features are opt-in. Enable them with `cargo build -p enya-editor --features <flag>`:

| Flag | Description |
|------|-------------|
| `terminal` | Ghostty terminal pane (requires Zig toolchain) |
| `sql` | DataFusion SQL pane (~500 additional dependencies) |
| `all-languages` | Tree-sitter syntax highlighting for Go, Python, JS/TS |
| `puffin` | Puffin profiling backend |
| `tracy` | Tracy profiling backend |

Note: `puffin` and `tracy` are mutually exclusive.

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

## AI-Assisted Contributions

AI-assisted PRs are welcome. However, please create an [issue](https://github.com/meldrumlabs/enya/issues) or reach out to the maintainers before starting work. Without prior alignment on scope and approach, there is a risk your PR will be rejected.

## Security & Dependencies

- Run `just deny` before PRs to check for vulnerable or unlicensed dependencies
- Use `just machete` to detect unused crates
- Never commit secrets or credentials

## Questions?

Open an issue or start a [discussion](https://github.com/meldrumlabs/enya/discussions).
