//! Download provider registry and runtime bootstrap.

mod bilibili;
mod netease;
mod youtube;

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once, OnceLock};

use crate::types::{
    DownloadPreviewRequest, DownloadSource, DownloadTrackRequest, OnlineProviderStatus,
    OnlineSearchResult,
};

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
}

#[async_trait(?Send)]
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
    async fn download_full(
        &self,
        request: &DownloadTrackRequest,
        target_dir: &Path,
    ) -> Result<FullDownloadResult, String>;
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

pub(crate) fn simple_hash(value: &str) -> u64 {
    value.bytes().fold(5381_u64, |acc, byte| {
        acc.wrapping_mul(33).wrapping_add(u64::from(byte))
    })
}

pub fn configure_ffmpeg_path_for_process() {
    FFMPEG_PATH_INIT.call_once(configure_ffmpeg_path_once);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_online_provider_statuses, create_download_staging_dir,
        ensure_ytdl_solver_dependencies_in_dir, sanitize_filename,
    };
    use std::fs;

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
}
