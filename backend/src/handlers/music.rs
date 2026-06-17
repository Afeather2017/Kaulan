//! Music streaming and metadata API handlers.
//!
//! This module provides endpoints for:
//! - Streaming individual music files with position-based seeking
//! - Getting all music from the database
//!
//! # Position-Based Streaming
//!
//! The `GET /api/music/id/{id}` endpoint supports an optional `position` query parameter
//! (0.0 to 1.0) for efficient seeking without downloading from the beginning.
//!
//! ## Example
//!
//! ```http
//! GET /api/music/id/25?position=0.5
//! ```
//!
//! This streams from 50% of the file, saving bandwidth for large seeks.
//!
//! See [`docs/position-based-streaming.md`](../../../docs/position-based-streaming.md) for details.

use crate::entities::music::{Column as MusicColumn, Entity as MusicEntity, Model as MusicModel};
use crate::file_ops::{get_file_reader, source_remove_file};
use crate::types::{
    AppState, DeleteMusicFailure, DeleteMusicRequest, DeleteMusicResponse, MusicResponse,
};
use actix_web::{delete, get, web, HttpRequest, HttpResponse, Responder};
use futures::TryStreamExt;
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use serde::Deserialize;
use std::path::Path;
use tracing::{debug, error, info, warn};

/// Query parameters for position-based seeking
#[derive(Deserialize)]
struct MusicQueryParams {
    /// Position in file (0.0 to 1.0) to start streaming from
    /// Example: 0.5 = 50% into the file
    position: Option<f64>,

    /// Legacy timestamp-based seek in seconds.
    t: Option<f64>,

    /// Total track duration in seconds, required when `t` is used.
    duration: Option<f64>,
}

/// Stream a music file by filename
///
/// This endpoint looks up the music file in the database by filename,
/// then streams the actual audio file from disk or content URI.
///
/// # Path Parameters
/// * `filename` - The filename to look up in the database
///
/// # Returns
/// - Audio file stream with an extension-aware audio content type if found
/// - `404 Not Found` if music not in database or file missing
/// - `500 Internal Server Error` for database errors
#[get("/api/music/{filename}")]
pub async fn get_music(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let filename = path.into_inner();
    debug!("Music request received for filename: {}", filename);

    // Simple access log
    info!("[ACCESS] GET /api/music/{} - Started", filename);

    match MusicEntity::find()
        .filter(MusicColumn::Filename.eq(&filename))
        .one(&data.db_conn)
        .await
    {
        Ok(Some(music)) => {
            debug!(
                "Found music in database: filename={}, file_path={}",
                music.filename, music.file_path
            );

            let file_reader = get_file_reader();
            debug!("File reader obtained for reading: {}", music.file_path);

            const CHUNK_SIZE: usize = 1024 * 1024;
            match file_reader.read_stream(&music.file_path, CHUNK_SIZE).await {
                Ok(stream) => {
                    debug!("Streaming music file: {}", filename);

                    // Get file size from FileReader trait (works for both desktop and Android)
                    let file_size = file_reader.get_file_size(&music.file_path).await.ok();

                    info!("[ACCESS] GET /api/music/{} - Status: 200", filename);
                    let mut response = HttpResponse::Ok();
                    response.insert_header(("Content-Type", audio_content_type(&music.filename)));
                    response
                        .insert_header(("Cache-Control", "public, max-age=86400, must-revalidate"));
                    response.insert_header(("Accept-Ranges", "bytes"));

                    // Add Content-Length if available (helps browser determine duration)
                    if let Some(size) = file_size {
                        response.insert_header(("Content-Length", size.to_string()));
                    }

                    response.streaming(stream.map_err(actix_web::Error::from))
                }
                Err(e) => {
                    warn!(
                        "File not found or could not be read: {} - Error: {}",
                        music.file_path, e
                    );
                    info!("[ACCESS] GET /api/music/{} - Status: 404", filename);
                    HttpResponse::NotFound().body("File not found")
                }
            }
        }
        Ok(None) => {
            warn!("Music not found in database: {}", filename);
            info!("[ACCESS] GET /api/music/{} - Status: 404", filename);
            HttpResponse::NotFound().body("Music not found")
        }
        Err(e) => {
            error!("Database error while fetching music {}: {}", filename, e);
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}

/// Stream a music file by ID
///
/// This endpoint looks up the music file in the database by ID,
/// then streams the actual audio file from disk or content URI.
/// Supports position-based seeking via query parameter and HTTP Range requests.
///
/// # Path Parameters
/// * `id` - The music ID to look up in the database
///
/// # Query Parameters
/// * `position` - Optional position in file (0.0 to 1.0) to start streaming from
///   - `0.0` = beginning of file
///   - `0.5` = middle of file
///   - `1.0` = end of file
///
/// # Returns
/// - Audio file stream with an extension-aware audio content type if found
/// - HTTP 206 (Partial Content) if `position` parameter is provided
/// - HTTP 206 (Partial Content) if Range header is present
/// - HTTP 200 OK for normal full file streaming
/// - `404 Not Found` if music not in database or file missing
/// - `500 Internal Server Error` for database errors
///
/// # Example
/// ```bash
/// # Stream from 10% position (saves bandwidth for large seeks)
/// curl "http://localhost:2080/api/music/id/25?position=0.1"
///
/// # Stream from beginning (default)
/// curl "http://localhost:2080/api/music/id/25"
/// ```
#[get("/api/music/id/{id}")]
pub async fn get_music_by_id(
    path: web::Path<i32>,
    query: web::Query<MusicQueryParams>,
    data: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    let id = path.into_inner();
    debug!("Music request received for ID: {}", id);

    // Simple access log
    info!("[ACCESS] GET /api/music/id/{} - Started", id);

    // Parse Range header if present (for seeking support)
    let range_header = req
        .headers()
        .get("Range")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    debug!("Range header: {:?}", range_header);

    match MusicEntity::find_by_id(id).one(&data.db_conn).await {
        Ok(Some(music)) => {
            debug!(
                "Found music in database: id={}, filename={}, file_path={}",
                music.id, music.filename, music.file_path
            );

            let file_reader = get_file_reader();
            debug!("File reader obtained for reading: {}", music.file_path);

            // Get file size for Range support
            let file_size = match file_reader.get_file_size(&music.file_path).await {
                Ok(size) => {
                    debug!("File size: {} bytes", size);
                    Some(size)
                }
                Err(e) => {
                    warn!("Could not get file size: {}", e);
                    None
                }
            };

            const CHUNK_SIZE: usize = 1024 * 1024;

            // Handle position-based seek (highest priority)
            // Priority: Position query parameter > legacy timestamp query parameters > Range header > Full file stream
            let seek_request = file_size.and_then(|size| {
                if let Some(pos) = query.position {
                    if !(0.0..=1.0).contains(&pos) {
                        debug!(
                            "Invalid position parameter: position={}, falling back to normal",
                            pos
                        );
                        return None;
                    }

                    let mut start_byte = proportional_byte_offset(pos, size);
                    if start_byte >= size {
                        start_byte = size.saturating_sub(1);
                    }

                    debug!(
                        "Position seek: position={}%, calculated start_byte={}",
                        pos * 100.0,
                        start_byte
                    );
                    return Some((start_byte, Some(("X-Seek-Position", pos.to_string()))));
                }

                if let (Some(timestamp), Some(duration)) = (query.t, query.duration) {
                    if duration <= 0.0 || timestamp < 0.0 || timestamp > duration {
                        debug!(
                            "Invalid timestamp seek parameters: t={}, duration={}, falling back to normal",
                            timestamp, duration
                        );
                        return None;
                    }

                    let mut start_byte = proportional_byte_offset(timestamp / duration, size);
                    if start_byte >= size {
                        start_byte = size.saturating_sub(1);
                    }

                    debug!(
                        "Timestamp seek: t={}, duration={}, calculated start_byte={}",
                        timestamp,
                        duration,
                        start_byte
                    );
                    return Some((start_byte, Some(("X-Seek-Timestamp", timestamp.to_string()))));
                }

                None
            });

            if let Some((start, seek_header)) = seek_request {
                match file_reader
                    .read_stream_from(&music.file_path, CHUNK_SIZE, start)
                    .await
                {
                    Ok(stream) => {
                        let Some(size) = file_size else {
                            warn!(
                                "Seek request for music ID {} had no file size after validation",
                                id
                            );
                            return HttpResponse::InternalServerError()
                                .body("File size unavailable for seek request");
                        };
                        let content_length = size.saturating_sub(start);
                        let end = size.saturating_sub(1);
                        info!(
                            "[ACCESS] GET /api/music/id/{} - Status: 206, bytes={}-{}",
                            id, start, end
                        );

                        let mut response = HttpResponse::PartialContent();
                        response
                            .insert_header(("Content-Type", audio_content_type(&music.filename)));
                        response.insert_header(("Content-Length", content_length.to_string()));
                        response.insert_header((
                            "Content-Range",
                            format!("bytes {}-{}/{}", start, end, size),
                        ));
                        response.insert_header(("Accept-Ranges", "bytes"));
                        response.insert_header((
                            "Cache-Control",
                            "public, max-age=86400, must-revalidate",
                        ));
                        if let Some((header_name, header_value)) = seek_header {
                            response.insert_header((header_name, header_value));
                        }

                        return response.streaming(stream.map_err(actix_web::Error::from));
                    }
                    Err(e) => {
                        warn!("Could not seek in file: {} - Error: {}", music.file_path, e);
                        // Fall through to Range header handling
                    }
                }
            }

            // Handle Range request
            if let Some(range) = range_header {
                if let Some(size) = file_size {
                    // Parse Range header (format: "bytes=start-end")
                    if let Some((start, end)) = parse_range_header(&range, size) {
                        debug!("Range request: bytes={}-{}", start, end);

                        match file_reader
                            .read_stream_from(&music.file_path, CHUNK_SIZE, start)
                            .await
                        {
                            Ok(stream) => {
                                let content_length = end.saturating_sub(start).saturating_add(1);
                                info!("[ACCESS] GET /api/music/id/{} - Status: 206, Range: bytes={}-{}", id, start, end);

                                return HttpResponse::PartialContent()
                                    .insert_header((
                                        "Content-Type",
                                        audio_content_type(&music.filename),
                                    ))
                                    .insert_header(("Content-Length", content_length.to_string()))
                                    .insert_header((
                                        "Content-Range",
                                        format!("bytes {}-{}/{}", start, end, size),
                                    ))
                                    .insert_header(("Accept-Ranges", "bytes"))
                                    .insert_header((
                                        "Cache-Control",
                                        "public, max-age=86400, must-revalidate",
                                    ))
                                    .streaming(stream.map_err(actix_web::Error::from));
                            }
                            Err(e) => {
                                warn!("Could not seek in file: {} - Error: {}", music.file_path, e);
                                info!("[ACCESS] GET /api/music/id/{} - Status: 404", id);
                                return HttpResponse::NotFound().body("File not found");
                            }
                        }
                    }
                }
            }

            // Non-range request or no file size available
            match file_reader.read_stream(&music.file_path, CHUNK_SIZE).await {
                Ok(stream) => {
                    debug!("Streaming music file: {} (ID: {})", music.filename, id);

                    info!("[ACCESS] GET /api/music/id/{} - Status: 200", id);
                    let mut response = HttpResponse::Ok();
                    response.insert_header(("Content-Type", audio_content_type(&music.filename)));
                    response.insert_header(("Accept-Ranges", "bytes"));
                    response
                        .insert_header(("Cache-Control", "public, max-age=86400, must-revalidate"));

                    // Add Content-Length if available (helps browser determine duration)
                    if let Some(size) = file_size {
                        response.insert_header(("Content-Length", size.to_string()));
                    }

                    response.streaming(stream.map_err(actix_web::Error::from))
                }
                Err(e) => {
                    warn!(
                        "File not found or could not be read: {} (ID: {}) - Error: {}",
                        music.file_path, id, e
                    );
                    info!("[ACCESS] GET /api/music/id/{} - Status: 404", id);
                    HttpResponse::NotFound().body("File not found")
                }
            }
        }
        Ok(None) => {
            warn!("Music not found in database: ID {}", id);
            info!("[ACCESS] GET /api/music/id/{} - Status: 404", id);
            HttpResponse::NotFound().body("Music not found")
        }
        Err(e) => {
            error!("Database error while fetching music ID {}: {}", id, e);
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}

/// Parse HTTP Range header (format: "bytes=start-end")
/// Returns (start, end) tuple or None if invalid
#[allow(clippy::as_conversions)]
fn proportional_byte_offset(ratio: f64, size: u64) -> u64 {
    (ratio * size as f64).floor() as u64
}

fn parse_range_header(range: &str, file_size: u64) -> Option<(u64, u64)> {
    // Expected format: "bytes=start-end" or "bytes=start-"
    let range_spec = range.strip_prefix("bytes=")?;
    let (start_text, end_text) = range_spec.split_once('-')?;

    let start: u64 = start_text.parse().ok()?;
    let end = if end_text.is_empty() {
        // "bytes=start-" means from start to end of file
        file_size.checked_sub(1)?
    } else {
        end_text.parse().ok()?
    };

    // Validate range
    if start >= file_size || end >= file_size || start > end {
        return None;
    }

    Some((start, end))
}

fn audio_content_type(filename: &str) -> &'static str {
    let extension = Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match extension.as_deref() {
        Some("flac") => "audio/flac",
        Some("ogg") | Some("oga") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("m4a") | Some("mp4") => "audio/mp4",
        Some("aac") => "audio/aac",
        Some("opus") => "audio/opus",
        Some("webm") => "audio/webm",
        Some("mp3") => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

/// Get all music from the database
///
/// Returns a list of all music entries with their metadata including
/// filename, file path, LUFS value, and creation timestamp.
///
/// # Returns
/// JSON array of `MusicResponse` objects
#[get("/api/music")]
pub async fn get_all_music(data: web::Data<AppState>) -> impl Responder {
    debug!("Get all music request received");
    info!("[ACCESS] GET /api/music - Started");
    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            info!("Returning {} music entries", music_list.len());
            info!("[ACCESS] GET /api/music - Status: 200");
            let response: Vec<MusicResponse> = music_list
                .into_iter()
                .map(|music: MusicModel| MusicResponse {
                    id: music.id,
                    filename: music.filename,
                    file_path: music.file_path,
                    lufs: music.lufs,
                    created_at: music.created_at.to_rfc3339(),
                })
                .collect();
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            error!("Database error while fetching all music: {}", e);
            info!("[ACCESS] GET /api/music - Status: 500");
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}

#[delete("/api/music/batch")]
pub async fn delete_music_batch(
    payload: web::Json<DeleteMusicRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    let ids = payload.ids.clone();
    info!(
        "[ACCESS] DELETE /api/music/batch - Started ({} ids)",
        ids.len()
    );

    if ids.is_empty() {
        return HttpResponse::BadRequest().json(DeleteMusicResponse {
            success: false,
            message: "No music IDs provided".to_string(),
            deleted_ids: Vec::new(),
            failed: Vec::new(),
        });
    }

    let mut deleted_ids = Vec::new();
    let mut failed = Vec::new();

    for id in ids {
        match MusicEntity::find_by_id(id).one(&data.db_conn).await {
            Ok(Some(music)) => {
                if let Err(err) = source_remove_file(&music.file_path).await {
                    warn!(
                        "Failed to delete music file for ID {} ({}): {}",
                        id, music.file_path, err
                    );
                    failed.push(DeleteMusicFailure {
                        id,
                        reason: err.to_string(),
                    });
                    continue;
                }

                remove_sidecar_lyrics(&music.file_path).await;

                match music.delete(&data.db_conn).await {
                    Ok(_) => {
                        info!("Deleted music ID {} from filesystem and database", id);
                        deleted_ids.push(id);
                    }
                    Err(err) => {
                        error!("Failed to delete music ID {} from database: {}", id, err);
                        failed.push(DeleteMusicFailure {
                            id,
                            reason: err.to_string(),
                        });
                    }
                }
            }
            Ok(None) => failed.push(DeleteMusicFailure {
                id,
                reason: "Music not found".to_string(),
            }),
            Err(err) => {
                error!(
                    "Database error while loading music ID {} for deletion: {}",
                    id, err
                );
                failed.push(DeleteMusicFailure {
                    id,
                    reason: err.to_string(),
                });
            }
        }
    }

    let success = failed.is_empty();
    let mut status = if success {
        HttpResponse::Ok()
    } else if deleted_ids.is_empty() {
        HttpResponse::BadRequest()
    } else {
        HttpResponse::Ok()
    };

    info!(
        "[ACCESS] DELETE /api/music/batch - Status: {} ({} deleted, {} failed)",
        if success {
            200
        } else if deleted_ids.is_empty() {
            400
        } else {
            200
        },
        deleted_ids.len(),
        failed.len()
    );

    status.json(DeleteMusicResponse {
        success,
        message: if success {
            format!("Deleted {} songs", deleted_ids.len())
        } else {
            format!(
                "Deleted {} songs, {} failed",
                deleted_ids.len(),
                failed.len()
            )
        },
        deleted_ids,
        failed,
    })
}

async fn remove_sidecar_lyrics(file_path: &str) {
    for extension in ["lrc", "vtt"] {
        let lyric_path = std::path::Path::new(file_path)
            .with_extension(extension)
            .to_string_lossy()
            .to_string();
        match source_remove_file(&lyric_path).await {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => warn!("Failed to remove sidecar lyric {}: {}", lyric_path, err),
        }
    }
}

/// Get cover art for a music file by ID
///
/// Extracts embedded cover art from audio file metadata using the shared
/// FFmpeg pipeline. Non-filesystem sources are first materialized into a
/// temporary local file so the same probing logic works for Android content
/// URIs and desktop paths.
///
/// # Path Parameters
/// * `id` - The music ID to look up in the database
///
/// # Returns
/// - Image data with appropriate `Content-Type` if cover art found
/// - `404 Not Found` if music not in database, file missing, or no cover art
/// - `500 Internal Server Error` for database errors
#[get("/api/music/id/{id}/cover")]
pub async fn get_music_cover(path: web::Path<i32>, data: web::Data<AppState>) -> impl Responder {
    let id = path.into_inner();
    debug!("Cover art request for music ID: {}", id);
    info!("[ACCESS] GET /api/music/id/{}/cover - Started", id);

    match MusicEntity::find_by_id(id).one(&data.db_conn).await {
        Ok(Some(music)) => {
            let file_path = music.file_path.clone();
            match crate::ffmpeg::prepare_input(&file_path).await {
                Ok(prepared_input) => {
                    let result = tokio::task::spawn_blocking(move || {
                        let prepared = prepared_input;
                        crate::ffmpeg::extract_cover_art(prepared.path())
                    })
                    .await;

                    match result {
                        Ok(Ok(Some((content_type, data)))) => {
                            info!(
                                "[ACCESS] GET /api/music/id/{}/cover - Status: 200, {} bytes",
                                id,
                                data.len()
                            );
                            HttpResponse::Ok()
                                .insert_header(("Content-Type", content_type))
                                .insert_header((
                                    "Cache-Control",
                                    "public, max-age=86400, must-revalidate",
                                ))
                                .body(data)
                        }
                        Ok(Ok(None)) => {
                            debug!("No cover art found for music ID: {}", id);
                            info!("[ACCESS] GET /api/music/id/{}/cover - Status: 404", id);
                            HttpResponse::NotFound().body("No cover art")
                        }
                        Ok(Err(e)) => {
                            warn!("Failed to extract cover art for ID {}: {}", id, e);
                            info!("[ACCESS] GET /api/music/id/{}/cover - Status: 404", id);
                            HttpResponse::NotFound().body("Could not extract cover art")
                        }
                        Err(e) => {
                            error!("Task join error for cover art ID {}: {}", id, e);
                            HttpResponse::InternalServerError().body("Internal error")
                        }
                    }
                }
                Err(e) => {
                    warn!("Could not prepare file for cover art ID {}: {}", id, e);
                    info!("[ACCESS] GET /api/music/id/{}/cover - Status: 404", id);
                    HttpResponse::NotFound().body("File not found")
                }
            }
        }
        Ok(None) => {
            warn!("Music not found in database: ID {}", id);
            info!("[ACCESS] GET /api/music/id/{}/cover - Status: 404", id);
            HttpResponse::NotFound().body("Music not found")
        }
        Err(e) => {
            error!("Database error while fetching music ID {}: {}", id, e);
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}
