use std::fs;
use std::sync::Mutex;
use tauri::{Manager, State, Emitter};
use serde_json::json;

// MediaStore adapter module
mod mediastore_adapter;

// Music notification plugin
use tauri_plugin_music_notification_api::{Server, set_server};

// JNI bindings for Android (needed for music notification plugin)
#[cfg(target_os = "android")]
mod jni_wrappers {
    // Android log binding for debugging
    #[allow(non_upper_case_globals)]
    #[no_mangle]
    pub extern "C" fn Java_com_plugin_music_1notification_MusicPlayerService_serverStart(
        _env: jni::JNIEnv,
        _this: jni::objects::JObject,
    ) -> i32 {
        log::info!("JNI: serverStart called from Kaulan app");
        // Call the plugin's server_start which will invoke the registered Server trait
        tauri_plugin_music_notification_api::server_start()
    }

    #[no_mangle]
    pub extern "C" fn Java_com_plugin_music_1notification_MusicPlayerService_serverStop(
        _env: jni::JNIEnv,
        _this: jni::objects::JObject,
    ) -> i32 {
        log::info!("JNI: serverStop called from Kaulan app");
        tauri_plugin_music_notification_api::server_stop()
    }
}

// HTTP server implementation that implements the Server trait
struct KaulanServer {
    running: std::sync::atomic::AtomicBool,
    music_dir: Mutex<Option<String>>,
    data_dir: Mutex<Option<String>>,
}

impl Server for KaulanServer {
    fn start(self: std::sync::Arc<Self>) -> Result<(), String> {
        log::info!("KaulanServer trait: start() called from foreground service");

        // Get the music directory and data directory
        let music_dir = self.music_dir.lock().unwrap().clone();
        let data_dir = self.data_dir.lock().unwrap().clone();

        // Set environment variables for Android support
        #[cfg(target_os = "android")]
        {
            std::env::set_var("TAURI_PLATFORM", "android");
            if let Some(ref dir) = data_dir {
                std::env::set_var("TAURI_ANDROID_DATA_DIR", dir);
                log::info!("Set Android data directory: {}", dir);
            }
        }

        // Spawn the HTTP server in a background task
        let server_handle = self.clone();
        let server_handle_for_loop = server_handle.clone();
        std::thread::spawn(move || {
            log::info!("Spawning HTTP server from foreground service context");

            // Create a new async runtime for this thread
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    log::error!("Failed to create runtime: {}", e);
                    return;
                }
            };

            // For Android, use /storage as default music directory
            // For other platforms, use None (will read from config file)
            let music_dir_arg = if cfg!(target_os = "android") {
                music_dir.or_else(|| Some("/storage".to_string()))
            } else {
                None
            };

            rt.block_on(async move {
                match kaulan::start_server(music_dir_arg).await {
                    Ok(server_info) => {
                        log::info!("HTTP server started on: http://{}", server_info.url());
                        server_handle.running.store(true, std::sync::atomic::Ordering::Release);
                    }
                    Err(e) => {
                        log::error!("Failed to start HTTP server: {}", e);
                    }
                }
            });

            // Keep the thread alive to maintain the server
            log::info!("Server thread running, keeping alive for foreground service");
            loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                if !server_handle_for_loop.running.load(std::sync::atomic::Ordering::Acquire) {
                    log::info!("Server marked as stopped, exiting thread");
                    break;
                }
            }
        });

        // Mark as running immediately - the server is starting in background
        self.running.store(true, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    fn stop(self: std::sync::Arc<Self>) -> Result<(), String> {
        log::info!("KaulanServer trait: stop() called");
        self.running.store(false, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    fn is_running(self: std::sync::Arc<Self>) -> bool {
        self.running.load(std::sync::atomic::Ordering::Acquire)
    }
}

// State to hold the current music directory
struct MusicDirectory(Mutex<String>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing first (must happen before Tauri setup to avoid conflicts)
    kaulan::init_tracing();

    tauri::Builder::default()
        .manage(MusicDirectory(Mutex::new(String::new())))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_android_mediastore::init())
        .plugin(tauri_plugin_music_notification_api::init())
        .setup(|app| {
            log::info!("======================= Start =======================");

            // Read config from Tauri's app data directory for UI display purposes
            let app_handle = app.handle().clone();
            let app_handle_for_config = app_handle.clone();
            let music_path = tauri::async_runtime::block_on(async move {
                // Try to load from config using Tauri's path API and std::fs
                let path_resolver = app_handle_for_config.path();
                if let Ok(config_dir) = path_resolver.app_config_dir() {
                    let config_path = config_dir.join("config.json");
                    if config_path.exists() {
                        if let Ok(content) = fs::read_to_string(&config_path) {
                            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(path) = config.get("music_directory").and_then(|v| v.as_str()) {
                                    log::info!("Loaded music directory from config: {}", path);
                                    return path.to_string();
                                }
                            }
                        }
                    }
                }

                // No config file - the backend will abort, but we store empty string for UI
                log::warn!("No config file found, backend startup may fail");
                String::new()
            });

            // Store in state for UI display
            let state = app.state::<MusicDirectory>();
            *state.0.lock().unwrap() = music_path.clone();

            // Set up custom file operations implementations for Android
            #[cfg(target_os = "android")]
            {
                log::info!("Setting up MediaStore adapters for Android");
                let app_handle_for_adapter = app.handle().clone();
                let _ = kaulan::set_file_reader(Box::new(mediastore_adapter::MediaStoreFileReader::new(app_handle_for_adapter.clone())));
                let _ = kaulan::set_music_file_lister(Box::new(mediastore_adapter::MediaStoreMusicFileLister::new(app_handle_for_adapter)));
                log::info!("MediaStore adapters configured successfully");
            }

            // Prepare data directory config for server startup
            let data_dir_for_server = if cfg!(target_os = "android") {
                app_handle.path().app_data_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            } else {
                None
            };

            // Register the HTTP server implementation with the music notification plugin
            let kaulan_server = std::sync::Arc::new(KaulanServer {
                running: std::sync::atomic::AtomicBool::new(false),
                music_dir: Mutex::new(if cfg!(target_os = "android") { Some("/storage".to_string()) } else { None }),
                data_dir: Mutex::new(data_dir_for_server),
            });
            set_server(kaulan_server.clone());
            log::info!("Registered KaulanServer with music notification plugin");

            // Emit the server library name for auto-registration
            // IMPORTANT: The frontend MUST call setServer() BEFORE calling startService()
            // Otherwise the service will use the default "musicnotification_lib" name
            #[cfg(target_os = "android")]
            {
                let lib_name = env!("COMPILED_LIB_NAME");
                log::info!("Emitting server_library_name event: {} - frontend MUST call setServer() before startService()", lib_name);
                let _ = app.emit("server_library_name", lib_name);
                log::info!("On Android: Server will be started by foreground service via serverStart() JNI call");
            }

            // Start the backend server directly ONLY on desktop
            // On Android, the foreground service will start it via serverStart()
            #[cfg(not(target_os = "android"))]
            {
                let _handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    log::info!("Starting backend server (desktop mode, will use config file)");

                    match kaulan::start_server(None).await {
                        Ok(server_info) => {
                            log::info!("Backend server started on: http://{}", server_info.url());
                        }
                        Err(e) => {
                            log::error!("Failed to start backend server: {}", e);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_music_directory,
            set_music_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Get the current music directory
#[tauri::command]
fn get_music_directory(state: State<'_, MusicDirectory>) -> Result<String, String> {
    let path = state.0.lock().unwrap().clone();
    Ok(path)
}

/// Set a new music directory (saved to config, takes effect on restart)
#[tauri::command]
fn set_music_directory(
    app: tauri::AppHandle,
    new_path: String,
) -> Result<(), String> {
    log::info!("Music directory change requested to: {}", new_path);

    // Validate the path exists and is a directory
    if !std::path::Path::new(&new_path).exists() {
        return Err(format!("Path does not exist: {}", new_path));
    }
    if !std::path::Path::new(&new_path).is_dir() {
        return Err(format!("Path is not a directory: {}", new_path));
    }

    // Save config file using Tauri's path API and std::fs
    let path_resolver = app.path();
    let config_dir = path_resolver.app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;

    // Create config directory if needed
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    }

    let config_path = config_dir.join("config.json");
    let config = json!({
        "music_directory": new_path
    });

    fs::write(&config_path, config.to_string()).map_err(|e| e.to_string())?;

    log::info!("Music directory saved to config: {}", new_path);
    Ok(())
}
