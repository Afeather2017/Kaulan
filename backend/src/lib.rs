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
pub use services::scanner::{initialize_database, update_database};

// Re-export types for external use
pub use types::AppState;

// Re-export file operations for Android MediaStore integration
pub mod file_ops;
pub use file_ops::{
    clear_scan_backends, register_scan_backend, set_android_sources, set_file_reader,
    set_lyric_reader, set_music_file_lister, FileReader, LyricReader, MusicFileInfo,
    MusicFileLister, ReadSeekSendSync, ScanBackend, StdFsScanBackend, SUPPORTED_EXTENSIONS,
};

/// Environment variable that carries the cold-start launch file path.
///
/// Set by the Tauri shell before [`start_server`] runs (see
/// `frontend/src-tauri/src/lib.rs`), drained into [`launch_broker`] during
/// backend startup. See `docs/default-music-app.md`.
pub const LAUNCH_FILE_ENV: &str = "KAULAN_LAUNCH_FILE";

/// Singleton broker holding the pending launch file and SSE subscribers.
///
/// One instance lives at the crate root ([`launch_broker`]) and is the single
/// source of truth for both warm-start calls from the Tauri shell's
/// single-instance plugin callback and the cold-start env-var seed.
///
/// Subscribers are managed via a `tokio::sync::broadcast` channel: dropped
/// receivers are pruned automatically by the broadcast runtime, and a slow
/// receiver lags (skipping events) rather than being disconnected by
/// backpressure. Since each event is a single `()` ping, lag is harmless.
///
/// See `handlers/launch` and `docs/default-music-app.md`.
pub struct LaunchBroker {
    path: std::sync::Mutex<Option<String>>,
    display_name: std::sync::Mutex<Option<String>>,
    notify_tx: tokio::sync::broadcast::Sender<()>,
}

impl LaunchBroker {
    fn new() -> Self {
        let (notify_tx, _) = tokio::sync::broadcast::channel(8);
        Self {
            path: std::sync::Mutex::new(None),
            display_name: std::sync::Mutex::new(None),
            notify_tx,
        }
    }

    /// Stash a new launch path and notify all SSE subscribers.
    ///
    /// `display_name` carries an optional friendly filename (e.g. the
    /// `_display_name` Android's ContentResolver returns for a `content://`
    /// URI). Desktop leaves it `None` — the path itself ends in a filename.
    ///
    /// `broadcast::send` returns `Err` only when there are no receivers, which
    /// is a no-op for our purposes (nothing to notify).
    pub fn set_path(&self, path: String, display_name: Option<String>) {
        {
            let mut guard = self.path.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(path);
        }
        {
            let mut guard = self.display_name.lock().unwrap_or_else(|e| e.into_inner());
            *guard = display_name;
        }
        let _ = self.notify_tx.send(());
    }

    /// Atomically take (clear) the stashed path. Returns `None` if nothing
    /// pending or already consumed.
    pub fn take_path(&self) -> Option<String> {
        self.path.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    /// Atomically take (clear) the stashed display name. Returns `None` if
    /// nothing was stashed (desktop cold-start, or no name available).
    pub fn take_display_name(&self) -> Option<String> {
        self.display_name
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    /// Register a new SSE subscriber. Each live subscriber receives a `()`
    /// notification on every [`set_path`] call until the returned receiver is
    /// dropped. The broadcast runtime prunes dropped receivers automatically —
    /// no manual retain() pass is needed on `set_path`.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<()> {
        self.notify_tx.subscribe()
    }
}

static LAUNCH_BROKER: std::sync::OnceLock<LaunchBroker> = std::sync::OnceLock::new();

/// Access the singleton [`LaunchBroker`], initializing it on first call.
pub fn launch_broker() -> &'static LaunchBroker {
    LAUNCH_BROKER.get_or_init(LaunchBroker::new)
}

/// Stash a launch file path for the frontend to consume.
///
/// Called by the Tauri shell's `tauri-plugin-single-instance` callback on
/// warm-start launches. Also called once by [`start_server`] during cold start
/// to drain the `KAULAN_LAUNCH_FILE` env var seed.
///
/// Equivalent to [`set_pending_launch_file_with_name`] with `display_name=None`
/// — desktop paths already end in a filename the frontend can derive, so no
/// friendly name is needed.
///
/// See `docs/default-music-app.md`.
pub fn set_pending_launch_file(path: String) {
    launch_broker().set_path(path, None);
}

/// Stash a launch file path together with a friendly display name.
///
/// Used by Android, where the launch URI is typically a `content://` URI whose
/// last path segment is a numeric id — the frontend can't derive a useful
/// filename from it, so MainActivity queries `_display_name` via ContentResolver
/// and forwards it here.
///
/// See `docs/default-music-app.md`.
pub fn set_pending_launch_file_with_name(path: String, display_name: Option<String>) {
    launch_broker().set_path(path, display_name);
}

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
