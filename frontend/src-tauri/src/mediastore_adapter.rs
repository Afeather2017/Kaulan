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
use bytes::Bytes;
#[cfg(target_os = "android")]
use futures::{stream, Stream};
#[cfg(target_os = "android")]
use kaulan::{FileReader, MusicFileLister, MusicFileInfo, ReadSeekSendSync};
#[cfg(target_os = "android")]
use tauri_plugin_android_mediastore::{
    AndroidMediastoreExt,
    FileReaderCloseRequest,
    FileReaderOpenRequest,
    FileReaderReadRequest,
    FileReaderSeekRequest,
    FileReaderReadToEndRequest,
    GetAudioFilesRequest,
};
#[cfg(target_os = "android")]
use std::io::{self, Read, Seek, SeekFrom};
#[cfg(target_os = "android")]
use std::pin::Pin;
#[cfg(target_os = "android")]
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

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
struct SessionGuard {
    app_handle: tauri::AppHandle,
    session_id: i64,
    closed: Arc<AtomicBool>,
}

#[cfg(target_os = "android")]
impl Drop for SessionGuard {
    fn drop(&mut self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }

        let app_handle = self.app_handle.clone();
        let session_id = self.session_id;
        tokio::spawn(async move {
            let _ = app_handle.android_mediastore()
                .file_reader_close(FileReaderCloseRequest { session_id })
                .await;
        });
    }
}

#[cfg(target_os = "android")]
struct MediaStoreSeekableReader {
    app_handle: tauri::AppHandle,
    session_id: i64,
    position: u64,
    file_size: Option<u64>,
    eof: bool,
    closed: Arc<AtomicBool>,
}

#[cfg(target_os = "android")]
impl MediaStoreSeekableReader {
    fn close_session_if_needed(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let app_handle = self.app_handle.clone();
        let session_id = self.session_id;
        tauri::async_runtime::spawn(async move {
            let _ = app_handle.android_mediastore()
                .file_reader_close(FileReaderCloseRequest { session_id })
                .await;
        });
    }
}

#[cfg(target_os = "android")]
impl Drop for MediaStoreSeekableReader {
    fn drop(&mut self) {
        self.close_session_if_needed();
    }
}

#[cfg(target_os = "android")]
impl Read for MediaStoreSeekableReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.eof {
            return Ok(0);
        }

        let req_size = std::cmp::min(buf.len(), i32::MAX as usize) as i32;
        let response = tauri::async_runtime::block_on(
            self.app_handle
                .android_mediastore()
                .file_reader_read(FileReaderReadRequest {
                    session_id: self.session_id,
                    size: req_size,
                }),
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e)))?;

        if !response.success {
            let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to read file: {}", error),
            ));
        }

        if response.is_eof && response.data.is_none() {
            self.eof = true;
            return Ok(0);
        }

        let bytes = match response.data {
            Some(data_base64) => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(&data_base64)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Base64 decode error: {}", e)))?
            }
            None => Vec::new(),
        };

        let to_copy = std::cmp::min(bytes.len(), buf.len());
        buf[..to_copy].copy_from_slice(&bytes[..to_copy]);
        self.position = self.position.saturating_add(to_copy as u64);
        self.eof = response.is_eof;
        Ok(to_copy)
    }
}

#[cfg(target_os = "android")]
impl Seek for MediaStoreSeekableReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target: i128 = match pos {
            SeekFrom::Start(offset) => offset as i128,
            SeekFrom::Current(offset) => self.position as i128 + offset as i128,
            SeekFrom::End(offset) => {
                let size = self
                    .file_size
                    .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "File size unavailable for SeekFrom::End"))?;
                size as i128 + offset as i128
            }
        };

        if target < 0 || target > i64::MAX as i128 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid seek target"));
        }

        let response = tauri::async_runtime::block_on(
            self.app_handle
                .android_mediastore()
                .file_reader_seek(FileReaderSeekRequest {
                    session_id: self.session_id,
                    position: target as i64,
                }),
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e)))?;

        if !response.success {
            let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to seek file: {}", error),
            ));
        }

        self.position = response.new_position as u64;
        self.eof = false;
        Ok(self.position)
    }
}

#[cfg(target_os = "android")]
#[async_trait]
impl FileReader for MediaStoreFileReader {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, io::Error> {
        log::debug!("MediaStoreFileReader::read_file called with path: {}", path);

        // If it's a content URI, use the MediaStore plugin
        if path.starts_with("content://") {
            log::info!("Reading content URI via MediaStore: {}", path);

            // Open a file reader session - direct await, no block_on needed!
            log::debug!("Attempting to open file reader session for: {}", path);
            let open_result = self.app_handle.android_mediastore()
                .file_reader_open(FileReaderOpenRequest {
                    content_uri: path.to_string(),
                })
                .await;

            let session_id = match open_result {
                Ok(response) if response.success => {
                    log::debug!("Successfully opened file reader session: {}", response.session_id);
                    response.session_id
                }
                Ok(response) => {
                    let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                    log::error!("Failed to open file reader for {}: success=false, error={}", path, error);
                    return Err(io::Error::new(io::ErrorKind::Other, format!("Failed to open file reader: {}", error)));
                }
                Err(e) => {
                    log::error!("Plugin error while opening file reader for {}: {:?}", path, e);
                    return Err(io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e)));
                }
            };

            // Read all data to end - direct await, no block_on needed!
            log::debug!("Reading file content for session: {}", session_id);
            let read_result = self.app_handle.android_mediastore()
                .file_reader_read_to_end(FileReaderReadToEndRequest {
                    session_id,
                })
                .await;

            // Close the session - direct await, no block_on needed!
            log::debug!("Closing file reader session: {}", session_id);
            let close_result = self.app_handle.android_mediastore()
                .file_reader_close(FileReaderCloseRequest {
                    session_id,
                })
                .await;

            match close_result {
                Ok(response) => {
                    if response.success {
                        log::debug!("Successfully closed session: {}", session_id);
                    } else {
                        log::warn!("Failed to close session {}: {}", session_id, response.error.unwrap_or_default());
                    }
                }
                Err(e) => {
                    log::warn!("Error closing session {}: {:?}", session_id, e);
                }
            }

            match read_result {
                Ok(response) if response.success => {
                    if let Some(data_base64) = response.data {
                        log::debug!("Received {} bytes of base64-encoded data", data_base64.len());
                        // Decode base64 to bytes using the new API
                        use base64::Engine;
                        match base64::engine::general_purpose::STANDARD.decode(&data_base64) {
                            Ok(bytes) => {
                                log::info!("Successfully read {} bytes from MediaStore: {}", bytes.len(), path);
                                Ok(bytes)
                            }
                            Err(e) => {
                                log::error!("Base64 decode error for {}: {}", path, e);
                                Err(io::Error::new(io::ErrorKind::InvalidData, format!("Base64 decode error: {}", e)))
                            }
                        }
                    } else {
                        log::error!("No data received for {}", path);
                        Err(io::Error::new(io::ErrorKind::UnexpectedEof, "No data received"))
                    }
                }
                Ok(response) => {
                    let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                    log::error!("Failed to read file {}: success=false, error={}", path, error);
                    Err(io::Error::new(io::ErrorKind::Other, format!("Failed to read file: {}", error)))
                }
                Err(e) => {
                    log::error!("Plugin error while reading file {}: {:?}", path, e);
                    Err(io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e)))
                }
            }
        } else {
            // Fall back to std::fs for regular paths (shouldn't happen on Android)
            log::warn!("MediaStoreFileReader called with non-content URI: {}", path);
            log::warn!("Falling back to std::fs for: {}", path);
            let path = path.to_string();
            tokio::task::spawn_blocking(move || std::fs::read(path))
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?
        }
    }

    async fn read_stream(
        &self,
        path: &str,
        chunk_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, io::Error>> + Send>>, io::Error> {
        log::debug!("MediaStoreFileReader::read_stream called with path: {}", path);

        if !path.starts_with("content://") {
            log::warn!("MediaStoreFileReader::read_stream called with non-content URI: {}", path);
            return Err(io::Error::new(io::ErrorKind::Unsupported, "Expected content URI"));
        }

        log::info!("Streaming content URI via MediaStore: {}", path);
        let open_result = self.app_handle.android_mediastore()
            .file_reader_open(FileReaderOpenRequest {
                content_uri: path.to_string(),
            })
            .await;

        let session_id = match open_result {
            Ok(response) if response.success => response.session_id,
            Ok(response) => {
                let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                return Err(io::Error::new(io::ErrorKind::Other, format!("Failed to open file reader: {}", error)));
            }
            Err(e) => {
                return Err(io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e)));
            }
        };

        let closed = Arc::new(AtomicBool::new(false));
        let guard = SessionGuard {
            app_handle: self.app_handle.clone(),
            session_id,
            closed: closed.clone(),
        };

        let app_handle = self.app_handle.clone();
        let stream = stream::unfold((app_handle, session_id, closed, guard, false), move |state| async move {
            let (app_handle, session_id, closed, guard, done) = state;
            if done {
                return None;
            }
            let read_result = app_handle.android_mediastore()
                .file_reader_read(FileReaderReadRequest {
                    session_id,
                    size: chunk_size as i32,
                })
                .await;

            match read_result {
                Ok(response) if response.success => {
                    if response.is_eof {
                        closed.store(true, Ordering::SeqCst);
                        let _ = app_handle.android_mediastore()
                            .file_reader_close(FileReaderCloseRequest { session_id })
                            .await;
                        return None;
                    }

                    match response.data {
                        Some(data_base64) => {
                            use base64::Engine;
                            match base64::engine::general_purpose::STANDARD.decode(&data_base64) {
                                Ok(bytes) => {
                                    let item = Ok(Bytes::from(bytes));
                                    Some((item, (app_handle, session_id, closed, guard, false)))
                                }
                                Err(e) => {
                                    let _ = app_handle.android_mediastore()
                                        .file_reader_close(FileReaderCloseRequest { session_id })
                                        .await;
                                    closed.store(true, Ordering::SeqCst);
                                    let err = io::Error::new(io::ErrorKind::InvalidData, format!("Base64 decode error: {}", e));
                                    Some((Err(err), (app_handle, session_id, closed, guard, true)))
                                }
                            }
                        }
                        None => {
                            let _ = app_handle.android_mediastore()
                                .file_reader_close(FileReaderCloseRequest { session_id })
                                .await;
                            closed.store(true, Ordering::SeqCst);
                            let err = io::Error::new(io::ErrorKind::UnexpectedEof, "No data received");
                            Some((Err(err), (app_handle, session_id, closed, guard, true)))
                        }
                    }
                }
                Ok(response) => {
                    let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                    let _ = app_handle.android_mediastore()
                        .file_reader_close(FileReaderCloseRequest { session_id })
                        .await;
                    closed.store(true, Ordering::SeqCst);
                    let err = io::Error::new(io::ErrorKind::Other, format!("Failed to read file: {}", error));
                    Some((Err(err), (app_handle, session_id, closed, guard, true)))
                }
                Err(e) => {
                    let _ = app_handle.android_mediastore()
                        .file_reader_close(FileReaderCloseRequest { session_id })
                        .await;
                    closed.store(true, Ordering::SeqCst);
                    let err = io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e));
                    Some((Err(err), (app_handle, session_id, closed, guard, true)))
                }
            }
        });

        Ok(Box::pin(stream))
    }

    async fn read_stream_from(
        &self,
        path: &str,
        chunk_size: usize,
        start_pos: u64,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, io::Error>> + Send>>, io::Error> {
        log::debug!("MediaStoreFileReader::read_stream_from called with path: {}, start_pos: {}", path, start_pos);

        if !path.starts_with("content://") {
            log::warn!("MediaStoreFileReader::read_stream_from called with non-content URI: {}", path);
            return Err(io::Error::new(io::ErrorKind::Unsupported, "Expected content URI"));
        }

        let open_result = self.app_handle.android_mediastore()
            .file_reader_open(FileReaderOpenRequest {
                content_uri: path.to_string(),
            })
            .await;

        let session_id = match open_result {
            Ok(response) if response.success => response.session_id,
            Ok(response) => {
                let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                return Err(io::Error::new(io::ErrorKind::Other, format!("Failed to open file reader: {}", error)));
            }
            Err(e) => {
                return Err(io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e)));
            }
        };

        // Use the seek API to position the file pointer - much more efficient than reading and discarding
        if start_pos > 0 {
            log::debug!("Seeking to position {} using MediaStore seek API", start_pos);
            let seek_result = self.app_handle.android_mediastore()
                .file_reader_seek(FileReaderSeekRequest {
                    session_id,
                    position: start_pos as i64,
                })
                .await;

            match seek_result {
                Ok(response) if response.success => {
                    log::debug!("Seek successful, new position: {}", response.new_position);
                }
                Ok(response) => {
                    let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                    log::error!("Failed to seek: {}", error);
                    let _ = self.app_handle.android_mediastore()
                        .file_reader_close(FileReaderCloseRequest { session_id })
                        .await;
                    return Err(io::Error::new(io::ErrorKind::Other, format!("Failed to seek: {}", error)));
                }
                Err(e) => {
                    log::error!("Plugin error while seeking: {:?}", e);
                    let _ = self.app_handle.android_mediastore()
                        .file_reader_close(FileReaderCloseRequest { session_id })
                        .await;
                    return Err(io::Error::new(io::ErrorKind::Other, format!("Plugin error while seeking: {}", e)));
                }
            }
        }

        let closed = Arc::new(AtomicBool::new(false));
        let guard = SessionGuard {
            app_handle: self.app_handle.clone(),
            session_id,
            closed: closed.clone(),
        };

        let app_handle = self.app_handle.clone();

        let stream = stream::unfold((app_handle, session_id, closed, guard, false), move |state| async move {
            let (app_handle, session_id, closed, guard, done) = state;
            if done {
                return None;
            }

            let read_result = app_handle.android_mediastore()
                .file_reader_read(FileReaderReadRequest {
                    session_id,
                    size: chunk_size as i32,
                })
                .await;

            match read_result {
                Ok(response) if response.success => {
                    if response.is_eof {
                        closed.store(true, Ordering::SeqCst);
                        let _ = app_handle.android_mediastore()
                            .file_reader_close(FileReaderCloseRequest { session_id })
                            .await;
                        return None;
                    }

                    match response.data {
                        Some(data_base64) => {
                            use base64::Engine;
                            match base64::engine::general_purpose::STANDARD.decode(&data_base64) {
                                Ok(bytes) => {
                                    let item = Ok(Bytes::from(bytes));
                                    Some((item, (app_handle, session_id, closed, guard, false)))
                                }
                                Err(e) => {
                                    let _ = app_handle.android_mediastore()
                                        .file_reader_close(FileReaderCloseRequest { session_id })
                                        .await;
                                    closed.store(true, Ordering::SeqCst);
                                    let err = io::Error::new(io::ErrorKind::InvalidData, format!("Base64 decode error: {}", e));
                                    Some((Err(err), (app_handle, session_id, closed, guard, true)))
                                }
                            }
                        }
                        None => {
                            let _ = app_handle.android_mediastore()
                                .file_reader_close(FileReaderCloseRequest { session_id })
                                .await;
                            closed.store(true, Ordering::SeqCst);
                            let err = io::Error::new(io::ErrorKind::UnexpectedEof, "No data received");
                            Some((Err(err), (app_handle, session_id, closed, guard, true)))
                        }
                    }
                }
                Ok(response) => {
                    let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                    let _ = app_handle.android_mediastore()
                        .file_reader_close(FileReaderCloseRequest { session_id })
                        .await;
                    closed.store(true, Ordering::SeqCst);
                    let err = io::Error::new(io::ErrorKind::Other, format!("Failed to read file: {}", error));
                    Some((Err(err), (app_handle, session_id, closed, guard, true)))
                }
                Err(e) => {
                    let _ = app_handle.android_mediastore()
                        .file_reader_close(FileReaderCloseRequest { session_id })
                        .await;
                    closed.store(true, Ordering::SeqCst);
                    let err = io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e));
                    Some((Err(err), (app_handle, session_id, closed, guard, true)))
                }
            }
        });

        Ok(Box::pin(stream))
    }

    async fn get_file_size(&self, path: &str) -> Result<u64, io::Error> {
        log::debug!("MediaStoreFileReader::get_file_size called with path: {}", path);

        if !path.starts_with("content://") {
            log::warn!("MediaStoreFileReader::get_file_size called with non-content URI: {}", path);
            return Err(io::Error::new(io::ErrorKind::Unsupported, "Expected content URI"));
        }

        // Open file reader to get file size from response
        let open_result = self.app_handle.android_mediastore()
            .file_reader_open(FileReaderOpenRequest {
                content_uri: path.to_string(),
            })
            .await;

        let file_size = match open_result {
            Ok(response) if response.success => {
                if let Some(size) = response.file_size {
                    // Close the session immediately since we only needed the size
                    let _ = self.app_handle.android_mediastore()
                        .file_reader_close(FileReaderCloseRequest { session_id: response.session_id })
                        .await;
                    Ok(size as u64)
                } else {
                    // No size available - return error
                    let _ = self.app_handle.android_mediastore()
                        .file_reader_close(FileReaderCloseRequest { session_id: response.session_id })
                        .await;
                    Err(io::Error::new(io::ErrorKind::Other, "File size not available"))
                }
            }
            Ok(response) => {
                let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                Err(io::Error::new(io::ErrorKind::Other, format!("Failed to open file reader: {}", error)))
            }
            Err(e) => {
                Err(io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e)))
            }
        };

        file_size
    }

    async fn open_seekable_reader(&self, path: &str) -> Result<Box<dyn ReadSeekSendSync>, io::Error> {
        log::debug!("MediaStoreFileReader::open_seekable_reader called with path: {}", path);

        if path.starts_with("content://") {
            let open_result = self.app_handle.android_mediastore()
                .file_reader_open(FileReaderOpenRequest {
                    content_uri: path.to_string(),
                })
                .await;

            match open_result {
                Ok(response) if response.success => {
                    let reader = MediaStoreSeekableReader {
                        app_handle: self.app_handle.clone(),
                        session_id: response.session_id,
                        position: 0,
                        file_size: response.file_size.map(|s| s as u64),
                        eof: false,
                        closed: Arc::new(AtomicBool::new(false)),
                    };
                    Ok(Box::new(reader))
                }
                Ok(response) => {
                    let error = response.error.unwrap_or_else(|| "Unknown error".to_string());
                    Err(io::Error::new(io::ErrorKind::Other, format!("Failed to open file reader: {}", error)))
                }
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, format!("Plugin error: {}", e))),
            }
        } else {
            let path = path.to_string();
            let file = tokio::task::spawn_blocking(move || std::fs::File::open(path))
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))??;
            Ok(Box::new(file))
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
        // Note: MediaStore already filters by MIME type audio/*
        // We use empty request (no exclusions) since AudioFile doesn't include file extension
        let response = self.app_handle.android_mediastore().get_audio_files(GetAudioFilesRequest {
            exclude_suffixes: None,
        }).await;

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

    async fn read_stream(
        &self,
        path: &str,
        _chunk_size: usize,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send>>, std::io::Error> {
        log::warn!("MediaStoreFileReader::read_stream called on desktop (stub): {}", path);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "MediaStore is only available on Android"
        ))
    }

    async fn read_stream_from(
        &self,
        path: &str,
        _chunk_size: usize,
        _start_pos: u64,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, std::io::Error>> + Send>>, std::io::Error> {
        log::warn!("MediaStoreFileReader::read_stream_from called on desktop (stub): {}", path);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "MediaStore is only available on Android"
        ))
    }

    async fn get_file_size(&self, path: &str) -> Result<u64, std::io::Error> {
        log::warn!("MediaStoreFileReader::get_file_size called on desktop (stub): {}", path);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "MediaStore is only available on Android"
        ))
    }

    async fn open_seekable_reader(&self, path: &str) -> Result<Box<dyn kaulan::ReadSeekSendSync>, std::io::Error> {
        log::warn!("MediaStoreFileReader::open_seekable_reader called on desktop (stub): {}", path);
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
