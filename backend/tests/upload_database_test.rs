//! Integration tests for file upload database update functionality
//!
//! This test directly verifies that update_database() correctly adds new files
//! to the database after they are written to disk.

use actix_web::{http::StatusCode, test, web, App};
use kaulan::{
    file_ops::{ScanRegistry, StdFsScanBackend},
    get_all_music, update_database, AppState,
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

    Ok(db)
}

#[actix_web::test]
async fn test_update_database_adds_new_files_to_database() {
    // Setup: Create temporary directory
    let temp_dir = std::env::temp_dir();
    let test_music_dir = temp_dir.join("test_upload_db_update");
    fs::create_dir_all(&test_music_dir).expect("Failed to create test directory");

    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // STEP 1: Check initial state - database should be empty
    use kaulan::entities::music::Entity as MusicEntity;
    let initial_music = MusicEntity::find()
        .all(&db)
        .await
        .expect("Failed to query database");
    assert_eq!(initial_music.len(), 0, "Database should be empty initially");

    // STEP 2: Write a test MP3 file to the test directory
    let dest_mp3 = test_music_dir.join("test_song.mp3");
    write_test_mp3(&dest_mp3);

    // Verify file exists on disk
    assert!(dest_mp3.exists(), "Test MP3 file should exist on disk");

    // STEP 3: Call update_database() to simulate what happens after upload
    println!("Calling update_database() after file copy...");
    let scan_registry = Arc::new(ScanRegistry::new());
    scan_registry.register(Arc::new(StdFsScanBackend::new(PathBuf::from(
        test_music_dir.clone(),
    ))));
    let result = update_database(&db, &scan_registry).await;

    assert!(result.is_ok(), "update_database should succeed");

    // STEP 4: CRITICAL TEST - Verify database now contains the file
    let music_after_update = MusicEntity::find()
        .all(&db)
        .await
        .expect("Failed to query database after update");

    println!("Database now has {} entries", music_after_update.len());

    assert_eq!(
        music_after_update.len(),
        1,
        "Database should contain 1 entry after update. Got {} entries: {:?}",
        music_after_update.len(),
        music_after_update
    );

    // Verify the entry has correct metadata
    let entry = &music_after_update[0];
    assert_eq!(entry.filename, "test_song.mp3", "Filename should match");
    let expected_path = dest_mp3
        .canonicalize()
        .unwrap_or(dest_mp3.clone())
        .to_string_lossy()
        .to_string();
    assert_eq!(
        entry.file_path, expected_path,
        "File path should be correct"
    );
    assert_eq!(
        entry.lufs, None,
        "LUFS should remain uncached after database update"
    );

    println!("SUCCESS: Database was updated with new file!");
    println!("  - Filename: {}", entry.filename);
    println!("  - File path: {}", entry.file_path);
    println!("  - LUFS: {:?}", entry.lufs);

    // Cleanup
    fs::remove_dir_all(test_music_dir).ok();
}

#[actix_web::test]
async fn test_upload_files_then_check_database_via_api() {
    // Setup: Create temporary directory
    let temp_dir = std::env::temp_dir();
    let test_music_dir = temp_dir.join("test_upload_api_integration");
    fs::create_dir_all(&test_music_dir).expect("Failed to create test directory");

    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // STEP 1: Manually write a file to simulate what upload does (write to disk)
    let dest_mp3 = test_music_dir.join("uploaded_song.mp3");
    write_test_mp3(&dest_mp3);

    // STEP 2: Call update_database to simulate the upload endpoint behavior
    let scan_registry = Arc::new(ScanRegistry::new());
    scan_registry.register(Arc::new(StdFsScanBackend::new(PathBuf::from(
        test_music_dir.clone(),
    ))));
    let _result = update_database(&db, &scan_registry).await;

    // STEP 3: Use GET /api/music endpoint to verify the file appears
    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let music_path_str = test_music_dir.to_string_lossy().to_string();
    let app_state = AppState {
        music_path: Arc::new(music_path_str.clone()),
        download_root: Arc::new(music_path_str.clone()),
        preview_root: Arc::new(format!("{}/.preview", music_path_str)),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
        download_jobs: Arc::new(kaulan::services::download::DownloadJobStore::new()),
        discovery: discovery_state,
        scan_registry: Arc::new(ScanRegistry::new()),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_all_music),
    )
    .await;

    let req = test::TestRequest::get().uri("/api/music").to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let all_music: serde_json::Value = test::read_body_json(resp).await;
    let music_array = all_music.as_array().unwrap();

    println!("GET /api/music returned {} files", music_array.len());
    for (i, item) in music_array.iter().enumerate() {
        println!("  [{}] {}", i, item);
    }

    assert_eq!(
        music_array.len(),
        1,
        "API should return 1 file after upload + update"
    );
    assert_eq!(
        music_array[0]["filename"], "uploaded_song.mp3",
        "API should return the uploaded file"
    );

    // Cleanup
    fs::remove_dir_all(test_music_dir).ok();
}

#[actix_web::test]
async fn test_update_database_with_multiple_new_files() {
    // Use unique directory name to avoid conflicts between test runs
    let unique_id = std::process::id();
    let temp_dir = std::env::temp_dir();
    let test_music_dir = temp_dir.join(format!("test_upload_multiple_{}", unique_id));
    // Clean up any existing directory from previous runs
    let _ = fs::remove_dir_all(&test_music_dir);
    fs::create_dir_all(&test_music_dir).expect("Failed to create test directory");

    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    // Create multiple test MP3 files
    let dest_files = ["song1.mp3", "song2.mp3"];

    for dest in &dest_files {
        let dest_path = test_music_dir.join(dest);
        write_test_mp3(&dest_path);
    }

    // Call update_database
    let scan_registry = Arc::new(ScanRegistry::new());
    scan_registry.register(Arc::new(StdFsScanBackend::new(PathBuf::from(
        test_music_dir.clone(),
    ))));
    let _result = update_database(&db, &scan_registry).await;

    // Verify database
    use kaulan::entities::music::Entity as MusicEntity;
    let music_after = MusicEntity::find()
        .all(&db)
        .await
        .expect("Failed to query database");

    println!("Database has {} entries after update", music_after.len());
    for entry in &music_after {
        println!("  - {} (LUFS: {:?})", entry.filename, entry.lufs);
    }

    assert_eq!(music_after.len(), 2, "Database should contain 2 entries");

    // Cleanup
    fs::remove_dir_all(test_music_dir).ok();
}
