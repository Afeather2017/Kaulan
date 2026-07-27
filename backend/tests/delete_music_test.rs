//! Integration tests for batch music deletion.

use actix_web::{http::StatusCode, test, web, App};
use chrono::Utc;
use kaulan::{
    delete_music_batch,
    file_ops::{clear_scan_backends, register_scan_backend, StdFsScanBackend},
    update_database, AppState,
};
use sea_orm::{
    sea_query::TableCreateStatement, ConnectionTrait, Database, DatabaseConnection, DbErr,
    EntityTrait, Schema,
};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

fn write_test_mp3(path: &std::path::Path) {
    fs::write(path, b"fake mp3 test content").expect("Failed to write test MP3 file");
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect("sqlite::memory:").await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);
    let music_stmt: TableCreateStatement = schema
        .create_table_from_entity(kaulan::entities::music::Entity)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&music_stmt)).await?;
    Ok(db)
}

fn build_app_state(db: DatabaseConnection, music_path: String) -> AppState {
    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));

    AppState {
        music_path: Arc::new(music_path.clone()),
        download_root: Arc::new(music_path.clone()),
        preview_root: Arc::new(format!("{music_path}/.preview")),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        download_jobs: Arc::new(kaulan::services::download::DownloadJobStore::new()),
        discovery: discovery_state,
    }
}

#[actix_web::test]
async fn test_delete_music_batch_removes_file_and_database_entry() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let music_dir = temp_dir.path().join("music");
    fs::create_dir_all(&music_dir).expect("Failed to create music dir");
    let song_path = music_dir.join("delete-me.mp3");
    let lyric_path = music_dir.join("delete-me.lrc");
    write_test_mp3(&song_path);
    fs::write(&lyric_path, b"[00:00.00] lyric").expect("Failed to write lyric file");

    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    clear_scan_backends();
    register_scan_backend(Arc::new(StdFsScanBackend::new(PathBuf::from(
        music_dir.clone(),
    ))));
    update_database(&db)
        .await
        .expect("Failed to scan test music");

    let music_entry = kaulan::entities::music::Entity::find()
        .one(&db)
        .await
        .expect("Failed to query music entry")
        .expect("Expected one music entry");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(build_app_state(
                db.clone(),
                music_dir.to_string_lossy().to_string(),
            )))
            .service(delete_music_batch),
    )
    .await;

    let req = test::TestRequest::delete()
        .uri("/api/music/batch")
        .set_json(serde_json::json!({ "ids": [music_entry.id] }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["deleted_ids"], serde_json::json!([music_entry.id]));
    assert_eq!(body["failed"], serde_json::json!([]));
    assert!(
        !song_path.exists(),
        "Audio file should be deleted from disk"
    );
    assert!(
        !lyric_path.exists(),
        "Sidecar lyric should be deleted from disk"
    );

    let remaining = kaulan::entities::music::Entity::find()
        .all(&db)
        .await
        .expect("Failed to query remaining music");
    assert!(remaining.is_empty(), "Database entry should be deleted");
}

#[actix_web::test]
async fn test_delete_music_batch_keeps_database_entry_when_delete_fails() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    use kaulan::entities::music;
    use sea_orm::{ActiveModelTrait, Set};

    let model = music::ActiveModel {
        filename: Set("content-song.mp3".to_string()),
        file_path: Set("content://media/external/audio/media/42".to_string()),
        lufs: Set(None),
        created_at: Set(Utc::now()),
        ..Default::default()
    };
    let inserted = model
        .insert(&db)
        .await
        .expect("Failed to insert content-backed music entry");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(build_app_state(
                db.clone(),
                temp_dir.path().to_string_lossy().to_string(),
            )))
            .service(delete_music_batch),
    )
    .await;

    let req = test::TestRequest::delete()
        .uri("/api/music/batch")
        .set_json(serde_json::json!({ "ids": [inserted.id] }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["deleted_ids"], serde_json::json!([]));
    assert_eq!(body["failed"][0]["id"], inserted.id);

    let remaining = kaulan::entities::music::Entity::find()
        .all(&db)
        .await
        .expect("Failed to query remaining music");
    assert_eq!(
        remaining.len(),
        1,
        "Database entry should remain on failure"
    );
}
