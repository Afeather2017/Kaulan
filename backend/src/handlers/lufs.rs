//! LUFS pre-caching API handlers.
//!
//! This module provides on-demand LUFS calculation endpoint.
//! LUFS values are calculated when a song starts playing (for the next song)
//! rather than during database updates.
//!
//! Documentation: [docs/settings-and-database-management.md](../../../docs/settings-and-database-management.md)

use actix_web::{post, web, HttpResponse, Responder};
use crate::entities::music::{Entity as MusicEntity, ActiveModel as MusicActiveModel};
use crate::file_ops::get_file_reader;
use crate::types::AppState;
use lufsgen::LufsCalculator;
use sea_orm::{EntityTrait, ActiveModelTrait, Set};
use serde::Serialize;
use tracing::{info, warn, error, debug};

/// Response for LUFS pre-cache endpoint
#[derive(Serialize)]
pub struct PrecacheLufsResponse {
    pub success: bool,
    pub lufs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Calculate LUFS for a file path or content URI using seekable reader API.
async fn calculate_lufs(file_path: &str) -> Result<Option<f64>, String> {
    let reader = get_file_reader()
        .open_seekable_reader(file_path)
        .await
        .map_err(|e| format!("Failed to open seekable reader: {}", e))?;

    let file_label = file_path.to_string();
    tokio::task::spawn_blocking(move || {
        let calc = LufsCalculator::default();
        match calc.calculate_from_reader(reader) {
            Ok(Some(lufs)) => {
                info!("[LUFS] SUCCESS: {} - LUFS: {}", file_label, lufs);
                Some(lufs)
            }
            Ok(None) => {
                warn!("[LUFS] FAILED: Unsupported format for: {}", file_label);
                None
            }
            Err(e) => {
                error!("[LUFS] ERROR: Failed to calculate LUFS for {}: {}", file_label, e);
                None
            }
        }
    })
    .await
    .map_err(|e| format!("LUFS task execution failed: {}", e))
}

/// Pre-cache LUFS for a music track
///
/// This endpoint calculates LUFS on-demand for playback. It's called when
/// a song starts playing to pre-cache the LUFS value for the next song.
///
/// # Path Parameters
/// * `id` - The music ID to calculate LUFS for
///
/// # Behavior
/// - If LUFS already exists in database, returns immediately with existing value
/// - If LUFS is null, calculates it asynchronously using tokio::task::spawn_blocking
/// - Updates database with calculated LUFS value
/// - Uses seekable file readers for both regular paths and content URIs
///
/// # Returns
/// - `200 OK` with LUFS value (either newly calculated or already cached)
/// - `404 Not Found` if music ID doesn't exist
/// - `500 Internal Server Error` for database or calculation errors
#[post("/api/music/{id}/precache-lufs")]
pub async fn precache_lufs(
    path: web::Path<i32>,
    data: web::Data<AppState>,
) -> impl Responder {
    let id = path.into_inner();
    debug!("LUFS pre-cache requested for music ID: {}", id);
    info!("[ACCESS] POST /api/music/{}/precache-lufs - Started", id);

    // Look up music by ID
    match MusicEntity::find_by_id(id).one(&data.db_conn).await {
        Ok(Some(music)) => {
            // Check if LUFS already exists
            if music.lufs.is_some() {
                let existing_lufs = music.lufs.unwrap();
                debug!("LUFS already cached for music ID {}: {}", id, existing_lufs);
                info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 208 (Already Cached)", id);
                return HttpResponse::Ok().json(PrecacheLufsResponse {
                    success: true,
                    lufs: Some(existing_lufs),
                    cached: Some(true),
                    error: None,
                });
            }

            // LUFS is null, need to calculate
            let file_path = music.file_path.clone();
            debug!("Calculating LUFS for music ID {} (file: {})", id, file_path);

            match calculate_lufs(&file_path).await {
                Ok(Some(lufs_value)) => {
                    // Update database with calculated LUFS
                    let mut active_model: MusicActiveModel = music.into();
                    active_model.lufs = Set(Some(lufs_value));

                    match active_model.update(&data.db_conn).await {
                        Ok(_) => {
                            info!("LUFS pre-cache complete for music ID {}: {}", id, lufs_value);
                            info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 200 (Calculated)", id);
                            HttpResponse::Ok().json(PrecacheLufsResponse {
                                success: true,
                                lufs: Some(lufs_value),
                                cached: Some(false),
                                error: None,
                            })
                        }
                        Err(e) => {
                            error!("Failed to update LUFS in database for music ID {}: {}", id, e);
                            info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 500 (DB Update Failed)", id);
                            HttpResponse::InternalServerError().json(PrecacheLufsResponse {
                                success: false,
                                lufs: None,
                                cached: None,
                                error: Some(format!("Database update failed: {}", e)),
                            })
                        }
                    }
                }
                Ok(None) => {
                    // LUFS calculation returned None (unsupported format)
                    info!("LUFS pre-cache skipped for music ID {}: Unsupported audio format", id);
                    info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 200 (Unsupported Format)", id);
                    HttpResponse::Ok().json(PrecacheLufsResponse {
                        success: true,
                        lufs: None,
                        cached: Some(false),
                        error: Some("Unsupported audio format".to_string()),
                    })
                }
                Err(e) => {
                    error!("LUFS pre-cache failed for music ID {}: {}", id, e);
                    warn!("LUFS pre-cache could not process path {}: {}", file_path, e);
                    info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 500 (Calculation Failed)", id);
                    HttpResponse::InternalServerError().json(PrecacheLufsResponse {
                        success: false,
                        lufs: None,
                        cached: None,
                        error: Some(format!("LUFS calculation failed: {}", e)),
                    })
                }
            }
        }
        Ok(None) => {
            warn!("Music not found for LUFS pre-cache: ID {}", id);
            info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 404", id);
            HttpResponse::NotFound().json(PrecacheLufsResponse {
                success: false,
                lufs: None,
                cached: None,
                error: Some("Music not found".to_string()),
            })
        }
        Err(e) => {
            error!("Database error while fetching music ID {} for LUFS pre-cache: {}", id, e);
            info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 500", id);
            HttpResponse::InternalServerError().json(PrecacheLufsResponse {
                success: false,
                lufs: None,
                cached: None,
                error: Some(format!("Database error: {}", e)),
            })
        }
    }
}
