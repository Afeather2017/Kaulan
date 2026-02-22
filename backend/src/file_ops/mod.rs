//! Pluggable file operations for desktop and Android platforms.
//!
//! This module provides traits for file reading and music file listing,
//! allowing platform-specific implementations (std::fs for desktop,
//! MediaStore API for Android).

use std::path::Path;
use std::sync::OnceLock;
use std::fs;
use tracing::debug;

/// Static storage for custom file reader implementation
static FILE_READER: OnceLock<Box<dyn FileReader>> = OnceLock::new();

/// Static storage for custom music file lister implementation
static MUSIC_FILE_LISTER: OnceLock<Box<dyn MusicFileLister>> = OnceLock::new();

/// Trait for reading file content
///
/// This trait allows different implementations for file reading:
/// - StdFileReader: Uses std::fs::read (default for desktop)
/// - MediaStoreFileReader: Uses Android MediaStore content URIs
pub trait FileReader: Send + Sync {
    /// Read the entire contents of a file into a bytes vector
    ///
    /// # Arguments
    /// * `path` - File path or content URI to read
    ///
    /// # Returns
    /// - `Ok(Vec<u8>)` - File contents as bytes
    /// - `Err(std::io::Error)` - I/O error occurred
    fn read_file(&self, path: &str) -> Result<Vec<u8>, std::io::Error>;
}

/// Trait for listing music files in a directory
///
/// This trait allows different implementations for scanning music files:
/// - StdMusicFileLister: Uses std::fs recursive directory scan (default for desktop)
/// - MediaStoreMusicFileLister: Uses Android MediaStore API
pub trait MusicFileLister: Send + Sync {
    /// List all music files in the given base path
    ///
    /// # Arguments
    /// * `base_path` - Base directory path to scan (may be ignored on Android)
    ///
    /// # Returns
    /// - `Ok(Vec<MusicFileInfo>)` - List of music files with metadata
    /// - `Err(std::io::Error)` - I/O error occurred
    fn list_music_files(&self, base_path: &str) -> Result<Vec<MusicFileInfo>, std::io::Error>;
}

/// Information about a music file
#[derive(Clone, Debug)]
pub struct MusicFileInfo {
    /// File path or content URI
    pub path: String,
    /// Display filename
    pub filename: String,
    /// Song title (from metadata if available)
    pub title: Option<String>,
    /// Artist name (from metadata if available)
    pub artist: Option<String>,
    /// Album name (from metadata if available)
    pub album: Option<String>,
    /// Duration in milliseconds (from metadata if available)
    pub duration_ms: Option<i64>,
}

/// Default FileReader using std::fs
///
/// This implementation uses the standard library's file reading
/// capabilities, suitable for desktop platforms.
pub struct StdFileReader;

impl FileReader for StdFileReader {
    fn read_file(&self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        fs::read(path)
    }
}

/// Supported audio file extensions
pub const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "ogg", "wav", "aac", "flac", "m4a", "opus"];

/// Default MusicFileLister using std::fs
///
/// This implementation performs a recursive directory scan
/// using the standard library, suitable for desktop platforms.
pub struct StdMusicFileLister;

impl MusicFileLister for StdMusicFileLister {
    fn list_music_files(&self, base_path: &str) -> Result<Vec<MusicFileInfo>, std::io::Error> {
        let mut audio_files = Vec::new();
        let dir_path = Path::new(base_path);

        debug!("Scanning directory with StdMusicFileLister: {}", base_path);

        scan_directory_recursive(dir_path, &mut audio_files);

        debug!("StdMusicFileLister scan complete. Found {} audio files", audio_files.len());
        Ok(audio_files)
    }
}

/// Recursively scan directory for audio files
///
/// This is a helper function for StdMusicFileLister that performs
/// the actual recursive directory traversal.
fn scan_directory_recursive(dir_path: &Path, audio_files: &mut Vec<MusicFileInfo>) {
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if let Some(extension) = path.extension() {
                        let ext_str = extension.to_string_lossy().to_lowercase();
                        if SUPPORTED_EXTENSIONS.contains(&ext_str.as_str()) {
                            let filename = path.file_name()
                                .unwrap()
                                .to_string_lossy()
                                .to_string();
                            let absolute_path = path.canonicalize()
                                .unwrap_or_else(|_| path.clone())
                                .to_string_lossy()
                                .to_string();

                            debug!("Found music file: {}", absolute_path);

                            audio_files.push(MusicFileInfo {
                                path: absolute_path,
                                filename,
                                title: None,
                                artist: None,
                                album: None,
                                duration_ms: None,
                            });
                        }
                    }
                } else if file_type.is_dir() {
                    scan_directory_recursive(&entry.path(), audio_files);
                }
            }
        }
    }
}

/// Set a custom file reader implementation
///
/// This function should be called before the server starts to inject
/// a platform-specific file reader (e.g., MediaStore on Android).
///
/// # Arguments
/// * `reader` - Boxed trait object implementing FileReader
///
/// # Returns
/// - `Ok(())` - Successfully set the reader
/// - `Err(Box<dyn FileReader>)` - A reader was already set, returns the new one
pub fn set_file_reader(reader: Box<dyn FileReader>) -> Result<(), Box<dyn FileReader>> {
    FILE_READER.set(reader)
}

/// Set a custom music file lister implementation
///
/// This function should be called before the server starts to inject
/// a platform-specific file lister (e.g., MediaStore on Android).
///
/// # Arguments
/// * `lister` - Boxed trait object implementing MusicFileLister
///
/// # Returns
/// - `Ok(())` - Successfully set the lister
/// - `Err(Box<dyn MusicFileLister>)` - A lister was already set, returns the new one
pub fn set_music_file_lister(lister: Box<dyn MusicFileLister>) -> Result<(), Box<dyn MusicFileLister>> {
    MUSIC_FILE_LISTER.set(lister)
}

/// Get the current file reader (custom or default)
///
/// Returns the custom reader if one was set, otherwise returns
/// the default StdFileReader.
pub fn get_file_reader() -> &'static dyn FileReader {
    FILE_READER.get().map(|b| b.as_ref()).unwrap_or(&StdFileReader)
}

/// Get the current music file lister (custom or default)
///
/// Returns the custom lister if one was set, otherwise returns
/// the default StdMusicFileLister.
pub fn get_music_file_lister() -> &'static dyn MusicFileLister {
    MUSIC_FILE_LISTER.get().map(|b| b.as_ref()).unwrap_or(&StdMusicFileLister)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_std_file_reader() {
        // Create a temporary file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"Hello, World!").unwrap();

        let reader = StdFileReader;
        let content = reader.read_file(file_path.to_str().unwrap()).unwrap();
        assert_eq!(content, b"Hello, World!");
    }

    #[test]
    fn test_supported_extensions() {
        assert!(SUPPORTED_EXTENSIONS.contains(&"mp3"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"flac"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&"txt"));
    }
}
