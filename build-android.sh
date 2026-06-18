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
ANDROID_FFMPEG_ROOT="$PROJECT_ROOT/build/android-ffmpeg/android"

DEFAULT_ANDROID_TARGETS=(
    aarch64
    armv7
    i686
    x86_64
)

resolve_android_rust_target() {
    case "$1" in
        aarch64)
            printf '%s\n' "aarch64-linux-android"
            ;;
        armv7)
            printf '%s\n' "armv7-linux-androideabi"
            ;;
        i686)
            printf '%s\n' "i686-linux-android"
            ;;
        x86_64)
            printf '%s\n' "x86_64-linux-android"
            ;;
        *)
            printf '%s\n' ""
            return 1
            ;;
    esac
}

collect_requested_android_targets() {
    if [ "$#" -eq 0 ]; then
        printf '%s\n' "${DEFAULT_ANDROID_TARGETS[@]}"
        return 0
    fi

    local expecting_target_value=false
    local arg=""
    local raw_target=""

    for arg in "$@"; do
        if [ "$expecting_target_value" = true ]; then
            raw_target="$arg"
            expecting_target_value=false
        elif [ "$arg" = "--target" ] || [ "$arg" = "-t" ]; then
            expecting_target_value=true
            continue
        elif [[ "$arg" == --target=* ]]; then
            raw_target="${arg#--target=}"
        elif [[ "$arg" == -t=* ]]; then
            raw_target="${arg#-t=}"
        else
            continue
        fi

        local split_target=""
        local trimmed_target=""
        for split_target in ${raw_target//,/ }; do
            trimmed_target="${split_target// /}"
            if [ -n "$trimmed_target" ]; then
                printf '%s\n' "$trimmed_target"
            fi
        done
        raw_target=""
    done

    if [ "$expecting_target_value" = true ]; then
        echo "Missing value for --target." >&2
        exit 1
    fi
}

validate_android_ffmpeg_bundle() {
    local requested_targets=("$@")
    local missing=()

    if [ ! -f "$ANDROID_FFMPEG_ROOT/binding.rs" ]; then
        missing+=("$ANDROID_FFMPEG_ROOT/binding.rs")
    fi

    local target=""
    local rust_target=""
    for target in "${requested_targets[@]}"; do
        rust_target="$(resolve_android_rust_target "$target")" || {
            echo "Unsupported Android target: $target" >&2
            exit 1
        }

        if [ ! -d "$ANDROID_FFMPEG_ROOT/$rust_target/lib" ]; then
            missing+=("$ANDROID_FFMPEG_ROOT/$rust_target/lib")
        fi
        if [ ! -d "$ANDROID_FFMPEG_ROOT/$rust_target/prefix/include" ]; then
            missing+=("$ANDROID_FFMPEG_ROOT/$rust_target/prefix/include")
        fi
    done

    if [ "${#missing[@]}" -eq 0 ]; then
        return 0
    fi

    echo "Missing staged Android FFmpeg bundle files required for this build:" >&2
    printf '  - %s\n' "${missing[@]}" >&2
    echo >&2
    echo "Stage the Android FFmpeg bundle under build/android-ffmpeg/android before building." >&2
    exit 1
}

export_single_target_android_ffmpeg_env() {
    local requested_target="$1"
    local rust_target=""
    rust_target="$(resolve_android_rust_target "$requested_target")" || return 1

    export FFMPEG_LIBS_DIR="$ANDROID_FFMPEG_ROOT/$rust_target/lib"
    export FFMPEG_INCLUDE_DIR="$ANDROID_FFMPEG_ROOT/$rust_target/prefix/include"
    export FFMPEG_BINDING_PATH="$ANDROID_FFMPEG_ROOT/binding.rs"
    export FFMPEG_LINK_MODE="dynamic"
}

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

mapfile -t REQUESTED_ANDROID_TARGETS < <(collect_requested_android_targets "$@")
if [ "${#REQUESTED_ANDROID_TARGETS[@]}" -eq 0 ]; then
    REQUESTED_ANDROID_TARGETS=("${DEFAULT_ANDROID_TARGETS[@]}")
fi

echo "Requested Android targets: ${REQUESTED_ANDROID_TARGETS[*]}"
echo

validate_android_ffmpeg_bundle "${REQUESTED_ANDROID_TARGETS[@]}"

if [ "${#REQUESTED_ANDROID_TARGETS[@]}" -eq 1 ]; then
    export_single_target_android_ffmpeg_env "${REQUESTED_ANDROID_TARGETS[0]}"
fi

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
