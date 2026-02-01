use std::process::{Command, Stdio};
use std::path::Path;
use std::io::{BufRead, BufReader};
use tracing::{debug, info, warn, error};

/// Audio file extensions supported for LUFS analysis
const SUPPORTED_EXTENSIONS: [&str; 5] = ["wav", "mp3", "ogg", "aac", "flac"];

/// Represents LUFS analysis result
#[derive(Debug, Clone)]
pub struct LufsResult {
    pub filename: String,
    pub path: String,
    pub lufs: Option<f64>,
}

/// Runs FFmpeg command to get LUFS value and parses the result
pub fn get_lufs(file_path: &str) -> Option<f64> {
    debug!("Calculating LUFS for: {}", file_path);
    let cmd = Command::new("ffmpeg")
        .args([
            "-i", file_path,
            "-filter_complex", "ebur128=peak=true",
            "-f", "null",
            "-",
        ])
        .stderr(Stdio::piped())
        .spawn();

    match cmd {
        Ok(mut child) => {
            let stderr = child.stderr.take().expect("Failed to capture stderr");
            let reader = BufReader::new(stderr);

            for line in reader.lines() {
                if let Ok(line) = line {
                    let line = line.trim();

                    // Skip lines starting with [
                    if line.starts_with('[') {
                        continue;
                    }

                    // Look for lines starting with "I:"
                    if line.starts_with("I:") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 && parts[0] == "I:" {
                            if let Ok(lufs_value) = parts[1].parse::<f64>() {
                                debug!("LUFS calculated for {}: {}", file_path, lufs_value);
                                return Some(lufs_value);
                            }
                        }
                    }
                }
            }

            // Wait for the process to finish
            let _ = child.wait();
            warn!("Failed to extract LUFS value from ffmpeg output for: {}", file_path);
            None
        }
        Err(e) => {
            error!("Failed to execute ffmpeg for {}: {}", file_path, e);
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
