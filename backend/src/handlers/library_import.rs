//! Library import API handler.
//!
//! Pulls tracks (audio + lyrics sidecar) from a remote Kaulan server into the
//! local `download_root`, then refreshes the local library database so the
//! imported songs appear in the local source. The heavy lifting is a
//! server-to-server HTTP fetch performed by the local backend, which keeps the
//! behavior identical across the desktop and Android Tauri shells.
//!
//! The browser runtime does NOT call this endpoint: in a plain browser the
//! frontend downloads each file directly via the browser. The runtime split is
//! documented in `docs/library-import.md`.
//!
//! Related documentation:
//! - `docs/library-import.md`

use crate::file_ops::{
    is_std_fs_path, source_exists, source_remove_file, source_write_file, SUPPORTED_EXTENSIONS,
};
use crate::handlers::download::{lyric_sidecar_extension, resolve_target_dir};
use crate::services::download::sanitize_filename;
use crate::services::scanner;
use crate::types::{
    AppState, CreateDownloadJobResponse, DownloadSource, ImportFromRemoteRequest, ImportRemoteItem,
};
use actix_web::{post, web, HttpResponse};
use download_core::DownloadProgressPhase;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Per-item HTTP timeout. Music files can be large, so this is generous.
const IMPORT_HTTP_TIMEOUT: Duration = Duration::from_secs(300);
/// Soft cap on a single buffered file to avoid unbounded memory use. Files are
/// processed sequentially, so peak memory is bounded by the largest single item.
const IMPORT_MAX_BYTES_PER_ITEM: usize = 512 * 1024 * 1024;

/// `POST /api/library/import-from-remote`
///
/// Creates an asynchronous import job (tracked via the shared `DownloadJobStore`,
/// pollable through `GET /api/download/jobs/{id}`) and returns its id immediately.
#[post("/api/library/import-from-remote")]
pub async fn import_from_remote(
    body: web::Json<ImportFromRemoteRequest>,
    data: web::Data<AppState>,
) -> HttpResponse {
    let request = body.into_inner();

    let remote_base = match validate_remote_api_base(&request.remote_api_base) {
        Ok(base) => base,
        Err(message) => {
            return HttpResponse::BadRequest().json(CreateDownloadJobResponse {
                success: false,
                message,
                job_id: None,
            });
        }
    };

    if request.items.is_empty() {
        return HttpResponse::BadRequest().json(CreateDownloadJobResponse {
            success: false,
            message: "未选择要导入的歌曲".to_string(),
            job_id: None,
        });
    }

    let target_dir = match resolve_target_dir(
        Path::new(data.download_root.as_ref()),
        request.target_subdir.as_deref(),
    ) {
        Ok(path) => path,
        Err(message) => {
            warn!(
                "[IMPORT] Rejecting import request: remote={}, reason={}",
                remote_base, message
            );
            return HttpResponse::BadRequest().json(CreateDownloadJobResponse {
                success: false,
                message,
                job_id: None,
            });
        }
    };

    // Defensive: ensure the resolved target is a writable StdFs path. Android
    // `content://` targets are read-only; mirrors the upload handler guard.
    if !is_std_fs_path(target_dir.to_string_lossy().as_ref()) {
        error!(
            "[IMPORT] Target download dir is not a writable filesystem path: {}",
            target_dir.display()
        );
        return HttpResponse::InternalServerError().json(CreateDownloadJobResponse {
            success: false,
            message: "本机下载目录不可写".to_string(),
            job_id: None,
        });
    }

    let total = request.items.len();
    let summary = format!("从远端导入 {} 首歌曲", total);
    let job_id = Uuid::new_v4().to_string();
    data.download_jobs
        .create(&job_id, DownloadSource::Import, &summary)
        .await;

    info!(
        "[IMPORT] job={} remote={} items={} target={}",
        job_id,
        remote_base,
        total,
        target_dir.display()
    );

    let data_clone = data.clone();
    let spawned_job_id = job_id.clone();
    tokio::spawn(async move {
        if let Err(err) = run_import_job(
            data_clone.clone(),
            &spawned_job_id,
            remote_base,
            target_dir,
            request,
        )
        .await
        {
            error!("[IMPORT] job={} aborted: {}", spawned_job_id, err);
            data_clone
                .download_jobs
                .mark_failed(&spawned_job_id, DownloadSource::Import, err)
                .await;
        }
    });

    HttpResponse::Ok().json(CreateDownloadJobResponse {
        success: true,
        message: "导入任务已创建".to_string(),
        job_id: Some(job_id),
    })
}

/// Outcome of importing a single item.
enum ImportOutcome {
    /// Audio (and lyrics, if available) written successfully.
    Imported,
    /// Nothing written because the target already existed.
    Skipped(String),
}

/// Process every item in the request under one job, then refresh the library.
async fn run_import_job(
    data: web::Data<AppState>,
    job_id: &str,
    remote_base: String,
    target_dir: PathBuf,
    request: ImportFromRemoteRequest,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(IMPORT_HTTP_TIMEOUT)
        .build()
        .map_err(|e| format!("无法创建 HTTP 客户端: {e}"))?;

    let include_lyrics = request.include_lyrics.unwrap_or(true);
    let total = request.items.len();
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    for (index, item) in request.items.iter().enumerate() {
        let percent = (index * 100) / total.max(1);
        let label = display_label(item);

        data.download_jobs
            .update_phase(
                job_id,
                DownloadSource::Import,
                DownloadProgressPhase::Downloading,
                format!("正在导入 {}/{total}: {label}", index + 1),
                Some(format!("{percent}%")),
            )
            .await;

        match import_one(&client, &remote_base, item, &target_dir, include_lyrics).await {
            Ok(ImportOutcome::Imported) => imported += 1,
            Ok(ImportOutcome::Skipped(reason)) => {
                skipped += 1;
                warn!(
                    "[IMPORT] job={} skipped id={}: {}",
                    job_id, item.music_id, reason
                );
                warnings.push(format!("{label}: {reason}"));
            }
            Err(err) => {
                failed += 1;
                warn!(
                    "[IMPORT] job={} failed id={}: {}",
                    job_id, item.music_id, err
                );
                warnings.push(format!("{label}: {err}"));
            }
        }
    }

    info!(
        "[IMPORT] job={} finished importing: imported={}, skipped={}, failed={}",
        job_id, imported, skipped, failed
    );

    data.download_jobs
        .update_phase(
            job_id,
            DownloadSource::Import,
            DownloadProgressPhase::RefreshingLibrary,
            "正在刷新曲库".to_string(),
            None,
        )
        .await;

    let library_roots = [
        data.music_path.as_ref().as_str(),
        data.download_root.as_ref().as_str(),
    ];
    if let Err(err) = scanner::update_database_with_roots(&library_roots, &data.db_conn).await {
        warn!("[IMPORT] job={} library refresh failed: {}", job_id, err);
        warnings.push(format!("刷新曲库失败: {err}"));
    }
    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("；"))
    };

    if imported > 0 {
        data.download_jobs
            .mark_completed(job_id, None, warning)
            .await;
    } else if skipped > 0 {
        // Nothing new, but not a failure: every selected song was already local.
        let note = warning.unwrap_or_else(|| "所选歌曲已全部存在".to_string());
        data.download_jobs
            .mark_completed(job_id, None, Some(note))
            .await;
    } else {
        let message = warning.unwrap_or_else(|| "未导入任何歌曲".to_string());
        data.download_jobs
            .mark_failed(job_id, DownloadSource::Import, message)
            .await;
    }

    Ok(())
}

/// Import a single track: fetch audio, derive a local filename, skip if it
/// already exists, write it, and (best-effort) write the lyrics sidecar.
async fn import_one(
    client: &reqwest::Client,
    remote_base: &str,
    item: &ImportRemoteItem,
    target_dir: &Path,
    include_lyrics: bool,
) -> Result<ImportOutcome, String> {
    let (stem, file_ext) = parse_filename_parts(item.filename.as_deref(), item.music_id);
    let known_ext = !file_ext.is_empty() && SUPPORTED_EXTENSIONS.contains(&file_ext.as_str());

    // Idempotent skip when the final filename is already known.
    if known_ext {
        let candidate = target_dir.join(format!("{stem}.{file_ext}"));
        if source_exists(&candidate.to_string_lossy())
            .await
            .unwrap_or(false)
        {
            return Ok(ImportOutcome::Skipped("已存在，跳过".to_string()));
        }
    }

    let audio_url = format!("{remote_base}/music/id/{}", item.music_id);
    let response = client
        .get(&audio_url)
        .send()
        .await
        .map_err(|e| format!("请求远端音频失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("远端音频返回 {}", response.status()));
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = read_bounded_bytes(response, IMPORT_MAX_BYTES_PER_ITEM)
        .await
        .map_err(|e| format!("读取远端音频失败: {e}"))?;

    let final_ext = if known_ext {
        file_ext
    } else {
        content_type_to_extension(&content_type).to_string()
    };
    let final_path = target_dir.join(format!("{stem}.{final_ext}"));

    // Re-check existence for the unknown-extension path so we never overwrite.
    if !known_ext
        && source_exists(&final_path.to_string_lossy())
            .await
            .unwrap_or(false)
    {
        return Ok(ImportOutcome::Skipped("已存在，跳过".to_string()));
    }

    let final_str = final_path.to_string_lossy().to_string();
    if let Err(e) = source_write_file(&final_str, &bytes).await {
        let _ = source_remove_file(&final_str).await;
        return Err(format!("写入本机失败: {e}"));
    }

    // Lyrics are best-effort: a missing sidecar (404) or write failure must not
    // fail the whole import.
    if include_lyrics {
        write_lyric_sidecar(client, remote_base, item.music_id, &final_path).await;
    }

    Ok(ImportOutcome::Imported)
}

/// Fetch the remote lyrics sidecar and write it next to the audio file.
async fn write_lyric_sidecar(
    client: &reqwest::Client,
    remote_base: &str,
    music_id: i32,
    audio_path: &Path,
) {
    let lyrics_url = format!("{remote_base}/lyrics/id/{}", music_id);
    let response = match client.get(&lyrics_url).send().await {
        Ok(response) => response,
        Err(e) => {
            warn!("[IMPORT] lyric fetch failed for id={}: {}", music_id, e);
            return;
        }
    };
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return;
    }
    if !response.status().is_success() {
        warn!(
            "[IMPORT] remote lyrics returned {} for id={}",
            response.status(),
            music_id
        );
        return;
    }
    let text = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            warn!("[IMPORT] lyric body read failed for id={}: {}", music_id, e);
            return;
        }
    };
    if text.trim().is_empty() {
        return;
    }
    let ext = lyric_sidecar_extension(&text);
    let lyric_path = audio_path.with_extension(ext);
    if let Err(e) = source_write_file(&lyric_path.to_string_lossy(), text.as_bytes()).await {
        warn!(
            "[IMPORT] lyric sidecar write failed for {}: {}",
            lyric_path.display(),
            e
        );
    }
}

/// Read the response body, rejecting items that exceed the size cap.
async fn read_bounded_bytes(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, String> {
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取响应失败: {e}"))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "文件过大（{} 字节，上限 {}）",
            bytes.len(),
            max_bytes
        ));
    }
    Ok(bytes.to_vec())
}

/// Validate and normalize the remote API base URL.
fn validate_remote_api_base(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("缺少远端服务器地址".to_string());
    }
    let url = reqwest::Url::parse(trimmed).map_err(|_| "远端服务器地址无效".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("远端服务器地址必须是 http 或 https".to_string());
    }
    if url.host_str().map(str::is_empty).unwrap_or(true) {
        return Err("远端服务器地址缺少主机名".to_string());
    }
    let mut base = url.to_string();
    while base.ends_with('/') {
        base.pop();
    }
    Ok(base)
}

/// Map an audio `Content-Type` to a file extension (lowercase, no dot).
fn content_type_to_extension(content_type: &str) -> &'static str {
    let lower = content_type.to_ascii_lowercase();
    let mime = lower.split(';').next().unwrap_or("").trim();
    match mime {
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/ogg" => "ogg",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "m4a",
        "audio/aac" => "aac",
        "audio/opus" => "opus",
        "audio/webm" | "audio/x-matroska" | "audio/x-mka" => "mka",
        _ => "mp3",
    }
}

/// Derive `(stem, extension)` for the local file from the supplied filename.
///
/// The stem is sanitized via the shared `sanitize_filename`; the extension is
/// lowercased but not yet validated against `SUPPORTED_EXTENSIONS` (the caller
/// decides whether to trust it or fall back to the content type).
fn parse_filename_parts(filename: Option<&str>, music_id: i32) -> (String, String) {
    let (raw_stem, ext): (String, String) = filename
        .and_then(|name| {
            let path = Path::new(name);
            let stem = path.file_stem()?.to_string_lossy().to_string();
            let extension = path.extension()?.to_string_lossy().to_lowercase();
            Some((stem, extension))
        })
        .unwrap_or_default();

    let stem = if raw_stem.trim().is_empty() {
        format!("remote-{music_id}")
    } else {
        let sanitized = sanitize_filename(&raw_stem);
        if sanitized.is_empty() {
            format!("remote-{music_id}")
        } else {
            sanitized
        }
    };

    (stem, ext)
}

/// Human-readable label for an item, used in progress messages and warnings.
fn display_label(item: &ImportRemoteItem) -> String {
    item.filename
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("remote-{}", item.music_id))
}

#[cfg(test)]
mod tests {
    use super::{
        content_type_to_extension, import_from_remote, parse_filename_parts,
        validate_remote_api_base, IMPORT_MAX_BYTES_PER_ITEM,
    };
    use crate::services::download::DownloadJobStore;
    use crate::types::{AppState, CreateDownloadJobResponse, DownloadJobSnapshot};
    use actix_web::{test as actix_test, web, App};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn validate_remote_base_accepts_http_urls() {
        assert_eq!(
            validate_remote_api_base("http://192.168.1.10:2080/api").unwrap(),
            "http://192.168.1.10:2080/api"
        );
        assert_eq!(
            validate_remote_api_base("https://example.local/api/").unwrap(),
            "https://example.local/api"
        );
    }

    #[test]
    fn validate_remote_base_rejects_non_http_empty_and_garbage() {
        assert!(validate_remote_api_base("ftp://example.local").is_err());
        assert!(validate_remote_api_base("not-a-url").is_err());
        assert!(validate_remote_api_base("   ").is_err());
    }

    #[test]
    fn content_type_maps_common_audio_types() {
        assert_eq!(content_type_to_extension("audio/mpeg"), "mp3");
        assert_eq!(content_type_to_extension("audio/flac"), "flac");
        assert_eq!(content_type_to_extension("audio/mp4"), "m4a");
        assert_eq!(content_type_to_extension("audio/ogg; codecs=opus"), "ogg");
        assert_eq!(content_type_to_extension("application/octet-stream"), "mp3");
    }

    #[test]
    fn parse_filename_parts_uses_supplied_stem_and_extension() {
        let (stem, ext) = parse_filename_parts(Some("Artist - Track.mp3"), 7);
        assert_eq!(stem, "Artist - Track");
        assert_eq!(ext, "mp3");
    }

    #[test]
    fn parse_filename_parts_falls_back_when_missing() {
        let (stem, ext) = parse_filename_parts(None, 42);
        assert_eq!(stem, "remote-42");
        assert_eq!(ext, "");
    }

    #[test]
    fn parse_filename_parts_sanitizes_reserved_characters() {
        let (stem, _) = parse_filename_parts(Some("bad/name:.mp3"), 1);
        assert!(!stem.contains('/'));
        assert!(!stem.contains(':'));
    }

    #[test]
    fn max_bytes_per_item_is_half_gigibyte() {
        assert_eq!(IMPORT_MAX_BYTES_PER_ITEM, 512 * 1024 * 1024);
    }

    async fn make_app_state(download_root: &std::path::Path) -> web::Data<AppState> {
        let db_conn =
            crate::database::establish_connection(download_root.to_str().expect("utf-8 temp path"))
                .await
                .expect("db connection");
        let discovery_state = Arc::new(crate::discovery::types::DiscoveryState::new(
            "test-id".to_string(),
            "Test Player".to_string(),
            2080,
        ));
        web::Data::new(AppState {
            music_path: Arc::new(download_root.to_string_lossy().to_string()),
            download_root: Arc::new(download_root.to_string_lossy().to_string()),
            preview_root: Arc::new(download_root.join(".preview").to_string_lossy().to_string()),
            db_conn,
            scan_lock: Arc::new(tokio::sync::Mutex::new(())),
            download_jobs: Arc::new(DownloadJobStore::new()),
            discovery: discovery_state,
        })
    }

    /// Poll the shared job store until the job reaches a terminal state.
    async fn wait_for_job(jobs: &Arc<DownloadJobStore>, job_id: &str) -> DownloadJobSnapshot {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(snapshot) = jobs.get(job_id).await {
                if snapshot.state == "completed" || snapshot.state == "failed" {
                    return snapshot;
                }
            }
            if Instant::now() > deadline {
                panic!("import job {job_id} did not finish in time");
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    fn post_import(
        remote_api_base: &str,
        items: &[serde_json::Value],
        include_lyrics: bool,
    ) -> actix_test::TestRequest {
        actix_test::TestRequest::post()
            .uri("/api/library/import-from-remote")
            .set_json(serde_json::json!({
                "remote_api_base": remote_api_base,
                "items": items,
                "include_lyrics": include_lyrics,
            }))
    }

    #[actix_web::test]
    async fn import_rejects_empty_items_with_http_400() {
        let temp = tempfile::tempdir().unwrap();
        let app_state = make_app_state(temp.path()).await;
        let app = actix_test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(import_from_remote),
        )
        .await;

        let req = post_import("http://127.0.0.1:1/api", &[], true).to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
        let body: CreateDownloadJobResponse = actix_test::read_body_json(resp).await;
        assert!(!body.success);
        assert!(body.job_id.is_none());
    }

    #[actix_web::test]
    async fn import_rejects_non_http_remote_base_with_http_400() {
        let temp = tempfile::tempdir().unwrap();
        let app_state = make_app_state(temp.path()).await;
        let app = actix_test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(import_from_remote),
        )
        .await;

        let req = post_import(
            "ftp://192.168.1.10",
            &[serde_json::json!({"music_id": 1, "filename": "track.mp3"})],
            true,
        )
        .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn import_pulls_audio_and_lyrics_and_refreshes_db() {
        let temp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/music/id/1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"FAKE_MP3".to_vec())
                    .insert_header("content-type", "audio/mpeg"),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/lyrics/id/1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("[00:01.00]hi\n")
                    .insert_header("content-type", "text/plain; charset=utf-8"),
            )
            .mount(&server)
            .await;

        let app_state = make_app_state(temp.path()).await;
        let app = actix_test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(import_from_remote),
        )
        .await;

        let req = post_import(
            &format!("{}/api", server.uri()),
            &[serde_json::json!({"music_id": 1, "filename": "track.mp3"})],
            true,
        )
        .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body: CreateDownloadJobResponse = actix_test::read_body_json(resp).await;
        let job_id = body.job_id.expect("job id returned");

        let snapshot = wait_for_job(&app_state.download_jobs, &job_id).await;
        assert_eq!(snapshot.state, "completed");

        assert_eq!(
            std::fs::read(temp.path().join("track.mp3")).unwrap(),
            b"FAKE_MP3"
        );
        let lrc = std::fs::read_to_string(temp.path().join("track.lrc")).unwrap();
        assert!(lrc.contains("[00:01.00]hi"));

        use crate::entities::music::{Column as MusicColumn, Entity as MusicEntity};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
        let row = MusicEntity::find()
            .filter(MusicColumn::Filename.eq("track.mp3"))
            .one(&app_state.db_conn)
            .await
            .unwrap();
        assert!(row.is_some(), "imported song should be in the local DB");
    }

    #[actix_web::test]
    async fn import_skips_when_target_file_already_exists() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("track.mp3"), b"EXISTING").unwrap();

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/music/id/1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"SHOULD_NOT_FETCH".to_vec())
                    .insert_header("content-type", "audio/mpeg"),
            )
            // The audio endpoint must never be hit when the file already exists.
            .expect(0)
            .mount(&server)
            .await;

        let app_state = make_app_state(temp.path()).await;
        let app = actix_test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(import_from_remote),
        )
        .await;

        let req = post_import(
            &format!("{}/api", server.uri()),
            &[serde_json::json!({"music_id": 1, "filename": "track.mp3"})],
            true,
        )
        .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body: CreateDownloadJobResponse = actix_test::read_body_json(resp).await;
        let job_id = body.job_id.expect("job id returned");

        let snapshot = wait_for_job(&app_state.download_jobs, &job_id).await;
        assert_eq!(snapshot.state, "completed");
        assert!(snapshot.warning.unwrap_or_default().contains("已存在"));

        // Existing file is untouched.
        assert_eq!(
            std::fs::read(temp.path().join("track.mp3")).unwrap(),
            b"EXISTING"
        );
        // Verifies the expect(0) audio mock was never hit.
        server.verify().await;
    }

    #[actix_web::test]
    async fn import_continues_after_an_item_failure() {
        let temp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/music/id/1"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/music/id/2"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"OK2".to_vec())
                    .insert_header("content-type", "audio/mpeg"),
            )
            .mount(&server)
            .await;

        let app_state = make_app_state(temp.path()).await;
        let app = actix_test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(import_from_remote),
        )
        .await;

        let req = post_import(
            &format!("{}/api", server.uri()),
            &[
                serde_json::json!({"music_id": 1, "filename": "a.mp3"}),
                serde_json::json!({"music_id": 2, "filename": "b.mp3"}),
            ],
            false,
        )
        .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert!(resp.status().is_success());
        let body: CreateDownloadJobResponse = actix_test::read_body_json(resp).await;
        let job_id = body.job_id.expect("job id returned");

        let snapshot = wait_for_job(&app_state.download_jobs, &job_id).await;
        assert_eq!(snapshot.state, "completed");
        assert!(snapshot.warning.unwrap_or_default().contains("a.mp3"));
        assert!(!temp.path().join("a.mp3").exists());
        assert_eq!(std::fs::read(temp.path().join("b.mp3")).unwrap(), b"OK2");
    }
}
