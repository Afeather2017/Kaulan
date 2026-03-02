//! LUFS pre-caching API handlers.
//!
//! This module provides on-demand LUFS calculation endpoint.
//! LUFS values are calculated when a song starts playing (for the next song)
//! rather than during database updates.

use actix_web::{post, web, HttpResponse, Responder};
use crate::entities::music::{Entity as MusicEntity, ActiveModel as MusicActiveModel};
use crate::lufsgen::get_lufs;
use crate::types::AppState;
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

/// Check if a path is a content URI (Android MediaStore)
fn is_content_uri(path: &str) -> bool {
    path.starts_with("content://")
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
/// - For content URIs (Android), returns without calculation (FFmpeg can't access them)
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

            // For content URIs, we can't calculate LUFS (FFmpeg can't access them)
            if is_content_uri(&file_path) {
                warn!("LUFS pre-cache skipped for content URI (FFmpeg cannot access): {}", file_path);
                info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 200 (Skipped - Content URI)", id);
                return HttpResponse::Ok().json(PrecacheLufsResponse {
                    success: true,
                    lufs: None,
                    cached: Some(false),
                    error: Some("Content URIs cannot be processed by FFmpeg".to_string()),
                });
            }

            debug!("Calculating LUFS for music ID {} (file: {})", id, file_path);

            // Use spawn_blocking for CPU-bound LUFS calculation
            let lufs_result = tokio::task::spawn_blocking(move || {
                get_lufs(&file_path)
            })
            .await;

            match lufs_result {
                Ok(inner_result) => {
                    match inner_result {
                        Some(lufs_value) => {
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
                        None => {
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
                    }
                }
                Err(e) => {
                    // Task join error
                    error!("LUFS pre-cache task failed for music ID {}: {}", id, e);
                    info!("[ACCESS] POST /api/music/{}/precache-lufs - Status: 500 (Task Failed)", id);
                    HttpResponse::InternalServerError().json(PrecacheLufsResponse {
                        success: false,
                        lufs: None,
                        cached: None,
                        error: Some(format!("Task execution failed: {}", e)),
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
