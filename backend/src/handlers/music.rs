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
use crate::file_ops::get_file_reader;
use crate::types::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use futures::TryStreamExt;
// lofty imports are used locally in cover art extraction functions below
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
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
/// - Audio file stream with `audio/mpeg` content type if found
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
                    response.insert_header(("Content-Type", "audio/mpeg"));
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
/// - Audio file stream with `audio/mpeg` content type if found
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

                    let mut start_byte = (pos * size as f64).floor() as u64;
                    if start_byte >= size {
                        start_byte = size - 1;
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

                    let mut start_byte = ((timestamp / duration) * size as f64).floor() as u64;
                    if start_byte >= size {
                        start_byte = size - 1;
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
                        let content_length = file_size.unwrap() - start;
                        let end = file_size.unwrap() - 1;
                        info!(
                            "[ACCESS] GET /api/music/id/{} - Status: 206, bytes={}-{}",
                            id, start, end
                        );

                        let mut response = HttpResponse::PartialContent();
                        response.insert_header(("Content-Type", "audio/mpeg"));
                        response.insert_header(("Content-Length", content_length.to_string()));
                        response.insert_header((
                            "Content-Range",
                            format!("bytes {}-{}/{}", start, end, file_size.unwrap()),
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
                                let content_length = end - start + 1;
                                info!("[ACCESS] GET /api/music/id/{} - Status: 206, Range: bytes={}-{}", id, start, end);

                                return HttpResponse::PartialContent()
                                    .insert_header(("Content-Type", "audio/mpeg"))
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
                    response.insert_header(("Content-Type", "audio/mpeg"));
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
fn parse_range_header(range: &str, file_size: u64) -> Option<(u64, u64)> {
    // Expected format: "bytes=start-end" or "bytes=start-"
    if !range.starts_with("bytes=") {
        return None;
    }

    let range_spec = &range[6..]; // Skip "bytes="
    let parts: Vec<&str> = range_spec.split('-').collect();

    if parts.len() != 2 {
        return None;
    }

    let start: u64 = parts[0].parse().ok()?;
    let end = if parts[1].is_empty() {
        // "bytes=start-" means from start to end of file
        file_size - 1
    } else {
        parts[1].parse().ok()?
    };

    // Validate range
    if start >= file_size || end >= file_size || start > end {
        return None;
    }

    Some((start, end))
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

/// Get cover art for a music file by ID
///
/// Extracts embedded cover art from audio file metadata using lofty.
/// Returns the front cover image if available, otherwise the first picture found.
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
            let file_reader = get_file_reader();

            // Use open_seekable_reader for both desktop and Android.
            // lofty's Probe::new() accepts any Read + Seek type, so it works
            // with both std::fs::File and MediaStoreSeekableReader.
            // This avoids loading the entire file into memory (which causes OOM on Android).
            match file_reader.open_seekable_reader(&file_path).await {
                Ok(reader) => {
                    let result = tokio::task::spawn_blocking(move || {
                        extract_cover_art_from_reader(reader)
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
                    warn!("Could not open file for cover art ID {}: {}", id, e);
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

/// Extract cover art from a seekable reader using lofty.
///
/// Uses `Probe::new(BufReader::new(reader)).guess_file_type()?.read()` which works
/// with any `Read + Seek` type — both `std::fs::File` (desktop) and
/// `MediaStoreSeekableReader` (Android). Only reads metadata headers, does NOT
/// load the entire file into memory.
fn extract_cover_art_from_reader(
    reader: Box<dyn crate::file_ops::ReadSeekSendSync>,
) -> Result<Option<(String, Vec<u8>)>, String> {
    use lofty::config::ParseOptions;
    use lofty::probe::Probe;

    use std::io::BufReader;

    let tagged_file = Probe::new(BufReader::new(reader))
        .guess_file_type()
        .map_err(|e| format!("Failed to probe file type: {}", e))?
        .options(ParseOptions::new().read_properties(false))
        .read()
        .map_err(|e| format!("Failed to parse audio file: {}", e))?;

    extract_cover_from_tagged_file(&tagged_file)
}

/// Extract the front cover (or first picture) from a parsed TaggedFile.
fn extract_cover_from_tagged_file(
    tagged_file: &lofty::file::TaggedFile,
) -> Result<Option<(String, Vec<u8>)>, String> {
    use lofty::file::TaggedFileExt;
    use lofty::picture::PictureType;
    use lofty::tag::Tag;

    let tag: &Tag = match tagged_file
        .tags()
        .iter()
        .find(|t: &&Tag| !t.pictures().is_empty())
    {
        Some(t) => t,
        None => return Ok(None),
    };

    let pictures = tag.pictures();

    // Prefer front cover, fall back to first picture
    let picture = pictures
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures.first());

    match picture {
        Some(pic) => {
            let mime = pic
                .mime_type()
                .map(|m: &lofty::picture::MimeType| m.as_str().to_string())
                .unwrap_or_else(|| "image/jpeg".to_string());
            Ok(Some((mime, pic.data().to_vec())))
        }
        None => Ok(None),
    }
}

/// Music response with database metadata
#[derive(Serialize)]
pub struct MusicResponse {
    pub id: i32,
    pub filename: String,
    pub file_path: String,
    pub lufs: Option<f64>,
    pub created_at: String,
}
