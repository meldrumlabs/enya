# =============================================================================
# Justfile - Development Build & Test Commands
# =============================================================================
#
# Install Just command runner:    cargo install just
# Install dev dependencies:       just install
# List available commands:        just -l
#
# Reference documentation: https://github.com/casey/just
# =============================================================================

features := ""

_features := if features == "all" {
        "--all-features"
    } else if features != "" {
        "--features=" + features
    } else { "" }

# Installs required dev tools
install:
    cargo install --locked cargo-machete taplo-cli cargo-nextest cargo-deny

# Cleans everything through cargo clean
clean:
    cargo clean

# Runs cargo fmt
fmt:
    cargo fmt --all

# Find unused dependencies through machete
machete:
    cargo machete

# Checks for rust fmt issues
check-fmt:
    cargo fmt --all -- --check

# runs cargo-deny across the workspace
deny:
    cargo deny check

# Runs clippy checks across the workspace
clippy:
    cargo clippy --all-targets {{ _features }}  -- -D warnings

check-wasm:
    cargo check -p ui --target wasm32-unknown-unknown

# Runs taplo fmt across the repo
toml-fmt:
    taplo fmt --check --diff

# Run all lints
lint: check-fmt clippy toml-fmt

# Runs workspace tests using nextest
test:
    cargo nextest run {{ _features }}

# Runs a local CI check (enables --all-features)
ci: 
    just features=all lint machete test check-wasm
