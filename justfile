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

# Initialize git submodules (ghostty for terminal emulator)
submodules:
    git submodule update --init --recursive

# Installs required dev tools
install: submodules
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
    #!/usr/bin/env bash
    if command -v npm &> /dev/null && [ -d "website/node_modules" ]; then
        cd website && npm run build
    else
        echo "Skipping website build (npm not available or node_modules not installed)"
    fi

# Run the website dev server
website-dev:
    cd website && npm run dev

# Run all lints
lint: check-fmt clippy

# Runs workspace tests using nextest
test:
    cargo nextest run {{ _features }} --cargo-profile ci

# Runs integration tests (requires Docker)
it-test:
    cargo nextest run -p enya-integration-tests --run-ignored ignored-only

# Runs a local CI check
# Note: We don't use --all-features because puffin and tracy profiling backends are mutually exclusive
ci: submodules
    just lint machete test check-wasm website-build

# Run the editor (initializes submodules first)
run: submodules
    cargo run -p enya-editor

# Build the editor (initializes submodules first)
build: submodules
    cargo build -p enya-editor

# Build the serve binary (trunk first, then cargo with embedded WASM assets)
serve-build:
    cd crates/editor && trunk build --release
    cargo build -p enya --features serve --release

# Deploy website + WASM editor to Cloudflare Pages
deploy-website:
    cd crates/editor && trunk build --release --public-url /editor/
    mkdir -p website/public/editor
    cp -r crates/editor/dist/* website/public/editor/
    cd website && npm install && npm run build
    npx wrangler pages deploy website/dist --project-name=enya --branch=deploy

# Run enya serve in development
serve workspace:
    cargo run -p enya --features serve -- serve {{workspace}}
