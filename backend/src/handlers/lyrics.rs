//! Lyrics API handlers.
//!
//! This module provides endpoints for:
//! - Streaming sidecar lyric files (`.lrc` preferred, `.vtt` fallback) synchronized with music playback
//! - Updating existing writable sidecar lyric files after frontend timing edits
//!
//! Related documentation:
//! - `docs/lyrics-display.md`
//! - `docs/lyric-editing.md`

use crate::entities::music::{Column as MusicColumn, Entity as MusicEntity};
use crate::file_ops::{
    get_lyric_reader, lyric_candidate_paths, resolve_path, source_exists, source_write_file,
    PathKind, SUPPORTED_EXTENSIONS,
};
use crate::types::AppState;
use actix_web::{get, put, web, HttpResponse, Responder};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use std::path::Path;
use tracing::{debug, error, info, warn};

/// Stream lyrics file by music filename.
///
/// This endpoint looks up the music file in the database by filename,
/// constructs the corresponding sidecar lyric file path, and streams the lyric content.
///
/// Sidecar lyric files should have the same base name as the audio file:
/// - `song.mp3` → `song.lrc`
/// - `song.mp3` → `song.vtt`
/// - `album/track.flac` → `album/track.lrc`
///
/// # Path Parameters
/// * `filename` - The filename to look up in the database (e.g., `song.mp3`)
///
/// # Returns
/// - Sidecar lyric file content with `text/plain; charset=utf-8` content type if found
/// - `404 Not Found` if music not in database or no supported sidecar lyric file exists
/// - `500 Internal Server Error` for database errors
///
/// # Example
/// ```bash
/// curl http://localhost:2080/api/lyrics/song.mp3
/// ```
#[get("/api/lyrics/{filename}")]
pub async fn get_lyrics(path: web::Path<String>, data: web::Data<AppState>) -> impl Responder {
    let filename = path.into_inner();
    debug!("Lyrics request received for filename: {}", filename);

    // Simple access log
    info!("[ACCESS] GET /api/lyrics/{} - Started", filename);

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

            let lyric_reader = get_lyric_reader();

            match lyric_reader
                .read_lyric(&music.file_path, &music.filename)
                .await
            {
                Ok(Some(content)) => {
                    debug!(
                        "Successfully served lyrics for {}: {} bytes",
                        music.filename,
                        content.len()
                    );
                    info!("[ACCESS] GET /api/lyrics/{} - Status: 200", filename);
                    let mut response = HttpResponse::Ok();
                    response.insert_header(("Content-Type", "text/plain; charset=utf-8"));
                    response.insert_header((
                        "Cache-Control",
                        "no-store, no-cache, must-revalidate, max-age=0",
                    ));
                    response.insert_header(("Pragma", "no-cache"));
                    response.insert_header(("Expires", "0"));
                    response.body(content)
                }
                Ok(None) => {
                    debug!(
                        "Lyrics not found for {} (this is expected for songs without lyrics)",
                        music.filename
                    );
                    info!("[ACCESS] GET /api/lyrics/{} - Status: 404", filename);
                    HttpResponse::NotFound().body("Lyrics not found")
                }
                Err(e) => {
                    error!("Error reading lyrics for {}: {}", music.filename, e);
                    info!("[ACCESS] GET /api/lyrics/{} - Status: 500", filename);
                    HttpResponse::InternalServerError().body("Error reading lyrics")
                }
            }
        }
        Ok(None) => {
            warn!("Music not found in database: {}", filename);
            info!("[ACCESS] GET /api/lyrics/{} - Status: 404", filename);
            HttpResponse::NotFound().body("Music not found")
        }
        Err(e) => {
            error!("Database error while fetching music {}: {}", filename, e);
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}

/// Query parameters for path-based lyric lookup (the "open as default app" flow).
///
/// `p` is the URL-encoded absolute filesystem path of the **audio** file. The
/// handler derives the sidecar lyric candidate paths from it (`.lrc` then
/// `.vtt`) — same derivation rule as the DB-backed endpoints above.
#[derive(Deserialize)]
struct LyricsPathQuery {
    p: String,
}

/// Stream sidecar lyrics for an arbitrary filesystem audio path.
///
/// Used by the "open file as default app" flow when the OS launches Kaulan with
/// a file the user double-clicked in their file manager. The audio path is not
/// required to be in the `music` table — the sidecar lookup goes straight
/// through [`get_lyric_reader`] against the filesystem.
///
/// # Security
///
/// Gated by the same extension whitelist ([`SUPPORTED_EXTENSIONS`]) and
/// `content://` rejection as [`crate::handlers::music::get_music_by_path`].
/// Without the extension guard, any local process could probe for arbitrary
/// sidecar files via `?p=/etc/passwd`. With it, the surface mirrors what
/// `/api/music/path` already exposes.
///
/// # Query Parameters
/// * `p` (required) — URL-encoded absolute filesystem path of the audio file
///
/// # Returns
/// - `200 OK` with `text/plain; charset=utf-8` body when a `.lrc`/`.vtt` sidecar exists
/// - `400 Bad Request` when `p` is a `content://` URI, has no extension, or has a non-audio extension
/// - `404 Not Found` when the audio file has no sidecar lyric
/// - `500 Internal Server Error` on filesystem errors
///
/// See `docs/default-music-app.md` for the full launch flow.
#[get("/api/lyrics/path")]
pub async fn get_lyrics_by_path(query: web::Query<LyricsPathQuery>) -> impl Responder {
    let LyricsPathQuery { p } = query.into_inner();

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

    let filename = Path::new(&p)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio");

    info!(
        "[ACCESS] GET /api/lyrics/path - Started (p=<{} bytes>)",
        p.len()
    );

    let lyric_reader = get_lyric_reader();
    match lyric_reader.read_lyric(&p, filename).await {
        Ok(Some(content)) => {
            info!(
                "[ACCESS] GET /api/lyrics/path - Status: 200, {} bytes",
                content.len()
            );
            let mut response = HttpResponse::Ok();
            response.insert_header(("Content-Type", "text/plain; charset=utf-8"));
            response.insert_header((
                "Cache-Control",
                "no-store, no-cache, must-revalidate, max-age=0",
            ));
            response.insert_header(("Pragma", "no-cache"));
            response.insert_header(("Expires", "0"));
            response.body(content)
        }
        Ok(None) => {
            debug!("No sidecar lyrics found for {}", p);
            info!("[ACCESS] GET /api/lyrics/path - Status: 404");
            HttpResponse::NotFound().body("Lyrics not found")
        }
        Err(e) => {
            error!("Error reading lyrics for {}: {}", p, e);
            info!("[ACCESS] GET /api/lyrics/path - Status: 500");
            HttpResponse::InternalServerError().body("Error reading lyrics")
        }
    }
}

/// Stream lyrics file by music ID.
///
/// This endpoint looks up the music file in the database by ID,
/// constructs the corresponding sidecar lyric file path, and streams the lyric content.
///
/// Sidecar lyric files should have the same base name as the audio file:
/// - `song.mp3` → `song.lrc`
/// - `song.mp3` → `song.vtt`
/// - `album/track.flac` → `album/track.lrc`
///
/// # Path Parameters
/// * `id` - The music ID to look up in the database
///
/// # Returns
/// - Sidecar lyric file content with `text/plain; charset=utf-8` content type if found
/// - `404 Not Found` if music not in database or no supported sidecar lyric file exists
/// - `500 Internal Server Error` for database errors
///
/// # Example
/// ```bash
/// curl http://localhost:2080/api/lyrics/id/1
/// ```
#[get("/api/lyrics/id/{id}")]
pub async fn get_lyrics_by_id(path: web::Path<i32>, data: web::Data<AppState>) -> impl Responder {
    let id = path.into_inner();
    debug!("Lyrics request received for ID: {}", id);

    // Simple access log
    info!("[ACCESS] GET /api/lyrics/id/{} - Started", id);

    match MusicEntity::find_by_id(id).one(&data.db_conn).await {
        Ok(Some(music)) => {
            debug!(
                "Found music in database: id={}, filename={}, file_path={}",
                music.id, music.filename, music.file_path
            );

            let lyric_reader = get_lyric_reader();

            match lyric_reader
                .read_lyric(&music.file_path, &music.filename)
                .await
            {
                Ok(Some(content)) => {
                    debug!(
                        "Successfully served lyrics for {}: {} bytes",
                        music.filename,
                        content.len()
                    );
                    info!("[ACCESS] GET /api/lyrics/id/{} - Status: 200", id);
                    let mut response = HttpResponse::Ok();
                    response.insert_header(("Content-Type", "text/plain; charset=utf-8"));
                    response.insert_header((
                        "Cache-Control",
                        "no-store, no-cache, must-revalidate, max-age=0",
                    ));
                    response.insert_header(("Pragma", "no-cache"));
                    response.insert_header(("Expires", "0"));
                    response.body(content)
                }
                Ok(None) => {
                    debug!(
                        "Lyrics not found for {} (this is expected for songs without lyrics)",
                        music.filename
                    );
                    info!("[ACCESS] GET /api/lyrics/id/{} - Status: 404", id);
                    HttpResponse::NotFound().body("Lyrics not found")
                }
                Err(e) => {
                    error!("Error reading lyrics for {}: {}", music.filename, e);
                    info!("[ACCESS] GET /api/lyrics/id/{} - Status: 500", id);
                    HttpResponse::InternalServerError().body("Error reading lyrics")
                }
            }
        }
        Ok(None) => {
            warn!("Music not found in database: ID {}", id);
            info!("[ACCESS] GET /api/lyrics/id/{} - Status: 404", id);
            HttpResponse::NotFound().body("Music not found")
        }
        Err(e) => {
            error!("Database error while fetching music ID {}: {}", id, e);
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}

/// Update an existing writable lyric sidecar file by music ID.
///
/// The backend derives candidate `.lrc` and `.vtt` paths from the database-stored
/// music path, then writes only when the resolved source is standard filesystem.
/// MediaStore and other non-writable sources return `409 Conflict`.
///
/// # Path Parameters
/// * `id` - The music ID to look up in the database
///
/// # Request Body
/// JSON object with a non-empty `content` string containing the complete lyric file.
///
/// # Returns
/// - `200 OK` when the existing lyric file is updated
/// - `400 Bad Request` when `content` is empty or whitespace only
/// - `404 Not Found` when music or lyric sidecar file does not exist
/// - `409 Conflict` when the source is not writable
/// - `500 Internal Server Error` for database, path, or write errors
#[put("/api/lyrics/id/{id}")]
pub async fn update_lyrics_by_id(
    path: web::Path<i32>,
    body: web::Json<crate::types::UpdateLyricContentRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    let id = path.into_inner();
    debug!("Lyrics update request received for ID: {}", id);
    info!("[ACCESS] PUT /api/lyrics/id/{} - Started", id);

    let content = body.content.clone();
    if content.trim().is_empty() {
        info!("[ACCESS] PUT /api/lyrics/id/{} - Status: 400", id);
        return HttpResponse::BadRequest().json(crate::types::UpdateLyricContentResponse {
            success: false,
            message: "Lyric content cannot be empty".to_string(),
            lyric_filename: None,
        });
    }

    match MusicEntity::find_by_id(id).one(&data.db_conn).await {
        Ok(Some(music)) => {
            let resolved = match resolve_path(&music.file_path) {
                Ok(resolved) => resolved,
                Err(err) => {
                    error!(
                        "Failed to resolve lyric source for {}: {}",
                        music.file_path, err
                    );
                    info!("[ACCESS] PUT /api/lyrics/id/{} - Status: 500", id);
                    return HttpResponse::InternalServerError().json(
                        crate::types::UpdateLyricContentResponse {
                            success: false,
                            message: "Failed to resolve lyric source".to_string(),
                            lyric_filename: None,
                        },
                    );
                }
            };

            if resolved.path_kind != PathKind::StdFs {
                info!("[ACCESS] PUT /api/lyrics/id/{} - Status: 409", id);
                return HttpResponse::Conflict().json(crate::types::UpdateLyricContentResponse {
                    success: false,
                    message: "Lyric source is not writable".to_string(),
                    lyric_filename: None,
                });
            }

            let lyric_path = match resolve_existing_lyric_path(&music.file_path).await {
                Ok(Some(path)) => path,
                Ok(None) => {
                    info!("[ACCESS] PUT /api/lyrics/id/{} - Status: 404", id);
                    return HttpResponse::NotFound().json(
                        crate::types::UpdateLyricContentResponse {
                            success: false,
                            message: "Lyrics not found".to_string(),
                            lyric_filename: None,
                        },
                    );
                }
                Err(err) => {
                    error!(
                        "Failed to resolve writable lyric path for {}: {}",
                        music.file_path, err
                    );
                    info!("[ACCESS] PUT /api/lyrics/id/{} - Status: 500", id);
                    return HttpResponse::InternalServerError().json(
                        crate::types::UpdateLyricContentResponse {
                            success: false,
                            message: "Failed to resolve lyric file".to_string(),
                            lyric_filename: None,
                        },
                    );
                }
            };

            if let Err(err) = source_write_file(&lyric_path, content.as_bytes()).await {
                error!("Failed to write lyric file {}: {}", lyric_path, err);
                info!("[ACCESS] PUT /api/lyrics/id/{} - Status: 500", id);
                return HttpResponse::InternalServerError().json(
                    crate::types::UpdateLyricContentResponse {
                        success: false,
                        message: "Failed to save lyric file".to_string(),
                        lyric_filename: None,
                    },
                );
            }

            info!("[ACCESS] PUT /api/lyrics/id/{} - Status: 200", id);
            HttpResponse::Ok().json(crate::types::UpdateLyricContentResponse {
                success: true,
                message: "Lyrics updated".to_string(),
                lyric_filename: Path::new(&lyric_path)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string()),
            })
        }
        Ok(None) => {
            warn!("Music not found in database: ID {}", id);
            info!("[ACCESS] PUT /api/lyrics/id/{} - Status: 404", id);
            HttpResponse::NotFound().json(crate::types::UpdateLyricContentResponse {
                success: false,
                message: "Music not found".to_string(),
                lyric_filename: None,
            })
        }
        Err(err) => {
            error!("Database error while updating music ID {}: {}", id, err);
            info!("[ACCESS] PUT /api/lyrics/id/{} - Status: 500", id);
            HttpResponse::InternalServerError().json(crate::types::UpdateLyricContentResponse {
                success: false,
                message: "Database error".to_string(),
                lyric_filename: None,
            })
        }
    }
}

async fn resolve_existing_lyric_path(file_path: &str) -> Result<Option<String>, std::io::Error> {
    for lyric_path in lyric_candidate_paths(file_path) {
        if source_exists(&lyric_path).await? {
            return Ok(Some(lyric_path));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use std::sync::Arc;

    /// Helper function to create a test directory with music and lyrics files
    async fn create_test_setup() -> (tempfile::TempDir, web::Data<AppState>) {
        let temp_dir = tempfile::tempdir().unwrap();
        let music_dir = temp_dir.path();

        // Create test audio file
        let audio_path = music_dir.join("test-song.mp3");
        std::fs::write(&audio_path, b"fake audio content").unwrap();

        // Create test lyrics file
        let lyrics_path = music_dir.join("test-song.lrc");
        std::fs::write(
            &lyrics_path,
            "[00:00.54]First lyric line\n[00:02.52]Second lyric line\n[00:05.00]Third lyric line",
        )
        .unwrap();

        // Setup database connection
        let db_conn = crate::database::establish_connection(music_dir.to_str().unwrap())
            .await
            .unwrap();

        // Initialize database to scan the music file
        crate::services::scanner::initialize_database(music_dir.to_str().unwrap(), &db_conn)
            .await
            .unwrap();

        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_dir.to_str().unwrap().to_string()),
            download_root: Arc::new(music_dir.to_str().unwrap().to_string()),
            preview_root: Arc::new(music_dir.join(".preview").to_string_lossy().to_string()),
            db_conn,
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        (temp_dir, app_state)
    }

    #[actix_web::test]
    async fn test_get_lyrics_with_lrc_file() {
        let (_temp_dir, app_state) = create_test_setup().await;

        let app = test::init_service(App::new().app_data(app_state).service(get_lyrics)).await;

        let req = test::TestRequest::get()
            .uri("/api/lyrics/test-song.mp3")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        assert_eq!(resp.status().as_u16(), 200);

        // Check content type
        let content_type = resp.headers().get("Content-Type").unwrap();
        assert!(content_type.to_str().unwrap().contains("text/plain"));

        // Check body content
        let body = test::read_body(resp).await;
        let content = String::from_utf8(body.to_vec()).unwrap();
        assert!(content.contains("[00:00.54]First lyric line"));
        assert!(content.contains("[00:02.52]Second lyric line"));
    }

    #[actix_web::test]
    async fn test_get_lyrics_missing_lrc_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let music_dir = temp_dir.path();

        // Create test audio file WITHOUT lyrics file
        let audio_path = music_dir.join("no-lyrics.mp3");
        std::fs::write(&audio_path, b"fake audio content").unwrap();

        // Setup database connection
        let db_conn = crate::database::establish_connection(music_dir.to_str().unwrap())
            .await
            .unwrap();

        // Initialize database to scan the music file
        crate::services::scanner::initialize_database(music_dir.to_str().unwrap(), &db_conn)
            .await
            .unwrap();

        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_dir.to_str().unwrap().to_string()),
            download_root: Arc::new(music_dir.to_str().unwrap().to_string()),
            preview_root: Arc::new(music_dir.join(".preview").to_string_lossy().to_string()),
            db_conn,
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        let app = test::init_service(App::new().app_data(app_state).service(get_lyrics)).await;

        let req = test::TestRequest::get()
            .uri("/api/lyrics/no-lyrics.mp3")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 404);
    }

    #[actix_web::test]
    async fn test_get_lyrics_falls_back_to_vtt() {
        let temp_dir = tempfile::tempdir().unwrap();
        let music_dir = temp_dir.path();

        let audio_path = music_dir.join("with-vtt.mp3");
        std::fs::write(&audio_path, b"fake audio content").unwrap();

        let lyrics_path = music_dir.join("with-vtt.vtt");
        std::fs::write(
            &lyrics_path,
            "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nthat's \"my\" line, right?\n",
        )
        .unwrap();

        let db_conn = crate::database::establish_connection(music_dir.to_str().unwrap())
            .await
            .unwrap();

        crate::services::scanner::initialize_database(music_dir.to_str().unwrap(), &db_conn)
            .await
            .unwrap();

        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_dir.to_str().unwrap().to_string()),
            download_root: Arc::new(music_dir.to_str().unwrap().to_string()),
            preview_root: Arc::new(music_dir.join(".preview").to_string_lossy().to_string()),
            db_conn,
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        let app = test::init_service(App::new().app_data(app_state).service(get_lyrics)).await;

        let req = test::TestRequest::get()
            .uri("/api/lyrics/with-vtt.mp3")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        let body = test::read_body(resp).await;
        let content = String::from_utf8(body.to_vec()).unwrap();
        assert!(content.contains("WEBVTT"));
        assert!(content.contains("that's \"my\" line, right?"));
    }

    #[actix_web::test]
    async fn test_get_lyrics_music_not_in_database() {
        let temp_dir = tempfile::tempdir().unwrap();
        let music_dir = temp_dir.path();

        let db_conn = crate::database::establish_connection(music_dir.to_str().unwrap())
            .await
            .unwrap();

        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_dir.to_str().unwrap().to_string()),
            download_root: Arc::new(music_dir.to_str().unwrap().to_string()),
            preview_root: Arc::new(music_dir.join(".preview").to_string_lossy().to_string()),
            db_conn,
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        let app = test::init_service(App::new().app_data(app_state).service(get_lyrics)).await;

        let req = test::TestRequest::get()
            .uri("/api/lyrics/nonexistent.mp3")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 404);
    }

    #[actix_web::test]
    async fn test_get_lyrics_utf8_content() {
        let temp_dir = tempfile::tempdir().unwrap();
        let music_dir = temp_dir.path();

        // Create test audio file
        let audio_path = music_dir.join("japanese.mp3");
        std::fs::write(&audio_path, b"fake audio content").unwrap();

        // Create test lyrics file with UTF-8 content (Japanese and Chinese)
        let lyrics_path = music_dir.join("japanese.lrc");
        std::fs::write(
            &lyrics_path,
            "[00:00.54]欢迎光临。\n[00:02.52]是美容店的店员茉子哦♪\n[00:05.00]ときめきもどぎまぎも",
        )
        .unwrap();

        // Setup database connection
        let db_conn = crate::database::establish_connection(music_dir.to_str().unwrap())
            .await
            .unwrap();

        // Initialize database to scan the music file
        crate::services::scanner::initialize_database(music_dir.to_str().unwrap(), &db_conn)
            .await
            .unwrap();

        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_dir.to_str().unwrap().to_string()),
            download_root: Arc::new(music_dir.to_str().unwrap().to_string()),
            preview_root: Arc::new(music_dir.join(".preview").to_string_lossy().to_string()),
            db_conn,
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        let app = test::init_service(App::new().app_data(app_state).service(get_lyrics)).await;

        let req = test::TestRequest::get()
            .uri("/api/lyrics/japanese.mp3")
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());

        // Check body content for UTF-8 characters
        let body = test::read_body(resp).await;
        let content = String::from_utf8(body.to_vec()).unwrap();
        assert!(content.contains("欢迎光临"));
        assert!(content.contains("ときめきもどぎまぎも"));
    }

    #[actix_web::test]
    async fn test_update_lyrics_by_id_updates_existing_lrc_file() {
        let (_temp_dir, app_state) = create_test_setup().await;
        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(update_lyrics_by_id)
                .service(get_lyrics_by_id),
        )
        .await;

        let req = test::TestRequest::put()
            .uri("/api/lyrics/id/1")
            .set_json(crate::types::UpdateLyricContentRequest {
                content: "[00:01.00]Updated line".to_string(),
            })
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let req = test::TestRequest::get()
            .uri("/api/lyrics/id/1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body = test::read_body(resp).await;
        let content = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(content, "[00:01.00]Updated line");
    }

    #[actix_web::test]
    async fn test_update_lyrics_by_id_rejects_blank_content() {
        let (_temp_dir, app_state) = create_test_setup().await;
        let app =
            test::init_service(App::new().app_data(app_state).service(update_lyrics_by_id)).await;

        let req = test::TestRequest::put()
            .uri("/api/lyrics/id/1")
            .set_json(crate::types::UpdateLyricContentRequest {
                content: "  \n\t".to_string(),
            })
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_update_lyrics_by_id_returns_not_found_without_sidecar_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let music_dir = temp_dir.path();
        let audio_path = music_dir.join("no-lyrics.mp3");
        std::fs::write(&audio_path, b"fake audio content").unwrap();

        let db_conn = crate::database::establish_connection(music_dir.to_str().unwrap())
            .await
            .unwrap();
        crate::services::scanner::initialize_database(music_dir.to_str().unwrap(), &db_conn)
            .await
            .unwrap();

        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_dir.to_str().unwrap().to_string()),
            download_root: Arc::new(music_dir.to_str().unwrap().to_string()),
            preview_root: Arc::new(music_dir.join(".preview").to_string_lossy().to_string()),
            db_conn,
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        let app =
            test::init_service(App::new().app_data(app_state).service(update_lyrics_by_id)).await;

        let req = test::TestRequest::put()
            .uri("/api/lyrics/id/1")
            .set_json(crate::types::UpdateLyricContentRequest {
                content: "[00:01.00]Updated line".to_string(),
            })
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 404);
    }

    // --- get_lyrics_by_path tests (open-as-default-app flow) ---

    fn percent_encoded_query(path: &str) -> String {
        let mut out = String::with_capacity(path.len() * 3);
        for byte in path.bytes() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                out.push(char::from(byte));
            } else {
                out.push('%');
                out.push_str(&format!("{:02X}", byte));
            }
        }
        format!("/api/lyrics/path?p={out}")
    }

    #[actix_web::test]
    async fn lyrics_by_path_serves_lrc_sidecar() {
        let temp_dir = tempfile::tempdir().unwrap();
        let audio_path = temp_dir.path().join("launch.mp3");
        std::fs::write(&audio_path, b"fake audio").unwrap();
        let lyrics_path = temp_dir.path().join("launch.lrc");
        std::fs::write(
            &lyrics_path,
            "[00:00.50]Launch lyric line\n[00:02.00]Second line",
        )
        .unwrap();

        let app = test::init_service(App::new().service(get_lyrics_by_path)).await;
        let req = test::TestRequest::get()
            .uri(&percent_encoded_query(audio_path.to_str().unwrap()))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 200);
        let body = test::read_body(resp).await;
        let content = String::from_utf8(body.to_vec()).unwrap();
        assert!(content.contains("[00:00.50]Launch lyric line"));
    }

    #[actix_web::test]
    async fn lyrics_by_path_falls_back_to_vtt_sidecar() {
        let temp_dir = tempfile::tempdir().unwrap();
        let audio_path = temp_dir.path().join("launch.flac");
        std::fs::write(&audio_path, b"fake audio").unwrap();
        let lyrics_path = temp_dir.path().join("launch.vtt");
        std::fs::write(
            &lyrics_path,
            "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\nhello\n",
        )
        .unwrap();

        let app = test::init_service(App::new().service(get_lyrics_by_path)).await;
        let req = test::TestRequest::get()
            .uri(&percent_encoded_query(audio_path.to_str().unwrap()))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 200);
        let body = test::read_body(resp).await;
        let content = String::from_utf8(body.to_vec()).unwrap();
        assert!(content.contains("WEBVTT"));
    }

    #[actix_web::test]
    async fn lyrics_by_path_returns_404_without_sidecar() {
        let temp_dir = tempfile::tempdir().unwrap();
        let audio_path = temp_dir.path().join("alone.mp3");
        std::fs::write(&audio_path, b"fake audio").unwrap();

        let app = test::init_service(App::new().service(get_lyrics_by_path)).await;
        let req = test::TestRequest::get()
            .uri(&percent_encoded_query(audio_path.to_str().unwrap()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 404);
    }

    #[actix_web::test]
    async fn lyrics_by_path_rejects_non_audio_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let secret_path = temp_dir.path().join("passwd.txt");
        std::fs::write(&secret_path, b"root:x:0:0:").unwrap();

        let app = test::init_service(App::new().service(get_lyrics_by_path)).await;
        let req = test::TestRequest::get()
            .uri(&percent_encoded_query(secret_path.to_str().unwrap()))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn lyrics_by_path_rejects_content_uri() {
        let app = test::init_service(App::new().service(get_lyrics_by_path)).await;
        let req = test::TestRequest::get()
            .uri("/api/lyrics/path?p=content%3A%2F%2Fmedia%2Faudio.mp3")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }
}
