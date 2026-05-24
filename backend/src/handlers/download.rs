//! HTTP handlers for online music download (YouTube).

use actix_web::{get, post, web, HttpResponse};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

use crate::types::{
    AppState, DirectoryNode, DownloadTrackRequest, DownloadTrackResponse,
    OnlineSearchRequest, OnlineSearchResult,
};
use crate::services::scanner;
use ytdl_audio::{DownloadOpts, YoutubeClient};

/// Search YouTube for music matching the given query.
#[post("/api/download/search")]
pub async fn search_online(
    body: web::Json<OnlineSearchRequest>,
) -> HttpResponse {
    let client = match YoutubeClient::new(None) {
        Ok(c) => c,
        Err(e) => {
            error!("[DOWNLOAD] Failed to create YouTube client: {}", e);
            return HttpResponse::InternalServerError()
                .json(DownloadTrackResponse {
                    success: false,
                    message: "无法初始化搜索客户端".to_string(),
                    filename: None,
                });
        }
    };

    info!(
        "[DOWNLOAD] Searching YouTube for: {} (max_results={})",
        body.query, body.max_results
    );

    match client.search(&body.query, body.max_results).await {
        Ok(videos) => {
            let results: Vec<OnlineSearchResult> = videos
                .into_iter()
                .map(|v| OnlineSearchResult {
                    thumbnail_url: format!(
                        "https://i.ytimg.com/vi/{}/hqdefault.jpg",
                        v.id
                    ),
                    id: v.id,
                    title: v.title,
                    channel: v.channel,
                    duration: v.duration,
                })
                .collect();
            info!("[DOWNLOAD] Found {} results", results.len());
            HttpResponse::Ok().json(results)
        }
        Err(e) => {
            error!("[DOWNLOAD] YouTube search failed: {}", e);
            HttpResponse::BadGateway().json(DownloadTrackResponse {
                success: false,
                message: "搜索失败，无法连接到YouTube".to_string(),
                filename: None,
            })
        }
    }
}

/// Get directory tree under the download directory for the folder picker.
#[get("/api/download/directory-tree")]
pub async fn get_download_directory_tree(data: web::Data<AppState>) -> HttpResponse {
    if cfg!(target_os = "android") && !has_android_external_storage_permission() {
        warn!("[DOWNLOAD] Rejecting directory tree request without Android external storage permission");
        return HttpResponse::Forbidden().body("External storage permission required");
    }

    let music_path = data.music_path.as_ref();
    let base = Path::new(music_path);

    if !base.exists() {
        if let Err(e) = fs::create_dir_all(base) {
            error!("[DOWNLOAD] Failed to create download dir: {}", e);
            return HttpResponse::InternalServerError()
                .body("Failed to create download directory");
        }
    }

    fn build_tree(dir_path: &Path, base_path: &Path) -> Option<DirectoryNode> {
        let name = dir_path
            .file_name()?
            .to_string_lossy()
            .to_string();
        let relative_path = dir_path
            .strip_prefix(base_path)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut children = Vec::new();
        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        if let Some(node) = build_tree(&entry.path(), base_path) {
                            children.push(node);
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

    match build_tree(base, base) {
        Some(root) => HttpResponse::Ok().json(root),
        None => HttpResponse::InternalServerError()
            .body("Failed to generate directory tree"),
    }
}

/// Download a YouTube track, remux to the correct container, and register in the database.
#[post("/api/download/track")]
pub async fn download_track(
    body: web::Json<DownloadTrackRequest>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let client = match YoutubeClient::new(None) {
        Ok(c) => c,
        Err(e) => {
            error!("[DOWNLOAD] Failed to create YouTube client: {}", e);
            return HttpResponse::InternalServerError().json(
                DownloadTrackResponse {
                    success: false,
                    message: "无法初始化下载客户端".to_string(),
                    filename: None,
                },
            );
        }
    };

    let video_url = format!(
        "https://www.youtube.com/watch?v={}",
        body.video_id
    );
    info!(
        "[DOWNLOAD] Downloading: {} ({})",
        body.title, body.video_id
    );

    let temp_dir = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => {
            error!("[DOWNLOAD] Failed to create temp dir: {}", e);
            return HttpResponse::InternalServerError().json(
                DownloadTrackResponse {
                    success: false,
                    message: "创建临时目录失败".to_string(),
                    filename: None,
                },
            );
        }
    };
    let temp_path = temp_dir.path().to_path_buf();

    let opts = DownloadOpts {
        output_dir: temp_path.to_string_lossy().to_string(),
        ..Default::default()
    };

    let download_result = match client.download(&video_url, opts).await {
        Ok(r) => r,
        Err(e) => {
            error!("[DOWNLOAD] Download failed: {}", e);
            return HttpResponse::BadGateway().json(DownloadTrackResponse {
                success: false,
                message: format!("下载失败: {}", e),
                filename: None,
            });
        }
    };

    let downloaded_path = download_result.audio_path;
    let thumbnail_path = download_result.thumbnail_path;

    if !downloaded_path.exists() {
        return HttpResponse::InternalServerError().json(
            DownloadTrackResponse {
                success: false,
                message: "下载的音频文件不存在".to_string(),
                filename: None,
            },
        );
    }

    // Detect codec to pick the right container:
    //   Opus → .ogg (no cover — OGG muxer doesn't support attached pictures)
    //   AAC  → .m4a (with cover)
    let codec = detect_codec(&downloaded_path);
    let (output_ext, embed_cover) = if codec.contains("opus") {
        ("ogg", false)
    } else {
        ("m4a", true)
    };

    info!(
        "[DOWNLOAD] Codec={}, container={}, cover={}",
        codec, output_ext, embed_cover
    );

    let safe_title = sanitize_filename(&body.title);
    let final_filename = format!("{}.{}", safe_title, output_ext);

    if cfg!(target_os = "android") && !has_android_external_storage_permission() {
        warn!("[DOWNLOAD] Rejecting download without Android external storage permission");
        return HttpResponse::Forbidden().json(DownloadTrackResponse {
            success: false,
            message: "缺少外部存储权限，无法下载到 /sdcard/Music".to_string(),
            filename: None,
        });
    }

    let download_base = PathBuf::from(data.music_path.as_ref().as_str());
    let target_dir = match resolve_target_dir(&download_base, body.target_subdir.as_deref()) {
        Ok(dir) => dir,
        Err(message) => {
            warn!("[DOWNLOAD] Rejecting invalid target directory: {}", message);
            return HttpResponse::BadRequest().json(DownloadTrackResponse {
                success: false,
                message,
                filename: None,
            });
        }
    };

    if let Err(e) = fs::create_dir_all(&target_dir) {
        error!("[DOWNLOAD] Failed to create target dir: {}", e);
        return HttpResponse::InternalServerError().json(
            DownloadTrackResponse {
                success: false,
                message: "无法创建目标目录".to_string(),
                filename: None,
            },
        );
    }

    let final_path = target_dir.join(&final_filename);

    setup_ffmpeg_path();

    let cover = if embed_cover {
        thumbnail_path.as_deref().filter(|p| p.exists())
    } else {
        None
    };

    match ytdl_audio::convert_audio(&downloaded_path, &final_path, cover) {
        Ok(()) => {
            info!(
                "[DOWNLOAD] Converted to {} (cover={})",
                final_path.display(),
                cover.is_some()
            );
        }
        Err(e) => {
            error!("[DOWNLOAD] Audio conversion failed: {}", e);
            return HttpResponse::InternalServerError().json(
                DownloadTrackResponse {
                    success: false,
                    message: format!("音频转换失败: {}", e),
                    filename: None,
                },
            );
        }
    }

    // Update the music database so the new file appears in the library
    if let Err(e) = scanner::update_database(data.music_path.as_ref(), &data.db_conn).await {
        warn!(
            "[DOWNLOAD] Database update after download failed: {}",
            e
        );
    }

    info!("[DOWNLOAD] Download complete: {}", final_filename);

    HttpResponse::Ok().json(DownloadTrackResponse {
        success: true,
        message: "下载完成".to_string(),
        filename: Some(final_filename),
    })
}

fn resolve_target_dir(base_dir: &Path, target_subdir: Option<&str>) -> Result<PathBuf, String> {
    let requested_subdir = target_subdir.unwrap_or("").trim();
    let requested_path = if requested_subdir.is_empty() {
        base_dir.to_path_buf()
    } else {
        base_dir.join(requested_subdir)
    };

    if requested_subdir.is_empty() {
        return Ok(requested_path);
    }

    let base_canonical = fs::canonicalize(base_dir).unwrap_or_else(|_| base_dir.to_path_buf());
    let parent = requested_path
        .parent()
        .ok_or_else(|| "目标目录无效".to_string())?;
    let parent_canonical = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    if !parent_canonical.starts_with(&base_canonical) {
        return Err("目标目录必须位于音乐目录内".to_string());
    }

    let relative = requested_path
        .strip_prefix(base_dir)
        .map_err(|_| "目标目录必须位于音乐目录内".to_string())?;
    if relative
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err("目标目录不能包含上级路径".to_string());
    }

    Ok(requested_path)
}

fn has_android_external_storage_permission() -> bool {
    std::env::var("KAULAN_ANDROID_EXTERNAL_STORAGE_GRANTED")
        .map(|value| value == "true")
        .unwrap_or(false)
}

/// Detect the audio codec of a downloaded file using ffprobe.
fn detect_codec(path: &Path) -> String {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout)
                .trim()
                .to_lowercase()
        }
        _ => {
            // ffprobe unavailable — guess from extension
            match path.extension().and_then(|e| e.to_str()) {
                Some("webm") => "opus".to_string(),
                Some("mp4") | Some("m4a") => "aac".to_string(),
                ext => format!("unknown({})", ext.unwrap_or("none")),
            }
        }
    }
}

/// On Android, prepend the ffmpeg binary location to PATH so that
/// `convert_audio()` (which calls `ffmpeg` via PATH) can find it.
fn setup_ffmpeg_path() {
    if cfg!(target_os = "android") {
        if let Ok(data_dir) = std::env::var("TAURI_ANDROID_DATA_DIR") {
            let ffmpeg_dir = format!("{}/files", data_dir);
            if let Ok(current) = std::env::var("PATH") {
                std::env::set_var("PATH", format!("{}:{}", ffmpeg_dir, current));
            } else {
                std::env::set_var("PATH", ffmpeg_dir);
            }
        }
    }
}

/// Sanitize a YouTube title for use as a filename.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::resolve_target_dir;
    use std::fs;

    #[test]
    fn resolve_target_dir_accepts_nested_subdir_within_music_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_dir = temp_dir.path().join("Music");
        fs::create_dir_all(base_dir.join("Albums")).unwrap();

        let resolved = resolve_target_dir(&base_dir, Some("Albums/Live")).unwrap();

        assert_eq!(resolved, base_dir.join("Albums/Live"));
    }

    #[test]
    fn resolve_target_dir_rejects_parent_traversal() {
        let temp_dir = tempfile::tempdir().unwrap();
        let base_dir = temp_dir.path().join("Music");
        fs::create_dir_all(&base_dir).unwrap();

        let error = resolve_target_dir(&base_dir, Some("../Escape")).unwrap_err();

        assert!(error.contains("音乐目录") || error.contains("上级路径"));
    }
}
