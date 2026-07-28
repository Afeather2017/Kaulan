# Android MediaStore Integration

## Overview

Kaulan integrates with Android's MediaStore API to scan and play music files on Android devices. This integration is necessary because Android's scoped storage restrictions (introduced in Android 10) prevent direct filesystem access to media files.

For Android playback queue ownership, session polling, and webview restart recovery, see [`docs/android/playback-session.md`](./playback-session.md).

## Problem

On Android 10 (API 29) and later, direct filesystem access to media files is restricted due to scoped storage. A traditional music player that scans directories using `std::fs` cannot:

1. Access media files outside the app-specific directories
2. Scan the user's entire music library stored in standard locations like `/Music` or `/Download`
3. Read audio files using normal file paths

## Solution

Kaulan separates two concerns that used to be conflated:

- **Source** (`backend/src/file_ops/mod.rs`): given a path, who handles read/write/exists I/O? Used by streaming, upload, lyric reading, and ad-hoc folder listing.
  - **`StdFs` source**: `std::fs` / `tokio::fs` for desktop paths and Android app-private filesystem paths
  - **`AndroidMediaStoreContent` source**: the [tauri-plugin-android-mediastore](https://github.com/rustmini/tauri-plugin-android-mediastore) plugin for `content://` paths

- **ScanBackend** (`backend/src/file_ops/mod.rs`): what gets added to the library database? Each backend owns its scan scope — no path argument. The library scan iterates registered backends, not paths.
  - **`StdFsScanBackend { scan_root }`**: walks one filesystem tree (desktop music dir, Android app-private download dir, etc.). Register one per root.
  - **`MediaStoreScanBackend { app_handle }`** (Android only): runs the MediaStore query for all device audio/video. Returns every row MediaStore reports.

The database still stores raw paths (filesystem paths or `content://` URIs), and backend code resolves each path to a `Source` before reading. Library population is the `ScanBackend` registry's job.

## Architecture

```mermaid
sequenceDiagram
    participant App as Tauri App
    participant Server as Rust Server init
    participant Registry as ScanRegistry (in AppState)
    participant Adapter as MediaStore Adapter
    participant Plugin as MediaStore Plugin
    participant MediaStore as Android MediaStore
    participant Resolver as Source Resolver

    Note over App,Registry: App Startup (Android only)
    App->>Adapter: Register MediaStore adapters (set_android_sources)
    App->>Registry: scan_registry.register(MediaStoreScanBackend)
    App->>Server: Start server(music_path, scan_registry)
    Server->>Registry: scan_registry.register(StdFsScanBackend(music_path))
    Server->>Registry: scan_registry.register(StdFsScanBackend(download_root)) if distinct

    Note over App,MediaStore: Music Scanning (initialize_database / update_database)
    Server->>Registry: scan_registry.scan_all(media_types)
    Registry->>Adapter: MediaStoreScanBackend.scan()
    Adapter->>Plugin: get_media_files()
    Plugin->>MediaStore: Query audio content
    MediaStore-->>Plugin: Return audio metadata
    Plugin-->>Adapter: Return AudioFile list
    Adapter-->>Scan: Return MusicFileInfo list
    Scan->>Scan: StdFsScanBackend.scan() walks music_path / download_root
    Scan->>Scan: Dedupe by normalized path
    Scan-->>Server: Combined MusicFileInfo list
    Server->>Server: Populate database

Note: the `ScanRegistry` is owned by `AppState` (`backend/src/types/mod.rs`) and passed into `start_server`. There is no global registry — each server instance (and each test) gets its own.

    Note over App,Resolver: Music Playback
    App->>Resolver: GET /api/music/id/{id}
    Resolver-->>Resolver: resolve(file_path) → Source
    Resolver->>Adapter: read_file("content://...")
    Adapter->>Plugin: file_reader_open()
    Plugin->>MediaStore: Open content URI
    MediaStore-->>Plugin: Session ID
    Plugin-->>Adapter: session_id
    loop Read chunks (1MB)
        Adapter->>Plugin: file_reader_read(size=1MB)
        Plugin->>MediaStore: Read file chunk
        MediaStore-->>Plugin: Base64 data
        Plugin-->>Adapter: Base64 data
        Adapter-->>Resolver: Decoded bytes
    end
    Resolver-->>App: Stream audio
    Adapter->>Plugin: file_reader_close()
```

## Implementation Details

### Backend: Source-Resolved File Operations

**Source: [`backend/src/file_ops/mod.rs`](../../../backend/src/file_ops/mod.rs)**

The backend resolves each raw path through a registry of `Source` implementations. Sources handle I/O — not library population.

#### Source Trait

```rust
#[async_trait]
pub trait Source: Send + Sync {
    fn matches(&self, raw_path: &str) -> bool;
    fn normalize_path(&self, raw_path: &str) -> String;
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, std::io::Error>;
    async fn read_stream(&self, path: &str, chunk_size: usize) -> Result<ByteStream, std::io::Error>;
    async fn get_file_size(&self, path: &str) -> Result<u64, std::io::Error>;
    async fn open_seekable_reader(&self, path: &str) -> Result<Box<dyn ReadSeekSendSync>, std::io::Error>;
    async fn list_music_files(&self, base_path: &str, media_types: &[String]) -> Result<Vec<MusicFileInfo>, std::io::Error>;
    async fn exists(&self, path: &str) -> Result<bool, std::io::Error>;
}
```

- **`StdFs` source** handles desktop paths and Android app-private filesystem paths. `matches` returns true for any non-`content://` path.
- **`AndroidMediaStoreContent` source** handles `content://` paths. `matches` returns true iff the path starts with `content://`.

`Source::list_music_files` still exists for ad-hoc per-path listing (playlist folders, directory tree endpoint). It is **not** how library population works — see `ScanBackend` below.

#### ScanBackend Trait

```rust
#[async_trait]
pub trait ScanBackend: Send + Sync {
    fn id(&self) -> &str;                         // stable identifier for logs
    fn scope(&self) -> String;                    // human-readable scope (path or MediaStore label)
    async fn scan(&self, media_types: &[String]) -> Result<Vec<MusicFileInfo>, io::Error>;
}
```

`ScanBackend`s live inside a `ScanRegistry` (`backend/src/file_ops/mod.rs`), which is owned by `AppState` and passed into `start_server`. The library scan iterates the registry's backends, concatenates their results, and dedupes by normalized path.

- **`StdFsScanBackend { scan_root: PathBuf }`** — recursively walks one filesystem tree. Register one per root.
- **`MediaStoreScanBackend { app_handle }`** (Android only) — calls MediaStore directly. No path argument; MediaStore semantics aren't path-based.

#### MusicFileInfo Structure

```rust
pub struct MusicFileInfo {
    pub path: String,        // Raw path stored in DB
    pub filename: String,    // Generated filename from metadata on Android
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
}
```

### Frontend: MediaStore Adapters

**Source: [`frontend/src-tauri/src/android_media_adapter.rs`](../../../frontend/src-tauri/src/android_media_adapter.rs)**

The adapters are compiled only on Android and provide MediaStore-backed behavior that the backend registers into the source registry.

#### MediaStoreFileReader

Reads file content using Android's content resolver:

```rust
#[cfg(target_os = "android")]
pub struct MediaStoreFileReader {
    app_handle: tauri::AppHandle,
}
```

**Process:**
1. Opens a file reader session with the content URI
2. Reads all data to end (returns base64-encoded data)
3. Closes the session
4. Decodes base64 to bytes and returns

#### MediaStoreMusicFileLister

Queries MediaStore for audio files. Implements `MusicFileLister` for ad-hoc per-path listing; library population goes through `MediaStoreScanBackend::scan` (which calls the same `query_mediastore` helper internally).

```rust
#[cfg(target_os = "android")]
pub struct MediaStoreMusicFileLister {
    app_handle: tauri::AppHandle,
}
```

**Process:**
1. Calls `get_media_files()` on the MediaStore plugin
2. Passes `availability_check = Path` so dead MediaStore rows are filtered during scans without opening every file
3. Receives metadata (title, artist, album, duration, content URI)
4. Generates safe filenames from metadata (e.g., `Artist_Title.mp3`)
5. Returns `MusicFileInfo` list

The `base_path` argument to `list_music_files` is ignored — MediaStore returns rows for the whole device. Per-path filtering was the leaky abstraction removed by the `ScanBackend` refactor.

#### MediaStoreScanBackend

`ScanBackend` implementation that drives library population on Android. Calls `MediaStoreMusicFileLister::query_mediastore` directly without going through the path-based `Source::list_music_files` dispatch.

```rust
#[cfg(target_os = "android")]
pub struct MediaStoreScanBackend {
    app_handle: tauri::AppHandle,
}
```

### Desktop Stub Implementations

The adapters include stub implementations for desktop platforms to allow development and testing:

```rust
#[cfg(not(target_os = "android"))]
pub struct MediaStoreFileReader;

#[cfg(not(target_os = "android"))]
impl FileReader for MediaStoreFileReader {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "MediaStore is only available on Android"
        ))
    }
}
```

## Initialization

**Source: [`frontend/src-tauri/src/lib.rs`](../../../frontend/src-tauri/src/lib.rs)**

The `KaulanServer` struct owns a `ScanRegistry` (`Arc<kaulan::file_ops::ScanRegistry>`) at construction time. MediaStore adapters (I/O dispatch via `Source`s) and the `MediaStoreScanBackend` (library population) are registered against that registry during app setup:

```rust
let kaulan_server = Arc::new(KaulanServer {
    // ...
    scan_registry: std::sync::Arc::new(kaulan::file_ops::ScanRegistry::new()),
    // ...
});

#[cfg(target_os = "android")]
{
    log::info!("Setting up MediaStore adapters for Android");
    let app_handle_for_adapter = app.handle().clone();

    // Source-level I/O dispatch: content:// paths → MediaStore, fs paths → StdFs.
    kaulan::set_android_sources(
        Box::new(android_media_adapter::MediaStoreFileReader::new(app_handle_for_adapter.clone())),
        Box::new(android_media_adapter::MediaStoreMusicFileLister::new(app_handle_for_adapter.clone())),
        Box::new(android_media_adapter::AndroidLyricReader::new(app_handle_for_adapter.clone())),
    );

    // Library scan: MediaStore returns every audio row on the device, independent
    // of any filesystem path. StdFs scan backends for the music_path / download_root
    // are added to the same registry by the Rust server init in backend/src/server/mod.rs.
    kaulan_server.scan_registry.register(std::sync::Arc::new(
        android_media_adapter::MediaStoreScanBackend::new(app_handle_for_adapter),
    ));
    log::info!("MediaStore adapters configured successfully");
}
```

The same `scan_registry` Arc is cloned into the backend thread and passed to `kaulan::start_server(music_dir_arg, scan_registry)`. Inside `start_server`, the Rust side registers one `StdFsScanBackend` per scan root on that same registry after resolving `music_path` and `KAULAN_DOWNLOAD_ROOT`:

```rust
scan_registry.register(Arc::new(file_ops::StdFsScanBackend::new(PathBuf::from(&music_path))));
let download_root = env::var("KAULAN_DOWNLOAD_ROOT").unwrap_or_else(|_| music_path.clone());
if download_root != music_path {
    scan_registry.register(Arc::new(file_ops::StdFsScanBackend::new(PathBuf::from(&download_root))));
}
```

The `AppState` constructed inside `start_server` carries this `scan_registry`, so every handler — and every test that builds an `AppState` — gets its own isolated registry. There is no global state, which is what makes parallel `cargo test` runs safe.

## Android Permissions

**Source: [`frontend/src-tauri/gen/android/app/src/main/AndroidManifest.xml`](../../../frontend/src-tauri/gen/android/app/src/main/AndroidManifest.xml)**

The app requires the `READ_MEDIA_AUDIO` permission for Android 13 (API 33) and later:

```xml
<uses-permission android:name="android.permission.READ_MEDIA_AUDIO" />
```

For Android 12L (API 32) and earlier, the deprecated `READ_EXTERNAL_STORAGE` permission may be used (removed in commit f65dff2).

## Data Storage

On Android, the database stores raw paths instead of forcing one path format:

| Field | Desktop | Android |
|-------|---------|---------|
| `file_path` | `/path/to/music/song.mp3` | `content://media/external/audio/media/123` (from `MediaStoreScanBackend`) or `/data/user/0/<app>/.../Music/song.mp3` (from `StdFsScanBackend` on app_data_dir) |
| `filename` | `song.mp3` | `Artist_Title.mp3` (generated from metadata by MediaStore lister) |

The source resolver normalizes each raw path according to the owning source before scan deduplication and existence checks.

## Usage

### Scanning for Music

When the app starts on Android:

1. `MediaStoreScanBackend.scan()` queries `get_media_files()` for all device audio/video
2. `StdFsScanBackend::scan()` walks the app-private `music_path` and `download_root` (downloaded online tracks land here)
3. `ScanRegistry::scan_all` concatenates both result sets and dedupes by normalized path
4. The scanner populates the database with the combined `MusicFileInfo` list
5. The UI displays the music library

### Playing Music

When a user plays a song:

1. Localhost callers receive the raw database path in the `path` field
2. Remote callers receive the HTTP stream URL in the `path` field
3. Android localhost callers may still add `?stream=content`
4. When `?stream=content` is present, the backend exposes the raw `content://` path in `stream_url` for the Android direct-play backend
5. Remote callers never receive raw MediaStore URIs

### LUFS Pre-caching

When LUFS pre-cache is triggered on Android:

1. Frontend requests `POST /api/music/{id}/precache-lufs`
2. Backend opens a seekable reader via `MediaStoreFileReader` using the content URI
3. LUFS is calculated directly from the reader and cached in the database

### Deleting Stale MediaStore Rows

During `POST /api/database/update`, Kaulan also checks existing database rows and removes songs whose stored `content://` URI is no longer valid.

- The database still stores the raw MediaStore URI in `music.file_path`
- `MediaStoreScanBackend` filters dead rows during scan with `availability_check = Path`
- `MediaStoreFileReader::exists()` uses `check_media_file_availability(..., Open)` for per-row deletion checks

This keeps scan-time filtering cheap while making database cleanup use the stricter "can Android still open this URI?" check.

## Related Source Files

### Backend
- **[`backend/src/file_ops/mod.rs`](../../../backend/src/file_ops/mod.rs)** - `Source` registry, `ScanBackend` trait, `ScanRegistry`, `StdFsScanBackend`
- **[`backend/src/types/mod.rs`](../../../backend/src/types/mod.rs)** - `AppState`, which owns the per-server `Arc<ScanRegistry>`
- **[`backend/src/services/scanner.rs`](../../../backend/src/services/scanner.rs)** - Music scanner using `ScanRegistry::scan_all` and source-backed existence checks
- **[`backend/src/handlers/music.rs`](../../../backend/src/handlers/music.rs)** - Music streaming endpoint using source-backed reader
- **[`backend/src/server/mod.rs`](../../../backend/src/server/mod.rs)** - `start_server(..., scan_registry)` registers `StdFsScanBackend` for `music_path` and `download_root` onto the passed registry

### Frontend
- **[`frontend/src-tauri/src/android_media_adapter.rs`](../../../frontend/src-tauri/src/android_media_adapter.rs)** - MediaStore adapter implementations + `MediaStoreScanBackend`
- **[`frontend/src-tauri/src/lib.rs`](../../../frontend/src-tauri/src/lib.rs)** - App setup; owns `KaulanServer.scan_registry` and registers `MediaStoreScanBackend` alongside `set_android_sources`
- **[`frontend/src-tauri/Cargo.toml`](../../../frontend/src-tauri/Cargo.toml)** - Plugin dependency

### Configuration
- **[`frontend/src-tauri/gen/android/app/src/main/AndroidManifest.xml`](../../../frontend/src-tauri/gen/android/app/src/main/AndroidManifest.xml)** - Android permissions
- **[`frontend/src-tauri/capabilities/default.json`](../../../frontend/src-tauri/capabilities/default.json)** - ACL permissions for MediaStore plugin

## Troubleshooting

### No Music Files Found

**Symptoms:** Database is empty after scanning

**Possible causes:**
1. **MediaStore permission not granted** - Check if the app has `READ_MEDIA_AUDIO` permission
2. **No audio files on device** - MediaStore only returns files that have been indexed by Android
3. **Plugin not initialized** - Check logs for "Setting up MediaStore adapters for Android"

**Solution:** Verify the plugin is initialized in `lib.rs` and permissions are granted.

### File Read Errors

**Symptoms:** Playback fails with file read errors

**Possible causes:**
1. **Content URI changed** - MediaStore URIs can change if the file is moved
2. **File removed** - The audio file was deleted from the device

**Solution:** Trigger a database update via `POST /api/database/update` to refresh the MediaStore data.

### Compilation Errors on Desktop

**Symptoms:** Build fails with MediaStore-related errors

**Possible causes:**
1. Missing `#[cfg(target_os = "android")]` guards
2. Missing stub implementations

**Solution:** Ensure all MediaStore-specific code is properly guarded with `#[cfg(target_os = "android")]` and stub implementations exist for desktop.

## Known Limitations

### Full File Loading Before Streaming

**Current Behavior:**

The current implementation loads entire music files into memory before streaming them to the client. This applies to both desktop and Android platforms:

```
MediaStore/StdFileReader ──► Read Chunk (1MB) ──► Bytes ──► HttpResponse::streaming
        (1MB)                    (buffer)         (send immediately)
```

**Impact:**

| File Type | Typical Size | Memory Usage per Stream |
|-----------|--------------|-------------------------|
| MP3 (320kbps, 5min) | ~5-8 MB | ~1 MB buffer |
| FLAC (5min) | ~20-50 MB | ~1 MB buffer |
| Base64 overhead (Android) | +33% | +~0.33 MB per chunk |

**Consequences:**

1. **Low memory usage** - Fixed-size buffer per stream regardless of file size
2. **Faster playback start** - Audio begins once the first chunk arrives
3. **Bounded base64 overhead** - Overhead applies per chunk, not for entire file
4. **Better concurrency** - Multiple streams scale with chunk size, not file size

**Source Files:**

- [`backend/src/handlers/music.rs`](../../../backend/src/handlers/music.rs) - Streams audio using `read_stream()`
- [`backend/src/file_ops/mod.rs`](../../../backend/src/file_ops/mod.rs) - Defines `read_stream()` on `FileReader`
- [`frontend/src-tauri/src/android_media_adapter.rs`](../../../frontend/src-tauri/src/android_media_adapter.rs) - Uses `file_reader_read()` with 1MB chunks

## Build Configuration

### Tauri Plugin Dependency

**File: [`frontend/src-tauri/Cargo.toml`](../../../frontend/src-tauri/Cargo.toml)**

```toml
[dependencies]
tauri-plugin-android-mediastore = "0.2.3"
```

### ACL Permissions

**File: [`frontend/src-tauri/capabilities/default.json`](../../../frontend/src-tauri/capabilities/default.json)**

```json
{
  "identifier": "default",
  "description": "Default capabilities for the app",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "android-mediastore:default"
  ]
}
```

## Development Notes

### Testing on Desktop

To test the app on desktop during development:

1. The StdFileReader and StdMusicFileLister are used by default
2. No MediaStore plugin is available on desktop
3. Stub implementations allow compilation but return errors when called

### Testing on Android

1. Build the Android app: `cd frontend && npm run tauri android build`
2. Install on a physical device or emulator
3. Grant the READ_MEDIA_AUDIO permission when prompted
4. The app will automatically scan MediaStore on first launch

### Debug Logging

The MediaStore adapters include extensive debug logging:

```rust
log::info!("Querying MediaStore for audio files...");
log::debug!("Found audio file: {} - {} ({})", af.artist, af.title, af.content_uri);
log::info!("MediaStore query complete: {} audio files found", files.len());
```

Enable logging via:
- **Desktop**: `RUST_LOG=debug cargo run`
- **Android**: Check logcat or backend `tracing` output with `RUST_LOG=debug`

## References

- **Commit:** [`07e67c7`](https://github.com/your-repo/commit/07e67c7) - Initial MediaStore integration
- **Follow-up:** [`36376cf`](https://github.com/your-repo/commit/36376cf) - Fix Android READ_MEDIA_AUDIO permission
- **Plugin:** [tauri-plugin-android-mediastore](https://github.com/rustmini/tauri-plugin-android-mediastore)
- **Android Docs:** [MediaStore Overview](https://developer.android.com/training/data-storage/app-specific#best-practices)
