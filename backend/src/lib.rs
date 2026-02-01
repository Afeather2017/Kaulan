use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use sea_orm::{DatabaseConnection, EntityTrait, ActiveModelTrait, Set, ColumnTrait, QueryFilter};
use chrono::Utc;

mod lufsgen;
mod entities;
mod database;
use entities::music::{Entity as MusicEntity, Model as MusicModel, ActiveModel as MusicActiveModel, Column as MusicColumn};
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

const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "ogg", "wav", "aac", "flac"];

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
                    let mut sub_files = scan_directory_recursive(&entry.path(), music_path);
                    audio_files.append(&mut sub_files);
                }
            }
        }
    }

    audio_files
}

async fn initialize_database(music_path: &str, db_conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let audio_files = scan_directory_recursive(Path::new(music_path), music_path);

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
                let music = MusicActiveModel {
                    filename: Set(filename),
                    file_path: Set(relative_path),
                    lufs: Set(Some(0.5)),
                    created_at: Set(Utc::now()),
                    ..Default::default()
                };
                let _ = music.insert(db_conn).await;
            }
            Ok(Some(_)) => {}
            Err(e) => {
                eprintln!("Database error while checking file {}: {}", relative_path, e);
            }
        }
    }

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
/// # Arguments
/// * `music_path` - Path to the directory containing music files
///
/// # Returns
/// A `ServerInfo` containing the IP address and port the server is running on
pub async fn start_server(music_path: String) -> Result<ServerInfo, Box<dyn std::error::Error>> {
    println!("Connecting to database...");
    let db_conn = match establish_connection(&music_path).await {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Failed to connect to database: {}", e);
            return Err(Box::new(e));
        }
    };

    println!("Scanning music files from: {}", music_path);

    if let Err(e) = initialize_database(&music_path, &db_conn).await {
        eprintln!("Failed to initialize database: {}", e);
    }

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

    // Bind to localhost:2080 for the standalone app
    let ip = "127.0.0.1".to_string();
    let port = 2080;

    // Start the server in the background
    let server = HttpServer::new(move || {
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
    })
    .bind((ip.clone(), port))?
    .run();

    // Run the server in the background
    tokio::spawn(async move {
        match server.await {
            Ok(_) => println!("Server shutdown gracefully"),
            Err(e) => eprintln!("Server error: {}", e),
        }
    });

    // Give the server a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Ok(ServerInfo { ip, port })
}
