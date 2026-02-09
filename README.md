<p align="center">
  <img width="300" height="300" src="assets/logo.png">
</p>

![ci](https://github.com/meldrumlabs/enya/actions/workflows/ci.yml/badge.svg)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

# Enya

Neovim meets Grafana, reimagined for human and agent collaboration.

- **AI native** — agents with access to metrics, logs, traces, and your codebase. Supports Codex and Claude through ACP.
- **Multi-backend** — one interface for metrics, logs, traces, SQL, and git.
- **Codebase-aware** — jump from observability data to source code. Uses tree-sitter to analyze your codebase for Prometheus metrics and alerts.
- **Shared workspaces** — both you and AI agents create, edit, and iterate in the same workspace. Dashboards, queries, and runbooks — built together.
- **Extensible** — Neovim-inspired, modal editing, and fully customizable. Create your own Lua plugins that hook into the editor.
- **Fast** — built in Rust. Native performance for queries, rendering, and navigation. No JavaScript. No Electron.

## Platform Support

| Platform | Status |
|----------|--------|
| macOS (arm64, x64) | Tested |
| Web (WASM) | Tested |
| Linux (x64) | Builds, community-tested |
| Windows (x64) | Builds, community-tested |

Enya has primarily been developed and tested on macOS and WASM. Linux and Windows builds compile and pass CI, but have received less hands-on testing. Bug reports and contributions for these platforms are welcome.

## Requirements

- [Rust](https://rustup.rs/) 1.88+
- [just](https://github.com/casey/just) command runner

Optional:

- [Zig](https://ziglang.org/) toolchain — required for the `terminal` feature (Ghostty)
- [Docker](https://www.docker.com/) — required for integration tests

## Getting Started

```bash
# Clone the repository
git clone https://github.com/meldrumlabs/enya.git
cd enya

# Install dev tools (cargo-machete, cargo-nextest, cargo-deny) and init submodules
just install

# Build and run the editor
just run
```

## Just Commands

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

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow, coding style, and PR guidelines.

## Acknowledgements

- [rerun.io](https://rerun.io) for inspiration on egui-based UI
- [PlanetScale](https://planetscale.com) for inspiration on series UX
- [Conductor](https://conductor.build) for inspiration on UX design
- [Neovim](https://neovim.io) for inspiration on plugin system architecture and keybindings
- [gpui-ghostty](https://github.com/Xuanwo/gpui-ghostty) by [Xuanwo](https://github.com/Xuanwo) for inspiration on terminal emulator integration
- [Zed](https://zed.dev) for inspiration on agent integration with ACP

## License

Licensed under the [MIT license](LICENSE).
