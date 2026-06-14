//! HTTP handlers for online music search, preview, lyrics, and download.

use crate::entities::music::Entity as MusicEntity;
use actix_files::NamedFile;
use actix_web::{get, post, web, HttpRequest, HttpResponse};
use futures::future::join_all;
use netease_api::types::SearchType;
use netease_api::{NeteaseClient, NeteaseError};
use sea_orm::EntityTrait;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use tokio::task;
use tracing::{error, warn};

use crate::services::{download as download_service, scanner};
use crate::types::{
    AppState, ApplyLyricRequest, ApplyLyricResponse, DirectoryNode, DownloadPreviewRequest,
    DownloadPreviewResponse, DownloadSource, DownloadTrackRequest, DownloadTrackResponse,
    LyricCandidate, OnlineSearchRequest, OnlineSearchResult, PreviewSong,
};

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

    let mut results = Vec::new();
    for (source, result) in join_all(search_jobs).await {
        match result {
            Ok(mut items) => results.append(&mut items),
            Err(err) => warn!("[DOWNLOAD] {} search failed: {}", source.as_str(), err),
        }
    }

    HttpResponse::Ok().json(results)
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

    let provider = match download_service::provider(body.source) {
        Some(provider) => provider,
        None => {
            return HttpResponse::BadRequest().json(DownloadTrackResponse {
                success: false,
                message: "不支持的下载源".to_string(),
                filename: None,
                lyric_filename: None,
                warning: None,
            });
        }
    };

    let output = match provider.download_full(&body, &target_dir).await {
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

    download_service::try_attach_cover_art_from_url(
        &output.final_path,
        output.cover_url.as_deref(),
    )
    .await;

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

fn join_artists(artists: &[netease_api::types::Artist]) -> String {
    artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::{apply_lyric, merge_lyric_content, resolve_target_dir, ApplyLyricRequest};
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
        crate::services::scanner::initialize_database(&music_dir.to_str().unwrap(), &db_conn)
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
            discovery: discovery_state,
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
    fn target_dir_allows_nested_children() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp_dir.path().join("Album")).unwrap();
        let resolved = resolve_target_dir(temp_dir.path(), Some("Album/Live")).unwrap();
        assert_eq!(resolved, temp_dir.path().join("Album/Live"));
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
}
