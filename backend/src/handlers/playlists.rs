//! Playlist API handlers for folder-based playlists.
//!
//! This module provides endpoints for:
//! - Getting all folder-based playlists
//! - Getting a specific playlist by name
//!
//! The folder-based playlist structure automatically groups music by their parent folder.

use actix_web::{get, web, HttpResponse, Responder};
use crate::entities::music::{Entity as MusicEntity};
use crate::types::{AppState, MusicInfo, Playlist};
use sea_orm::EntityTrait;
use std::path::Path;

/// Get all playlists
///
/// Returns a hashmap of folder-based playlists. The keys are folder names
/// (or "所有音乐" for "All Music"), and the values are lists of songs in each playlist.
///
/// The function automatically groups music files by their parent folder directory.
/// Files in the root of the music directory are grouped under "所有音乐" (All Music).
///
/// # Returns
/// JSON object with playlist names as keys and arrays of `MusicInfo` as values
#[get("/api/playlists")]
pub async fn get_all_playlists(data: web::Data<AppState>) -> impl Responder {
    // Block until database scan completes
    let _lock = data.scan_lock.lock().await;

    let mut playlists: std::collections::HashMap<String, Vec<MusicInfo>> = std::collections::HashMap::new();

    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            for music in &music_list {
                let info = MusicInfo {
                    id: music.id,
                    name: music.filename.clone(),
                    lufs: music.lufs,
                    path: music.file_path.clone(),
                };

                // Add to "All Music" playlist
                playlists.entry("所有音乐".to_string()).or_insert_with(Vec::new).push(info.clone());

                // Add to folder-based playlist
                if let Some(parent) = Path::new(&music.file_path).parent() {
                    if !parent.as_os_str().is_empty() {
                        if let Some(dir_name) = parent.file_name() {
                            let playlist_name = dir_name.to_string_lossy().to_string();
                            playlists.entry(playlist_name).or_insert_with(Vec::new).push(info);
                        }
                    }
                }
            }
        }
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    }

    HttpResponse::Ok().json(playlists)
}

/// Get a specific playlist by name
///
/// Returns the songs in a specific folder-based playlist.
/// Use "所有音乐" to get all music regardless of folder.
///
/// # Path Parameters
/// * `name` - The playlist name (folder name or "所有音乐" for all music)
///
/// # Returns
/// JSON object with `name` and `songs` array
#[get("/api/playlists/{name}")]
pub async fn get_playlist(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> impl Responder {
    // Block until database scan completes
    let _lock = data.scan_lock.lock().await;

    let playlist_name = path.into_inner();
    let mut songs: Vec<MusicInfo> = Vec::new();

    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            for music in music_list {
                let info = MusicInfo {
                    id: music.id,
                    name: music.filename.clone(),
                    lufs: music.lufs,
                    path: music.file_path.clone(),
                };

                let belongs_to_playlist = if playlist_name == "所有音乐" {
                    true
                } else {
                    match Path::new(&music.file_path).parent() {
                        Some(parent) if !parent.as_os_str().is_empty() => {
                            parent.file_name()
                                .map(|name| name.to_string_lossy() == playlist_name)
                                .unwrap_or(false)
                        }
                        _ => false,
                    }
                };

                if belongs_to_playlist {
                    songs.push(info);
                }
            }
        }
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    }

    HttpResponse::Ok().json(Playlist {
        name: playlist_name,
        songs,
    })
}
