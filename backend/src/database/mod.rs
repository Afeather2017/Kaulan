use sea_orm::{Database, DatabaseConnection, DbErr, Schema, ConnectionTrait};
use sea_orm::sea_query::TableCreateStatement;
use crate::entities;
use tracing::{info, debug};

pub async fn establish_connection(music_path: &str) -> Result<DatabaseConnection, DbErr> {
    let db_path = format!("{}/music.db", music_path);
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

    Ok(db)
}
