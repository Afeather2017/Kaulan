use super::{
    create_download_staging_dir, sanitize_filename, simple_hash, FullDownloadResult, MusicProvider,
    PreviewBuildResult,
};
use crate::types::{
    DownloadPreviewRequest, DownloadSource, DownloadTrackRequest, OnlineSearchResult,
};
use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::task;
use tracing::warn;
use ytdl_audio::{DownloadOpts, YoutubeClient};

const YOUTUBE_COOKIE_HEADER_PATH_ENV: &str = "KAULAN_YOUTUBE_COOKIE_HEADER_PATH";

pub struct YoutubeProvider;

impl YoutubeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for YoutubeProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl MusicProvider for YoutubeProvider {
    fn source(&self) -> DownloadSource {
        DownloadSource::Youtube
    }

    fn is_enabled(&self) -> bool {
        load_youtube_cookie_header().is_some()
    }

    fn status_summary(&self) -> String {
        if load_youtube_cookie_header().is_some() {
            "YouTube cookies configured".to_string()
        } else {
            "YouTube cookies not configured".to_string()
        }
    }

    async fn search(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<OnlineSearchResult>, String> {
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

    async fn build_preview(
        &self,
        request: &DownloadPreviewRequest,
        preview_root: &Path,
    ) -> Result<PreviewBuildResult, String> {
        let synthetic_id = -((simple_hash(&request.id) as i32).abs());
        let token = format!(
            "preview-{}-{}",
            self.source().as_str(),
            uuid::Uuid::new_v4()
        );
        let client = youtube_client().map_err(|e| e.to_string())?;
        let temp_dir = create_download_staging_dir(preview_root)?;
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
        let source_audio = result.audio_path;
        let output_dir = preview_root.to_path_buf();
        let (final_name, final_path) = task::spawn_blocking(move || {
            finalize_youtube_audio(&source_audio, &output_dir, &token)
        })
        .await
        .map_err(|e| format!("YouTube audio export task failed: {e}"))??;

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

    async fn download_full(
        &self,
        request: &DownloadTrackRequest,
        target_dir: &Path,
    ) -> Result<FullDownloadResult, String> {
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
        let source_audio = result.audio_path;
        let output_dir = target_dir.to_path_buf();
        let (_final_filename, final_path) = task::spawn_blocking(move || {
            finalize_youtube_audio(&source_audio, &output_dir, &title)
        })
        .await
        .map_err(|e| format!("YouTube audio export task failed: {e}"))??;

        Ok(FullDownloadResult {
            final_path,
            cover_url: Some(format!(
                "https://i.ytimg.com/vi/{}/hqdefault.jpg",
                request.id
            )),
        })
    }
}

fn youtube_client() -> Result<YoutubeClient, ytdl_audio::Error> {
    let mut client = YoutubeClient::new(None)?;
    match crate::create_youtube_js_runner() {
        Ok(Some(runner)) => client.set_js_runner(runner),
        Ok(None) => super::ensure_ytdl_solver_dependencies().map_err(ytdl_audio::Error::Other)?,
        Err(err) => {
            warn!(
                "[DOWNLOAD] Failed to create webview YouTube JS runner, falling back to default solver: {}",
                err
            );
        }
    }
    Ok(client)
}

fn finalize_youtube_audio(
    source_audio: &Path,
    output_dir: &Path,
    output_stem: &str,
) -> Result<(String, PathBuf), String> {
    crate::ffmpeg::export_audio_for_download(source_audio, output_dir, output_stem)
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

#[cfg(test)]
mod tests {
    use super::finalize_youtube_audio;
    use std::path::Path;

    #[test]
    fn youtube_finalize_reports_transcode_errors() {
        let temp_dir = tempfile::tempdir().unwrap();
        let source_audio = temp_dir.path().join("source.webm");
        std::fs::write(&source_audio, b"webm").unwrap();
        let err =
            finalize_youtube_audio(Path::new(&source_audio), temp_dir.path(), "preview-token")
                .unwrap_err();

        assert!(err.contains("failed to open input file") || err.contains("Invalid data found"));
    }
}
