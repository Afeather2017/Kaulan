#!/usr/bin/env bash
set -euo pipefail

# Standalone server packaging documented in docs/server-package.md.

if [ "$#" -ne 3 ]; then
    echo "Usage: $0 <version> <architecture> <backend-binary>" >&2
    exit 2
fi

VERSION="$1"
ARCH="$2"
BINARY="$3"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_ROOT="${PROJECT_ROOT}/target/server"
DOC_FILE="${PROJECT_ROOT}/docs/server-package.md"
ICON_FILE="${PROJECT_ROOT}/frontend/src-tauri/icons/128x128.png"

case "$ARCH" in
    x86_64)
        APPIMAGE_ARCH="x86_64"
        ;;
    aarch64|arm64)
        APPIMAGE_ARCH="aarch64"
        ;;
    *)
        echo "Unsupported architecture: $ARCH" >&2
        exit 1
        ;;
esac

if [ ! -x "$BINARY" ]; then
    echo "Backend binary is missing or not executable: $BINARY" >&2
    exit 1
fi
if [ ! -f "$DOC_FILE" ] || [ ! -f "$ICON_FILE" ]; then
    echo "Server package documentation or icon is missing" >&2
    exit 1
fi

require_tool() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "Missing required tool: $1" >&2
        exit 1
    }
}

require_tool curl

rm -rf "$OUTPUT_ROOT"
mkdir -p "$OUTPUT_ROOT"

WORK_ROOT="$(mktemp -d -t kaulan-server-appimage-XXXXXX)"
cleanup() { rm -rf "$WORK_ROOT"; }
trap cleanup EXIT

APPDIR="$WORK_ROOT/Kaulan_Server.AppDir"
DESKTOP_FILE="$WORK_ROOT/kaulan-server.desktop"
PACKAGED_ICON="$WORK_ROOT/kaulan-server.png"
LINUXDEPLOY="$WORK_ROOT/linuxdeploy.AppImage"
OUTPUT_FILE="$OUTPUT_ROOT/kaulan-server-linux-$APPIMAGE_ARCH-$VERSION.AppImage"

install -D -m 0755 "$BINARY" "$APPDIR/usr/bin/kaulan-server"
install -m 0644 "$ICON_FILE" "$PACKAGED_ICON"

# AppImage metadata is kept inside the image so packaging tools can identify
# its entry point. Running the server does not install a desktop entry or icon.
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Kaulan Server
Exec=kaulan-server
Icon=kaulan-server
Terminal=true
Categories=Network;
EOF

curl --fail --location --retry 3 \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${APPIMAGE_ARCH}.AppImage" \
    --output "$LINUXDEPLOY"
chmod +x "$LINUXDEPLOY"

# linuxdeploy follows the binary's dependency graph, including the FFmpeg
# libraries selected through LD_LIBRARY_PATH by ci-install-ffmpeg-linux.sh.
# APPIMAGE_EXTRACT_AND_RUN avoids requiring FUSE on GitHub-hosted runners.
APPIMAGE_EXTRACT_AND_RUN=1 "$LINUXDEPLOY" \
    --appdir "$APPDIR" \
    --executable "$APPDIR/usr/bin/kaulan-server" \
    --desktop-file "$DESKTOP_FILE" \
    --icon-file "$PACKAGED_ICON"

install -D -m 0644 "$DOC_FILE" \
    "$APPDIR/usr/share/doc/kaulan-server/README.md"

# linuxdeploy's appimage plugin has used both LDAI_OUTPUT and OUTPUT for its
# destination across releases. Set both so the continuous binary cannot fall
# back to its default AppImage name (which would make the subsequent smoke
# test fail and leave no artifact for the release upload).
ARCH="$APPIMAGE_ARCH" OUTPUT="$OUTPUT_FILE" LDAI_OUTPUT="$OUTPUT_FILE" APPIMAGE_EXTRACT_AND_RUN=1 \
    "$LINUXDEPLOY" --appdir "$APPDIR" --output appimage

chmod +x "$OUTPUT_FILE"
# The standalone binary has command-based CLI parsing and does not implement
# a --help flag. Run a harmless database update against an empty directory so
# the smoke test verifies that the packaged executable can start and exit.
SMOKE_MUSIC_ROOT="$WORK_ROOT/smoke-music"
mkdir -p "$SMOKE_MUSIC_ROOT"
APPIMAGE_EXTRACT_AND_RUN=1 "$OUTPUT_FILE" update "$SMOKE_MUSIC_ROOT" >/dev/null
echo "Created standalone server AppImage: $OUTPUT_FILE"
