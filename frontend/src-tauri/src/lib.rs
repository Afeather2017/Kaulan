use serde_json::json;
use std::fs;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use tauri::{Manager, State};
#[cfg(target_os = "android")]
use tauri_plugin_android_external_storage::AndroidExternalStorageExt;

// MediaStore adapter module
mod android_media_adapter;
// Android wake lock module
#[cfg(target_os = "android")]
mod wakelock;

// Music notification plugin
use tauri_plugin_music_notification_api::{set_server, Server};

// HTTP server implementation that implements the Server trait
struct KaulanServer {
    running: std::sync::atomic::AtomicBool,
    music_dir: Mutex<Option<String>>,
    data_dir: Mutex<Option<String>>,
    #[cfg(target_os = "android")]
    wake_lock: Mutex<Option<wakelock::WakeLock>>,
}

impl KaulanServer {
    fn start_backend(self: &Arc<Self>, source: &str) -> Result<(), String> {
        if self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            log::info!("Backend server already running, skipping startup from {}", source);
            return Ok(());
        }

        let music_dir = self.music_dir.lock().unwrap().clone();

        #[cfg(target_os = "android")]
        {
            let data_dir = self.data_dir.lock().unwrap().clone();
            std::env::set_var("TAURI_PLATFORM", "android");
            if let Some(ref dir) = data_dir {
                std::env::set_var("TAURI_ANDROID_DATA_DIR", dir);
            }
        }

        let server_handle = self.clone();
        let keepalive_handle = self.clone();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    log::error!("Failed to create runtime: {}", e);
                    server_handle.running.store(false, Ordering::Release);
                    return;
                }
            };

            let music_dir_arg = if cfg!(target_os = "android") {
                music_dir.or_else(|| Some("/storage".to_string()))
            } else {
                None
            };

            rt.block_on(async move {
                if let Err(e) = kaulan::start_server(music_dir_arg).await {
                    log::error!("Failed to start HTTP server: {}", e);
                    server_handle.running.store(false, Ordering::Release);
                }
            });

            loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                if !keepalive_handle.running.load(Ordering::Acquire) {
                    break;
                }
            }
        });

        Ok(())
    }
}

impl Server for KaulanServer {
    fn library_name(&self) -> &str {
        "app_lib"
    }

    fn start(self: std::sync::Arc<Self>) -> Result<(), String> {
        self.start_backend("foreground service")?;

        // Acquire a partial wake lock to keep the CPU running during playback.
        #[cfg(target_os = "android")]
        {
            let mut wl = self.wake_lock.lock().unwrap();
            if wl.is_none() {
                match wakelock::WakeLock::new("kaulan:playback") {
                    Ok(mut lock) => {
                        lock.acquire()?;
                        *wl = Some(lock);
                    }
                    Err(e) => log::error!("Failed to create wake lock: {}", e),
                }
            }
        }

        Ok(())
    }

    fn stop(self: std::sync::Arc<Self>) -> Result<(), String> {
        // Release the wake lock.
        #[cfg(target_os = "android")]
        {
            if let Some(mut wl) = self.wake_lock.lock().unwrap().take() {
                wl.release()?;
            }
        }
        self.running.store(false, Ordering::Release);
        Ok(())
    }

    fn is_running(self: std::sync::Arc<Self>) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

// State to hold the current music directory
struct MusicDirectory(Mutex<String>);

#[derive(Debug, Clone, serde::Serialize)]
struct ExportedCookie {
    name: String,
    value: String,
    domain: String,
    path: String,
    secure: bool,
    http_only: bool,
    expires: Option<i64>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing first (must happen before Tauri setup to avoid conflicts)
    kaulan::init_tracing();

    let kaulan_server = Arc::new(KaulanServer {
        running: std::sync::atomic::AtomicBool::new(false),
        music_dir: Mutex::new(if cfg!(target_os = "android") {
            Some("/storage".to_string())
        } else {
            None
        }),
        data_dir: Mutex::new(None),
        #[cfg(target_os = "android")]
        wake_lock: Mutex::new(None),
    });
    set_server(kaulan_server.clone());
    log::info!("Registered KaulanServer with music notification plugin");

    tauri::Builder::default()
        .manage(MusicDirectory(Mutex::new(String::new())))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_android_mediastore::init())
        .plugin(tauri_plugin_android_external_storage::init())
        .plugin(tauri_plugin_music_notification_api::init())
        .setup(move |app| {
            log::info!("======================= Start =======================");

            // Read config from Tauri-managed storage for UI display purposes.
            // Android uses app_data_dir so it matches the backend config path.
            let app_handle = app.handle().clone();
            let app_handle_for_config = app_handle.clone();
            let music_path = tauri::async_runtime::block_on(async move {
                let path_resolver = app_handle_for_config.path();
                let config_dir = if cfg!(target_os = "android") {
                    path_resolver.app_data_dir()
                } else {
                    path_resolver.app_config_dir()
                };

                if let Ok(config_dir) = config_dir {
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

            {
                let mut server_music_dir = kaulan_server.music_dir.lock().unwrap();
                *server_music_dir = if music_path.is_empty() {
                    if cfg!(target_os = "android") {
                        Some("/storage".to_string())
                    } else {
                        None
                    }
                } else {
                    Some(music_path.clone())
                };
            }

            // Set up custom file operations implementations for Android
            #[cfg(target_os = "android")]
            {
                log::info!("Setting up MediaStore adapters for Android");
                let app_handle_for_adapter = app.handle().clone();

                let _ = kaulan::set_file_reader(Box::new(android_media_adapter::MediaStoreFileReader::new(app_handle_for_adapter.clone())));
                let _ = kaulan::set_music_file_lister(Box::new(android_media_adapter::MediaStoreMusicFileLister::new(app_handle_for_adapter.clone())));
                let _ = kaulan::set_lyric_reader(Box::new(android_media_adapter::AndroidLyricReader::new(app_handle_for_adapter.clone())));
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
            *kaulan_server.data_dir.lock().unwrap() = data_dir_for_server;
            #[cfg(not(target_os = "android"))]
            if let Err(e) = kaulan_server.start_backend("desktop app startup") {
                log::error!("Failed to start backend server during desktop startup: {}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_platform,
            get_music_directory,
            set_music_directory,
            request_external_storage_permission,
            check_external_storage_permission,
            export_webview_cookies
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Get the current platform for frontend boot gating.
#[tauri::command]
fn get_platform() -> String {
    if cfg!(target_os = "android") {
        "android".to_string()
    } else if cfg!(target_os = "ios") {
        "ios".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else if cfg!(target_os = "linux") {
        "linux".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Get the current music directory
#[tauri::command]
fn get_music_directory(state: State<'_, MusicDirectory>) -> Result<String, String> {
    let path = state.0.lock().unwrap().clone();
    Ok(path)
}

/// Set a new music directory (saved to config, takes effect on restart)
#[tauri::command]
fn set_music_directory(app: tauri::AppHandle, new_path: String) -> Result<(), String> {
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
    let config_dir = path_resolver
        .app_config_dir()
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

/// Request MANAGE_EXTERNAL_STORAGE permission for reading local lyrics files on Android
///
/// This command is called when the user enables the "使用本地歌词" checkbox.
/// On Android, it requests the MANAGE_EXTERNAL_STORAGE permission via the plugin.
/// On other platforms, it does nothing (permission not needed).
#[tauri::command]
fn request_external_storage_permission(_app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let app = _app;
        log::info!("Requesting MANAGE_EXTERNAL_STORAGE permission for lyrics");

        match app
            .android_external_storage()
            .request_all_files_access()
        {
            Ok(response) => {
                if response.is_granted {
                    log::info!("MANAGE_EXTERNAL_STORAGE permission granted");
                    Ok(true)
                } else {
                    log::warn!("MANAGE_EXTERNAL_STORAGE permission not granted");
                    Ok(false)
                }
            }
            Err(e) => {
                log::error!("Failed to request MANAGE_EXTERNAL_STORAGE permission: {}", e);
                Err(format!("Failed to request permission: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        Ok(true)
    }
}

/// Check whether MANAGE_EXTERNAL_STORAGE permission is currently granted.
#[tauri::command]
fn check_external_storage_permission(_app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        let app = _app;
        match app
            .android_external_storage()
            .check_all_files_access()
        {
            Ok(response) => {
                log::info!("MANAGE_EXTERNAL_STORAGE permission check: granted={}", response.is_granted);
                Ok(response.is_granted)
            }
            Err(e) => {
                log::error!("Failed to check MANAGE_EXTERNAL_STORAGE permission: {}", e);
                Err(format!("Failed to check permission: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        Ok(true)
    }
}

#[tauri::command]
async fn export_webview_cookies(app: tauri::AppHandle) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        let _ = app;
        return Err("Webview cookie export is only supported on desktop".to_string());
    }

    #[cfg(not(target_os = "android"))]
    {
        let webview = app
            .get_webview_window("main")
            .ok_or_else(|| "main webview window not found".to_string())?;

        let cookies = webview
            .cookies()
            .map_err(|e| format!("failed to read webview cookies: {}", e))?;

        let mut exported = Vec::new();
        let mut seen = HashSet::new();

        for cookie in cookies {
            let name = cookie.name().to_string();
            let value = cookie.value().to_string();
            let domain = cookie
                .domain_raw()
                .or_else(|| cookie.domain())
                .unwrap_or("")
                .to_string();
            let path = cookie.path().unwrap_or("/").to_string();

            let domain_filter = domain.to_ascii_lowercase();
            if name.is_empty()
                || value.is_empty()
                || domain.is_empty()
                || (!domain_filter.contains("youtube.com") && !domain_filter.contains("google.com"))
            {
                continue;
            }

            let key = (name.clone(), domain.clone(), path.clone());
            if !seen.insert(key) {
                continue;
            }

            let expires = cookie.expires_datetime().map(|dt| dt.unix_timestamp());

            exported.push(ExportedCookie {
                name,
                value,
                domain,
                path,
                secure: cookie.secure().unwrap_or(false),
                http_only: cookie.http_only().unwrap_or(false),
                expires,
            });
        }

        if exported.is_empty() {
            return Err("no webview cookies found".to_string());
        }

        let mut out = String::from("# Netscape HTTP Cookie File\n");
        out.push_str("# Exported from Tauri webview cookie store\n");
        for cookie in &exported {
            let include_subdomains = if cookie.domain.starts_with('.') { "TRUE" } else { "FALSE" };
            let secure = if cookie.secure { "TRUE" } else { "FALSE" };
            let expires = cookie.expires.unwrap_or(0);
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                cookie.domain,
                include_subdomains,
                cookie.path,
                secure,
                expires,
                cookie.name,
                cookie.value,
            ));
        }

        let mut output_path = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        output_path.push(format!("kaulan-webview-cookies-{}.txt", nanos));
        fs::write(&output_path, out)
            .map_err(|e| format!("failed to write cookie jar: {}", e))?;

        Ok(output_path.to_string_lossy().to_string())
    }
}
