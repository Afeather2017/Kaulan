//! Configuration file management for persisting settings.
//!
//! Documentation: [docs/settings-and-database-management.md](../../../docs/settings-and-database-management.md)
//!
//! This module provides configuration file management for persisting
//! the music directory path and device identification settings.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;
use uuid::Uuid;

const GENERIC_HOSTNAMES: &[&str] = &[
    "",
    "localhost",
    "localhost.localdomain",
    "localhost6",
    "android",
    "unknown",
];

/// Configuration file structure
///
/// # Config Format
/// ```json
/// {
///   "music_directory": "/path/to/music",
///   "device_id": "550e8400-e29b-41d4-a716-446655440000",
///   "device_name": "Living Room Player",
///   "periodic_discovery_enabled": true,
///   "media_types": ["audio"]
/// }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Music directory path
    pub music_directory: Option<String>,

    /// Unique device identifier (UUID v4)
    pub device_id: Option<String>,

    /// Human-readable device name (user-configurable)
    pub device_name: Option<String>,

    /// Whether this server periodically announces itself on the LAN.
    #[serde(default = "default_periodic_discovery_enabled")]
    pub periodic_discovery_enabled: bool,

    /// Enabled media types for scanning: "audio" and/or "video"
    #[serde(default = "default_media_types")]
    pub media_types: Option<Vec<String>>,
}

fn default_media_types() -> Option<Vec<String>> {
    Some(vec!["audio".to_string()])
}

fn default_periodic_discovery_enabled() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music_directory: None,
            device_id: None,
            device_name: None,
            periodic_discovery_enabled: true,
            media_types: default_media_types(),
        }
    }
}

/// Get the config directory path
///
/// # Returns
/// - `Some(PathBuf)` - Platform-specific config directory with "kaulan" subdirectory
/// - `None` - Config directory cannot be determined
///
/// # Config Locations
/// - Linux: `~/.config/kaulan/`
/// - macOS: `~/Library/Application Support/kaulan/`
/// - Windows: `%APPDATA%\kaulan\`
pub fn get_config_dir() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("KAULAN_CONFIG_DIR") {
        return Some(PathBuf::from(path));
    }

    if std::env::var("TAURI_PLATFORM").ok().as_deref() == Some("android") {
        return std::env::var("TAURI_ANDROID_DATA_DIR")
            .ok()
            .map(PathBuf::from);
    }

    let mut path = dirs::config_dir()?;
    path.push("kaulan");
    Some(path)
}

/// Get the config file path
///
/// # Returns
/// - `Some(PathBuf)` - Full path to config.json
/// - `None` - Config directory cannot be determined
pub fn get_config_path() -> Option<PathBuf> {
    let mut path = get_config_dir()?;
    path.push("config.json");
    Some(path)
}

/// Load music directory from config file
///
/// # Returns
/// - `Some(String)` - Music directory path from config
/// - `None` - Config doesn't exist, is invalid, or has no music_directory set
///
/// # Documentation
/// See [`docs/settings-and-database-management.md`](../../../docs/settings-and-database-management.md)
pub fn load_config() -> Option<String> {
    let config_path = get_config_path()?;
    if !config_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&config_path).ok()?;
    let config: Config = serde_json::from_str(&content).ok()?;
    config.music_directory
}

/// Save music directory to config file
///
/// # Arguments
/// * `music_directory` - Path to the music directory to save
///
/// # Returns
/// - `Ok(())` - Config saved successfully
/// - `Err(...)` - Failed to save config (directory creation or file write error)
///
/// # Documentation
/// See [`docs/settings-and-database-management.md`](../../../docs/settings-and-database-management.md)
///
/// # Behavior
/// - Creates config directory if it doesn't exist
/// - Preserves other config fields (device_id, device_name, etc.)
pub fn save_config(music_directory: &str) -> Result<(), Box<dyn std::error::Error>> {
    save_config_field(|config| Config {
        music_directory: Some(music_directory.to_string()),
        ..config
    })?;
    info!("Music directory saved to config: {}", music_directory);
    Ok(())
}

/// Load or create a unique device identifier
///
/// # Returns
/// A UUID v4 string that uniquely identifies this device installation
///
/// # Behavior
/// - If device_id exists in config: returns it
/// - If not: generates a new UUID and saves it to config
pub fn load_or_create_device_id() -> String {
    // Try to load existing device_id from config
    if let Ok(config) = load_full_config() {
        if let Some(id) = config.device_id {
            info!("Loaded existing device_id: {}", id);
            return id;
        }
    }

    // Generate new UUID
    let new_id = Uuid::new_v4().to_string();
    info!("Generated new device_id: {}", new_id);

    // Save to config
    let config_dir = match get_config_dir() {
        Some(dir) => dir,
        None => {
            info!("Cannot save device_id: no config directory available");
            return new_id;
        }
    };

    if !config_dir.exists() {
        let _ = fs::create_dir_all(&config_dir);
    }

    let config_path = match get_config_path() {
        Some(path) => path,
        None => {
            info!("Cannot save device_id: no config path available");
            return new_id;
        }
    };

    let mut config = load_full_config().unwrap_or_default();
    config.device_id = Some(new_id.clone());

    if let Ok(content) = serde_json::to_string_pretty(&config) {
        if fs::write(&config_path, content).is_ok() {
            info!("Saved device_id to config");
        }
    }

    new_id
}

fn is_generic_hostname(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    GENERIC_HOSTNAMES
        .iter()
        .any(|candidate| *candidate == normalized)
}

/// Get the configured device name.
///
/// # Returns
/// - `Some(String)` - Device name explicitly saved in config
/// - `None` - No configured device name
pub fn get_configured_device_name() -> Option<String> {
    if let Ok(config) = load_full_config() {
        if let Some(name) = config.device_name {
            return Some(name);
        }
    }

    None
}

/// Get a hostname-derived device name when the hostname is usable for display.
///
/// # Returns
/// - `Some(String)` - Hostname when it is non-generic
/// - `None` - Hostname missing, invalid UTF-8, or too generic for display
pub fn get_hostname_device_name() -> Option<String> {
    let hostname = gethostname::gethostname().into_string().ok()?;
    if is_generic_hostname(&hostname) {
        return None;
    }
    Some(hostname)
}

/// Build a deterministic fallback device name from the device ID.
///
/// Example: `Kaulan Player a1b2c3`
pub fn generate_fallback_device_name(device_id: &str) -> String {
    let short_id: String = device_id.chars().filter(|c| *c != '-').take(6).collect();
    if short_id.is_empty() {
        return "Kaulan Player".to_string();
    }
    format!("Kaulan Player {}", short_id)
}

/// Set the device name
///
/// # Arguments
/// * `name` - New device name (1-64 characters)
///
/// # Returns
/// - `Ok(())` - Device name saved successfully
/// - `Err(...)` - Invalid name or save failed
pub fn set_device_name(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if name.is_empty() || name.len() > 64 {
        return Err("Device name must be 1-64 characters".into());
    }

    save_config_field(|config| Config {
        device_name: Some(name.to_string()),
        ..config
    })?;

    info!("Device name saved to config: {}", name);
    Ok(())
}

/// Load whether periodic LAN discovery announcements are enabled.
///
/// This defaults to `true` for existing installations so IP changes are
/// repaired automatically. Manual scans remain available when it is disabled.
pub fn load_periodic_discovery_enabled() -> bool {
    load_full_config()
        .map(|config| config.periodic_discovery_enabled)
        .unwrap_or(true)
}

/// Persist the periodic LAN discovery setting.
pub fn save_periodic_discovery_enabled(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    save_config_field(|config| Config {
        periodic_discovery_enabled: enabled,
        ..config
    })?;
    info!("Periodic device discovery enabled: {}", enabled);
    Ok(())
}

/// Load enabled media types from config
///
/// # Returns
/// `["audio"]` if not set (backward-compatible default)
pub fn load_media_types() -> Vec<String> {
    load_full_config()
        .ok()
        .and_then(|c| c.media_types)
        .unwrap_or_else(|| vec!["audio".to_string()])
}

/// Save media types to config file
///
/// # Arguments
/// * `media_types` - Slice of enabled types, e.g. `["audio", "video"]`
///
/// # Validation
/// - Only "audio" and "video" are allowed values
/// - At least one type must be selected
pub fn save_media_types(media_types: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    for mt in media_types {
        if mt != "audio" && mt != "video" {
            return Err(format!("Invalid media type: {}. Must be 'audio' or 'video'.", mt).into());
        }
    }
    if media_types.is_empty() {
        return Err("At least one media type must be selected.".into());
    }

    save_config_field(|config| Config {
        media_types: Some(media_types.to_vec()),
        ..config
    })?;
    info!("Media types saved to config: {:?}", media_types);
    Ok(())
}

/// Load the full config from file
///
/// # Returns
/// - `Ok(Config)` - Config loaded successfully
/// - `Err(...)` - Config doesn't exist or is invalid
fn load_full_config() -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = get_config_path().ok_or("No config path")?;
    if !config_path.exists() {
        return Err("Config file doesn't exist".into());
    }

    let content = fs::read_to_string(&config_path)?;
    let config: Config = serde_json::from_str(&content)?;
    Ok(config)
}

/// Save a config field update
///
/// Preserves existing config fields while updating one field.
///
/// # Arguments
/// * `update_fn` - Function that takes the current config and returns the updated config
fn save_config_field<F>(update_fn: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(Config) -> Config,
{
    let config_dir = get_config_dir().ok_or("Failed to get config dir")?;

    // Create config directory if it doesn't exist
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    let config_path = get_config_path().ok_or("Failed to get config path")?;

    // Load existing config or use default
    let existing_config = load_full_config().unwrap_or_default();

    // Apply the update
    let new_config = update_fn(existing_config);

    // Write to file
    let content = serde_json::to_string_pretty(&new_config)?;
    fs::write(&config_path, content)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        load_media_types, load_periodic_discovery_enabled, save_media_types,
        save_periodic_discovery_enabled,
    };
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    fn config_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct ConfigEnvGuard {
        _temp_dir: TempDir,
        old_kaulan_config_dir: Option<String>,
        old_xdg_config_home: Option<String>,
        old_home: Option<String>,
        old_appdata: Option<String>,
    }

    impl ConfigEnvGuard {
        fn new() -> Self {
            let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
            let root = temp_dir.path().to_string_lossy().to_string();

            let old_kaulan_config_dir = std::env::var("KAULAN_CONFIG_DIR").ok();
            let old_xdg_config_home = std::env::var("XDG_CONFIG_HOME").ok();
            let old_home = std::env::var("HOME").ok();
            let old_appdata = std::env::var("APPDATA").ok();

            unsafe {
                std::env::set_var("KAULAN_CONFIG_DIR", &root);
                std::env::set_var("XDG_CONFIG_HOME", &root);
                std::env::set_var("HOME", &root);
                std::env::set_var("APPDATA", &root);
            }

            Self {
                _temp_dir: temp_dir,
                old_kaulan_config_dir,
                old_xdg_config_home,
                old_home,
                old_appdata,
            }
        }
    }

    impl Drop for ConfigEnvGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.old_kaulan_config_dir {
                    Some(value) => std::env::set_var("KAULAN_CONFIG_DIR", value),
                    None => std::env::remove_var("KAULAN_CONFIG_DIR"),
                }
                match &self.old_xdg_config_home {
                    Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                    None => std::env::remove_var("XDG_CONFIG_HOME"),
                }
                match &self.old_home {
                    Some(value) => std::env::set_var("HOME", value),
                    None => std::env::remove_var("HOME"),
                }
                match &self.old_appdata {
                    Some(value) => std::env::set_var("APPDATA", value),
                    None => std::env::remove_var("APPDATA"),
                }
            }
        }
    }

    #[test]
    fn load_media_types_defaults_to_audio() {
        let _lock = config_env_lock().lock().expect("lock poisoned");
        let _guard = ConfigEnvGuard::new();

        assert_eq!(load_media_types(), vec!["audio".to_string()]);
    }

    #[test]
    fn save_media_types_round_trips_valid_values() {
        let _lock = config_env_lock().lock().expect("lock poisoned");
        let _guard = ConfigEnvGuard::new();

        let media_types = vec!["audio".to_string(), "video".to_string()];
        save_media_types(&media_types).expect("save should succeed");

        assert_eq!(load_media_types(), media_types);
    }

    #[test]
    fn save_media_types_rejects_invalid_value() {
        let _lock = config_env_lock().lock().expect("lock poisoned");
        let _guard = ConfigEnvGuard::new();

        let err = save_media_types(&["image".to_string()]).expect_err("save should fail");
        assert!(err.to_string().contains("Invalid media type"));
    }

    #[test]
    fn save_media_types_requires_at_least_one_value() {
        let _lock = config_env_lock().lock().expect("lock poisoned");
        let _guard = ConfigEnvGuard::new();

        let err = save_media_types(&[]).expect_err("save should fail");
        assert!(err
            .to_string()
            .contains("At least one media type must be selected"));
    }

    #[test]
    fn periodic_discovery_defaults_to_enabled_and_round_trips() {
        let _lock = config_env_lock().lock().expect("lock poisoned");
        let _guard = ConfigEnvGuard::new();

        assert!(load_periodic_discovery_enabled());
        save_periodic_discovery_enabled(false).expect("save should succeed");
        assert!(!load_periodic_discovery_enabled());
    }
}
