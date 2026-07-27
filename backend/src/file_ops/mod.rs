//! Source-resolved file operations for desktop and Android platforms.
//!
//! The database stores raw paths. Backend-side file access resolves each raw path
//! to a registered source and delegates read/list/write/existence operations there.
//!
//! Related documentation:
//! - `docs/lyric-editing.md`

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, OnceLock, RwLock};
use tracing::debug;

/// Trait object bound for seekable readers used by LUFS calculation.
pub trait ReadSeekSendSync: Read + Seek + Send + Sync {}

impl<T> ReadSeekSendSync for T where T: Read + Seek + Send + Sync {}

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

/// Information about a music file discovered during a source scan.
#[derive(Clone, Debug)]
pub struct MusicFileInfo {
    /// File path or content URI
    pub path: String,
    /// Display filename
    pub filename: String,
    /// Song title (from metadata if available)
    pub title: Option<String>,
    /// Artist name (from metadata if available)
    pub artist: Option<String>,
    /// Album name (from metadata if available)
    pub album: Option<String>,
    /// Duration in milliseconds (from metadata if available)
    pub duration_ms: Option<i64>,
    /// Parent directory name (for folder-based playlists)
    pub parent_dir: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    StdFs,
    AndroidMediaStoreContent,
}

#[derive(Clone, Debug)]
pub struct ResolvedPath {
    pub source_id: &'static str,
    pub raw_path: String,
    pub normalized_path: String,
    pub path_kind: PathKind,
}

#[async_trait]
pub trait Source: Send + Sync {
    fn id(&self) -> &'static str;

    fn path_kind(&self) -> PathKind;

    fn matches(&self, raw_path: &str) -> bool;

    fn normalize_path(&self, raw_path: &str) -> String;

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, io::Error>;

    async fn read_stream(&self, path: &str, chunk_size: usize) -> Result<ByteStream, io::Error>;

    async fn read_stream_from(
        &self,
        path: &str,
        chunk_size: usize,
        start_pos: u64,
    ) -> Result<ByteStream, io::Error>;

    async fn get_file_size(&self, path: &str) -> Result<u64, io::Error>;

    async fn open_seekable_reader(
        &self,
        path: &str,
    ) -> Result<Box<dyn ReadSeekSendSync>, io::Error>;

    async fn list_music_files(
        &self,
        base_path: &str,
        media_types: &[String],
    ) -> Result<Vec<MusicFileInfo>, io::Error>;

    async fn exists(&self, path: &str) -> Result<bool, io::Error>;

    async fn create_dir_all(&self, path: &str) -> Result<(), io::Error>;

    async fn remove_file(&self, path: &str) -> Result<(), io::Error>;

    async fn write_file(&self, path: &str, bytes: &[u8]) -> Result<(), io::Error>;

    async fn write_stream(&self, path: &str, chunks: Vec<Bytes>) -> Result<(), io::Error>;

    async fn read_lyric(
        &self,
        file_path: &str,
        filename: &str,
    ) -> Result<Option<Vec<u8>>, io::Error>;
}

#[derive(Default)]
pub struct SourceRegistry {
    sources: Vec<Arc<dyn Source>>,
}

impl SourceRegistry {
    pub fn register(&mut self, source: Arc<dyn Source>) {
        self.sources.push(source);
    }

    pub fn resolve(&self, raw_path: &str) -> io::Result<ResolvedSource> {
        for source in &self.sources {
            if source.matches(raw_path) {
                return Ok(ResolvedSource {
                    source: Arc::clone(source),
                    resolved: ResolvedPath {
                        source_id: source.id(),
                        raw_path: raw_path.to_string(),
                        normalized_path: source.normalize_path(raw_path),
                        path_kind: source.path_kind(),
                    },
                });
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No source registered for path: {raw_path}"),
        ))
    }
}

pub struct ResolvedSource {
    pub source: Arc<dyn Source>,
    pub resolved: ResolvedPath,
}

static SOURCE_REGISTRY: OnceLock<RwLock<SourceRegistry>> = OnceLock::new();

fn source_registry() -> &'static RwLock<SourceRegistry> {
    SOURCE_REGISTRY.get_or_init(|| {
        let mut registry = SourceRegistry::default();
        registry.register(Arc::new(StdFsSource));
        RwLock::new(registry)
    })
}

pub fn register_source(source: Arc<dyn Source>) {
    let registry = source_registry();
    match registry.write() {
        Ok(mut registry) => registry.register(source),
        Err(err) => tracing::error!("Source registry lock poisoned: {}", err),
    }
}

pub fn resolve_path(raw_path: &str) -> io::Result<ResolvedPath> {
    source_registry()
        .read()
        .map_err(|e| io::Error::other(format!("source registry poisoned: {e}")))?
        .resolve(raw_path)
        .map(|resolved| resolved.resolved)
}

fn resolve_source(raw_path: &str) -> io::Result<ResolvedSource> {
    source_registry()
        .read()
        .map_err(|e| io::Error::other(format!("source registry poisoned: {e}")))?
        .resolve(raw_path)
}

pub fn normalize_path(raw_path: &str) -> String {
    resolve_source(raw_path)
        .map(|resolved| resolved.resolved.normalized_path)
        .unwrap_or_else(|_| raw_path.to_string())
}

pub struct SourceBackedFileReader;

#[async_trait]
pub trait FileReader: Send + Sync {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, std::io::Error>;
    async fn read_stream(
        &self,
        path: &str,
        chunk_size: usize,
    ) -> Result<ByteStream, std::io::Error>;
    async fn read_stream_from(
        &self,
        path: &str,
        chunk_size: usize,
        start_pos: u64,
    ) -> Result<ByteStream, std::io::Error>;
    async fn get_file_size(&self, path: &str) -> Result<u64, std::io::Error>;
    async fn exists(&self, path: &str) -> Result<bool, std::io::Error>;
    async fn open_seekable_reader(
        &self,
        path: &str,
    ) -> Result<Box<dyn ReadSeekSendSync>, std::io::Error>;
}

#[async_trait]
impl FileReader for SourceBackedFileReader {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        resolve_source(path)?.source.read_file(path).await
    }

    async fn read_stream(
        &self,
        path: &str,
        chunk_size: usize,
    ) -> Result<ByteStream, std::io::Error> {
        resolve_source(path)?
            .source
            .read_stream(path, chunk_size)
            .await
    }

    async fn read_stream_from(
        &self,
        path: &str,
        chunk_size: usize,
        start_pos: u64,
    ) -> Result<ByteStream, std::io::Error> {
        resolve_source(path)?
            .source
            .read_stream_from(path, chunk_size, start_pos)
            .await
    }

    async fn get_file_size(&self, path: &str) -> Result<u64, std::io::Error> {
        resolve_source(path)?.source.get_file_size(path).await
    }

    async fn exists(&self, path: &str) -> Result<bool, std::io::Error> {
        resolve_source(path)?.source.exists(path).await
    }

    async fn open_seekable_reader(
        &self,
        path: &str,
    ) -> Result<Box<dyn ReadSeekSendSync>, std::io::Error> {
        resolve_source(path)?
            .source
            .open_seekable_reader(path)
            .await
    }
}

pub struct SourceBackedMusicFileLister;

#[async_trait]
pub trait MusicFileLister: Send + Sync {
    async fn list_music_files(
        &self,
        base_path: &str,
        media_types: &[String],
    ) -> Result<Vec<MusicFileInfo>, std::io::Error>;
}

#[async_trait]
impl MusicFileLister for SourceBackedMusicFileLister {
    async fn list_music_files(
        &self,
        base_path: &str,
        media_types: &[String],
    ) -> Result<Vec<MusicFileInfo>, std::io::Error> {
        resolve_source(base_path)?
            .source
            .list_music_files(base_path, media_types)
            .await
    }
}

pub struct SourceBackedLyricReader;

#[async_trait]
pub trait LyricReader: Send + Sync {
    async fn read_lyric(
        &self,
        file_path: &str,
        filename: &str,
    ) -> Result<Option<Vec<u8>>, io::Error>;
}

#[async_trait]
impl LyricReader for SourceBackedLyricReader {
    async fn read_lyric(
        &self,
        file_path: &str,
        filename: &str,
    ) -> Result<Option<Vec<u8>>, io::Error> {
        resolve_source(file_path)?
            .source
            .read_lyric(file_path, filename)
            .await
    }
}

static SOURCE_BACKED_FILE_READER: SourceBackedFileReader = SourceBackedFileReader;
static SOURCE_BACKED_MUSIC_FILE_LISTER: SourceBackedMusicFileLister = SourceBackedMusicFileLister;
static SOURCE_BACKED_LYRIC_READER: SourceBackedLyricReader = SourceBackedLyricReader;

pub fn get_file_reader() -> &'static dyn FileReader {
    &SOURCE_BACKED_FILE_READER
}

pub fn get_music_file_lister() -> &'static dyn MusicFileLister {
    &SOURCE_BACKED_MUSIC_FILE_LISTER
}

pub fn get_lyric_reader() -> &'static dyn LyricReader {
    &SOURCE_BACKED_LYRIC_READER
}

/// Compatibility hook: wraps a custom reader as a source with content URI matching.
pub fn set_file_reader(reader: Box<dyn FileReader>) -> Result<(), Box<dyn FileReader>> {
    register_source(Arc::new(CompatContentSource::new(Some(reader), None, None)));
    Ok(())
}

/// Compatibility hook: wraps a custom lister as a source with content URI matching.
pub fn set_music_file_lister(
    lister: Box<dyn MusicFileLister>,
) -> Result<(), Box<dyn MusicFileLister>> {
    register_source(Arc::new(CompatContentSource::new(None, Some(lister), None)));
    Ok(())
}

/// Compatibility hook: wraps a custom lyric reader as a source with content URI matching.
pub fn set_lyric_reader(reader: Box<dyn LyricReader>) -> Result<(), Box<dyn LyricReader>> {
    register_source(Arc::new(CompatContentSource::new(None, None, Some(reader))));
    Ok(())
}

/// Register Android MediaStore adapters as a single consolidated source.
///
/// All three adapters share one `CompatContentSource` so any of them can be
/// reached for paths under Android's scoped storage (`/storage`, `/sdcard`)
/// or `content://` URIs. Registering them separately via `set_file_reader`,
/// `set_music_file_lister`, and `set_lyric_reader` would create three
/// sources that each match the same paths — the first-registered one would
/// shadow the others and break operations whose adapter lives on a later
/// instance (e.g., scanning would resolve `/storage` to the file_reader
/// instance, which has no lister, instead of the music_lister instance).
pub fn set_android_sources(
    file_reader: Box<dyn FileReader>,
    music_lister: Box<dyn MusicFileLister>,
    lyric_reader: Box<dyn LyricReader>,
) {
    register_source(Arc::new(CompatContentSource::new(
        Some(file_reader),
        Some(music_lister),
        Some(lyric_reader),
    )));
}

pub async fn source_exists(path: &str) -> Result<bool, io::Error> {
    resolve_source(path)?.source.exists(path).await
}

pub async fn source_create_dir_all(path: &str) -> Result<(), io::Error> {
    resolve_source(path)?.source.create_dir_all(path).await
}

pub async fn source_remove_file(path: &str) -> Result<(), io::Error> {
    resolve_source(path)?.source.remove_file(path).await
}

pub async fn source_write_file(path: &str, bytes: &[u8]) -> Result<(), io::Error> {
    resolve_source(path)?.source.write_file(path, bytes).await
}

pub async fn source_write_stream(path: &str, chunks: Vec<Bytes>) -> Result<(), io::Error> {
    resolve_source(path)?
        .source
        .write_stream(path, chunks)
        .await
}

/// Supported audio file extensions scanned into the user library.
pub const SUPPORTED_EXTENSIONS: &[&str] =
    &["mp3", "ogg", "wav", "aac", "flac", "m4a", "opus", "mka"];

/// Supported video file extensions
pub const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov", "3gp"];

/// Check if a file extension matches the enabled media types
pub fn is_supported_extension(ext: &str, media_types: &[String]) -> bool {
    let ext_lower = ext.to_lowercase();
    let audio_enabled = media_types.iter().any(|t| t == "audio");
    let video_enabled = media_types.iter().any(|t| t == "video");

    (audio_enabled && SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str()))
        || (video_enabled && VIDEO_EXTENSIONS.contains(&ext_lower.as_str()))
}

/// Check if a file path points to a video file
pub fn is_video_file(file_path: &str) -> bool {
    if let Some(ext) = Path::new(file_path).extension().and_then(|e| e.to_str()) {
        return VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str());
    }

    if file_path.starts_with("content://") && file_path.contains("/video/") {
        return true;
    }

    false
}

pub struct StdFsSource;

#[async_trait]
impl Source for StdFsSource {
    fn id(&self) -> &'static str {
        "std-fs"
    }

    fn path_kind(&self) -> PathKind {
        PathKind::StdFs
    }

    fn matches(&self, raw_path: &str) -> bool {
        if raw_path.starts_with("content://") {
            return false;
        }
        // On Android, scoped-storage paths under /storage and /sdcard are
        // claimed by CompatContentSource (registered via set_android_sources)
        // so they route through the MediaStore-backed lister/reader. StdFs
        // cannot enumerate them under scoped storage even with
        // READ_MEDIA_AUDIO granted — the directory appears empty.
        #[cfg(target_os = "android")]
        {
            if raw_path.starts_with("/storage") || raw_path.starts_with("/sdcard") {
                return false;
            }
        }
        true
    }

    fn normalize_path(&self, raw_path: &str) -> String {
        Path::new(raw_path)
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| raw_path.to_string())
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, io::Error> {
        debug!("StdFsSource::read_file called with path: {}", path);
        let path = path.to_string();
        tokio::task::spawn_blocking(move || fs::read(path))
            .await
            .map_err(|e| io::Error::other(e.to_string()))?
    }

    async fn read_stream(&self, path: &str, chunk_size: usize) -> Result<ByteStream, io::Error> {
        let file = tokio::fs::File::open(path).await?;
        Ok(Box::pin(tokio_util::io::ReaderStream::with_capacity(
            file, chunk_size,
        )))
    }

    async fn read_stream_from(
        &self,
        path: &str,
        chunk_size: usize,
        start_pos: u64,
    ) -> Result<ByteStream, io::Error> {
        let mut file = tokio::fs::File::open(path).await?;
        use tokio::io::{AsyncSeekExt, SeekFrom};
        file.seek(SeekFrom::Start(start_pos)).await?;
        Ok(Box::pin(tokio_util::io::ReaderStream::with_capacity(
            file, chunk_size,
        )))
    }

    async fn get_file_size(&self, path: &str) -> Result<u64, io::Error> {
        Ok(tokio::fs::metadata(path).await?.len())
    }

    async fn open_seekable_reader(
        &self,
        path: &str,
    ) -> Result<Box<dyn ReadSeekSendSync>, io::Error> {
        let path = path.to_string();
        let file = tokio::task::spawn_blocking(move || std::fs::File::open(path))
            .await
            .map_err(|e| io::Error::other(e.to_string()))??;
        Ok(Box::new(file))
    }

    async fn list_music_files(
        &self,
        base_path: &str,
        media_types: &[String],
    ) -> Result<Vec<MusicFileInfo>, io::Error> {
        let base_path = base_path.to_string();
        let media_types = media_types.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut audio_files = Vec::new();
            scan_directory_recursive_sync(Path::new(&base_path), &mut audio_files, &media_types);
            Ok(audio_files)
        })
        .await
        .map_err(|e| io::Error::other(e.to_string()))?
    }

    async fn exists(&self, path: &str) -> Result<bool, io::Error> {
        Ok(tokio::fs::metadata(path).await.is_ok())
    }

    async fn create_dir_all(&self, path: &str) -> Result<(), io::Error> {
        tokio::fs::create_dir_all(path).await
    }

    async fn remove_file(&self, path: &str) -> Result<(), io::Error> {
        tokio::fs::remove_file(path).await
    }

    async fn write_file(&self, path: &str, bytes: &[u8]) -> Result<(), io::Error> {
        tokio::fs::write(path, bytes).await
    }

    async fn write_stream(&self, path: &str, chunks: Vec<Bytes>) -> Result<(), io::Error> {
        let path = path.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), io::Error> {
            let mut file = std::fs::File::create(path)?;
            for chunk in chunks {
                file.write_all(&chunk)?;
            }
            Ok(())
        })
        .await
        .map_err(|e| io::Error::other(e.to_string()))?
    }

    async fn read_lyric(
        &self,
        file_path: &str,
        _filename: &str,
    ) -> Result<Option<Vec<u8>>, io::Error> {
        for lyric_path in lyric_candidate_paths(file_path) {
            let candidate = lyric_path.clone();
            match tokio::task::spawn_blocking(move || std::fs::read(candidate))
                .await
                .map_err(|e| io::Error::other(e.to_string()))?
            {
                Ok(content) => return Ok(Some(content)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            }
        }

        Ok(None)
    }
}

fn scan_directory_recursive_sync(
    dir_path: &Path,
    files: &mut Vec<MusicFileInfo>,
    media_types: &[String],
) {
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if let Some(extension) = path.extension() {
                        let ext_str = extension.to_string_lossy().to_lowercase();
                        if is_supported_extension(&ext_str, media_types) {
                            let Some(file_name) = path.file_name() else {
                                continue;
                            };
                            let filename = file_name.to_string_lossy().to_string();
                            let absolute_path = path
                                .canonicalize()
                                .unwrap_or_else(|_| path.clone())
                                .to_string_lossy()
                                .to_string();

                            files.push(MusicFileInfo {
                                path: absolute_path,
                                filename,
                                title: None,
                                artist: None,
                                album: None,
                                duration_ms: None,
                                parent_dir: dir_path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string()),
                            });
                        }
                    }
                } else if file_type.is_dir() {
                    scan_directory_recursive_sync(&entry.path(), files, media_types);
                }
            }
        }
    }
}

pub(crate) fn lyric_candidate_paths(file_path: &str) -> Vec<String> {
    let base_path = Path::new(file_path);

    ["lrc", "vtt"]
        .iter()
        .map(|extension| {
            base_path
                .with_extension(extension)
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

struct CompatContentSource {
    file_reader: Option<Box<dyn FileReader>>,
    music_lister: Option<Box<dyn MusicFileLister>>,
    lyric_reader: Option<Box<dyn LyricReader>>,
}

impl CompatContentSource {
    fn new(
        file_reader: Option<Box<dyn FileReader>>,
        music_lister: Option<Box<dyn MusicFileLister>>,
        lyric_reader: Option<Box<dyn LyricReader>>,
    ) -> Self {
        Self {
            file_reader,
            music_lister,
            lyric_reader,
        }
    }

    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "operation not supported by compatibility content source",
        )
    }
}

#[async_trait]
impl Source for CompatContentSource {
    fn id(&self) -> &'static str {
        "compat-content"
    }

    fn path_kind(&self) -> PathKind {
        PathKind::AndroidMediaStoreContent
    }

    fn matches(&self, raw_path: &str) -> bool {
        if raw_path.starts_with("content://") {
            return true;
        }
        // On Android, also claim scoped-storage filesystem paths so they
        // route through the MediaStore lister/reader rather than StdFsSource
        // (which sees an empty directory under scoped storage). The lister's
        // should_scan_with_filesystem() still routes app-private dirs
        // (/storage/emulated/0/Android/data/...) back through std::fs.
        #[cfg(target_os = "android")]
        {
            if raw_path.starts_with("/storage") || raw_path.starts_with("/sdcard") {
                return true;
            }
        }
        false
    }

    fn normalize_path(&self, raw_path: &str) -> String {
        raw_path.to_string()
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, io::Error> {
        match &self.file_reader {
            Some(reader) => reader.read_file(path).await,
            None => Err(Self::unsupported()),
        }
    }

    async fn read_stream(&self, path: &str, chunk_size: usize) -> Result<ByteStream, io::Error> {
        match &self.file_reader {
            Some(reader) => reader.read_stream(path, chunk_size).await,
            None => Err(Self::unsupported()),
        }
    }

    async fn read_stream_from(
        &self,
        path: &str,
        chunk_size: usize,
        start_pos: u64,
    ) -> Result<ByteStream, io::Error> {
        match &self.file_reader {
            Some(reader) => reader.read_stream_from(path, chunk_size, start_pos).await,
            None => Err(Self::unsupported()),
        }
    }

    async fn get_file_size(&self, path: &str) -> Result<u64, io::Error> {
        match &self.file_reader {
            Some(reader) => reader.get_file_size(path).await,
            None => Err(Self::unsupported()),
        }
    }

    async fn open_seekable_reader(
        &self,
        path: &str,
    ) -> Result<Box<dyn ReadSeekSendSync>, io::Error> {
        match &self.file_reader {
            Some(reader) => reader.open_seekable_reader(path).await,
            None => Err(Self::unsupported()),
        }
    }

    async fn list_music_files(
        &self,
        base_path: &str,
        media_types: &[String],
    ) -> Result<Vec<MusicFileInfo>, io::Error> {
        match &self.music_lister {
            Some(lister) => lister.list_music_files(base_path, media_types).await,
            None => Err(Self::unsupported()),
        }
    }

    async fn exists(&self, path: &str) -> Result<bool, io::Error> {
        match &self.file_reader {
            Some(reader) => reader.exists(path).await,
            None => Err(Self::unsupported()),
        }
    }

    async fn create_dir_all(&self, _path: &str) -> Result<(), io::Error> {
        Err(Self::unsupported())
    }

    async fn remove_file(&self, _path: &str) -> Result<(), io::Error> {
        Err(Self::unsupported())
    }

    async fn write_file(&self, _path: &str, _bytes: &[u8]) -> Result<(), io::Error> {
        Err(Self::unsupported())
    }

    async fn write_stream(&self, _path: &str, _chunks: Vec<Bytes>) -> Result<(), io::Error> {
        Err(Self::unsupported())
    }

    async fn read_lyric(
        &self,
        file_path: &str,
        filename: &str,
    ) -> Result<Option<Vec<u8>>, io::Error> {
        match &self.lyric_reader {
            Some(reader) => reader.read_lyric(file_path, filename).await,
            None => Err(Self::unsupported()),
        }
    }
}

pub fn is_std_fs_path(path: &str) -> bool {
    !path.starts_with("content://")
}

pub fn is_content_uri(path: &str) -> bool {
    path.starts_with("content://")
}

pub fn join_relative_path(base: &str, relative: &str) -> PathBuf {
    Path::new(base).join(relative)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use std::io::{Read, Seek, SeekFrom};

    #[tokio::test]
    async fn test_std_file_reader() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"Hello, World!").unwrap();

        let content = get_file_reader()
            .read_file(file_path.to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(content, b"Hello, World!");
    }

    #[tokio::test]
    async fn test_std_file_reader_stream() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("stream.bin");
        let data = vec![0_u8; 1024 * 1024 + 17];
        fs::write(&file_path, &data).unwrap();

        let mut stream = get_file_reader()
            .read_stream(file_path.to_str().unwrap(), 1024 * 1024)
            .await
            .unwrap();
        let mut collected = Vec::new();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.unwrap();
            collected.extend_from_slice(&bytes);
        }

        assert_eq!(collected, data);
    }

    #[test]
    fn test_supported_extensions() {
        assert!(SUPPORTED_EXTENSIONS.contains(&"mp3"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"flac"));
        assert!(SUPPORTED_EXTENSIONS.contains(&"mka"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&"m4s"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&"webm"));
        assert!(!SUPPORTED_EXTENSIONS.contains(&"txt"));
    }

    #[tokio::test]
    async fn test_std_file_reader_seekable_reader() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("seekable.txt");
        fs::write(&file_path, b"abcdef").unwrap();

        let mut file = get_file_reader()
            .open_seekable_reader(file_path.to_str().unwrap())
            .await
            .unwrap();

        file.seek(SeekFrom::Start(2)).unwrap();
        let mut buf = [0_u8; 2];
        let read = file.read(&mut buf).unwrap();
        assert_eq!(read, 2);
        assert_eq!(&buf, b"cd");
    }

    #[test]
    fn normalize_content_uri_is_stable() {
        let path = "content://media/external/audio/media/42";
        assert_eq!(normalize_path(path), path);
    }

    #[tokio::test]
    async fn source_exists_reports_existing_and_missing_std_fs_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let existing = temp_dir.path().join("exists.txt");
        let missing = temp_dir.path().join("missing.txt");
        fs::write(&existing, b"ok").unwrap();

        assert!(source_exists(existing.to_str().unwrap()).await.unwrap());
        assert!(!source_exists(missing.to_str().unwrap()).await.unwrap());
    }
}
