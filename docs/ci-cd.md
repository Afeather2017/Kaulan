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
- Android APK/AAB bundles when the staged Android FFmpeg bundle is present

The Android release job is present on the `migrate-ffmpeg` branch, but it checks for the staged FFmpeg bundle described in [`ffmpeg-audio-pipeline.md`](ffmpeg-audio-pipeline.md) before setting up the Android toolchain. If the bundle is missing, the job exits successfully with a step-summary notice instead of failing desktop publishing.

## FFmpeg build dependencies

The workflow installs FFmpeg 8.1.1 development dependencies before Rust builds:

- Linux runs on `ubuntu-24.04`, builds FFmpeg 8.1.1 from the official source tarball, caches the installed output, and exposes it through `PKG_CONFIG_PATH`.
- Windows downloads the Gyan FFmpeg 8.1.1 shared development package and exposes it through `FFMPEG_INCLUDE_DIR`, `FFMPEG_LIBS_DIR`, `FFMPEG_DLL_PATH`, and `FFMPEG_LINK_MODE=dynamic`. The workflow also copies the package `.lib` import libraries into the DLL directory because `rusty_ffmpeg` links from `FFMPEG_DLL_PATH` in dynamic mode.
- Android uses the staged FFmpeg bundle under `build/android-ffmpeg/android/<target>`. The release job requires `binding.rs`, each target `lib` directory, and each target `prefix/include` directory before running `build-android.sh`.

These match the backend dependency setup in `backend/Cargo.toml`:

- non-Windows targets use `rusty_ffmpeg` with `link_system_ffmpeg` and the `ffmpeg8_1` API feature
- Windows targets enable the `link_vcpkg_ffmpeg` fallback and the `ffmpeg8_1` API feature, while CI points `rusty_ffmpeg` directly at the downloaded FFmpeg 8.1.1 include and library directories

## Publish flow

1. Push a version tag.
2. Create a GitHub Release for that tag.
3. Publish the release.
4. GitHub Actions runs tests.
5. If tests pass, Windows and Linux desktop release packages are built and attached.
