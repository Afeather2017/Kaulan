//! Integration tests for file upload database update functionality
//!
//! This test directly verifies that update_database() correctly adds new files
//! to the database after they are written to disk.

use actix_web::{test, App, http::StatusCode, web};
use kaulan::{
    AppState,
    update_database,
    get_all_music,
};
use sea_orm::{Database, DatabaseConnection, DbErr, EntityTrait, ConnectionTrait, Schema, sea_query::TableCreateStatement};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use std::fs;

fn find_test_mp3_path() -> PathBuf {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("test_music");
    let mut stack = vec![base];

    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("mp3"))
                    .unwrap_or(false)
                {
                    return path;
                }
            }
        }
    }

    panic!("No .mp3 files found under test_music");
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

    let db = setup_test_db().await.expect("Failed to setup test database");

    // STEP 1: Check initial state - database should be empty
    use kaulan::entities::music::Entity as MusicEntity;
    let initial_music = MusicEntity::find()
        .all(&db)
        .await
        .expect("Failed to query database");
    assert_eq!(initial_music.len(), 0, "Database should be empty initially");

    // STEP 2: Copy a real MP3 file to the test directory
    let source_mp3 = find_test_mp3_path();
    let dest_mp3 = test_music_dir.join("test_song.mp3");
    fs::copy(&source_mp3, &dest_mp3).expect("Failed to copy test MP3 file");

    // Verify file exists on disk
    assert!(
        dest_mp3.exists(),
        "Test MP3 file should exist on disk"
    );

    // STEP 3: Call update_database() to simulate what happens after upload
    println!("Calling update_database() after file copy...");
    let result = update_database(
        &test_music_dir.to_string_lossy().to_string(),
        &db
    ).await;

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
    assert_eq!(entry.file_path, expected_path, "File path should be correct");
    assert!(entry.lufs.is_some(), "LUFS should be calculated");

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

    let db = setup_test_db().await.expect("Failed to setup test database");

    // STEP 1: Manually copy a file to simulate what upload does (write to disk)
    let source_mp3 = find_test_mp3_path();
    let dest_mp3 = test_music_dir.join("uploaded_song.mp3");
    fs::copy(&source_mp3, &dest_mp3).expect("Failed to copy test MP3 file");

    // STEP 2: Call update_database to simulate the upload endpoint behavior
    let music_path_str = test_music_dir.to_string_lossy().to_string();
    let _result = update_database(&music_path_str, &db).await;

    // STEP 3: Use GET /api/music endpoint to verify the file appears
    let app_state = AppState {
        music_path: Arc::new(music_path_str.clone()),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_all_music)
    ).await;

    let req = test::TestRequest::get()
        .uri("/api/music")
        .to_request();

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
        music_array[0]["filename"],
        "uploaded_song.mp3",
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

    let db = setup_test_db().await.expect("Failed to setup test database");

    // Copy multiple files using the same valid MP3 source
    let source_mp3 = find_test_mp3_path();
    let dest_files = ["song1.mp3", "song2.mp3"];

    for dest in &dest_files {
        let dest_path = test_music_dir.join(dest);
        fs::copy(&source_mp3, &dest_path)
            .expect(&format!("Failed to copy source MP3 to {}", dest));
    }

    // Call update_database
    let _result = update_database(
        &test_music_dir.to_string_lossy().to_string(),
        &db
    ).await;

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

    assert_eq!(
        music_after.len(),
        2,
        "Database should contain 2 entries"
    );

    // Cleanup
    fs::remove_dir_all(test_music_dir).ok();
}
