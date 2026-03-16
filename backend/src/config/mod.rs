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

/// Configuration file structure
///
/// # Config Format
/// ```json
/// {
///   "music_directory": "/path/to/music",
///   "device_id": "550e8400-e29b-41d4-a716-446655440000",
///   "device_name": "Living Room Player"
/// }
/// ```
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    /// Music directory path
    pub music_directory: Option<String>,

    /// Unique device identifier (UUID v4)
    pub device_id: Option<String>,

    /// Human-readable device name (user-configurable)
    pub device_name: Option<String>,

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

/// Get the device name
///
/// # Returns
/// - `Some(String)` - Device name from config, or hostname as fallback
/// - `None` - No device name available (very unlikely)
pub fn get_device_name() -> Option<String> {
    // Try to load from config first
    if let Ok(config) = load_full_config() {
        if let Some(name) = config.device_name {
            return Some(name);
        }
    }

    // Fallback to hostname if no device name is set
    gethostname::gethostname()
        .into_string()
        .ok()
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
