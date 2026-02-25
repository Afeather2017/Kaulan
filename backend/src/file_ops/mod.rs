//! Pluggable file operations for desktop and Android platforms.
//!
//! This module provides traits for file reading and music file listing,
//! allowing platform-specific implementations (std::fs for desktop,
//! MediaStore API for Android).

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::fs;
use std::path::Path;
use std::pin::Pin;
use std::sync::OnceLock;
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
#[async_trait]
pub trait FileReader: Send + Sync {
    /// Read the entire contents of a file into a bytes vector
    ///
    /// # Arguments
    /// * `path` - File path or content URI to read
    ///
    /// # Returns
    /// - `Ok(Vec<u8>)` - File contents as bytes
    /// - `Err(std::io::Error)` - I/O error occurred
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, std::io::Error>;

    /// Read a file as a stream of byte chunks
    ///
    /// # Arguments
    /// * `path` - File path or content URI to read
    /// * `chunk_size` - Size of each chunk in bytes
    ///
    /// # Returns
    /// - `Ok(Stream)` - Stream of file chunks
    /// - `Err(std::io::Error)` - I/O error occurred
    async fn read_stream(
        &self,
        path: &str,
        chunk_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>, std::io::Error>;
}

/// Trait for listing music files in a directory
///
/// This trait allows different implementations for scanning music files:
/// - StdMusicFileLister: Uses std::fs recursive directory scan (default for desktop)
/// - MediaStoreMusicFileLister: Uses Android MediaStore API
#[async_trait]
pub trait MusicFileLister: Send + Sync {
    /// List all music files in the given base path
    ///
    /// # Arguments
    /// * `base_path` - Base directory path to scan (may be ignored on Android)
    ///
    /// # Returns
    /// - `Ok(Vec<MusicFileInfo>)` - List of music files with metadata
    /// - `Err(std::io::Error)` - I/O error occurred
    async fn list_music_files(&self, base_path: &str) -> Result<Vec<MusicFileInfo>, std::io::Error>;
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

#[async_trait]
impl FileReader for StdFileReader {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        debug!("StdFileReader::read_file called with path: {}", path);
        // Use tokio::task::spawn_blocking for std::fs operations
        // to avoid blocking the async runtime
        let path = path.to_string();
        let path_clone = path.clone();
        tokio::task::spawn_blocking(move || {
            debug!("StdFileReader: Attempting to read file: {}", path_clone);
            let result = std::fs::read(&path_clone);
            match &result {
                Ok(bytes) => debug!("StdFileReader: Successfully read {} bytes from {}", bytes.len(), path_clone),
                Err(e) => debug!("StdFileReader: Failed to read file {}: {}", path_clone, e),
            }
            result
        })
            .await
            .map_err(|e| {
                debug!("StdFileReader: Task join error for path {}: {}", path, e);
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?
    }

    async fn read_stream(
        &self,
        path: &str,
        chunk_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>, std::io::Error> {
        debug!("StdFileReader::read_stream called with path: {}", path);
        let file = tokio::fs::File::open(path).await?;
        let stream = tokio_util::io::ReaderStream::with_capacity(file, chunk_size);
        Ok(Box::pin(stream))
    }
}

/// Supported audio file extensions
pub const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "ogg", "wav", "aac", "flac", "m4a", "opus"];

/// Default MusicFileLister using std::fs
///
/// This implementation performs a recursive directory scan
/// using the standard library, suitable for desktop platforms.
pub struct StdMusicFileLister;

#[async_trait]
impl MusicFileLister for StdMusicFileLister {
    async fn list_music_files(&self, base_path: &str) -> Result<Vec<MusicFileInfo>, std::io::Error> {
        // Use spawn_blocking for synchronous directory scanning
        let base_path = base_path.to_string();
        tokio::task::spawn_blocking(move || {
            let mut audio_files = Vec::new();
            let dir_path = Path::new(&base_path);

            debug!("Scanning directory with StdMusicFileLister: {}", base_path);
            scan_directory_recursive_sync(dir_path, &mut audio_files);
            debug!("StdMusicFileLister scan complete. Found {} audio files", audio_files.len());
            Ok(audio_files)
        })
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
    }
}

/// Recursively scan directory for audio files (synchronous helper)
///
/// This is a helper function for StdMusicFileLister that performs
/// the actual recursive directory traversal synchronously.
fn scan_directory_recursive_sync(dir_path: &Path, audio_files: &mut Vec<MusicFileInfo>) {
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
                    scan_directory_recursive_sync(&entry.path(), audio_files);
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
    debug!("set_file_reader: Setting custom file reader");
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
    let reader = FILE_READER.get().map(|b| b.as_ref()).unwrap_or(&StdFileReader);
    if FILE_READER.get().is_some() {
        debug!("get_file_reader: Returning custom file reader");
    } else {
        debug!("get_file_reader: Returning default StdFileReader");
    }
    reader
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

    #[tokio::test]
    async fn test_std_file_reader() {
        // Create a temporary file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"Hello, World!").unwrap();

        let reader = StdFileReader;
        let content = reader.read_file(file_path.to_str().unwrap()).await.unwrap();
        assert_eq!(content, b"Hello, World!");
    }

    #[tokio::test]
    async fn test_std_file_reader_stream() {
        use futures::StreamExt;

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("stream.bin");
        let data = vec![0_u8; 1024 * 1024 + 17];
        fs::write(&file_path, &data).unwrap();

        let reader = StdFileReader;
        let mut stream = reader.read_stream(file_path.to_str().unwrap(), 1024 * 1024).await.unwrap();
        let mut collected = Vec::new();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.unwrap();
            collected.extend_from_slice(&bytes);
        }

        assert_eq!(collected, data);
    }

    #[test]
    fn test_supported_extensions() {
        assert!(SUPPORTED_EXTENSIONS.contains(&"mp3"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"flac"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&"txt"));
    }
}
