//! Shared request/response types for the Kaulan music player API.
//!
//! This module contains all the data structures used for API requests and responses.

use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// Music response with database metadata
#[derive(Serialize, Deserialize)]
pub struct MusicResponse {
    pub id: i32,
    pub filename: String,
    pub file_path: String,
    pub lufs: Option<f64>,
    pub created_at: String,
}

/// Music information for playlist responses
#[derive(Serialize, Deserialize, Clone)]
pub struct MusicInfo {
    pub id: i32,
    pub name: String,
    pub lufs: Option<f64>,
    pub path: String,
}

/// Playlist with songs
#[derive(Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub songs: Vec<MusicInfo>,
}

/// Collection metadata (without songs)
#[derive(Serialize, Deserialize)]
pub struct Collection {
    pub id: i32,
    pub name: String,
    pub created_at: String,
}

/// Collection with songs
#[derive(Serialize, Deserialize)]
pub struct CollectionWithSongs {
    pub id: i32,
    pub name: String,
    pub songs: Vec<MusicInfo>,
}

/// Request to create a new collection
#[derive(Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
}

/// Request to add songs to a collection
#[derive(Deserialize)]
pub struct AddToCollectionRequest {
    pub music_ids: Vec<i32>,
}

/// Request to remove songs from a collection
#[derive(Deserialize)]
pub struct RemoveFromCollectionRequest {
    pub music_ids: Vec<i32>,
}

/// Application state shared across all handlers
pub struct AppState {
    pub music_path: Arc<String>,
    pub db_conn: DatabaseConnection,
    pub scan_lock: Arc<TokioMutex<()>>,
    pub discovery: Arc<crate::discovery::types::DiscoveryState>,
}

/// Directory tree node for representing file system structure
#[derive(Serialize, Deserialize)]
pub struct DirectoryNode {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub children: Option<Vec<DirectoryNode>>,
}

/// Response for file upload operation
#[derive(Serialize, Deserialize)]
pub struct UploadResponse {
    pub success: bool,
    pub message: String,
    pub uploaded: Vec<String>,
    pub failed: Vec<String>,
}

/// Request to set music directory
#[derive(Deserialize)]
pub struct SetMusicDirectoryRequest {
    pub path: String,
}

/// Response for get music directory endpoint
#[derive(Serialize)]
pub struct MusicDirectoryResponse {
    pub path: String,
}

/// Response for set music directory endpoint
#[derive(Serialize)]
pub struct SetDirectoryResponse {
    pub success: bool,
    pub message: String,
}

/// Request to set media types
#[derive(Deserialize)]
pub struct SetMediaTypesRequest {
    pub media_types: Vec<String>,
}

/// Response for get media types endpoint
#[derive(Serialize)]
pub struct MediaTypesResponse {
    pub media_types: Vec<String>,
}

/// Response for database update endpoint
#[derive(Serialize)]
pub struct UpdateResponse {
    pub success: bool,
    pub message: String,
}
