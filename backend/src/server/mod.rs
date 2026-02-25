//! Server startup and configuration.
//!
//! This module provides the HTTP server startup functionality.

use actix_web::{web, App, HttpServer};
use actix_cors::Cors;
use std::env;
use std::sync::Arc;
use std::path::PathBuf;
use tracing::{info, error};
use tokio::sync::Mutex as TokioMutex;

use crate::types::AppState;
use crate::config;
use crate::database::establish_connection;
use crate::services::scanner;
use crate::entities::music::Entity as MusicEntity;
use sea_orm::EntityTrait;
use crate::handlers::music;
use crate::handlers::playlists;
use crate::handlers::collections;
use crate::handlers::settings;
use crate::handlers::upload;
use crate::handlers::database;
use crate::handlers::lyrics;

// Re-export handler modules for convenience and for integration tests
pub use music::{get_music, get_all_music};
pub use playlists::{get_all_playlists, get_playlist};
pub use collections::{
    get_all_collections,
    create_collection,
    delete_collection,
    get_collection,
    get_collection_items,
    add_to_collection,
    remove_from_collection,
};
pub use settings::{get_music_directory, set_music_directory};
pub use upload::{get_directory_tree, upload_files};
pub use database::{update_database_endpoint, get_playlists_collection_mode};
pub use lyrics::{get_lyrics};

/// Represents the server address information
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub ip: String,
    pub port: u16,
}

impl ServerInfo {
    /// Returns the full base URL for the server
    pub fn url(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

/// Starts the backend HTTP server
///
/// The server is spawned in the background and this function returns immediately with ServerInfo.
/// For CLI use where you want to wait for the server, you would typically use a different approach
/// or keep the main thread alive.
///
/// # Arguments
/// * `cli_path` - Optional path from CLI argument. If provided, overrides config file.
///
/// # Music Directory Priority
/// 1. CLI argument (if provided)
/// 2. Config file (if exists)
/// 3. Environment variable `KAULAN_MUSIC_DIR`
///
/// # Returns
/// A `ServerInfo` containing the IP address and port the server is running on
///
/// # Errors
/// Returns an error if:
/// - No music directory is configured (no CLI arg, no config file, no env var)
/// - Database connection fails
pub async fn start_server(cli_path: Option<String>) -> Result<ServerInfo, Box<dyn std::error::Error>> {
    // Priority: CLI arg > Config file > Environment variable
    let music_path = if let Some(path) = cli_path {
        // CLI argument provided - use it (highest priority)
        info!("Using music directory from CLI argument: {}", path);
        path
    } else if let Some(path) = config::load_config() {
        // Config file has music directory
        info!("Using music directory from config file: {}", path);
        path
    } else if let Ok(path) = env::var("KAULAN_MUSIC_DIR") {
        // Environment variable set
        info!("Using music directory from environment variable: {}", path);
        path
    } else {
        // No music directory configured - abort
        error!("No music directory configured!");
        error!("Please provide music directory via:");
        error!("  1. CLI argument: {} run <music_path>", env::args().next().unwrap_or_else(|| "kaulan".to_string()));
        error!("  2. Config file: {}/config.json", config::get_config_dir().unwrap_or_else(|| PathBuf::from("~/.config/kaulan")).display());
        error!("  3. Environment variable: KAULAN_MUSIC_DIR");
        return Err("No music directory configured. Use CLI argument, config file, or KAULAN_MUSIC_DIR environment variable.".into());
    };

    info!("Connecting to database...");
    let db_conn = match establish_connection(&music_path).await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return Err(Box::new(e));
        }
    };
    info!("Database connection established");

    // Create the scan lock that will block playlist endpoints until scan completes
    let scan_lock = Arc::new(TokioMutex::new(()));

    let initial_scan_done = match scanner::get_initial_scan_done(&db_conn).await {
        Ok(done) => done,
        Err(e) => {
            error!("Failed to read startup scan flag: {}", e);
            false
        }
    };

    info!("Scanning music files from: {}", music_path);

    // Acquire the scan lock before spawning the scan task
    // This lock will be held during the entire scan, blocking API requests
    let scan_lock_for_scan = scan_lock.clone();
    let db_conn_for_scan = db_conn.clone();
    let music_path_for_scan = music_path.clone();

    if !initial_scan_done {
        // Spawn database scan as background task with lock held
        tokio::spawn(async move {
            let _guard = scan_lock_for_scan.lock().await;
            if let Err(e) = scanner::initialize_database(&music_path_for_scan, &db_conn_for_scan).await {
                error!("Failed to initialize database: {}", e);
            } else if let Err(e) = scanner::set_initial_scan_done(&db_conn_for_scan, true).await {
                error!("Failed to update startup scan flag: {}", e);
            }
            match MusicEntity::find().all(&db_conn_for_scan).await {
                Ok(music_list) => {
                    info!("Found {} music files in database", music_list.len());
                }
                Err(e) => {
                    error!("Failed to count music files: {}", e);
                }
            }
            drop(_guard);  // Release lock when scan completes
        });
    } else {
        info!("Skipping startup scan (initial scan already completed). Use Update Database to rescan.");
    }

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(music_path.clone()),
        db_conn,
        scan_lock,
    });

    let ip = "0.0.0.0".to_string();
    let port = 2080;
    let ip_clone = ip.clone();

    info!("Starting HTTP server on {}:{}", ip, port);

    // Spawn the server in the background using tokio (this works around Send issues)
    tokio::spawn(async move {
        match HttpServer::new(move || {
            let cors = Cors::default()
                .allow_any_origin()
                .allow_any_method()
                .allow_any_header()
                .max_age(3600);

            App::new()
                .wrap(cors)
                .app_data(app_state.clone())
                // Music endpoints
                .service(get_music)
                .service(get_all_music)
                // Lyrics endpoints
                .service(get_lyrics)
                // Playlist endpoints (order matters - specific routes before parameterized ones)
                .service(get_playlists_collection_mode)
                .service(get_all_playlists)
                .service(get_playlist)
                // Collection endpoints (order matters - longer paths first)
                .service(get_collection_items)
                .service(get_all_collections)
                .service(create_collection)
                .service(delete_collection)
                .service(get_collection)
                .service(add_to_collection)
                .service(remove_from_collection)
                // Settings endpoints
                .service(get_music_directory)
                .service(set_music_directory)
                // Database endpoints
                .service(update_database_endpoint)
                // File upload endpoints
                .service(get_directory_tree)
                .service(upload_files)
        })
        .bind((ip_clone, port))
        .unwrap()
        .run()
        .await
        {
            Ok(_) => info!("Server shutdown gracefully"),
            Err(e) => error!("Server error: {}", e),
        }
    });

    Ok(ServerInfo { ip, port })
}
