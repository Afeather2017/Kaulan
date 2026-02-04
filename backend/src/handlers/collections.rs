//! Collection API handlers for user-defined music collections.
//!
//! This module provides endpoints for:
//! - Getting all collections
//! - Creating a new collection
//! - Deleting a collection
//! - Getting a single collection by ID
//! - Getting songs in a collection
//! - Adding songs to a collection
//! - Removing songs from a collection
//!
//! Collections are user-defined playlists that can contain any music files,
//! regardless of their folder location.

use actix_web::{get, post, delete, web, HttpResponse, Responder};
use crate::entities::music::{Entity as MusicEntity};
use crate::entities::collection::{Entity as CollectionEntity, Model as CollectionModel, ActiveModel as CollectionActiveModel, Column as CollectionColumn};
use crate::entities::collection_item::{Entity as CollectionItemEntity, ActiveModel as CollectionItemActiveModel, Column as CollectionItemColumn};
use crate::types::{AppState, Collection, CollectionWithSongs, CreateCollectionRequest, AddToCollectionRequest, RemoveFromCollectionRequest, MusicInfo};
use sea_orm::{EntityTrait, ActiveModelTrait, Set, ColumnTrait, QueryFilter};
use chrono::Utc;
use tracing::{info, warn, error, debug};

/// Get all collections
///
/// Returns a list of all user-defined collections without their songs.
/// Use `/api/collections/{id}/items` to get the songs in a specific collection.
///
/// # Returns
/// JSON array of `Collection` objects
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
///
/// Creates a new user-defined collection with the specified name.
/// The collection name must be unique.
///
/// # Request Body
/// ```json
/// {
///   "name": "My Favorites"
/// }
/// ```
///
/// # Returns
/// - `200 OK` with the created `Collection` object
/// - `400 Bad Request` if a collection with this name already exists
/// - `500 Internal Server Error` for database errors
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
///
/// Deletes a collection and all associated collection items (junction table entries).
///
/// # Path Parameters
/// * `id` - The collection ID to delete
///
/// # Returns
/// - `200 OK` if deleted successfully
/// - `404 Not Found` if collection doesn't exist
/// - `500 Internal Server Error` for database errors
#[delete("/api/collections/{id}")]
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
///
/// Returns the metadata for a specific collection without its songs.
/// Use `/api/collections/{id}/items` to get the songs.
///
/// # Path Parameters
/// * `id` - The collection ID
///
/// # Returns
/// - `200 OK` with `Collection` object
/// - `404 Not Found` if collection doesn't exist
/// - `500 Internal Server Error` for database errors
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
///
/// Returns a collection with all its songs. This is the primary endpoint
/// for getting the full collection data.
///
/// **IMPORTANT:** This route must be registered before `/api/collections/{id}`
/// in the server configuration, otherwise it will match the wrong route.
///
/// # Path Parameters
/// * `id` - The collection ID
///
/// # Returns
/// - `200 OK` with `CollectionWithSongs` object
/// - `404 Not Found` if collection doesn't exist
/// - `500 Internal Server Error` for database errors
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
///
/// Adds the specified music IDs to a collection. Duplicate entries are ignored.
///
/// # Path Parameters
/// * `id` - The collection ID
///
/// # Request Body
/// ```json
/// {
///   "music_ids": [1, 2, 3]
/// }
/// ```
///
/// # Returns
/// - `200 OK` with success message
/// - `404 Not Found` if collection doesn't exist
/// - `500 Internal Server Error` for database errors
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
///
/// Removes the specified music IDs from a collection.
///
/// # Path Parameters
/// * `id` - The collection ID
///
/// # Request Body
/// ```json
/// {
///   "music_ids": [1, 2, 3]
/// }
/// ```
///
/// # Returns
/// - `200 OK` with success message
/// - `500 Internal Server Error` for database errors
#[delete("/api/collections/{id}/items")]
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
