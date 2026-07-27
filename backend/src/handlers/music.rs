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
use crate::file_ops::{get_file_reader, source_remove_file, SUPPORTED_EXTENSIONS};
use crate::services::download::sanitize_filename;
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

    /// When set (`?download=1`), send `Content-Disposition: attachment` so the
    /// browser saves the file via its download manager instead of playing it
    /// inline. Used by the browser "download to local" flow; playback omits it.
    /// Accepted as a string so `1`/`true`/`yes` (and a bare flag) count as on,
    /// while `0`/`false`/`no` count as off; see [`is_download_requested`].
    #[serde(default)]
    download: Option<String>,
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
/// * `download` - Optional flag (`1`/`true`/`yes`, or `0`/`false`/`no`). When on,
///   the response carries `Content-Disposition: attachment; filename="...";
///   filename*=UTF-8''...` so the browser saves the file via its download
///   manager instead of playing it inline (used by the browser "download to
///   local" flow; see `docs/library-import.md`). Absent for normal playback.
///
/// # Returns
/// - Audio file stream with an extension-aware audio content type if found
/// - HTTP 206 (Partial Content) if `position` parameter is provided
/// - HTTP 206 (Partial Content) if Range header is present
/// - HTTP 200 OK for normal full file streaming
/// - `Content-Disposition: attachment` header present on every 2xx response
///   when `download=1` is set (so resume/Range requests keep the filename too)
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

    match MusicEntity::find_by_id(id).one(&data.db_conn).await {
        Ok(Some(music)) => {
            debug!(
                "Found music in database: id={}, filename={}, file_path={}",
                music.id, music.filename, music.file_path
            );
            let access_tag = format!("/api/music/id/{}", id);
            build_audio_stream_response(
                &music.file_path,
                &music.filename,
                &query,
                &req,
                &access_tag,
            )
            .await
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

/// Build the streaming `HttpResponse` for an audio file at `file_path`.
///
/// Handles position-based seeking (`?position=`/`?t=`+`?duration=`), HTTP Range
/// requests, and full-file streaming. Shared by the DB-backed
/// `/api/music/id/{id}` endpoint and the path-based `/api/music/path` endpoint
/// used by the "open file as default app" flow.
///
/// `filename` is used to derive the `Content-Type` and the optional
/// `Content-Disposition` header (set only when `query.download` requests it).
/// `access_tag` is the route prefix used in `[ACCESS]` log lines (e.g.
/// `/api/music/id/42` or `/api/music/path`) so the shared log lines stay
/// uniform across callers.
async fn build_audio_stream_response(
    file_path: &str,
    filename: &str,
    query: &MusicQueryParams,
    req: &HttpRequest,
    access_tag: &str,
) -> HttpResponse {
    // Compute the Content-Disposition once so every response branch
    // (seek 206, Range 206, full 200) stays consistent. Only set when
    // the caller asked for a download; absent for normal playback.
    let disposition =
        download_disposition(is_download_requested(query.download.as_deref()), filename);

    let file_reader = get_file_reader();
    debug!("File reader obtained for reading: {}", file_path);

    // Parse Range header if present (for seeking support)
    let range_header = req
        .headers()
        .get("Range")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    debug!("Range header: {:?}", range_header);

    // Get file size for Range support
    let file_size = match file_reader.get_file_size(file_path).await {
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
                timestamp, duration, start_byte
            );
            return Some((
                start_byte,
                Some(("X-Seek-Timestamp", timestamp.to_string())),
            ));
        }

        None
    });

    if let Some((start, seek_header)) = seek_request {
        match file_reader
            .read_stream_from(file_path, CHUNK_SIZE, start)
            .await
        {
            Ok(stream) => {
                let Some(size) = file_size else {
                    warn!(
                        "Seek request for {} had no file size after validation",
                        file_path
                    );
                    info!("[ACCESS] GET {} - Status: 500", access_tag);
                    return HttpResponse::InternalServerError()
                        .body("File size unavailable for seek request");
                };
                let content_length = size.saturating_sub(start);
                let end = size.saturating_sub(1);

                info!(
                    "[ACCESS] GET {} - Status: 206, bytes={}-{}",
                    access_tag, start, end
                );
                let mut response = HttpResponse::PartialContent();
                response.insert_header(("Content-Type", audio_content_type(filename)));
                response.insert_header(("Content-Length", content_length.to_string()));
                response
                    .insert_header(("Content-Range", format!("bytes {}-{}/{}", start, end, size)));
                response.insert_header(("Accept-Ranges", "bytes"));
                response.insert_header(("Cache-Control", "public, max-age=86400, must-revalidate"));
                if let Some((header_name, header_value)) = seek_header {
                    response.insert_header((header_name, header_value));
                }
                if let Some(value) = disposition.as_deref() {
                    response.insert_header(("Content-Disposition", value));
                }

                return response.streaming(stream.map_err(actix_web::Error::from));
            }
            Err(e) => {
                warn!("Could not seek in file: {} - Error: {}", file_path, e);
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
                    .read_stream_from(file_path, CHUNK_SIZE, start)
                    .await
                {
                    Ok(stream) => {
                        let content_length = end.saturating_sub(start).saturating_add(1);

                        info!(
                            "[ACCESS] GET {} - Status: 206, Range: bytes={}-{}",
                            access_tag, start, end
                        );
                        let mut response = HttpResponse::PartialContent();
                        response.insert_header(("Content-Type", audio_content_type(filename)));
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
                        if let Some(value) = disposition.as_deref() {
                            response.insert_header(("Content-Disposition", value));
                        }
                        return response.streaming(stream.map_err(actix_web::Error::from));
                    }
                    Err(e) => {
                        warn!("Could not seek in file: {} - Error: {}", file_path, e);
                        info!("[ACCESS] GET {} - Status: 404", access_tag);
                        return HttpResponse::NotFound().body("File not found");
                    }
                }
            }
        }
    }

    // Non-range request or no file size available
    match file_reader.read_stream(file_path, CHUNK_SIZE).await {
        Ok(stream) => {
            debug!("Streaming music file: {}", filename);

            info!("[ACCESS] GET {} - Status: 200", access_tag);
            let mut response = HttpResponse::Ok();
            response.insert_header(("Content-Type", audio_content_type(filename)));
            response.insert_header(("Accept-Ranges", "bytes"));
            response.insert_header(("Cache-Control", "public, max-age=86400, must-revalidate"));

            // Add Content-Length if available (helps browser determine duration)
            if let Some(size) = file_size {
                response.insert_header(("Content-Length", size.to_string()));
            }
            if let Some(value) = disposition.as_deref() {
                response.insert_header(("Content-Disposition", value));
            }

            response.streaming(stream.map_err(actix_web::Error::from))
        }
        Err(e) => {
            warn!(
                "File not found or could not be read: {} - Error: {}",
                file_path, e
            );
            info!("[ACCESS] GET {} - Status: 404", access_tag);
            HttpResponse::NotFound().body("File not found")
        }
    }
}

/// Query parameters for path-based streaming (the "open as default app" flow).
///
/// `p` is the URL-encoded absolute filesystem path. The optional `position`,
/// `t`, and `duration` fields mirror [`MusicQueryParams`] for seek support.
/// `download` is intentionally absent — downloads are always DB-id based.
#[derive(Deserialize)]
struct PathQueryParams {
    p: String,
    position: Option<f64>,
    t: Option<f64>,
    duration: Option<f64>,
}

/// Stream an arbitrary filesystem audio file by absolute path.
///
/// Used by the "open file as default app" flow when the OS launches Kaulan with
/// a file the user double-clicked in their file manager. The path is not
/// required to be in the `music` table — it streams directly via the `StdFs`
/// source on desktop, or the `AndroidMediaStoreContent` source for Android
/// `content://` URIs.
///
/// # Security
///
/// The endpoint is gated by an extension whitelist ([`SUPPORTED_EXTENSIONS`])
/// and rejects `content://` URIs on desktop. Without the extension guard, any
/// local process could read arbitrary files via `?p=/etc/passwd`. With it, the
/// surface is limited to audio files the user could already open from their
/// file manager.
///
/// On Android, `content://` URIs are accepted because that's what the launch
/// intent carries when the user taps an audio file from a file manager or the
/// MediaStore. The OS already validated the MIME type via the intent-filter, so
/// the extension whitelist would just reject every MediaStore URI (their last
/// path segment is a numeric id, not a filename). The `file_ops` layer
/// dispatches `content://` to the MediaStore reader, which enforces Android's
/// own URI permission grant — Kaulan can only read URIs it received via an
/// Intent with `FLAG_GRANT_READ_URI_PERMISSION`.
///
/// # Query Parameters
/// * `p` (required) — URL-encoded absolute filesystem path, or `content://`
///   URI on Android
/// * `position`, `t`, `duration` — optional seek params (same as
///   `/api/music/id/{id}`)
///
/// See `docs/default-music-app.md` for the full launch flow.
#[get("/api/music/path")]
pub async fn get_music_by_path(
    query: web::Query<PathQueryParams>,
    req: HttpRequest,
) -> impl Responder {
    if let Some(reject) = crate::handlers::local_guard::reject_non_local_peer(&req) {
        return reject;
    }

    let PathQueryParams {
        p,
        position,
        t,
        duration,
    } = query.into_inner();

    // Log path length rather than the path itself — a launch file may sit under
    // a user directory whose full path is sensitive.
    info!(
        "[ACCESS] GET /api/music/path - Started (p=<{} bytes>)",
        p.len()
    );

    // Android: accept content:// URIs from the launch intent. The OS validated
    // the MIME type; skip the extension whitelist (content URIs don't carry a
    // filename in the last path segment).
    #[cfg(target_os = "android")]
    if p.starts_with("content://") {
        let music_query = MusicQueryParams {
            position,
            t,
            duration,
            download: None,
        };
        // Filename only affects the Content-Type sniff and Content-Disposition
        // header; pass "audio" so audio_content_type defaults to audio/mpeg.
        return build_audio_stream_response(&p, "audio", &music_query, &req, "/api/music/path")
            .await;
    }

    if p.starts_with("content://") {
        info!("[ACCESS] GET /api/music/path - Status: 400");
        return HttpResponse::BadRequest().body("content:// URIs not supported");
    }

    let ext = Path::new(&p)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let Some(ext) = ext else {
        info!("[ACCESS] GET /api/music/path - Status: 400");
        return HttpResponse::BadRequest().body("File has no extension");
    };
    if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
        info!("[ACCESS] GET /api/music/path - Status: 400");
        return HttpResponse::BadRequest().body("Unsupported file type");
    }

    let filename = Path::new(&p)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio");

    let music_query = MusicQueryParams {
        position,
        t,
        duration,
        download: None,
    };

    build_audio_stream_response(&p, filename, &music_query, &req, "/api/music/path").await
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

/// Interpret the `download` query value leniently: presence (and any value
/// other than an explicit off-token) counts as a download request. Accepts
/// `1`/`true`/`yes` and a bare flag, while `0`/`false`/`no` and absence do not.
fn is_download_requested(value: Option<&str>) -> bool {
    match value.map(str::trim) {
        Some("0" | "false" | "no") => false,
        Some(_) => true,
        None => false,
    }
}

/// Build an RFC 6266 `Content-Disposition: attachment` value for `filename`
/// when a download is requested.
///
/// Returns `None` for normal playback so the header is omitted entirely and
/// streaming behavior is unchanged. Both the legacy `filename="..."` form
/// (ASCII-safe, via `sanitize_filename`) and the `filename*=UTF-8''...` form
/// (percent-encoded) are emitted so non-ASCII names render correctly across
/// browsers.
///
/// Related documentation: `docs/library-import.md`
fn download_disposition(download: bool, filename: &str) -> Option<String> {
    if !download {
        return None;
    }
    let ascii = ascii_filename(filename);
    let encoded = percent_encode_filename(filename);
    Some(format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        ascii, encoded
    ))
}

/// ASCII-only fallback for the legacy `filename="..."` token (RFC 6266 keeps
/// that quoted form to visible ASCII). The real name, including any non-ASCII
/// characters, travels in the percent-encoded `filename*=` token instead, so
/// the whole header value stays ASCII and parseable end-to-end.
fn ascii_filename(filename: &str) -> String {
    let sanitized = sanitize_filename(filename);
    let mut out = String::with_capacity(sanitized.len());
    for ch in sanitized.chars() {
        if ch.is_ascii() && !ch.is_ascii_control() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.trim_matches('_').is_empty() {
        "download".to_string()
    } else {
        out
    }
}

/// Percent-encode a filename for the `filename*=` token per RFC 5987.
///
/// Unreserved characters (RFC 3986) are left as-is; every other byte is emitted
/// as `%HH`. The browser's download manager then streams the body straight to
/// disk rather than buffering it in page memory.
fn percent_encode_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for byte in name.bytes() {
        if is_unreserved_byte(byte) {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(hex_digit(byte / 16));
            out.push(hex_digit(byte % 16));
        }
    }
    out
}

fn is_unreserved_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

/// Map a single nibble (0..=15) to an uppercase hex digit.
fn hex_digit(nibble: u8) -> char {
    match nibble {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        9 => '9',
        10 => 'A',
        11 => 'B',
        12 => 'C',
        13 => 'D',
        14 => 'E',
        _ => 'F',
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

/// Query parameters for path-based cover art (the "open as default app" flow).
///
/// Mirrors [`PathQueryParams`] but only needs the path itself — cover extraction
/// has no seek/download semantics.
#[derive(Deserialize)]
struct CoverPathQuery {
    p: String,
}

/// Extract embedded cover art for an arbitrary filesystem audio path.
///
/// Used by the "open file as default app" flow so the click-open player shows
/// the same cover art as regular playlist playback. Mirrors the security
/// gating of [`get_music_by_path`]: extension whitelist on desktop, accept
/// `content://` URIs on Android (where the OS already validated the MIME type
/// via the launch intent).
///
/// # Query Parameters
/// * `p` (required) — URL-encoded absolute filesystem path, or `content://`
///   URI on Android
///
/// # Returns
/// - Image data with appropriate `Content-Type` if cover art is embedded
/// - `400 Bad Request` when `p` is `content://` (desktop), has no extension,
///   or has a non-audio extension
/// - `404 Not Found` when the file is missing or has no embedded cover art
/// - `500 Internal Server Error` on FFmpeg/task errors
///
/// See `docs/default-music-app.md` for the full launch flow.
/// Shared cover-art extraction pipeline.
///
/// Materializes the source (no-op for filesystem paths, temp-file streaming
/// for Android `content://` URIs) via [`crate::ffmpeg::prepare_input`], then
/// runs the FFmpeg probe on a blocking thread to avoid stalling the async
/// runtime. Both [`get_music_cover`] (DB-id lookup) and [`get_music_cover_by_path`]
/// (launch-handoff path) go through this so any fix to content:// handling
/// lives in one place.
async fn extract_cover_response(file_path: &str, access_tag: &str) -> HttpResponse {
    info!("[ACCESS] GET {} - Started", access_tag);

    match crate::ffmpeg::prepare_input(file_path).await {
        Ok(prepared_input) => {
            let result = tokio::task::spawn_blocking(move || {
                let prepared = prepared_input;
                crate::ffmpeg::extract_cover_art(prepared.path())
            })
            .await;

            match result {
                Ok(Ok(Some((content_type, data)))) => {
                    info!(
                        "[ACCESS] GET {} - Status: 200, {} bytes",
                        access_tag,
                        data.len()
                    );
                    HttpResponse::Ok()
                        .insert_header(("Content-Type", content_type))
                        .insert_header(("Cache-Control", "public, max-age=86400, must-revalidate"))
                        .body(data)
                }
                Ok(Ok(None)) => {
                    debug!("No cover art found for {}", file_path);
                    info!("[ACCESS] GET {} - Status: 404", access_tag);
                    HttpResponse::NotFound().body("No cover art")
                }
                Ok(Err(e)) => {
                    warn!("Failed to extract cover art for {}: {}", file_path, e);
                    info!("[ACCESS] GET {} - Status: 404", access_tag);
                    HttpResponse::NotFound().body("Could not extract cover art")
                }
                Err(e) => {
                    error!("Task join error for cover art {}: {}", file_path, e);
                    HttpResponse::InternalServerError().body("Internal error")
                }
            }
        }
        Err(e) => {
            warn!("Could not prepare file for cover art {}: {}", file_path, e);
            info!("[ACCESS] GET {} - Status: 404", access_tag);
            HttpResponse::NotFound().body("File not found")
        }
    }
}

#[get("/api/music/path/cover")]
pub async fn get_music_cover_by_path(
    query: web::Query<CoverPathQuery>,
    req: HttpRequest,
) -> impl Responder {
    if let Some(reject) = crate::handlers::local_guard::reject_non_local_peer(&req) {
        return reject;
    }

    let CoverPathQuery { p } = query.into_inner();

    #[cfg(target_os = "android")]
    if p.starts_with("content://") {
        return extract_cover_response(&p, "/api/music/path/cover").await;
    }

    if p.starts_with("content://") {
        return HttpResponse::BadRequest().body("content:// URIs not supported");
    }

    let ext = Path::new(&p)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let Some(ext) = ext else {
        return HttpResponse::BadRequest().body("File has no extension");
    };
    if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
        return HttpResponse::BadRequest().body("Unsupported file type");
    }

    extract_cover_response(&p, "/api/music/path/cover").await
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
    let access_tag = format!("/api/music/id/{}/cover", id);

    match MusicEntity::find_by_id(id).one(&data.db_conn).await {
        Ok(Some(music)) => extract_cover_response(&music.file_path, &access_tag).await,
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

#[cfg(test)]
mod tests {
    use super::{
        download_disposition, get_music_by_id, get_music_by_path, get_music_cover_by_path,
        percent_encode_filename,
    };
    use crate::entities::music::Entity as MusicEntity;
    use crate::types::AppState;
    use actix_web::body::to_bytes;
    use actix_web::{test as actix_test, web, App};
    use sea_orm::EntityTrait;
    use std::sync::Arc;

    /// Build an AppState over a temp music dir and scan it so the seeded file
    /// is in the database. Returns the app state and the id of the first song.
    async fn make_app_state(music_dir: &std::path::Path) -> (web::Data<AppState>, i32) {
        let db_conn = crate::database::establish_connection(music_dir.to_str().unwrap())
            .await
            .unwrap();
        crate::file_ops::clear_scan_backends();
        crate::file_ops::register_scan_backend(std::sync::Arc::new(
            crate::file_ops::StdFsScanBackend::new(std::path::PathBuf::from(music_dir)),
        ));
        crate::services::scanner::initialize_database(&db_conn)
            .await
            .unwrap();
        let music = MusicEntity::find().one(&db_conn).await.unwrap().unwrap();
        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_dir.to_string_lossy().to_string()),
            download_root: Arc::new(music_dir.to_string_lossy().to_string()),
            preview_root: Arc::new(music_dir.join(".preview").to_string_lossy().to_string()),
            db_conn,
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });
        (app_state, music.id)
    }

    fn disposition_header(resp: &actix_web::dev::ServiceResponse) -> Option<String> {
        resp.headers()
            .get("content-disposition")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
    }

    #[test]
    fn disposition_is_none_unless_download_requested() {
        assert!(download_disposition(false, "song.mp3").is_none());
    }

    #[test]
    fn disposition_emits_attachment_with_both_filename_forms() {
        let value = download_disposition(true, "Song.mp3").unwrap();
        assert_eq!(
            value,
            "attachment; filename=\"Song.mp3\"; filename*=UTF-8''Song.mp3"
        );
    }

    #[test]
    fn disposition_keeps_legacy_token_ascii_for_non_ascii_names() {
        let value = download_disposition(true, "歌曲.mp3").unwrap();
        // Legacy quoted token is ASCII-only; the real name lives in filename*.
        assert!(value.contains("filename=\"__.mp3\""), "{value}");
        assert!(
            value.contains("filename*=UTF-8''%E6%AD%8C%E6%9B%B2.mp3"),
            "{value}"
        );
    }

    #[test]
    fn percent_encode_keeps_unreserved_and_encodes_cjk() {
        // Unreserved set is left untouched.
        assert_eq!(percent_encode_filename("a-1_.mp3~"), "a-1_.mp3~");
        // 歌 = U+6B4C -> E6 AD 8C, 曲 = U+66F2 -> E6 9B B2.
        assert_eq!(percent_encode_filename("歌曲"), "%E6%AD%8C%E6%9B%B2");
    }

    #[actix_web::test]
    async fn download_flag_returns_attachment_header() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test-song.mp3"), b"FAKE_AUDIO").unwrap();
        let (app_state, id) = make_app_state(temp.path()).await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(get_music_by_id)).await;
        let req = actix_test::TestRequest::get()
            .uri(&format!("/api/music/id/{id}?download=1"))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 200);
        let disposition = disposition_header(&resp).expect("content-disposition header present");
        assert!(disposition.starts_with("attachment"), "{disposition}");
        assert!(
            disposition.contains("filename=\"test-song.mp3\""),
            "{disposition}"
        );
    }

    #[actix_web::test]
    async fn no_download_flag_has_no_disposition_header() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test-song.mp3"), b"FAKE_AUDIO").unwrap();
        let (app_state, id) = make_app_state(temp.path()).await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(get_music_by_id)).await;
        let req = actix_test::TestRequest::get()
            .uri(&format!("/api/music/id/{id}"))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 200);
        assert!(disposition_header(&resp).is_none());
    }

    #[actix_web::test]
    async fn download_false_flag_has_no_disposition_header() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test-song.mp3"), b"FAKE_AUDIO").unwrap();
        let (app_state, id) = make_app_state(temp.path()).await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(get_music_by_id)).await;
        let req = actix_test::TestRequest::get()
            .uri(&format!("/api/music/id/{id}?download=0"))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 200);
        assert!(disposition_header(&resp).is_none());
    }

    #[actix_web::test]
    async fn download_flag_with_non_ascii_filename_emits_filename_star() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("歌曲.mp3"), b"FAKE_AUDIO").unwrap();
        let (app_state, id) = make_app_state(temp.path()).await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(get_music_by_id)).await;
        let req = actix_test::TestRequest::get()
            .uri(&format!("/api/music/id/{id}?download=1"))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 200);
        let disposition = disposition_header(&resp).expect("content-disposition header present");
        // The percent-encoded UTF-8 form must carry the original CJK bytes, and
        // the legacy quoted token is an ASCII fallback.
        assert!(
            disposition.contains("filename*=UTF-8''%E6%AD%8C%E6%9B%B2.mp3"),
            "{disposition}"
        );
    }

    #[actix_web::test]
    async fn download_flag_with_range_returns_206_with_disposition() {
        let temp = tempfile::tempdir().unwrap();
        // Larger than the requested range so bytes=0-99 is valid.
        std::fs::write(temp.path().join("test-song.mp3"), vec![b'X'; 200]).unwrap();
        let (app_state, id) = make_app_state(temp.path()).await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(get_music_by_id)).await;
        let req = actix_test::TestRequest::get()
            .uri(&format!("/api/music/id/{id}?download=1"))
            .insert_header(("Range", "bytes=0-99"))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 206);
        assert!(resp.headers().contains_key("content-range"));
        let disposition = disposition_header(&resp).expect("content-disposition header present");
        assert!(disposition.starts_with("attachment"), "{disposition}");
    }

    // --- get_music_by_path tests (open-as-default-app flow) ---

    fn url_encoded_path_query(path: &str) -> String {
        format!("/api/music/path?p={}", percent_encode_filename(path))
    }

    /// Loopback peer address for tests of endpoints guarded by
    /// [`reject_non_local_peer`]. `TestRequest` defaults `peer_addr` to `None`,
    /// which the guard treats as non-local — preset this so the guard passes.
    fn local_peer() -> std::net::SocketAddr {
        "127.0.0.1:0".parse().unwrap()
    }

    #[actix_web::test]
    async fn music_by_path_streams_known_audio_extension() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("external.mp3");
        std::fs::write(&file_path, b"FAKE_AUDIO_BYTES").unwrap();
        let app = actix_test::init_service(App::new().service(get_music_by_path)).await;

        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri(&url_encoded_path_query(file_path.to_str().unwrap()))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "audio/mpeg"
        );
        let bytes = to_bytes(resp.into_body()).await.unwrap();
        assert_eq!(bytes.as_ref(), b"FAKE_AUDIO_BYTES");
    }

    #[actix_web::test]
    async fn music_by_path_honors_range_header() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("ranged.flac");
        std::fs::write(&file_path, vec![b'A'; 200]).unwrap();
        let app = actix_test::init_service(App::new().service(get_music_by_path)).await;

        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri(&url_encoded_path_query(file_path.to_str().unwrap()))
            .insert_header(("Range", "bytes=10-19"))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 206);
        // Content-Range + Content-Length advertise the slice; the body stream
        // itself starts at byte 10 and the browser truncates to Content-Length
        // (existing /api/music/id/{id} Range behavior — see download_flag_with_range).
        assert!(resp.headers().contains_key("content-range"));
        assert_eq!(
            resp.headers()
                .get("content-length")
                .unwrap()
                .to_str()
                .unwrap(),
            "10"
        );
        assert_eq!(
            resp.headers()
                .get("content-range")
                .unwrap()
                .to_str()
                .unwrap(),
            "bytes 10-19/200"
        );
    }

    #[actix_web::test]
    async fn music_by_path_rejects_non_audio_extension() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("secrets.txt");
        std::fs::write(&file_path, b"root:x:0:0:").unwrap();
        let app = actix_test::init_service(App::new().service(get_music_by_path)).await;

        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri(&url_encoded_path_query(file_path.to_str().unwrap()))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn music_by_path_rejects_content_uri() {
        let app = actix_test::init_service(App::new().service(get_music_by_path)).await;
        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri("/api/music/path?p=content%3A%2F%2Fmedia%2Fsong.mp3")
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn music_by_path_rejects_missing_extension() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("noext");
        std::fs::write(&file_path, b"x").unwrap();
        let app = actix_test::init_service(App::new().service(get_music_by_path)).await;

        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri(&url_encoded_path_query(file_path.to_str().unwrap()))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    // --- get_music_cover_by_path tests (open-as-default-app flow) ---

    fn url_encoded_cover_query(path: &str) -> String {
        format!("/api/music/path/cover?p={}", percent_encode_filename(path))
    }

    #[actix_web::test]
    async fn cover_by_path_rejects_non_audio_extension() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("secrets.txt");
        std::fs::write(&file_path, b"root:x:0:0:").unwrap();
        let app = actix_test::init_service(App::new().service(get_music_cover_by_path)).await;

        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri(&url_encoded_cover_query(file_path.to_str().unwrap()))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn cover_by_path_rejects_missing_extension() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("noext");
        std::fs::write(&file_path, b"x").unwrap();
        let app = actix_test::init_service(App::new().service(get_music_cover_by_path)).await;

        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri(&url_encoded_cover_query(file_path.to_str().unwrap()))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn cover_by_path_rejects_content_uri() {
        let app = actix_test::init_service(App::new().service(get_music_cover_by_path)).await;
        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri("/api/music/path/cover?p=content%3A%2F%2Fmedia%2Fsong.mp3")
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn cover_by_path_returns_404_when_file_missing() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("missing.mp3");
        let app = actix_test::init_service(App::new().service(get_music_cover_by_path)).await;

        let req = actix_test::TestRequest::get()
            .peer_addr(local_peer())
            .uri(&url_encoded_cover_query(file_path.to_str().unwrap()))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 404);
    }
}
