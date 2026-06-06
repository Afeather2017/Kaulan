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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_url: Option<String>,
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
    pub download_root: Arc<String>,
    pub preview_root: Arc<String>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadSource {
    Youtube,
    Netease,
    Bilibili,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineSearchRequest {
    pub query: String,
    pub max_results: usize,
    #[serde(default)]
    pub sources: Vec<DownloadSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineSearchResult {
    pub source: DownloadSource,
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration: Option<String>,
    pub thumbnail_url: Option<String>,
    pub can_preview: bool,
    pub can_download: bool,
    pub requires_login: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineProviderStatus {
    pub source: DownloadSource,
    pub enabled: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadPreviewRequest {
    pub source: DownloadSource,
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadPreviewResponse {
    pub success: bool,
    pub message: String,
    pub song: Option<PreviewSong>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewSong {
    pub id: i32,
    pub name: String,
    pub path: String,
    pub stream_url: String,
    pub cover_url: Option<String>,
    pub source: DownloadSource,
    pub is_temporary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsSearchRequest {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricCandidate {
    pub source: DownloadSource,
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTrackRequest {
    pub source: DownloadSource,
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub target_subdir: Option<String>,
    pub lyric_selection: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTrackResponse {
    pub success: bool,
    pub message: String,
    pub filename: Option<String>,
    pub lyric_filename: Option<String>,
    pub warning: Option<String>,
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

/// Query parameter for requesting direct raw-path playback URLs on Android localhost.
#[derive(Deserialize)]
pub struct StreamQuery {
    /// When set to `content` on Android localhost requests, expose the raw path
    /// in `stream_url` for direct player access.
    pub stream: Option<String>,
}

/// Build the public HTTP stream URL for a music item.
pub fn build_http_stream_url(req: &actix_web::HttpRequest, music_id: i32) -> String {
    let connection = req.connection_info();
    format!(
        "{}://{}/api/music/id/{}",
        connection.scheme(),
        connection.host(),
        music_id
    )
}

/// Check if the request originates from localhost (loopback address).
pub fn is_localhost_request(req: &actix_web::HttpRequest) -> bool {
    req.peer_addr()
        .map(|addr| addr.ip().is_loopback())
        .unwrap_or(false)
}

pub fn validate_stream_request(
    stream_param: &Option<String>,
    is_localhost: bool,
) -> Result<(), actix_web::HttpResponse> {
    if stream_param.as_deref() == Some("content") && !is_localhost {
        return Err(actix_web::HttpResponse::Forbidden()
            .body("content stream is only available from localhost"));
    }

    #[cfg(not(target_os = "android"))]
    if stream_param.as_deref() == Some("content") {
        return Err(actix_web::HttpResponse::BadRequest()
            .body("content stream is only available on Android"));
    }

    Ok(())
}

/// Resolve the playback path to expose in list responses.
pub fn resolve_playback_path(file_path: &str, is_localhost: bool) -> String {
    if is_localhost {
        file_path.to_string()
    } else {
        String::new()
    }
}

pub fn resolve_stream_url(
    file_path: &str,
    stream_param: &Option<String>,
    is_localhost: bool,
) -> Option<String> {
    #[cfg(target_os = "android")]
    if is_localhost && stream_param.as_deref() == Some("content") {
        return Some(file_path.to_string());
    }

    #[cfg(not(target_os = "android"))]
    let _ = (file_path, stream_param, is_localhost);

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    #[test]
    fn builds_absolute_http_stream_url() {
        let req = TestRequest::default()
            .insert_header(("host", "192.168.136.29:2080"))
            .to_http_request();

        assert_eq!(
            build_http_stream_url(&req, 42),
            "http://192.168.136.29:2080/api/music/id/42"
        );
    }

    #[test]
    fn localhost_request_detection_works_for_loopback() {
        let req = TestRequest::default()
            .peer_addr("127.0.0.1:12345".parse().unwrap())
            .to_http_request();

        assert!(is_localhost_request(&req));
    }

    #[test]
    fn localhost_request_detection_rejects_lan_clients() {
        let req = TestRequest::default()
            .peer_addr("192.168.136.10:54321".parse().unwrap())
            .to_http_request();

        assert!(!is_localhost_request(&req));
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn non_android_builds_reject_content_stream_requests() {
        let response = validate_stream_request(&Some("content".to_string()), true);
        assert!(response.is_err());
    }
}
