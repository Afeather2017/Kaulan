//! File upload API handlers for uploading music files.
//!
//! This module provides endpoints for:
//! - Getting the directory tree structure
//! - Uploading music files to the music directory

use crate::file_ops::{
    is_std_fs_path, source_create_dir_all, source_remove_file, source_write_stream,
    SUPPORTED_EXTENSIONS,
};
use crate::services::scanner;
use crate::types::{AppState, DirectoryNode, UploadResponse};
use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpResponse, Responder};
use bytes::Bytes;
use futures::TryStreamExt;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

/// Get directory tree structure of the music directory
///
/// Returns a hierarchical tree structure of all directories in the music folder.
/// This is used by the frontend to display the folder selection UI for uploads.
///
/// # Returns
/// JSON `DirectoryNode` representing the root directory with nested children
#[get("/api/files/directory-tree")]
pub async fn get_directory_tree(data: web::Data<AppState>) -> impl Responder {
    debug!("Directory tree request received");

    let music_path_str = &*data.music_path;

    fn build_tree(dir_path: &Path, base_path: &Path) -> Option<DirectoryNode> {
        let name = dir_path.file_name()?.to_string_lossy().to_string();
        let relative_path = dir_path
            .strip_prefix(base_path)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

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
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
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

/// Upload a single file to the music directory
///
/// Accepts a multipart form upload with a file and optional target path.
///
/// # Form Fields
/// - `targetPath` (optional): Subdirectory path within music directory
/// - `files`: Single audio file to upload
///
/// # Supported File Types
/// - MP3, OGG, WAV, AAC, FLAC
///
/// # Security
/// - Validates that target path is within music directory (prevents path traversal)
/// - Validates file extension against supported types
/// - Creates target directory if it doesn't exist
///
/// # Example
/// ```bash
/// curl -X POST http://localhost:2080/api/files/upload \
///   -F "targetPath=playlist" \
///   -F "files=@song.mp3"
/// ```
///
/// # Returns
/// JSON response with upload status:
/// ```json
/// {
///   "success": true,
///   "message": "Uploaded 1 file(s)",
///   "uploaded": ["song.mp3"],
///   "failed": []
/// }
/// ```
#[post("/api/files/upload")]
pub async fn upload_files(mut payload: Multipart, data: web::Data<AppState>) -> impl Responder {
    info!("[UPLOAD] ========== FILE UPLOAD REQUEST STARTED ==========");
    let music_path_str = &*data.music_path;
    info!("[UPLOAD] Music directory: {}", music_path_str);

    let mut target_path = String::new();
    let mut file_processed = false;
    let mut uploaded_filename = None;
    let mut failed_filename = None;

    // Process targetPath field first (if present)
    info!("[UPLOAD] Processing multipart fields");
    loop {
        let field_result = payload.try_next().await;
        let mut field = match field_result {
            Ok(f) => match f {
                Some(field) => field,
                None => break,
            },
            Err(e) => {
                error!("[UPLOAD] Error reading field: {}", e);
                break;
            }
        };

        let content_disposition = field.content_disposition();
        let field_name = content_disposition
            .and_then(|cd| cd.get_name())
            .unwrap_or("")
            .to_string();

        info!("[UPLOAD] Processing field: '{}'", field_name);

        match field_name.as_str() {
            "targetPath" => {
                // Read the target path
                let mut path_bytes = Vec::new();
                while let Ok(Some(chunk)) = field.try_next().await {
                    path_bytes.extend_from_slice(&chunk);
                }
                target_path = String::from_utf8_lossy(&path_bytes).to_string();
                info!("[UPLOAD] Target path: {}", target_path);

                // Security check: ensure target path is valid and within music directory
                let target_dir = Path::new(&music_path_str).join(&target_path);

                // Canonicalize both paths for proper comparison
                let music_path_canonical = fs::canonicalize(music_path_str)
                    .unwrap_or_else(|_| PathBuf::from(&music_path_str));

                // Try to canonicalize the target directory - if it would escape, it will fail
                // or resolve outside the music directory
                let target_dir_canonical = if target_dir.exists() {
                    fs::canonicalize(&target_dir).ok()
                } else {
                    // For non-existent paths, we need to check the parent
                    if let Some(parent) = target_dir.parent() {
                        fs::canonicalize(parent).ok()
                    } else {
                        None
                    }
                };

                // Check if the target would be outside the music directory
                if let Some(canonical) = target_dir_canonical {
                    if !canonical.starts_with(&music_path_canonical) {
                        warn!(
                            "[UPLOAD] Invalid target path: {} (not within music directory)",
                            target_path
                        );
                        return HttpResponse::BadRequest().json(UploadResponse {
                            success: false,
                            message: "Invalid target path".to_string(),
                            uploaded: vec![],
                            failed: vec![],
                        });
                    }
                } else {
                    // Fallback: check for path traversal patterns in the raw path
                    if target_path.contains("..") {
                        warn!(
                            "[UPLOAD] Invalid target path: {} (contains path traversal)",
                            target_path
                        );
                        return HttpResponse::BadRequest().json(UploadResponse {
                            success: false,
                            message: "Invalid target path".to_string(),
                            uploaded: vec![],
                            failed: vec![],
                        });
                    }
                }

                // Create target directory if it doesn't exist
                if !target_dir.exists() {
                    if !is_std_fs_path(target_dir.to_string_lossy().as_ref()) {
                        error!(
                            "[UPLOAD] Unsupported target directory source: {}",
                            target_dir.display()
                        );
                        return HttpResponse::InternalServerError().json(UploadResponse {
                            success: false,
                            message: "Unsupported upload target".to_string(),
                            uploaded: vec![],
                            failed: vec![],
                        });
                    }
                    if let Err(e) =
                        source_create_dir_all(target_dir.to_string_lossy().as_ref()).await
                    {
                        error!(
                            "[UPLOAD] Failed to create target directory {}: {}",
                            target_dir.display(),
                            e
                        );
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
                if file_processed {
                    warn!("[UPLOAD] Multiple files detected, only processing first file");
                    // Skip additional files
                    while let Ok(Some(_)) = field.try_next().await {}
                    continue;
                }

                file_processed = true;
                let filename = content_disposition
                    .and_then(|cd| cd.get_filename())
                    .unwrap_or("unknown")
                    .to_string();

                // Validate file extension
                if let Some(extension) = Path::new(&filename).extension() {
                    let ext_str = extension.to_string_lossy().to_lowercase();
                    if !SUPPORTED_EXTENSIONS.contains(&ext_str.as_str()) {
                        warn!("[UPLOAD] Unsupported file type: {}", filename);
                        failed_filename = Some(filename.clone());
                        // Consume remaining field data
                        while let Ok(Some(_)) = field.try_next().await {}
                        continue;
                    }
                } else {
                    warn!("[UPLOAD] File without extension: {}", filename);
                    failed_filename = Some(filename.clone());
                    // Consume remaining field data
                    while let Ok(Some(_)) = field.try_next().await {}
                    continue;
                }

                // Determine the full file path
                let full_target_path = if target_path.is_empty() {
                    Path::new(&music_path_str).join(&filename)
                } else {
                    Path::new(&music_path_str)
                        .join(&target_path)
                        .join(&filename)
                };

                // Security check: ensure file path is within music directory
                if !full_target_path.starts_with(music_path_str) {
                    warn!(
                        "[UPLOAD] Invalid file path: {} (not within music directory)",
                        full_target_path.display()
                    );
                    failed_filename = Some(filename.clone());
                    // Consume remaining field data
                    while let Ok(Some(_)) = field.try_next().await {}
                    continue;
                }

                let mut file_size = 0u64;
                let mut chunks = Vec::<Bytes>::new();
                while let Ok(Some(chunk)) = field.try_next().await {
                    file_size += chunk.len() as u64;
                    chunks.push(chunk);
                }

                let full_target_str = full_target_path.to_string_lossy().to_string();
                match source_write_stream(&full_target_str, chunks).await {
                    Ok(()) => {
                        info!(
                            "[UPLOAD] Successfully uploaded file: {} ({} bytes) -> {}",
                            filename,
                            file_size,
                            full_target_path.display()
                        );
                        uploaded_filename = Some(filename);
                    }
                    Err(e) => {
                        error!("[UPLOAD] Failed to write file {}: {}", filename, e);
                        let _ = source_remove_file(&full_target_str).await;
                        failed_filename = Some(filename);
                    }
                }
            }
            _ => {
                debug!("[UPLOAD] Ignoring unknown field: {}", field_name);
                // Consume unknown field data
                while let Ok(Some(_)) = field.try_next().await {}
            }
        }
    }

    // Build response arrays (format stays the same for API compatibility)
    let uploaded_files = match uploaded_filename {
        Some(f) => vec![f],
        None => vec![],
    };
    let failed_files = match failed_filename {
        Some(f) => vec![f],
        None => vec![],
    };

    if uploaded_files.is_empty() && failed_files.is_empty() {
        warn!("[UPLOAD] No files provided in request");
        return HttpResponse::BadRequest().json(UploadResponse {
            success: false,
            message: "No files provided".to_string(),
            uploaded: vec![],
            failed: vec![],
        });
    }

    info!(
        "[UPLOAD] Upload summary: {} successful, {} failed",
        uploaded_files.len(),
        failed_files.len()
    );

    // Trigger database update after successful upload
    if !uploaded_files.is_empty() {
        info!("[UPLOAD] ========== TRIGGERING DATABASE UPDATE AFTER UPLOAD ==========");
        info!("[UPLOAD] Files to process: {:?}", uploaded_files);
        let library_roots = [
            data.music_path.as_ref().as_str(),
            data.download_root.as_ref().as_str(),
        ];
        match scanner::update_database_with_roots(&library_roots, &data.db_conn).await {
            Ok(_) => {
                info!("[UPLOAD] ========== DATABASE UPDATE COMPLETED SUCCESSFULLY ==========");
            }
            Err(e) => {
                warn!("[UPLOAD] Database update failed after upload: {}", e);
            }
        }
    }

    let success = !uploaded_files.is_empty();
    let message = if success {
        format!("Uploaded {} file(s)", uploaded_files.len())
    } else {
        "Upload failed".to_string()
    };

    info!(
        "[UPLOAD] ========== UPLOAD REQUEST COMPLETE: {} ==========",
        success
    );
    HttpResponse::Ok().json(UploadResponse {
        success,
        message,
        uploaded: uploaded_files,
        failed: failed_files,
    })
}
