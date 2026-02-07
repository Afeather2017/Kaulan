#!/bin/bash
# Build and sign Kaulan Android APK
# This script handles the Tauri symlink bug and signs the APK for installation

set -e

# Configuration
ANDROID_HOME="${ANDROID_HOME:-$HOME/.local/android}"
BUILD_TOOLS_VERSION="${BUILD_TOOLS_VERSION:-33.0.1}"
BUILD_TOOLS="$ANDROID_HOME/build-tools/$BUILD_TOOLS_VERSION"
KEYSTORE="/tmp/release.keystore"
KEY_ALIAS="release"
KEY_STOREPASS="123456"
KEY_KEYPASS="123456"

# Project paths
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
SRC_TAURI="$FRONTEND_DIR/src-tauri"
UNSIGNED_APK="$SRC_TAURI/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
ALIGNED_APK="/tmp/app-aligned.apk"
SIGNED_APK="/tmp/app-signed.apk"

echo "=== Kaulan Android Build Script ==="
echo "Project root: $PROJECT_ROOT"
echo "Android build tools: $BUILD_TOOLS"
echo

# Step 1: Initialize Android project with Tauri
echo "[1/6] Initializing Android project with Tauri..."
cd "$FRONTEND_DIR"
npx tauri android init
echo

# Step 2: Fix Tauri symlink bug
echo "[2/6] Fixing Tauri symlink..."
TAURI_SYMLINK="$SRC_TAURI/tauri"
TAURI_TARGET="../node_modules/@tauri-apps/cli/tauri.js"

# Remove existing symlink if broken or wrong
if [ -L "$TAURI_SYMLINK" ]; then
    if [ ! -e "$TAURI_SYMLINK" ] || [ "$(readlink "$TAURI_SYMLINK")" != "$TAURI_TARGET" ]; then
        rm "$TAURI_SYMLINK"
    fi
fi

# Create symlink if it doesn't exist
if [ ! -e "$TAURI_SYMLINK" ]; then
    (cd "$SRC_TAURI" && ln -s "$TAURI_TARGET" tauri)
    echo "  Created symlink: $TAURI_SYMLINK -> $TAURI_TARGET"
else
    echo "  Symlink OK"
fi

# Step 3: Build unsigned APK
echo
echo "[3/6] Building APK with Tauri..."
cd "$FRONTEND_DIR"
npx tauri android build

# Step 4: Generate keystore if needed
echo
echo "[4/6] Checking keystore..."
if [ ! -f "$KEYSTORE" ]; then
    echo "  Generating new keystore..."
    keytool -genkey -v -keystore "$KEYSTORE" -alias "$KEY_ALIAS" \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass "$KEY_STOREPASS" -keypass "$KEY_KEYPASS" \
        -dname "CN=Kaulan,O=Kaulan,C=US"
else
    echo "  Using existing keystore: $KEYSTORE"
fi

rm -f $UNSIGNED_APK $SIGNED_APK

# Step 5: Zipalign
echo
echo "[5/6] Zipaligning APK..."
"$BUILD_TOOLS/zipalign" -v -p 4 "$UNSIGNED_APK" "$ALIGNED_APK"

# Step 6: Sign APK
echo
echo "[6/6] Signing APK..."
"$BUILD_TOOLS/apksigner" sign \
    --ks "$KEYSTORE" \
    --ks-pass "pass:$KEY_STOREPASS" \
    --key-pass "pass:$KEY_KEYPASS" \
    --out "$SIGNED_APK" \
    "$ALIGNED_APK"

# Done
echo
echo "=== Build Complete ==="
echo "Signed APK: $SIGNED_APK"
echo
echo "To install, run:"
echo "  adb install $SIGNED_APK"
echo

# Optionally install immediately
read -p "Install to device now? [y/N] " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    adb install "$SIGNED_APK"
    echo "Installation complete!"
fi

