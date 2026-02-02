use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use actix_cors::Cors;
use actix_multipart::Multipart;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::path::Path;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::RwLock;
use sea_orm::{DatabaseConnection, EntityTrait, ActiveModelTrait, Set, ColumnTrait, QueryFilter, ModelTrait};
use chrono::Utc;
use tracing::{debug, info, warn, error};

// Declare modules
pub mod lufsgen;
pub mod entities;
pub mod database;

use lufsgen::get_lufs;
use entities::music::{Entity as MusicEntity, Model as MusicModel, ActiveModel as MusicActiveModel, Column as MusicColumn};
use entities::collection::{Entity as CollectionEntity, Model as CollectionModel, ActiveModel as CollectionActiveModel, Column as CollectionColumn};
use entities::collection_item::{Entity as CollectionItemEntity, ActiveModel as CollectionItemActiveModel, Column as CollectionItemColumn};
pub use database::establish_connection;

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

pub struct AppState {
    pub music_path: Arc<RwLock<String>>,
    pub db_conn: DatabaseConnection,
}

#[get("/api/music/{filename}")]
pub async fn get_music(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> impl Responder {
    let filename = path.into_inner();
    debug!("Music request received for filename: {}", filename);

    match MusicEntity::find()
        .filter(MusicColumn::Filename.eq(&filename))
        .one(&data.db_conn)
        .await
    {
        Ok(Some(music)) => {
            let music_path = data.music_path.read().await;
            let file_path = Path::new(&*music_path).join(&music.file_path);
            drop(music_path);

            match fs::read(&file_path) {
                Ok(content) => {
                    debug!("Successfully served music file: {}", filename);
                    let mut response = HttpResponse::Ok();
                    response.insert_header(("Content-Type", "audio/mpeg"));
                    response.insert_header(("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0"));
                    response.insert_header(("Pragma", "no-cache"));
                    response.insert_header(("Expires", "0"));
                    response.body(content)
                }
                Err(_) => {
                    warn!("File not found on disk: {}", file_path.display());
                    HttpResponse::NotFound().body("File not found")
                }
            }
        }
        Ok(None) => {
            warn!("Music not found in database: {}", filename);
            HttpResponse::NotFound().body("Music not found")
        }
        Err(e) => {
            error!("Database error while fetching music {}: {}", filename, e);
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}

#[get("/api/music")]
pub async fn get_all_music(data: web::Data<AppState>) -> impl Responder {
    debug!("Get all music request received");
    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            info!("Returning {} music entries", music_list.len());
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
        Err(e) => {
            error!("Database error while fetching all music: {}", e);
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}

#[get("/api/playlists")]
pub async fn get_all_playlists(data: web::Data<AppState>) -> impl Responder {
    let mut playlists: std::collections::HashMap<String, Vec<MusicInfo>> = std::collections::HashMap::new();

    match MusicEntity::find().all(&data.db_conn).await {
        Ok(music_list) => {
            for music in &music_list {
                let lufs_value = music.lufs.unwrap_or(0.5);
                let info = MusicInfo {
                    name: music.filename.clone(),
                    lufs: lufs_value,
                    path: music.file_path.clone(),
                };

                playlists.entry("所有音乐".to_string()).or_insert_with(Vec::new).push(info.clone());

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
pub async fn get_playlist(
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

                let belongs_to_playlist = if playlist_name == "所有音乐" {
                    true
                } else {
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
pub async fn get_all_collections(data: web::Data<AppState>) -> impl Responder {
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
pub async fn create_collection(
    req: web::Json<CreateCollectionRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    info!("Creating collection with name: {}", req.name);
    match CollectionEntity::find()
        .filter(CollectionColumn::Name.eq(&req.name))
        .one(&data.db_conn)
        .await
    {
        Ok(Some(_)) => {
            warn!("Collection with name '{}' already exists", req.name);
            return HttpResponse::BadRequest().body("Collection with this name already exists");
        }
        Ok(None) => {}
        Err(e) => {
            error!("Database error while checking collection existence: {}", e);
            return HttpResponse::InternalServerError().body("Database error");
        }
    }

    let collection = CollectionActiveModel {
        name: Set(req.name.clone()),
        created_at: Set(Utc::now()),
        ..Default::default()
    };

    match collection.insert(&data.db_conn).await {
        Ok(c) => {
            info!("Collection created successfully with ID: {}", c.id);
            HttpResponse::Ok().json(Collection {
                id: c.id,
                name: c.name,
                created_at: c.created_at.to_rfc3339(),
            })
        }
        Err(e) => {
            error!("Failed to create collection: {}", e);
            HttpResponse::InternalServerError().body("Failed to create collection")
        }
    }
}

/// Delete a collection
#[actix_web::delete("/api/collections/{id}")]
pub async fn delete_collection(
    path: web::Path<i32>,
    data: web::Data<AppState>,
) -> impl Responder {
    let collection_id = path.into_inner();
    info!("Deleting collection with ID: {}", collection_id);

    // First, delete all collection items for this collection
    let _ = CollectionItemEntity::delete_many()
        .filter(CollectionItemColumn::CollectionId.eq(collection_id))
        .exec(&data.db_conn)
        .await;

    // Then delete the collection itself
    match CollectionEntity::delete_by_id(collection_id)
        .exec(&data.db_conn)
        .await
    {
        Ok(result) => {
            if result.rows_affected > 0 {
                info!("Collection {} deleted successfully", collection_id);
                HttpResponse::Ok().body("Collection deleted")
            } else {
                warn!("Collection {} not found", collection_id);
                HttpResponse::NotFound().body("Collection not found")
            }
        }
        Err(e) => {
            error!("Database error while deleting collection {}: {}", collection_id, e);
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}

/// Get a single collection by ID (without songs)
#[get("/api/collections/{id}")]
pub async fn get_collection(
    path: web::Path<i32>,
    data: web::Data<AppState>,
) -> impl Responder {
    let collection_id = path.into_inner();

    match CollectionEntity::find_by_id(collection_id)
        .one(&data.db_conn)
        .await
    {
        Ok(Some(collection)) => HttpResponse::Ok().json(Collection {
            id: collection.id,
            name: collection.name,
            created_at: collection.created_at.to_rfc3339(),
        }),
        Ok(None) => HttpResponse::NotFound().body("Collection not found"),
        Err(_) => HttpResponse::InternalServerError().body("Database error"),
    }
}

/// Get songs in a collection
#[get("/api/collections/{id}/items")]
pub async fn get_collection_items(
    path: web::Path<i32>,
    data: web::Data<AppState>,
) -> impl Responder {
    let collection_id = path.into_inner();

    let collection_opt = CollectionEntity::find_by_id(collection_id)
        .one(&data.db_conn)
        .await;

    let collection = match collection_opt {
        Ok(Some(c)) => c,
        Ok(None) => return HttpResponse::NotFound().body("Collection not found"),
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

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
pub async fn add_to_collection(
    path: web::Path<i32>,
    req: web::Json<AddToCollectionRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    let collection_id = path.into_inner();
    info!("Adding {} songs to collection {}", req.music_ids.len(), collection_id);

    match CollectionEntity::find_by_id(collection_id)
        .one(&data.db_conn)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            warn!("Collection {} not found", collection_id);
            return HttpResponse::NotFound().body("Collection not found");
        }
        Err(e) => {
            error!("Database error while fetching collection {}: {}", collection_id, e);
            return HttpResponse::InternalServerError().body("Database error");
        }
    }

    let mut added_count = 0;
    for music_id in &req.music_ids {
        match MusicEntity::find_by_id(*music_id)
            .one(&data.db_conn)
            .await
        {
            Ok(Some(_)) => {
                match CollectionItemEntity::find()
                    .filter(CollectionItemColumn::CollectionId.eq(collection_id))
                    .filter(CollectionItemColumn::MusicId.eq(*music_id))
                    .one(&data.db_conn)
                    .await
                {
                    Ok(Some(_)) => {
                        debug!("Music {} already in collection {}", music_id, collection_id);
                        continue;
                    }
                    Ok(None) => {
                        let item = CollectionItemActiveModel {
                            collection_id: Set(collection_id),
                            music_id: Set(*music_id),
                            created_at: Set(Utc::now()),
                            ..Default::default()
                        };
                        match item.insert(&data.db_conn).await {
                            Ok(_) => added_count += 1,
                            Err(e) => warn!("Failed to add music {} to collection {}: {}", music_id, collection_id, e),
                        }
                    }
                    Err(e) => {
                        warn!("Database error while checking collection item: {}", e);
                        continue;
                    }
                }
            }
            Ok(None) => {
                debug!("Music {} not found", music_id);
                continue;
            }
            Err(e) => {
                warn!("Database error while fetching music {}: {}", music_id, e);
                continue;
            }
        }
    }

    info!("Added {} songs to collection {}", added_count, collection_id);
    HttpResponse::Ok().body("Songs added to collection")
}

/// Remove songs from a collection
#[actix_web::delete("/api/collections/{id}/items")]
pub async fn remove_from_collection(
    path: web::Path<i32>,
    req: web::Json<RemoveFromCollectionRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    let collection_id = path.into_inner();
    info!("Removing {} songs from collection {}", req.music_ids.len(), collection_id);

    let mut removed_count = 0;
    for music_id in &req.music_ids {
        match CollectionItemEntity::delete_many()
            .filter(CollectionItemColumn::CollectionId.eq(collection_id))
            .filter(CollectionItemColumn::MusicId.eq(*music_id))
            .exec(&data.db_conn)
            .await
        {
            Ok(result) => {
                if result.rows_affected > 0 {
                    removed_count += 1;
                }
            }
            Err(e) => {
                warn!("Failed to remove music {} from collection {}: {}", music_id, collection_id, e);
                continue;
            }
        }
    }

    info!("Removed {} songs from collection {}", removed_count, collection_id);
    HttpResponse::Ok().body("Songs removed from collection")
}

/// Get current music directory
#[get("/api/settings/music-directory")]
pub async fn get_music_directory(data: web::Data<AppState>) -> impl Responder {
    #[derive(Serialize)]
    struct MusicDirectoryResponse {
        path: String,
    }

    let music_path = data.music_path.read().await;
    HttpResponse::Ok().json(MusicDirectoryResponse {
        path: music_path.clone(),
    })
}

#[derive(Deserialize)]
struct SetMusicDirectoryRequest {
    path: String,
}

/// Set music directory
#[post("/api/settings/music-directory")]
pub async fn set_music_directory(
    req: web::Json<SetMusicDirectoryRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    #[derive(Serialize)]
    struct SetDirectoryResponse {
        success: bool,
        message: String,
    }

    let new_path = &req.path;

    // Validate the path exists and is a directory
    let path_obj = Path::new(new_path);
    if !path_obj.exists() {
        warn!("Music directory does not exist: {}", new_path);
        return HttpResponse::BadRequest().json(SetDirectoryResponse {
            success: false,
            message: format!("Directory does not exist: {}", new_path),
        });
    }

    if !path_obj.is_dir() {
        warn!("Path is not a directory: {}", new_path);
        return HttpResponse::BadRequest().json(SetDirectoryResponse {
            success: false,
            message: format!("Path is not a directory: {}", new_path),
        });
    }

    // Update the music path
    let mut music_path = data.music_path.write().await;
    *music_path = new_path.clone();
    drop(music_path);

    info!("Music directory updated to: {}", new_path);

    // Re-initialize the database with the new path
    match initialize_database(new_path, &data.db_conn).await {
        Ok(_) => {
            info!("Database re-initialized with new music directory");
            HttpResponse::Ok().json(SetDirectoryResponse {
                success: true,
                message: format!("Music directory updated to: {}", new_path),
            })
        }
        Err(e) => {
            error!("Failed to re-initialize database: {}", e);
            HttpResponse::InternalServerError().json(SetDirectoryResponse {
                success: false,
                message: format!("Failed to update database: {}", e),
            })
        }
    }
}

// ============= File Upload API Endpoints =============

/// Directory tree node for representing file system structure
#[derive(Serialize, Deserialize)]
struct DirectoryNode {
    name: String,
    path: String,
    #[serde(rename = "type")]
    node_type: String,
    children: Option<Vec<DirectoryNode>>,
}

/// Get directory tree structure of the music directory
#[get("/api/files/directory-tree")]
pub async fn get_directory_tree(data: web::Data<AppState>) -> impl Responder {
    debug!("Directory tree request received");

    let lock = data.music_path.read().await;
    let music_path_str = lock.clone();
    drop(lock);

    fn build_tree(dir_path: &Path, base_path: &Path) -> Option<DirectoryNode> {
        let name = dir_path.file_name()?.to_string_lossy().to_string();
        let relative_path = dir_path.strip_prefix(base_path)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| String::new());

        let mut children = Vec::new();

        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    let path = entry.path();
                    if file_type.is_dir() {
                        if let Some(child_node) = build_tree(&path, base_path) {
                            children.push(child_node);
                        }
                    }
                }
            }
        }

        children.sort_by(|a, b| a.name.cmp(&b.name));

        Some(DirectoryNode {
            name,
            path: relative_path,
            node_type: "directory".to_string(),
            children: if children.is_empty() { None } else { Some(children) },
        })
    }

    let music_path = Path::new(&music_path_str);
    match build_tree(music_path, music_path) {
        Some(root_node) => {
            debug!("Directory tree generated successfully");
            HttpResponse::Ok().json(root_node)
        }
        None => {
            warn!("Failed to generate directory tree");
            HttpResponse::InternalServerError().body("Failed to generate directory tree")
        }
    }
}

/// Response for file upload operation
#[derive(Serialize, Deserialize)]
struct UploadResponse {
    success: bool,
    message: String,
    uploaded: Vec<String>,
    failed: Vec<String>,
}

/// Upload files to a specific directory within the music directory
#[post("/api/files/upload")]
pub async fn upload_files(
    mut payload: Multipart,
    data: web::Data<AppState>,
) -> impl Responder {
    debug!("File upload request received");
    let lock = data.music_path.read().await;
    let music_path_str = lock.clone();
    drop(lock);

    let mut target_path = String::new();
    let mut uploaded_files = Vec::new();
    let mut failed_files = Vec::new();

    // Process the multipart form data
    let mut field_result = payload.try_next().await;
    while field_result.is_ok() {
        if let Some(mut field) = field_result.unwrap() {
            let content_disposition = field.content_disposition();
            let field_name = content_disposition
                .and_then(|cd| cd.get_name())
                .unwrap_or("")
                .to_string();

            match field_name.as_str() {
                "targetPath" => {
                    // Read the target path
                    let mut path_bytes = Vec::new();
                    let mut chunk_result = field.try_next().await;
                    while chunk_result.is_ok() {
                        if let Some(chunk) = chunk_result.unwrap() {
                            path_bytes.extend_from_slice(&chunk);
                            chunk_result = field.try_next().await;
                        } else {
                            break;
                        }
                    }
                    target_path = String::from_utf8_lossy(&path_bytes).to_string();
                    debug!("Target path: {}", target_path);

                    // Security check: ensure target path is valid and within music directory
                    let target_dir = Path::new(&music_path_str).join(&target_path);
                    if !target_dir.starts_with(&music_path_str) {
                        warn!("Invalid target path: {} (not within music directory)", target_path);
                        return HttpResponse::BadRequest().json(UploadResponse {
                            success: false,
                            message: "Invalid target path".to_string(),
                            uploaded: vec![],
                            failed: vec![],
                        });
                    }

                    // Create target directory if it doesn't exist
                    if !target_dir.exists() {
                        if let Err(e) = fs::create_dir_all(&target_dir) {
                            error!("Failed to create target directory {}: {}", target_dir.display(), e);
                            return HttpResponse::InternalServerError().json(UploadResponse {
                                success: false,
                                message: format!("Failed to create target directory: {}", e),
                                uploaded: vec![],
                                failed: vec![],
                            });
                        }
                    }
                }
                "files" => {
                    let filename = content_disposition
                        .and_then(|cd| cd.get_filename())
                        .unwrap_or("unknown")
                        .to_string();

                    // Validate file extension
                    if let Some(extension) = Path::new(&filename).extension() {
                        let ext_str = extension.to_string_lossy().to_lowercase();
                        if !SUPPORTED_EXTENSIONS.contains(&ext_str.as_str()) {
                            warn!("Unsupported file type: {}", filename);
                            failed_files.push(filename.clone());
                            field_result = payload.try_next().await;
                            continue;
                        }
                    } else {
                        warn!("File without extension: {}", filename);
                        failed_files.push(filename.clone());
                        field_result = payload.try_next().await;
                        continue;
                    }

                    // Determine the full file path
                    let full_target_path = if target_path.is_empty() {
                        Path::new(&music_path_str).join(&filename)
                    } else {
                        Path::new(&music_path_str).join(&target_path).join(&filename)
                    };

                    // Security check: ensure file path is within music directory
                    if !full_target_path.starts_with(&music_path_str) {
                        warn!("Invalid file path: {} (not within music directory)", full_target_path.display());
                        failed_files.push(filename.clone());
                        field_result = payload.try_next().await;
                        continue;
                    }

                    // Write the file
                    match File::create(&full_target_path) {
                        Ok(mut file) => {
                            let mut file_size = 0u64;
                            let mut write_error = false;

                            let mut chunk_result = field.try_next().await;
                            while chunk_result.is_ok() {
                                if let Some(chunk) = chunk_result.unwrap() {
                                    file_size += chunk.len() as u64;
                                    if file.write_all(&chunk).is_err() {
                                        write_error = true;
                                        break;
                                    }
                                    chunk_result = field.try_next().await;
                                } else {
                                    break;
                                }
                            }

                            if write_error {
                                error!("Failed to write file: {}", filename);
                                let _ = fs::remove_file(&full_target_path);
                                failed_files.push(filename.clone());
                            } else {
                                info!("Successfully uploaded file: {} ({} bytes)", filename, file_size);
                                uploaded_files.push(filename);
                            }
                        }
                        Err(e) => {
                            error!("Failed to create file {}: {}", filename, e);
                            failed_files.push(filename.clone());
                        }
                    }
                }
                _ => {
                    debug!("Ignoring unknown field: {}", field_name);
                }
            }
        }
        field_result = payload.try_next().await;
    }

    if uploaded_files.is_empty() && failed_files.is_empty() {
        return HttpResponse::BadRequest().json(UploadResponse {
            success: false,
            message: "No files provided".to_string(),
            uploaded: vec![],
            failed: vec![],
        });
    }

    // Trigger database update after successful upload
    if !uploaded_files.is_empty() {
        info!("Triggering database update after file upload");
        match update_database(&music_path_str, &data.db_conn).await {
            Ok(_) => {
                info!("Database update completed after upload");
            }
            Err(e) => {
                warn!("Database update failed after upload: {}", e);
            }
        }
    }

    let success = !uploaded_files.is_empty();
    let message = if success {
        format!("Uploaded {} file(s)", uploaded_files.len())
    } else {
        "Upload failed".to_string()
    };

    HttpResponse::Ok().json(UploadResponse {
        success,
        message,
        uploaded: uploaded_files,
        failed: failed_files,
    })
}

/// Update database (scan for new files, update LUFS, remove deleted files)
#[post("/api/database/update")]
pub async fn update_database_endpoint(data: web::Data<AppState>) -> impl Responder {
    info!("Database update requested via API");

    #[derive(Serialize)]
    struct UpdateResponse {
        success: bool,
        message: String,
    }

    let lock = data.music_path.read().await;
    let music_path_str = lock.clone();
    drop(lock);

    match update_database(&music_path_str, &data.db_conn).await {
        Ok(_) => {
            info!("Database update completed successfully");
            HttpResponse::Ok().json(UpdateResponse {
                success: true,
                message: "Database updated successfully".to_string(),
            })
        }
        Err(e) => {
            error!("Database update failed: {}", e);
            HttpResponse::InternalServerError().json(UpdateResponse {
                success: false,
                message: format!("Database update failed: {}", e),
            })
        }
    }
}

/// Get playlists in collection mode (returns collections instead of folders)
#[get("/api/playlists/collection-mode")]
pub async fn get_playlists_collection_mode(data: web::Data<AppState>) -> impl Responder {
    let mut playlists: std::collections::HashMap<String, Vec<MusicInfo>> = std::collections::HashMap::new();

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
fn scan_directory_recursive(dir_path: &Path, _music_path: &str) -> Vec<std::path::PathBuf> {
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
                    let mut sub_files = scan_directory_recursive(&entry.path(), _music_path);
                    audio_files.append(&mut sub_files);
                }
            }
        }
    }

    audio_files
}

/// Initialize database with music files (only insert if path not exists)
pub async fn initialize_database(music_path: &str, db_conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    info!("Initializing database with music from: {}", music_path);
    let audio_files = scan_directory_recursive(Path::new(music_path), music_path);
    info!("Found {} audio files in directory", audio_files.len());

    let mut new_files = 0;
    for file_path in audio_files {
        let filename = file_path.file_name().unwrap().to_string_lossy().to_string();
        let relative_path = file_path.strip_prefix(music_path)
            .unwrap_or(&file_path)
            .to_string_lossy()
            .to_string();

        match MusicEntity::find()
            .filter(MusicColumn::FilePath.eq(&relative_path))
            .one(db_conn)
            .await
        {
            Ok(None) => {
                debug!("Inserting new file into database: {}", relative_path);
                let music = MusicActiveModel {
                    filename: Set(filename),
                    file_path: Set(relative_path.clone()),
                    lufs: Set(Some(0.5)),
                    created_at: Set(Utc::now()),
                    ..Default::default()
                };
                match music.insert(db_conn).await {
                    Ok(_) => new_files += 1,
                    Err(e) => error!("Failed to insert music {}: {}", relative_path, e),
                }
            }
            Ok(Some(_)) => {
                debug!("File already exists in database: {}", relative_path);
            }
            Err(e) => {
                error!("Database error while checking file {}: {}", relative_path, e);
            }
        }
    }

    info!("Database initialization complete: {} new files added", new_files);
    Ok(())
}

/// Update database: scan for new files, calculate LUFS, and insert
pub async fn update_database(music_path: &str, db_conn: &DatabaseConnection) -> Result<(), std::io::Error> {
    info!("Scanning for new files in: {}", music_path);

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

        match MusicEntity::find()
            .filter(MusicColumn::FilePath.eq(&relative_path))
            .one(db_conn)
            .await
        {
            Ok(None) => {
                info!("Found new file: {}", filename);
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
                            info!("Inserted: {} (LUFS: {})", filename, lufs_value);
                            new_files += 1;
                        }
                        Err(e) => {
                            error!("Failed to insert {}: {}", filename, e);
                        }
                    }
                } else {
                    warn!("Failed to calculate LUFS for {}", filename);
                }
            }
            Ok(Some(existing_music)) => {
                if existing_music.lufs.is_none() || existing_music.lufs == Some(0.5) {
                    info!("Updating LUFS for: {}", filename);
                    if let Some(lufs_value) = get_lufs(&full_path) {
                        let mut active_model: MusicActiveModel = existing_music.clone().into();
                        active_model.lufs = Set(Some(lufs_value));
                        match active_model.update(db_conn).await {
                            Ok(_) => {
                                info!("Updated: {} (LUFS: {})", filename, lufs_value);
                                updated_files += 1;
                            }
                            Err(e) => {
                                error!("Failed to update {}: {}", filename, e);
                            }
                        }
                    } else {
                        warn!("Failed to calculate LUFS for {}", filename);
                    }
                }
            }
            Err(e) => {
                error!("Database error while checking file {}: {}", relative_path, e);
            }
        }
    }

    info!("Checking for deleted files...");
    let mut deleted_files = 0;
    match MusicEntity::find().all(db_conn).await {
        Ok(all_music) => {
            for music in all_music {
                let full_path = Path::new(music_path).join(&music.file_path);
                if !full_path.exists() {
                    let filename = music.filename.clone();
                    info!("Deleting non-existent file from database: {}", filename);
                    match music.delete(db_conn).await {
                        Ok(_) => {
                            deleted_files += 1;
                        }
                        Err(e) => {
                            error!("Failed to delete {}: {}", filename, e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            error!("Database error while checking for deleted files: {}", e);
        }
    }

    info!("Update complete: {} new files, {} updated files, {} deleted files", new_files, updated_files, deleted_files);
    Ok(())
}

/// Represents the server address information
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub ip: String,
    pub port: u16,
}

impl ServerInfo {
    /// Returns the full base URL for the server
    pub fn url(&self) -> String {
        format!("{}:{}", self.ip, self.port)
    }
}

/// Starts the backend HTTP server
///
/// The server is spawned in the background and this function returns immediately with ServerInfo.
/// For CLI use where you want to wait for the server, you would typically use a different approach
/// or keep the main thread alive.
///
/// # Arguments
/// * `music_path` - Path to the directory containing music files
///
/// # Returns
/// A `ServerInfo` containing the IP address and port the server is running on
pub async fn start_server(music_path: String) -> Result<ServerInfo, Box<dyn std::error::Error>> {
    info!("Connecting to database...");
    let db_conn = match establish_connection(&music_path).await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return Err(Box::new(e));
        }
    };
    info!("Database connection established");

    info!("Scanning music files from: {}", music_path);

    if let Err(e) = initialize_database(&music_path, &db_conn).await {
        error!("Failed to initialize database: {}", e);
    }

    match MusicEntity::find().all(&db_conn).await {
        Ok(music_list) => {
            info!("Found {} music files in database", music_list.len());
        }
        Err(e) => {
            error!("Failed to count music files: {}", e);
        }
    }

    let app_state = web::Data::new(AppState {
        music_path: Arc::new(RwLock::new(music_path.clone())),
        db_conn,
    });

    let ip = "0.0.0.0".to_string();
    let port = 2080;
    let ip_clone = ip.clone();

    info!("Starting HTTP server on {}:{}", ip, port);

    // Spawn the server in the background using tokio (this works around Send issues)
    tokio::spawn(async move {
        match HttpServer::new(move || {
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
                .service(get_playlists_collection_mode) // Must be before get_playlist (route with parameter)
                .service(get_playlist)
                .service(get_all_collections)
                .service(create_collection)
                .service(delete_collection)
                .service(get_collection_items) // Must be before get_collection (longer path first)
                .service(get_collection)
                .service(add_to_collection)
                .service(remove_from_collection)
                .service(get_music_directory)
                .service(set_music_directory)
                .service(update_database_endpoint)
                .service(get_directory_tree)
                .service(upload_files)
        })
        .bind((ip_clone, port))
        .unwrap()
        .run()
        .await
        {
            Ok(_) => info!("Server shutdown gracefully"),
            Err(e) => error!("Server error: {}", e),
        }
    });

    // Give the server a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Ok(ServerInfo { ip, port })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;
    use actix_web::http::StatusCode;

    /// Helper function to create a temporary test directory structure
    fn create_test_directory() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        let music_dir = dir.path();

        // Create test directory structure
        let folder1 = music_dir.join("folder1");
        let folder2 = music_dir.join("folder2");
        let subfolder = folder2.join("subfolder");

        fs::create_dir_all(&folder1).unwrap();
        fs::create_dir_all(&subfolder).unwrap();

        dir
    }

    #[actix_web::test]
    async fn test_directory_tree_empty_directory() {
        let temp_dir = create_test_directory();
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(RwLock::new(temp_dir.path().to_str().unwrap().to_string())),
            db_conn: establish_connection(temp_dir.path().to_str().unwrap()).await.unwrap(),
        });

        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(get_directory_tree)
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/files/directory-tree")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: DirectoryNode = test::read_body_json(resp).await;
        assert_eq!(body.node_type, "directory");
        assert_eq!(body.path, "");
        // Root should have children (folder1 and folder2)
        assert!(body.children.is_some());
        assert_eq!(body.children.as_ref().unwrap().len(), 2);
    }

    #[actix_web::test]
    async fn test_directory_tree_nested_structure() {
        let temp_dir = create_test_directory();
        let music_path = temp_dir.path().to_str().unwrap().to_string();

        // Verify the nested structure is returned correctly
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(RwLock::new(music_path.clone())),
            db_conn: establish_connection(&music_path).await.unwrap(),
        });

        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(get_directory_tree)
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/files/directory-tree")
            .to_request();

        let resp = test::call_service(&app, req).await;
        let body: DirectoryNode = test::read_body_json(resp).await;

        // Check that folder2 has a subfolder
        let folder2 = body.children.as_ref()
            .unwrap()
            .iter()
            .find(|c| c.name == "folder2")
            .unwrap();
        assert!(folder2.children.is_some());
        assert_eq!(folder2.children.as_ref().unwrap().len(), 1);
        assert_eq!(folder2.children.as_ref().unwrap()[0].name, "subfolder");
    }

    #[actix_web::test]
    async fn test_upload_files_empty_request() {
        let temp_dir = create_test_directory();
        let music_path = temp_dir.path().to_str().unwrap().to_string();

        let app_state = web::Data::new(AppState {
            music_path: Arc::new(RwLock::new(music_path.clone())),
            db_conn: establish_connection(&music_path).await.unwrap(),
        });

        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(upload_files)
        ).await;

        // Test with no files - should return error
        let req = test::TestRequest::post()
            .uri("/api/files/upload")
            .to_request();

        let resp = test::call_service(&app, req).await;

        // Should get a response (either bad request or success with empty result)
        assert!(resp.status().is_client_error() || resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_upload_endpoint_service_exists() {
        let temp_dir = create_test_directory();
        let music_path = temp_dir.path().to_str().unwrap().to_string();

        let app_state = web::Data::new(AppState {
            music_path: Arc::new(RwLock::new(music_path.clone())),
            db_conn: establish_connection(&music_path).await.unwrap(),
        });

        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(upload_files)
        ).await;

        // Just verify the endpoint exists and responds
        let req = test::TestRequest::post()
            .uri("/api/files/upload")
            .to_request();

        let resp = test::call_service(&app, req).await;
        // Should get some response (not 404)
        assert_ne!(resp.status(), StatusCode::NOT_FOUND);
    }
}
