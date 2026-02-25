use sea_orm::{Database, DatabaseConnection, DbErr, Schema, ConnectionTrait};
use sea_orm::sea_query::TableCreateStatement;
use crate::entities;
use tracing::{info, debug};

/// Get the database path based on platform and environment
fn get_database_path(music_path: &str) -> String {
    // Check if we're running on Android via Tauri
    if std::env::var("TAURI_PLATFORM").ok().as_deref() == Some("android") {
        // On Android, store database in app data directory
        let data_dir = std::env::var("TAURI_ANDROID_DATA_DIR")
            .ok()
            .unwrap_or_else(|| {
                // Fallback to using the music directory on Android if Tauri env var not set
                music_path.to_string()
            });

        if data_dir != music_path {
            // Ensure data directory exists
            let _ = std::fs::create_dir_all(&data_dir);
        }

        format!("{}/music.db", data_dir)
    } else {
        // On desktop platforms, store database in music directory
        format!("{}/music.db", music_path)
    }
}

pub async fn establish_connection(music_path: &str) -> Result<DatabaseConnection, DbErr> {
    let db_path = get_database_path(music_path);
    debug!("Connecting to database at: {}", db_path);
    let db = Database::connect(format!("sqlite://{}?mode=rwc", db_path)).await?;
    info!("Database connection established");

    // Create tables if they don't exist
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Create music table
    let music_stmt: TableCreateStatement = schema
        .create_table_from_entity(entities::music::Entity)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&music_stmt)).await?;
    debug!("Music table created/verified");

    // Create collection table
    let collection_stmt: TableCreateStatement = schema
        .create_table_from_entity(entities::collection::Entity)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&collection_stmt)).await?;
    debug!("Collection table created/verified");

    // Create collection_item table
    let collection_item_stmt: TableCreateStatement = schema
        .create_table_from_entity(entities::collection_item::Entity)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&collection_item_stmt)).await?;
    debug!("Collection_item table created/verified");

    // Create db_meta table for startup scan state
    let db_meta_stmt: TableCreateStatement = schema
        .create_table_from_entity(entities::db_meta::Entity)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&db_meta_stmt)).await?;
    debug!("Db_meta table created/verified");

    Ok(db)
}
