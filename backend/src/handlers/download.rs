//! HTTP handlers for online music search, preview, lyrics, and download.

use crate::entities::music::Entity as MusicEntity;
use crate::services::download::MusicProvider;
use actix_files::NamedFile;
use actix_web::{get, http::header, post, web, HttpRequest, HttpResponse};
use download_core::{DownloadProgressPhase, DownloadProgressReporter};
use futures::future::join_all;
use netease_api::types::SearchType;
use netease_api::{NeteaseClient, NeteaseError};
use reqwest::StatusCode;
use sea_orm::EntityTrait;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::task;
use tracing::{error, warn};
use uuid::Uuid;

use crate::services::{download as download_service, scanner};
use crate::types::{
    AppState, ApplyLyricRequest, ApplyLyricResponse, CreateDownloadJobResponse, DirectoryNode,
    DownloadJobListResponse, DownloadPreviewRequest, DownloadPreviewResponse, DownloadSource,
    DownloadTrackRequest, DownloadTrackResponse, LyricCandidate, OnlineSearchRequest,
    OnlineSearchResult, PreviewSong,
};

#[post("/api/download/search")]
pub async fn search_online(body: web::Json<OnlineSearchRequest>, req: HttpRequest) -> HttpResponse {
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
    let enabled_providers = download_service::providers_for_sources(&sources)
        .into_iter()
        .filter(|provider| provider.is_enabled())
        .collect::<Vec<_>>();

    if enabled_providers.is_empty() {
        warn!("[DOWNLOAD] Search skipped because no enabled providers are logged in");
        return HttpResponse::Ok().json(Vec::<OnlineSearchResult>::new());
    }

    let search_jobs = enabled_providers.into_iter().map(|provider| {
        let query = query.clone();
        async move {
            let source = provider.source();
            let result = provider.search(&query, max_results).await;
            (source, result)
        }
    });

    let host = format!(
        "{}://{}",
        req.connection_info().scheme(),
        req.connection_info().host()
    );
    let mut results = Vec::new();
    for (source, result) in join_all(search_jobs).await {
        match result {
            Ok(mut items) => results.append(&mut items),
            Err(err) => warn!("[DOWNLOAD] {} search failed: {}", source.as_str(), err),
        }
    }

    for item in &mut results {
        if item.source == DownloadSource::Bilibili {
            item.thumbnail_url = Some(format!(
                "{host}/api/download/bilibili/thumbnail/{}",
                item.id
            ));
        }
    }

    HttpResponse::Ok().json(results)
}

#[get("/api/download/bilibili/thumbnail/{bvid}")]
pub async fn get_bilibili_thumbnail(path: web::Path<String>) -> HttpResponse {
    let bvid = path.into_inner();
    if bvid.trim().is_empty() || bvid.contains('/') || bvid.contains('\\') {
        return HttpResponse::BadRequest().body("Invalid Bilibili video id");
    }

    let cover_url = match download_service::resolve_bilibili_cover_url(&bvid).await {
        Ok(url) => url,
        Err(err) => {
            warn!(
                "[DOWNLOAD] Failed to resolve Bilibili thumbnail for {}: {}",
                bvid, err
            );
            return HttpResponse::BadGateway().body("Failed to resolve Bilibili thumbnail");
        }
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            error!("[DOWNLOAD] Failed to build thumbnail HTTP client: {}", err);
            return HttpResponse::InternalServerError()
                .body("Failed to initialize thumbnail proxy");
        }
    };

    let response = match client.get(&cover_url).send().await {
        Ok(response) => response,
        Err(err) => {
            warn!(
                "[DOWNLOAD] Failed to fetch Bilibili thumbnail {} for {}: {}",
                cover_url, bvid, err
            );
            return HttpResponse::BadGateway().body("Failed to fetch Bilibili thumbnail");
        }
    };

    if response.status() != StatusCode::OK {
        warn!(
            "[DOWNLOAD] Bilibili thumbnail upstream returned {} for {} ({})",
            response.status(),
            bvid,
            cover_url
        );
        return HttpResponse::BadGateway().body("Bilibili thumbnail upstream rejected request");
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => {
            warn!(
                "[DOWNLOAD] Failed to read Bilibili thumbnail body for {} ({}): {}",
                bvid, cover_url, err
            );
            return HttpResponse::BadGateway().body("Failed to read Bilibili thumbnail");
        }
    };

    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, content_type))
        .insert_header((header::CACHE_CONTROL, "public, max-age=86400"))
        .body(bytes)
}

#[get("/api/download/providers")]
pub async fn get_online_provider_statuses() -> HttpResponse {
    HttpResponse::Ok().json(download_service::build_online_provider_statuses())
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

#[post("/api/download/lyrics/apply")]
pub async fn apply_lyric(
    body: web::Json<ApplyLyricRequest>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let lyric_selection = body.lyric_selection.trim().to_string();
    if lyric_selection.is_empty() {
        return HttpResponse::BadRequest().json(ApplyLyricResponse {
            success: false,
            message: "缺少歌词选择".to_string(),
            lyric_filename: None,
        });
    }

    let music = match MusicEntity::find_by_id(body.song_id)
        .one(&data.db_conn)
        .await
    {
        Ok(Some(music)) => music,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApplyLyricResponse {
                success: false,
                message: "歌曲不存在".to_string(),
                lyric_filename: None,
            });
        }
        Err(err) => {
            error!(
                "[DOWNLOAD] Failed to load music for lyric apply: song_id={}, error={}",
                body.song_id, err
            );
            return HttpResponse::InternalServerError().json(ApplyLyricResponse {
                success: false,
                message: "读取歌曲信息失败".to_string(),
                lyric_filename: None,
            });
        }
    };

    match write_selected_lyric(&lyric_selection, Path::new(&music.file_path)).await {
        Ok(Some(path)) => HttpResponse::Ok().json(ApplyLyricResponse {
            success: true,
            message: "歌词已保存".to_string(),
            lyric_filename: path
                .file_name()
                .map(|value| value.to_string_lossy().to_string()),
        }),
        Ok(None) => HttpResponse::NotFound().json(ApplyLyricResponse {
            success: false,
            message: "未获取到可用歌词".to_string(),
            lyric_filename: None,
        }),
        Err(err) if err == "无效的歌词歌曲 ID" => {
            HttpResponse::BadRequest().json(ApplyLyricResponse {
                success: false,
                message: err,
                lyric_filename: None,
            })
        }
        Err(err) => {
            warn!(
                "[DOWNLOAD] Failed to apply lyric to song_id={}: {}",
                body.song_id, err
            );
            HttpResponse::BadGateway().json(ApplyLyricResponse {
                success: false,
                message: format!("歌词下载失败: {err}"),
                lyric_filename: None,
            })
        }
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
    let provider = match download_service::provider(preview_request.source) {
        Some(provider) => provider,
        None => {
            return HttpResponse::BadRequest().json(DownloadPreviewResponse {
                success: false,
                message: "不支持的下载源".to_string(),
                song: None,
            });
        }
    };
    let host = format!(
        "{}://{}",
        req.connection_info().scheme(),
        req.connection_info().host()
    );

    let preview_result = match provider
        .build_preview(&preview_request, &preview_root)
        .await
    {
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
    match prepare_download_request(body.into_inner(), &data) {
        Ok((request, target_dir, provider)) => {
            match execute_download_request(request, target_dir, provider, data, None).await {
                Ok(response) => HttpResponse::Ok().json(response),
                Err(message) => HttpResponse::BadGateway().json(DownloadTrackResponse {
                    success: false,
                    message: format!("下载失败: {message}"),
                    filename: None,
                    lyric_filename: None,
                    warning: None,
                }),
            }
        }
        Err(response) => HttpResponse::BadRequest().json(response),
    }
}

#[post("/api/download/jobs")]
pub async fn create_download_job(
    body: web::Json<DownloadTrackRequest>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let prepared = match prepare_download_request(body.into_inner(), &data) {
        Ok(prepared) => prepared,
        Err(response) => {
            return HttpResponse::BadRequest().json(CreateDownloadJobResponse {
                success: false,
                message: response.message,
                job_id: None,
            });
        }
    };

    let (request, target_dir, provider) = prepared;
    let job_id = Uuid::new_v4().to_string();
    data.download_jobs
        .create(&job_id, request.source, &request.title)
        .await;

    let data_clone = data.clone();
    let spawned_job_id = job_id.clone();
    tokio::spawn(async move {
        let source = request.source;
        if let Err(message) = execute_download_request(
            request,
            target_dir,
            provider,
            data_clone.clone(),
            Some(spawned_job_id.clone()),
        )
        .await
        {
            data_clone
                .download_jobs
                .mark_failed(&spawned_job_id, source, format!("下载失败: {message}"))
                .await;
        }
    });

    HttpResponse::Ok().json(CreateDownloadJobResponse {
        success: true,
        message: "下载任务已创建".to_string(),
        job_id: Some(job_id),
    })
}

#[get("/api/download/jobs")]
pub async fn get_download_jobs(data: web::Data<AppState>) -> HttpResponse {
    let jobs = data.download_jobs.active_jobs().await;
    HttpResponse::Ok().json(DownloadJobListResponse { jobs })
}

#[get("/api/download/jobs/{job_id}")]
pub async fn get_download_job(path: web::Path<String>, data: web::Data<AppState>) -> HttpResponse {
    let job_id = path.into_inner();
    match data.download_jobs.get(&job_id).await {
        Some(snapshot) => HttpResponse::Ok().json(snapshot),
        None => HttpResponse::NotFound().finish(),
    }
}

fn prepare_download_request(
    mut body: DownloadTrackRequest,
    data: &web::Data<AppState>,
) -> Result<(DownloadTrackRequest, PathBuf, &'static dyn MusicProvider), DownloadTrackResponse> {
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
            return Err(rejected_download_response(message));
        }
    };

    body.file_name = match download_service::resolve_download_file_stem(
        body.file_name.as_deref(),
        &body.title,
    ) {
        Ok(file_stem) => Some(file_stem),
        Err(message) => {
            warn!(
                "[DOWNLOAD] Rejecting track download request: source={}, id={}, requested_file_name={:?}, reason={}",
                body.source.as_str(),
                body.id,
                body.file_name,
                message
            );
            return Err(rejected_download_response(message));
        }
    };

    if let Err(err) = fs::create_dir_all(&target_dir) {
        error!("[DOWNLOAD] Failed to create target download dir: {}", err);
        return Err(rejected_download_response("无法创建目标目录".to_string()));
    }

    let provider = download_service::provider(body.source)
        .ok_or_else(|| rejected_download_response("不支持的下载源".to_string()))?;

    Ok((body, target_dir, provider))
}

async fn execute_download_request(
    body: DownloadTrackRequest,
    target_dir: PathBuf,
    provider: &'static dyn MusicProvider,
    data: web::Data<AppState>,
    job_id: Option<String>,
) -> Result<DownloadTrackResponse, String> {
    let reporter: Option<Arc<dyn DownloadProgressReporter>> = job_id.as_ref().map(|_| {
        let reporter: Arc<dyn DownloadProgressReporter> = Arc::new(
            download_service::JobProgressReporter::new(data.download_jobs.clone()),
        );
        reporter
    });
    let output = match (&job_id, &reporter) {
        (Some(job_id), Some(reporter)) => {
            provider
                .download_full_with_progress(&body, &target_dir, job_id, reporter.clone())
                .await
        }
        _ => provider.download_full(&body, &target_dir).await,
    }
    .map_err(|err| {
        warn!("[DOWNLOAD] Full download failed: {}", err);
        err
    })?;

    // Resolve lyric text (Netease) before finalizing so the shared core can
    // write the sidecar uniformly for both online downloads and remote-library
    // imports.
    let mut lyric_warning = None;
    let lyric = if let Some(lyric_id) = body.lyric_selection.as_deref() {
        match fetch_merged_lyric_text(lyric_id).await {
            Ok(Some(text)) => Some(LyricSidecar::Text(text)),
            Ok(None) => {
                lyric_warning = Some("未获取到可用歌词".to_string());
                None
            }
            Err(err) => {
                lyric_warning = Some(format!("歌词下载失败: {err}"));
                None
            }
        }
    } else {
        None
    };

    let (warning, lyric_filename) = finalize_downloaded_audio(
        &output.final_path,
        output.cover_url.as_deref(),
        lyric,
        job_id.as_deref(),
        body.source,
        &data,
        lyric_warning,
    )
    .await;

    let filename = output
        .final_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string());

    Ok(DownloadTrackResponse {
        success: true,
        message: "下载完成".to_string(),
        filename,
        lyric_filename,
        warning,
    })
}

fn rejected_download_response(message: String) -> DownloadTrackResponse {
    DownloadTrackResponse {
        success: false,
        message,
        filename: None,
        lyric_filename: None,
        warning: None,
    }
}

fn append_warning(current: &mut Option<String>, warning: String) {
    match current {
        Some(existing) => {
            existing.push('；');
            existing.push_str(&warning);
        }
        None => *current = Some(warning),
    }
}

/// Lyric content to write as a sidecar during finalization.
#[derive(Debug, Clone)]
pub(crate) enum LyricSidecar {
    /// Raw LRC or WEBVTT text. The extension is sniffed from the content.
    Text(String),
}

/// Choose the sidecar extension for lyric text.
///
/// The remote lyrics endpoint always returns `text/plain`, so the format must
/// be sniffed from the body: a `WEBVTT` header (after any BOM/whitespace) →
/// `.vtt`, otherwise `.lrc`.
pub(crate) fn lyric_sidecar_extension(text: &str) -> &'static str {
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    if trimmed.starts_with("WEBVTT") {
        "vtt"
    } else {
        "lrc"
    }
}

/// Shared finalization for any freshly downloaded audio file.
///
/// Used by both the online-provider download path (`execute_download_request`)
/// and the remote-library import path (`handlers::library_import`). Performs,
/// as applicable: cover-art embedding, lyric sidecar creation, library database
/// refresh, and job completion. Returns `(warning, lyric_filename)`.
///
/// Related documentation: `docs/library-import.md`
pub(crate) async fn finalize_downloaded_audio(
    audio_path: &Path,
    cover_url: Option<&str>,
    lyric: Option<LyricSidecar>,
    job_id: Option<&str>,
    source: DownloadSource,
    data: &web::Data<AppState>,
    extra_warning: Option<String>,
) -> (Option<String>, Option<String>) {
    let mut warning = extra_warning;
    let mut lyric_filename = None;

    if cover_url.is_some() {
        if let Some(job_id) = job_id {
            data.download_jobs
                .update_phase(
                    job_id,
                    source,
                    DownloadProgressPhase::EmbeddingCover,
                    "Embedding cover art",
                    None,
                )
                .await;
        }
        if let Some(cover_url) = cover_url {
            if let Err(err) =
                download_service::attach_cover_art_from_url(audio_path, cover_url).await
            {
                warn!(
                    "[DOWNLOAD] Failed to attach cover art to {}: {}",
                    audio_path.display(),
                    err
                );
                append_warning(&mut warning, format!("封面写入失败: {err}"));
            }
        }
    }

    if let Some(LyricSidecar::Text(text)) = lyric.as_ref() {
        if let Some(job_id) = job_id {
            data.download_jobs
                .update_phase(
                    job_id,
                    source,
                    DownloadProgressPhase::SavingLyrics,
                    "Saving lyrics",
                    None,
                )
                .await;
        }
        let ext = lyric_sidecar_extension(text);
        let lyric_path = audio_path.with_extension(ext);
        match fs::write(&lyric_path, text) {
            Ok(()) => {
                lyric_filename = lyric_path
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string());
            }
            Err(err) => {
                warn!(
                    "[DOWNLOAD] Failed to write lyric sidecar {}: {}",
                    lyric_path.display(),
                    err
                );
                append_warning(&mut warning, format!("歌词保存失败: {err}"));
            }
        }
    }

    if let Some(job_id) = job_id {
        data.download_jobs
            .update_phase(
                job_id,
                source,
                DownloadProgressPhase::RefreshingLibrary,
                "Refreshing library",
                None,
            )
            .await;
    }
    if let Err(err) = scanner::update_database(&data.db_conn, &data.scan_registry).await {
        warn!("[DOWNLOAD] Database update after download failed: {}", err);
        append_warning(&mut warning, format!("刷新曲库失败: {err}"));
    }

    let filename = audio_path
        .file_name()
        .map(|value| value.to_string_lossy().to_string());
    if let Some(job_id) = job_id {
        data.download_jobs
            .mark_completed(job_id, filename, warning.clone())
            .await;
    }

    (warning, lyric_filename)
}

/// Fetch and merge Netease lyric text for a track id without writing a sidecar.
///
/// Returns `Ok(None)` when the provider has no usable lyric content; the caller
/// decides whether that warrants a warning. Used by both the online-download
/// finalization path and (indirectly) `apply_lyric`.
async fn fetch_merged_lyric_text(lyric_id: &str) -> Result<Option<String>, String> {
    let track_id = lyric_id
        .parse::<u64>()
        .map_err(|_| "无效的歌词歌曲 ID".to_string())?;

    task::spawn_blocking(move || -> Result<Option<String>, String> {
        let client = NeteaseClient::new().map_err(|e| e.to_string())?;
        let lyric = client.track_lyric(track_id).map_err(|e| match e {
            NeteaseError::Other(message) => message,
            other => other.to_string(),
        })?;

        Ok(merge_lyric_content(lyric.lrc, lyric.tlyric))
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn write_selected_lyric(
    lyric_id: &str,
    audio_path: &Path,
) -> Result<Option<PathBuf>, String> {
    let Some(content) = fetch_merged_lyric_text(lyric_id).await? else {
        return Ok(None);
    };

    let lyric_path = audio_path.to_path_buf().with_extension("lrc");
    fs::write(&lyric_path, &content).map_err(|e| e.to_string())?;
    Ok(Some(lyric_path))
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
    let stamp = line.get(..=end_index)?;
    let text = line.get(end_index.checked_add(1)?..)?;
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

pub(crate) fn resolve_target_dir(
    base_dir: &Path,
    target_subdir: Option<&str>,
) -> Result<PathBuf, String> {
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
        fs::canonicalize(&requested_path).unwrap_or_else(|_| requested_path.clone());
    if !requested_canonical.starts_with(&base_canonical) {
        return Err("目标目录必须位于下载目录内".to_string());
    }

    Ok(requested_path)
}

fn join_artists(artists: &[netease_api::types::Artist]) -> String {
    artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::{
        apply_lyric, create_download_job, download_track, get_download_jobs,
        lyric_sidecar_extension, merge_lyric_content, resolve_target_dir, ApplyLyricRequest,
    };
    use crate::types::AppState;
    use actix_web::{test as actix_test, web, App};
    use std::fs;
    use std::sync::Arc;

    async fn create_test_setup() -> (tempfile::TempDir, web::Data<AppState>) {
        let temp_dir = tempfile::tempdir().unwrap();
        let music_dir = temp_dir.path();

        let audio_path = music_dir.join("test-song.mp3");
        std::fs::write(&audio_path, b"fake audio content").unwrap();

        let db_conn = crate::database::establish_connection(music_dir.to_str().unwrap())
            .await
            .unwrap();
        let scan_registry = std::sync::Arc::new(crate::file_ops::ScanRegistry::new());
        scan_registry.register(std::sync::Arc::new(crate::file_ops::StdFsScanBackend::new(
            std::path::PathBuf::from(&music_dir),
        )));
        crate::services::scanner::initialize_database(&db_conn, &scan_registry)
            .await
            .unwrap();

        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        let app_state = web::Data::new(AppState {
            music_path: Arc::new(music_dir.to_str().unwrap().to_string()),
            download_root: Arc::new(music_dir.to_str().unwrap().to_string()),
            preview_root: Arc::new(music_dir.join(".preview").to_string_lossy().to_string()),
            db_conn,
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(crate::services::download::DownloadJobStore::new()),
            discovery: discovery_state,
            scan_registry,
        });

        (temp_dir, app_state)
    }

    #[test]
    fn target_dir_rejects_parent_components() {
        let temp_dir = tempfile::tempdir().unwrap();
        let error = resolve_target_dir(temp_dir.path(), Some("../outside")).unwrap_err();
        assert!(error.contains("上级路径"));
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
    fn lyric_sidecar_extension_sniffs_format_from_content() {
        assert_eq!(lyric_sidecar_extension("[00:01.00]hi"), "lrc");
        assert_eq!(lyric_sidecar_extension("WEBVTT\n\n00:00:01.000 -->"), "vtt");
        // Leading BOM and whitespace must not defeat the WEBVTT sniff.
        assert_eq!(
            lyric_sidecar_extension("\u{feff}  WEBVTT\n\n00:00:01.000"),
            "vtt"
        );
    }

    #[test]
    fn target_dir_allows_nested_children() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp_dir.path().join("Album")).unwrap();
        let resolved = resolve_target_dir(temp_dir.path(), Some("Album/Live")).unwrap();
        assert_eq!(resolved, temp_dir.path().join("Album/Live"));
    }

    #[actix_web::test]
    async fn download_jobs_endpoint_starts_empty() {
        let (_temp_dir, app_state) = create_test_setup().await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(get_download_jobs))
                .await;
        let req = actix_test::TestRequest::get()
            .uri("/api/download/jobs")
            .to_request();

        let resp = actix_test::call_service(&app, req).await;
        let body: serde_json::Value = actix_test::read_body_json(resp).await;

        assert_eq!(body["jobs"].as_array().map(Vec::len), Some(0));
    }

    #[actix_web::test]
    async fn test_apply_lyric_returns_not_found_for_missing_song() {
        let (_temp_dir, app_state) = create_test_setup().await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(apply_lyric)).await;
        let req = actix_test::TestRequest::post()
            .uri("/api/download/lyrics/apply")
            .set_json(&ApplyLyricRequest {
                song_id: 999_999,
                lyric_selection: "123".to_string(),
            })
            .to_request();

        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 404);
    }

    #[actix_web::test]
    async fn test_apply_lyric_rejects_invalid_lyric_id() {
        let (_temp_dir, app_state) = create_test_setup().await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(apply_lyric)).await;
        let req = actix_test::TestRequest::post()
            .uri("/api/download/lyrics/apply")
            .set_json(&ApplyLyricRequest {
                song_id: 1,
                lyric_selection: "invalid".to_string(),
            })
            .to_request();

        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_download_track_rejects_blank_custom_filename() {
        let (_temp_dir, app_state) = create_test_setup().await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(download_track)).await;
        let req = actix_test::TestRequest::post()
            .uri("/api/download/track")
            .set_json(&crate::types::DownloadTrackRequest {
                source: crate::types::DownloadSource::Netease,
                id: "123".to_string(),
                title: "Song".to_string(),
                artist: Some("Artist".to_string()),
                file_name: Some("   ".to_string()),
                target_subdir: None,
                lyric_selection: None,
            })
            .to_request();

        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn create_download_job_rejects_blank_custom_filename() {
        let (_temp_dir, app_state) = create_test_setup().await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(create_download_job))
                .await;
        let req = actix_test::TestRequest::post()
            .uri("/api/download/jobs")
            .set_json(&crate::types::DownloadTrackRequest {
                source: crate::types::DownloadSource::Netease,
                id: "123".to_string(),
                title: "Song".to_string(),
                artist: Some("Artist".to_string()),
                file_name: Some("   ".to_string()),
                target_subdir: None,
                lyric_selection: None,
            })
            .to_request();

        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn test_download_track_rejects_path_custom_filename() {
        let (_temp_dir, app_state) = create_test_setup().await;

        let app =
            actix_test::init_service(App::new().app_data(app_state).service(download_track)).await;
        let req = actix_test::TestRequest::post()
            .uri("/api/download/track")
            .set_json(&crate::types::DownloadTrackRequest {
                source: crate::types::DownloadSource::Netease,
                id: "123".to_string(),
                title: "Song".to_string(),
                artist: Some("Artist".to_string()),
                file_name: Some("../Song".to_string()),
                target_subdir: None,
                lyric_selection: None,
            })
            .to_request();

        let resp = actix_test::call_service(&app, req).await;

        assert_eq!(resp.status().as_u16(), 400);
    }
}
