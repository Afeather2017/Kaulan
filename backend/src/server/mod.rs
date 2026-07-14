//! Server startup and configuration.
//!
//! This module provides the HTTP server startup functionality.

use actix_cors::Cors;
use actix_files::NamedFile;
use actix_web::{route, web, App, HttpRequest, HttpResponse, HttpServer};
use std::env;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info, warn};

use crate::config;
use crate::database::establish_connection;
use crate::handlers::database;
use crate::handlers::discovery;
use crate::handlers::download;
use crate::handlers::library_import;
use crate::handlers::lufs;
use crate::handlers::lyrics;
use crate::handlers::music;
use crate::handlers::playlists;
use crate::handlers::settings;
use crate::handlers::upload;
use crate::services::download as download_service;
use crate::types::AppState;

// Re-export handler modules for convenience and for integration tests
pub use database::update_database_endpoint;
pub use discovery::{
    finish_discovery_scan, get_discovered_devices, get_self_device, request_discovery_once,
    set_device_name, start_discovery_scan,
};
pub use download::{
    apply_lyric, create_download_job, download_preview, download_track, get_bilibili_thumbnail,
    get_download_directory_tree, get_download_job, get_download_jobs, get_online_provider_statuses,
    get_preview_track, search_lyrics, search_online,
};
pub use library_import::import_from_remote;
pub use lufs::precache_lufs;
pub use lyrics::{get_lyrics, get_lyrics_by_id, update_lyrics_by_id};
pub use music::{delete_music_batch, get_all_music, get_music, get_music_by_id, get_music_cover};
pub use playlists::{get_all_playlists, get_playlist};
pub use settings::{get_media_types, get_music_directory, set_media_types, set_music_directory};
pub use upload::{get_directory_tree, upload_files};

/// Static frontend files served by the backend.
///
/// See docs/static-frontend-serving.md for the request flow and deployment
/// layout. `dist_dir` is optional so API-only development can keep running
/// before `frontend/npm run build` has produced `frontend/dist`.
#[derive(Debug, Clone)]
pub struct StaticFrontendConfig {
    pub dist_dir: Option<PathBuf>,
}

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

/// Resolve the Vue production build directory for backend static hosting.
///
/// The optional `KAULAN_FRONTEND_DIST` environment variable can point to a
/// custom build output directory. Without it, the backend checks common
/// development and release working directories.
pub fn resolve_frontend_dist() -> Option<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(path) = env::var("KAULAN_FRONTEND_DIST") {
        candidates.push(PathBuf::from(path));
    }

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("frontend/dist"));
        candidates.push(current_dir.join("../frontend/dist"));
    }

    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend/dist"));

    candidates
        .into_iter()
        .find(|candidate| candidate.join("index.html").is_file())
}

fn static_frontend_file(dist_dir: &Path, requested_path: &str) -> Option<PathBuf> {
    let requested_path = requested_path.trim_start_matches('/');
    let mut relative_path = PathBuf::new();
    let mut is_asset_path = false;

    if requested_path.is_empty() {
        relative_path.push("index.html");
    } else {
        for (index, component) in Path::new(requested_path).components().enumerate() {
            match component {
                Component::Normal(part) => {
                    if index == 0 && part == "assets" {
                        is_asset_path = true;
                    }
                    relative_path.push(part);
                }
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            }
        }
    }

    let candidate = dist_dir.join(relative_path);
    if candidate.is_dir() {
        let index_file = candidate.join("index.html");
        if index_file.is_file() {
            return Some(index_file);
        }
    }

    if candidate.is_file() {
        return Some(candidate);
    }

    if is_asset_path || Path::new(requested_path).extension().is_some() {
        return None;
    }

    let index_file = dist_dir.join("index.html");
    index_file.is_file().then_some(index_file)
}

/// Serve the built Vue frontend from `frontend/dist`.
///
/// API routes are intentionally excluded so unknown `/api/...` requests still
/// behave like API 404s instead of returning the SPA shell.
#[route("/{path:.*}", method = "GET", method = "HEAD")]
pub async fn serve_static_frontend(
    request: HttpRequest,
    path: web::Path<String>,
    config: web::Data<StaticFrontendConfig>,
) -> actix_web::Result<HttpResponse> {
    let request_path = request.path();
    if request_path == "/api" || request_path.starts_with("/api/") {
        return Ok(HttpResponse::NotFound().finish());
    }

    let Some(dist_dir) = config.dist_dir.as_deref() else {
        return Ok(HttpResponse::NotFound().body(
            "Frontend build not found. Run `npm run build` in frontend/ or set KAULAN_FRONTEND_DIST.",
        ));
    };

    let Some(file_path) = static_frontend_file(dist_dir, &path.into_inner()) else {
        return Ok(HttpResponse::NotFound().finish());
    };

    let file = NamedFile::open_async(file_path).await?;
    Ok(file.into_response(&request))
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
pub async fn start_server(
    cli_path: Option<String>,
) -> Result<ServerInfo, Box<dyn std::error::Error>> {
    download_service::initialize_runtime().map_err(|err| {
        std::io::Error::other(format!("download runtime initialization failed: {err}"))
    })?;

    // Priority: CLI arg > Config file > Environment variable > Platform default
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
    } else if cfg!(target_os = "android") {
        // Android: use /storage as default (covers both internal storage and SD card)
        let default_path = "/storage".to_string();
        info!(
            "Using default music directory for Android: {}",
            default_path
        );
        default_path
    } else {
        // Desktop: try ~/Music first, then ./music as fallback
        let home_dir = dirs::home_dir();
        let music_dir = home_dir.as_ref().map(|h| h.join("Music"));

        if let Some(ref music_path) = music_dir {
            if music_path.exists() && music_path.is_dir() {
                let path_str = music_path.to_string_lossy().to_string();
                info!("Using default music directory ~/Music: {}", path_str);
                path_str
            } else {
                // Try ./music as fallback
                let local_music = PathBuf::from("./music");
                if local_music.exists() && local_music.is_dir() {
                    let path_str = local_music.to_string_lossy().to_string();
                    info!("Using ./music as default music directory: {}", path_str);
                    path_str
                } else {
                    // No music directory configured - abort
                    error!("No music directory configured!");
                    error!("Please provide music directory via:");
                    error!(
                        "  1. CLI argument: {} run <music_path>",
                        env::args().next().unwrap_or_else(|| "kaulan".to_string())
                    );
                    error!(
                        "  2. Config file: {}/config.json",
                        config::get_config_dir()
                            .unwrap_or_else(|| PathBuf::from("~/.config/kaulan"))
                            .display()
                    );
                    error!("  3. Environment variable: KAULAN_MUSIC_DIR");
                    error!("");
                    error!("Default locations checked (none exist):");
                    error!("  - ~/Music");
                    error!("  - ./music");
                    return Err("No music directory configured. Use CLI argument, config file, or KAULAN_MUSIC_DIR environment variable.".into());
                }
            }
        } else {
            // No home directory found - abort
            error!("No music directory configured!");
            error!("Please provide music directory via:");
            error!(
                "  1. CLI argument: {} run <music_path>",
                env::args().next().unwrap_or_else(|| "kaulan".to_string())
            );
            error!(
                "  2. Config file: {}/config.json",
                config::get_config_dir()
                    .unwrap_or_else(|| PathBuf::from("~/.config/kaulan"))
                    .display()
            );
            error!("  3. Environment variable: KAULAN_MUSIC_DIR");
            return Err("No music directory configured. Use CLI argument, config file, or KAULAN_MUSIC_DIR environment variable.".into());
        }
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

    info!("Startup scan is handled by frontend via /api/database/update?startup=true (docs/startup-scan.md).");

    // Initialize device discovery
    let device_id = config::load_or_create_device_id();
    let device_name = config::get_configured_device_name()
        .or_else(config::get_hostname_device_name)
        .unwrap_or_else(|| config::generate_fallback_device_name(&device_id));
    info!("Device ID: {}", device_id);
    info!("Device name: {}", device_name);
    info!("Discovery mode: manual scan request/reply");

    // Create shared UDP socket for discovery (single socket for both send and receive)
    let discovery_socket: Option<Arc<UdpSocket>> =
        match crate::discovery::socket::create_discovery_socket().await {
            Some(socket) => Some(socket),
            None => {
                error!("Failed to create discovery socket, device discovery disabled");
                None
            }
        };

    let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
        device_id.clone(),
        device_name.clone(),
        2080, // API port
    ));

    // Start discovery listener if socket was created
    if let Some(socket) = discovery_socket {
        let discovery_state_clone = discovery_state.clone();
        tokio::spawn(async move {
            crate::discovery::discovery::start_discovery_listener(socket, discovery_state_clone)
                .await;
        });

        info!("Device discovery services started");
    }

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(music_path.clone()),
        download_root: Arc::new(
            env::var("KAULAN_DOWNLOAD_ROOT").unwrap_or_else(|_| music_path.clone()),
        ),
        preview_root: Arc::new(env::var("KAULAN_PREVIEW_ROOT").unwrap_or_else(|_| {
            std::env::temp_dir()
                .join("kaulan-preview")
                .to_string_lossy()
                .to_string()
        })),
        db_conn,
        scan_lock,
        download_jobs: Arc::new(download_service::DownloadJobStore::new()),
        discovery: discovery_state.clone(),
    });

    // Also add discovery state as separate app_data for discovery handlers
    let discovery_data = web::Data::new((*discovery_state).clone());
    let static_frontend_config = StaticFrontendConfig {
        dist_dir: resolve_frontend_dist(),
    };
    match &static_frontend_config.dist_dir {
        Some(dist_dir) => info!("Serving static frontend from {}", dist_dir.display()),
        None => warn!(
            "Static frontend build not found; backend API will still run. Build frontend/dist or set KAULAN_FRONTEND_DIST to serve the web app."
        ),
    }

    let ip = "0.0.0.0".to_string();
    let port = 2080;
    let ip_clone = ip.clone();

    info!("Starting HTTP server on {}:{}", ip, port);

    // Spawn the server in the background using tokio (this works around Send issues)
    tokio::spawn(async move {
        let server = match HttpServer::new(move || {
            let cors = Cors::default()
                .allow_any_origin()
                .allow_any_method()
                .allow_any_header()
                .max_age(3600);

            App::new()
                .wrap(cors)
                .app_data(app_state.clone())
                .app_data(discovery_data.clone())
                .app_data(web::Data::new(static_frontend_config.clone()))
                // Music endpoints (ID-based first, then filename-based)
                .service(get_music_cover)
                .service(get_music_by_id)
                .service(get_music)
                .service(get_all_music)
                .service(delete_music_batch)
                .service(precache_lufs)
                // Lyrics endpoints (ID-based first, then filename-based)
                .service(get_lyrics_by_id)
                .service(get_lyrics)
                .service(update_lyrics_by_id)
                // Playlist endpoints
                .service(get_all_playlists)
                .service(get_playlist)
                // Settings endpoints
                .service(get_music_directory)
                .service(set_music_directory)
                .service(get_media_types)
                .service(set_media_types)
                // Discovery endpoints (order matters - specific routes first)
                .service(get_discovered_devices)
                .service(get_self_device)
                .service(start_discovery_scan)
                .service(request_discovery_once)
                .service(finish_discovery_scan)
                .service(set_device_name)
                // Database endpoints
                .service(update_database_endpoint)
                // Online download endpoints
                .service(search_online)
                .service(get_online_provider_statuses)
                .service(get_bilibili_thumbnail)
                .service(search_lyrics)
                .service(apply_lyric)
                .service(get_download_directory_tree)
                .service(download_preview)
                .service(get_preview_track)
                .service(create_download_job)
                .service(get_download_jobs)
                .service(get_download_job)
                .service(download_track)
                // Remote-library import endpoint (Tauri runtimes)
                .service(import_from_remote)
                // File upload endpoints
                .service(get_directory_tree)
                .service(upload_files)
                // Static frontend endpoint. Keep this last so API routes take priority.
                .service(serve_static_frontend)
        })
        .bind((ip_clone.clone(), port))
        {
            Ok(server) => server,
            Err(e) => {
                error!("Failed to bind HTTP server on {}:{}: {}", ip_clone, port, e);
                return;
            }
        };

        match server.run().await {
            Ok(_) => info!("Server shutdown gracefully"),
            Err(e) => error!("Server error: {}", e),
        }
    });

    Ok(ServerInfo { ip, port })
}
