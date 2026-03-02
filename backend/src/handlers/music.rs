//! Music streaming and metadata API handlers.
//!
//! This module provides endpoints for:
//! - Streaming individual music files
//! - Getting all music from the database

use actix_web::{get, web, HttpResponse, Responder};
use futures::TryStreamExt;
use serde::Serialize;
use crate::entities::music::{Entity as MusicEntity, Model as MusicModel, Column as MusicColumn};
use crate::types::AppState;
use crate::file_ops::get_file_reader;
use sea_orm::{EntityTrait, ColumnTrait, QueryFilter};
use tracing::{debug, info, warn, error};

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
pub async fn get_music(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> impl Responder {
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
            debug!("Found music in database: filename={}, file_path={}", music.filename, music.file_path);

            let file_reader = get_file_reader();
            debug!("File reader obtained for reading: {}", music.file_path);

            const CHUNK_SIZE: usize = 1024 * 1024;
            match file_reader.read_stream(&music.file_path, CHUNK_SIZE).await {
                Ok(stream) => {
                    debug!("Streaming music file: {}", filename);
                    info!("[ACCESS] GET /api/music/{} - Status: 200", filename);
                    let mut response = HttpResponse::Ok();
                    response.insert_header(("Content-Type", "audio/mpeg"));
                    response.insert_header(("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0"));
                    response.insert_header(("Pragma", "no-cache"));
                    response.insert_header(("Expires", "0"));
                    response.streaming(stream.map_err(actix_web::Error::from))
                }
                Err(e) => {
                    warn!("File not found or could not be read: {} - Error: {}", music.file_path, e);
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
///
/// # Path Parameters
/// * `id` - The music ID to look up in the database
///
/// # Returns
/// - Audio file stream with `audio/mpeg` content type if found
/// - `404 Not Found` if music not in database or file missing
/// - `500 Internal Server Error` for database errors
#[get("/api/music/id/{id}")]
pub async fn get_music_by_id(
    path: web::Path<i32>,
    data: web::Data<AppState>,
) -> impl Responder {
    let id = path.into_inner();
    debug!("Music request received for ID: {}", id);

    // Simple access log
    info!("[ACCESS] GET /api/music/id/{} - Started", id);

    match MusicEntity::find_by_id(id).one(&data.db_conn).await {
        Ok(Some(music)) => {
            debug!("Found music in database: id={}, filename={}, file_path={}", music.id, music.filename, music.file_path);

            let file_reader = get_file_reader();
            debug!("File reader obtained for reading: {}", music.file_path);

            const CHUNK_SIZE: usize = 1024 * 1024;
            match file_reader.read_stream(&music.file_path, CHUNK_SIZE).await {
                Ok(stream) => {
                    debug!("Streaming music file: {} (ID: {})", music.filename, id);
                    info!("[ACCESS] GET /api/music/id/{} - Status: 200", id);
                    let mut response = HttpResponse::Ok();
                    response.insert_header(("Content-Type", "audio/mpeg"));
                    response.insert_header(("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0"));
                    response.insert_header(("Pragma", "no-cache"));
                    response.insert_header(("Expires", "0"));
                    response.streaming(stream.map_err(actix_web::Error::from))
                }
                Err(e) => {
                    warn!("File not found or could not be read: {} (ID: {}) - Error: {}", music.file_path, id, e);
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
                .map(|music: MusicModel| {
                    MusicResponse {
                        id: music.id,
                        filename: music.filename,
                        file_path: music.file_path,
                        lufs: music.lufs,
                        created_at: music.created_at.to_rfc3339(),
                    }
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

/// Music response with database metadata
#[derive(Serialize)]
pub struct MusicResponse {
    pub id: i32,
    pub filename: String,
    pub file_path: String,
    pub lufs: Option<f64>,
    pub created_at: String,
}
