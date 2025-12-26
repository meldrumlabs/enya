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
    cargo install --locked cargo-machete cargo-nextest cargo-deny
    cd website && npm install

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
    cargo clippy --all-targets {{ _features }} --profile ci -- -D warnings

check-wasm:
    cargo check -p enya-editor --target wasm32-unknown-unknown --profile ci

# Build the website (validates links and content)
website-build:
    cd website && npm run build

# Run the website dev server
website-dev:
    cd website && npm run dev

# Run all lints
lint: check-fmt clippy

# Runs workspace tests using nextest
test:
    cargo nextest run {{ _features }} --cargo-profile ci

# Runs a local CI check
# Note: We don't use --all-features because puffin and tracy profiling backends are mutually exclusive
ci:
    just lint machete test check-wasm website-build
