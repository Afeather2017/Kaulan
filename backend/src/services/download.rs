//! Download provider registry and runtime bootstrap.

mod bilibili;
mod netease;
mod youtube;

use async_trait::async_trait;
use download_core::{
    apply_progress_event, DownloadProgressEvent, DownloadProgressPhase, DownloadProgressReporter,
    DownloadProgressSnapshot, NoopProgressReporter,
};
use reqwest::StatusCode;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Once, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex as TokioMutex;
use tokio::task;
use tracing::warn;

use crate::types::{
    DownloadPreviewRequest, DownloadSource, DownloadTrackRequest, OnlineProviderStatus,
    OnlineSearchResult,
};

pub use bilibili::resolve_cover_url as resolve_bilibili_cover_url;
pub use bilibili::BilibiliProvider;
pub use netease::NeteaseProvider;
pub use youtube::YoutubeProvider;

static FFMPEG_PATH_INIT: Once = Once::new();
static PROVIDERS: OnceLock<Vec<Arc<dyn MusicProvider>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct PreviewBuildResult {
    pub source: DownloadSource,
    pub file_name: String,
    pub absolute_path: PathBuf,
    pub cover_url: Option<String>,
    pub synthetic_id: i32,
}

#[derive(Debug, Clone)]
pub struct FullDownloadResult {
    pub final_path: PathBuf,
    pub cover_url: Option<String>,
}

const DOWNLOAD_JOB_TTL: Duration = Duration::from_secs(120);

#[derive(Debug)]
struct DownloadJobRecord {
    snapshot: DownloadProgressSnapshot,
    finished_at: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct DownloadJobStore {
    jobs: TokioMutex<HashMap<String, DownloadJobRecord>>,
}

impl DownloadJobStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create(&self, job_id: &str, source: DownloadSource, title: &str) {
        self.cleanup().await;
        let event = DownloadProgressEvent {
            job_id: job_id.to_string(),
            source: source.as_str().to_string(),
            phase: DownloadProgressPhase::Queued,
            percent: None,
            message: format!("Queued download: {title}"),
            detail: None,
        };
        let snapshot = apply_progress_event(None, event);
        let mut jobs = self.jobs.lock().await;
        jobs.insert(
            job_id.to_string(),
            DownloadJobRecord {
                snapshot,
                finished_at: None,
            },
        );
    }

    pub async fn apply_event(&self, event: DownloadProgressEvent) {
        let mut jobs = self.jobs.lock().await;
        let record = jobs
            .entry(event.job_id.clone())
            .or_insert_with(|| DownloadJobRecord {
                snapshot: apply_progress_event(None, event.clone()),
                finished_at: None,
            });
        record.snapshot = apply_progress_event(Some(record.snapshot.clone()), event.clone());
        if !matches!(event.phase, DownloadProgressPhase::Failed) {
            record.snapshot.error = None;
        }
        record.finished_at = terminal_phase(&event.phase).then_some(Instant::now());
    }

    pub async fn update_phase(
        &self,
        job_id: &str,
        source: DownloadSource,
        phase: DownloadProgressPhase,
        message: impl Into<String>,
        detail: Option<String>,
    ) {
        self.apply_event(DownloadProgressEvent {
            job_id: job_id.to_string(),
            source: source.as_str().to_string(),
            phase,
            percent: None,
            message: message.into(),
            detail,
        })
        .await;
    }

    pub async fn mark_warning(&self, job_id: &str, warning: Option<String>) {
        let mut jobs = self.jobs.lock().await;
        if let Some(record) = jobs.get_mut(job_id) {
            record.snapshot.warning = warning;
        }
    }

    pub async fn mark_completed(
        &self,
        job_id: &str,
        filename: Option<String>,
        warning: Option<String>,
    ) {
        let mut jobs = self.jobs.lock().await;
        if let Some(record) = jobs.get_mut(job_id) {
            record.snapshot.state = "completed".to_string();
            record.snapshot.phase = DownloadProgressPhase::Completed;
            record.snapshot.percent = Some(100);
            record.snapshot.message = "Download complete".to_string();
            record.snapshot.filename = filename;
            record.snapshot.warning = warning;
            record.snapshot.error = None;
            record.finished_at = Some(Instant::now());
        }
    }

    pub async fn mark_failed(&self, job_id: &str, source: DownloadSource, message: String) {
        self.apply_event(DownloadProgressEvent {
            job_id: job_id.to_string(),
            source: source.as_str().to_string(),
            phase: DownloadProgressPhase::Failed,
            percent: None,
            message: message.clone(),
            detail: Some(message),
        })
        .await;
    }

    pub async fn active_jobs(&self) -> Vec<DownloadProgressSnapshot> {
        self.cleanup().await;
        let jobs = self.jobs.lock().await;
        jobs.values()
            .filter(|record| !terminal_phase(&record.snapshot.phase))
            .map(|record| record.snapshot.clone())
            .collect()
    }

    pub async fn get(&self, job_id: &str) -> Option<DownloadProgressSnapshot> {
        self.cleanup().await;
        let jobs = self.jobs.lock().await;
        jobs.get(job_id).map(|record| record.snapshot.clone())
    }

    async fn cleanup(&self) {
        self.cleanup_with_ttl(DOWNLOAD_JOB_TTL).await;
    }

    async fn cleanup_with_ttl(&self, ttl: Duration) {
        let mut jobs = self.jobs.lock().await;
        jobs.retain(|_, record| {
            record
                .finished_at
                .is_none_or(|finished_at| finished_at.elapsed() < ttl)
        });
    }
}

#[derive(Clone)]
pub struct JobProgressReporter {
    job_store: Arc<DownloadJobStore>,
}

impl JobProgressReporter {
    pub fn new(job_store: Arc<DownloadJobStore>) -> Self {
        Self { job_store }
    }
}

impl DownloadProgressReporter for JobProgressReporter {
    fn emit(&self, event: DownloadProgressEvent) {
        let job_store = self.job_store.clone();
        tokio::spawn(async move {
            job_store.apply_event(event).await;
        });
    }
}

fn terminal_phase(phase: &DownloadProgressPhase) -> bool {
    matches!(
        phase,
        DownloadProgressPhase::Completed | DownloadProgressPhase::Failed
    )
}

// Provider futures must be Send because async download jobs are spawned onto the Tokio runtime.
#[async_trait]
pub trait MusicProvider: Send + Sync {
    fn source(&self) -> DownloadSource;
    fn is_enabled(&self) -> bool;
    fn status_summary(&self) -> String;
    async fn search(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<OnlineSearchResult>, String>;
    async fn build_preview(
        &self,
        request: &DownloadPreviewRequest,
        preview_root: &Path,
    ) -> Result<PreviewBuildResult, String>;
    async fn download_full_with_progress(
        &self,
        request: &DownloadTrackRequest,
        target_dir: &Path,
        job_id: &str,
        reporter: Arc<dyn DownloadProgressReporter>,
    ) -> Result<FullDownloadResult, String>;
    async fn download_full(
        &self,
        request: &DownloadTrackRequest,
        target_dir: &Path,
    ) -> Result<FullDownloadResult, String> {
        self.download_full_with_progress(
            request,
            target_dir,
            "download",
            Arc::new(NoopProgressReporter),
        )
        .await
    }
}

pub fn initialize_runtime() -> Result<(), String> {
    configure_ffmpeg_path_for_process();
    Ok(())
}

pub fn provider(source: DownloadSource) -> Option<&'static dyn MusicProvider> {
    providers()
        .iter()
        .find(|provider| provider.source() == source)
        .map(|provider| provider.as_ref())
}

pub fn providers_for_sources(sources: &[DownloadSource]) -> Vec<&'static dyn MusicProvider> {
    sources
        .iter()
        .filter_map(|source| provider(*source))
        .collect()
}

pub fn build_online_provider_statuses() -> Vec<OnlineProviderStatus> {
    providers()
        .iter()
        .map(|provider| OnlineProviderStatus {
            source: provider.source(),
            enabled: provider.is_enabled(),
            summary: provider.status_summary(),
        })
        .collect()
}

pub fn ensure_ytdl_solver_dependencies() -> Result<(), String> {
    let solver_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/ytdl-audio/js");
    ensure_ytdl_solver_dependencies_in_dir(&solver_dir)
}

pub(crate) fn create_download_staging_dir(base_dir: &Path) -> Result<tempfile::TempDir, String> {
    std::fs::create_dir_all(base_dir).map_err(|e| format!("无法创建下载缓存目录: {e}"))?;

    let staging_root = base_dir.join(".staging");
    std::fs::create_dir_all(&staging_root).map_err(|e| format!("无法创建下载缓存目录: {e}"))?;

    tempfile::Builder::new()
        .prefix(".tmp")
        .tempdir_in(&staging_root)
        .map_err(|e| format!("无法创建下载缓存目录: {e}"))
}

pub(crate) fn sanitize_filename(name: &str) -> String {
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

pub(crate) fn resolve_download_file_stem(
    requested_name: Option<&str>,
    fallback_title: &str,
) -> Result<String, String> {
    match requested_name {
        Some(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return Err("文件名不能为空".to_string());
            }

            let mut components = Path::new(trimmed).components();
            if !matches!(components.next(), Some(Component::Normal(_)))
                || components.next().is_some()
            {
                return Err("文件名不能包含路径".to_string());
            }

            let stem = Path::new(trimmed)
                .file_stem()
                .and_then(|value| value.to_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "文件名不能为空".to_string())?;

            Ok(sanitize_filename(stem))
        }
        None => Ok(sanitize_filename(fallback_title)),
    }
}

pub(crate) fn simple_hash(value: &str) -> u64 {
    value.bytes().fold(5381_u64, |acc, byte| {
        acc.wrapping_mul(33).wrapping_add(u64::from(byte))
    })
}

pub(crate) fn synthetic_preview_id(value: &str) -> i32 {
    let max = u64::try_from(i32::MAX).unwrap_or(u64::MAX);
    let bounded = simple_hash(value).checked_rem(max).unwrap_or(0);
    let positive = i32::try_from(bounded).unwrap_or(i32::MAX);
    positive.saturating_neg()
}

pub fn configure_ffmpeg_path_for_process() {
    FFMPEG_PATH_INIT.call_once(configure_ffmpeg_path_once);
}

fn should_embed_cover_art(audio_path: &Path) -> bool {
    !audio_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ogg"))
}

pub async fn attach_cover_art_from_url(audio_path: &Path, cover_url: &str) -> Result<(), String> {
    if !should_embed_cover_art(audio_path) {
        return Ok(());
    }

    let response = reqwest::get(cover_url)
        .await
        .map_err(|e| format!("failed to download cover art {cover_url}: {e}"))?;
    if response.status() != StatusCode::OK {
        return Err(format!(
            "cover art download failed with status {} from {}",
            response.status(),
            cover_url
        ));
    }

    let cover_bytes = response
        .bytes()
        .await
        .map_err(|e| format!("failed to read cover art response {cover_url}: {e}"))?
        .to_vec();
    let audio_path = audio_path.to_path_buf();

    task::spawn_blocking(move || -> Result<(), String> {
        let cover = crate::ffmpeg::normalize_cover_art_bytes(&cover_bytes)?;
        crate::ffmpeg::replace_cover_art_in_place(&audio_path, &cover)
    })
    .await
    .map_err(|e| format!("cover-art attachment task failed: {e}"))?
}

pub async fn try_attach_cover_art_from_url(audio_path: &Path, cover_url: Option<&str>) {
    let Some(cover_url) = cover_url else {
        return;
    };

    if let Err(err) = attach_cover_art_from_url(audio_path, cover_url).await {
        warn!(
            "[DOWNLOAD] Failed to attach cover art to {} from {}: {}",
            audio_path.display(),
            cover_url,
            err
        );
    }
}

fn ensure_ytdl_solver_dependencies_in_dir(solver_dir: &Path) -> Result<(), String> {
    let meriyah = solver_dir.join("node_modules/meriyah/package.json");
    let astring = solver_dir.join("node_modules/astring/package.json");

    if meriyah.exists() && astring.exists() {
        return Ok(());
    }

    let install_command = if solver_dir.join("package-lock.json").exists() {
        "npm ci --no-fund --no-audit"
    } else {
        "npm install --no-fund --no-audit"
    };

    Err(format!(
        "missing ytdl-audio solver dependencies under {}. Run `{}` during setup before starting the server",
        solver_dir.display(),
        install_command
    ))
}

fn configure_ffmpeg_path_once() {
    #[cfg(target_os = "android")]
    {
        let Some(ffmpeg_dir) = ffmpeg_path_dir_from_env() else {
            return;
        };

        let new_path = match std::env::var("PATH") {
            Ok(current_path) if !current_path.is_empty() => {
                if current_path.split(':').any(|entry| entry == ffmpeg_dir) {
                    return;
                }
                format!("{ffmpeg_dir}:{current_path}")
            }
            _ => ffmpeg_dir.to_string(),
        };

        std::env::set_var("PATH", new_path);
    }
}

fn providers() -> &'static Vec<Arc<dyn MusicProvider>> {
    PROVIDERS.get_or_init(|| {
        vec![
            Arc::new(YoutubeProvider::new()),
            Arc::new(NeteaseProvider::new()),
            Arc::new(BilibiliProvider::new()),
        ]
    })
}

#[cfg(target_os = "android")]
fn ffmpeg_path_dir_from_env() -> Option<String> {
    std::env::var("TAURI_ANDROID_DATA_DIR")
        .ok()
        .map(|data_dir| format!("{data_dir}/files"))
}

impl DownloadSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            DownloadSource::Youtube => "youtube",
            DownloadSource::Netease => "netease",
            DownloadSource::Bilibili => "bilibili",
            DownloadSource::Import => "import",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_online_provider_statuses, create_download_staging_dir,
        ensure_ytdl_solver_dependencies_in_dir, resolve_download_file_stem, sanitize_filename,
        should_embed_cover_art, DownloadJobStore,
    };
    use download_core::{DownloadProgressEvent, DownloadProgressPhase};
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    const YOUTUBE_COOKIE_HEADER_PATH_ENV: &str = "KAULAN_YOUTUBE_COOKIE_HEADER_PATH";

    #[test]
    fn solver_dependency_check_passes_when_required_packages_exist() {
        let temp_dir = tempfile::tempdir().unwrap();
        let meriyah = temp_dir.path().join("node_modules/meriyah");
        let astring = temp_dir.path().join("node_modules/astring");
        fs::create_dir_all(&meriyah).unwrap();
        fs::create_dir_all(&astring).unwrap();
        fs::write(meriyah.join("package.json"), "{}").unwrap();
        fs::write(astring.join("package.json"), "{}").unwrap();

        assert!(ensure_ytdl_solver_dependencies_in_dir(temp_dir.path()).is_ok());
    }

    #[test]
    fn solver_dependency_check_reports_missing_packages_without_running_npm() {
        let temp_dir = tempfile::tempdir().unwrap();
        fs::write(temp_dir.path().join("package-lock.json"), "{}").unwrap();

        let err = ensure_ytdl_solver_dependencies_in_dir(temp_dir.path()).unwrap_err();
        assert!(err.contains("missing ytdl-audio solver dependencies"));
        assert!(err.contains("npm ci --no-fund --no-audit"));
    }

    #[test]
    fn sanitize_filename_replaces_reserved_characters() {
        assert_eq!(sanitize_filename("a:b/c"), "a_b_c");
    }

    #[test]
    fn resolve_download_file_stem_strips_extension() {
        let stem = resolve_download_file_stem(Some("Artist - Song.mp3"), "fallback").unwrap();
        assert_eq!(stem, "Artist - Song");
    }

    #[test]
    fn resolve_download_file_stem_rejects_paths() {
        let err = resolve_download_file_stem(Some("../song"), "fallback").unwrap_err();
        assert!(err.contains("不能包含路径"));
    }

    #[test]
    fn resolve_download_file_stem_rejects_nested_relative_paths() {
        let err = resolve_download_file_stem(Some("Album/Song.mp3"), "fallback").unwrap_err();
        assert!(err.contains("不能包含路径"));
    }

    #[test]
    fn resolve_download_file_stem_rejects_blank_names() {
        let err = resolve_download_file_stem(Some("   "), "fallback").unwrap_err();
        assert!(err.contains("不能为空"));
    }

    #[test]
    fn staging_dir_is_created_under_requested_base() {
        let temp_dir = tempfile::tempdir().unwrap();
        let staging = create_download_staging_dir(temp_dir.path()).unwrap();
        assert!(staging.path().starts_with(temp_dir.path().join(".staging")));
    }

    #[test]
    fn provider_status_reports_youtube_disabled_without_cookie_file() {
        std::env::remove_var(YOUTUBE_COOKIE_HEADER_PATH_ENV);

        let statuses = build_online_provider_statuses();
        let youtube = statuses
            .into_iter()
            .find(|status| status.source == crate::types::DownloadSource::Youtube)
            .unwrap();

        assert!(!youtube.enabled);
        assert!(youtube.summary.contains("not configured"));
    }

    #[test]
    fn cover_art_embedding_skips_ogg_only() {
        assert!(!should_embed_cover_art(Path::new("/tmp/example.ogg")));
        assert!(!should_embed_cover_art(Path::new("/tmp/example.OGG")));
        assert!(should_embed_cover_art(Path::new("/tmp/example.mka")));
        assert!(should_embed_cover_art(Path::new("/tmp/example.mp3")));
        assert!(should_embed_cover_art(Path::new("/tmp/example.flac")));
        assert!(should_embed_cover_art(Path::new("/tmp/example.m4a")));
    }

    #[tokio::test]
    async fn download_job_store_tracks_progress_until_completion() {
        let store = DownloadJobStore::new();
        store
            .create("job-1", crate::types::DownloadSource::Youtube, "Track")
            .await;
        store
            .apply_event(DownloadProgressEvent {
                job_id: "job-1".to_string(),
                source: "youtube".to_string(),
                phase: DownloadProgressPhase::Downloading,
                percent: Some(42),
                message: "Downloading".to_string(),
                detail: Some("42%".to_string()),
            })
            .await;
        store
            .mark_completed("job-1", Some("track.mp3".to_string()), None)
            .await;

        let snapshot = store.get("job-1").await.expect("snapshot should exist");
        assert_eq!(snapshot.state, "completed");
        assert!(matches!(snapshot.phase, DownloadProgressPhase::Completed));
        assert_eq!(snapshot.percent, Some(100));
        assert_eq!(snapshot.filename.as_deref(), Some("track.mp3"));
    }

    #[tokio::test]
    async fn download_job_store_records_failures() {
        let store = DownloadJobStore::new();
        store
            .create("job-2", crate::types::DownloadSource::Bilibili, "Track")
            .await;
        store
            .mark_failed(
                "job-2",
                crate::types::DownloadSource::Bilibili,
                "download failed".to_string(),
            )
            .await;

        let snapshot = store.get("job-2").await.expect("snapshot should exist");
        assert_eq!(snapshot.state, "failed");
        assert!(matches!(snapshot.phase, DownloadProgressPhase::Failed));
        assert_eq!(snapshot.error.as_deref(), Some("download failed"));
    }

    #[tokio::test]
    async fn download_job_store_evicts_terminal_jobs_after_ttl() {
        let store = DownloadJobStore::new();
        store
            .create("job-3", crate::types::DownloadSource::Netease, "Track")
            .await;
        store
            .mark_completed("job-3", Some("track.mp3".to_string()), None)
            .await;

        store.cleanup_with_ttl(Duration::ZERO).await;

        assert!(store.get("job-3").await.is_none());
    }
}
