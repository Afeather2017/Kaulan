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
/// - If LUFS is null, spawns a background task to calculate it and returns 202 Accepted
/// - The background task updates the database when calculation completes
/// - Uses seekable file readers for both regular paths and content URIs
///
/// # Returns
/// - `200 OK` with LUFS value if already cached
/// - `202 Accepted` if calculation started in background (non-blocking)
/// - `404 Not Found` if music ID doesn't exist
/// - `500 Internal Server Error` for database errors
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
                info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 200 (Already Cached)", id);
                return HttpResponse::Ok().json(PrecacheLufsResponse {
                    success: true,
                    lufs: Some(existing_lufs),
                    cached: Some(true),
                    error: None,
                });
            }

            // LUFS is null, spawn background task and return immediately
            let file_path = music.file_path.clone();
            let file_label = file_path.clone();
            let db_conn = data.db_conn.clone();

            debug!("Spawning background LUFS calculation for music ID {} (file: {})", id, file_path);

            // Spawn background task - non-blocking
            tokio::spawn(async move {
                info!("Background LUFS calculation started for music ID {}: {}", id, file_label);

                // Open seekable reader
                let reader = match get_file_reader().open_seekable_reader(&file_path).await {
                    Ok(r) => r,
                    Err(e) => {
                        error!("Background LUFS: Failed to open seekable reader for music ID {}: {}", id, e);
                        return;
                    }
                };

                // Calculate LUFS in blocking thread
                let lufs_result = tokio::task::spawn_blocking(move || {
                    let calc = LufsCalculator::default();
                    match calc.calculate_from_reader(reader) {
                        Ok(Some(lufs)) => {
                            info!("[LUFS] BACKGROUND SUCCESS: {} - LUFS: {}", file_label, lufs);
                            Some(lufs)
                        }
                        Ok(None) => {
                            warn!("[LUFS] BACKGROUND FAILED: Unsupported format for: {}", file_label);
                            None
                        }
                        Err(e) => {
                            error!("[LUFS] BACKGROUND ERROR: Failed to calculate LUFS for {}: {}", file_label, e);
                            None
                        }
                    }
                })
                .await;

                match lufs_result {
                    Ok(Some(lufs_value)) => {
                        // Update database with calculated LUFS
                        match MusicEntity::find_by_id(id).one(&db_conn).await {
                            Ok(Some(music_to_update)) => {
                                let mut active_model: MusicActiveModel = music_to_update.into();
                                active_model.lufs = Set(Some(lufs_value));

                                match active_model.update(&db_conn).await {
                                    Ok(_) => {
                                        info!("Background LUFS pre-cache complete for music ID {}: {}", id, lufs_value);
                                    }
                                    Err(e) => {
                                        error!("Background LUFS: Failed to update database for music ID {}: {}", id, e);
                                    }
                                }
                            }
                            Ok(None) => {
                                warn!("Background LUFS: Music ID {} no longer exists", id);
                            }
                            Err(e) => {
                                error!("Background LUFS: Database error fetching music ID {}: {}", id, e);
                            }
                        }
                    }
                    Ok(None) => {
                        debug!("Background LUFS: Unsupported format for music ID {}", id);
                    }
                    Err(e) => {
                        error!("Background LUFS: Task execution failed for music ID {}: {}", id, e);
                    }
                }
            });

            info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 202 (Processing in Background)", id);
            HttpResponse::Accepted().json(PrecacheLufsResponse {
                success: true,
                lufs: None,
                cached: Some(false),
                error: None,
            })
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
