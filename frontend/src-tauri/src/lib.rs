use std::env;
use std::sync::Mutex;
use tauri::State;

// State to hold the current music directory
struct MusicDirectory(Mutex<String>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Get initial music directory path
    let initial_music_path = env::var("HOME")
        .map(|h| format!("{}/Music", h))
        .unwrap_or_else(|_| String::from("./music"));

    tauri::Builder::default()
        .manage(MusicDirectory(Mutex::new(initial_music_path.clone())))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Start the backend server
            let _handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                log::info!("Starting backend server with music path: {}", initial_music_path);

                match kaulan::start_server(initial_music_path).await {
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

/// Set a new music directory and restart the backend
#[tauri::command]
async fn set_music_directory(
    state: State<'_, MusicDirectory>,
    new_path: String,
) -> Result<(), String> {
    log::info!("Updating music directory to: {}", new_path);

    // Validate the path exists and is a directory
    if !std::path::Path::new(&new_path).exists() {
        return Err(format!("Path does not exist: {}", new_path));
    }
    if !std::path::Path::new(&new_path).is_dir() {
        return Err(format!("Path is not a directory: {}", new_path));
    }

    // Update the stored music directory
    {
        let mut path = state.0.lock().unwrap();
        *path = new_path.clone();
    }

    // Start a new backend server with the new path
    // Note: The old server will continue running in the background
    // This is a known limitation that could be addressed in a future update
    tauri::async_runtime::spawn(async move {
        log::info!("Starting new backend server with music path: {}", new_path);
        match kaulan::start_server(new_path).await {
            Ok(server_info) => {
                log::info!("New backend server started on: http://{}", server_info.url());
            }
            Err(e) => {
                log::error!("Failed to start new backend server: {}", e);
            }
        }
    });

    Ok(())
}
