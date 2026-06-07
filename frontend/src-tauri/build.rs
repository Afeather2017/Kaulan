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

    tauri_build::build()
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
