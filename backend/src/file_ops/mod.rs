//! Pluggable file operations for desktop and Android platforms.
//!
//! This module provides traits for file reading and music file listing,
//! allowing platform-specific implementations (std::fs for desktop,
//! MediaStore API for Android).

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::fs;
use std::io::{self, Read, Seek};
use std::path::Path;
use std::pin::Pin;
use std::sync::OnceLock;
use tracing::debug;

/// Static storage for custom file reader implementation
static FILE_READER: OnceLock<Box<dyn FileReader>> = OnceLock::new();

/// Static storage for custom music file lister implementation
static MUSIC_FILE_LISTER: OnceLock<Box<dyn MusicFileLister>> = OnceLock::new();

/// Trait object bound for seekable readers used by LUFS calculation.
pub trait ReadSeekSendSync: Read + Seek + Send + Sync {}

impl<T> ReadSeekSendSync for T where T: Read + Seek + Send + Sync {}

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

    /// Read a file as a stream starting from a specific byte position (for Range requests)
    ///
    /// # Arguments
    /// * `path` - File path or content URI to read
    /// * `chunk_size` - Size of each chunk in bytes
    /// * `start_pos` - Starting byte position
    ///
    /// # Returns
    /// - `Ok(Stream)` - Stream of file chunks from start_pos to end
    /// - `Err(std::io::Error)` - I/O error occurred
    async fn read_stream_from(
        &self,
        path: &str,
        chunk_size: usize,
        start_pos: u64,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>, std::io::Error>;

    /// Get the file size for a given path
    ///
    /// # Arguments
    /// * `path` - File path or content URI
    ///
    /// # Returns
    /// - `Ok(u64)` - File size in bytes
    /// - `Err(std::io::Error)` - I/O error occurred
    async fn get_file_size(&self, path: &str) -> Result<u64, std::io::Error>;

    /// Open a file as a seekable reader (used by reader-based LUFS calculation).
    ///
    /// # Arguments
    /// * `path` - File path or content URI
    ///
    /// # Returns
    /// - `Ok(Box<dyn ReadSeekSendSync>)` - Seekable reader
    /// - `Err(std::io::Error)` - I/O error occurred
    async fn open_seekable_reader(
        &self,
        path: &str,
    ) -> Result<Box<dyn ReadSeekSendSync>, std::io::Error>;
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
    /// * `media_types` - Enabled media types (e.g. `["audio"]` or `["audio", "video"]`)
    ///
    /// # Returns
    /// - `Ok(Vec<MusicFileInfo>)` - List of music files with metadata
    /// - `Err(std::io::Error)` - I/O error occurred
    async fn list_music_files(
        &self,
        base_path: &str,
        media_types: &[String],
    ) -> Result<Vec<MusicFileInfo>, std::io::Error>;
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
    /// Parent directory name (for folder-based playlists)
    pub parent_dir: Option<String>,
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
                Ok(bytes) => debug!(
                    "StdFileReader: Successfully read {} bytes from {}",
                    bytes.len(),
                    path_clone
                ),
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
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>, std::io::Error>
    {
        debug!("StdFileReader::read_stream called with path: {}", path);
        let file = tokio::fs::File::open(path).await?;
        let stream = tokio_util::io::ReaderStream::with_capacity(file, chunk_size);
        Ok(Box::pin(stream))
    }

    async fn read_stream_from(
        &self,
        path: &str,
        chunk_size: usize,
        start_pos: u64,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>, std::io::Error>
    {
        debug!(
            "StdFileReader::read_stream_from called with path: {}, start_pos: {}",
            path, start_pos
        );
        let mut file = tokio::fs::File::open(path).await?;
        // Seek to the starting position
        use tokio::io::{AsyncSeekExt, SeekFrom};
        file.seek(SeekFrom::Start(start_pos)).await?;
        let stream = tokio_util::io::ReaderStream::with_capacity(file, chunk_size);
        Ok(Box::pin(stream))
    }

    async fn get_file_size(&self, path: &str) -> Result<u64, std::io::Error> {
        debug!("StdFileReader::get_file_size called with path: {}", path);
        let metadata = tokio::fs::metadata(path).await?;
        Ok(metadata.len())
    }

    async fn open_seekable_reader(
        &self,
        path: &str,
    ) -> Result<Box<dyn ReadSeekSendSync>, std::io::Error> {
        debug!(
            "StdFileReader::open_seekable_reader called with path: {}",
            path
        );
        let path = path.to_string();
        let file = tokio::task::spawn_blocking(move || std::fs::File::open(path))
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))??;
        Ok(Box::new(file))
    }
}

/// Supported audio file extensions
pub const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "ogg", "wav", "aac", "flac", "m4a", "opus"];

/// Supported video file extensions
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "webm", "avi", "mov", "3gp"];

/// Check if a file extension matches the enabled media types
pub fn is_supported_extension(ext: &str, media_types: &[String]) -> bool {
    let ext_lower = ext.to_lowercase();
    let audio_enabled = media_types.iter().any(|t| t == "audio");
    let video_enabled = media_types.iter().any(|t| t == "video");

    (audio_enabled && SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str()))
        || (video_enabled && VIDEO_EXTENSIONS.contains(&ext_lower.as_str()))
}

/// Check if a file path points to a video file
pub fn is_video_file(file_path: &str) -> bool {
    // Check file extension for regular paths
    if let Some(ext) = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
    {
        return VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str());
    }

    // For Android content URIs, check if the URI path contains "/video/"
    if file_path.starts_with("content://") && file_path.contains("/video/") {
        return true;
    }

    false
}

/// Default MusicFileLister using std::fs
///
/// This implementation performs a recursive directory scan
/// using the standard library, suitable for desktop platforms.
pub struct StdMusicFileLister;

#[async_trait]
impl MusicFileLister for StdMusicFileLister {
    async fn list_music_files(
        &self,
        base_path: &str,
        media_types: &[String],
    ) -> Result<Vec<MusicFileInfo>, std::io::Error> {
        // Use spawn_blocking for synchronous directory scanning
        let base_path = base_path.to_string();
        let media_types = media_types.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut audio_files = Vec::new();
            let dir_path = Path::new(&base_path);

            debug!("Scanning directory with StdMusicFileLister: {}", base_path);
            scan_directory_recursive_sync(dir_path, &mut audio_files, &media_types);
            debug!(
                "StdMusicFileLister scan complete. Found {} files",
                audio_files.len()
            );
            Ok(audio_files)
        })
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
    }
}

/// Recursively scan directory for media files (synchronous helper)
///
/// This is a helper function for StdMusicFileLister that performs
/// the actual recursive directory traversal synchronously.
fn scan_directory_recursive_sync(
    dir_path: &Path,
    files: &mut Vec<MusicFileInfo>,
    media_types: &[String],
) {
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if let Some(extension) = path.extension() {
                        let ext_str = extension.to_string_lossy().to_lowercase();
                        if is_supported_extension(&ext_str, media_types) {
                            let filename =
                                path.file_name().unwrap().to_string_lossy().to_string();
                            let absolute_path = path
                                .canonicalize()
                                .unwrap_or_else(|_| path.clone())
                                .to_string_lossy()
                                .to_string();

                            debug!("Found media file: {}", absolute_path);

                            files.push(MusicFileInfo {
                                path: absolute_path,
                                filename,
                                title: None,
                                artist: None,
                                album: None,
                                duration_ms: None,
                                parent_dir: dir_path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string()),
                            });
                        }
                    }
                } else if file_type.is_dir() {
                    scan_directory_recursive_sync(&entry.path(), files, media_types);
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
pub fn set_music_file_lister(
    lister: Box<dyn MusicFileLister>,
) -> Result<(), Box<dyn MusicFileLister>> {
    MUSIC_FILE_LISTER.set(lister)
}

/// Get the current file reader (custom or default)
///
/// Returns the custom reader if one was set, otherwise returns
/// the default StdFileReader.
pub fn get_file_reader() -> &'static dyn FileReader {
    let reader = FILE_READER
        .get()
        .map(|b| b.as_ref())
        .unwrap_or(&StdFileReader);
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
    MUSIC_FILE_LISTER
        .get()
        .map(|b| b.as_ref())
        .unwrap_or(&StdMusicFileLister)
}

/// Static storage for custom lyric reader implementation
static LYRIC_READER: OnceLock<Box<dyn LyricReader>> = OnceLock::new();

/// Trait for reading lyrics files
///
/// This trait allows platform-specific implementations for reading LRC lyrics files:
/// - StdLyricReader: Uses std::fs to read .lrc files (default for desktop)
/// - AndroidLyricReader: Uses a content-URI-to-filesystem-path mapping to read .lrc files
#[async_trait]
pub trait LyricReader: Send + Sync {
    /// Read lyrics for a music file.
    ///
    /// # Arguments
    /// * `file_path` - The path stored in the database (filesystem path on desktop, content URI on Android)
    /// * `filename` - The display filename (e.g., "song.mp3")
    ///
    /// # Returns
    /// - `Ok(Some(bytes))` - Lyrics file content as bytes
    /// - `Ok(None)` - Lyrics file not found (expected for songs without lyrics)
    /// - `Err(io::Error)` - I/O error occurred
    async fn read_lyric(&self, file_path: &str, filename: &str) -> Result<Option<Vec<u8>>, io::Error>;
}

/// Default LyricReader using std::fs
///
/// This implementation constructs the .lrc file path by replacing the extension
/// of the given file_path with ".lrc" and reads it using std::fs.
pub struct StdLyricReader;

#[async_trait]
impl LyricReader for StdLyricReader {
    async fn read_lyric(&self, file_path: &str, _filename: &str) -> Result<Option<Vec<u8>>, io::Error> {
        debug!("StdLyricReader::read_lyric called with file_path: {}", file_path);

        // Construct LRC file path by replacing the file extension with .lrc
        let lrc_path = Path::new(file_path)
            .with_extension("lrc")
            .to_string_lossy()
            .to_string();

        debug!("Attempting to read lyrics file: {}", lrc_path);

        // Use spawn_blocking for std::fs operations to avoid blocking the async runtime
        let path = lrc_path.clone();
        match tokio::task::spawn_blocking(move || std::fs::read(&path))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
        {
            Ok(content) => {
                debug!(
                    "Successfully read lyrics file: {} ({} bytes)",
                    lrc_path,
                    content.len()
                );
                Ok(Some(content))
            }
            Err(e) => {
                debug!(
                    "Lyrics file not found (this is expected for songs without lyrics): {} - Error: {}",
                    lrc_path, e
                );
                Ok(None)
            }
        }
    }
}

/// Set a custom lyric reader implementation
///
/// This function should be called before the server starts to inject
/// a platform-specific lyric reader (e.g., AndroidLyricReader).
///
/// # Arguments
/// * `reader` - Boxed trait object implementing LyricReader
///
/// # Returns
/// - `Ok(())` - Successfully set the reader
/// - `Err(Box<dyn LyricReader>)` - A reader was already set, returns the new one
pub fn set_lyric_reader(reader: Box<dyn LyricReader>) -> Result<(), Box<dyn LyricReader>> {
    debug!("set_lyric_reader: Setting custom lyric reader");
    LYRIC_READER.set(reader)
}

/// Get the current lyric reader (custom or default)
///
/// Returns the custom reader if one was set, otherwise returns
/// the default StdLyricReader.
pub fn get_lyric_reader() -> &'static dyn LyricReader {
    LYRIC_READER
        .get()
        .map(|b| b.as_ref())
        .unwrap_or(&StdLyricReader)
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
        let mut stream = reader
            .read_stream(file_path.to_str().unwrap(), 1024 * 1024)
            .await
            .unwrap();
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

    #[tokio::test]
    async fn test_std_file_reader_seekable_reader() {
        use std::io::{Read, Seek, SeekFrom};

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("seekable.txt");
        fs::write(&file_path, b"abcdef").unwrap();

        let reader = StdFileReader;
        let mut file = reader
            .open_seekable_reader(file_path.to_str().unwrap())
            .await
            .unwrap();

        file.seek(SeekFrom::Start(2)).unwrap();
        let mut buf = [0_u8; 2];
        let read = file.read(&mut buf).unwrap();
        assert_eq!(read, 2);
        assert_eq!(&buf, b"cd");
    }
}
