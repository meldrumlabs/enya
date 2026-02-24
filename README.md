<p align="center">
  <img width="300" height="300" src="assets/logo.png">
</p>

![ci](https://github.com/meldrumlabs/enya/actions/workflows/ci.yml/badge.svg)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

# Enya

Observability for Builders. A shared interface for humans and AI agents.

Learn more at [enya.build](https://enya.build) or try it out directly in the [browser](https://enya.build/editor).

- **AI Native** — Built from the ground up for AI. Agents have access to the same commands you do. Supports Codex and Claude through ACP.
- **Multi-Tool** — Code, metrics, logs, traces, SQL, terminals — all in one interface.
- **Codebase-Aware** — Uses tree-sitter to analyze metrics, alerts, and source definitions across your codebase.
- **Shared Workspaces** — Both you and AI agents create, edit, and iterate in the same workspace.
- **Extensible** — Neovim-inspired modal editing with Lua plugins. Fully customizable.
- **Fast** — Built in Rust on top of egui. Runs natively on desktop and web through WASM.

## Status

Enya is under active development. We're building in two phases:

1. **Human interface** (current) — Editor, multi-tool panes, modal editing, Lua plugins, and the core workspace experience.
2. **Agent** (next) — Headless CLI interface for agents to monitor, analyze, and create workspaces viewable by humans.

## Platform Support

| Platform | Status |
|----------|--------|
| macOS (arm64, x64) | Tested |
| Web (WASM) | Tested |
| Linux (x64) | Builds, community-tested |
| Windows (x64) | Builds, community-tested |

Enya has primarily been developed and tested on macOS and WASM. Linux and Windows builds compile and pass CI, but have received less hands-on testing. Bug reports and contributions for these platforms are welcome.

## Getting Started

```bash
git clone https://github.com/meldrumlabs/enya.git
cd enya
just install
just run
```

Requires [Rust](https://rustup.rs/) 1.88+ and [just](https://github.com/casey/just). See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development setup, commands, and feature flags.

## Acknowledgements

- [rerun.io](https://rerun.io) for inspiration on egui-based UI
- [PlanetScale](https://planetscale.com) for inspiration on series UX
- [Conductor](https://conductor.build) for inspiration on UX design
- [Linear](https://linear.app) for inspiration on UX design
- [Neovim](https://neovim.io) for inspiration on plugin system architecture and keybindings
- [gpui-ghostty](https://github.com/Xuanwo/gpui-ghostty) by [Xuanwo](https://github.com/Xuanwo) for inspiration on terminal emulator integration
- [Zed](https://zed.dev) for inspiration on agent integration with ACP

## Data Privacy

Enya runs on your machine. Your workspaces, configuration, observability data, and source code stay local. Enya connects directly to your data sources — nothing is proxied through external servers. Enya does not collect telemetry, usage analytics, or crash reports.

Snapshots let you share a workspace via a link. Sharing is entirely opt-in — nothing is uploaded unless you explicitly choose to share. Public snapshots require GitHub authentication and are stored on Cloudflare R2 with a per-user quota. Snapshots auto-expire after 7 days. Read more in our [privacy policy](https://enya.build/privacy).

## License

Enya is developed by [Meldrum Labs](https://github.com/meldrumlabs) and licensed under the [MIT license](LICENSE).
