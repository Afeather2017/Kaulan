//! Configuration file management for persisting the music directory path.
//!
//! Documentation: [docs/settings-and-database-management.md](../../../docs/settings-and-database-management.md)
//!
//! This module provides configuration file management for persisting
//! the music directory path across application restarts.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;

/// Configuration file structure
///
/// # Config Format
/// ```json
/// {
///   "music_directory": "/path/to/music"
/// }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    music_directory: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            music_directory: None,
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
/// - Overwrites existing config file
pub fn save_config(music_directory: &str) -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = get_config_dir().ok_or("Failed to get config dir")?;

    // Create config directory if it doesn't exist
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    let config_path = get_config_path().ok_or("Failed to get config path")?;
    let config = Config {
        music_directory: Some(music_directory.to_string()),
    };

    let content = serde_json::to_string_pretty(&config)?;
    fs::write(&config_path, content)?;

    info!("Music directory saved to config: {}", music_directory);
    Ok(())
}
