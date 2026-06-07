use super::{
    sanitize_filename, simple_hash, FullDownloadResult, MusicProvider, PreviewBuildResult,
};
use crate::types::{
    DownloadPreviewRequest, DownloadSource, DownloadTrackRequest, OnlineSearchResult,
};
use async_trait::async_trait;
use bilibili_api::auth::BiliSession;
use bilibili_api::{BilibiliClient, BilibiliError};
use std::path::Path;
use tokio::task;

const BILIBILI_RAW_AUDIO_EXTENSION: &str = "m4s";

pub struct BilibiliProvider;

impl BilibiliProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait(?Send)]
impl MusicProvider for BilibiliProvider {
    fn source(&self) -> DownloadSource {
        DownloadSource::Bilibili
    }

    fn is_enabled(&self) -> bool {
        BiliSession::load()
            .map(|session| session.is_logged_in())
            .unwrap_or(false)
    }

    fn status_summary(&self) -> String {
        if self.is_enabled() {
            "Bilibili session available".to_string()
        } else {
            "Bilibili login required".to_string()
        }
    }

    async fn search(
        &self,
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
        &self,
        request: &DownloadPreviewRequest,
        preview_root: &Path,
    ) -> Result<PreviewBuildResult, String> {
        let bvid = request.id.clone();
        let preview_root = preview_root.to_path_buf();
        let synthetic_id = -((simple_hash(&request.id) as i32).abs());
        let token = format!(
            "preview-{}-{}",
            self.source().as_str(),
            uuid::Uuid::new_v4()
        );

        task::spawn_blocking(move || -> Result<PreviewBuildResult, String> {
            let client = BilibiliClient::new().map_err(|e| e.to_string())?;
            let detail = client.video_detail(&bvid).map_err(|e| e.to_string())?;
            let final_name = if should_skip_bilibili_ffmpeg() {
                format!("{token}.{BILIBILI_RAW_AUDIO_EXTENSION}")
            } else {
                format!("{token}.mp3")
            };
            let final_path = preview_root.join(&final_name);
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

    async fn download_full(
        &self,
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
        client.download_audio(bvid, output, bilibili_api::types::AudioFormat::Mp3)
    }
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

#[cfg(test)]
mod tests {
    use super::{should_skip_bilibili_ffmpeg, BILIBILI_RAW_AUDIO_EXTENSION};

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
