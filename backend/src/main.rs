use actix_web::{get, post, delete, web, App, HttpResponse, HttpServer, Responder};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use sea_orm::{DatabaseConnection, EntityTrait, ActiveModelTrait, Set, ColumnTrait, QueryFilter, ModelTrait, QuerySelect};
use chrono::Utc;

mod lufsgen;
use lufsgen::get_lufs;
mod entities;
mod database;
use entities::music::{Entity as MusicEntity, Model as MusicModel, ActiveModel as MusicActiveModel, Column as MusicColumn};
use entities::collection::{Entity as CollectionEntity, Model as CollectionModel, ActiveModel as CollectionActiveModel, Column as CollectionColumn};
use entities::collection_item::{Entity as CollectionItemEntity, Model as CollectionItemModel, ActiveModel as CollectionItemActiveModel, Column as CollectionItemColumn};
use database::establish_connection;

#[derive(Serialize, Deserialize)]
struct MusicResponse {
    id: i32,
    filename: String,
    file_path: String,
    lufs: Option<f64>,
    created_at: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct MusicInfo {
    name: String,
    lufs: f64,
    path: String,
}

#[derive(Serialize, Deserialize)]
struct Playlist {
    name: String,
    songs: Vec<MusicInfo>,
}

#[derive(Serialize, Deserialize)]
struct Collection {
    id: i32,
    name: String,
    created_at: String,
}

#[derive(Serialize, Deserialize)]
struct CollectionWithSongs {
    id: i32,
    name: String,
    songs: Vec<MusicInfo>,
}

#[derive(Deserialize)]
struct CreateCollectionRequest {
    name: String,
}

#[derive(Deserialize)]
struct AddToCollectionRequest {
    music_ids: Vec<i32>,
}

#[derive(Deserialize)]
struct RemoveFromCollectionRequest {
    music_ids: Vec<i32>,
}

struct AppState {
    music_path: String,
    db_conn: DatabaseConnection,
}

#[get("/api/music/{filename}")]
async fn get_music(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> impl Responder {
    let filename = path.into_inner();

    // Find music by filename
    match MusicEntity::find()
        .filter(MusicColumn::Filename.eq(&filename))
        .one(&data.db_conn)
        .await
    {
        Ok(Some(music)) => {
            let file_path = Path::new(&data.music_path).join(&music.file_path);

            match fs::read(&file_path) {
                Ok(content) => {
                    let mut response = HttpResponse::Ok();
                    response.insert_header(("Content-Type", "audio/mpeg"));
                    response.insert_header(("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0"));
                    response.insert_header(("Pragma", "no-cache"));
                    response.insert_header(("Expires", "0"));
                    response.body(content)
                }
                Err(_) => HttpResponse::NotFound().body("File not found"),
            }
        }
        Ok(None) => HttpResponse::NotFound().body("Music not found"),
        Err(_) => HttpResponse::InternalServerError().body("Database error"),
    }
}

#[get("/api/music")]
async fn get_all_music(data: web::Data<AppState>) -> impl Responder {
    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            let response: Vec<MusicResponse> = music_list
                .into_iter()
                .map(|music: MusicModel| MusicResponse {
                    id: music.id,
                    filename: music.filename,
                    file_path: music.file_path,
                    lufs: music.lufs,
                    created_at: music.created_at.to_rfc3339(),
                })
                .collect();
            HttpResponse::Ok().json(response)
        }
        Err(_) => HttpResponse::InternalServerError().body("Database error"),
    }
}

#[get("/api/playlists")]
async fn get_all_playlists(data: web::Data<AppState>) -> impl Responder {
    let mut playlists: std::collections::HashMap<String, Vec<MusicInfo>> = std::collections::HashMap::new();

    // Get all music from database
    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            for music in &music_list {
                let lufs_value = music.lufs.unwrap_or(0.5);
                let info = MusicInfo {
                    name: music.filename.clone(),
                    lufs: lufs_value,
                    path: music.file_path.clone(),
                };

                // Add all music to "所有音乐" playlist
                playlists.entry("所有音乐".to_string()).or_insert_with(Vec::new).push(info.clone());

                // Also add to folder-based playlists (use directory name only)
                if let Some(parent) = Path::new(&music.file_path).parent() {
                    if !parent.as_os_str().is_empty() {
                        if let Some(dir_name) = parent.file_name() {
                            let playlist_name = dir_name.to_string_lossy().to_string();
                            playlists.entry(playlist_name).or_insert_with(Vec::new).push(info);
                        }
                    }
                }
            }
        }
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    }

    HttpResponse::Ok().json(playlists)
}

#[get("/api/playlists/{name}")]
async fn get_playlist(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> impl Responder {
    let playlist_name = path.into_inner();
    let mut songs: Vec<MusicInfo> = Vec::new();

    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            for music in music_list {
                let lufs_value = music.lufs.unwrap_or(0.5);
                let info = MusicInfo {
                    name: music.filename.clone(),
                    lufs: lufs_value,
                    path: music.file_path.clone(),
                };

                // Check if this song belongs to the requested playlist
                let belongs_to_playlist = if playlist_name == "所有音乐" {
                    // "All Music" includes ALL songs
                    true
                } else {
                    // Check if the file's immediate parent directory name matches the playlist name
                    match Path::new(&music.file_path).parent() {
                        Some(parent) if !parent.as_os_str().is_empty() => {
                            parent.file_name()
                                .map(|name| name.to_string_lossy() == playlist_name)
                                .unwrap_or(false)
                        }
                        _ => false,
                    }
                };

                if belongs_to_playlist {
                    songs.push(info);
                }
            }
        }
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    }

    HttpResponse::Ok().json(Playlist {
        name: playlist_name,
        songs,
    })
}

// ============= Collection API Endpoints =============

/// Get all collections
#[get("/api/collections")]
async fn get_all_collections(data: web::Data<AppState>) -> impl Responder {
    match CollectionEntity::find()
        .all(&data.db_conn)
        .await
    {
        Ok(collections) => {
            let response: Vec<Collection> = collections
                .into_iter()
                .map(|c: CollectionModel| Collection {
                    id: c.id,
                    name: c.name,
                    created_at: c.created_at.to_rfc3339(),
                })
                .collect();
            HttpResponse::Ok().json(response)
        }
        Err(_) => HttpResponse::InternalServerError().body("Database error"),
    }
}

/// Create a new collection
#[post("/api/collections")]
async fn create_collection(
    req: web::Json<CreateCollectionRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    // Check if collection with same name exists
    match CollectionEntity::find()
        .filter(CollectionColumn::Name.eq(&req.name))
        .one(&data.db_conn)
        .await
    {
        Ok(Some(_)) => {
            return HttpResponse::BadRequest().body("Collection with this name already exists");
        }
        Ok(None) => {}
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    }

    let collection = CollectionActiveModel {
        name: Set(req.name.clone()),
        created_at: Set(Utc::now()),
        ..Default::default()
    };

    match collection.insert(&data.db_conn).await {
        Ok(c) => HttpResponse::Ok().json(Collection {
            id: c.id,
            name: c.name,
            created_at: c.created_at.to_rfc3339(),
        }),
        Err(_) => HttpResponse::InternalServerError().body("Failed to create collection"),
    }
}

/// Delete a collection
#[delete("/api/collections/{id}")]
async fn delete_collection(
    path: web::Path<i32>,
    data: web::Data<AppState>,
) -> impl Responder {
    let collection_id = path.into_inner();

    match CollectionEntity::delete_by_id(collection_id)
        .exec(&data.db_conn)
        .await
    {
        Ok(result) => {
            if result.rows_affected > 0 {
                HttpResponse::Ok().body("Collection deleted")
            } else {
                HttpResponse::NotFound().body("Collection not found")
            }
        }
        Err(_) => HttpResponse::InternalServerError().body("Database error"),
    }
}

/// Get songs in a collection
#[get("/api/collections/{id}/items")]
async fn get_collection_items(
    path: web::Path<i32>,
    data: web::Data<AppState>,
) -> impl Responder {
    let collection_id = path.into_inner();

    // First check if collection exists
    let collection_opt = CollectionEntity::find_by_id(collection_id)
        .one(&data.db_conn)
        .await;

    let collection = match collection_opt {
        Ok(Some(c)) => c,
        Ok(None) => return HttpResponse::NotFound().body("Collection not found"),
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    // Get all collection items for this collection
    match CollectionItemEntity::find()
        .filter(CollectionItemColumn::CollectionId.eq(collection_id))
        .find_also_related(MusicEntity)
        .all(&data.db_conn)
        .await
    {
        Ok(items) => {
            let songs: Vec<MusicInfo> = items
                .into_iter()
                .filter_map(|(_, music_opt)| music_opt)
                .map(|music| MusicInfo {
                    name: music.filename,
                    lufs: music.lufs.unwrap_or(0.5),
                    path: music.file_path,
                })
                .collect();

            HttpResponse::Ok().json(CollectionWithSongs {
                id: collection.id,
                name: collection.name,
                songs,
            })
        }
        Err(_) => HttpResponse::InternalServerError().body("Database error"),
    }
}

/// Add songs to a collection
#[post("/api/collections/{id}/items")]
async fn add_to_collection(
    path: web::Path<i32>,
    req: web::Json<AddToCollectionRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    let collection_id = path.into_inner();

    // Check if collection exists
    match CollectionEntity::find_by_id(collection_id)
        .one(&data.db_conn)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => return HttpResponse::NotFound().body("Collection not found"),
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    }

    // Add each music item to the collection
    for music_id in &req.music_ids {
        // Check if this music_id exists
        match MusicEntity::find_by_id(*music_id)
            .one(&data.db_conn)
            .await
        {
            Ok(Some(_)) => {
                // Check if already in collection
                match CollectionItemEntity::find()
                    .filter(CollectionItemColumn::CollectionId.eq(collection_id))
                    .filter(CollectionItemColumn::MusicId.eq(*music_id))
                    .one(&data.db_conn)
                    .await
                {
                    Ok(Some(_)) => {
                        // Already exists, skip
                        continue;
                    }
                    Ok(None) => {
                        // Add to collection
                        let item = CollectionItemActiveModel {
                            collection_id: Set(collection_id),
                            music_id: Set(*music_id),
                            created_at: Set(Utc::now()),
                            ..Default::default()
                        };
                        let _ = item.insert(&data.db_conn).await;
                    }
                    Err(_) => continue,
                }
            }
            Ok(None) => continue,
            Err(_) => continue,
        }
    }

    HttpResponse::Ok().body("Songs added to collection")
}

/// Remove songs from a collection
#[delete("/api/collections/{id}/items")]
async fn remove_from_collection(
    path: web::Path<i32>,
    req: web::Json<RemoveFromCollectionRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    let collection_id = path.into_inner();

    for music_id in &req.music_ids {
        match CollectionItemEntity::delete_many()
            .filter(CollectionItemColumn::CollectionId.eq(collection_id))
            .filter(CollectionItemColumn::MusicId.eq(*music_id))
            .exec(&data.db_conn)
            .await
        {
            Ok(_) => {}
            Err(_) => continue,
        }
    }

    HttpResponse::Ok().body("Songs removed from collection")
}

/// Get playlists in collection mode (returns collections instead of folders)
#[get("/api/playlists/collection-mode")]
async fn get_playlists_collection_mode(data: web::Data<AppState>) -> impl Responder {
    let mut playlists: std::collections::HashMap<String, Vec<MusicInfo>> = std::collections::HashMap::new();

    // Add "All Music" playlist with all songs
    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            for music in &music_list {
                let lufs_value = music.lufs.unwrap_or(0.5);
                let info = MusicInfo {
                    name: music.filename.clone(),
                    lufs: lufs_value,
                    path: music.file_path.clone(),
                };
                playlists.entry("所有音乐".to_string()).or_insert_with(Vec::new).push(info);
            }

            // Get all collections and their songs
            match CollectionEntity::find().all(&data.db_conn).await {
                Ok(collections) => {
                    for collection in collections {
                        match CollectionItemEntity::find()
                            .filter(CollectionItemColumn::CollectionId.eq(collection.id))
                            .find_also_related(MusicEntity)
                            .all(&data.db_conn)
                            .await
                        {
                            Ok(items) => {
                                let songs: Vec<MusicInfo> = items
                                    .into_iter()
                                    .filter_map(|(_, music_opt)| music_opt)
                                    .map(|music| MusicInfo {
                                        name: music.filename,
                                        lufs: music.lufs.unwrap_or(0.5),
                                        path: music.file_path,
                                    })
                                    .collect();
                                playlists.insert(collection.name, songs);
                            }
                            Err(_) => continue,
                        }
                    }
                }
                Err(_) => {}
            }
        }
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    }

    HttpResponse::Ok().json(playlists)
}

/// Supported audio file extensions
const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "ogg", "wav", "aac", "flac"];

/// Recursively scan directory for audio files
fn scan_directory_recursive(dir_path: &Path, music_path: &str) -> Vec<std::path::PathBuf> {
    let mut audio_files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if let Some(extension) = path.extension() {
                        let ext_str = extension.to_string_lossy().to_lowercase();
                        if SUPPORTED_EXTENSIONS.contains(&ext_str.as_str()) {
                            audio_files.push(path);
                        }
                    }
                } else if file_type.is_dir() {
                    // Recursively scan subdirectories
                    let mut sub_files = scan_directory_recursive(&entry.path(), music_path);
                    audio_files.append(&mut sub_files);
                }
            }
        }
    }

    audio_files
}

/// Initialize database with music files (only insert if path not exists)
async fn initialize_database(music_path: &str, db_conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let audio_files = scan_directory_recursive(Path::new(music_path), music_path);

    for file_path in audio_files {
        let filename = file_path.file_name().unwrap().to_string_lossy().to_string();
        let relative_path = file_path.strip_prefix(music_path)
            .unwrap_or(&file_path)
            .to_string_lossy()
            .to_string();

        // Check if this file_path already exists in database
        match MusicEntity::find()
            .filter(MusicColumn::FilePath.eq(&relative_path))
            .one(db_conn)
            .await
        {
            Ok(None) => {
                // Insert new record with default LUFS value
                let music = MusicActiveModel {
                    filename: Set(filename),
                    file_path: Set(relative_path),
                    lufs: Set(Some(0.5)), // Default LUFS value
                    created_at: Set(Utc::now()),
                    ..Default::default()
                };
                let _ = music.insert(db_conn).await;
            }
            Ok(Some(_)) => {
                // File already exists, skip
            }
            Err(e) => {
                eprintln!("Database error while checking file {}: {}", relative_path, e);
            }
        }
    }

    Ok(())
}

/// Update database: scan for new files, calculate LUFS, and insert
async fn update_database(music_path: &str, db_conn: &DatabaseConnection) -> Result<(), std::io::Error> {
    println!("Scanning for new files in: {}", music_path);

    let audio_files = scan_directory_recursive(Path::new(music_path), music_path);
    let mut new_files = 0;
    let mut updated_files = 0;

    for file_path in &audio_files {
        let filename = file_path.file_name().unwrap().to_string_lossy().to_string();
        let relative_path = file_path.strip_prefix(music_path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();
        let full_path = file_path.to_string_lossy().to_string();

        // Check if this file_path already exists in database
        match MusicEntity::find()
            .filter(MusicColumn::FilePath.eq(&relative_path))
            .one(db_conn)
            .await
        {
            Ok(None) => {
                // New file - calculate LUFS and insert
                println!("Found new file: {}", filename);
                if let Some(lufs_value) = get_lufs(&full_path) {
                    let music = MusicActiveModel {
                        filename: Set(filename.clone()),
                        file_path: Set(relative_path),
                        lufs: Set(Some(lufs_value)),
                        created_at: Set(Utc::now()),
                        ..Default::default()
                    };
                    match music.insert(db_conn).await {
                        Ok(_) => {
                            println!("  Inserted: {} (LUFS: {})", filename, lufs_value);
                            new_files += 1;
                        }
                        Err(e) => {
                            eprintln!("  Failed to insert {}: {}", filename, e);
                        }
                    }
                } else {
                    eprintln!("  Failed to calculate LUFS for {}", filename);
                }
            }
            Ok(Some(existing_music)) => {
                // File exists - update LUFS if missing or different
                if existing_music.lufs.is_none() || existing_music.lufs == Some(0.5) {
                    println!("Updating LUFS for: {}", filename);
                    if let Some(lufs_value) = get_lufs(&full_path) {
                        let mut active_model: MusicActiveModel = existing_music.clone().into();
                        active_model.lufs = Set(Some(lufs_value));
                        match active_model.update(db_conn).await {
                            Ok(_) => {
                                println!("  Updated: {} (LUFS: {})", filename, lufs_value);
                                updated_files += 1;
                            }
                            Err(e) => {
                                eprintln!("  Failed to update {}: {}", filename, e);
                            }
                        }
                    } else {
                        eprintln!("  Failed to calculate LUFS for {}", filename);
                    }
                }
            }
            Err(e) => {
                eprintln!("Database error while checking file {}: {}", relative_path, e);
            }
        }
    }

    // Delete database entries for files that no longer exist on disk
    println!("Checking for deleted files...");
    let mut deleted_files = 0;
    match MusicEntity::find().all(db_conn).await {
        Ok(all_music) => {
            for music in all_music {
                let full_path = Path::new(music_path).join(&music.file_path);
                if !full_path.exists() {
                    let filename = music.filename.clone();
                    println!("Deleting non-existent file from database: {}", filename);
                    match music.delete(db_conn).await {
                        Ok(_) => {
                            deleted_files += 1;
                        }
                        Err(e) => {
                            eprintln!("  Failed to delete {}: {}", filename, e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Database error while checking for deleted files: {}", e);
        }
    }

    println!("Update complete: {} new files, {} updated files, {} deleted files", new_files, updated_files, deleted_files);
    Ok(())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <run|update> [music_path]", args[0]);
        eprintln!("  run   - Start the web server");
        eprintln!("  update - Scan for new music files and update database");
        std::process::exit(1);
    }

    let command = &args[1];
    let music_path = if args.len() > 2 {
        args[2].clone()
    } else {
        std::env::var("HOME")
            .map(|h| format!("{}/Music", h))
            .unwrap_or_else(|_| String::from("./music"))
    };

    println!("Connecting to database...");
    let db_conn = match establish_connection(&music_path).await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Failed to connect to database: {}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        }
    };

    match command.as_str() {
        "run" => {
            println!("Scanning music files from: {}", music_path);

            // Initialize database with music files (only insert if path not exists)
            if let Err(e) = initialize_database(&music_path, &db_conn).await {
                eprintln!("Failed to initialize database: {}", e);
            }

            // Get count of music files in database
            match MusicEntity::find().all(&db_conn).await {
                Ok(music_list) => {
                    println!("Found {} music files in database", music_list.len());
                }
                Err(e) => {
                    eprintln!("Failed to count music files: {}", e);
                }
            }

            let app_state = web::Data::new(AppState {
                music_path: music_path.clone(),
                db_conn,
            });

            HttpServer::new(move || {
                let cors = Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600);

                App::new()
                    .wrap(cors)
                    .app_data(app_state.clone())
                    .service(get_music)
                    .service(get_all_music)
                    .service(get_all_playlists)
                    .service(get_playlist)
                    .service(get_all_collections)
                    .service(create_collection)
                    .service(delete_collection)
                    .service(get_collection_items)
                    .service(add_to_collection)
                    .service(remove_from_collection)
                    .service(get_playlists_collection_mode)
            })
            .bind(("0.0.0.0", 2080))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
            .run()
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
        }
        "update" => {
            update_database(&music_path, &db_conn).await?;
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            eprintln!("Usage: {} <run|update> [music_path]", args[0]);
            std::process::exit(1);
        }
    }

    Ok(())
}
