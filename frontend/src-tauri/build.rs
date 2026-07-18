use std::fs;
use std::path::{Path, PathBuf};

const MERIYAH_URL: &str = "https://cdn.jsdelivr.net/npm/meriyah@6.1.4/dist/meriyah.umd.min.js";
const ASTRING_URL: &str = "https://cdn.jsdelivr.net/npm/astring@1.9.0/dist/astring.min.js";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KAULAN_REFRESH_YTDL_ASSETS");

    if let Err(err) = prepare_ytdl_solver_assets() {
        panic!("failed to prepare ytdl solver assets: {err}");
    }

    // Copy FFmpeg runtime DLLs from vcpkg's installed bin next to the binary
    // so Tauri's bundler picks them up. Without this, the MSI/NSIS installer
    // ships without avformat-62.dll etc., and the installed app dies on launch
    // with "cannot find avformat-62.dll". Desktop Windows-only — skipped when
    // cross-compiling (e.g. to aarch64-linux-android from a Windows host).
    #[cfg(target_os = "windows")]
    {
        let target = std::env::var("TARGET").unwrap_or_default();
        // Match real Windows triples like `x86_64-pc-windows-msvc` and
        // `aarch64-pc-windows-msvc`. The previous `ends_with("-windows")`
        // check never matched because every Windows triple ends with the
        // toolchain suffix (`-msvc` or `-gnu`), so staging silently
        // no-op'd and installers shipped without FFmpeg.
        if target.contains("-windows-") {
            if let Err(err) = stage_ffmpeg_dlls() {
                panic!("failed to stage FFmpeg DLLs for bundling: {err}");
            }
        }
    }

    tauri_build::build()
}

/// Copy FFmpeg runtime DLLs from vcpkg's `installed/<triplet>/bin` into the
/// cargo target directory (next to `app.exe`). Tauri's bundler scans the
/// binary's directory for sibling DLLs and includes them in the MSI/NSIS
/// installer.
#[cfg(target_os = "windows")]
fn stage_ffmpeg_dlls() -> Result<(), String> {
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
    println!("cargo:rerun-if-env-changed=VCPKGRS_DYNAMIC");

    let target_dir = target_dir_from_out_dir()?;
    let dll_dir = vcpkg_ffmpeg_bin_dir()?;

    // FFmpeg library names start with one of these prefixes. Skip pkgconf and
    // any other utilities vcpkg happens to ship in the same bin dir.
    const FFMPEG_PREFIXES: &[&str] = &[
        "avcodec",
        "avdevice",
        "avfilter",
        "avformat",
        "avutil",
        "swresample",
        "swscale",
    ];

    let mut copied = 0usize;
    for entry in
        fs::read_dir(&dll_dir).map_err(|e| format!("read_dir {}: {e}", dll_dir.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let is_dll = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("dll"))
            .unwrap_or(false);
        if !is_dll {
            continue;
        }
        let name_lower = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_ascii_lowercase(),
            None => continue,
        };
        if !FFMPEG_PREFIXES.iter().any(|p| name_lower.starts_with(p)) {
            continue;
        }
        let dest = target_dir.join(entry.file_name());
        fs::copy(&path, &dest)
            .map_err(|e| format!("copy {} -> {}: {e}", path.display(), dest.display()))?;
        copied += 1;
    }

    if copied == 0 {
        return Err(format!(
            "no FFmpeg DLLs found under {}; run scripts/setup-windows-vcpkg.ps1 \
             or `vcpkg install ffmpeg` first",
            dll_dir.display()
        ));
    }
    println!(
        "cargo:warning=staged {copied} FFmpeg DLLs into {}",
        target_dir.display()
    );
    Ok(())
}

/// Resolve the cargo target directory (e.g. `target/release`) from `OUT_DIR`,
/// which cargo sets to `target/{profile}/build/<crate>-<hash>/out`.
#[cfg(target_os = "windows")]
fn target_dir_from_out_dir() -> Result<PathBuf, String> {
    let out_dir = std::env::var("OUT_DIR").map_err(|e| format!("OUT_DIR not set: {e}"))?;
    let out_path = PathBuf::from(out_dir);
    out_path
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "could not resolve target dir from OUT_DIR={}",
                out_path.display()
            )
        })
}

/// Resolve vcpkg's FFmpeg bin directory for the current target architecture.
#[cfg(target_os = "windows")]
fn vcpkg_ffmpeg_bin_dir() -> Result<PathBuf, String> {
    let vcpkg_root = std::env::var("VCPKG_ROOT")
        .map_err(|_| "VCPKG_ROOT not set; run scripts/setup-windows-vcpkg.ps1 first".to_string())?;
    let target = std::env::var("TARGET").map_err(|e| format!("TARGET not set: {e}"))?;
    let triplet = if target.starts_with("x86_64") {
        "x64-windows"
    } else if target.starts_with("aarch64") {
        "arm64-windows"
    } else {
        return Err(format!("unsupported Windows target: {target}"));
    };
    Ok(PathBuf::from(vcpkg_root)
        .join("installed")
        .join(triplet)
        .join("bin"))
}

fn prepare_ytdl_solver_assets() -> Result<(), String> {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").map_err(|e| e.to_string())?);
    let generated_dir = manifest_dir.join("generated").join("ytdl");
    let android_assets_dir = manifest_dir
        .join("gen")
        .join("android")
        .join("app")
        .join("src")
        .join("main")
        .join("assets")
        .join("ytdl");
    let refresh = std::env::var_os("KAULAN_REFRESH_YTDL_ASSETS").is_some();

    prepare_asset(
        MERIYAH_URL,
        "meriyah.umd.min.js",
        &generated_dir,
        &android_assets_dir,
        refresh,
    )?;
    prepare_asset(
        ASTRING_URL,
        "astring.min.js",
        &generated_dir,
        &android_assets_dir,
        refresh,
    )?;

    Ok(())
}

fn prepare_asset(
    url: &str,
    file_name: &str,
    generated_dir: &Path,
    android_assets_dir: &Path,
    refresh: bool,
) -> Result<(), String> {
    let generated_path = generated_dir.join(file_name);
    let android_path = android_assets_dir.join(file_name);

    if !refresh && generated_path.exists() && android_path.exists() {
        return Ok(());
    }

    fs::create_dir_all(generated_dir).map_err(|e| {
        format!(
            "failed to create generated asset dir {}: {e}",
            generated_dir.display()
        )
    })?;
    fs::create_dir_all(android_assets_dir).map_err(|e| {
        format!(
            "failed to create android asset dir {}: {e}",
            android_assets_dir.display()
        )
    })?;

    let response = reqwest::blocking::get(url)
        .map_err(|e| format!("failed to download {url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("failed to download {url}: {e}"))?;
    let content = response
        .text()
        .map_err(|e| format!("failed to read {url} body: {e}"))?;

    fs::write(&generated_path, &content).map_err(|e| {
        format!(
            "failed to write generated asset {}: {e}",
            generated_path.display()
        )
    })?;
    fs::write(&android_path, &content).map_err(|e| {
        format!(
            "failed to write android asset {}: {e}",
            android_path.display()
        )
    })?;

    println!("cargo:warning=prepared ytdl solver asset {}", file_name);
    Ok(())
}
