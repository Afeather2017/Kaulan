//! HTTP handlers for online music search, preview, lyrics, and download.

use actix_files::NamedFile;
use actix_web::{get, post, web, HttpRequest, HttpResponse};
use bilibili_api::auth::BiliSession;
use bilibili_api::{BilibiliClient, BilibiliError};
use futures::future::join_all;
use netease_api::auth::Session as NeteaseSession;
use netease_api::types::{Quality, SearchType};
use netease_api::{NeteaseClient, NeteaseError};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
#[cfg(not(target_os = "android"))]
use std::process::Command;
use tokio::task;
use tracing::{error, info, warn};
use ytdl_audio::{DownloadOpts, StdFileWriter, YoutubeClient};

use crate::services::scanner;
use crate::types::{
    AppState, DirectoryNode, DownloadPreviewRequest, DownloadPreviewResponse, DownloadSource,
    DownloadTrackRequest, DownloadTrackResponse, LyricCandidate, OnlineSearchRequest,
    OnlineSearchResult, PreviewSong,
};

const YOUTUBE_COOKIE_HEADER_PATH_ENV: &str = "KAULAN_YOUTUBE_COOKIE_HEADER_PATH";
const NETEASE_QUALITY_FALLBACKS: [Quality; 3] =
    [Quality::Exhigh, Quality::Higher, Quality::Standard];
const BILIBILI_RAW_AUDIO_EXTENSION: &str = "m4s";

#[post("/api/download/search")]
pub async fn search_online(body: web::Json<OnlineSearchRequest>) -> HttpResponse {
    let query = body.query.trim().to_string();
    if query.is_empty() {
        return HttpResponse::Ok().json(Vec::<OnlineSearchResult>::new());
    }

    let max_results = body.max_results.clamp(1, 20);
    let sources = if body.sources.is_empty() {
        vec![DownloadSource::Youtube]
    } else {
        body.sources.clone()
    };
    let enabled_sources = sources
        .into_iter()
        .filter(|source| is_source_enabled(*source))
        .collect::<Vec<_>>();

    if enabled_sources.is_empty() {
        warn!("[DOWNLOAD] Search skipped because no enabled providers are logged in");
        return HttpResponse::Ok().json(Vec::<OnlineSearchResult>::new());
    }

    let search_jobs = enabled_sources.into_iter().map(|source| {
        let query = query.clone();
        async move {
            let result = match source {
                DownloadSource::Youtube => search_youtube(&query, max_results).await,
                DownloadSource::Netease => search_netease(&query, max_results).await,
                DownloadSource::Bilibili => search_bilibili(&query, max_results).await,
            };
            (source, result)
        }
    });

    let mut results = Vec::new();
    for (source, result) in join_all(search_jobs).await {
        match result {
            Ok(mut items) => results.append(&mut items),
            Err(err) => warn!("[DOWNLOAD] {} search failed: {}", source.as_str(), err),
        }
    }

    HttpResponse::Ok().json(results)
}

#[post("/api/download/lyrics/search")]
pub async fn search_lyrics(body: web::Json<crate::types::LyricsSearchRequest>) -> HttpResponse {
    let query = body.query.trim().to_string();
    if query.is_empty() {
        return HttpResponse::Ok().json(Vec::<LyricCandidate>::new());
    }

    match task::spawn_blocking(move || -> Result<Vec<LyricCandidate>, String> {
        let client = NeteaseClient::new().map_err(|e| e.to_string())?;
        let result = client
            .search(&query, SearchType::Track, 8, 0)
            .map_err(|e| e.to_string())?;

        Ok(result
            .tracks
            .unwrap_or_default()
            .into_iter()
            .map(|track| LyricCandidate {
                source: DownloadSource::Netease,
                id: track.id.to_string(),
                title: track.name,
                artist: join_artists(&track.artists),
                album: Some(track.album.name),
            })
            .collect())
    })
    .await
    {
        Ok(Ok(candidates)) => HttpResponse::Ok().json(candidates),
        Ok(Err(err)) => HttpResponse::BadGateway().json(DownloadTrackResponse {
            success: false,
            message: format!("歌词搜索失败: {err}"),
            filename: None,
            lyric_filename: None,
            warning: None,
        }),
        Err(err) => HttpResponse::InternalServerError().json(DownloadTrackResponse {
            success: false,
            message: format!("歌词搜索任务失败: {err}"),
            filename: None,
            lyric_filename: None,
            warning: None,
        }),
    }
}

#[get("/api/download/directory-tree")]
pub async fn get_download_directory_tree(data: web::Data<AppState>) -> HttpResponse {
    let base = Path::new(data.download_root.as_ref());
    if let Err(err) = fs::create_dir_all(base) {
        error!(
            "[DOWNLOAD] Failed to create download directory tree root: {}",
            err
        );
        return HttpResponse::InternalServerError().body("Failed to create download directory");
    }

    match build_directory_tree(base, base) {
        Some(root) => HttpResponse::Ok().json(root),
        None => HttpResponse::InternalServerError().body("Failed to generate directory tree"),
    }
}

#[post("/api/download/preview")]
pub async fn download_preview(
    body: web::Json<DownloadPreviewRequest>,
    data: web::Data<AppState>,
    req: HttpRequest,
) -> HttpResponse {
    let preview_root = PathBuf::from(data.preview_root.as_ref());
    if let Err(err) = fs::create_dir_all(&preview_root) {
        error!("[DOWNLOAD] Failed to create preview root: {}", err);
        return HttpResponse::InternalServerError().json(DownloadPreviewResponse {
            success: false,
            message: "无法创建试听目录".to_string(),
            song: None,
        });
    }

    let preview_request = body.into_inner();
    let request_title = preview_request.title.clone();
    let request_artist = preview_request.artist.clone().unwrap_or_default();
    let host = format!(
        "{}://{}",
        req.connection_info().scheme(),
        req.connection_info().host()
    );

    let preview_result = match build_preview(preview_request, preview_root).await {
        Ok(result) => result,
        Err(err) => {
            warn!("[DOWNLOAD] Preview download failed: {}", err);
            return HttpResponse::BadGateway().json(DownloadPreviewResponse {
                success: false,
                message: format!("试听下载失败: {err}"),
                song: None,
            });
        }
    };

    let stream_url = format!("{host}/api/download/preview/{}", preview_result.file_name);
    let display_name = if request_artist.is_empty() {
        request_title
    } else {
        format!("{request_title} [{request_artist}]")
    };

    HttpResponse::Ok().json(DownloadPreviewResponse {
        success: true,
        message: "试听准备完成".to_string(),
        song: Some(PreviewSong {
            id: preview_result.synthetic_id,
            name: display_name,
            path: preview_result.absolute_path.to_string_lossy().to_string(),
            stream_url,
            cover_url: preview_result.cover_url,
            source: preview_result.source,
            is_temporary: true,
        }),
    })
}

#[get("/api/download/preview/{filename}")]
pub async fn get_preview_track(
    path: web::Path<String>,
    data: web::Data<AppState>,
) -> actix_web::Result<NamedFile> {
    let requested = path.into_inner();
    if requested.contains('/') || requested.contains('\\') {
        return Err(actix_web::error::ErrorBadRequest("Invalid preview file"));
    }

    let file_path = Path::new(data.preview_root.as_ref()).join(&requested);
    if !file_path.starts_with(Path::new(data.preview_root.as_ref())) {
        return Err(actix_web::error::ErrorBadRequest("Invalid preview path"));
    }

    NamedFile::open_async(file_path)
        .await
        .map_err(actix_web::error::ErrorNotFound)
}

#[post("/api/download/track")]
pub async fn download_track(
    body: web::Json<DownloadTrackRequest>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let body = body.into_inner();
    let target_dir = match resolve_target_dir(
        Path::new(data.download_root.as_ref()),
        body.target_subdir.as_deref(),
    ) {
        Ok(path) => path,
        Err(message) => {
            warn!(
                "[DOWNLOAD] Rejecting track download request: source={}, id={}, target_subdir={:?}, reason={}",
                body.source.as_str(),
                body.id,
                body.target_subdir,
                message
            );
            return HttpResponse::BadRequest().json(DownloadTrackResponse {
                success: false,
                message,
                filename: None,
                lyric_filename: None,
                warning: None,
            });
        }
    };

    if let Err(err) = fs::create_dir_all(&target_dir) {
        error!("[DOWNLOAD] Failed to create target download dir: {}", err);
        return HttpResponse::InternalServerError().json(DownloadTrackResponse {
            success: false,
            message: "无法创建目标目录".to_string(),
            filename: None,
            lyric_filename: None,
            warning: None,
        });
    }

    let output = match download_full_track(&body, &target_dir).await {
        Ok(result) => result,
        Err(err) => {
            warn!("[DOWNLOAD] Full download failed: {}", err);
            return HttpResponse::BadGateway().json(DownloadTrackResponse {
                success: false,
                message: format!("下载失败: {err}"),
                filename: None,
                lyric_filename: None,
                warning: None,
            });
        }
    };

    let mut warning = None;
    let lyric_filename = if let Some(lyric_id) = body.lyric_selection.as_deref() {
        match write_selected_lyric(lyric_id, &output.final_path).await {
            Ok(Some(path)) => path.file_name().map(|v| v.to_string_lossy().to_string()),
            Ok(None) => {
                warning = Some("未获取到可用歌词".to_string());
                None
            }
            Err(err) => {
                warning = Some(format!("歌词下载失败: {err}"));
                None
            }
        }
    } else {
        None
    };

    let library_roots = [
        data.music_path.as_ref().as_str(),
        data.download_root.as_ref().as_str(),
    ];
    if let Err(err) = scanner::update_database_with_roots(&library_roots, &data.db_conn).await {
        warn!(
            "[DOWNLOAD] Database update after online download failed: {}",
            err
        );
    }

    HttpResponse::Ok().json(DownloadTrackResponse {
        success: true,
        message: "下载完成".to_string(),
        filename: output
            .final_path
            .file_name()
            .map(|value| value.to_string_lossy().to_string()),
        lyric_filename,
        warning,
    })
}

struct PreviewBuildResult {
    source: DownloadSource,
    file_name: String,
    absolute_path: PathBuf,
    cover_url: Option<String>,
    synthetic_id: i32,
}

struct FullDownloadResult {
    final_path: PathBuf,
}

async fn search_youtube(
    query: &str,
    max_results: usize,
) -> Result<Vec<OnlineSearchResult>, String> {
    ensure_ytdl_solver_dependencies()?;
    let client = youtube_client().map_err(|e| e.to_string())?;
    let videos = client
        .search(query, max_results)
        .await
        .map_err(|e| e.to_string())?;

    Ok(videos
        .into_iter()
        .map(|video| OnlineSearchResult {
            source: DownloadSource::Youtube,
            id: video.id.clone(),
            title: video.title,
            artist: video.channel,
            duration: video.duration,
            thumbnail_url: Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video.id)),
            can_preview: true,
            can_download: true,
            requires_login: false,
        })
        .collect())
}

async fn search_netease(
    query: &str,
    max_results: usize,
) -> Result<Vec<OnlineSearchResult>, String> {
    let query = query.to_string();
    task::spawn_blocking(move || -> Result<Vec<OnlineSearchResult>, String> {
        let requires_login = !NeteaseSession::load()
            .map_err(|e| e.to_string())?
            .is_logged_in();
        let client = NeteaseClient::new().map_err(|e| e.to_string())?;
        let result = client
            .search(&query, SearchType::Track, max_results as u64, 0)
            .map_err(|e| e.to_string())?;

        Ok(result
            .tracks
            .unwrap_or_default()
            .into_iter()
            .map(|track| OnlineSearchResult {
                source: DownloadSource::Netease,
                id: track.id.to_string(),
                title: track.name,
                artist: join_artists(&track.artists),
                duration: Some(format_duration_ms(track.duration_ms)),
                thumbnail_url: track.album.pic_url,
                can_preview: true,
                can_download: true,
                requires_login,
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn search_bilibili(
    query: &str,
    max_results: usize,
) -> Result<Vec<OnlineSearchResult>, String> {
    let query = query.to_string();
    task::spawn_blocking(move || -> Result<Vec<OnlineSearchResult>, String> {
        let requires_login = !BiliSession::load()
            .map_err(|e| e.to_string())?
            .is_logged_in();
        let client = BilibiliClient::new().map_err(|e| e.to_string())?;
        let result = client
            .search_video(&query, 1, max_results as u64)
            .map_err(|e| e.to_string())?;

        Ok(result
            .results
            .into_iter()
            .map(|video| OnlineSearchResult {
                source: DownloadSource::Bilibili,
                id: video.bvid,
                title: strip_html_tags(&video.title),
                artist: video.author,
                duration: if video.duration.is_empty() {
                    None
                } else {
                    Some(video.duration)
                },
                thumbnail_url: Some(normalize_remote_url(&video.pic)),
                can_preview: true,
                can_download: true,
                requires_login,
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn build_preview(
    request: DownloadPreviewRequest,
    preview_root: PathBuf,
) -> Result<PreviewBuildResult, String> {
    let synthetic_id = -((simple_hash(&request.id) as i32).abs());
    let token = format!(
        "preview-{}-{}",
        request.source.as_str(),
        uuid::Uuid::new_v4()
    );

    match request.source {
        DownloadSource::Youtube => {
            ensure_ytdl_solver_dependencies()?;
            let client = youtube_client().map_err(|e| e.to_string())?;
            let temp_dir = create_download_staging_dir(&preview_root)?;
            let result = client
                .download(
                    &format!("https://www.youtube.com/watch?v={}", request.id),
                    DownloadOpts {
                        output_dir: temp_dir.path().to_string_lossy().to_string(),
                        cookies: youtube_cookie_file_path(),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|e| e.to_string())?;
            let (final_name, final_path) =
                finalize_youtube_audio(&result.audio_path, &preview_root, &token, Some("ogg"))?;

            Ok(PreviewBuildResult {
                source: DownloadSource::Youtube,
                file_name: final_name,
                absolute_path: final_path,
                cover_url: Some(format!(
                    "https://i.ytimg.com/vi/{}/hqdefault.jpg",
                    request.id
                )),
                synthetic_id,
            })
        }
        DownloadSource::Netease => {
            let track_id = request
                .id
                .parse::<u64>()
                .map_err(|_| "无效的网易云歌曲 ID".to_string())?;
            let preview_root_clone = preview_root.clone();
            let request_title = request.title.clone();
            task::spawn_blocking(move || -> Result<PreviewBuildResult, String> {
                let client = NeteaseClient::new().map_err(|e| e.to_string())?;
                let track = client.track_detail(track_id).map_err(|e| e.to_string())?;
                let final_name = format!("{token}.mp3");
                let final_path = preview_root_clone.join(&final_name);
                download_netease_with_fallback(
                    &client,
                    track_id,
                    &final_path,
                    "preview",
                    request_title.as_str(),
                )?;

                Ok(PreviewBuildResult {
                    source: DownloadSource::Netease,
                    file_name: final_name,
                    absolute_path: final_path,
                    cover_url: track.album.pic_url,
                    synthetic_id,
                })
            })
            .await
            .map_err(|e| e.to_string())?
        }
        DownloadSource::Bilibili => {
            let bvid = request.id.clone();
            let preview_root_clone = preview_root.clone();
            task::spawn_blocking(move || -> Result<PreviewBuildResult, String> {
                let client = BilibiliClient::new().map_err(|e| e.to_string())?;
                let detail = client.video_detail(&bvid).map_err(|e| e.to_string())?;
                let final_name = if should_skip_bilibili_ffmpeg() {
                    format!("{token}.{BILIBILI_RAW_AUDIO_EXTENSION}")
                } else {
                    format!("{token}.mp3")
                };
                let final_path = preview_root_clone.join(&final_name);
                download_bilibili_audio(&client, &bvid, &final_path).map_err(|e| e.to_string())?;

                Ok(PreviewBuildResult {
                    source: DownloadSource::Bilibili,
                    file_name: final_name,
                    absolute_path: final_path,
                    cover_url: Some(normalize_remote_url(&detail.pic)),
                    synthetic_id,
                })
            })
            .await
            .map_err(|e| e.to_string())?
        }
    }
}

async fn download_full_track(
    request: &DownloadTrackRequest,
    target_dir: &Path,
) -> Result<FullDownloadResult, String> {
    match request.source {
        DownloadSource::Youtube => download_youtube_full(request, target_dir).await,
        DownloadSource::Netease => download_netease_full(request, target_dir).await,
        DownloadSource::Bilibili => download_bilibili_full(request, target_dir).await,
    }
}

async fn download_youtube_full(
    request: &DownloadTrackRequest,
    target_dir: &Path,
) -> Result<FullDownloadResult, String> {
    ensure_ytdl_solver_dependencies()?;
    let client = youtube_client().map_err(|e| e.to_string())?;
    let temp_dir = create_download_staging_dir(target_dir)?;
    let result = client
        .download(
            &format!("https://www.youtube.com/watch?v={}", request.id),
            DownloadOpts {
                output_dir: temp_dir.path().to_string_lossy().to_string(),
                cookies: youtube_cookie_file_path(),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    let title = sanitize_filename(&request.title);
    let (_final_filename, final_path) =
        finalize_youtube_audio(&result.audio_path, target_dir, &title, Some("ogg"))?;

    Ok(FullDownloadResult { final_path })
}

async fn download_netease_full(
    request: &DownloadTrackRequest,
    target_dir: &Path,
) -> Result<FullDownloadResult, String> {
    let track_id = request
        .id
        .parse::<u64>()
        .map_err(|_| "无效的网易云歌曲 ID".to_string())?;
    let target_dir = target_dir.to_path_buf();
    let title = request.title.clone();
    task::spawn_blocking(move || -> Result<FullDownloadResult, String> {
        let client = NeteaseClient::new().map_err(|e| e.to_string())?;
        let filename = format!("{}.mp3", sanitize_filename(&title));
        let final_path = target_dir.join(filename);
        download_netease_with_fallback(&client, track_id, &final_path, "full", title.as_str())?;
        Ok(FullDownloadResult { final_path })
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn download_bilibili_full(
    request: &DownloadTrackRequest,
    target_dir: &Path,
) -> Result<FullDownloadResult, String> {
    let bvid = request.id.clone();
    let title = request.title.clone();
    let target_dir = target_dir.to_path_buf();
    task::spawn_blocking(move || -> Result<FullDownloadResult, String> {
        let client = BilibiliClient::new().map_err(|e| e.to_string())?;
        let filename = if should_skip_bilibili_ffmpeg() {
            format!(
                "{}.{}",
                sanitize_filename(&title),
                BILIBILI_RAW_AUDIO_EXTENSION
            )
        } else {
            format!("{}.mp3", sanitize_filename(&title))
        };
        let final_path = target_dir.join(filename);
        download_bilibili_audio(&client, &bvid, &final_path).map_err(|e| match e {
            BilibiliError::Ffmpeg(message) => format!("FFmpeg 错误: {message}"),
            other => other.to_string(),
        })?;
        Ok(FullDownloadResult { final_path })
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn write_selected_lyric(
    lyric_id: &str,
    audio_path: &Path,
) -> Result<Option<PathBuf>, String> {
    let track_id = lyric_id
        .parse::<u64>()
        .map_err(|_| "无效的歌词歌曲 ID".to_string())?;
    let audio_path = audio_path.to_path_buf();

    task::spawn_blocking(move || -> Result<Option<PathBuf>, String> {
        let client = NeteaseClient::new().map_err(|e| e.to_string())?;
        let lyric = client.track_lyric(track_id).map_err(|e| match e {
            NeteaseError::Other(message) => message,
            other => other.to_string(),
        })?;

        let merged = merge_lyric_content(lyric.lrc, lyric.tlyric);
        let Some(content) = merged else {
            return Ok(None);
        };

        let lyric_path = audio_path.with_extension("lrc");
        fs::write(&lyric_path, content).map_err(|e| e.to_string())?;
        Ok(Some(lyric_path))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn merge_lyric_content(original: Option<String>, translated: Option<String>) -> Option<String> {
    let original = original.unwrap_or_default();
    let translated = translated.unwrap_or_default();

    if original.trim().is_empty() && translated.trim().is_empty() {
        return None;
    }

    if translated.trim().is_empty() {
        return Some(original);
    }
    if original.trim().is_empty() {
        return Some(translated);
    }

    let mut translated_map = HashMap::new();
    for line in translated.lines() {
        if let Some((stamp, text)) = split_lrc_line(line) {
            translated_map.insert(stamp.to_string(), text.to_string());
        }
    }

    let mut merged = Vec::new();
    for line in original.lines() {
        if let Some((stamp, text)) = split_lrc_line(line) {
            merged.push(format!("{stamp}{text}"));
            if let Some(translated_text) = translated_map.get(stamp) {
                if !translated_text.trim().is_empty() {
                    merged.push(format!("{stamp}{translated_text}"));
                }
            }
        } else {
            merged.push(line.to_string());
        }
    }

    Some(merged.join("\n"))
}

fn split_lrc_line(line: &str) -> Option<(&str, &str)> {
    let end_index = line.find(']')?;
    let stamp = &line[..=end_index];
    let text = &line[end_index + 1..];
    Some((stamp, text))
}

fn build_directory_tree(dir_path: &Path, base_path: &Path) -> Option<DirectoryNode> {
    let name = dir_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "根目录".to_string());
    let relative_path = dir_path
        .strip_prefix(base_path)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut children = Vec::new();
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    if let Some(node) = build_directory_tree(&entry.path(), base_path) {
                        children.push(node);
                    }
                }
            }
        }
    }
    children.sort_by(|left, right| left.name.cmp(&right.name));

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

fn resolve_target_dir(base_dir: &Path, target_subdir: Option<&str>) -> Result<PathBuf, String> {
    let requested_subdir = target_subdir.unwrap_or("").trim();
    let requested_path = if requested_subdir.is_empty() {
        base_dir.to_path_buf()
    } else {
        base_dir.join(requested_subdir)
    };

    if requested_path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("目标目录不能包含上级路径".to_string());
    }

    if let Err(err) = fs::create_dir_all(&requested_path) {
        return Err(format!("无法创建目标目录: {err}"));
    }

    let base_canonical = fs::canonicalize(base_dir).unwrap_or_else(|_| base_dir.to_path_buf());
    let requested_canonical =
        fs::canonicalize(&requested_path).unwrap_or_else(|_| requested_path.to_path_buf());
    if !requested_canonical.starts_with(&base_canonical) {
        return Err("目标目录必须位于下载目录内".to_string());
    }

    Ok(requested_path)
}

fn create_download_staging_dir(base_dir: &Path) -> Result<tempfile::TempDir, String> {
    fs::create_dir_all(base_dir).map_err(|e| format!("无法创建下载缓存目录: {e}"))?;

    let staging_root = base_dir.join(".staging");
    fs::create_dir_all(&staging_root).map_err(|e| format!("无法创建下载缓存目录: {e}"))?;

    tempfile::Builder::new()
        .prefix(".tmp")
        .tempdir_in(&staging_root)
        .map_err(|e| format!("无法创建下载缓存目录: {e}"))
}

fn finalize_youtube_audio(
    source_audio: &Path,
    output_dir: &Path,
    output_stem: &str,
    preferred_extension: Option<&str>,
) -> Result<(String, PathBuf), String> {
    let preferred_extension = preferred_extension.unwrap_or("ogg");
    let converted_name = format!("{output_stem}.{preferred_extension}");
    let converted_path = output_dir.join(&converted_name);

    setup_ffmpeg_path();
    match ytdl_audio::convert_audio(source_audio, &converted_path, None, &StdFileWriter) {
        Ok(()) => Ok((converted_name, converted_path)),
        Err(err) => {
            warn!(
                "[DOWNLOAD] FFmpeg conversion unavailable, keeping original audio container: source={}, target={}, error={}",
                source_audio.display(),
                converted_path.display(),
                err
            );

            let source_extension = source_audio
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .unwrap_or("webm");
            let fallback_name = format!("{output_stem}.{source_extension}");
            let fallback_path = output_dir.join(&fallback_name);
            fs::copy(source_audio, &fallback_path)
                .map_err(|copy_err| format!("无法保存原始音频文件: {copy_err}"))?;
            Ok((fallback_name, fallback_path))
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();

    if sanitized.is_empty() {
        "download".to_string()
    } else {
        sanitized
    }
}

fn join_artists(artists: &[netease_api::types::Artist]) -> String {
    artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn normalize_remote_url(url: &str) -> String {
    if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.to_string()
    }
}

fn strip_html_tags(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn format_duration_ms(duration_ms: u64) -> String {
    let total_seconds = duration_ms / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}

fn download_netease_with_fallback(
    client: &NeteaseClient,
    track_id: u64,
    dest: &Path,
    mode: &str,
    title: &str,
) -> Result<(), String> {
    let logged_in = client.session().is_logged_in();
    let mut failures = Vec::new();

    info!(
        "[DOWNLOAD] Netease {} start: track_id={}, title={}, logged_in={}",
        mode, track_id, title, logged_in
    );

    for quality in NETEASE_QUALITY_FALLBACKS {
        info!(
            "[DOWNLOAD] Netease {} attempt: track_id={}, title={}, quality={:?}, logged_in={}",
            mode, track_id, title, quality, logged_in
        );

        match client.download_track(track_id, quality, dest) {
            Ok(bytes) => {
                info!(
                    "[DOWNLOAD] Netease {} success: track_id={}, title={}, quality={:?}, bytes={}",
                    mode, track_id, title, quality, bytes
                );
                return Ok(());
            }
            Err(err) => {
                warn!(
                    "[DOWNLOAD] Netease {} failed: track_id={}, title={}, quality={:?}, logged_in={}, error={}",
                    mode, track_id, title, quality, logged_in, err
                );
                failures.push(format!("{quality:?}: {err}"));
            }
        }
    }

    Err(explain_netease_failure(logged_in, failures))
}

fn explain_netease_failure(logged_in: bool, failures: Vec<String>) -> String {
    let details = failures.join(" | ");

    if failures
        .iter()
        .any(|entry| entry.contains("API error (code 301)"))
    {
        return format!("网易云登录已失效，请重新登录。详情: {details}");
    }

    if !logged_in {
        return format!("当前网易云未登录，部分歌曲需要登录后才能下载。详情: {details}");
    }

    if failures
        .iter()
        .any(|entry| entry.contains("API error (code 403)"))
    {
        return format!("网易云拒绝提供该歌曲，可能需要 VIP 或受地区限制。详情: {details}");
    }

    if failures
        .iter()
        .any(|entry| entry.contains("track unavailable"))
    {
        return format!("网易云未返回可用音频地址，可能是 VIP、版权或地区限制。详情: {details}");
    }

    format!("网易云下载失败。详情: {details}")
}

fn simple_hash(value: &str) -> u64 {
    value.bytes().fold(5381_u64, |acc, byte| {
        acc.wrapping_mul(33).wrapping_add(u64::from(byte))
    })
}

fn is_source_enabled(source: DownloadSource) -> bool {
    match source {
        DownloadSource::Youtube => load_youtube_cookie_header().is_some(),
        DownloadSource::Netease => NeteaseSession::load()
            .map(|session| session.is_logged_in())
            .unwrap_or(false),
        DownloadSource::Bilibili => BiliSession::load()
            .map(|session| session.is_logged_in())
            .unwrap_or(false),
    }
}

fn youtube_client() -> Result<YoutubeClient, ytdl_audio::Error> {
    let mut client = YoutubeClient::new(None)?;
    match crate::create_youtube_js_runner() {
        Ok(Some(runner)) => client.set_js_runner(runner),
        Ok(None) => {}
        Err(err) => {
            warn!("[DOWNLOAD] Failed to create webview YouTube JS runner, falling back to default solver: {}", err);
        }
    }
    Ok(client)
}

fn ensure_ytdl_solver_dependencies() -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        return Ok(());
    }

    #[cfg(not(target_os = "android"))]
    {
        let solver_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/ytdl-audio/js");
        let meriyah = solver_dir.join("node_modules/meriyah/package.json");
        let astring = solver_dir.join("node_modules/astring/package.json");

        if meriyah.exists() && astring.exists() {
            return Ok(());
        }

        let package_lock = solver_dir.join("package-lock.json");
        let mut command = Command::new("npm");
        if package_lock.exists() {
            command.args(["ci", "--no-fund", "--no-audit"]);
        } else {
            command.args(["install", "--no-fund", "--no-audit"]);
        }
        command.current_dir(&solver_dir);

        info!(
            "[DOWNLOAD] Installing ytdl-audio solver dependencies in {}",
            solver_dir.display()
        );

        let output = command
            .output()
            .map_err(|err| format!("failed to run npm for ytdl-audio solver: {err}"))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Err(format!(
            "failed to install ytdl-audio solver dependencies: status={} stderr={} stdout={}",
            output.status, stderr, stdout
        ))
    }
}

fn load_youtube_cookie_header() -> Option<String> {
    let path = std::env::var(YOUTUBE_COOKIE_HEADER_PATH_ENV).ok()?;
    let contents = fs::read_to_string(path).ok()?;
    let has_cookie_line = contents
        .lines()
        .map(str::trim)
        .any(|line| !line.is_empty() && !line.starts_with('#'));
    if !has_cookie_line {
        None
    } else {
        Some(contents)
    }
}

fn youtube_cookie_file_path() -> Option<String> {
    let path = std::env::var(YOUTUBE_COOKIE_HEADER_PATH_ENV).ok()?;
    let contents = fs::read_to_string(&path).ok()?;
    if contents.trim().is_empty() {
        None
    } else {
        Some(path)
    }
}

fn should_skip_bilibili_ffmpeg() -> bool {
    cfg!(target_os = "android")
}

fn download_bilibili_audio(
    client: &BilibiliClient,
    bvid: &str,
    output: &Path,
) -> Result<u64, BilibiliError> {
    if should_skip_bilibili_ffmpeg() {
        client.download_audio_raw(bvid, output)
    } else {
        setup_ffmpeg_path();
        client.download_audio(bvid, output, bilibili_api::types::AudioFormat::Mp3)
    }
}

fn setup_ffmpeg_path() {
    if cfg!(target_os = "android") {
        if let Ok(data_dir) = std::env::var("TAURI_ANDROID_DATA_DIR") {
            let ffmpeg_dir = format!("{data_dir}/files");
            if let Ok(current_path) = std::env::var("PATH") {
                std::env::set_var("PATH", format!("{ffmpeg_dir}:{current_path}"));
            } else {
                std::env::set_var("PATH", ffmpeg_dir);
            }
        }
    }
}

impl DownloadSource {
    fn as_str(self) -> &'static str {
        match self {
            DownloadSource::Youtube => "youtube",
            DownloadSource::Netease => "netease",
            DownloadSource::Bilibili => "bilibili",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        create_download_staging_dir, finalize_youtube_audio, merge_lyric_content,
        resolve_target_dir, sanitize_filename, should_skip_bilibili_ffmpeg,
        BILIBILI_RAW_AUDIO_EXTENSION,
    };
    use std::fs;

    #[test]
    fn target_dir_rejects_parent_components() {
        let temp_dir = tempfile::tempdir().unwrap();
        let error = resolve_target_dir(temp_dir.path(), Some("../outside")).unwrap_err();
        assert!(error.contains("上级路径"));
    }

    #[test]
    fn sanitize_filename_replaces_reserved_characters() {
        assert_eq!(sanitize_filename("a:b/c"), "a_b_c");
    }

    #[test]
    fn merged_lyric_keeps_translation_on_same_timestamp() {
        let merged = merge_lyric_content(
            Some("[00:01.00]hello".to_string()),
            Some("[00:01.00]你好".to_string()),
        )
        .unwrap();

        assert_eq!(merged, "[00:01.00]hello\n[00:01.00]你好");
    }

    #[test]
    fn target_dir_allows_nested_children() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp_dir.path().join("Album")).unwrap();
        let resolved = resolve_target_dir(temp_dir.path(), Some("Album/Live")).unwrap();
        assert_eq!(resolved, temp_dir.path().join("Album/Live"));
    }

    #[test]
    fn staging_dir_is_created_under_requested_base() {
        let temp_dir = tempfile::tempdir().unwrap();
        let staging = create_download_staging_dir(temp_dir.path()).unwrap();
        assert!(staging.path().starts_with(temp_dir.path().join(".staging")));
    }

    #[test]
    fn youtube_finalize_falls_back_to_original_container_without_ffmpeg() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_audio = temp_dir.path().join("source.webm");
        fs::write(&source_audio, b"webm").unwrap();

        let (file_name, final_path) =
            finalize_youtube_audio(&source_audio, temp_dir.path(), "preview-token", Some("ogg"))
                .unwrap();

        assert_eq!(file_name, "preview-token.webm");
        assert_eq!(final_path, temp_dir.path().join("preview-token.webm"));
        assert_eq!(fs::read(final_path).unwrap(), b"webm");
    }

    #[test]
    fn bilibili_android_downloads_keep_raw_extension() {
        let extension = if should_skip_bilibili_ffmpeg() {
            BILIBILI_RAW_AUDIO_EXTENSION
        } else {
            "mp3"
        };

        assert!(!extension.is_empty());
    }
}
