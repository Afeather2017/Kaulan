# Android Build Documentation

This document covers building and signing the Android APK for Kaulan using Tauri.

## Environment

### Android SDK Location

Install Android SDK first, say:

```
ANDROID_HOME=~/.local/android/
```

### Build Tools Path

Load the path.

```
/home/afeather/.local/android/build-tools/33.0.1/
```

Contains: `zipalign`, `apksigner`, `aapt`, etc.

## Building the APK

### Recommended: Use the Build Script

The easiest way is to use the provided build script which handles the Tauri symlink bug and signing:

```bash
./build-android.sh
```

This script:
1. Fixes the `src-tauri/tauri` symlink bug
2. Builds the unsigned APK
3. Generates/uses the keystore
4. Zipaligns and signs the APK
5. Optionally installs to connected device

### Manual Build

```bash
cd frontend

# Build the unsigned APK
npx tauri android build
```

Output location:
```
src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk
```

### Tauri Symlink Bug

Tauri creates a symlink at `frontend/src-tauri/tauri` that points to `../node_modules/@tauri-apps/cli/tauri.js`. This relative path may not resolve correctly during Android build.

**Current symlink:**
```bash
frontend/src-tauri/tauri -> ../node_modules/@tauri-apps/cli/tauri.js
```

The `build-android.sh` script automatically fixes this before building. If building manually, ensure the symlink exists:

```bash
cd frontend/src-tauri
ln -sf ../node_modules/@tauri-apps/cli/tauri.js tauri
```

## Signing the APK

The built APK is unsigned and must be signed before installation.

### Step 1: Generate a Keystore

```bash
keytool -genkey -v -keystore /tmp/release.keystore -alias release \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -storepass 123456 -keypass 123456 \
  -dname "CN=Kaulan,O=Kaulan,C=US"
```

### Step 2: Zipalign the APK

```bash
/home/afeather/.local/android/build-tools/33.0.1/zipalign -v -p 4 \
  src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk \
  /tmp/app-aligned.apk
```

### Step 3: Sign the APK

```bash
/home/afeather/.local/android/build-tools/33.0.1/apksigner sign \
  --ks /tmp/release.keystore \
  --ks-pass pass:123456 \
  --key-pass pass:123456 \
  --out /tmp/app-signed.apk \
  /tmp/app-aligned.apk
```

### Step 4: Install to Device

```bash
adb install /tmp/app-signed.apk
```

## Quick One-Liner

For future builds, you can combine the steps:

```bash
# Build, align, sign, and install
npx tauri android build && \
/home/afeather/.local/android/build-tools/33.0.1/zipalign -v -p 4 \
  src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk \
  /tmp/app-aligned.apk && \
/home/afeather/.local/android/build-tools/33.0.1/apksigner sign \
  --ks /tmp/release.keystore --ks-pass pass:123456 --key-pass pass:123456 \
  --out /tmp/app-signed.apk /tmp/app-aligned.apk && \
adb install /tmp/app-signed.apk
```

## Troubleshooting

### Error: INSTALL_PARSE_FAILED_NO_CERTIFICATES
This means the APK is unsigned. Follow the signing steps above.

### Finding Android build tools version
```bash
ls $ANDROID_HOME/build-tools/
```

### Verifying APK signature
```bash
$ANDROID_HOME/build-tools/33.0.1/apksigner verify --verbose /tmp/app-signed.apk
```

## Keystore Management

For production builds, you should:
1. Store the keystore in a secure location (not `/tmp`)
2. Use strong passwords
3. Keep backups of the keystore (cannot recover if lost)
4. Never commit the keystore to version control

