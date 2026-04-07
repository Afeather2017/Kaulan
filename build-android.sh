#!/bin/bash
# Build Kaulan Android release packages.
# Produces signed APK/AAB bundles when a signing key is available.

set -euo pipefail

ANDROID_HOME="${ANDROID_HOME:-$HOME/.local/android}"
BUILD_TOOLS_VERSION="${BUILD_TOOLS_VERSION:-35.0.0}"
BUILD_TOOLS="$ANDROID_HOME/build-tools/$BUILD_TOOLS_VERSION"
KEYSTORE="${KEYSTORE:-${ANDROID_HOME}/release.keystore}"
KEY_ALIAS="${KEY_ALIAS:-release}"
KEY_STOREPASS="${KEY_STOREPASS:-123456}"
KEY_KEYPASS="${KEY_KEYPASS:-$KEY_STOREPASS}"
CI_MODE="${CI:-false}"
INSTALL_ON_DEVICE="${INSTALL_ON_DEVICE:-true}"

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
ANDROID_DIR="$FRONTEND_DIR/src-tauri/gen/android"
KEYSTORE_PROPERTIES="$ANDROID_DIR/keystore.properties"
APK_GLOB="$ANDROID_DIR/app/build/outputs/apk"
AAB_GLOB="$ANDROID_DIR/app/build/outputs/bundle"

cleanup() {
    rm -f "$KEYSTORE_PROPERTIES"
}
trap cleanup EXIT

write_keystore_properties() {
    cat > "$KEYSTORE_PROPERTIES" <<EOF
keyAlias=$KEY_ALIAS
password=$KEY_STOREPASS
storeFile=$KEYSTORE
EOF
}

echo "=== Kaulan Android Build Script ==="
echo "Project root: $PROJECT_ROOT"
echo "Android build tools: $BUILD_TOOLS"
echo "CI mode: $CI_MODE"
echo

if [ "$CI_MODE" = "true" ] && [ -z "${ANDROID_KEY_BASE64:-}" ]; then
    echo "ANDROID_KEY_BASE64 is required for CI release builds." >&2
    exit 1
fi

if [ -n "${ANDROID_KEY_BASE64:-}" ]; then
    echo "[1/4] Restoring CI keystore..."
    KEYSTORE="${RUNNER_TEMP:-/tmp}/kaulan-upload-keystore.jks"
    printf '%s' "$ANDROID_KEY_BASE64" | base64 --decode > "$KEYSTORE"
    KEY_ALIAS="${ANDROID_KEY_ALIAS:?ANDROID_KEY_ALIAS is required when ANDROID_KEY_BASE64 is set}"
    KEY_STOREPASS="${ANDROID_KEY_PASSWORD:?ANDROID_KEY_PASSWORD is required when ANDROID_KEY_BASE64 is set}"
    KEY_KEYPASS="${ANDROID_KEY_PASSWORD}"
    write_keystore_properties
elif [ -f "$KEYSTORE" ]; then
    echo "[1/4] Using existing keystore: $KEYSTORE"
    write_keystore_properties
else
    echo "[1/4] Generating local keystore..."
    keytool -genkey -v -keystore "$KEYSTORE" -alias "$KEY_ALIAS" \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass "$KEY_STOREPASS" -keypass "$KEY_KEYPASS" \
        -dname "CN=Kaulan,O=Kaulan,C=US"
    write_keystore_properties
fi

echo
echo "[2/4] Building signed Android packages..."
cd "$FRONTEND_DIR"
npx tauri android build --ci --apk --aab "$@"

echo
echo "[3/4] Collecting build outputs..."
find "$APK_GLOB" -type f \( -name '*.apk' -o -name '*mapping.txt' \) | sort || true
find "$AAB_GLOB" -type f -name '*.aab' | sort || true

SIGNED_APK="$(find "$APK_GLOB" -type f -name '*release*.apk' ! -name '*unsigned.apk' | sort | head -n 1 || true)"
if [ -n "$SIGNED_APK" ]; then
    echo
    echo "Primary signed APK: $SIGNED_APK"
fi

echo
echo "[4/4] Finishing..."
if [ "$CI_MODE" = "true" ] || [ "$INSTALL_ON_DEVICE" != "true" ]; then
    echo "Skipping device install."
    exit 0
fi

if [ -z "$SIGNED_APK" ]; then
    echo "No signed APK found to install."
    exit 0
fi

if command -v adb >/dev/null 2>&1; then
    if adb install -r "$SIGNED_APK"; then
        echo "Installation complete!"
    else
        echo "adb install failed or no device found; skipping."
    fi
else
    echo "adb not found; skipping install."
fi
