#[cfg(not(target_os = "android"))]
use base64::Engine;
use bilibili_api::auth::BiliSession;
#[cfg(target_os = "android")]
use jni::objects::{GlobalRef, JObject, JString, JValue};
#[cfg(target_os = "android")]
use jni::JavaVM;
use netease_api::auth::Session as NeteaseSession;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "android"))]
use tauri::webview::PageLoadPayload;
use tauri::{Manager, State, Url};
#[cfg(not(target_os = "android"))]
use tauri::{WebviewUrl, WebviewWindow};
#[cfg(target_os = "android")]
use tauri_plugin_android_external_storage::AndroidExternalStorageExt;
use ytdl_audio::JsRunner;

// MediaStore adapter module
mod android_media_adapter;
// Android wake lock module
#[cfg(target_os = "android")]
mod wakelock;

// Music notification plugin
use tauri_plugin_music_notification_api::{set_server, Server};

const NETEASE_LOGIN_URL: &str = "https://music.163.com/";
const BILIBILI_LOGIN_URL: &str = "https://www.bilibili.com/";
const YOUTUBE_LOGIN_URL: &str = "https://www.youtube.com/";
const NCMDUMP_CONFIG_DIR_ENV: &str = "NCMDUMP_CONFIG_DIR";
const YOUTUBE_COOKIE_HEADER_PATH_ENV: &str = "KAULAN_YOUTUBE_COOKIE_HEADER_PATH";
#[cfg(not(target_os = "android"))]
const SOLVER_WINDOW_LABEL: &str = "youtube-solver";
#[cfg(not(target_os = "android"))]
const SOLVER_TITLE_PREFIX: &str = "kaulan-solver-result:";

#[cfg(target_os = "android")]
static ANDROID_EXTERNAL_FILES_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
#[cfg(target_os = "android")]
static ANDROID_ACTIVITY_VM: OnceLock<Mutex<Option<JavaVM>>> = OnceLock::new();
#[cfg(target_os = "android")]
static ANDROID_ACTIVITY_GLOBAL: OnceLock<Mutex<Option<GlobalRef>>> = OnceLock::new();
#[cfg(not(target_os = "android"))]
static SOLVER_WINDOW_READY: OnceLock<Arc<(Mutex<bool>, std::sync::Condvar)>> = OnceLock::new();
#[cfg(not(target_os = "android"))]
static SOLVER_RESPONSE_CHANNELS: OnceLock<
    Mutex<std::collections::HashMap<String, std::sync::mpsc::Sender<String>>>,
> = OnceLock::new();

struct WebviewJsRunner {
    #[cfg(not(target_os = "android"))]
    app: tauri::AppHandle,
}

impl JsRunner for WebviewJsRunner {
    fn run(&self, input: &str) -> Result<String, ytdl_audio::Error> {
        #[cfg(target_os = "android")]
        {
            return run_hidden_android_solver(input);
        }

        #[cfg(not(target_os = "android"))]
        {
            let window = solver_window(&self.app).map_err(ytdl_audio::Error::Other)?;
            wait_for_solver_window_ready(&self.app)?;
            self.ensure_solver_ready(&window)?;
            eval_in_solver_window(
                &window,
                format!(
                    r#"(() => {{
  try {{
    const input = JSON.parse({input:?});
    return window.__ytdlSolve(input);
  }} catch (error) {{
    return {{"type":"error","error": String(error && error.stack ? error.stack : error)}};
  }}
}})()"#
                ),
                std::time::Duration::from_secs(30),
            )
            .map_err(ytdl_audio::Error::Other)
        }
    }
}

impl WebviewJsRunner {
    #[cfg(not(target_os = "android"))]
    fn ensure_solver_ready(&self, window: &WebviewWindow) -> Result<(), ytdl_audio::Error> {
        let init_script = format!(
            r#"(function() {{
  if (window.__ytdlSolveReady) {{
    return;
  }}
  try {{
    if (!window.__ytdlSolverBootstrapping) {{
      window.__ytdlSolverBootstrapping = true;
      const loadScript = (src) => new Promise((resolve, reject) => {{
        const existing = document.querySelector(`script[data-ytdl-src="${{src}}"]`);
        if (existing) {{
          if (existing.dataset.ytdlReady === "true") {{
            resolve();
            return;
          }}
          existing.addEventListener('load', () => resolve(), {{ once: true }});
          existing.addEventListener('error', () => reject(new Error(`failed to load ${{src}}`)), {{ once: true }});
          return;
        }}
        const script = document.createElement('script');
        script.src = src;
        script.async = false;
        script.dataset.ytdlSrc = src;
        script.onload = () => {{
          script.dataset.ytdlReady = "true";
          resolve();
        }};
        script.onerror = () => reject(new Error(`failed to load ${{src}}`));
        document.head.appendChild(script);
      }});
      Promise.resolve()
        .then(() => loadScript('https://cdn.jsdelivr.net/npm/meriyah@6.1.4/dist/meriyah.umd.min.js'))
        .then(() => loadScript('https://cdn.jsdelivr.net/npm/astring@1.9.0/dist/astring.min.js'))
        .then(() => {{
          globalThis.meriyah = globalThis.meriyah || window.meriyah;
          globalThis.astring = globalThis.astring || window.astring;
          const coreCode = {core_code:?};
          window.__ytdlSolve = eval(`${{coreCode}}\n; jsc;`);
          window.__ytdlSolveReady = true;
          window.__ytdlSolveError = null;
        }})
        .catch((error) => {{
          window.__ytdlSolveError = String(error && error.stack ? error.stack : error);
        }})
        .finally(() => {{
          window.__ytdlSolverBootstrapping = false;
        }});
    }}
  }} catch (error) {{
    window.__ytdlSolveError = String(error && error.stack ? error.stack : error);
  }}
}})()"#,
            core_code = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../vendor/ytdl-audio/js/yt.solver.core.js"
            ))
        );
        window
            .eval(init_script)
            .map_err(|e| ytdl_audio::Error::Other(e.to_string()))?;

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let status = eval_in_solver_window(
                window,
                r#"(() => ({
  ready: !!window.__ytdlSolveReady,
  error: window.__ytdlSolveError || null
}))()"#
                    .to_string(),
                std::time::Duration::from_secs(2),
            )
            .map_err(ytdl_audio::Error::Other)?;
            let parsed: serde_json::Value = serde_json::from_str(&status)?;
            if parsed.get("ready").and_then(|v| v.as_bool()) == Some(true) {
                return Ok(());
            }
            if let Some(err) = parsed.get("error").and_then(|v| v.as_str()) {
                return Err(ytdl_audio::Error::Other(format!(
                    "webview solver init failed: {err}"
                )));
            }
            if std::time::Instant::now() >= deadline {
                return Err(ytdl_audio::Error::Other(
                    "webview solver init timed out".into(),
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
}

#[cfg(not(target_os = "android"))]
fn eval_in_solver_window(
    window: &WebviewWindow,
    expression: String,
    timeout: std::time::Duration,
) -> Result<String, String> {
    let token = format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    solver_response_channels()
        .lock()
        .map_err(|_| "solver response channel mutex poisoned".to_string())?
        .insert(token.clone(), tx);
    let script = format!(
        r#"(() => {{
  let __kaulanResult;
  try {{
    __kaulanResult = {expression};
  }} catch (error) {{
    __kaulanResult = {{"type":"error","error": String(error && error.stack ? error.stack : error)}};
  }}
  const __kaulanJson = JSON.stringify(__kaulanResult);
  const __kaulanBase64 = btoa(unescape(encodeURIComponent(__kaulanJson)));
  document.title = {title_prefix:?} + {token:?} + ":" + __kaulanBase64;
}})();"#,
        title_prefix = SOLVER_TITLE_PREFIX
    );
    if let Err(err) = window.eval(script) {
        if let Ok(mut channels) = solver_response_channels().lock() {
            channels.remove(&token);
        }
        return Err(err.to_string());
    }
    let received = rx
        .recv_timeout(timeout)
        .map_err(|e| format!("solver callback timeout for {token}: {e}"));
    if let Ok(mut channels) = solver_response_channels().lock() {
        channels.remove(&token);
    }
    received
}

#[cfg(not(target_os = "android"))]
fn solver_response_channels(
) -> &'static Mutex<std::collections::HashMap<String, std::sync::mpsc::Sender<String>>> {
    SOLVER_RESPONSE_CHANNELS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

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
            log::info!(
                "Backend server already running, skipping startup from {}",
                source
            );
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

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum OnlineProvider {
    Netease,
    Bilibili,
    Youtube,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ProviderStatus {
    provider: &'static str,
    is_logged_in: bool,
    session_path: String,
    summary: String,
}

#[cfg(not(target_os = "android"))]
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
        .manage(kaulan_server.clone())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_android_mediastore::init())
        .plugin(tauri_plugin_android_external_storage::init())
        .plugin(tauri_plugin_music_notification_api::init())
        .setup(move |app| {
            log::info!("======================= Start =======================");
            let app_handle = app.handle().clone();
            kaulan::set_youtube_js_runner_factory({
                let app_handle = app_handle.clone();
                move || {
                    #[cfg(target_os = "android")]
                    {
                        Ok(Box::new(WebviewJsRunner {}))
                    }

                    #[cfg(not(target_os = "android"))]
                    {
                        Ok(Box::new(WebviewJsRunner {
                            app: app_handle.clone(),
                        }))
                    }
                }
            })?;
            let external_download_root = resolve_online_download_root(&app_handle)?;
            let preview_root = resolve_preview_root(&app_handle, &external_download_root)?;
            let online_config_dir = resolve_online_config_dir(&app_handle)?;

            if preview_root.exists() {
                fs::remove_dir_all(&preview_root)
                    .map_err(|e| format!("Failed to clear preview cache: {e}"))?;
            }
            fs::create_dir_all(&preview_root)
                .map_err(|e| format!("Failed to create preview cache: {e}"))?;

            std::env::set_var(
                "KAULAN_DOWNLOAD_ROOT",
                external_download_root.to_string_lossy().to_string(),
            );
            std::env::set_var(
                "KAULAN_PREVIEW_ROOT",
                preview_root.to_string_lossy().to_string(),
            );
            std::env::set_var(
                NCMDUMP_CONFIG_DIR_ENV,
                online_config_dir.to_string_lossy().to_string(),
            );
            std::env::set_var(
                YOUTUBE_COOKIE_HEADER_PATH_ENV,
                youtube_cookie_jar_path(&app_handle)?
                    .to_string_lossy()
                    .to_string(),
            );

            // Read config from Tauri-managed storage for UI display purposes.
            // Android uses app_data_dir so it matches the backend config path.
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
                            if let Ok(config) = serde_json::from_str::<serde_json::Value>(&content)
                            {
                                if let Some(path) =
                                    config.get("music_directory").and_then(|v| v.as_str())
                                {
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

                let _ = kaulan::set_file_reader(Box::new(
                    android_media_adapter::MediaStoreFileReader::new(
                        app_handle_for_adapter.clone(),
                    ),
                ));
                let _ = kaulan::set_music_file_lister(Box::new(
                    android_media_adapter::MediaStoreMusicFileLister::new(
                        app_handle_for_adapter.clone(),
                    ),
                ));
                let _ = kaulan::set_lyric_reader(Box::new(
                    android_media_adapter::AndroidLyricReader::new(app_handle_for_adapter.clone()),
                ));
                log::info!("MediaStore adapters configured successfully");
            }

            // Prepare data directory config for server startup
            let data_dir_for_server = if cfg!(target_os = "android") {
                app_handle
                    .path()
                    .app_data_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            } else {
                None
            };
            *kaulan_server.data_dir.lock().unwrap() = data_dir_for_server;
            #[cfg(not(target_os = "android"))]
            if let Err(e) = kaulan_server.start_backend("desktop app startup") {
                log::error!(
                    "Failed to start backend server during desktop startup: {}",
                    e
                );
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_platform,
            exit_android_app,
            get_music_directory,
            set_music_directory,
            request_external_storage_permission,
            check_external_storage_permission,
            export_webview_cookies,
            online_open_login,
            online_capture_login,
            online_login_status,
            online_logout
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

/// Stop Android playback/backend state and exit the app process.
#[tauri::command]
fn exit_android_app(
    app: tauri::AppHandle,
    state: State<'_, Arc<KaulanServer>>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_music_notification_api::MusicNotificationExt;

        log::info!("Android timer requested app exit");

        if let Err(err) = app.music_notification().stop_service() {
            log::warn!(
                "Failed to stop Android playback service during exit: {}",
                err
            );
        }

        if let Err(err) = state.inner().clone().stop() {
            log::warn!("Failed to stop Android backend state during exit: {}", err);
        }

        app.exit(0);
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        let _ = state;
        Err("exit_android_app is only available on Android".to_string())
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

        match app.android_external_storage().request_all_files_access() {
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
                log::error!(
                    "Failed to request MANAGE_EXTERNAL_STORAGE permission: {}",
                    e
                );
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
        match app.android_external_storage().check_all_files_access() {
            Ok(response) => {
                log::info!(
                    "MANAGE_EXTERNAL_STORAGE permission check: granted={}",
                    response.is_granted
                );
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
            let include_subdomains = if cookie.domain.starts_with('.') {
                "TRUE"
            } else {
                "FALSE"
            };
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
        fs::write(&output_path, out).map_err(|e| format!("failed to write cookie jar: {}", e))?;

        Ok(output_path.to_string_lossy().to_string())
    }
}

fn provider_login_url(provider: OnlineProvider) -> &'static str {
    match provider {
        OnlineProvider::Netease => NETEASE_LOGIN_URL,
        OnlineProvider::Bilibili => BILIBILI_LOGIN_URL,
        OnlineProvider::Youtube => YOUTUBE_LOGIN_URL,
    }
}

fn resolve_online_config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let base = if cfg!(target_os = "android") {
        app.path()
            .app_data_dir()
            .map_err(|e| format!("Failed to resolve app data dir: {e}"))?
    } else {
        app.path()
            .app_config_dir()
            .map_err(|e| format!("Failed to resolve app config dir: {e}"))?
    };
    let dir = base.join("ncmdump");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create provider config dir: {e}"))?;
    Ok(dir)
}

fn resolve_online_download_root(_app: &tauri::AppHandle) -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        let base = android_external_files_dir()?;
        let music_dir = base.join("Music");
        fs::create_dir_all(&music_dir)
            .map_err(|e| format!("Failed to create online music dir: {e}"))?;
        return Ok(music_dir);
    }

    #[cfg(not(target_os = "android"))]
    {
        let state = _app.state::<MusicDirectory>();
        let configured = state.0.lock().unwrap().clone();
        let dir = if configured.is_empty() {
            _app.path()
                .app_data_dir()
                .map_err(|e| format!("Failed to resolve app data dir: {e}"))?
                .join("downloads")
        } else {
            PathBuf::from(configured)
        };
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create online download dir: {e}"))?;
        Ok(dir)
    }
}

fn resolve_preview_root(
    app: &tauri::AppHandle,
    _download_root: &PathBuf,
) -> Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        let dir = _download_root.join(".preview-cache");
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create preview cache dir: {e}"))?;
        return Ok(dir);
    }

    #[cfg(not(target_os = "android"))]
    {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to resolve app data dir: {e}"))?
            .join("preview-cache");
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create preview cache dir: {e}"))?;
        Ok(dir)
    }
}

fn netease_session_path(app: &tauri::AppHandle) -> Result<String, String> {
    Ok(resolve_online_config_dir(app)?
        .join("session.json")
        .to_string_lossy()
        .to_string())
}

fn bilibili_session_path(app: &tauri::AppHandle) -> Result<String, String> {
    Ok(resolve_online_config_dir(app)?
        .join("bilibili_session.json")
        .to_string_lossy()
        .to_string())
}

fn youtube_cookie_jar_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(resolve_online_config_dir(app)?.join("youtube_cookies.txt"))
}

fn load_netease_status(app: &tauri::AppHandle) -> Result<ProviderStatus, String> {
    std::env::set_var(NCMDUMP_CONFIG_DIR_ENV, resolve_online_config_dir(app)?);
    let session = NeteaseSession::load().map_err(|e| e.to_string())?;
    Ok(ProviderStatus {
        provider: "netease",
        is_logged_in: session.is_logged_in(),
        session_path: netease_session_path(app)?,
        summary: if session.is_logged_in() {
            "MUSIC_U is present".to_string()
        } else {
            "No MUSIC_U saved".to_string()
        },
    })
}

fn load_bilibili_status(app: &tauri::AppHandle) -> Result<ProviderStatus, String> {
    std::env::set_var(NCMDUMP_CONFIG_DIR_ENV, resolve_online_config_dir(app)?);
    let session = BiliSession::load().map_err(|e| e.to_string())?;
    let mut fields = Vec::new();
    if session.sessdata.as_ref().is_some_and(|v| !v.is_empty()) {
        fields.push("SESSDATA");
    }
    if session.bili_jct.as_ref().is_some_and(|v| !v.is_empty()) {
        fields.push("bili_jct");
    }
    if session.dede_user_id.as_ref().is_some_and(|v| !v.is_empty()) {
        fields.push("DedeUserID");
    }

    Ok(ProviderStatus {
        provider: "bilibili",
        is_logged_in: session.is_logged_in(),
        session_path: bilibili_session_path(app)?,
        summary: if fields.is_empty() {
            "No Bilibili cookies saved".to_string()
        } else {
            format!("Saved cookies: {}", fields.join(", "))
        },
    })
}

fn load_youtube_status(app: &tauri::AppHandle) -> Result<ProviderStatus, String> {
    let path = youtube_cookie_jar_path(app)?;
    let contents = fs::read_to_string(&path).unwrap_or_default();
    let cookies = parse_netscape_cookie_names(&contents);
    let mut fields = Vec::new();

    for field in ["SAPISID", "__Secure-3PAPISID", "SID", "HSID", "SSID"] {
        if cookies.contains(field) {
            fields.push(field);
        }
    }

    Ok(ProviderStatus {
        provider: "youtube",
        is_logged_in: !cookies.is_empty(),
        session_path: path.to_string_lossy().to_string(),
        summary: if fields.is_empty() {
            if cookies.is_empty() {
                "No YouTube cookies saved".to_string()
            } else {
                format!("Saved {} cookies", cookies.len())
            }
        } else {
            format!("Saved cookies: {}", fields.join(", "))
        },
    })
}

fn load_provider_status(
    app: &tauri::AppHandle,
    provider: OnlineProvider,
) -> Result<ProviderStatus, String> {
    match provider {
        OnlineProvider::Netease => load_netease_status(app),
        OnlineProvider::Bilibili => load_bilibili_status(app),
        OnlineProvider::Youtube => load_youtube_status(app),
    }
}

fn login_webview_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    app.get_webview_window("main")
        .ok_or_else(|| "main webview window not found".to_string())
}

#[cfg(not(target_os = "android"))]
fn solver_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(SOLVER_WINDOW_LABEL) {
        return Ok(window);
    }

    *solver_ready_state()
        .0
        .lock()
        .map_err(|_| "solver ready mutex poisoned".to_string())? = false;

    tauri::WebviewWindowBuilder::new(
        app,
        SOLVER_WINDOW_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .visible(false)
    .title("youtube-solver")
    .on_page_load(|_, payload: PageLoadPayload<'_>| {
        let url = payload.url().as_str();
        if url.starts_with("tauri://")
            || url.starts_with("http://tauri.localhost")
            || url.starts_with("http://localhost:")
            || url.starts_with("http://127.0.0.1:")
        {
            let state = solver_ready_state();
            if let Ok(mut ready) = state.0.lock() {
                *ready = true;
                state.1.notify_all();
            }
        }
    })
    .on_document_title_changed(|window, title| {
        if !title.starts_with(SOLVER_TITLE_PREFIX) {
            return;
        }

        let payload = &title[SOLVER_TITLE_PREFIX.len()..];
        let Some((token, encoded)) = payload.split_once(':') else {
            log::warn!("Invalid solver title payload: {}", title);
            return;
        };

        let decoded = match base64::engine::general_purpose::STANDARD.decode(encoded) {
            Ok(bytes) => bytes,
            Err(err) => {
                log::warn!("Failed to decode solver title payload: {}", err);
                return;
            }
        };
        let decoded = match String::from_utf8(decoded) {
            Ok(value) => value,
            Err(err) => {
                log::warn!("Failed to decode solver UTF-8 payload: {}", err);
                return;
            }
        };

        let sender = solver_response_channels()
            .lock()
            .ok()
            .and_then(|mut channels| channels.remove(token));
        if let Some(sender) = sender {
            let _ = sender.send(decoded);
        }

        let _ = window.set_title("youtube-solver");
    })
    .build()
    .map_err(|e| format!("failed to create solver window: {e}"))
}

#[cfg(not(target_os = "android"))]
fn solver_ready_state() -> &'static Arc<(Mutex<bool>, std::sync::Condvar)> {
    SOLVER_WINDOW_READY.get_or_init(|| Arc::new((Mutex::new(false), std::sync::Condvar::new())))
}

#[cfg(not(target_os = "android"))]
fn wait_for_solver_window_ready(app: &tauri::AppHandle) -> Result<(), ytdl_audio::Error> {
    let _ = solver_window(app).map_err(ytdl_audio::Error::Other)?;
    let state = solver_ready_state();
    let ready = state
        .0
        .lock()
        .map_err(|_| ytdl_audio::Error::Other("solver ready mutex poisoned".into()))?;
    let (ready, timeout) = state
        .1
        .wait_timeout_while(ready, std::time::Duration::from_secs(10), |ready| !*ready)
        .map_err(|_| ytdl_audio::Error::Other("solver ready wait poisoned".into()))?;
    if *ready {
        return Ok(());
    }
    if timeout.timed_out() {
        return Err(ytdl_audio::Error::Other(
            "solver window did not finish loading".into(),
        ));
    }
    Err(ytdl_audio::Error::Other(
        "solver window did not become ready".into(),
    ))
}

#[tauri::command]
fn online_open_login(app: tauri::AppHandle, provider: OnlineProvider) -> Result<(), String> {
    let url = provider_login_url(provider);
    let window = login_webview_window(&app)?;
    window
        .navigate(Url::parse(url).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
fn run_hidden_android_solver(input: &str) -> Result<String, ytdl_audio::Error> {
    let vm_guard = ANDROID_ACTIVITY_VM
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| ytdl_audio::Error::Other("android vm mutex poisoned".into()))?;
    let vm = vm_guard
        .as_ref()
        .ok_or_else(|| ytdl_audio::Error::Other("android vm not initialized".into()))?;
    let activity_guard = ANDROID_ACTIVITY_GLOBAL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| ytdl_audio::Error::Other("android activity mutex poisoned".into()))?;
    let activity = activity_guard
        .as_ref()
        .ok_or_else(|| ytdl_audio::Error::Other("android activity not initialized".into()))?;

    let mut env = vm
        .attach_current_thread()
        .map_err(|e| ytdl_audio::Error::Other(format!("attach_current_thread failed: {e}")))?;
    let input_java = env.new_string(input).map_err(|e| {
        ytdl_audio::Error::Other(format!("failed to allocate solver input string: {e}"))
    })?;
    let core_java = env
        .new_string(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/ytdl-audio/js/yt.solver.core.js"
        )))
        .map_err(|e| {
            ytdl_audio::Error::Other(format!("failed to allocate solver core string: {e}"))
        })?;
    let result = env
        .call_method(
            activity.as_obj(),
            "runHiddenSolver",
            "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
            &[
                JValue::Object(&JObject::from(input_java)),
                JValue::Object(&JObject::from(core_java)),
            ],
        )
        .and_then(|v| v.l())
        .map_err(|e| ytdl_audio::Error::Other(format!("runHiddenSolver failed: {e}")))?;
    if result.is_null() {
        return Err(ytdl_audio::Error::Other(
            "hidden android solver returned null".into(),
        ));
    }
    let result = JString::from(result);
    env.get_string(&result)
        .map(|s| s.to_string_lossy().into_owned())
        .map_err(|e| ytdl_audio::Error::Other(format!("failed to decode solver result: {e}")))
}

#[tauri::command]
fn online_capture_login(
    app: tauri::AppHandle,
    provider: OnlineProvider,
) -> Result<ProviderStatus, String> {
    std::env::set_var(NCMDUMP_CONFIG_DIR_ENV, resolve_online_config_dir(&app)?);
    match provider {
        OnlineProvider::Netease => {
            let music_u = extract_netease_cookie(&app, provider)?;
            NeteaseSession {
                music_u: Some(music_u),
            }
            .save()
            .map_err(|e| e.to_string())?;
            load_netease_status(&app)
        }
        OnlineProvider::Bilibili => {
            let session = extract_bilibili_session(&app, provider)?;
            session.save().map_err(|e| e.to_string())?;
            load_bilibili_status(&app)
        }
        OnlineProvider::Youtube => {
            export_youtube_cookie_jar(&app, &youtube_cookie_jar_path(&app)?)?;
            load_youtube_status(&app)
        }
    }
}

#[tauri::command]
fn online_login_status(
    app: tauri::AppHandle,
    provider: OnlineProvider,
) -> Result<ProviderStatus, String> {
    load_provider_status(&app, provider)
}

#[tauri::command]
fn online_logout(
    app: tauri::AppHandle,
    provider: OnlineProvider,
) -> Result<ProviderStatus, String> {
    std::env::set_var(NCMDUMP_CONFIG_DIR_ENV, resolve_online_config_dir(&app)?);
    match provider {
        OnlineProvider::Netease => {
            NeteaseSession::clear().map_err(|e| e.to_string())?;
            load_netease_status(&app)
        }
        OnlineProvider::Bilibili => {
            BiliSession::clear().map_err(|e| e.to_string())?;
            load_bilibili_status(&app)
        }
        OnlineProvider::Youtube => {
            let path = youtube_cookie_jar_path(&app)?;
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|e| format!("failed to remove youtube cookies: {e}"))?;
            }
            load_youtube_status(&app)
        }
    }
}

fn extract_netease_cookie(
    _app: &tauri::AppHandle,
    _provider: OnlineProvider,
) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        if let Some(value) = extract_netease_cookie_android()? {
            return Ok(value);
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        let window = login_webview_window(_app)?;

        let cookies = window
            .cookies()
            .map_err(|e| format!("failed to read login cookies: {e}"))?;

        for cookie in cookies {
            let domain = cookie
                .domain_raw()
                .or_else(|| cookie.domain())
                .unwrap_or("")
                .to_ascii_lowercase();

            if (domain.contains("music.163.com") || domain.contains(".163.com"))
                && cookie.name() == "MUSIC_U"
            {
                let value = cookie.value().to_string();
                if !value.is_empty() {
                    return Ok(value);
                }
            }
        }
    }

    Err("MUSIC_U cookie not found in login webview".to_string())
}

fn extract_bilibili_session(
    _app: &tauri::AppHandle,
    _provider: OnlineProvider,
) -> Result<BiliSession, String> {
    #[cfg(target_os = "android")]
    {
        let session = extract_bilibili_cookie_android()?;
        if session.is_logged_in() {
            return Ok(session);
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        let window = login_webview_window(_app)?;

        let cookies = window
            .cookies()
            .map_err(|e| format!("failed to read login cookies: {e}"))?;
        let mut session = BiliSession::default();

        for cookie in cookies {
            let domain = cookie
                .domain_raw()
                .or_else(|| cookie.domain())
                .unwrap_or("")
                .to_ascii_lowercase();

            if !domain.contains("bilibili.com") {
                continue;
            }

            let value = cookie.value().to_string();
            if value.is_empty() {
                continue;
            }

            match cookie.name() {
                "SESSDATA" => session.sessdata = Some(value),
                "bili_jct" => session.bili_jct = Some(value),
                "DedeUserID" => session.dede_user_id = Some(value),
                "buvid3" => session.buvid3 = Some(value),
                "buvid4" => session.buvid4 = Some(value),
                _ => {}
            }
        }

        if session.is_logged_in() {
            return Ok(session);
        }
    }

    Err("SESSDATA cookie not found in login webview".to_string())
}

fn export_youtube_cookie_jar(app: &tauri::AppHandle, path: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        return export_android_youtube_cookie_jar(path);
    }

    #[cfg(not(target_os = "android"))]
    {
        let window = login_webview_window(app)?;

        let cookies = window
            .cookies()
            .map_err(|e| format!("failed to read login cookies: {e}"))?;
        let mut seen = HashSet::new();
        let mut lines = vec![
            "# Netscape HTTP Cookie File".to_string(),
            "# Exported from Tauri webview".to_string(),
        ];
        let mut exported = 0usize;

        for cookie in cookies {
            let name = cookie.name().to_string();
            let value = cookie.value().to_string();
            let domain = cookie
                .domain_raw()
                .or_else(|| cookie.domain())
                .unwrap_or("")
                .to_string();
            let path_part = cookie.path().unwrap_or("/").to_string();
            if name.is_empty() || value.is_empty() || domain.is_empty() {
                continue;
            }

            let domain_lower = domain.to_ascii_lowercase();
            if !domain_lower.contains("youtube.com") && !domain_lower.contains("google.com") {
                continue;
            }

            if !seen.insert((name.clone(), domain.clone(), path_part.clone())) {
                continue;
            }

            let secure = if cookie.secure().unwrap_or(false) {
                "TRUE"
            } else {
                "FALSE"
            };
            let include_subdomains = if domain.starts_with('.') {
                "TRUE"
            } else {
                "FALSE"
            };
            let expires = cookie
                .expires_datetime()
                .map(|t| t.unix_timestamp())
                .unwrap_or(0);
            lines.push(format!(
                "{domain}\t{include_subdomains}\t{path_part}\t{secure}\t{expires}\t{name}\t{value}"
            ));
            exported += 1;
        }

        if exported == 0 {
            return Err("no YouTube/Google cookies found".to_string());
        }

        fs::write(path, lines.join("\n") + "\n")
            .map_err(|e| format!("failed to write cookie jar: {e}"))?;
        return Ok(());
    }
}

fn parse_netscape_cookie_names(contents: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 7 {
            continue;
        }
        let name = fields[5].trim();
        let value = fields[6].trim();
        if !name.is_empty() && !value.is_empty() {
            names.insert(name.to_string());
        }
    }
    names
}

#[cfg(target_os = "android")]
fn export_android_youtube_cookie_jar(path: &PathBuf) -> Result<(), String> {
    let sources = [
        ("https://www.youtube.com/", ".youtube.com"),
        ("https://m.youtube.com/", ".youtube.com"),
        ("https://music.youtube.com/", ".youtube.com"),
        ("https://studio.youtube.com/", ".youtube.com"),
        ("https://accounts.google.com/", ".google.com"),
        ("https://accounts.youtube.com/", ".youtube.com"),
    ];
    let mut lines = vec![
        "# Netscape HTTP Cookie File".to_string(),
        "# Exported from Android CookieManager".to_string(),
    ];
    for (url, domain) in sources {
        let header = android_cookie_header(url)?;
        if header.is_empty() {
            continue;
        }
        lines.extend(cookie_header_to_netscape_lines(domain, &header));
    }

    if lines.len() <= 2 {
        return Err("no YouTube/Google cookies found".to_string());
    }

    fs::write(path, lines.join("\n") + "\n")
        .map_err(|e| format!("failed to write cookie jar: {e}"))?;
    Ok(())
}

#[cfg(target_os = "android")]
fn cookie_header_to_netscape_lines(domain: &str, header: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut seen = HashSet::new();
    for pair in header.split(';') {
        let pair = pair.trim();
        let Some((name, value)) = pair.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let value = value.trim();
        if name.is_empty() || value.is_empty() || !seen.insert(name.to_string()) {
            continue;
        }
        lines.push(format!("{domain}\tTRUE\t/\tTRUE\t0\t{name}\t{value}"));
    }
    lines
}

#[cfg(target_os = "android")]
fn extract_netease_cookie_android() -> Result<Option<String>, String> {
    for url in [
        "https://music.163.com/",
        "https://y.music.163.com/",
        "https://interface.music.163.com/",
    ] {
        let header = android_cookie_header(url)?;
        for pair in header.split(';') {
            let pair = pair.trim();
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            if name.trim() == "MUSIC_U" {
                let value = value.trim();
                if !value.is_empty() {
                    return Ok(Some(value.to_string()));
                }
            }
        }
    }
    Ok(None)
}

#[cfg(target_os = "android")]
fn extract_bilibili_cookie_android() -> Result<BiliSession, String> {
    let mut session = BiliSession::default();
    for url in [
        "https://www.bilibili.com/",
        "https://m.bilibili.com/",
        "https://passport.bilibili.com/",
        "https://api.bilibili.com/",
    ] {
        let header = android_cookie_header(url)?;
        for pair in header.split(';') {
            let pair = pair.trim();
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match name.trim() {
                "SESSDATA" => session.sessdata = Some(value.to_string()),
                "bili_jct" => session.bili_jct = Some(value.to_string()),
                "DedeUserID" => session.dede_user_id = Some(value.to_string()),
                "buvid3" => session.buvid3 = Some(value.to_string()),
                "buvid4" => session.buvid4 = Some(value.to_string()),
                _ => {}
            }
        }
    }
    Ok(session)
}

#[cfg(target_os = "android")]
fn android_cookie_header(url: &str) -> Result<String, String> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm() as *mut _) }
        .map_err(|e| format!("Failed to get JavaVM: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach thread: {e}"))?;

    let cookie_manager_class = env
        .find_class("android/webkit/CookieManager")
        .map_err(|e| format!("Failed to find CookieManager: {e}"))?;
    let cookie_manager = env
        .call_static_method(
            cookie_manager_class,
            "getInstance",
            "()Landroid/webkit/CookieManager;",
            &[],
        )
        .and_then(|v| v.l())
        .map_err(|e| format!("Failed to get CookieManager instance: {e}"))?;

    let url_string = env
        .new_string(url)
        .map_err(|e| format!("Failed to allocate Java URL string: {e}"))?;
    let value = env
        .call_method(
            &cookie_manager,
            "getCookie",
            "(Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&JObject::from(url_string))],
        )
        .and_then(|v| v.l())
        .map_err(|e| format!("Failed to read cookies for {url}: {e}"))?;

    if value.is_null() {
        return Ok(String::new());
    }

    env.get_string(&value.into())
        .map(|s| s.to_string_lossy().into_owned())
        .map_err(|e| format!("Failed to decode cookie header: {e}"))
}

#[cfg(target_os = "android")]
fn android_external_files_dir() -> Result<PathBuf, String> {
    if let Some(slot) = ANDROID_EXTERNAL_FILES_DIR.get() {
        if let Ok(guard) = slot.lock() {
            if let Some(path) = guard.clone() {
                return Ok(path);
            }
        }
    }

    let ctx = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(ctx.vm() as *mut _) }
        .map_err(|e| format!("Failed to get JavaVM: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach thread: {e}"))?;
    let activity = unsafe { JObject::from_raw(ctx.context() as *mut _) };
    let file_obj = env
        .call_method(
            &activity,
            "getExternalFilesDir",
            "(Ljava/lang/String;)Ljava/io/File;",
            &[JValue::Object(&JObject::null())],
        )
        .and_then(|v| v.l())
        .map_err(|e| format!("Failed to get external files dir: {e}"))?;
    let path_obj = env
        .call_method(&file_obj, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .and_then(|v| v.l())
        .map_err(|e| format!("Failed to get external files dir path: {e}"))?;
    let path = env
        .get_string(&path_obj.into())
        .map_err(|e| format!("Failed to decode external files dir path: {e}"))?
        .to_string_lossy()
        .into_owned();
    let resolved = PathBuf::from(path);
    let slot = ANDROID_EXTERNAL_FILES_DIR.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(resolved.clone());
    }
    Ok(resolved)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_afeather_kaulan_MainActivity_nativeInitAndroidContext(
    mut env: jni::JNIEnv,
    activity: JObject,
) {
    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(err) => {
            log::error!(
                "Failed to capture Android JavaVM for hidden solver: {}",
                err
            );
            return;
        }
    };
    let activity = match env.new_global_ref(activity) {
        Ok(activity) => activity,
        Err(err) => {
            log::error!(
                "Failed to create global Activity ref for hidden solver: {}",
                err
            );
            return;
        }
    };

    let vm_slot = ANDROID_ACTIVITY_VM.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = vm_slot.lock() {
        *guard = Some(vm);
    }

    let activity_slot = ANDROID_ACTIVITY_GLOBAL.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = activity_slot.lock() {
        *guard = Some(activity);
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_afeather_kaulan_MainActivity_nativeReleaseAndroidContext(
    _env: jni::JNIEnv,
    _activity: JObject,
) {
    if let Some(slot) = ANDROID_ACTIVITY_GLOBAL.get() {
        if let Ok(mut guard) = slot.lock() {
            *guard = None;
        }
    }
    if let Some(slot) = ANDROID_ACTIVITY_VM.get() {
        if let Ok(mut guard) = slot.lock() {
            *guard = None;
        }
    }
}
