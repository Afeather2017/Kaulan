//! Directory scanning and database operations for the music library.
//!
//! This module handles:
//! - Recursive scanning of music directories
//! - Database initialization on first run
//! - Database updates for new/modified/deleted files
//! - LUFS calculation integration

use crate::entities::music::{Entity as MusicEntity, ActiveModel as MusicActiveModel, Column as MusicColumn};
use crate::lufsgen::get_lufs;
use sea_orm::{DatabaseConnection, EntityTrait, Set, ActiveModelTrait, ModelTrait, ColumnTrait, QueryFilter};
use std::path::Path;
use std::fs;
use chrono::Utc;
use tracing::{debug, info, warn, error};

/// Supported audio file extensions
pub const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "ogg", "wav", "aac", "flac", "m4a", "opus"];

/// Recursively scan directory for audio files (desktop version using std::fs)
pub fn scan_directory_recursive(dir_path: &Path, _music_path: &str) -> Vec<std::path::PathBuf> {
    let mut audio_files = Vec::new();
    let dir_str = dir_path.to_string_lossy();

    debug!("Scanning directory: {}", dir_str);

    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if let Some(extension) = path.extension() {
                        let ext_str = extension.to_string_lossy().to_lowercase();
                        if SUPPORTED_EXTENSIONS.contains(&ext_str.as_str()) {
                            debug!("Found music file: {}", path.display());
                            audio_files.push(path);
                        }
                    }
                } else if file_type.is_dir() {
                    let mut sub_files = scan_directory_recursive(&entry.path(), _music_path);
                    audio_files.append(&mut sub_files);
                }
            }
        }
    }

    debug!("Directory scan complete. Found {} audio files in {}", audio_files.len(), dir_str);
    audio_files
}

/// Initialize database with music files (only insert if path not exists)
///
/// This function scans the music directory and adds any new files to the database.
/// Files that already exist in the database (matched by file_path) are skipped.
/// New files are added with a default LUFS value of 0.5.
///
/// # Arguments
/// * `music_path` - Path to the music directory to scan
/// * `db_conn` - Database connection
///
/// # Returns
/// - `Ok(())` - Database initialized successfully
/// - `Err(DbErr)` - Database error occurred
pub async fn initialize_database(music_path: &str, db_conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    info!("Initializing database with music from: {}", music_path);
    let audio_files = scan_directory_recursive(Path::new(music_path), music_path);
    info!("Found {} audio files in directory", audio_files.len());

    let mut new_files = 0;
    let mut existing_files = 0;

    for (idx, file_path) in audio_files.iter().enumerate() {
        let filename = file_path.file_name().unwrap().to_string_lossy().to_string();
        debug!("Processing file {}/{}: {}", idx + 1, audio_files.len(), filename);

        // Use absolute path consistently to avoid duplicates from path normalization issues
        // Canonicalize resolves "..", ".", symlinks, etc. to get a unique absolute path
        let absolute_path = file_path.canonicalize()
            .unwrap_or_else(|_| file_path.clone())
            .to_string_lossy()
            .to_string();

        match MusicEntity::find()
            .filter(MusicColumn::FilePath.eq(&absolute_path))
            .one(db_conn)
            .await
        {
            Ok(None) => {
                debug!("Inserting new file into database: {}", absolute_path);
                let music = MusicActiveModel {
                    filename: Set(filename),
                    file_path: Set(absolute_path.clone()),
                    lufs: Set(Some(0.5)),
                    created_at: Set(Utc::now()),
                    ..Default::default()
                };
                match music.insert(db_conn).await {
                    Ok(_) => {
                        debug!("Successfully inserted file into database");
                        new_files += 1;
                    }
                    Err(e) => error!("Failed to insert music {}: {}", absolute_path, e),
                }
            }
            Ok(Some(_)) => {
                debug!("File already exists in database: {}", absolute_path);
                existing_files += 1;
            }
            Err(e) => {
                error!("Database error while checking file {}: {}", absolute_path, e);
            }
        }
    }

    info!("Database initialization complete: {} new files, {} existing files", new_files, existing_files);
    Ok(())
}

/// Update database: scan for new files, calculate LUFS, and insert
///
/// This function performs a comprehensive database update:
/// - Scans the music directory for all audio files
/// - Adds new files with LUFS calculation
/// - Updates existing files that have no LUFS value or have the default 0.5
/// - Removes database entries for files that no longer exist on disk
///
/// # Arguments
/// * `music_path` - Path to the music directory to scan
/// * `db_conn` - Database connection
///
/// # Returns
/// - `Ok(())` - Database updated successfully
/// - `Err(io::Error)` - File system error occurred
pub async fn update_database(music_path: &str, db_conn: &DatabaseConnection) -> Result<(), std::io::Error> {
    info!("[DB_UPDATE] ========== STARTING DATABASE UPDATE ==========");
    info!("[DB_UPDATE] Music directory: {}", music_path);

    let audio_files = scan_directory_recursive(Path::new(music_path), music_path);
    info!("[DB_UPDATE] Found {} audio files in directory", audio_files.len());

    let mut new_files = 0;
    let mut updated_files = 0;
    let mut skipped_files = 0;

    for (idx, file_path) in audio_files.iter().enumerate() {
        let filename = file_path.file_name().unwrap().to_string_lossy().to_string();

        // Use absolute path consistently to avoid duplicates from path normalization issues
        // Canonicalize resolves "..", ".", symlinks, etc. to get a unique absolute path
        let absolute_path = file_path.canonicalize()
            .unwrap_or_else(|_| file_path.clone())
            .to_string_lossy()
            .to_string();

        info!("[DB_UPDATE] [{}/{}] Checking file: {}", idx + 1, audio_files.len(), filename);
        debug!("[DB_UPDATE]   Absolute path: {}", absolute_path);

        match MusicEntity::find()
            .filter(MusicColumn::FilePath.eq(&absolute_path))
            .one(db_conn)
            .await
        {
            Ok(None) => {
                info!("[DB_UPDATE]   NEW FILE detected - calling FFmpeg for LUFS...");
                if let Some(lufs_value) = get_lufs(&absolute_path) {
                    info!("[DB_UPDATE]   LUFS calculated: {} - inserting to database...", lufs_value);
                    let music = MusicActiveModel {
                        filename: Set(filename.clone()),
                        file_path: Set(absolute_path.clone()),
                        lufs: Set(Some(lufs_value)),
                        created_at: Set(Utc::now()),
                        ..Default::default()
                    };
                    match music.insert(db_conn).await {
                        Ok(_) => {
                            info!("[DB_UPDATE]   INSERTED: {} (LUFS: {})", filename, lufs_value);
                            new_files += 1;
                        }
                        Err(e) => {
                            error!("[DB_UPDATE]   FAILED to insert {}: {}", filename, e);
                        }
                    }
                } else {
                    warn!("[DB_UPDATE]   SKIPPED: Failed to calculate LUFS for {}", filename);
                }
            }
            Ok(Some(existing_music)) => {
                if existing_music.lufs.is_none() || existing_music.lufs == Some(0.5) {
                    info!("[DB_UPDATE]   EXISTING FILE without LUFS - updating...");
                    if let Some(lufs_value) = get_lufs(&absolute_path) {
                        let mut active_model: MusicActiveModel = existing_music.clone().into();
                        active_model.lufs = Set(Some(lufs_value));
                        match active_model.update(db_conn).await {
                            Ok(_) => {
                                info!("[DB_UPDATE]   UPDATED: {} (LUFS: {})", filename, lufs_value);
                                updated_files += 1;
                            }
                            Err(e) => {
                                error!("[DB_UPDATE]   FAILED to update {}: {}", filename, e);
                            }
                        }
                    } else {
                        warn!("[DB_UPDATE]   SKIPPED: Failed to calculate LUFS for {}", filename);
                    }
                } else {
                    debug!("[DB_UPDATE]   SKIPPED: File already in database with LUFS: {}", existing_music.lufs.unwrap());
                    skipped_files += 1;
                }
            }
            Err(e) => {
                error!("[DB_UPDATE]   DATABASE ERROR while checking file {}: {}", absolute_path, e);
            }
        }
    }

    info!("[DB_UPDATE] Checking for deleted files...");
    let mut deleted_files = 0;
    match MusicEntity::find().all(db_conn).await {
        Ok(all_music) => {
            for music in all_music {
                // Check if file exists - use the stored absolute path directly
                if !Path::new(&music.file_path).exists() {
                    let filename = music.filename.clone();
                    info!("[DB_UPDATE] Deleting non-existent file from database: {}", filename);
                    match music.delete(db_conn).await {
                        Ok(_) => {
                            deleted_files += 1;
                        }
                        Err(e) => {
                            error!("[DB_UPDATE] Failed to delete {}: {}", filename, e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            error!("[DB_UPDATE] Database error while checking for deleted files: {}", e);
        }
    }

    info!("[DB_UPDATE] ========== DATABASE UPDATE COMPLETE ==========");
    info!("[DB_UPDATE] Summary: {} new, {} updated, {} skipped, {} deleted", new_files, updated_files, skipped_files, deleted_files);
    Ok(())
}
