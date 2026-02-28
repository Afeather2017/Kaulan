use std::path::Path;
use tracing::{info, warn, error};
use lufsgen::LufsCalculator;

/// Audio file extensions supported for LUFS analysis
const SUPPORTED_EXTENSIONS: [&str; 5] = ["wav", "mp3", "ogg", "aac", "flac"];

/// Represents LUFS analysis result
#[derive(Debug, Clone)]
pub struct LufsResult {
    pub filename: String,
    pub path: String,
    pub lufs: Option<f64>,
}

/// Calculates LUFS value using the lufsgen crate
pub fn get_lufs(file_path: &str) -> Option<f64> {
    let calc = LufsCalculator::default();
    match calc.calculate_from_file(Path::new(file_path)) {
        Ok(Some(lufs)) => {
            info!("[LUFS] SUCCESS: {} - LUFS: {}", file_path, lufs);
            Some(lufs)
        }
        Ok(None) => {
            warn!("[LUFS] FAILED: Unsupported format for: {}", file_path);
            None
        }
        Err(e) => {
            error!("[LUFS] ERROR: Failed to calculate LUFS for {}: {}", file_path, e);
            None
        }
    }
}

/// Checks if a file has supported audio extension
pub fn is_audio_file(filename: &str) -> bool {
    if let Some(extension) = Path::new(filename).extension() {
        let ext_str = extension.to_string_lossy().to_lowercase();
        SUPPORTED_EXTENSIONS.contains(&ext_str.as_str())
    } else {
        false
    }
}

/// Scans directory recursively for audio files and generates LUFS data
pub fn scan_and_generate_lufs(root_dir: &str) -> Vec<LufsResult> {
    info!("Scanning directory for LUFS generation: {}", root_dir);
    let mut results = Vec::new();
    let mut file_count = 0;

    if let Ok(entries) = std::fs::read_dir(root_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let filename = entry.file_name().to_string_lossy().to_string();

                    if is_audio_file(&filename) {
                        file_count += 1;
                        let file_path = entry.path();
                        let path_str = file_path.to_string_lossy().to_string();

                        let lufs = get_lufs(&path_str);

                        results.push(LufsResult {
                            filename,
                            path: path_str,
                            lufs,
                        });
                    }
                } else if file_type.is_dir() {
                    // Recursively scan subdirectories
                    let subdir_path = entry.path().to_string_lossy().to_string();
                    let mut sub_results = scan_and_generate_lufs(&subdir_path);
                    results.append(&mut sub_results);
                }
            }
        }
    }

    info!("LUFS scan complete: processed {} audio files", file_count);
    results
}

/// Generates LUFS data for specific files
pub fn generate_lufs_for_files(file_paths: Vec<String>) -> Vec<LufsResult> {
    info!("Generating LUFS for {} files", file_paths.len());
    let results: Vec<_> = file_paths
        .into_iter()
        .map(|file_path| {
            let filename = Path::new(&file_path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let lufs = get_lufs(&file_path);

            LufsResult {
                filename,
                path: file_path,
                lufs,
            }
        })
        .collect();
    info!("LUFS generation complete for {} files", results.len());
    results
}

/// Writes LUFS results to a file in the format expected by the backend
pub fn write_lufs_data(results: &[LufsResult], output_path: &str) -> std::io::Result<()> {
    info!("Writing LUFS data to: {}", output_path);
    let mut content = String::new();
    let mut written_count = 0;

    for result in results {
        if let Some(lufs) = result.lufs {
            content.push_str(&result.filename);
            content.push('\n');
            content.push_str(&format!("path: {}\n", result.path));
            content.push_str(&format!("lufs: {}\n", lufs));
            content.push('\n');
            written_count += 1;
        }
    }

    std::fs::write(output_path, content)?;
    info!("Wrote {} LUFS entries to {}", written_count, output_path);
    Ok(())
}
