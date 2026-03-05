#!/bin/bash
# Creates a signed, notarized macOS DMG for Enya.
#
# Required environment variables:
#   ENYA_VERSION              - Release version (e.g., "0.1.0")
#
# Optional environment variables (for code signing + notarization):
#   MACOS_CERTIFICATE         - Base64-encoded Developer ID Application .p12
#   MACOS_CERTIFICATE_PASSWORD - Password for the .p12 certificate
#   APPLE_NOTARIZATION_KEY    - Contents of the .p8 API key
#   APPLE_NOTARIZATION_KEY_ID - API key ID from App Store Connect
#   APPLE_NOTARIZATION_ISSUER_ID - Issuer ID from App Store Connect
#
# If signing secrets are not set, the script produces an unsigned DMG (for forks).

set -euo pipefail

# ---- Configuration ----
APP_NAME="Enya"
VERSION="${ENYA_VERSION:?ENYA_VERSION must be set}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_BUNDLE="${APP_NAME}.app"
DMG_NAME="${APP_NAME}.dmg"

# ---- Detect signing capability ----
CAN_SIGN=false
if [[ -n "${MACOS_CERTIFICATE:-}" && -n "${MACOS_CERTIFICATE_PASSWORD:-}" ]]; then
    CAN_SIGN=true
fi

CAN_NOTARIZE=false
if [[ "$CAN_SIGN" == "true" \
    && -n "${APPLE_NOTARIZATION_KEY:-}" \
    && -n "${APPLE_NOTARIZATION_KEY_ID:-}" \
    && -n "${APPLE_NOTARIZATION_ISSUER_ID:-}" ]]; then
    CAN_NOTARIZE=true
fi

echo "==> Signing available: $CAN_SIGN"
echo "==> Notarization available: $CAN_NOTARIZE"

# ---- Cleanup handler ----
KEYCHAIN_PATH=""
NOTARY_KEY_PATH=""

cleanup() {
    if [[ -n "$KEYCHAIN_PATH" ]]; then
        echo "==> Cleaning up keychain"
        security delete-keychain "$KEYCHAIN_PATH" 2>/dev/null || true
    fi
    if [[ -n "$NOTARY_KEY_PATH" ]]; then
        rm -f "$NOTARY_KEY_PATH"
    fi
}
trap cleanup EXIT

# ---- Keychain setup ----
SIGNING_IDENTITY=""

if [[ "$CAN_SIGN" == "true" ]]; then
    echo "==> Setting up ephemeral keychain"
    KEYCHAIN_PASSWORD="$(openssl rand -hex 32)"
    KEYCHAIN_PATH="${RUNNER_TEMP:-/tmp}/enya-signing.keychain-db"

    security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
    security set-keychain-settings -lut 21600 "$KEYCHAIN_PATH"
    security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"

    # Import certificate
    CERT_PATH="${RUNNER_TEMP:-/tmp}/enya-certificate.p12"
    echo "$MACOS_CERTIFICATE" | base64 --decode > "$CERT_PATH"
    security import "$CERT_PATH" \
        -k "$KEYCHAIN_PATH" \
        -P "$MACOS_CERTIFICATE_PASSWORD" \
        -T /usr/bin/codesign \
        -T /usr/bin/security
    rm -f "$CERT_PATH"

    # Allow codesign to access the keychain without UI prompts
    security set-key-partition-list -S apple-tool:,apple: \
        -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"

    # Add ephemeral keychain to the search list
    security list-keychains -d user -s "$KEYCHAIN_PATH" \
        $(security list-keychains -d user | tr -d '"')

    # Extract signing identity
    SIGNING_IDENTITY=$(security find-identity -v -p codesigning "$KEYCHAIN_PATH" \
        | grep "Developer ID Application" | head -1 | awk -F'"' '{print $2}')

    if [[ -z "$SIGNING_IDENTITY" ]]; then
        echo "ERROR: No 'Developer ID Application' identity found in certificate"
        exit 1
    fi
    echo "==> Signing identity: $SIGNING_IDENTITY"
fi

if [[ "$CAN_NOTARIZE" == "true" ]]; then
    NOTARY_KEY_PATH="${RUNNER_TEMP:-/tmp}/enya-notarization-key.p8"
    echo "$APPLE_NOTARIZATION_KEY" > "$NOTARY_KEY_PATH"
fi

# ---- Create .app bundle ----
echo "==> Creating app bundle"
rm -rf "$APP_BUNDLE"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

cp "target/universal-apple-darwin/release/Enya" "$APP_BUNDLE/Contents/MacOS/"
cp "${ROOT_DIR}/assets/icon.icns" "$APP_BUNDLE/Contents/Resources/Enya.icns"

sed "s/__VERSION__/${VERSION}/g" \
    "${ROOT_DIR}/assets/macos/Info.plist.template" \
    > "$APP_BUNDLE/Contents/Info.plist"

# ---- Code signing ----
if [[ "$CAN_SIGN" == "true" ]]; then
    ENTITLEMENTS="${ROOT_DIR}/assets/macos/Enya.entitlements"
    echo "==> Signing app bundle (hardened runtime)"

    # Sign the main binary first (bottom-up signing)
    codesign --force --options runtime --timestamp \
        --sign "$SIGNING_IDENTITY" \
        --entitlements "$ENTITLEMENTS" \
        "$APP_BUNDLE/Contents/MacOS/Enya"

    # Sign the .app bundle
    codesign --force --options runtime --timestamp \
        --sign "$SIGNING_IDENTITY" \
        --entitlements "$ENTITLEMENTS" \
        "$APP_BUNDLE"

    # Verify signature
    codesign --verify --deep --strict "$APP_BUNDLE"
    echo "==> Code signing verified"
else
    echo "==> Skipping code signing (no certificate configured)"
fi

# ---- Create DMG ----
echo "==> Creating DMG"

if ! command -v create-dmg &>/dev/null; then
    echo "==> Installing create-dmg"
    brew install create-dmg
fi

DMG_BACKGROUND="${ROOT_DIR}/assets/macos/dmg-background.png"

# create-dmg returns exit code 2 when it cannot set the custom icon
# (common in headless CI without Finder). Treat exit code 2 as success.
set +e
create-dmg \
    --volname "Enya" \
    --volicon "${ROOT_DIR}/assets/icon.icns" \
    --background "$DMG_BACKGROUND" \
    --window-pos 200 120 \
    --window-size 800 400 \
    --icon-size 80 \
    --icon "Enya.app" 200 190 \
    --hide-extension "Enya.app" \
    --app-drop-link 600 190 \
    --no-internet-enable \
    "$DMG_NAME" \
    "$APP_BUNDLE"
CREATE_DMG_EXIT=$?
set -e

if [[ $CREATE_DMG_EXIT -ne 0 && $CREATE_DMG_EXIT -ne 2 ]]; then
    echo "ERROR: create-dmg failed with exit code $CREATE_DMG_EXIT"
    exit 1
fi

# ---- Sign the DMG ----
if [[ "$CAN_SIGN" == "true" ]]; then
    echo "==> Signing DMG"
    codesign --force --timestamp \
        --sign "$SIGNING_IDENTITY" \
        "$DMG_NAME"
    codesign --verify "$DMG_NAME"
    echo "==> DMG signed"
fi

# ---- Notarize and staple ----
if [[ "$CAN_NOTARIZE" == "true" ]]; then
    echo "==> Submitting DMG for notarization (this may take several minutes)"
    xcrun notarytool submit "$DMG_NAME" \
        --key "$NOTARY_KEY_PATH" \
        --key-id "$APPLE_NOTARIZATION_KEY_ID" \
        --issuer "$APPLE_NOTARIZATION_ISSUER_ID" \
        --wait \
        --timeout 60m

    echo "==> Stapling notarization ticket"
    xcrun stapler staple "$DMG_NAME"
    echo "==> Notarization complete"
else
    echo "==> Skipping notarization (credentials not configured)"
fi

echo "==> Done: $DMG_NAME"
