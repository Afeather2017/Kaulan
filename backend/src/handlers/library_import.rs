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

use crate::file_ops::{is_std_fs_path, source_exists, source_write_file, SUPPORTED_EXTENSIONS};
use crate::handlers::download::{lyric_sidecar_extension, resolve_target_dir};
use crate::services::download::sanitize_filename;
use crate::services::scanner;
use crate::types::{
    AppState, CreateDownloadJobResponse, DownloadSource, ImportFromRemoteRequest, ImportRemoteItem,
};
use actix_web::{post, web, HttpResponse};
use download_core::DownloadProgressPhase;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
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

    // SSRF guard: resolve the remote host and refuse internal targets (cloud
    // metadata, link-local, loopback, unspecified). Redirect-following is also
    // disabled on the client so a 3xx cannot bypass this check. See
    // `remote_host_is_safe` for the exact policy.
    if !remote_host_is_safe(&remote_base).await {
        warn!(
            "[IMPORT] Rejecting import request: remote host resolves to a blocked range: {}",
            remote_base
        );
        return HttpResponse::BadRequest().json(CreateDownloadJobResponse {
            success: false,
            message: "不允许的远端服务器地址".to_string(),
            job_id: None,
        });
    }

    if request.items.is_empty() {
        return HttpResponse::BadRequest().json(CreateDownloadJobResponse {
            success: false,
            message: "未选择要导入的歌曲".to_string(),
            job_id: None,
        });
    }

    if let Err(message) = validate_import_item_filenames(&request.items) {
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
        // Do NOT follow redirects: the remote host was validated against
        // blocked ranges in `remote_host_is_safe`, but a 3xx from that host to
        // an internal address (e.g. cloud metadata) would silently bypass the
        // check. A non-2xx (including 3xx) is treated as a fetch failure.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("无法创建 HTTP 客户端: {e}"))?;

    let include_lyrics = request.include_lyrics.unwrap_or(true);
    let total = request.items.len();
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    for (index, item) in request.items.iter().enumerate() {
        let percent = index
            .saturating_mul(100)
            .checked_div(total.max(1))
            .unwrap_or(0);
        let label = display_label(item);

        data.download_jobs
            .update_phase(
                job_id,
                DownloadSource::Import,
                DownloadProgressPhase::Downloading,
                format!("正在导入 {}/{total}: {label}", index.saturating_add(1)),
                Some(format!("{percent}%")),
            )
            .await;

        match import_one(&client, &remote_base, item, &target_dir, include_lyrics).await {
            Ok(ImportOutcome::Imported) => imported = imported.saturating_add(1),
            Ok(ImportOutcome::Skipped(reason)) => {
                skipped = skipped.saturating_add(1);
                warn!(
                    "[IMPORT] job={} skipped id={}: {}",
                    job_id, item.music_id, reason
                );
                warnings.push(format!("{label}: {reason}"));
            }
            Err(err) => {
                failed = failed.saturating_add(1);
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

    if let Err(err) = scanner::update_database(&data.db_conn, &data.scan_registry).await {
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
    let (stem, file_ext) = parse_filename_parts(item.filename.as_deref(), item.music_id)?;
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

    let final_ext = if known_ext {
        file_ext
    } else {
        match content_type_to_extension(&content_type) {
            Some(ext) => ext.to_string(),
            None => {
                return Err(format!(
                    "远端返回了非音频内容类型，无法导入: {content_type}"
                ));
            }
        }
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

    // Stream the body straight to a temp file, bounding memory to one chunk and
    // enforcing the size cap DURING the write (not after). The target is
    // guaranteed to be a StdFs path by the handler's `is_std_fs_path` guard, so
    // plain `tokio::fs` is safe; a `.part` → rename leaves no partial file.
    stream_bounded_to_file(response, &final_path, IMPORT_MAX_BYTES_PER_ITEM).await?;

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

/// Stream `response` to `dest`, aborting (and deleting the partial file) if the
/// body exceeds `max_bytes`.
///
/// Writes to a sibling `.part` temp file and renames it into place on success,
/// so an oversized or interrupted download never leaves a half-written audio
/// file. Memory is bounded by a single chunk — the body is never buffered
/// whole — and the cap is enforced while bytes flow, so an oversized file is
/// rejected before it fills memory/disk. `dest` must be on a StdFs path (the
/// import handler rejects non-filesystem targets), so plain `tokio::fs` is used
/// directly. Returns the number of bytes written.
async fn stream_bounded_to_file(
    mut response: reqwest::Response,
    dest: &Path,
    max_bytes: usize,
) -> Result<u64, String> {
    let dir = dest.parent().unwrap_or_else(|| Path::new("."));
    let part_path = dir.join(format!(".{}.part", Uuid::new_v4().simple()));

    let mut file = tokio::fs::File::create(&part_path)
        .await
        .map_err(|e| format!("创建临时文件失败: {e}"))?;

    // Compare in u64 to avoid `as` conversions (banned by CI clippy). usize can
    // only be larger than u64 on a >64-bit target, which this crate does not
    // target; clamp to u64::MAX there so an absurd value trips the cap.
    let max_bytes = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    let mut total: u64 = 0;
    loop {
        let chunk = response
            .chunk()
            .await
            .map_err(|e| format!("读取远端音频失败: {e}"))?;
        let Some(chunk) = chunk else {
            break;
        };
        total = total.saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
        if total > max_bytes {
            // Abort before writing the overflowing chunk; drop the partial file.
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(format!("文件过大（已读取 {total} 字节，上限 {max_bytes}）"));
        }
        if let Err(e) = file.write_all(&chunk).await {
            let _ = tokio::fs::remove_file(&part_path).await;
            return Err(format!("写入本机失败: {e}"));
        }
    }

    // Flush before the rename so all bytes hit disk; the rename is atomic on the
    // same filesystem, so `dest` only ever appears complete.
    if let Err(e) = file.flush().await {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(format!("写入本机失败: {e}"));
    }
    drop(file);

    if let Err(e) = tokio::fs::rename(&part_path, dest).await {
        let _ = tokio::fs::remove_file(&part_path).await;
        return Err(format!("移动临时文件失败: {e}"));
    }
    Ok(total)
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

/// Pure SSRF predicate: is `ip` in a range we refuse to import from?
///
/// Always blocks the high-value SSRF targets — link-local (which includes the
/// cloud metadata address `169.254.169.254`), unspecified, and IPv4-mapped-v6
/// forms of the same. Loopback is blocked when `block_loopback` is true.
///
/// RFC1918 / ULA private ranges are deliberately **not** blocked: importing
/// from another Kaulan server on the LAN is this feature's intended use, so a
/// blanket private-range ban would break it. The remaining risk (loopback
/// services on the device) is gated by `block_loopback` instead.
fn is_blocked_ip(ip: IpAddr, block_loopback: bool) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_link_local() || v4.is_unspecified() || (block_loopback && v4.is_loopback())
        }
        IpAddr::V6(v6) => {
            v6.is_unicast_link_local()
                || v6.is_unspecified()
                || (block_loopback && v6.is_loopback())
                // Catch ::ffff:a.b.c.d mapped forms of the above.
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_blocked_ip(IpAddr::V4(v4), block_loopback))
        }
    }
}

/// Runtime variant of [`is_blocked_ip`]: loopback is blocked in production but
/// exempted under `cfg(test)` because `wiremock` serves its mocks on
/// `127.0.0.1` (link-local / unspecified / metadata stay blocked in all
/// configurations).
fn runtime_blocked(ip: IpAddr) -> bool {
    is_blocked_ip(ip, !cfg!(test))
}

/// Resolve `remote_base` and return `false` if any resolved address lands in a
/// blocked range (SSRF mitigation). Returns `false` on parse or resolver
/// failure so a broken or hostile name cannot slip through. A host is only safe
/// when **every** resolved address is safe.
///
/// Residual gap: a DNS name that resolves to a safe address here but to an
/// internal one when `reqwest` re-resolves (DNS rebinding) is not fully closed;
/// disabling redirects limits the practical impact. Pinning the resolved IP
/// would break TLS hostname verification for https remotes, so it is not done.
async fn remote_host_is_safe(remote_base: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(remote_base) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let port = url.port_or_known_default().unwrap_or(80);
    let target = format!("{host}:{port}");
    let Ok(addrs) = tokio::net::lookup_host(target).await else {
        return false;
    };
    let resolved: Vec<_> = addrs.collect();
    if resolved.is_empty() {
        return false;
    }
    resolved.iter().all(|addr| !runtime_blocked(addr.ip()))
}

/// Map an audio `Content-Type` to a file extension (lowercase, no dot).
///
/// Returns `None` for anything that is not a recognized audio type, so a
/// non-audio response (e.g. an HTML error page that still returned 200) is
/// rejected at import time rather than mislabeled as `.mp3`. The generic
/// `application/octet-stream` is intentionally accepted (as `mp3`) because many
/// audio servers send it.
fn content_type_to_extension(content_type: &str) -> Option<&'static str> {
    let lower = content_type.to_ascii_lowercase();
    let mime = lower.split(';').next().unwrap_or("").trim();
    match mime {
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/ogg" => Some("ogg"),
        "audio/wav" | "audio/x-wav" => Some("wav"),
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => Some("m4a"),
        "audio/aac" => Some("aac"),
        "audio/opus" => Some("opus"),
        "audio/webm" | "audio/x-matroska" | "audio/x-mka" => Some("mka"),
        // Generic binary: many audio servers send this; default to mp3.
        "application/octet-stream" => Some("mp3"),
        _ => None,
    }
}

/// Derive `(stem, extension)` for the local file from the supplied filename.
///
/// The stem is sanitized via the shared `sanitize_filename`; the extension is
/// lowercased but not yet validated against `SUPPORTED_EXTENSIONS` (the caller
/// decides whether to trust it or fall back to the content type).
fn parse_filename_parts(filename: Option<&str>, music_id: i32) -> Result<(String, String), String> {
    let (raw_stem, ext): (String, String) =
        match filename.map(str::trim).filter(|name| !name.is_empty()) {
            Some(name) => {
                if name.contains('/') || name.contains('\\') {
                    return Err("文件名不能包含路径分隔符".to_string());
                }

                let path = Path::new(name);
                let stem = path
                    .file_stem()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_default();
                let extension = path
                    .extension()
                    .map(|value| value.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                (stem, extension)
            }
            None => (String::new(), String::new()),
        };

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

    Ok((stem, ext))
}

fn validate_import_item_filenames(items: &[ImportRemoteItem]) -> Result<(), String> {
    for item in items {
        if let Some(name) = item.filename.as_deref().map(str::trim) {
            if name.contains('/') || name.contains('\\') {
                return Err(format!("{name}: 文件名不能包含路径分隔符"));
            }
        }
    }

    Ok(())
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
        content_type_to_extension, import_from_remote, is_blocked_ip, parse_filename_parts,
        remote_host_is_safe, stream_bounded_to_file, validate_import_item_filenames,
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
        assert_eq!(content_type_to_extension("audio/mpeg"), Some("mp3"));
        assert_eq!(content_type_to_extension("audio/flac"), Some("flac"));
        assert_eq!(content_type_to_extension("audio/mp4"), Some("m4a"));
        assert_eq!(
            content_type_to_extension("audio/ogg; codecs=opus"),
            Some("ogg")
        );
        // Generic binary is treated as mp3 (common for audio servers).
        assert_eq!(
            content_type_to_extension("application/octet-stream"),
            Some("mp3")
        );
        // Clearly non-audio types are refused rather than mislabeled as mp3.
        assert_eq!(content_type_to_extension("text/html"), None);
        assert_eq!(content_type_to_extension("application/json"), None);
        assert_eq!(content_type_to_extension(""), None);
    }

    #[test]
    fn parse_filename_parts_uses_supplied_stem_and_extension() {
        let (stem, ext) = parse_filename_parts(Some("Artist - Track.mp3"), 7).unwrap();
        assert_eq!(stem, "Artist - Track");
        assert_eq!(ext, "mp3");
    }

    #[test]
    fn parse_filename_parts_falls_back_when_missing() {
        let (stem, ext) = parse_filename_parts(None, 42).unwrap();
        assert_eq!(stem, "remote-42");
        assert_eq!(ext, "");
    }

    #[test]
    fn parse_filename_parts_rejects_path_separators() {
        let slash = parse_filename_parts(Some("album/track.mp3"), 1).unwrap_err();
        assert!(slash.contains("路径分隔符"));

        let backslash = parse_filename_parts(Some("album\\track.mp3"), 1).unwrap_err();
        assert!(backslash.contains("路径分隔符"));
    }

    #[test]
    fn parse_filename_parts_sanitizes_other_reserved_characters() {
        let (stem, _) = parse_filename_parts(Some("bad:name?.mp3"), 1).unwrap();
        assert!(!stem.contains(':'));
        assert!(!stem.contains('?'));
    }

    #[test]
    fn validate_import_item_filenames_rejects_path_separators() {
        let items = vec![crate::types::ImportRemoteItem {
            music_id: 1,
            filename: Some("album\\track.mp3".to_string()),
        }];
        let error = validate_import_item_filenames(&items).unwrap_err();
        assert!(error.contains("路径分隔符"));
    }

    #[test]
    fn max_bytes_per_item_is_half_gigibyte() {
        assert_eq!(IMPORT_MAX_BYTES_PER_ITEM, 512 * 1024 * 1024);
    }

    #[test]
    fn blocked_ip_refuses_metadata_loopback_and_unspecified() {
        // Cloud metadata (link-local) is the primary SSRF target.
        assert!(is_blocked_ip("169.254.169.254".parse().unwrap(), true));
        assert!(is_blocked_ip("169.254.0.1".parse().unwrap(), true));
        // Loopback and unspecified are blocked in production mode.
        assert!(is_blocked_ip("127.0.0.1".parse().unwrap(), true));
        assert!(is_blocked_ip("127.1.2.3".parse().unwrap(), true));
        assert!(is_blocked_ip("::1".parse().unwrap(), true));
        assert!(is_blocked_ip("0.0.0.0".parse().unwrap(), true));
        assert!(is_blocked_ip("::".parse().unwrap(), true));
        // IPv6 link-local.
        assert!(is_blocked_ip("fe80::1".parse().unwrap(), true));
        // IPv4-mapped-v6 metadata must not sneak through as IPv6.
        assert!(is_blocked_ip(
            "::ffff:169.254.169.254".parse().unwrap(),
            true
        ));
    }

    #[test]
    fn blocked_ip_allows_public_and_private_ranges() {
        // Public internet is fine.
        assert!(!is_blocked_ip("8.8.8.8".parse().unwrap(), true));
        // RFC1918 private ranges are deliberately allowed: importing from a LAN
        // Kaulan server is the feature's intended purpose.
        assert!(!is_blocked_ip("192.168.1.10".parse().unwrap(), true));
        assert!(!is_blocked_ip("10.0.0.5".parse().unwrap(), true));
        assert!(!is_blocked_ip("172.16.0.2".parse().unwrap(), true));
    }

    #[test]
    fn blocked_ip_loopback_exemption_is_independent_of_link_local() {
        // With loopback exempted (test/wiremock mode), loopback is allowed but
        // metadata/link-local/unspecified must still be blocked.
        assert!(!is_blocked_ip("127.0.0.1".parse().unwrap(), false));
        assert!(is_blocked_ip("169.254.169.254".parse().unwrap(), false));
        assert!(is_blocked_ip("0.0.0.0".parse().unwrap(), false));
    }

    #[actix_web::test]
    async fn remote_host_is_safe_accepts_loopback_literal_under_test() {
        // wiremock binds 127.0.0.1, so loopback must be reachable from tests.
        assert!(remote_host_is_safe("http://127.0.0.1:2080/api").await);
    }

    #[actix_web::test]
    async fn remote_host_is_safe_rejects_metadata_address() {
        // A literal link-local IP needs no network/DNS to resolve.
        assert!(!remote_host_is_safe("http://169.254.169.254/api").await);
    }

    #[actix_web::test]
    async fn stream_writes_full_body_and_renames_into_place() {
        let temp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/music/id/1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"HELLO".to_vec())
                    .insert_header("content-type", "audio/mpeg"),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/api/music/id/1", server.uri()))
            .send()
            .await
            .unwrap();
        let dest = temp.path().join("ok.mp3");
        let written = stream_bounded_to_file(resp, &dest, 1024).await.unwrap();
        assert_eq!(written, 5);
        assert_eq!(std::fs::read(&dest).unwrap(), b"HELLO");
        // Only the final file remains — no leftover .part temp.
        assert_eq!(
            std::fs::read_dir(temp.path()).unwrap().count(),
            1,
            "no partial temp should remain"
        );
    }

    #[actix_web::test]
    async fn stream_aborts_on_overflow_and_leaves_no_file() {
        let temp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/music/id/1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![b'X'; 1000])
                    .insert_header("content-type", "audio/mpeg"),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/api/music/id/1", server.uri()))
            .send()
            .await
            .unwrap();
        let dest = temp.path().join("big.mp3");
        let err = stream_bounded_to_file(resp, &dest, 100)
            .await
            .expect_err("oversized body must be rejected");
        assert!(err.contains("文件过大"), "{err}");
        assert!(!dest.exists(), "no final file on overflow");
        // No partial .part temp left behind either.
        assert_eq!(
            std::fs::read_dir(temp.path()).unwrap().count(),
            0,
            "no partial temp should remain"
        );
    }

    async fn make_app_state(download_root: &std::path::Path) -> web::Data<AppState> {
        let db_conn =
            crate::database::establish_connection(download_root.to_str().expect("utf-8 temp path"))
                .await
                .expect("db connection");
        let scan_registry = std::sync::Arc::new(crate::file_ops::ScanRegistry::new());
        scan_registry.register(std::sync::Arc::new(crate::file_ops::StdFsScanBackend::new(
            std::path::PathBuf::from(download_root),
        )));
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
            scan_registry,
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
    async fn import_rejects_metadata_remote_base_with_http_400() {
        let temp = tempfile::tempdir().unwrap();
        let app_state = make_app_state(temp.path()).await;
        let app = actix_test::init_service(
            App::new()
                .app_data(app_state.clone())
                .service(import_from_remote),
        )
        .await;

        // 169.254.169.254 is the cloud-metadata address; it must be refused
        // before any job is created, with no fetch attempted.
        let req = post_import(
            "http://169.254.169.254/api",
            &[serde_json::json!({"music_id": 1, "filename": "track.mp3"})],
            true,
        )
        .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
        let body: CreateDownloadJobResponse = actix_test::read_body_json(resp).await;
        assert!(!body.success);
        assert!(body.job_id.is_none(), "no job should be created for SSRF");
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
    async fn import_rejects_path_like_filename_before_creating_job() {
        let temp = tempfile::tempdir().unwrap();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/music/id/1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(b"SHOULD_NOT_FETCH".to_vec())
                    .insert_header("content-type", "audio/mpeg"),
            )
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
            &[serde_json::json!({"music_id": 1, "filename": "album/track.mp3"})],
            false,
        )
        .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
        let body: CreateDownloadJobResponse = actix_test::read_body_json(resp).await;
        assert!(!body.success);
        assert!(body.job_id.is_none());
        assert!(body.message.contains("路径分隔符"));
        assert!(
            app_state.download_jobs.active_jobs().await.is_empty(),
            "invalid filenames must not create import jobs"
        );
        assert!(!temp.path().join("album").exists());
        assert!(!temp.path().join("track.mp3").exists());
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
