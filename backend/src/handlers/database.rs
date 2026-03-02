//! Database update API handlers.
//!
//! This module provides endpoints for:
//! - Triggering a database update (scan for new files, update LUFS, remove deleted files)
//! - Getting playlists in collection mode (returns collections instead of folders)

use actix_web::{get, post, web, HttpResponse, Responder};
use crate::entities::music::{Entity as MusicEntity};
use crate::entities::collection::{Entity as CollectionEntity};
use crate::entities::collection_item::{Entity as CollectionItemEntity, Column as CollectionItemColumn};
use crate::types::{AppState, MusicInfo, UpdateResponse};
use crate::services::scanner;
use sea_orm::{EntityTrait, ColumnTrait, QueryFilter};
use tracing::info;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UpdateQuery {
    pub startup: Option<bool>,
}

/// Update database (scan for new files, update LUFS, remove deleted files)
///
/// Triggers a comprehensive database update:
/// - Scans the music directory for new audio files
/// - Calculates LUFS values for new files using FFmpeg
/// - Updates existing files that are missing LUFS values
/// - Removes database entries for deleted files
///
/// # Returns
/// JSON response with update status
#[post("/api/database/update")]
pub async fn update_database_endpoint(
    data: web::Data<AppState>,
    query: web::Query<UpdateQuery>,
) -> impl Responder {
    info!("Database update requested via API");

    let _scan_guard = data.scan_lock.lock().await;
    let is_startup = query.startup.unwrap_or(false);

    if is_startup {
        match scanner::get_initial_scan_done(&data.db_conn).await {
            Ok(true) => {
                info!("Startup scan skipped (already completed)");
                return HttpResponse::Ok().json(UpdateResponse {
                    success: true,
                    message: "Startup scan skipped (already completed)".to_string(),
                });
            }
            Ok(false) => {}
            Err(e) => {
                return HttpResponse::InternalServerError().json(UpdateResponse {
                    success: false,
                    message: format!("Failed to read startup scan flag: {}", e),
                });
            }
        }

        match scanner::initialize_database(&*data.music_path, &data.db_conn).await {
            Ok(_) => {
                if let Err(e) = scanner::set_initial_scan_done(&data.db_conn, true).await {
                    return HttpResponse::InternalServerError().json(UpdateResponse {
                        success: false,
                        message: format!("Failed to update startup scan flag: {}", e),
                    });
                }
                info!("Startup scan completed successfully");
                HttpResponse::Ok().json(UpdateResponse {
                    success: true,
                    message: "Startup scan completed successfully".to_string(),
                })
            }
            Err(e) => {
                HttpResponse::InternalServerError().json(UpdateResponse {
                    success: false,
                    message: format!("Startup scan failed: {}", e),
                })
            }
        }
    } else {
        match scanner::update_database(&*data.music_path, &data.db_conn).await {
            Ok(_) => {
                info!("Database update completed successfully");
                HttpResponse::Ok().json(UpdateResponse {
                    success: true,
                    message: "Database updated successfully".to_string(),
                })
            }
            Err(e) => {
                HttpResponse::InternalServerError().json(UpdateResponse {
                    success: false,
                    message: format!("Database update failed: {}", e),
                })
            }
        }
    }
}

/// Get playlists in collection mode (returns collections instead of folders)
///
/// Returns a hashmap similar to `/api/playlists` but with user-defined collections
/// instead of folder-based playlists. Includes "所有音乐" (All Music) which contains
/// all songs in the database.
///
/// This endpoint is used by the frontend when the user switches to "collection mode".
///
/// **IMPORTANT:** This route must be registered before `/api/playlists/{name}`
/// in the server configuration, otherwise it will match the wrong route.
///
/// # Returns
/// JSON object with collection names as keys and arrays of `MusicInfo` as values
#[get("/api/playlists/collection-mode")]
pub async fn get_playlists_collection_mode(data: web::Data<AppState>) -> impl Responder {
    // Block until database scan completes
    let _lock = data.scan_lock.lock().await;

    let mut playlists: std::collections::HashMap<String, Vec<MusicInfo>> = std::collections::HashMap::new();

    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            // Add all music to "所有音乐" (All Music)
            for music in &music_list {
                let lufs_value = music.lufs.unwrap_or(0.5);
                let info = MusicInfo {
                    id: music.id,
                    name: music.filename.clone(),
                    lufs: lufs_value,
                    path: music.file_path.clone(),
                };
                playlists.entry("所有音乐".to_string()).or_insert_with(Vec::new).push(info);
            }

            // Add collections
            match CollectionEntity::find().all(&data.db_conn).await {
                Ok(collections) => {
                    for collection in collections {
                        match CollectionItemEntity::find()
                            .filter(CollectionItemColumn::CollectionId.eq(collection.id))
                            .find_also_related(MusicEntity)
                            .all(&data.db_conn)
                            .await
                        {
                            Ok(items) => {
                                let songs: Vec<MusicInfo> = items
                                    .into_iter()
                                    .filter_map(|(_, music_opt)| music_opt)
                                    .map(|music| MusicInfo {
                                        id: music.id,
                                        name: music.filename,
                                        lufs: music.lufs.unwrap_or(0.5),
                                        path: music.file_path,
                                    })
                                    .collect();
                                playlists.insert(collection.name, songs);
                            }
                            Err(_) => continue,
                        }
                    }
                }
                Err(_) => {}
            }
        }
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    }

    HttpResponse::Ok().json(playlists)
}
