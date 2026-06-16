#!/bin/bash
# Build and stage FFmpeg shared libraries for Kaulan Android targets.
#
# Documentation: docs/ffmpeg-audio-pipeline.md

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FFMPEG_VERSION="${FFMPEG_VERSION:-8.1.1}"
FFMPEG_TAG="${FFMPEG_TAG:-n${FFMPEG_VERSION}}"
FFMPEG_SOURCE_URL="${FFMPEG_SOURCE_URL:-https://codeload.github.com/FFmpeg/FFmpeg/tar.gz/refs/tags/${FFMPEG_TAG}}"
ANDROID_API_LEVEL="${ANDROID_API_LEVEL:-24}"
ANDROID_NDK_VERSION="${ANDROID_NDK_VERSION:-27.0.12077973}"
ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/$ANDROID_NDK_VERSION}"
TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64"
WORK_ROOT="${KAULAN_ANDROID_FFMPEG_WORK_ROOT:-$PROJECT_ROOT/build/android-ffmpeg}"
SRC_ARCHIVE="$WORK_ROOT/src-${FFMPEG_TAG}.tar.gz"
SRC_DIR="$WORK_ROOT/FFmpeg-${FFMPEG_TAG}"
ANDROID_ROOT="$WORK_ROOT/android"
ANDROID_JNI_LIBS_DIR="$PROJECT_ROOT/frontend/src-tauri/gen/android/app/src/main/jniLibs"
TARGETS=(
  "aarch64-linux-android|arm64-v8a|aarch64|aarch64-linux-android"
  "armv7-linux-androideabi|armeabi-v7a|arm|arm-linux-androideabi"
  "i686-linux-android|x86|x86|i686-linux-android"
  "x86_64-linux-android|x86_64|x86_64|x86_64-linux-android"
)

if [ ! -d "$TOOLCHAIN/bin" ]; then
    echo "Android NDK toolchain not found at $TOOLCHAIN" >&2
    echo "Set ANDROID_HOME/ANDROID_SDK_ROOT/ANDROID_NDK_HOME before running this script." >&2
    exit 1
fi

mkdir -p "$WORK_ROOT" "$ANDROID_ROOT" "$ANDROID_JNI_LIBS_DIR"

if [ ! -f "$ANDROID_ROOT/binding.rs" ]; then
    cp "$PROJECT_ROOT/vendor/rusty_ffmpeg/src/binding.rs" "$ANDROID_ROOT/binding.rs"
fi

download_ffmpeg_source() {
    if [ -d "$SRC_DIR" ]; then
        return
    fi

    if [ ! -f "$SRC_ARCHIVE" ]; then
        echo "Downloading FFmpeg ${FFMPEG_TAG} from GitHub..."
        curl --http1.1 --fail --location --retry 3 --retry-all-errors "$FFMPEG_SOURCE_URL" -o "$SRC_ARCHIVE"
    fi

    echo "Extracting FFmpeg source..."
    tar -xzf "$SRC_ARCHIVE" -C "$WORK_ROOT"
}

build_target() {
    local triple="$1"
    local abi="$2"
    local arch="$3"
    local host="$4"
    local target_root="$ANDROID_ROOT/$triple"
    local prefix="$target_root/prefix"
    local lib_dir="$target_root/lib"
    local jni_dir="$ANDROID_JNI_LIBS_DIR/$abi"
    local cc="$TOOLCHAIN/bin/${host}${ANDROID_API_LEVEL}-clang"
    local cxx="$TOOLCHAIN/bin/${host}${ANDROID_API_LEVEL}-clang++"
    local extra_config=()

    if [ "$triple" = "armv7-linux-androideabi" ]; then
        cc="$TOOLCHAIN/bin/armv7a-linux-androideabi${ANDROID_API_LEVEL}-clang"
        cxx="$TOOLCHAIN/bin/armv7a-linux-androideabi${ANDROID_API_LEVEL}-clang++"
    elif [ "$triple" = "i686-linux-android" ]; then
        # Android x86 shared builds hit non-PIC relocations across FFmpeg's x86-specific code.
        # Disable asm entirely for this emulator ABI so the shared libraries remain linkable.
        extra_config+=(--disable-asm)
    fi

    mkdir -p "$target_root" "$jni_dir"

    if compgen -G "$lib_dir/*.so" > /dev/null; then
        echo "Using cached Android FFmpeg build for $triple"
    else
        echo "Building Android FFmpeg for $triple ($abi)..."
        rm -rf "$prefix" "$lib_dir"
        mkdir -p "$prefix" "$lib_dir"

        pushd "$SRC_DIR" >/dev/null
        make distclean >/dev/null 2>&1 || true
        ./configure \
            --prefix="$prefix" \
            --target-os=android \
            --arch="$arch" \
            --cc="$cc" \
            --cxx="$cxx" \
            --ar="$TOOLCHAIN/bin/llvm-ar" \
            --nm="$TOOLCHAIN/bin/llvm-nm" \
            --ranlib="$TOOLCHAIN/bin/llvm-ranlib" \
            --strip="$TOOLCHAIN/bin/llvm-strip" \
            --sysroot="$TOOLCHAIN/sysroot" \
            --enable-cross-compile \
            --enable-shared \
            --disable-static \
            --disable-doc \
            --disable-debug \
            --disable-ffplay \
            --disable-programs \
            --enable-pic \
            --disable-symver \
            --extra-cflags="-O2 -fPIC" \
            --extra-ldexeflags="-pie" \
            "${extra_config[@]}"
        make -j"$(getconf _NPROCESSORS_ONLN)"
        make install
        popd >/dev/null

        cp -a "$prefix/lib/." "$lib_dir/"
    fi

    find "$jni_dir" -maxdepth 1 -type f -name '*.so' -delete
    cp -a "$lib_dir/." "$jni_dir/"
}

download_ffmpeg_source

for entry in "${TARGETS[@]}"; do
    IFS="|" read -r triple abi arch host <<<"$entry"
    build_target "$triple" "$abi" "$arch" "$host"
done

echo
echo "Android FFmpeg staging complete:"
echo "  Shared libs: $ANDROID_JNI_LIBS_DIR/<abi>"
echo "  Rust link root: $ANDROID_ROOT/<target>/lib"
