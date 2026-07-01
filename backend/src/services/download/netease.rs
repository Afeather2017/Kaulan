use super::{
    resolve_download_file_stem, synthetic_preview_id, FullDownloadResult, MusicProvider,
    PreviewBuildResult,
};
use crate::types::{
    DownloadPreviewRequest, DownloadSource, DownloadTrackRequest, OnlineSearchResult,
};
use async_trait::async_trait;
use download_core::DownloadProgressReporter;
use netease_api::auth::Session as NeteaseSession;
use netease_api::types::{Quality, SearchType};
use netease_api::DownloadTrackRequest as NeteaseSourceDownloadTrackRequest;
use netease_api::NeteaseClient;
use std::path::Path;
use std::sync::Arc;
use tokio::task;
use tracing::{info, warn};

const NETEASE_QUALITY_FALLBACKS: [Quality; 3] =
    [Quality::Exhigh, Quality::Higher, Quality::Standard];

pub struct NeteaseProvider;

impl NeteaseProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NeteaseProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MusicProvider for NeteaseProvider {
    fn source(&self) -> DownloadSource {
        DownloadSource::Netease
    }

    fn is_enabled(&self) -> bool {
        NeteaseSession::load()
            .map(|session| session.is_logged_in())
            .unwrap_or(false)
    }

    fn status_summary(&self) -> String {
        if self.is_enabled() {
            "Netease session available".to_string()
        } else {
            "Netease login required".to_string()
        }
    }

    async fn search(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<OnlineSearchResult>, String> {
        let query = query.to_string();
        task::spawn_blocking(move || -> Result<Vec<OnlineSearchResult>, String> {
            let requires_login = !NeteaseSession::load()
                .map_err(|e| e.to_string())?
                .is_logged_in();
            let client = NeteaseClient::new().map_err(|e| e.to_string())?;
            let limit = u64::try_from(max_results).unwrap_or(u64::MAX);
            let result = client
                .search(&query, SearchType::Track, limit, 0)
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

    async fn build_preview(
        &self,
        request: &DownloadPreviewRequest,
        preview_root: &Path,
    ) -> Result<PreviewBuildResult, String> {
        let track_id = request
            .id
            .parse::<u64>()
            .map_err(|_| "无效的网易云歌曲 ID".to_string())?;
        let preview_root = preview_root.to_path_buf();
        let request_title = request.title.clone();
        let request_id = request.id.clone();
        let token = format!(
            "preview-{}-{}",
            self.source().as_str(),
            uuid::Uuid::new_v4()
        );

        task::spawn_blocking(move || -> Result<PreviewBuildResult, String> {
            let client = NeteaseClient::new().map_err(|e| e.to_string())?;
            let track = client.track_detail(track_id).map_err(|e| e.to_string())?;
            let final_name = format!("{token}.mp3");
            let final_path = preview_root.join(&final_name);
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
                synthetic_id: synthetic_preview_id(&request_id),
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }

    async fn download_full_with_progress(
        &self,
        request: &DownloadTrackRequest,
        target_dir: &Path,
        job_id: &str,
        reporter: Arc<dyn DownloadProgressReporter>,
    ) -> Result<FullDownloadResult, String> {
        let track_id = request
            .id
            .parse::<u64>()
            .map_err(|_| "无效的网易云歌曲 ID".to_string())?;
        let target_dir = target_dir.to_path_buf();
        let job_id = job_id.to_string();
        let file_stem = resolve_download_file_stem(request.file_name.as_deref(), &request.title)?;

        task::spawn_blocking(move || -> Result<FullDownloadResult, String> {
            let client = NeteaseClient::new().map_err(|e| e.to_string())?;
            let track = client.track_detail(track_id).map_err(|e| e.to_string())?;
            let filename = format!("{file_stem}.mp3");
            let final_path = target_dir.join(filename);
            let source_request = NeteaseSourceDownloadTrackRequest {
                job_id,
                track_id,
                quality: Quality::Exhigh,
            };
            client
                .download_track_with_progress(&source_request, &final_path, reporter)
                .map_err(|err| {
                    explain_netease_failure(client.session().is_logged_in(), &[err.to_string()])
                })?;
            Ok(FullDownloadResult {
                final_path,
                cover_url: track.album.pic_url,
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

fn join_artists(artists: &[netease_api::types::Artist]) -> String {
    artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
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

    Err(explain_netease_failure(logged_in, &failures))
}

fn explain_netease_failure(logged_in: bool, failures: &[String]) -> String {
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
