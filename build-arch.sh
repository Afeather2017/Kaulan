#!/bin/bash
# Build Kaulan as an Arch Linux pacman package (.pkg.tar.zst).
#
# Tauri does not natively produce pacman packages, so this script converts
# the .deb bundle (which already carries the binary, .desktop file, icons,
# and MIME associations) into pacman format.
#
# Runs on any Linux distro with binutils, tar, and zstd — no makepkg or
# Arch base-system required, so it works in CI as well as on developer
# machines. On Arch itself, you can also use `makepkg -p build/arch/PKGBUILD`
# to install from a published release .deb instead.
#
# Usage:
#   ./build-arch.sh                 # Build .deb then convert (default)
#   ./build-arch.sh --no-build      # Reuse the most recent .deb in target/
#   ./build-arch.sh --deb <path>    # Convert a specific .deb file
#   ./build-arch.sh --help

set -euo pipefail

SKIP_BUILD=false
USE_DEB=""

print_help() {
    cat <<'EOF'
Usage: build-arch.sh [OPTIONS]

Options:
  --no-build        Reuse the most recent .deb in target/ instead of rebuilding
  --deb <path>      Convert a specific .deb file (implies --no-build)
  -h, --help        Show this help message
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --no-build) SKIP_BUILD=true; shift ;;
        --deb)
            [ $# -ge 2 ] || { echo "--deb requires a path argument" >&2; exit 1; }
            USE_DEB="$2"
            SKIP_BUILD=true
            shift 2
            ;;
        -h|--help) print_help; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; print_help >&2; exit 1 ;;
    esac
done

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
OUTPUT_DIR="$PROJECT_ROOT/target/arch"

# Tauri's bundle output location depends on whether the crate is in a cargo
# workspace. This repo has a root workspace, so the .deb lands at
# $PROJECT_ROOT/target/release/bundle/deb. A standalone Tauri app would put
# it under frontend/src-tauri/target/release/bundle/deb. Search both.
TAURI_BUNDLE_DIRS=(
    "$PROJECT_ROOT/target/release/bundle/deb"
    "$FRONTEND_DIR/src-tauri/target/release/bundle/deb"
)
PKG_NAME="kaulan"
PACKAGER="${PACKAGER:-Kaulan Build Script <kaulan@local>}"
PKG_URL="https://github.com/Afeather2017/Kaulan"
PKG_LICENSE="MIT"
PKG_DESC="A Tauri-based music player with Rust actix-web backend and Vue.js frontend."

need_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required tool: $1" >&2
        echo "  Debian/Ubuntu: sudo apt-get install $2" >&2
        echo "  Arch:          sudo pacman -S $3" >&2
        echo "  Fedora/RHEL:   sudo dnf install $4" >&2
        exit 1
    fi
}
need_tool ar binutils binutils binutils
need_tool tar tar tar tar
need_tool zstd zstd zstd zstd
need_tool strip binutils binutils binutils
need_tool file file file file

# Step 1: Build the .deb (unless the caller asked us not to).
if [ "$SKIP_BUILD" = false ]; then
    echo "[1/5] Building Tauri .deb bundle..."
    (
        cd "$FRONTEND_DIR"
        npm run tauri build -- --bundles deb
    )
else
    echo "[1/5] Skipping Tauri build (--no-build or --deb)"
fi

# Step 2: Locate the .deb.
DEB_PATH="$USE_DEB"
if [ -z "$DEB_PATH" ]; then
    for candidate in "${TAURI_BUNDLE_DIRS[@]}"; do
        DEB_PATH="$(find "$candidate" -maxdepth 1 -name '*.deb' 2>/dev/null | sort -V | tail -n 1 || true)"
        [ -n "$DEB_PATH" ] && [ -f "$DEB_PATH" ] && break
    done
    if [ -z "$DEB_PATH" ] || [ ! -f "$DEB_PATH" ]; then
        echo "No .deb found in any of:" >&2
        for d in "${TAURI_BUNDLE_DIRS[@]}"; do echo "  - $d" >&2; done
        echo "Run without --no-build, or pass --deb <path>." >&2
        exit 1
    fi
elif [ ! -f "$DEB_PATH" ]; then
    echo "No such .deb: $DEB_PATH" >&2
    exit 1
fi
echo "[2/5] Source .deb: $DEB_PATH"

# Parse version + arch from Tauri's .deb naming:
#   kaulan_<version>_amd64.deb  →  x86_64
#   kaulan_<version>_arm64.deb  →  aarch64
DEB_BASENAME="$(basename "$DEB_PATH")"
PKG_VER="$(echo "$DEB_BASENAME" | sed -E 's/^[^_]+_([^_]+)_[^_]+\.deb$/\1/')"
DEB_ARCH="$(echo "$DEB_BASENAME" | sed -E 's/^[^_]+_[^_]+_([^_]+)\.deb$/\1/')"
case "$DEB_ARCH" in
    amd64) PKG_ARCH="x86_64" ;;
    arm64|armhf) PKG_ARCH="aarch64" ;;
    *) PKG_ARCH="$DEB_ARCH" ;;
esac
PKG_REL=1

if [ -z "$PKG_VER" ] || [ "$PKG_VER" = "$DEB_BASENAME" ]; then
    echo "Could not parse version from .deb filename: $DEB_BASENAME" >&2
    exit 1
fi
echo "    version: $PKG_VER-$PKG_REL"
echo "    arch:    $PKG_ARCH (from $DEB_ARCH)"

# Step 3: Extract .deb contents.
WORK_DIR="$(mktemp -d -t kaulan-arch-XXXXXX)"
cleanup() { rm -rf "$WORK_DIR"; }
trap cleanup EXIT

EXTRACT_DIR="$WORK_DIR/extracted"
PKG_ROOT="$WORK_DIR/pkg"
mkdir -p "$EXTRACT_DIR" "$PKG_ROOT"

echo "[3/5] Extracting .deb data.tar..."
(
    cd "$EXTRACT_DIR"
    ar -x "$DEB_PATH"
)
DATA_TAR="$(ls "$EXTRACT_DIR"/data.tar.* 2>/dev/null | head -n 1 || true)"
if [ -z "$DATA_TAR" ]; then
    echo "Could not find data.tar.* inside .deb" >&2
    exit 1
fi
tar -xf "$DATA_TAR" -C "$PKG_ROOT"

# Normalize ownership to root:root so the package is reproducible regardless
# of who built the .deb.
find "$PKG_ROOT" -exec chown 0:0 {} + 2>/dev/null || true

# Strip release binaries in place — Tauri doesn't strip by default on Linux,
# and an unstripped binary is ~3-4x larger than necessary. `strip` comes
# from binutils, which we already require for `ar`.
BIN_GLOB="$PKG_ROOT/usr/bin/*"
shopt -s nullglob
for bin in $BIN_GLOB; do
    if file -b "$bin" | grep -q 'ELF.*executable'; then
        strip --strip-all "$bin" 2>/dev/null || true
    fi
done
shopt -u nullglob

# Step 4: Generate .PKGINFO.
# Size is computed after extraction+strip; pacman wants it in bytes.
SIZE_BYTES="$(find "$PKG_ROOT" -type f -printf '%s\n' | awk '{s+=$1} END {print s+0}')"
BUILD_DATE="$(date +%s)"

echo "[4/5] Generating .PKGINFO..."
cat > "$PKG_ROOT/.PKGINFO" <<EOF
pkgname = $PKG_NAME
pkgver = $PKG_VER-$PKG_REL
pkgdesc = $PKG_DESC
url = $PKG_URL
builddate = $BUILD_DATE
packager = $PACKAGER
size = $SIZE_BYTES
arch = $PKG_ARCH
license = $PKG_LICENSE
depend = webkit2gtk-4.1
depend = gtk3
depend = librsvg
depend = hicolor-icon-theme
depend = ffmpeg
EOF

# No .INSTALL file: modern pacman auto-runs update-desktop-database and
# gtk-update-icon-cache via alpm-hooks when files land in the standard
# directories. Adding those commands by hand triggers namcap warnings.

# Step 5: Repackage as .pkg.tar.zst.
OUTPUT_FILE="$OUTPUT_DIR/${PKG_NAME}-${PKG_VER}-${PKG_REL}-${PKG_ARCH}.pkg.tar.zst"
mkdir -p "$OUTPUT_DIR"
rm -f "$OUTPUT_FILE"

echo "[5/5] Packaging as .pkg.tar.zst..."
# Build the tar entry list with no `./` prefix — pacman matches the exact
# name `.PKGINFO` in the archive, so `./.PKGINFO` would be invisible to it.
# `find -printf '%P\n'` strips the leading `./`. .PKGINFO sorts first; the
# rest is name-sorted for reproducible output.
ENTRIES_LIST="$WORK_DIR/entries.txt"
{
    printf '%s\n' '.PKGINFO'
    ( cd "$PKG_ROOT" && find . -mindepth 1 \
        ! -name '.PKGINFO' \
        -printf '%P\n' ) | LC_ALL=C sort
} > "$ENTRIES_LIST"

# --no-recursion so tar archives exactly the listed entries (find already
# enumerated the full tree); without this tar would re-descend into listed
# directories and duplicate them. Owner/group as text ("root/root"), not
# numeric 0/0 — namcap flags the numeric form.
tar --no-recursion \
    -C "$PKG_ROOT" \
    --mtime="@${BUILD_DATE}" \
    --owner=root --group=root \
    -cf "$WORK_DIR/pkg.tar" -T "$ENTRIES_LIST"
zstd --long -19 -f -q -o "$OUTPUT_FILE" "$WORK_DIR/pkg.tar"

echo
echo "=== Build Complete ==="
echo "Package: $OUTPUT_FILE"
ls -lh "$OUTPUT_FILE" | awk '{print "  size:", $5}'
echo
echo "Install with:"
echo "  sudo pacman -U $OUTPUT_FILE"
echo
echo "Verify before installing (optional, requires namcap):"
echo "  namcap $OUTPUT_FILE"
