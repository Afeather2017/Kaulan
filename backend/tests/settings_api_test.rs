//! Integration tests for the Settings and Database Management API

use actix_web::{http::StatusCode, test, web, App};
use chrono::Utc;
use kaulan::{get_music_directory, update_database, update_database_endpoint, AppState};
use sea_orm::{
    sea_query::TableCreateStatement, ConnectionTrait, Database, DatabaseConnection, DbErr,
    EntityTrait, Schema,
};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

/// Creates an in-memory SQLite database for testing
async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect("sqlite::memory:").await?;

    // Create tables
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Create music table
    let music_stmt: TableCreateStatement = schema
        .create_table_from_entity(kaulan::entities::music::Entity)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&music_stmt)).await?;

    // Create db_meta table
    let meta_stmt: TableCreateStatement = schema
        .create_table_from_entity(kaulan::entities::db_meta::Entity)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&meta_stmt)).await?;

    Ok(db)
}

#[actix_web::test]
async fn test_get_music_directory() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    let test_music_path = "/tmp/test_music".to_string();

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new(test_music_path.clone()),
        download_root: Arc::new(test_music_path.clone()),
        preview_root: Arc::new(format!("{}/.preview", test_music_path)),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_music_directory),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/settings/music-directory")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let response_body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(response_body["path"], test_music_path);
}

#[actix_web::test]
async fn test_update_database_empty() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Create a temporary directory for testing
    let temp_dir = std::env::temp_dir();
    let test_music_dir = temp_dir.join("test_music_update_empty");
    std::fs::create_dir_all(&test_music_dir).expect("Failed to create test directory");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new(test_music_dir.to_string_lossy().to_string()),
        download_root: Arc::new(test_music_dir.to_string_lossy().to_string()),
        preview_root: Arc::new(
            test_music_dir
                .join(".preview")
                .to_string_lossy()
                .to_string(),
        ),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(update_database_endpoint),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/database/update")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let response_body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(response_body["success"], true);
    assert_eq!(response_body["message"], "Database updated successfully");

    // Cleanup
    std::fs::remove_dir_all(test_music_dir).ok();
}

#[actix_web::test]
async fn test_update_database_with_new_files() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Create a temporary directory with a test music file
    let temp_dir = std::env::temp_dir();
    let test_music_dir = temp_dir.join("test_music_update_files");
    std::fs::create_dir_all(&test_music_dir).expect("Failed to create test directory");

    // Create a dummy test file
    let test_file = test_music_dir.join("test.mp3");
    std::fs::write(&test_file, b"dummy audio data").expect("Failed to create test file");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new(test_music_dir.to_string_lossy().to_string()),
        download_root: Arc::new(test_music_dir.to_string_lossy().to_string()),
        preview_root: Arc::new(
            test_music_dir
                .join(".preview")
                .to_string_lossy()
                .to_string(),
        ),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(update_database_endpoint),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/database/update")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let response_body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(response_body["success"], true);

    // Verify the file was added to the database
    use kaulan::entities::music::Entity as MusicEntity;
    let all_music = MusicEntity::find()
        .all(&db)
        .await
        .expect("Failed to query database");
    assert_eq!(all_music.len(), 1);
    // Note: The file might not have valid LUFS calculated if ffmpeg is not available
    // so we only assert that the file was inserted into the database

    // Cleanup
    std::fs::remove_dir_all(test_music_dir).ok();
}

#[actix_web::test]
async fn test_startup_update_skips_when_done() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Seed db_meta with initial_scan_done = true
    use kaulan::entities::db_meta::ActiveModel as DbMetaActiveModel;
    use sea_orm::ActiveModelTrait;
    let meta = DbMetaActiveModel {
        id: sea_orm::ActiveValue::Set(1),
        initial_scan_done: sea_orm::ActiveValue::Set(true),
        updated_at: sea_orm::ActiveValue::Set(Utc::now()),
    };
    meta.insert(&db).await.expect("Failed to insert db_meta");

    let temp_dir = std::env::temp_dir();
    let test_music_dir = temp_dir.join("test_startup_skip");
    std::fs::create_dir_all(&test_music_dir).expect("Failed to create test directory");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new(test_music_dir.to_string_lossy().to_string()),
        download_root: Arc::new(test_music_dir.to_string_lossy().to_string()),
        preview_root: Arc::new(
            test_music_dir
                .join(".preview")
                .to_string_lossy()
                .to_string(),
        ),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(update_database_endpoint),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/database/update?startup=true")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let response_body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(response_body["success"], true);
    assert_eq!(
        response_body["message"],
        "Startup scan skipped (already completed)"
    );

    // Cleanup
    std::fs::remove_dir_all(test_music_dir).ok();
}

#[actix_web::test]
async fn test_startup_update_runs_and_sets_flag() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    let temp_dir = std::env::temp_dir();
    let test_music_dir = temp_dir.join("test_startup_run");
    std::fs::create_dir_all(&test_music_dir).expect("Failed to create test directory");

    let test_file = test_music_dir.join("startup.mp3");
    std::fs::write(&test_file, b"dummy audio data").expect("Failed to create test file");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new(test_music_dir.to_string_lossy().to_string()),
        download_root: Arc::new(test_music_dir.to_string_lossy().to_string()),
        preview_root: Arc::new(
            test_music_dir
                .join(".preview")
                .to_string_lossy()
                .to_string(),
        ),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(update_database_endpoint),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/database/update?startup=true")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let response_body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(response_body["success"], true);
    assert_eq!(
        response_body["message"],
        "Startup scan completed successfully"
    );

    // Verify flag is set and a music entry exists
    use kaulan::entities::db_meta::Entity as DbMetaEntity;
    use kaulan::entities::music::Entity as MusicEntity;
    let meta = DbMetaEntity::find_by_id(1)
        .one(&db)
        .await
        .expect("Failed to read db_meta");
    assert!(meta.is_some());
    assert!(meta.unwrap().initial_scan_done);

    let all_music = MusicEntity::find()
        .all(&db)
        .await
        .expect("Failed to query database");
    assert_eq!(all_music.len(), 1);

    // Cleanup
    std::fs::remove_dir_all(test_music_dir).ok();
}

#[actix_web::test]
async fn test_update_database_function_with_empty_db() {
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Create a temporary directory
    let temp_dir = std::env::temp_dir();
    let test_music_dir = temp_dir.join("test_music_update_func");
    std::fs::create_dir_all(&test_music_dir).expect("Failed to create test directory");

    // Call update_database directly
    let result = update_database(test_music_dir.to_string_lossy().as_ref(), &db).await;

    // Should succeed even with no music files
    assert!(result.is_ok());

    // Cleanup
    std::fs::remove_dir_all(test_music_dir).ok();
}
