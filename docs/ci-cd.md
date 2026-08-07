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

- Windows desktop bundles (`x86_64` and `arm64`)
- Linux desktop bundles (`x86_64` and `arm64`)
- Standalone server AppImages for Linux `x86_64` and `aarch64`
- Standalone server portable archives for Windows `x86_64` and `arm64`
- Android APKs, one per ABI: `arm64-v8a` (aarch64), `armeabi-v7a` (armv7), and `x86_64`, published as `kaulan-<abi>.apk`. Tauri always assembles the universal Gradle flavor, so each per-ABI leg builds `app-universal-release.apk` containing only that ABI's libs and renames it to `kaulan-<abi>.apk` before upload; the App Bundle (AAB) is no longer produced by CI.

Standalone server artifacts are built directly from the `backend` crate. They
do not contain the Vue frontend or Tauri desktop shell. Linux AppImages bundle
the backend's FFmpeg runtime libraries; Windows archives include the FFmpeg
DLLs required by the backend.

The Linux packaging step sets both `OUTPUT` and `LDAI_OUTPUT` when invoking
linuxdeploy, since the continuous linuxdeploy AppImage has used both variable
names for the generated image destination.

The Android release job runs as a per-ABI matrix. Each leg builds and caches only its own FFmpeg subtree (see [`ffmpeg-audio-pipeline.md`](ffmpeg-audio-pipeline.md)) before running the Android packaging script. Manual recovery publishes can be started with `workflow_dispatch` by passing the existing GitHub release tag, such as `v0.1.4`.

The root [`build-android.sh`](../build-android.sh) script performs the same staged-bundle preflight locally. Targeted builds such as `./build-android.sh --target aarch64` only require the matching target subtree plus `binding.rs`; the default multi-ABI build requires all four Android target subtrees. Pass `--no-aab` to emit APKs only (the per-ABI CI legs use this).

## FFmpeg build dependencies

The workflow installs FFmpeg 8.1.1 development dependencies before Rust builds:

- Linux runs on `ubuntu-24.04` (x86_64) and `ubuntu-24.04-arm` (arm64) runners, building FFmpeg 8.1.1 natively from the official source tarball on each. The installed output is cached per architecture (`runner.arch`) and exposed through `PKG_CONFIG_PATH`.
- Windows runs on `windows-latest` (x86_64) and `windows-11-arm` (arm64) runners. It bootstraps `vcpkg` through [`scripts/setup-windows-vcpkg.ps1`](../scripts/setup-windows-vcpkg.ps1), which selects the `x64-windows` or `arm64-windows` triplet from the host architecture, installs `ffmpeg`, caches the local `.cache/vcpkg` tree per architecture, and exposes `VCPKG_ROOT`, `VCPKGRS_DYNAMIC=1`, plus the triplet `bin` directory to later build steps.
- Android uses the staged FFmpeg bundle under `build/android-ffmpeg/android/<target>`. Each per-ABI release leg generates only its own target subtree with `scripts/build-android-ffmpeg.sh --target <short>`, then `build-android.sh` verifies `binding.rs`, that target's `lib` directory, and its `prefix/include` directory before packaging.

These match the backend dependency setup in `backend/Cargo.toml`:

- non-Windows targets use `rusty_ffmpeg` with `link_system_ffmpeg` and the `ffmpeg8_1` API feature
- Windows targets enable `link_vcpkg_ffmpeg` and consume the vendored `vendor/rusty_ffmpeg/src/binding.rs` through `.cargo/config.toml`, so local and CI builds do not need a separate LLVM or `libclang` install

## Publish flow

1. Push a version tag.
2. Create a GitHub Release for that tag.
3. Publish the release.
4. GitHub Actions runs tests.
5. If tests pass, x86_64 + arm64 Windows and Linux desktop packages are built natively on the matching GitHub-hosted runners. The server binary is built immediately after the Tauri/UI bundle in each desktop matrix leg, reusing the same Rust `target` artifacts and platform-specific cache, then packaged and attached alongside the desktop assets.
