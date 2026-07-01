//! Kaulan Music Player - Backend Library
//!
//! This is the main library for the Kaulan music player backend.
//! It provides HTTP API endpoints, database operations, and file management.

use std::sync::Arc;
use std::sync::OnceLock;
use ytdl_audio::JsRunner;

// Re-export public API for main.rs and external use
pub use config::load_config;
pub use database::establish_connection;
pub use server::{start_server, ServerInfo};
pub use services::scanner::{
    initialize_database, initialize_database_with_roots, update_database,
    update_database_with_roots,
};

// Re-export types for external use
pub use types::AppState;

// Re-export file operations for Android MediaStore integration
pub mod file_ops;
pub use file_ops::{
    set_file_reader, set_lyric_reader, set_music_file_lister, FileReader, LyricReader,
    MusicFileInfo, MusicFileLister, ReadSeekSendSync, SUPPORTED_EXTENSIONS,
};

// Re-export all handlers for integration tests
pub use server::{
    delete_music_batch, get_all_music, get_all_playlists, get_directory_tree, get_lyrics,
    get_music, get_music_by_id, get_music_directory, get_playlist, set_music_directory,
    update_database_endpoint, upload_files,
};

type YoutubeJsRunnerFactory = dyn Fn() -> Result<Box<dyn JsRunner>, String> + Send + Sync + 'static;
static YOUTUBE_JS_RUNNER_FACTORY: OnceLock<Arc<YoutubeJsRunnerFactory>> = OnceLock::new();

/// Initialize the tracing subscriber once for console logging.
pub fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init();
}

/// Register a factory that creates the YouTube JavaScript runner used by downloads.
pub fn set_youtube_js_runner_factory<F>(factory: F) -> Result<(), String>
where
    F: Fn() -> Result<Box<dyn JsRunner>, String> + Send + Sync + 'static,
{
    YOUTUBE_JS_RUNNER_FACTORY
        .set(Arc::new(factory))
        .map_err(|_| "YouTube JS runner factory is already initialized".to_string())
}

/// Build a YouTube JavaScript runner if the frontend layer registered one.
pub fn create_youtube_js_runner() -> Result<Option<Box<dyn JsRunner>>, String> {
    match YOUTUBE_JS_RUNNER_FACTORY.get() {
        Some(factory) => factory().map(Some),
        None => Ok(None),
    }
}

// Declare modules
pub mod cli;
pub mod config;
pub mod handlers;
pub mod middleware;
pub mod server;
pub mod services;
pub mod types;

// Existing modules (unchanged)
pub mod database;
pub mod discovery;
pub mod entities;
pub mod ffmpeg;
pub mod lufsgen;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DirectoryNode;
    use actix_web::{http::StatusCode, test, web, App};
    use std::sync::Arc;

    /// Helper function to create a temporary test directory structure
    fn create_test_directory() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let music_dir = dir.path();

        // Create test directory structure
        let folder1 = music_dir.join("folder1");
        let folder2 = music_dir.join("folder2");
        let subfolder = folder2.join("subfolder");

        std::fs::create_dir_all(&folder1).unwrap();
        std::fs::create_dir_all(&subfolder).unwrap();

        dir
    }

    #[actix_web::test]
    async fn test_directory_tree_empty_directory() {
        let temp_dir = create_test_directory();
        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(temp_dir.path().to_str().unwrap().to_string()),
            download_root: Arc::new(temp_dir.path().to_str().unwrap().to_string()),
            preview_root: Arc::new(
                temp_dir
                    .path()
                    .join(".preview")
                    .to_string_lossy()
                    .to_string(),
            ),
            db_conn: establish_connection(temp_dir.path().to_str().unwrap())
                .await
                .unwrap(),
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        let app =
            test::init_service(App::new().app_data(app_state).service(get_directory_tree)).await;

        let req = test::TestRequest::get()
            .uri("/api/files/directory-tree")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: DirectoryNode = test::read_body_json(resp).await;
        assert_eq!(body.node_type, "directory");
        assert_eq!(body.path, "");
        // Root should have children (folder1 and folder2)
        assert!(body.children.is_some());
        assert_eq!(body.children.as_ref().unwrap().len(), 2);
    }

    #[actix_web::test]
    async fn test_directory_tree_nested_structure() {
        let temp_dir = create_test_directory();
        let music_path = temp_dir.path().to_str().unwrap().to_string();

        // Verify the nested structure is returned correctly
        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_path.clone()),
            download_root: Arc::new(music_path.clone()),
            preview_root: Arc::new(
                std::path::PathBuf::from(&music_path)
                    .join(".preview")
                    .to_string_lossy()
                    .to_string(),
            ),
            db_conn: establish_connection(&music_path).await.unwrap(),
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        let app =
            test::init_service(App::new().app_data(app_state).service(get_directory_tree)).await;

        let req = test::TestRequest::get()
            .uri("/api/files/directory-tree")
            .to_request();

        let resp = test::call_service(&app, req).await;
        let body: DirectoryNode = test::read_body_json(resp).await;

        // Check that folder2 has a subfolder
        let folder2 = body
            .children
            .as_ref()
            .unwrap()
            .iter()
            .find(|c| c.name == "folder2")
            .unwrap();
        assert!(folder2.children.is_some());
        assert_eq!(folder2.children.as_ref().unwrap().len(), 1);
        assert_eq!(folder2.children.as_ref().unwrap()[0].name, "subfolder");
    }

    #[actix_web::test]
    async fn test_upload_files_empty_request() {
        let temp_dir = create_test_directory();
        let music_path = temp_dir.path().to_str().unwrap().to_string();

        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_path.clone()),
            download_root: Arc::new(music_path.clone()),
            preview_root: Arc::new(
                std::path::PathBuf::from(&music_path)
                    .join(".preview")
                    .to_string_lossy()
                    .to_string(),
            ),
            db_conn: establish_connection(&music_path).await.unwrap(),
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        let app = test::init_service(App::new().app_data(app_state).service(upload_files)).await;

        // Test with no files - should return error
        let req = test::TestRequest::post()
            .uri("/api/files/upload")
            .to_request();

        let resp = test::call_service(&app, req).await;

        // Should get a response (either bad request or success with empty result)
        assert!(resp.status().is_client_error() || resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_upload_endpoint_service_exists() {
        let temp_dir = create_test_directory();
        let music_path = temp_dir.path().to_str().unwrap().to_string();

        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_path.clone()),
            download_root: Arc::new(music_path.clone()),
            preview_root: Arc::new(
                std::path::PathBuf::from(&music_path)
                    .join(".preview")
                    .to_string_lossy()
                    .to_string(),
            ),
            db_conn: establish_connection(&music_path).await.unwrap(),
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
        });

        let app = test::init_service(App::new().app_data(app_state).service(upload_files)).await;

        // Just verify the endpoint exists and responds
        let req = test::TestRequest::post()
            .uri("/api/files/upload")
            .to_request();

        let resp = test::call_service(&app, req).await;
        // Should get some response (not 404)
        assert_ne!(resp.status(), StatusCode::NOT_FOUND);
    }
}
