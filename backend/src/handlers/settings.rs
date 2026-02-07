//! Settings API handlers for application configuration.
//!
//! This module provides endpoints for:
//! - Getting the current music directory
//! - Setting the music directory (saved to config file)
//!
//! Documentation: [docs/settings-and-database-management.md](../../../docs/settings-and-database-management.md)

use actix_web::{get, post, web, HttpResponse, Responder};
use crate::types::{AppState, MusicDirectoryResponse, SetDirectoryResponse, SetMusicDirectoryRequest};
use crate::config;
use std::path::Path;
use tracing::{info, warn, error};

/// Get current music directory
///
/// Returns the currently configured music directory path.
///
/// # Documentation
/// See [`docs/settings-and-database-management.md`](../../../docs/settings-and-database-management.md)
///
/// # Returns
/// JSON response with the current music directory path
#[get("/api/settings/music-directory")]
pub async fn get_music_directory(data: web::Data<AppState>) -> impl Responder {
    HttpResponse::Ok().json(MusicDirectoryResponse {
        path: (*data.music_path).clone(),
    })
}

/// Set music directory (saved to config, takes effect on restart)
///
/// Validates and saves the music directory path to the config file.
/// The new path will take effect on the next application restart.
///
/// # Request Body
/// ```json
/// {
///   "path": "/path/to/music"
/// }
/// ```
///
/// For Android SAF, the path will be a content URI:
/// ```json
/// {
///   "path": "content://com.android.externalstorage.documents/tree/primary%3AMusic"
/// }
/// ```
///
/// # Documentation
/// See [`docs/settings-and-database-management.md`](../../../docs/settings-and-database-management.md)
///
/// # Behavior
/// - For desktop platforms: Validates that the path exists and is a directory
/// - For Android SAF (content:// URIs): Skips file system validation
/// - Saves the path or URI to the config file
/// - Returns a success message indicating restart is required
///
/// # Config File Location
/// - Linux (standalone): `~/.config/kaulan/config.json`
/// - macOS (standalone): `~/Library/Application Support/kaulan/config.json`
/// - Windows (standalone): `%APPDATA%\kaulan\config.json`
/// - Tauri mode: Platform-specific app data directory
///
/// # Returns
/// - `200 OK` with success message if path is valid and saved
/// - `400 Bad Request` if path doesn't exist or is not a directory (desktop only)
/// - `500 Internal Server Error` if config save fails
#[post("/api/settings/music-directory")]
pub async fn set_music_directory(
    req: web::Json<SetMusicDirectoryRequest>,
) -> impl Responder {
    let new_path = &req.path;

    // Check if this is a SAF content URI (Android)
    let is_saf_uri = new_path.starts_with("content://");

    if is_saf_uri {
        info!("Received SAF tree URI: {}", new_path);
        // SAF URIs cannot be validated with std::path - skip validation
        // The URI will be validated when the filesystem layer tries to use it
    } else {
        // Validate the path exists and is a directory (desktop platforms)
        let path_obj = Path::new(new_path);
        if !path_obj.exists() {
            warn!("Music directory does not exist: {}", new_path);
            return HttpResponse::BadRequest().json(SetDirectoryResponse {
                success: false,
                message: format!("Directory does not exist: {}", new_path),
            });
        }

        if !path_obj.is_dir() {
            warn!("Path is not a directory: {}", new_path);
            return HttpResponse::BadRequest().json(SetDirectoryResponse {
                success: false,
                message: format!("Path is not a directory: {}", new_path),
            });
        }
    }

    // Save to config file
    match config::save_config(new_path) {
        Ok(_) => {
            info!("Music directory saved to config: {}", new_path);
            HttpResponse::Ok().json(SetDirectoryResponse {
                success: true,
                message: format!(
                    "Music directory will be set to '{}' on next restart.",
                    if is_saf_uri {
                        // Show a user-friendly name for SAF URIs
                        "[Android Storage Directory]"
                    } else {
                        new_path
                    }
                ),
            })
        }
        Err(e) => {
            error!("Failed to save config: {}", e);
            HttpResponse::InternalServerError().json(SetDirectoryResponse {
                success: false,
                message: format!("Failed to save configuration: {}", e),
            })
        }
    }
}
