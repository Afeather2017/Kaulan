//! Integration tests for the file upload API endpoint `/api/files/upload`
//!
//! This test module verifies the single-file multipart upload functionality
//! using real music files from the `test-music/` directory.
//!
//! Related documentation: docs/file-upload-feature.md
//! Related source: backend/src/lib.rs (upload_files function)

use actix_web::{test, App, http, web};
use kaulan::{AppState, upload_files, get_all_music};
use sea_orm::{Database, DatabaseConnection, DbErr, ConnectionTrait, Schema, sea_query::TableCreateStatement};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::fs;
use actix_web::web::Bytes;

/// Path to test music files
const TEST_MUSIC_DIR: &str = "/home/afeather/Codes/kaulan/test-music";

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

/// Creates a multipart body with binary content for single file upload
fn create_multipart_body_bytes(files: &[(&str, &[u8])], target_path: Option<&str>) -> (String, Bytes) {
    let boundary = "---------------------------202022185716362916172375148227";
    let mut body = Vec::new();

    // Add target path field if provided
    if let Some(path) = target_path {
        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"targetPath\"\r\n\r\n{}\r\n",
            boundary, path
        ).as_bytes());
    }

    // Add files
    for (filename, content) in files {
        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{}\"\r\nContent-Type: audio/mpeg\r\n\r\n",
            boundary, filename
        ).as_bytes());
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    (boundary.to_string(), Bytes::from(body))
}

#[actix_web::test]
async fn test_upload_single_file_to_root() {
    // Setup temporary directory
    let temp_dir = std::env::temp_dir().join("test_upload_single");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let music_path = temp_dir.to_str().unwrap().to_string();

    let db = setup_test_db().await.expect("Failed to setup DB");

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(music_path.clone()),
        db_conn: db,
    });

    let app = test::init_service(
        App::new().app_data(app_state).service(upload_files)
    ).await;

    // Read test file
    let test_file_path = format!("{}/0.5sinwave.mp3", TEST_MUSIC_DIR);
    let file_content = fs::read(&test_file_path).expect("Failed to read test file");

    // Create multipart body
    let (boundary, body) = create_multipart_body_bytes(&[("0.5sinwave.mp3", &file_content)], None);

    // Send upload request
    let req = test::TestRequest::post()
        .uri("/api/files/upload")
        .insert_header((
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success(), "Upload should succeed");

    // Verify file was written to disk
    let dest_file = temp_dir.join("0.5sinwave.mp3");
    assert!(dest_file.exists(), "File should exist on disk");

    // Cleanup
    fs::remove_dir_all(temp_dir).ok();
}

#[actix_web::test]
async fn test_upload_to_subdirectory() {
    let temp_dir = std::env::temp_dir().join("test_upload_subdir");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let music_path = temp_dir.to_str().unwrap().to_string();

    let db = setup_test_db().await.expect("Failed to setup DB");

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(music_path.clone()),
        db_conn: db,
    });

    let app = test::init_service(
        App::new().app_data(app_state).service(upload_files)
    ).await;

    let test_file_path = format!("{}/1-m.mp3", TEST_MUSIC_DIR);
    let file_content = fs::read(&test_file_path).expect("Failed to read test file");

    // Upload to subdirectory "test-subfolder"
    let (boundary, body) = create_multipart_body_bytes(&[("1-m.mp3", &file_content)], Some("test-subfolder"));

    let req = test::TestRequest::post()
        .uri("/api/files/upload")
        .insert_header((
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());

    // Verify file exists in subdirectory
    let dest_file = temp_dir.join("test-subfolder").join("1-m.mp3");
    assert!(dest_file.exists(), "File should exist in subdirectory");

    // Cleanup
    fs::remove_dir_all(temp_dir).ok();
}

#[actix_web::test]
async fn test_upload_unsupported_file_type_rejected() {
    let temp_dir = std::env::temp_dir().join("test_upload_unsupported");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let music_path = temp_dir.to_str().unwrap().to_string();

    let db = setup_test_db().await.expect("Failed to setup DB");

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(music_path.clone()),
        db_conn: db,
    });

    let app = test::init_service(
        App::new().app_data(app_state).service(upload_files)
    ).await;

    // Create multipart body with an unsupported .exe file
    let fake_content = b"This is not a real audio file";
    let (boundary, body) = create_multipart_body_bytes(
        &[("malicious.exe", fake_content)],
        None
    );

    let req = test::TestRequest::post()
        .uri("/api/files/upload")
        .insert_header((
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // The request should succeed, but the file should be in failed list
    assert!(resp.status().is_success());

    // Verify the exe was NOT created
    assert!(!temp_dir.join("malicious.exe").exists());

    // Cleanup
    fs::remove_dir_all(temp_dir).ok();
}

#[actix_web::test]
async fn test_upload_path_traversal_protection() {
    let temp_dir = std::env::temp_dir().join("test_upload_traversal");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let music_path = temp_dir.to_str().unwrap().to_string();

    let db = setup_test_db().await.expect("Failed to setup DB");

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(music_path.clone()),
        db_conn: db,
    });

    let app = test::init_service(
        App::new().app_data(app_state).service(upload_files)
    ).await;

    let test_file_path = format!("{}/3-m.mp3", TEST_MUSIC_DIR);
    let file_content = fs::read(&test_file_path).expect("Failed to read test file");

    // Try to upload with path traversal in target path
    let (boundary, body) = create_multipart_body_bytes(
        &[("3-m.mp3", &file_content)],
        Some("../../../etc")
    );

    let req = test::TestRequest::post()
        .uri("/api/files/upload")
        .insert_header((
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Should be rejected
    assert!(!resp.status().is_success(), "Path traversal should be rejected");

    // Verify file was NOT created outside the music directory
    assert!(!temp_dir.join("../../../etc").join("3-m.mp3").exists()
            || !temp_dir.join("3-m.mp3").exists());

    // Cleanup
    fs::remove_dir_all(temp_dir).ok();
}

#[actix_web::test]
async fn test_upload_updates_database() {
    let temp_dir = std::env::temp_dir().join("test_upload_db_update");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let music_path = temp_dir.to_str().unwrap().to_string();

    let db = setup_test_db().await.expect("Failed to setup DB");

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(music_path.clone()),
        db_conn: db.clone(),
    });

    let app = test::init_service(
        App::new()
            .app_data(app_state)
            .service(upload_files)
            .service(get_all_music)
    ).await;

    // Verify database is empty initially
    let req_get = test::TestRequest::get()
        .uri("/api/music")
        .to_request();
    let resp_get = test::call_service(&app, req_get).await;
    let initial_music: serde_json::Value = test::read_body_json(resp_get).await;
    assert_eq!(initial_music.as_array().unwrap().len(), 0);

    // Upload a file
    let test_file_path = format!("{}/4-m.mp3", TEST_MUSIC_DIR);
    let file_content = fs::read(&test_file_path).expect("Failed to read test file");

    let (boundary, body) = create_multipart_body_bytes(&[("4-m.mp3", &file_content)], None);

    let req_upload = test::TestRequest::post()
        .uri("/api/files/upload")
        .insert_header((
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();

    let resp_upload = test::call_service(&app, req_upload).await;
    assert!(resp_upload.status().is_success());

    // Give the database update a moment to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Verify database now contains the uploaded file
    let req_get_after = test::TestRequest::get()
        .uri("/api/music")
        .to_request();
    let resp_get_after = test::call_service(&app, req_get_after).await;
    let music_after: serde_json::Value = test::read_body_json(resp_get_after).await;

    assert!(music_after.as_array().unwrap().len() > 0, "Database should contain uploaded file");

    // Cleanup
    fs::remove_dir_all(temp_dir).ok();
}

#[actix_web::test]
async fn test_upload_empty_request() {
    let temp_dir = std::env::temp_dir().join("test_upload_empty");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let music_path = temp_dir.to_str().unwrap().to_string();

    let db = setup_test_db().await.expect("Failed to setup DB");

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(music_path.clone()),
        db_conn: db,
    });

    let app = test::init_service(
        App::new().app_data(app_state).service(upload_files)
    ).await;

    // Send empty multipart request
    let boundary = "---------------------------202022185716362916172375148227";
    let body = format!("--{}--\r\n", boundary);

    let req = test::TestRequest::post()
        .uri("/api/files/upload")
        .insert_header((
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();

    let resp = test::call_service(&app, req).await;

    // Should return error or success with empty result
    assert!(resp.status().is_client_error() || resp.status().is_success());

    // Cleanup
    fs::remove_dir_all(temp_dir).ok();
}

#[actix_web::test]
async fn test_upload_to_nested_subdirectories() {
    let temp_dir = std::env::temp_dir().join("test_upload_nested");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");
    let music_path = temp_dir.to_str().unwrap().to_string();

    let db = setup_test_db().await.expect("Failed to setup DB");

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(music_path.clone()),
        db_conn: db,
    });

    let app = test::init_service(
        App::new().app_data(app_state).service(upload_files)
    ).await;

    let test_file_path = format!("{}/5-m.mp3", TEST_MUSIC_DIR);
    let file_content = fs::read(&test_file_path).expect("Failed to read test file");

    // Upload to nested subdirectory
    let (boundary, body) = create_multipart_body_bytes(
        &[("5-m.mp3", &file_content)],
        Some("level1/level2/level3")
    );

    let req = test::TestRequest::post()
        .uri("/api/files/upload")
        .insert_header((
            http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary),
        ))
        .set_payload(body)
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());

    // Verify file exists in nested subdirectory
    let dest_file = temp_dir.join("level1").join("level2").join("level3").join("5-m.mp3");
    assert!(dest_file.exists(), "File should exist in nested subdirectory");

    // Cleanup
    fs::remove_dir_all(temp_dir).ok();
}
