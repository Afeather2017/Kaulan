//! Integration tests for timestamp-based music streaming
//!
//! Tests the query parameter based seeking feature that allows clients
//! to request audio starting from a specific timestamp.

use actix_web::{http::StatusCode, test, web, App};
use kaulan::{get_music_by_id, AppState};
use sea_orm::{
    sea_query::TableCreateStatement, ConnectionTrait, Database, DatabaseConnection, DbErr, Schema,
};
use std::fs;
use std::path::Path;
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

    Ok(db)
}

/// Helper function to create a test music entry
async fn create_test_music(
    db: &DatabaseConnection,
    id: i32,
    filename: &str,
    file_path: &str,
) -> Result<(), DbErr> {
    use chrono::Utc;
    use kaulan::entities::music::ActiveModel as MusicActiveModel;
    use sea_orm::{ActiveModelTrait, ActiveValue, Set};

    let music = MusicActiveModel {
        id: ActiveValue::Set(id),
        filename: Set(filename.to_string()),
        file_path: Set(file_path.to_string()),
        lufs: Set(Some(-12.0)),
        created_at: Set(Utc::now()),
    };

    music.insert(db).await?;
    Ok(())
}

/// Helper function to create a test audio file
fn create_test_audio_file(path: &str, size_bytes: usize) -> std::io::Result<()> {
    // Create parent directories if they don't exist
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    // Create a file with the specified size (filled with zeros)
    let content = vec![0u8; size_bytes];
    fs::write(path, content)?;
    Ok(())
}

/// Calculate expected start byte from timestamp
fn calculate_start_byte(timestamp: f64, duration: f64, file_size: u64) -> u64 {
    ((timestamp / duration) * file_size as f64).floor() as u64
}

#[actix_web::test]
async fn test_timestamp_seek_valid() {
    // Setup: Create test database and file
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let test_file_path = "/tmp/test_music_seek/audio.mp3";
    let file_size: usize = 3_145_728; // ~3 MB file

    create_test_audio_file(test_file_path, file_size).expect("Failed to create test audio file");

    create_test_music(&db, 1, "audio.mp3", test_file_path)
        .await
        .expect("Failed to create test music entry");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music_seek".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_music_by_id),
    )
    .await;

    // Test: Request with timestamp parameter (30 seconds into a 180 second file)
    let timestamp = 30.0;
    let duration = 180.0;
    let expected_start = calculate_start_byte(timestamp, duration, file_size as u64);

    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/music/id/1?t={}&duration={}",
            timestamp, duration
        ))
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Assert: HTTP 206 Partial Content
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);

    // Assert: Content-Range header shows correct byte range
    let content_range = resp
        .headers()
        .get("Content-Range")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_range.contains(&format!("bytes {}-", expected_start)));

    // Assert: X-Seek-Timestamp header is present
    let seek_timestamp = resp
        .headers()
        .get("X-Seek-Timestamp")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(seek_timestamp, timestamp.to_string());

    // Cleanup
    let _ = fs::remove_file(test_file_path);
    let _ = fs::remove_dir_all("/tmp/test_music_seek");
}

#[actix_web::test]
async fn test_timestamp_seek_start() {
    // Test that t=0 streams from beginning
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let test_file_path = "/tmp/test_music_seek_start/audio.mp3";
    let file_size: usize = 1_048_576; // 1 MB

    create_test_audio_file(test_file_path, file_size).expect("Failed to create test audio file");

    create_test_music(&db, 1, "audio.mp3", test_file_path)
        .await
        .expect("Failed to create test music entry");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music_seek_start".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_music_by_id),
    )
    .await;

    // t=0 should still return 206 but start from byte 0
    let req = test::TestRequest::get()
        .uri("/api/music/id/1?t=0&duration=120")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);

    let content_range = resp
        .headers()
        .get("Content-Range")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_range.starts_with("bytes 0-"));

    // Cleanup
    let _ = fs::remove_file(test_file_path);
    let _ = fs::remove_dir_all("/tmp/test_music_seek_start");
}

#[actix_web::test]
async fn test_timestamp_seek_end() {
    // Test that t=duration streams from end
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let test_file_path = "/tmp/test_music_seek_end/audio.mp3";
    let file_size: usize = 1_048_576; // 1 MB

    create_test_audio_file(test_file_path, file_size).expect("Failed to create test audio file");

    create_test_music(&db, 1, "audio.mp3", test_file_path)
        .await
        .expect("Failed to create test music entry");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music_seek_end".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_music_by_id),
    )
    .await;

    // t=duration should stream from near the end
    let duration = 120.0;
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/music/id/1?t={}&duration={}",
            duration, duration
        ))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);

    // Content-Range should start near file_size - 1 (allowing for floating point precision)
    let content_range = resp
        .headers()
        .get("Content-Range")
        .unwrap()
        .to_str()
        .unwrap();
    // Extract the start byte from Content-Range header (format: "bytes {start}-{end}/{size}")
    let range_parts: Vec<&str> = content_range
        .split(' ')
        .nth(1)
        .unwrap()
        .split('/')
        .nth(0)
        .unwrap()
        .split('-')
        .collect();
    let start_byte: u64 = range_parts[0].parse().unwrap();
    // Should start within 100 bytes of the end (due to floating point precision)
    assert!(start_byte >= file_size as u64 - 100);

    // Cleanup
    let _ = fs::remove_file(test_file_path);
    let _ = fs::remove_dir_all("/tmp/test_music_seek_end");
}

#[actix_web::test]
async fn test_timestamp_seek_negative() {
    // Test that negative timestamp falls back to full stream
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let test_file_path = "/tmp/test_music_seek_negative/audio.mp3";

    create_test_audio_file(test_file_path, 1024).expect("Failed to create test audio file");

    create_test_music(&db, 1, "audio.mp3", test_file_path)
        .await
        .expect("Failed to create test music entry");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music_seek_negative".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_music_by_id),
    )
    .await;

    // Negative timestamp should return 200 OK (fallback)
    let req = test::TestRequest::get()
        .uri("/api/music/id/1?t=-10&duration=120")
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Should fallback to full stream
    assert_eq!(resp.status(), StatusCode::OK);

    // Cleanup
    let _ = fs::remove_file(test_file_path);
    let _ = fs::remove_dir_all("/tmp/test_music_seek_negative");
}

#[actix_web::test]
async fn test_timestamp_seek_exceeds_duration() {
    // Test that t > duration falls back to full stream
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let test_file_path = "/tmp/test_music_seek_exceeds/audio.mp3";

    create_test_audio_file(test_file_path, 1024).expect("Failed to create test audio file");

    create_test_music(&db, 1, "audio.mp3", test_file_path)
        .await
        .expect("Failed to create test music entry");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music_seek_exceeds".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_music_by_id),
    )
    .await;

    // t > duration should return 200 OK (fallback)
    let req = test::TestRequest::get()
        .uri("/api/music/id/1?t=200&duration=120")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    // Cleanup
    let _ = fs::remove_file(test_file_path);
    let _ = fs::remove_dir_all("/tmp/test_music_seek_exceeds");
}

#[actix_web::test]
async fn test_timestamp_seek_missing_duration() {
    // Test that providing t without duration falls back to full stream
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let test_file_path = "/tmp/test_music_seek_missing/audio.mp3";

    create_test_audio_file(test_file_path, 1024).expect("Failed to create test audio file");

    create_test_music(&db, 1, "audio.mp3", test_file_path)
        .await
        .expect("Failed to create test music entry");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music_seek_missing".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_music_by_id),
    )
    .await;

    // t without duration should return 200 OK (fallback)
    let req = test::TestRequest::get()
        .uri("/api/music/id/1?t=30")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    // Cleanup
    let _ = fs::remove_file(test_file_path);
    let _ = fs::remove_dir_all("/tmp/test_music_seek_missing");
}

#[actix_web::test]
async fn test_timestamp_seek_invalid_id() {
    // Test that 404 is returned for invalid ID even with valid params
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music_seek_invalid".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_music_by_id),
    )
    .await;

    // Non-existent ID should return 404
    let req = test::TestRequest::get()
        .uri("/api/music/id/999?t=30&duration=180")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn test_normal_request_without_timestamp() {
    // Test that requests without timestamp parameters work as before
    let db = setup_test_db()
        .await
        .expect("Failed to setup test database");
    let test_file_path = "/tmp/test_music_normal/audio.mp3";

    create_test_audio_file(test_file_path, 1024).expect("Failed to create test audio file");

    create_test_music(&db, 1, "audio.mp3", test_file_path)
        .await
        .expect("Failed to create test music entry");

    let discovery_state = Arc::new(kaulan::discovery::types::DiscoveryState::new(
        "test-id".to_string(),
        "Test Player".to_string(),
        2080,
    ));
    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music_normal".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
        discovery: discovery_state,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_music_by_id),
    )
    .await;

    // Normal request without timestamp should return 200 OK
    let req = test::TestRequest::get().uri("/api/music/id/1").to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    // Should have Accept-Ranges header
    assert!(resp.headers().get("Accept-Ranges").is_some());

    // Should NOT have X-Seek-Timestamp header
    assert!(resp.headers().get("X-Seek-Timestamp").is_none());

    // Cleanup
    let _ = fs::remove_file(test_file_path);
    let _ = fs::remove_dir_all("/tmp/test_music_normal");
}
