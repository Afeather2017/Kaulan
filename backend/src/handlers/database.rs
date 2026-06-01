//! Database update API handlers.
//!
//! This module provides endpoints for:
//! - Triggering a database update (scan for new files, update LUFS, remove deleted files)

use crate::services::scanner;
use crate::types::{AppState, UpdateResponse};
use actix_web::{post, web, HttpResponse, Responder};
use serde::Deserialize;
use tracing::info;

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

        let library_roots = [
            data.music_path.as_ref().as_str(),
            data.download_root.as_ref().as_str(),
        ];
        match scanner::initialize_database_with_roots(&library_roots, &data.db_conn).await {
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
            Err(e) => HttpResponse::InternalServerError().json(UpdateResponse {
                success: false,
                message: format!("Startup scan failed: {}", e),
            }),
        }
    } else {
        let library_roots = [
            data.music_path.as_ref().as_str(),
            data.download_root.as_ref().as_str(),
        ];
        match scanner::update_database_with_roots(&library_roots, &data.db_conn).await {
            Ok(_) => {
                info!("Database update completed successfully");
                HttpResponse::Ok().json(UpdateResponse {
                    success: true,
                    message: "Database updated successfully".to_string(),
                })
            }
            Err(e) => HttpResponse::InternalServerError().json(UpdateResponse {
                success: false,
                message: format!("Database update failed: {}", e),
            }),
        }
    }
}
