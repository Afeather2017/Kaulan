#!/bin/bash
# Build and sign Kaulan Android APK
# This script builds and signs the APK for installation

set -e

# Configuration
ANDROID_HOME="${ANDROID_HOME:-$HOME/.local/android}"
BUILD_TOOLS_VERSION="${BUILD_TOOLS_VERSION:-33.0.1}"
BUILD_TOOLS="$ANDROID_HOME/build-tools/$BUILD_TOOLS_VERSION"
KEYSTORE="${ANDROID_HOME}/release.keystore"
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

# Step 1: Build unsigned APK
echo "[1/4] Building APK with Tauri..."
cd "$FRONTEND_DIR"
npx tauri android build --target aarch64

# Step 2: Generate keystore if needed
echo
echo "[2/4] Checking keystore..."
if [ ! -f "$KEYSTORE" ]; then
    echo "  Generating new keystore..."
    keytool -genkey -v -keystore "$KEYSTORE" -alias "$KEY_ALIAS" \
        -keyalg RSA -keysize 2048 -validity 10000 \
        -storepass "$KEY_STOREPASS" -keypass "$KEY_KEYPASS" \
        -dname "CN=Kaulan,O=Kaulan,C=US"
else
    echo "  Using existing keystore: $KEYSTORE"
fi

rm -f $ALIGNED_APK $SIGNED_APK

# Step 3: Zipalign
echo
echo "[3/4] Zipaligning APK..."
"$BUILD_TOOLS/zipalign" -v -p 4 "$UNSIGNED_APK" "$ALIGNED_APK"

# Step 4: Sign APK
echo
echo "[4/4] Signing APK..."
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

echo "Installing..."
# Optionally install immediately
adb install "$SIGNED_APK"
echo "Installation complete!"

