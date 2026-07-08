# CI/CD

GitHub Actions validates pushes and publishes desktop release packages for this repository when a GitHub Release is published.

## Trigger

- Workflow: `.github/workflows/publish.yml`
- CI events:
  - `push` to `master`
  - `push` to `migrate-ffmpeg`
  - `pull_request` targeting `master`
  - manual `workflow_dispatch`
- Publish event: `release.published`

## Release gate

The publish workflow runs these checks before any release asset is built:

- `cargo test -p kaulan`
- `npm run test -- --run`
- `npm run build`

If any of them fail, no release packages are uploaded.

## Release outputs

The workflow uploads these packages to the GitHub Release:

- Windows desktop bundles
- Linux desktop bundles
- Android APK/AAB bundles

The Android release job builds and caches the staged FFmpeg bundle described in [`ffmpeg-audio-pipeline.md`](ffmpeg-audio-pipeline.md) before running the Android packaging script. Manual recovery publishes can be started with `workflow_dispatch` by passing the existing GitHub release tag, such as `v0.1.4`.

The root [`build-android.sh`](../build-android.sh) script performs the same staged-bundle preflight locally. Targeted builds such as `./build-android.sh --target aarch64` only require the matching target subtree plus `binding.rs`; the default multi-ABI build requires all four Android target subtrees.

## FFmpeg build dependencies

The workflow installs FFmpeg 8.1.1 development dependencies before Rust builds:

- Linux runs on `ubuntu-24.04`, builds FFmpeg 8.1.1 from the official source tarball, caches the installed output, and exposes it through `PKG_CONFIG_PATH`.
- Windows bootstraps `vcpkg` through [`scripts/setup-windows-vcpkg.ps1`](../scripts/setup-windows-vcpkg.ps1), installs `ffmpeg` for the default `x64-windows` triplet, caches the local `.cache/vcpkg` tree, and exposes `VCPKG_ROOT`, `VCPKGRS_DYNAMIC=1`, plus the triplet `bin` directory to later build steps.
- Android uses the staged FFmpeg bundle under `build/android-ffmpeg/android/<target>`. The release job generates it with `scripts/build-android-ffmpeg.sh`, then `build-android.sh` verifies `binding.rs`, each target `lib` directory, and each target `prefix/include` directory before packaging.

These match the backend dependency setup in `backend/Cargo.toml`:

- non-Windows targets use `rusty_ffmpeg` with `link_system_ffmpeg` and the `ffmpeg8_1` API feature
- Windows targets enable `link_vcpkg_ffmpeg` and consume the vendored `vendor/rusty_ffmpeg/src/binding.rs` through `.cargo/config.toml`, so local and CI builds do not need a separate LLVM or `libclang` install

## Publish flow

1. Push a version tag.
2. Create a GitHub Release for that tag.
3. Publish the release.
4. GitHub Actions runs tests.
5. If tests pass, Windows and Linux desktop release packages are built and attached.
