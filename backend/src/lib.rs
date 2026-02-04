//! Kaulan Music Player - Backend Library
//!
//! This is the main library for the Kaulan music player backend.
//! It provides HTTP API endpoints, database operations, and file management.

// Re-export public API for main.rs and external use
pub use database::establish_connection;
pub use config::load_config;
pub use server::{ServerInfo, start_server};
pub use services::scanner::{initialize_database, update_database};

// Re-export types for external use
pub use types::AppState;

// Re-export all handlers for integration tests
pub use server::{
    get_music, get_all_music,
    get_all_playlists, get_playlist,
    get_all_collections, create_collection, delete_collection, get_collection,
    get_collection_items, add_to_collection, remove_from_collection,
    get_music_directory, set_music_directory,
    get_directory_tree, upload_files,
    update_database_endpoint, get_playlists_collection_mode,
};

// Declare modules
pub mod config;
pub mod types;
pub mod handlers;
pub mod services;
pub mod server;

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
