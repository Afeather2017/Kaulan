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

    // Stage Windows DLLs before Tauri resolves the bundle resources.
    #[cfg(target_os = "windows")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        if let Err(err) = stage_ffmpeg_dlls() {
            panic!("failed to stage FFmpeg DLLs for bundling: {err}");
        }
    }

    tauri_build::build()
}

/// Prepare the FFmpeg resources listed in `tauri.windows.conf.json`.
#[cfg(target_os = "windows")]
fn stage_ffmpeg_dlls() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
    println!("cargo:rerun-if-env-changed=VCPKGRS_DYNAMIC");

    let dll_dir = vcpkg_ffmpeg_bin_dir()?;
    println!("cargo:rerun-if-changed={}", dll_dir.display());
    let resource_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?)
        .join("generated")
        .join("windows-ffmpeg");

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

    let mut dlls = std::collections::HashSet::new();
    for entry in fs::read_dir(&dll_dir)? {
        let name = entry?.file_name();
        let name_lower = name.to_string_lossy().to_ascii_lowercase();
        if name_lower.ends_with(".dll") && FFMPEG_PREFIXES.iter().any(|p| name_lower.starts_with(p))
        {
            dlls.insert(name);
        }
    }

    if dlls.is_empty() {
        return Err(format!(
            "no FFmpeg DLLs found under {}; run scripts/setup-windows-vcpkg.ps1 \
             or `vcpkg install ffmpeg` first",
            dll_dir.display()
        )
        .into());
    }

    fs::create_dir_all(&resource_dir)?;
    for entry in fs::read_dir(&resource_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            && !dlls.contains(&entry.file_name())
        {
            fs::remove_file(path)?;
        }
    }
    for name in dlls {
        let source = dll_dir.join(&name);
        println!("cargo:rerun-if-changed={}", source.display());
        fs::copy(source, resource_dir.join(name))?;
    }
    Ok(())
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
