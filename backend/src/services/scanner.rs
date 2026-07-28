//! Database operations for the music library.
//!
//! This module handles:
//! - Database initialization on first run
//! - Database updates for new/modified/deleted files
//! - LUFS values are calculated on-demand during playback (see handlers/lufs.rs)
//!
//! Library scanning is backend-based: callers populate a
//! [`crate::file_ops::ScanRegistry`] (owned by [`crate::types::AppState`])
//! with `ScanBackend` instances, then this module iterates them via
//! [`crate::file_ops::ScanRegistry::scan_all`].
//! See `docs/android/mediastore-integration.md` for the source-vs-scan flow.

use crate::entities::db_meta::{ActiveModel as DbMetaActiveModel, Entity as DbMetaEntity};
use crate::entities::music::{
    ActiveModel as MusicActiveModel, Column as MusicColumn, Entity as MusicEntity,
};
use crate::file_ops::{normalize_path, source_exists, ScanRegistry};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, ModelTrait, QueryFilter,
    Set,
};
use std::io;
use tracing::{debug, error, info};

/// Initialize database by scanning all registered backends.
///
/// Scans every backend on `registry`, inserts newly-discovered files into the
/// database (dedup by normalized path), and skips files that already exist.
/// New files are added with null LUFS (calculated on-demand during playback).
pub async fn initialize_database(
    db_conn: &DatabaseConnection,
    registry: &ScanRegistry,
) -> Result<(), sea_orm::DbErr> {
    info!("Initializing database from registered scan backends");
    let media_types = crate::config::load_media_types();
    let audio_files = registry
        .scan_all(&media_types)
        .await
        .map_err(|e| DbErr::Custom(format!("scan_all failed: {e}")))?;
    info!("Found {} audio files across backends", audio_files.len());

    let mut new_files: usize = 0;
    let mut existing_files: usize = 0;

    for (idx, file_info) in audio_files.iter().enumerate() {
        let filename = &file_info.filename;
        let normalized_path = normalize_path(&file_info.path);

        debug!(
            "Processing file {}/{}: {}",
            idx.saturating_add(1),
            audio_files.len(),
            filename
        );

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
                    parent_dir: Set(file_info.parent_dir.clone()),
                    lufs: Set(None),
                    created_at: Set(Utc::now()),
                    ..Default::default()
                };
                match music.insert(db_conn).await {
                    Ok(_) => {
                        debug!("Successfully inserted file into database");
                        new_files = new_files.saturating_add(1);
                    }
                    Err(e) => error!("Failed to insert music {}: {}", normalized_path, e),
                }
            }
            Ok(Some(_)) => {
                debug!("File already exists in database: {}", normalized_path);
                existing_files = existing_files.saturating_add(1);
            }
            Err(e) => {
                error!(
                    "Database error while checking file {}: {}",
                    normalized_path, e
                );
            }
        }
    }

    info!(
        "Database initialization complete: {} new files, {} existing files",
        new_files, existing_files
    );
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
    };
    meta.insert(db_conn).await?;
    Ok(())
}

/// Update database: scan for new files and remove deleted ones.
///
/// Scans every backend on `registry`, inserts newly-discovered files (dedup
/// by normalized path), and removes database entries for files that no
/// longer resolve through any registered `Source`.
///
/// LUFS values are calculated on-demand when songs start playing via the
/// `/api/music/{id}/precache-lufs` endpoint.
pub async fn update_database(
    db_conn: &DatabaseConnection,
    registry: &ScanRegistry,
) -> Result<(), std::io::Error> {
    info!("Starting database update");

    let media_types = crate::config::load_media_types();
    let audio_files = registry
        .scan_all(&media_types)
        .await
        .map_err(|e| io::Error::other(e.to_string()))?;
    info!(
        "Found {} media files during database update",
        audio_files.len()
    );

    let mut new_files: usize = 0;
    let mut skipped_files: usize = 0;

    for (idx, file_info) in audio_files.iter().enumerate() {
        let filename = &file_info.filename;
        let normalized_path = normalize_path(&file_info.path);

        debug!(
            "[DB_UPDATE] [{}/{}] Checking file: {}",
            idx.saturating_add(1),
            audio_files.len(),
            filename
        );
        debug!("[DB_UPDATE]   Path: {}", normalized_path);

        match MusicEntity::find()
            .filter(MusicColumn::FilePath.eq(&normalized_path))
            .one(db_conn)
            .await
        {
            Ok(None) => {
                debug!("[DB_UPDATE]   NEW FILE detected - inserting with null LUFS...");
                let music = MusicActiveModel {
                    filename: Set(filename.clone()),
                    file_path: Set(normalized_path.clone()),
                    parent_dir: Set(file_info.parent_dir.clone()),
                    lufs: Set(None),
                    created_at: Set(Utc::now()),
                    ..Default::default()
                };
                match music.insert(db_conn).await {
                    Ok(_) => {
                        debug!(
                            "[DB_UPDATE]   INSERTED: {} (LUFS: null, will be calculated on-demand)",
                            filename
                        );
                        new_files = new_files.saturating_add(1);
                    }
                    Err(e) => {
                        error!("[DB_UPDATE]   FAILED to insert {}: {}", filename, e);
                    }
                }
            }
            Ok(Some(_)) => {
                debug!(
                    "[DB_UPDATE]   SKIPPED: File already in database: {}",
                    filename
                );
                skipped_files = skipped_files.saturating_add(1);
            }
            Err(e) => {
                error!(
                    "[DB_UPDATE]   DATABASE ERROR while checking file {}: {}",
                    normalized_path, e
                );
            }
        }
    }

    debug!("[DB_UPDATE] Checking for deleted files...");
    let mut deleted_files: usize = 0;

    // Only check for deleted files on non-Android platforms (when we have real file paths)
    // On Android with content URIs, we can't reliably check if files still exist
    match MusicEntity::find().all(db_conn).await {
        Ok(all_music) => {
            for music in all_music {
                match source_exists(&music.file_path).await {
                    Ok(true) => {}
                    Ok(false) => {
                        let filename = music.filename.clone();
                        debug!(
                            "[DB_UPDATE] Deleting non-existent file from database: {}",
                            filename
                        );
                        match music.delete(db_conn).await {
                            Ok(_) => {
                                deleted_files = deleted_files.saturating_add(1);
                            }
                            Err(e) => {
                                error!("[DB_UPDATE] Failed to delete {}: {}", filename, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "[DB_UPDATE] Failed to check existence for {}: {}",
                            music.file_path, e
                        );
                    }
                }
            }
        }
        Err(e) => {
            error!(
                "[DB_UPDATE] Database error while checking for deleted files: {}",
                e
            );
        }
    }

    info!(
        "Database update complete: {} new, {} skipped, {} deleted",
        new_files, skipped_files, deleted_files
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::update_database;
    use crate::database::establish_connection;
    use crate::entities::music::Entity as MusicEntity;
    use crate::file_ops::{ScanRegistry, StdFsScanBackend};
    use sea_orm::EntityTrait;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[tokio::test]
    async fn update_database_scans_every_registered_std_fs_backend() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let music_root = temp_dir.path().join("music");
        let download_root = temp_dir.path().join("downloads");
        std::fs::create_dir_all(&music_root).unwrap();
        std::fs::create_dir_all(&download_root).unwrap();

        std::fs::write(music_root.join("local.mp3"), b"local").unwrap();
        std::fs::write(download_root.join("online.mp3"), b"downloaded").unwrap();

        let registry = ScanRegistry::new();
        registry.register(Arc::new(StdFsScanBackend::new(PathBuf::from(&music_root))));
        registry.register(Arc::new(StdFsScanBackend::new(PathBuf::from(
            &download_root,
        ))));

        let db_conn = establish_connection(music_root.to_str().unwrap())
            .await
            .unwrap();

        update_database(&db_conn, &registry).await.unwrap();

        let music_rows = MusicEntity::find().all(&db_conn).await.unwrap();
        let filenames = music_rows
            .iter()
            .map(|row| row.filename.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(music_rows.len(), 2);
        assert!(filenames.contains("local.mp3"));
        assert!(filenames.contains("online.mp3"));
    }
}
