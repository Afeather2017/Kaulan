# Arch Linux Packaging (`pacman` / `.pkg.tar.zst`)

How Kaulan produces an Arch Linux pacman package, and how to install or
build one.

Tauri does not natively emit pacman packages, so Kaulan converts the
Tauri-generated `.deb` into `.pkg.tar.zst` format. The `.deb` already
contains the binary, `.desktop` entry, icons, and audio MIME associations,
so the conversion is a pure repackaging step — no rebuild.

Related files:

- [`build-arch.sh`](../build-arch.sh) — local/CI converter script.
- [`build/arch/PKGBUILD`](../build/arch/PKGBUILD) — AUR-style PKGBUILD
  (end users run `makepkg -si`).
- [`.github/workflows/publish.yml`](../.github/workflows/publish.yml) —
  CI job that runs `build-arch.sh --no-build` on the Linux release legs.

## Output

```
target/arch/kaulan-<version>-<release>-<arch>.pkg.tar.zst
```

For example: `kaulan-0.1.4-1-x86_64.pkg.tar.zst`.

The package installs:

- `/usr/bin/kaulan` — the Tauri binary
- `/usr/share/applications/afeather.kaulan.desktop` — desktop entry with
  `MimeType=` populated from `bundle.fileAssociations` in `tauri.conf.json`
- `/usr/share/icons/hicolor/*/apps/kaulan.png` — icons at all the standard
  sizes Tauri ships

## Build

### Local build (any Linux distro)

`build-arch.sh` runs on any Linux distro — it only needs `ar`, `tar`, and
`zstd`. No `makepkg`, no Arch base system, no AUR helper.

```bash
# Build the .deb then convert:
./build-arch.sh

# Reuse an existing .deb (skip the Tauri build step):
./build-arch.sh --no-build

# Convert a specific .deb:
./build-arch.sh --deb /path/to/kaulan_0.1.4_amd64.deb
```

The script is `set -euo pipefail` and prints a `[1/5]`-style progress log.
Output lands in `target/arch/`.

### Install via `makepkg` (Arch Linux only)

For end users on Arch who prefer the standard `makepkg` flow:

```bash
cd build/arch
updpkgsums           # fill in real sha256sums after bumping _pkgver
makepkg -si
```

The PKGBUILD downloads the published `.deb` from the GitHub release and
repackages it. It does not build from source.

### Install a CI-produced artifact

Every GitHub release since this feature landed includes:

- `kaulan_<version>_amd64.deb` (from Tauri, for Debian/Ubuntu)
- `kaulan-<version>-1-x86_64.pkg.tar.zst` (from `build-arch.sh`, for Arch)

The same release also includes standalone server packages with the
`kaulan-server` prefix. Those packages contain only the Rust backend and are
separate from the desktop UI packages described above.

Download the `.pkg.tar.zst` and:

```bash
sudo pacman -U kaulan-<version>-1-x86_64.pkg.tar.zst
```

## Set as default audio handler

Installing the package registers Kaulan as a *handler* for audio MIME
types (via the `.desktop` file's `MimeType=` line). It does not make
Kaulan the *default*. To set it as default:

```bash
xdg-mime default afeather.kaulan.desktop \
    audio/mpeg audio/flac audio/wav audio/ogg audio/opus audio/mp4 audio/aac

xdg-mime query default audio/mpeg
# → afeather.kaulan.desktop
```

See [`docs/default-music-app.md`](default-music-app.md) for the full
launch-handoff flow (cold start, warm start, SSE push).

## Runtime dependencies

The package declares these `depend=` lines in `.PKGINFO`:

| Arch package           | Why                                  |
| ---------------------- | ------------------------------------ |
| `webkit2gtk-4.1`       | Tauri's webview                      |
| `gtk3`                 | GTK window chrome                    |
| `libayatana-appindicator` | System tray / appindicator support |
| `librsvg`              | SVG icon rendering                   |
| `hicolor-icon-theme`   | Icon cache root                      |

FFmpeg is **not** declared — Kaulan links FFmpeg via the staged bundle
under `build/android-ffmpeg` on Android and the system-built FFmpeg on
desktop. Desktop Linux builds statically pull FFmpeg through
`rusty_ffmpeg` using headers from `scripts/ci-install-ffmpeg-linux.sh`,
so the binary has FFmpeg built in.

## Post-install hooks

`pacman` on Arch Linux ships alpm-hooks that automatically refresh the
desktop database and GTK icon cache whenever a package installs files
into `/usr/share/applications` or `/usr/share/icons/hicolor`. Kaulan
relies on those hooks — there is no `.INSTALL` file in the package.

If you need to refresh the caches manually (for example after copying
the `.desktop` file aside for testing):

```bash
sudo update-desktop-database /usr/share/applications
sudo gtk-update-icon-cache -f /usr/share/icons/hicolor
```

## CI integration

The `release-desktop` job in `.github/workflows/publish.yml` runs these
steps on every Linux leg (both `ubuntu-24.04` for `x86_64` and
`ubuntu-24.04-arm` for `aarch64`) after `tauri-apps/tauri-action` has
produced the `.deb`:

1. `Install Arch packaging tools` — `apt-get install zstd binutils`
2. `Build Arch Linux pacman package` — runs `./build-arch.sh --no-build`
3. `Upload Arch Linux package` — attaches `target/arch/*.pkg.tar.zst` to
   the GitHub release via `softprops/action-gh-release@v2`

The package file naming includes the pacman arch
(`kaulan-<version>-1-x86_64.pkg.tar.zst` vs.
`kaulan-<version>-1-aarch64.pkg.tar.zst`), so both legs can upload to
the same release without colliding.

## Reproducibility

`build-arch.sh` uses deterministic tar flags (`--sort=name`,
`--owner=0`, `--group=0`, `--mtime=@<builddate>`) so that two
conversions of the same `.deb` produce byte-identical `.pkg.tar.zst`
files. The `builddate` in `.PKGINFO` is `date +%s` at build time and
will naturally vary; if you need a fully reproducible build, set it to
a fixed timestamp.
