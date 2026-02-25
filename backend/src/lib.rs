//! Kaulan Music Player - Backend Library
//!
//! This is the main library for the Kaulan music player backend.
//! It provides HTTP API endpoints, database operations, and file management.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

// Re-export public API for main.rs and external use
pub use database::establish_connection;
pub use config::load_config;
pub use server::{ServerInfo, start_server};
pub use services::scanner::{initialize_database, update_database};

// Re-export types for external use
pub use types::AppState;

// Re-export file operations for Android MediaStore integration
pub mod file_ops;
pub use file_ops::{set_file_reader, set_music_file_lister, FileReader, MusicFileLister, MusicFileInfo, SUPPORTED_EXTENSIONS};

// Re-export log broadcast types
pub use log_broadcast::{LogBroadcaster, create_broadcast_layer, start_log_server};

// Re-export all handlers for integration tests
pub use server::{
    get_music, get_all_music,
    get_all_playlists, get_playlist,
    get_all_collections, create_collection, delete_collection, get_collection,
    get_collection_items, add_to_collection, remove_from_collection,
    get_music_directory, set_music_directory,
    get_directory_tree, upload_files,
    update_database_endpoint, get_playlists_collection_mode,
    get_lyrics,
};

/// Global broadcaster for log streaming (initialized once)
static GLOBAL_BROADCASTER: OnceLock<Arc<LogBroadcaster>> = OnceLock::new();

/// Static flag to ensure tracing is initialized only once
static TRACING_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize tracing subscriber for logging with broadcast support
///
/// This function uses lazy initialization - it will only initialize the tracing
/// subscriber once, regardless of how many times it's called. Subsequent calls
/// will return the existing broadcaster.
///
/// # Returns
/// The log broadcaster that can be used to start the TCP streaming server
///
/// # Example
/// ```rust,no_run
/// use kaulan::init_tracing;
///
/// let broadcaster = init_tracing();
/// // Start the log streaming server
/// tokio::spawn(kaulan::start_log_server(broadcaster));
/// ```
pub fn init_tracing() -> Arc<LogBroadcaster> {
    // Use a OnceLock to ensure we only initialize once
    GLOBAL_BROADCASTER.get_or_init(|| {
        // Double-check the atomic flag for extra safety
        if TRACING_INITIALIZED.load(Ordering::SeqCst) {
            // This shouldn't happen, but if it does, create a new broadcaster
            return Arc::new(LogBroadcaster::new(256));
        }

        // Import needed for tracing setup
        use tracing_subscriber::util::SubscriberInitExt;
        use tracing_subscriber::prelude::*;

        // Set default log level from RUST_LOG env var, or default to debug
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug"));

        // Create the log broadcaster
        let broadcaster = Arc::new(LogBroadcaster::new(256));

        // Create the broadcast layer
        let broadcast_layer = create_broadcast_layer(broadcaster.clone());

        // Build the subscriber with both console and broadcast layers
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .with(broadcast_layer)
            .init();

        // Mark as initialized
        TRACING_INITIALIZED.store(true, Ordering::SeqCst);

        broadcaster
    }).clone()
}

// Declare modules
pub mod config;
pub mod types;
pub mod handlers;
pub mod services;
pub mod server;
pub mod middleware;

// Existing modules (unchanged)
pub mod lufsgen;
pub mod entities;
pub mod database;
pub mod log_broadcast;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DirectoryNode;
    use actix_web::{test, App, http::StatusCode, web};
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
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(temp_dir.path().to_str().unwrap().to_string()),
            db_conn: establish_connection(temp_dir.path().to_str().unwrap()).await.unwrap(),
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
        });

        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(get_directory_tree)
        ).await;

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
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_path.clone()),
            db_conn: establish_connection(&music_path).await.unwrap(),
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
        });

        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(get_directory_tree)
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/files/directory-tree")
            .to_request();

        let resp = test::call_service(&app, req).await;
        let body: DirectoryNode = test::read_body_json(resp).await;

        // Check that folder2 has a subfolder
        let folder2 = body.children.as_ref()
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

        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_path.clone()),
            db_conn: establish_connection(&music_path).await.unwrap(),
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
        });

        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(upload_files)
        ).await;

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

        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_path.clone()),
            db_conn: establish_connection(&music_path).await.unwrap(),
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
        });

        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(upload_files)
        ).await;

        // Just verify the endpoint exists and responds
        let req = test::TestRequest::post()
            .uri("/api/files/upload")
            .to_request();

        let resp = test::call_service(&app, req).await;
        // Should get some response (not 404)
        assert_ne!(resp.status(), StatusCode::NOT_FOUND);
    }
}
