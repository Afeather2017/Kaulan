//! Integration tests for the Collections API

use actix_web::{test, App, http::StatusCode, web};
use serde_json::json;
use kaulan::{
    AppState,
    get_all_collections,
    get_collection,
    create_collection,
    delete_collection,
    get_collection_items,
    add_to_collection,
    remove_from_collection,
    get_playlists_collection_mode,
};
use sea_orm::{Database, DatabaseConnection, DbErr, ConnectionTrait, Schema, sea_query::{TableCreateStatement}};
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

    // Create collection table
    let collection_stmt: TableCreateStatement = schema
        .create_table_from_entity(kaulan::entities::collection::Entity)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&collection_stmt)).await?;

    // Create collection_item table
    let collection_item_stmt: TableCreateStatement = schema
        .create_table_from_entity(kaulan::entities::collection_item::Entity)
        .if_not_exists()
        .to_owned();
    db.execute(backend.build(&collection_item_stmt)).await?;

    Ok(db)
}

/// Helper function to create a test music entry
async fn create_test_music(db: &DatabaseConnection, filename: &str, file_path: &str) -> Result<i32, DbErr> {
    use kaulan::entities::music::{ActiveModel as MusicActiveModel};
    use sea_orm::{ActiveModelTrait, Set};
    use chrono::Utc;

    let music = MusicActiveModel {
        filename: Set(filename.to_string()),
        file_path: Set(file_path.to_string()),
        lufs: Set(Some(-12.0)),
        created_at: Set(Utc::now()),
        ..Default::default()
    };

    let result = music.insert(db).await?;
    Ok(result.id)
}

/// Helper function to create a test collection
async fn create_test_collection(db: &DatabaseConnection, name: &str) -> Result<i32, DbErr> {
    use kaulan::entities::collection::{ActiveModel as CollectionActiveModel};
    use sea_orm::{ActiveModelTrait, Set};
    use chrono::Utc;

    let collection = CollectionActiveModel {
        name: Set(name.to_string()),
        created_at: Set(Utc::now()),
        ..Default::default()
    };

    let result = collection.insert(db).await?;
    Ok(result.id)
}

/// Helper function to add a music item to a collection
async fn add_music_to_collection(db: &DatabaseConnection, collection_id: i32, music_id: i32) -> Result<(), DbErr> {
    use kaulan::entities::collection_item::{ActiveModel as CollectionItemActiveModel};
    use sea_orm::{ActiveModelTrait, Set};
    use chrono::Utc;

    let item = CollectionItemActiveModel {
        collection_id: Set(collection_id),
        music_id: Set(music_id),
        created_at: Set(Utc::now()),
        ..Default::default()
    };

    item.insert(db).await?;
    Ok(())
}

#[actix_web::test]
async fn test_get_all_collections_empty() {
    // Setup test database
    let db = setup_test_db().await.expect("Failed to setup test database");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_all_collections)
    ).await;

    let req = test::TestRequest::get()
        .uri("/api/collections")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let collections: Vec<serde_json::Value> = test::read_body_json(resp).await;
    assert_eq!(collections.len(), 0);
}

#[actix_web::test]
async fn test_get_playlists_collection_mode_empty() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_playlists_collection_mode)
    ).await;

    let req = test::TestRequest::get()
        .uri("/api/playlists/collection-mode")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let playlists: serde_json::Value = test::read_body_json(resp).await;
    // Should have "所有音乐" key even if empty
    assert!(playlists.is_object());
    // Note: when there's no music in the database, "所有音乐" key won't exist
    // because no playlists are created in the empty case
}

#[actix_web::test]
async fn test_create_collection() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(create_collection)
            .service(get_all_collections)
    ).await;

    // Create a new collection
    let create_req = test::TestRequest::post()
        .uri("/api/collections")
        .set_json(json!({ "name": "Test Collection" }))
        .to_request();

    let create_resp = test::call_service(&app, create_req).await;
    assert_eq!(create_resp.status(), StatusCode::OK);

    let created_collection: serde_json::Value = test::read_body_json(create_resp).await;
    assert_eq!(created_collection["name"], "Test Collection");
    assert!(created_collection["id"].is_number());

    // Verify the collection exists
    let get_req = test::TestRequest::get()
        .uri("/api/collections")
        .to_request();

    let get_resp = test::call_service(&app, get_req).await;
    let collections: Vec<serde_json::Value> = test::read_body_json(get_resp).await;
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0]["name"], "Test Collection");
}

#[actix_web::test]
async fn test_create_duplicate_collection_fails() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(create_collection)
    ).await;

    let collection_name = "Duplicate Collection";

    // Create first collection
    let req1 = test::TestRequest::post()
        .uri("/api/collections")
        .set_json(json!({ "name": collection_name }))
        .to_request();

    let resp1 = test::call_service(&app, req1).await;
    assert_eq!(resp1.status(), StatusCode::OK);

    // Try to create duplicate collection
    let req2 = test::TestRequest::post()
        .uri("/api/collections")
        .set_json(json!({ "name": collection_name }))
        .to_request();

    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn test_get_collection_by_id() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Create a test collection
    let collection_id = create_test_collection(&db, "Test Collection").await.expect("Failed to create test collection");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_collection)
    ).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/collections/{}", collection_id))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let collection: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(collection["id"], collection_id);
    assert_eq!(collection["name"], "Test Collection");
}

#[actix_web::test]
async fn test_get_nonexistent_collection_by_id() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_collection)
    ).await;

    let req = test::TestRequest::get()
        .uri("/api/collections/999")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn test_delete_collection() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Create a test collection
    let collection_id = create_test_collection(&db, "To Delete").await.expect("Failed to create test collection");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(delete_collection)
            .service(get_all_collections)
    ).await;

    // Delete the collection
    let delete_req = test::TestRequest::delete()
        .uri(&format!("/api/collections/{}", collection_id))
        .to_request();

    let delete_resp = test::call_service(&app, delete_req).await;
    assert_eq!(delete_resp.status(), StatusCode::OK);

    // Verify the collection is deleted
    use kaulan::entities::collection::Entity as CollectionEntity;
    use sea_orm::EntityTrait;

    let deleted_collection = CollectionEntity::find_by_id(collection_id)
        .one(&db)
        .await
        .expect("Database query failed");

    assert!(deleted_collection.is_none());
}

#[actix_web::test]
async fn test_delete_nonexistent_collection() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(delete_collection)
    ).await;

    let req = test::TestRequest::delete()
        .uri("/api/collections/999")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn test_get_collection_items() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Create test music entries
    let music_id1 = create_test_music(&db, "song1.mp3", "song1.mp3").await.expect("Failed to create music 1");
    let music_id2 = create_test_music(&db, "song2.mp3", "song2.mp3").await.expect("Failed to create music 2");

    // Create a test collection
    let collection_id = create_test_collection(&db, "My Collection").await.expect("Failed to create collection");

    // Add music to collection
    add_music_to_collection(&db, collection_id, music_id1).await.expect("Failed to add music 1");
    add_music_to_collection(&db, collection_id, music_id2).await.expect("Failed to add music 2");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_collection_items)
    ).await;

    let req = test::TestRequest::get()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let collection_with_songs: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(collection_with_songs["id"], collection_id);
    assert_eq!(collection_with_songs["name"], "My Collection");
    assert_eq!(collection_with_songs["songs"].as_array().unwrap().len(), 2);
}

#[actix_web::test]
async fn test_get_nonexistent_collection_items() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_collection_items)
    ).await;

    let req = test::TestRequest::get()
        .uri("/api/collections/999/items")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn test_add_to_collection() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Create test music entries
    let music_id1 = create_test_music(&db, "song1.mp3", "song1.mp3").await.expect("Failed to create music 1");
    let music_id2 = create_test_music(&db, "song2.mp3", "song2.mp3").await.expect("Failed to create music 2");

    // Create a test collection
    let collection_id = create_test_collection(&db, "My Collection").await.expect("Failed to create collection");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(add_to_collection)
            .service(get_collection_items)
    ).await;

    // Add music to collection
    let add_req = test::TestRequest::post()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .set_json(json!({ "music_ids": [music_id1, music_id2] }))
        .to_request();

    let add_resp = test::call_service(&app, add_req).await;
    assert_eq!(add_resp.status(), StatusCode::OK);

    // Verify the items were added
    let get_req = test::TestRequest::get()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .to_request();

    let get_resp = test::call_service(&app, get_req).await;
    let collection_with_songs: serde_json::Value = test::read_body_json(get_resp).await;
    assert_eq!(collection_with_songs["songs"].as_array().unwrap().len(), 2);
}

#[actix_web::test]
async fn test_add_to_nonexistent_collection() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Create a test music entry
    let music_id = create_test_music(&db, "song1.mp3", "song1.mp3").await.expect("Failed to create music");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(add_to_collection)
    ).await;

    let req = test::TestRequest::post()
        .uri("/api/collections/999/items")
        .set_json(json!({ "music_ids": [music_id] }))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn test_add_duplicate_to_collection() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Create test music entry
    let music_id = create_test_music(&db, "song1.mp3", "song1.mp3").await.expect("Failed to create music");

    // Create a test collection
    let collection_id = create_test_collection(&db, "My Collection").await.expect("Failed to create collection");

    // Add music to collection directly
    add_music_to_collection(&db, collection_id, music_id).await.expect("Failed to add music");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db,
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(add_to_collection)
            .service(get_collection_items)
    ).await;

    // Try to add the same music again (should succeed but not duplicate)
    let add_req = test::TestRequest::post()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .set_json(json!({ "music_ids": [music_id] }))
        .to_request();

    let add_resp = test::call_service(&app, add_req).await;
    assert_eq!(add_resp.status(), StatusCode::OK);

    // Verify only one item exists
    let get_req = test::TestRequest::get()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .to_request();

    let get_resp = test::call_service(&app, get_req).await;
    let collection_with_songs: serde_json::Value = test::read_body_json(get_resp).await;
    assert_eq!(collection_with_songs["songs"].as_array().unwrap().len(), 1);
}

#[actix_web::test]
async fn test_remove_from_collection() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Create test music entries
    let music_id1 = create_test_music(&db, "song1.mp3", "song1.mp3").await.expect("Failed to create music 1");
    let music_id2 = create_test_music(&db, "song2.mp3", "song2.mp3").await.expect("Failed to create music 2");

    // Create a test collection
    let collection_id = create_test_collection(&db, "My Collection").await.expect("Failed to create collection");

    // Add music to collection
    add_music_to_collection(&db, collection_id, music_id1).await.expect("Failed to add music 1");
    add_music_to_collection(&db, collection_id, music_id2).await.expect("Failed to add music 2");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(remove_from_collection)
            .service(get_collection_items)
    ).await;

    // Remove one music from collection
    let remove_req = test::TestRequest::delete()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .set_json(json!({ "music_ids": [music_id1] }))
        .to_request();

    let remove_resp = test::call_service(&app, remove_req).await;
    assert_eq!(remove_resp.status(), StatusCode::OK);

    // Verify only one item remains
    let get_req = test::TestRequest::get()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .to_request();

    let get_resp = test::call_service(&app, get_req).await;
    let collection_with_songs: serde_json::Value = test::read_body_json(get_resp).await;
    assert_eq!(collection_with_songs["songs"].as_array().unwrap().len(), 1);
}

#[actix_web::test]
async fn test_remove_nonexistent_item_from_collection() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    // Create a test collection
    let collection_id = create_test_collection(&db, "My Collection").await.expect("Failed to create collection");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(remove_from_collection)
            .service(get_collection_items)
    ).await;

    // Try to remove a music that doesn't exist in the collection
    let remove_req = test::TestRequest::delete()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .set_json(json!({ "music_ids": [999] }))
        .to_request();

    let remove_resp = test::call_service(&app, remove_req).await;
    // Should still return OK even if the item doesn't exist (idempotent)
    assert_eq!(remove_resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn test_collection_workflow() {
    let db = setup_test_db().await.expect("Failed to setup test database");

    let app_state = AppState {
        music_path: Arc::new("/tmp/test_music".to_string()),
        db_conn: db.clone(),
        scan_lock: Arc::new(TokioMutex::new(())),
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .service(get_all_collections)
            .service(create_collection)
            .service(get_collection_items)
            .service(add_to_collection)
            .service(remove_from_collection)
            .service(delete_collection)
    ).await;

    // Step 1: Create test music
    let music_id1 = create_test_music(&db, "song1.mp3", "song1.mp3").await.expect("Failed to create music 1");
    let music_id2 = create_test_music(&db, "song2.mp3", "song2.mp3").await.expect("Failed to create music 2");

    // Step 2: Create a collection
    let create_req = test::TestRequest::post()
        .uri("/api/collections")
        .set_json(json!({ "name": "Workflow Test Collection" }))
        .to_request();

    let create_resp = test::call_service(&app, create_req).await;
    assert_eq!(create_resp.status(), StatusCode::OK);
    let created_collection: serde_json::Value = test::read_body_json(create_resp).await;
    let collection_id: i32 = created_collection["id"].as_i64().unwrap() as i32;

    // Step 3: Add music to collection
    let add_req = test::TestRequest::post()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .set_json(json!({ "music_ids": [music_id1, music_id2] }))
        .to_request();

    let add_resp = test::call_service(&app, add_req).await;
    assert_eq!(add_resp.status(), StatusCode::OK);

    // Step 4: Verify collection has 2 songs
    let get_req = test::TestRequest::get()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .to_request();

    let get_resp = test::call_service(&app, get_req).await;
    let collection_with_songs: serde_json::Value = test::read_body_json(get_resp).await;
    assert_eq!(collection_with_songs["songs"].as_array().unwrap().len(), 2);

    // Step 5: Remove one song
    let remove_req = test::TestRequest::delete()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .set_json(json!({ "music_ids": [music_id1] }))
        .to_request();

    let remove_resp = test::call_service(&app, remove_req).await;
    assert_eq!(remove_resp.status(), StatusCode::OK);

    // Step 6: Verify collection has 1 song
    let get_req2 = test::TestRequest::get()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .to_request();

    let get_resp2 = test::call_service(&app, get_req2).await;
    let collection_with_songs2: serde_json::Value = test::read_body_json(get_resp2).await;
    assert_eq!(collection_with_songs2["songs"].as_array().unwrap().len(), 1);

    // Step 7: Delete the collection
    let delete_req = test::TestRequest::delete()
        .uri(&format!("/api/collections/{}", collection_id))
        .to_request();

    let delete_resp = test::call_service(&app, delete_req).await;
    assert_eq!(delete_resp.status(), StatusCode::OK);

    // Step 8: Verify collection is deleted
    let get_req3 = test::TestRequest::get()
        .uri(&format!("/api/collections/{}/items", collection_id))
        .to_request();

    let get_resp3 = test::call_service(&app, get_req3).await;
    assert_eq!(get_resp3.status(), StatusCode::NOT_FOUND);
}
