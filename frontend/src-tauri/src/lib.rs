use std::fs;
use std::sync::Mutex;
use tauri::{Manager, State};
use serde_json::json;

// State to hold the current music directory
struct MusicDirectory(Mutex<String>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(MusicDirectory(Mutex::new(String::new())))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

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

            // Start the backend server with no CLI argument (uses config file)
            let _handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                log::info!("Starting backend server (will use config file)");
                match kaulan::start_server(None).await {
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
