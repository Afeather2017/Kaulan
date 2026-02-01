use std::env;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
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
                // Get music directory path
                let music_path = env::var("HOME")
                    .map(|h| format!("{}/Music", h))
                    .unwrap_or_else(|_| String::from("./music"));

                log::info!("Starting backend server with music path: {}", music_path);

                match kaulan::start_server(music_path).await {
                    Ok(server_info) => {
                        log::info!("Backend server started on: {}", server_info.url());
                    }
                    Err(e) => {
                        log::error!("Failed to start backend server: {}", e);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
