//! Lyrics API handlers.
//!
//! This module provides endpoints for:
//! - Streaming sidecar lyric files (`.lrc` preferred, `.vtt` fallback) synchronized with music playback

use crate::entities::music::{Column as MusicColumn, Entity as MusicEntity};
use crate::file_ops::get_lyric_reader;
use crate::types::AppState;
use actix_web::{get, web, HttpResponse, Responder};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
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
}
