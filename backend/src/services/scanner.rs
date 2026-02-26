//! Directory scanning and database operations for the music library.
//!
//! This module handles:
//! - Recursive scanning of music directories
//! - Database initialization on first run
//! - Database updates for new/modified/deleted files
//! - LUFS calculation integration

use crate::entities::music::{Entity as MusicEntity, ActiveModel as MusicActiveModel, Column as MusicColumn};
use crate::entities::db_meta::{Entity as DbMetaEntity, ActiveModel as DbMetaActiveModel};
use crate::lufsgen::get_lufs;
use crate::file_ops::{get_music_file_lister, MusicFileInfo};
use sea_orm::{DatabaseConnection, EntityTrait, Set, ActiveModelTrait, ModelTrait, ColumnTrait, QueryFilter, DbErr};
use std::path::Path;
use chrono::Utc;
use tracing::{debug, info, warn, error};
use std::io;

/// Recursively scan directory for audio files (desktop version using std::fs)
///
/// This function is kept for backward compatibility but now delegates
/// to the pluggable music file lister. On desktop, this uses the
/// StdMusicFileLister; on Android, it uses the MediaStoreMusicFileLister.
///
/// Returns a list of MusicFileInfo structs containing file path, filename,
/// and optional metadata (title, artist, album, duration).
pub async fn scan_directory_recursive(dir_path: &Path, _music_path: &str) -> Result<Vec<MusicFileInfo>, io::Error> {
    let lister = get_music_file_lister();
    let dir_str = dir_path.to_string_lossy();

    debug!("Scanning directory: {}", dir_str);

    match lister.list_music_files(&dir_str).await {
        Ok(files) => {
            debug!("Directory scan complete. Found {} audio files in {}", files.len(), dir_str);
            Ok(files)
        }
        Err(e) => {
            error!("Failed to scan directory {}: {}", dir_str, e);
            Err(e)
        }
    }
}

/// Check if a path is a content URI (Android MediaStore)
fn is_content_uri(path: &str) -> bool {
    path.starts_with("content://")
}

/// Normalize a file path for database storage
///
/// For regular paths, this canonicalizes the path to get a unique absolute path.
/// For content URIs (Android), returns the URI as-is.
fn normalize_path(path: &str) -> String {
    if is_content_uri(path) {
        path.to_string()
    } else {
        Path::new(path)
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string())
    }
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
    let audio_files = match scan_directory_recursive(Path::new(music_path), music_path).await {
        Ok(files) => files,
        Err(e) => {
            error!("Failed to scan directory during initialization: {}", e);
            return Err(DbErr::Custom(format!("Scan failed: {}", e)));
        }
    };
    info!("Found {} audio files in directory", audio_files.len());

    let mut new_files = 0;
    let mut existing_files = 0;

    for (idx, file_info) in audio_files.iter().enumerate() {
        let filename = &file_info.filename;
        let normalized_path = normalize_path(&file_info.path);

        debug!("Processing file {}/{}: {}", idx + 1, audio_files.len(), filename);

        match MusicEntity::find()
            .filter(MusicColumn::FilePath.eq(&normalized_path))
            .one(db_conn)
            .await
        {
            Ok(None) => {
                debug!("Inserting new file into database: {}", normalized_path);
                let music = MusicActiveModel {
                    filename: Set(filename.clone()),
                    file_path: Set(normalized_path.clone()),
                    lufs: Set(Some(0.5)),
                    created_at: Set(Utc::now()),
                    ..Default::default()
                };
                match music.insert(db_conn).await {
                    Ok(_) => {
                        debug!("Successfully inserted file into database");
                        new_files += 1;
                    }
                    Err(e) => error!("Failed to insert music {}: {}", normalized_path, e),
                }
            }
            Ok(Some(_)) => {
                debug!("File already exists in database: {}", normalized_path);
                existing_files += 1;
            }
            Err(e) => {
                error!("Database error while checking file {}: {}", normalized_path, e);
            }
        }
    }

    info!("Database initialization complete: {} new files, {} existing files", new_files, existing_files);
    Ok(())
}

/// Get or initialize the startup scan flag.
///
/// If the metadata row doesn't exist, it is created with `initial_scan_done = false`.
///
/// See docs/startup-scan.md for behavior details.
pub async fn get_initial_scan_done(db_conn: &DatabaseConnection) -> Result<bool, DbErr> {
    if let Some(meta) = DbMetaEntity::find_by_id(1).one(db_conn).await? {
        return Ok(meta.initial_scan_done);
    }

    let meta = DbMetaActiveModel {
        id: Set(1),
        initial_scan_done: Set(false),
        updated_at: Set(Utc::now()),
        ..Default::default()
    };
    meta.insert(db_conn).await?;
    Ok(false)
}

/// Update the startup scan flag.
///
/// See docs/startup-scan.md for behavior details.
pub async fn set_initial_scan_done(db_conn: &DatabaseConnection, done: bool) -> Result<(), DbErr> {
    if let Some(meta) = DbMetaEntity::find_by_id(1).one(db_conn).await? {
        let mut active_model: DbMetaActiveModel = meta.into();
        active_model.initial_scan_done = Set(done);
        active_model.updated_at = Set(Utc::now());
        active_model.update(db_conn).await?;
        return Ok(());
    }

    let meta = DbMetaActiveModel {
        id: Set(1),
        initial_scan_done: Set(done),
        updated_at: Set(Utc::now()),
        ..Default::default()
    };
    meta.insert(db_conn).await?;
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

    let audio_files = scan_directory_recursive(Path::new(music_path), music_path).await?;
    info!("[DB_UPDATE] Found {} audio files in directory", audio_files.len());

    let mut new_files = 0;
    let mut updated_files = 0;
    let mut skipped_files = 0;

    for (idx, file_info) in audio_files.iter().enumerate() {
        let filename = &file_info.filename;
        let normalized_path = normalize_path(&file_info.path);

        info!("[DB_UPDATE] [{}/{}] Checking file: {}", idx + 1, audio_files.len(), filename);
        debug!("[DB_UPDATE]   Path: {}", normalized_path);

        match MusicEntity::find()
            .filter(MusicColumn::FilePath.eq(&normalized_path))
            .one(db_conn)
            .await
        {
            Ok(None) => {
                info!("[DB_UPDATE]   NEW FILE detected - calling FFmpeg for LUFS...");
                if let Some(lufs_value) = get_lufs(&normalized_path) {
                    info!("[DB_UPDATE]   LUFS calculated: {} - inserting to database...", lufs_value);
                    let music = MusicActiveModel {
                        filename: Set(filename.clone()),
                        file_path: Set(normalized_path.clone()),
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
                } else if is_content_uri(&normalized_path) {
                    // Android MediaStore entries may not be accessible by FFmpeg.
                    // Insert with default LUFS so the file is still playable.
                    warn!("[DB_UPDATE]   LUFS unavailable for content URI - inserting with default LUFS: {}", filename);
                    let music = MusicActiveModel {
                        filename: Set(filename.clone()),
                        file_path: Set(normalized_path.clone()),
                        lufs: Set(Some(0.5)),
                        created_at: Set(Utc::now()),
                        ..Default::default()
                    };
                    match music.insert(db_conn).await {
                        Ok(_) => {
                            info!("[DB_UPDATE]   INSERTED: {} (LUFS: default 0.5)", filename);
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
                    if let Some(lufs_value) = get_lufs(&normalized_path) {
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
                    } else if is_content_uri(&normalized_path) {
                        // Keep default LUFS for content URIs when FFmpeg is unavailable.
                        warn!("[DB_UPDATE]   LUFS unavailable for content URI - keeping default for {}", filename);
                        skipped_files += 1;
                    } else {
                        warn!("[DB_UPDATE]   SKIPPED: Failed to calculate LUFS for {}", filename);
                    }
                } else {
                    debug!("[DB_UPDATE]   SKIPPED: File already in database with LUFS: {}", existing_music.lufs.unwrap());
                    skipped_files += 1;
                }
            }
            Err(e) => {
                error!("[DB_UPDATE]   DATABASE ERROR while checking file {}: {}", normalized_path, e);
            }
        }
    }

    info!("[DB_UPDATE] Checking for deleted files...");
    let mut deleted_files = 0;

    // Only check for deleted files on non-Android platforms (when we have real file paths)
    // On Android with content URIs, we can't reliably check if files still exist
    let has_content_uris = audio_files.iter().any(|f| is_content_uri(&f.path));

    if !has_content_uris {
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
    } else {
        info!("[DB_UPDATE] Skipping deleted file check (Android content URIs)");
    }

    info!("[DB_UPDATE] ========== DATABASE UPDATE COMPLETE ==========");
    info!("[DB_UPDATE] Summary: {} new, {} updated, {} skipped, {} deleted", new_files, updated_files, skipped_files, deleted_files);
    Ok(())
}
