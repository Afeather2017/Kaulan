//! Android MediaStore adapters for the Kaulan music player.
//!
//! This module provides implementations of FileReader and MusicFileLister
//! that use Android's MediaStore API via the tauri-plugin-android-mediastore.
//!
//! These adapters are only compiled on Android and allow the app to:
//! - Query audio files from the device's MediaStore
//! - Read file contents using content URIs (e.g., content://media/external/audio/media/123)

#[cfg(target_os = "android")]
use async_trait::async_trait;
#[cfg(target_os = "android")]
use kaulan::{FileReader, MusicFileLister, MusicFileInfo};
#[cfg(target_os = "android")]
use tauri_plugin_android_mediastore::{AndroidMediastoreExt, FileReaderOpenRequest, FileReaderReadToEndRequest, FileReaderCloseRequest};
#[cfg(target_os = "android")]
use std::io;

/// MediaStore-based FileReader for Android
///
/// This implementation reads file content using Android's content resolver,
/// which can handle content URIs like `content://media/external/audio/media/123`.
#[cfg(target_os = "android")]
pub struct MediaStoreFileReader {
    app_handle: tauri::AppHandle,
}

#[cfg(target_os = "android")]
impl MediaStoreFileReader {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }
}

#[cfg(target_os = "android")]
#[async_trait]
impl FileReader for MediaStoreFileReader {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, io::Error> {
        // If it's a content URI, use the MediaStore plugin
        if path.starts_with("content://") {
            log::debug!("Reading content URI via MediaStore: {}", path);

            // Open a file reader session - direct await, no block_on needed!
            let open_result = self.app_handle.android_mediastore()
                .file_reader_open(FileReaderOpenRequest {
                    content_uri: path.to_string(),
                })
                .await;

            let session_id = match open_result {
                Ok(response) if response.success => {
                    log::debug!("Opened file reader session: {}", response.session_id);
                    response.session_id
                }
                Ok(response) => {
                    let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                    return Err(io::Error::new(io::ErrorKind::Other, format!("Failed to open file reader: {}", error)));
                }
                Err(e) => {
                    return Err(io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e)));
                }
            };

            // Read all data to end - direct await, no block_on needed!
            let read_result = self.app_handle.android_mediastore()
                .file_reader_read_to_end(FileReaderReadToEndRequest {
                    session_id,
                })
                .await;

            // Close the session - direct await, no block_on needed!
            let _ = self.app_handle.android_mediastore()
                .file_reader_close(FileReaderCloseRequest {
                    session_id,
                })
                .await;

            match read_result {
                Ok(response) if response.success => {
                    if let Some(data_base64) = response.data {
                        // Decode base64 to bytes using the new API
                        use base64::Engine;
                        match base64::engine::general_purpose::STANDARD.decode(&data_base64) {
                            Ok(bytes) => {
                                log::debug!("Successfully read {} bytes from MediaStore", bytes.len());
                                Ok(bytes)
                            }
                            Err(e) => {
                                Err(io::Error::new(io::ErrorKind::InvalidData, format!("Base64 decode error: {}", e)))
                            }
                        }
                    } else {
                        Err(io::Error::new(io::ErrorKind::UnexpectedEof, "No data received"))
                    }
                }
                Ok(response) => {
                    let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                    Err(io::Error::new(io::ErrorKind::Other, format!("Failed to read file: {}", error)))
                }
                Err(e) => {
                    Err(io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e)))
                }
            }
        } else {
            // Fall back to std::fs for regular paths (shouldn't happen on Android)
            log::warn!("MediaStoreFileReader called with non-content URI: {}", path);
            let path = path.to_string();
            tokio::task::spawn_blocking(move || std::fs::read(path))
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
        }
    }
}

/// MediaStore-based MusicFileLister for Android
///
/// This implementation queries Android's MediaStore for audio files,
/// returning metadata like title, artist, album, and content URI.
#[cfg(target_os = "android")]
pub struct MediaStoreMusicFileLister {
    app_handle: tauri::AppHandle,
}

#[cfg(target_os = "android")]
impl MediaStoreMusicFileLister {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    /// Generate a safe filename from metadata
    ///
    /// Since MediaStore doesn't always provide the original filename,
    /// we generate one from artist and title.
    fn generate_filename(artist: &str, title: &str, id: i64) -> String {
        // Sanitize artist and title for filesystem use
        let sanitize = |s: &str| -> String {
            s.chars()
                .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
                .collect()
        };

        let safe_artist = sanitize(artist);
        let safe_title = sanitize(title);

        if safe_artist.is_empty() {
            format!("{}_{}.mp3", id, safe_title)
        } else {
            format!("{}_{}.mp3", safe_artist, safe_title)
        }
    }
}

#[cfg(target_os = "android")]
#[async_trait]
impl MusicFileLister for MediaStoreMusicFileLister {
    async fn list_music_files(&self, _base_path: &str) -> Result<Vec<MusicFileInfo>, io::Error> {
        log::info!("Querying MediaStore for audio files...");

        // Direct await - no block_on needed!
        let response = self.app_handle.android_mediastore().get_audio_files().await;

        match response {
            Ok(audio_files_response) => {
                let files: Vec<MusicFileInfo> = audio_files_response.files.into_iter().map(|af| {
                    let filename = Self::generate_filename(&af.artist, &af.title, af.id);

                    log::debug!("Found audio file: {} - {} ({})", af.artist, af.title, af.content_uri);

                    MusicFileInfo {
                        path: af.content_uri.clone(),
                        filename,
                        title: Some(af.title),
                        artist: Some(af.artist),
                        album: Some(af.album),
                        duration_ms: Some(af.duration),
                    }
                }).collect();

                log::info!("MediaStore query complete: {} audio files found", files.len());
                Ok(files)
            }
            Err(e) => {
                let error_msg = format!("Failed to query MediaStore: {}", e);
                log::error!("{}", error_msg);
                Err(io::Error::new(io::ErrorKind::Other, error_msg))
            }
        }
    }
}

/// Desktop stub implementations for MediaStore adapters
///
/// These are provided so the code compiles on desktop platforms
/// for development and testing purposes.

#[cfg(not(target_os = "android"))]
pub struct MediaStoreFileReader;

#[cfg(not(target_os = "android"))]
impl MediaStoreFileReader {
    pub fn new(_app_handle: tauri::AppHandle) -> Self {
        log::warn!("MediaStoreFileReader is a stub on desktop platforms");
        Self
    }
}

#[cfg(not(target_os = "android"))]
#[async_trait::async_trait]
impl kaulan::FileReader for MediaStoreFileReader {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        log::warn!("MediaStoreFileReader::read_file called on desktop (stub): {}", path);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "MediaStore is only available on Android"
        ))
    }
}

#[cfg(not(target_os = "android"))]
pub struct MediaStoreMusicFileLister;

#[cfg(not(target_os = "android"))]
impl MediaStoreMusicFileLister {
    pub fn new(_app_handle: tauri::AppHandle) -> Self {
        log::warn!("MediaStoreMusicFileLister is a stub on desktop platforms");
        Self
    }
}

#[cfg(not(target_os = "android"))]
#[async_trait::async_trait]
impl kaulan::MusicFileLister for MediaStoreMusicFileLister {
    async fn list_music_files(&self, _base_path: &str) -> Result<Vec<kaulan::MusicFileInfo>, std::io::Error> {
        log::warn!("MediaStoreMusicFileLister::list_music_files called on desktop (stub)");
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "MediaStore is only available on Android"
        ))
    }
}
