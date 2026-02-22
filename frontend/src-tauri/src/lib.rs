use std::fs;
use std::sync::Mutex;
use tauri::{Manager, State};
use serde_json::json;

// MediaStore adapter module
mod mediastore_adapter;

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
        .setup(|app| {
            log::info!("======================= Start =======================");

            // Read config from Tauri's app data directory for UI display purposes
            let app_handle = app.handle().clone();
            let music_path = tauri::async_runtime::block_on(async move {
                // Try to load from config using Tauri's path API and std::fs
                let path_resolver = app_handle.path();
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

            // Start the backend server with no CLI argument (uses config file)
            let _handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                log::info!("Starting backend server (will use config file)");

                // Set environment variables for Android support
                #[cfg(target_os = "android")]
                {
                    std::env::set_var("TAURI_PLATFORM", "android");
                    // For Android, we need to use the app's data directory for the database
                    if let Ok(data_dir) = _handle.path().app_data_dir() {
                        let data_dir_str = data_dir.to_string_lossy().to_string();
                        std::env::set_var("TAURI_ANDROID_DATA_DIR", &data_dir_str);
                        log::info!("Set Android data directory: {}", data_dir_str);
                    }
                }

                // For Android, use /storage as default music directory
                // For other platforms, use None (will read from config file)
                #[cfg(target_os = "android")]
                let music_dir_arg = Some("/storage".to_string());
                #[cfg(not(target_os = "android"))]
                let music_dir_arg = None;

                match kaulan::start_server(music_dir_arg).await {
                    Ok(server_info) => {
                        log::info!("Backend server started on: http://{}", server_info.url());
                    }
                    Err(e) => {
                        log::error!("Failed to start backend server: {}", e);
                    }
                }
            });

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
