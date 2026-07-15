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
ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/.local/android}}"
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-}"
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
DEFAULT_TARGETS=(
    "aarch64-linux-android"
    "armv7-linux-androideabi"
    "i686-linux-android"
    "x86_64-linux-android"
)
REQUESTED_TARGETS=()

resolve_target_triple() {
    case "$1" in
        aarch64|arm64-v8a|aarch64-linux-android)
            printf '%s\n' "aarch64-linux-android"
            ;;
        armv7|armeabi-v7a|armv7-linux-androideabi)
            printf '%s\n' "armv7-linux-androideabi"
            ;;
        i686|x86|i686-linux-android)
            printf '%s\n' "i686-linux-android"
            ;;
        x86_64|x86_64-linux-android)
            printf '%s\n' "x86_64-linux-android"
            ;;
        *)
            printf '%s\n' ""
            return 1
            ;;
    esac
}

parse_args() {
    if [ "$#" -eq 0 ]; then
        REQUESTED_TARGETS=("${DEFAULT_TARGETS[@]}")
        return 0
    fi

    local expecting_target_value=false
    local raw_targets=""
    local item=""
    local normalized=""

    for item in "$@"; do
        if [ "$expecting_target_value" = true ]; then
            raw_targets="$item"
            expecting_target_value=false
        elif [ "$item" = "--target" ] || [ "$item" = "-t" ]; then
            expecting_target_value=true
            continue
        elif [[ "$item" == --target=* ]]; then
            raw_targets="${item#--target=}"
        elif [[ "$item" == -t=* ]]; then
            raw_targets="${item#-t=}"
        else
            echo "Unsupported argument: $item" >&2
            echo "Usage: $0 [--target <aarch64|armv7|i686|x86_64>[,<...>]]" >&2
            exit 1
        fi

        local split_target=""
        for split_target in ${raw_targets//,/ }; do
            normalized="$(resolve_target_triple "$split_target")" || {
                echo "Unsupported Android target: $split_target" >&2
                exit 1
            }
            REQUESTED_TARGETS+=("$normalized")
        done
        raw_targets=""
    done

    if [ "$expecting_target_value" = true ]; then
        echo "Missing value for --target." >&2
        exit 1
    fi

    if [ "${#REQUESTED_TARGETS[@]}" -eq 0 ]; then
        REQUESTED_TARGETS=("${DEFAULT_TARGETS[@]}")
    fi
}

detect_android_ndk_home() {
    local host_tag
    case "$(uname -s)" in
        Linux*) host_tag="linux-x86_64" ;;
        Darwin*) host_tag="darwin-x86_64" ;;
        MINGW*|MSYS*|CYGWIN*) host_tag="windows-x86_64" ;;
        *) host_tag="linux-x86_64" ;;
    esac

    if [ -n "$ANDROID_NDK_HOME" ] && [ -d "$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$host_tag/bin" ]; then
        printf '%s\n' "$ANDROID_NDK_HOME"
        return 0
    fi

    local ndk_root="$ANDROID_HOME/ndk"
    if [ ! -d "$ndk_root" ]; then
        echo "Android NDK directory not found under $ndk_root" >&2
        echo "Set ANDROID_HOME/ANDROID_SDK_ROOT or ANDROID_NDK_HOME before running this script." >&2
        exit 1
    fi

    local detected=""
    detected="$(
        find "$ndk_root" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' \
            | sort -V \
            | tail -n 1
    )"

    if [ -z "$detected" ]; then
        echo "No installed Android NDK version found under $ndk_root" >&2
        exit 1
    fi

    printf '%s\n' "$ndk_root/$detected"
}

# NDK prebuilt host tag — matches the OS this script runs on. Used to locate
# the NDK toolchain binaries (clang, ld, etc.) which are host-OS-specific.
ndk_host_tag() {
    case "$(uname -s)" in
        Linux*) printf '%s\n' "linux-x86_64" ;;
        Darwin*) printf '%s\n' "darwin-x86_64" ;;
        MINGW*|MSYS*|CYGWIN*) printf '%s\n' "windows-x86_64" ;;
        *) printf '%s\n' "linux-x86_64" ;;
    esac
}

copy_prebuilt_binding() {
    if [ ! -f "$ANDROID_ROOT/binding.rs" ]; then
        cp "$PROJECT_ROOT/vendor/rusty_ffmpeg/src/binding.rs" "$ANDROID_ROOT/binding.rs"
    fi
}

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

    # FFmpeg's response-file recipe uses `echo $^ > $@.objs` to write the .o list
    # before linking. On Windows, native GNU Make runs this through cmd.exe,
    # which silently truncates the redirect when the .o list exceeds cmd.exe's
    # ~8KB command-line limit. The @file then ends up empty or missing and the
    # link fails. We install a tiny bash helper and patch library.mak to call it
    # — bash's redirect isn't subject to the cmd.exe limit.
    if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* || "$(uname -s)" == CYGWIN* ]]; then
        local helper="$SRC_DIR/write_objs.sh"
        cat > "$helper" <<'EOF'
#!/bin/bash
# Write all args (except the first) space-separated to the file named by the first arg.
output="$1"
shift
printf '%s ' "$@" > "$output"
EOF
        chmod +x "$helper"
        # Replace both `echo ... > $@.objs` recipes with a bash-helper call.
        local mak="$SRC_DIR/ffbuild/library.mak"
        sed -i 's|\$(Q)echo \$$^ > \$$@\.objs|$(Q)bash $(SRC_PATH)/write_objs.sh $$@.objs $$^|' "$mak"
        sed -i 's|\$(Q)echo \$\$(filter %\.o,\$\$^) > \$\$@\.objs|$(Q)bash $(SRC_PATH)/write_objs.sh $$@.objs $$(filter %.o,$$^)|' "$mak"
    fi
}

target_requested() {
    local needle="$1"
    local requested=""
    for requested in "${REQUESTED_TARGETS[@]}"; do
        if [ "$requested" = "$needle" ]; then
            return 0
        fi
    done
    return 1
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

    if ! target_requested "$triple"; then
        return 0
    fi

    if [ "$triple" = "armv7-linux-androideabi" ]; then
        cc="$TOOLCHAIN/bin/armv7a-linux-androideabi${ANDROID_API_LEVEL}-clang"
        cxx="$TOOLCHAIN/bin/armv7a-linux-androideabi${ANDROID_API_LEVEL}-clang++"
    elif [ "$triple" = "i686-linux-android" ]; then
        # Android x86 shared builds hit non-PIC relocations across FFmpeg's x86-specific code.
        # Disable asm entirely for this emulator ABI so the shared libraries remain linkable.
        extra_config+=(--disable-asm)
    fi

    mkdir -p "$target_root" "$jni_dir"

    if compgen -G "$lib_dir/*.so" >/dev/null; then
        echo "Using cached Android FFmpeg build for $triple"
    else
        echo "Building Android FFmpeg for $triple ($abi)..."
        rm -rf "$prefix" "$lib_dir"
        mkdir -p "$prefix" "$lib_dir"

        pushd "$SRC_DIR" >/dev/null
        make distclean >/dev/null 2>&1 || true
        # FFmpeg's configure builds tiny host-side helper tools during make,
        # so it needs a working native C compiler. On Linux CI `gcc` is the
        # default; on Windows Git Bash there's no gcc on PATH, so fall back to
        # LLVM clang if available.
        local host_cc_args=()
        if command -v gcc >/dev/null 2>&1; then
            : # FFmpeg defaults to gcc, no flag needed.
        elif command -v clang >/dev/null 2>&1; then
            host_cc_args+=(--host-cc=clang --host-cflags=-O2)
        fi
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
            "${host_cc_args[@]}" \
            "${extra_config[@]}"
        make -j"$(getconf _NPROCESSORS_ONLN)"
        make install
        popd >/dev/null

        cp -a "$prefix/lib/." "$lib_dir/"
    fi

    find "$jni_dir" -maxdepth 1 -type f -name '*.so' -delete
    cp -a "$lib_dir/." "$jni_dir/"
}

parse_args "$@"

ANDROID_NDK_HOME="$(detect_android_ndk_home)"
TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$(ndk_host_tag)"

if [ ! -d "$TOOLCHAIN/bin" ]; then
    echo "Android NDK toolchain not found at $TOOLCHAIN" >&2
    exit 1
fi

mkdir -p "$WORK_ROOT" "$ANDROID_ROOT" "$ANDROID_JNI_LIBS_DIR"
copy_prebuilt_binding

echo "=== Kaulan Android FFmpeg Builder ==="
echo "Project root: $PROJECT_ROOT"
echo "FFmpeg tag: $FFMPEG_TAG"
echo "Android SDK: $ANDROID_HOME"
echo "Android NDK: $ANDROID_NDK_HOME"
echo "Requested targets: ${REQUESTED_TARGETS[*]}"
echo

download_ffmpeg_source

for entry in "${TARGETS[@]}"; do
    IFS="|" read -r triple abi arch host <<<"$entry"
    build_target "$triple" "$abi" "$arch" "$host"
done

echo
echo "Android FFmpeg staging complete:"
echo "  Shared libs: $ANDROID_JNI_LIBS_DIR/<abi>"
echo "  Rust link root: $ANDROID_ROOT/<target>/lib"
