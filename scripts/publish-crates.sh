#!/bin/bash
# Publishes Enya workspace crates to crates.io in dependency order.
#
# Required environment variables:
#   CARGO_REGISTRY_TOKEN - crates.io API token
#
# Usage:
#   ./scripts/publish-crates              # publish all crates
#   ./scripts/publish-crates --dry-run    # verify without publishing

set -euo pipefail

DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=true
    echo "==> Dry run mode (no crates will be published)"
fi

if [[ "$DRY_RUN" == "false" && -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
    echo "ERROR: CARGO_REGISTRY_TOKEN must be set"
    exit 1
fi

# Crates listed in topological dependency order.
# Tier 1: no internal dependencies
# Tier 2: depends on tier 1
# Tier 3+: depends on earlier tiers
CRATES=(
    # Tier 1 - leaf crates
    enya-build-tools
    enya-build-info
    enya-promql
    enya-logql
    enya-config
    enya-ai

    # Tier 2
    enya-plugin
    egui_nerdfonts
    enya-client
    enya-analyzer

    # Tier 3
    enya-search
    enya-headless

    # Tier 4
    enya-agent
    enya-editor

    # Tier 5 - the binary
    enya
)

PUBLISHED=0
SKIPPED=0
FAILED=0

for crate in "${CRATES[@]}"; do
    echo ""
    echo "==> [$((PUBLISHED + SKIPPED + FAILED + 1))/${#CRATES[@]}] $crate"

    # Check if this version is already published
    VERSION=$(cargo metadata --format-version=1 --no-deps \
        | python3 -c "import sys,json; pkgs=json.load(sys.stdin)['packages']; print(next(p['version'] for p in pkgs if p['name']=='$crate'))")

    PUBLISHED_VERSION=$(cargo search "$crate" --limit 1 2>/dev/null | head -1 | sed -n 's/^'"$crate"' = "\([^"]*\)".*/\1/p')

    if [[ "$VERSION" == "$PUBLISHED_VERSION" ]]; then
        echo "    Already published ($VERSION), skipping"
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    if [[ "$DRY_RUN" == "true" ]]; then
        echo "    Would publish $VERSION (current: ${PUBLISHED_VERSION:-none})"
        cargo publish -p "$crate" --dry-run --allow-dirty 2>&1 | tail -3 || true
        PUBLISHED=$((PUBLISHED + 1))
        continue
    fi

    echo "    Publishing $VERSION..."
    if cargo publish -p "$crate" --no-verify; then
        echo "    Published $crate $VERSION"
        PUBLISHED=$((PUBLISHED + 1))

        # Wait for the crate to appear in the registry index before
        # publishing dependents that reference it.
        echo "    Waiting for registry index..."
        sleep 25
    else
        echo "    FAILED to publish $crate"
        FAILED=$((FAILED + 1))
    fi
done

echo ""
echo "==> Done: $PUBLISHED published, $SKIPPED skipped, $FAILED failed"

if [[ $FAILED -gt 0 ]]; then
    exit 1
fi
