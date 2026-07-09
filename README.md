# Kaulan - Music Player

A modern music player built with Rust (Actix Web) backend and Vue.js (TypeScript) frontend. Features a mobile-friendly interface with playlist management, audio streaming, and LUFS volume normalization.

## Features

- **Music Library Management** - Automatically scans and organizes music files
- **Mobile-First Design** - Responsive UI optimized for mobile devices
- **Android Support** - Native Android app using MediaStore API for music access
- **Android Playback Session** - Queue and current playback state survive webview recreation
- **Android Notification Cover Art** - Android playback notifications reuse embedded cover art from the backend cover endpoint when available
- **Audio Streaming** - Direct streaming from server to browser with position-based seeking
- **File System Playlists** - Automatic playlist creation from folder structure
- **Collection Management** - User-defined playlists/collections stored locally in the browser
- **Volume Normalization** - LUFS support for consistent audio levels
- **Lyric Display & Timing Edits** - Show synchronized LRC/WEBVTT lyrics and shift writable sidecar timing from the player
- **Real-time Search** - Search across all songs instantly
- **Device Discovery** - Automatic discovery of Kaulan instances on local network via UDP broadcast
- **Online Search & Download** - Search YouTube, Netease, and Bilibili from the app, preview tracks, and download them with optional Netease lyrics
- **Standalone Web Hosting** - The backend can serve the built Vue app from `frontend/dist`
- **Shared Song Links** - Open `http://server_ip/?id={songId}` in a browser to load the player from that server and start the shared song

## Quick Start

### Prerequisites

- Rust 1.70+ and Cargo
- Node.js 18+ and npm
- FFmpeg (for LUFS calculation during database updates)
- Android SDK/NDK when building the Android app
- Music files organized in folders

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd kaulan

# Install frontend dependencies
cd frontend
npm install

# Build backend
cd ../backend
cargo build
```

On Windows, provision FFmpeg through the repository helper before the first Rust
build:

```powershell
pwsh -File .\scripts\setup-windows-vcpkg.ps1
$env:VCPKG_ROOT = (Resolve-Path .\.cache\vcpkg)
$env:VCPKGRS_DYNAMIC = '1'
cargo build
```

The Rust build reuses the vendored `vendor/rusty_ffmpeg/src/binding.rs`, so a
separate LLVM or `libclang` installation is not required on Windows anymore.

### Running the Application

The music directory can be provided via:

1. **CLI argument** (highest priority) - `cargo run -- run /path/to/music`
2. **Config file** - `~/.config/kaulan/config.json`
3. **Environment variable** - `KAULAN_MUSIC_DIR`

If none of the above are configured, the application will abort with an error message.

```bash
cd backend

# Option 1: Provide music directory via CLI argument
cargo run -- run /path/to/music

# Option 2: Use config file (see Configuration section below)
# The application will read from ~/.config/kaulan/config.json

# Option 3: Use environment variable
KAULAN_MUSIC_DIR=/path/to/music cargo run -- run
```

The backend API will start on `http://localhost:2080`

To serve the production web app from the backend, build the frontend first:

```bash
cd frontend
npm run build
cd ../backend
cargo run -- run /path/to/music
```

Then open `http://localhost:2080/`. The API remains available under
`http://localhost:2080/api`. See `docs/static-frontend-serving.md` for the full
request flow and custom `KAULAN_FRONTEND_DIST` deployment option.

Shared song links use the same backend-served web app:

```text
http://localhost:2080/?id=42
```

When a browser opens this URL, the frontend uses the page origin plus `/api` as
its session source, so `http://192.168.1.20:2080/?id=42` resolves from
`http://192.168.1.20:2080/api`. It opens the player panel and attempts to start
playback immediately. If the browser blocks autoplay, the page stays on the
selected song and shows a manual play button. See
`docs/shared-song-links.md` for the flow details.

### Standalone Online Search Auth

Standalone backend mode can import one auth file per online source at startup:

- `--youtube-cookie-file <path>` for a YouTube Netscape cookie jar
- `--netease-session-file <path>` for a Netease `session.json`
- `--bilibili-session-file <path>` for a Bilibili `bilibili_session.json`

Example:

```bash
cd backend
cargo run -- run /path/to/music \
  --youtube-cookie-file /path/to/youtube-cookies.txt \
  --netease-session-file /path/to/netease-session.json \
  --bilibili-session-file /path/to/bilibili-session.json
```

This is intended for standalone server usage where you already have provider login data and want online search/download without relying on the Tauri login capture flow. See `docs/online-search-download.md` for provider file details.

Standalone backend mode still relies on the vendored `ytdl-audio` Node.js solver for YouTube cipher resolution. Tauri desktop and Android builds use the embedded webview solver path instead.

If Netease requests must exit through a China-based proxy, set `NETEASE_RELAY_URL` before starting the backend. This only affects the vendored `netease-api` client used for Netease search, lyric lookup, preview, and downloads.

```bash
cd backend
NETEASE_RELAY_URL=http://your-cn-proxy:7890 \
cargo run -- run /path/to/music \
  --netease-session-file /path/to/netease-session.json
```

`reqwest` proxy URL schemes supported by this build include `http://`, `https://`, and `socks5://`.

Runtime logs are emitted through `tracing`. Use `RUST_LOG=debug` when you need per-file scan detail during backend runs.

In a separate terminal, start the frontend:

```bash
cd frontend
npm run dev
```

The frontend will be available at `http://localhost:3000`

### Building for Android

Android builds require a staged FFmpeg bundle for the target ABI before running the
main packaging script. The release workflow builds and caches this bundle
automatically before creating Android APK/AAB packages.

Single-target example (`aarch64` / `arm64-v8a`):

```bash
./scripts/build-android-ffmpeg.sh --target aarch64
./build-android.sh --target aarch64
```

Release-style multi-ABI build:

```bash
./scripts/build-android-ffmpeg.sh
./build-android.sh
```

The staged FFmpeg bundle is written under `build/android-ffmpeg/android/...` and is
consumed automatically by the vendored `rusty_ffmpeg` build script during Android
cross-compilation.

## How to Use

### 1. Setting Up Your Music Library

Organize your music files in folders. Each folder becomes a playlist:

```
music/
├── Rock/
│   ├── song1.mp3
│   └── song2.flac
├── Jazz/
│   ├── track1.ogg
│   └── track2.wav
└── Classical/
    └── piece.mp3
```

### 2. Starting the Server

**First time setup** - Create a config file:

```bash
# Create config directory
mkdir -p ~/.config/kaulan

# Create config file with your music directory
echo '{"music_directory": "/path/to/music"}' > ~/.config/kaulan/config.json

# Start the server (will read from config file)
cd backend
cargo run -- run
```

On the first run, the backend performs a one-time automatic scan and stores a flag in the database.
After that, startup scans are skipped. To rescan later, use the Update Database API
(`POST /api/database/update`). See `docs/startup-scan.md` for details.

**Or use CLI argument directly:**

```bash
cd backend
cargo run -- run /path/to/music
```

On first run, the server will:

- Create `music.db` SQLite database in your music directory
- Scan all audio files recursively
- Insert new files into the database

### 3. Updating the Database

To scan for new files and calculate LUFS values:

```bash
cd backend
cargo run update ~/Music
```

This command will:

- Scan for new files and add them to the database
- Calculate LUFS values for files without proper values
- Remove database entries for deleted files

### 4. Using the Web Player

1. Open `http://localhost:3000` in your browser
2. Select a playlist from the sidebar
3. Click on a song to start playback
4. Use the search bar to filter songs
5. Control playback with the audio player at the bottom

## API Reference

### Base URL

```
http://localhost:2080/api
```

### Static Frontend

The backend also serves the built Vue frontend when `frontend/dist/index.html`
exists.

- `GET /` - Serves the web player shell
- `GET /assets/...` - Serves built frontend assets
- `GET /<browser-route>` - Falls back to `index.html` for SPA navigation
- `GET /api/...` - Reserved for JSON and streaming APIs

Build with `cd frontend && npm run build`, then start the backend and open
`http://localhost:2080/`. See `docs/static-frontend-serving.md`.

### Endpoints

#### GET /api/music

Get all music files from the database.

**Response:** `MusicResponse[]`

```json
[
  {
    "id": 1,
    "filename": "song.mp3",
    "file_path": "Rock/song.mp3",
    "lufs": -14.5,
    "created_at": "2024-01-15T10:30:00Z"
  }
]
```

#### GET /api/music/{filename}

Stream an audio file by filename.

**Parameters:**

- `filename` (path parameter) - The filename to stream

**Response:** Audio file binary data (audio/mpeg)

**Headers:**

- `Content-Type: audio/mpeg`
- `Cache-Control: public, max-age=86400, must-revalidate`

#### GET /api/music/id/{id}

Stream an audio file by ID with optional position-based seeking.

#### POST /api/download/search

Search online providers with a merged result list.

#### POST /api/download/preview

Download a temporary preview track and play it as a one-track queue.

#### POST /api/download/lyrics/search

Search Netease lyric candidates for any selected result.

#### POST /api/download/track

Download the selected provider track into the configured online download root and optionally save a matching `.lrc` file beside it.

**Parameters:**

- `id` (path parameter) - The music ID to stream
- `position` (query parameter, optional) - Position in file (0.0 to 1.0)
  - `0.0` = Start of file
  - `0.5` = Middle of file
  - `1.0` = End of file

**Response:** Audio file binary data (audio/mpeg)

**Without position parameter (HTTP 200 OK):**

```http
HTTP/1.1 200 OK
Content-Type: audio/mpeg
Content-Length: 583703350
Accept-Ranges: bytes
Cache-Control: public, max-age=86400, must-revalidate
```

**With position parameter (HTTP 206 Partial Content):**

```http
HTTP/1.1 206 Partial Content
Content-Type: audio/mpeg
Content-Length: 525333015
Content-Range: bytes 58370335-583703349/583703350
Accept-Ranges: bytes
Cache-Control: public, max-age=86400, must-revalidate
X-Seek-Position: 0.1
```

#### GET /api/music/id/{id}/cover

Get embedded cover art for a music file by ID.

**Parameters:**

- `id` (path parameter) - The music ID

**Response:** Embedded image binary data with the detected image content type

**Behavior:**

- Returns `200 OK` when cover art is embedded in the audio metadata
- Returns `404 Not Found` when the file has no embedded cover art
- Used by the Android playback notification to show album artwork when available

**Example:** Stream from 10% position (saves bandwidth for large seeks)

```bash
curl "http://localhost:2080/api/music/id/25?position=0.1"
```

See [`docs/position-based-streaming.md`](docs/position-based-streaming.md) for detailed documentation.
See [`docs/streaming-flow.md`](docs/streaming-flow.md) for the full playback, cover art, and lyric request flow across desktop, Android localhost, and remote clients.

#### GET /api/playlists

Get all playlists with their songs.

**Response:** `Record<string, MusicInfo[]>`

```json
{
  "所有音乐": [
    {
      "name": "song.mp3",
      "lufs": -14.5,
      "path": "Rock/song.mp3"
    }
  ],
  "Rock": [
    {
      "name": "song.mp3",
      "lufs": -14.5,
      "path": "Rock/song.mp3"
    }
  ]
}
```

Note: "所有音乐" (All Music) is a special playlist containing all songs.
See [`docs/streaming-flow.md`](docs/streaming-flow.md) for how playlist metadata is converted into playback URLs.

#### GET /api/playlists/{name}

Get a specific playlist by name.

**Parameters:**

- `name` (path parameter) - The playlist name (folder name or "所有音乐")

**Response:** `Playlist`

```json
{
  "name": "Rock",
  "songs": [
    {
      "name": "song.mp3",
      "lufs": -14.5,
      "path": "Rock/song.mp3"
    }
  ]
}
```

#### GET /api/settings/music-directory

Get the current music directory path.

**Response:** `MusicDirectoryResponse`

```json
{
  "path": "/path/to/music"
}
```

#### POST /api/settings/music-directory

Set the music directory path. The change is saved to a config file and takes effect on the next application restart.

**Request:**

```json
{
  "path": "/new/path/to/music"
}
```

**Response (Success):**

```json
{
  "success": true,
  "message": "Music directory will be set to '/new/path/to/music' on next restart."
}
```

**Response (Error):**

```json
{
  "success": false,
  "message": "Directory does not exist: /invalid/path"
}
```

#### POST /api/database/update

Trigger a database update to scan for new files, update LUFS values, and remove deleted files.

**Response:** `UpdateResponse`

```json
{
  "success": true,
  "message": "Database updated successfully"
}
```

#### GET /api/discovery/devices

Get all Kaulan instances discovered on the local network.

**Response:** `DiscoveredDevice[]`

```json
[
  {
    "device_id": "550e8400-e29b-41d4-a716-446655440001",
    "device_name": "Bedroom Player",
    "api_url": "http://192.168.1.100:2080/api",
    "last_seen_secs_ago": 5
  }
]
```

#### GET /api/discovery/self

Get this device's information.

**Response:** `SelfDevice`

```json
{
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "device_name": "Living Room Player"
}
```

#### POST /api/discovery/name

Set this device's name.

**Request:**

```json
{
  "name": "My New Device Name"
}
```

**Response:**

```json
{
  "success": true,
  "message": "Device name updated"
}
```

#### POST /api/discovery/scan/start

Start a manual discovery scan transaction.

**Response:**

```json
{
  "success": true,
  "message": "Discovery scan started"
}
```

#### POST /api/discovery/request

Send one UDP discovery request packet. The frontend calls this every 1 second for 10 seconds after pressing "刷新设备".

**Response:**

```json
{
  "success": true,
  "message": "Discovery request sent"
}
```

#### POST /api/discovery/scan/finish

Finish a manual discovery scan transaction and commit or rollback scan results.

**Request:**

```json
{
  "success": true
}
```

**Response:**

```json
{
  "success": true,
  "message": "Discovery scan committed"
}
```

### Data Types

```typescript
// Music response with database metadata
interface MusicResponse {
  id: number;
  filename: string;
  file_path: string;
  lufs: number | null;
  created_at: string; // ISO 8601 datetime
}

// Music info for playback
interface MusicInfo {
  name: string;
  lufs: number;
  path: string;
}

// Playlist structure
interface Playlist {
  name: string;
  songs: MusicInfo[];
}
```

## Database Schema

### music Table

The application uses SQLite with SeaORM. The database file (`music.db`) is created in your music directory.

| Column       | Type    | Constraints                 | Description                         |
| ------------ | ------- | --------------------------- | ----------------------------------- |
| `id`         | INTEGER | PRIMARY KEY, AUTO INCREMENT | Unique identifier                   |
| `filename`   | TEXT    | NOT NULL                    | Original filename                   |
| `file_path`  | TEXT    | UNIQUE, NOT NULL            | Relative path from music directory  |
| `lufs`       | REAL    | NULLABLE                    | LUFS value for volume normalization |
| `created_at` | TEXT    | NOT NULL                    | ISO 8601 timestamp (UTC)            |

### Database Location

```
<path-to-music-directory>/music.db
```

### Database Operations

The application performs these operations automatically:

- **On server start**: Scans for new files and inserts them (if `file_path` doesn't exist)
- **On update command**: Scans for new/deleted files, calculates LUFS, updates existing entries

## Supported Audio Formats

- MP3 (`.mp3`)
- OGG Vorbis (`.ogg`)
- WAV (`.wav`)
- AAC (`.aac`)
- FLAC (`.flac`)
- M4A (`.m4a`)
- Opus (`.opus`)

## Configuration

### Config File

The application uses a JSON configuration file to persist the music directory path across restarts.

**Config File Locations:**

| Platform | Standalone Mode                                    | Tauri Mode                                             |
| -------- | -------------------------------------------------- | ------------------------------------------------------ |
| Linux    | `~/.config/kaulan/config.json`                     | `~/.config/<app-name>/config.json`                     |
| macOS    | `~/Library/Application Support/kaulan/config.json` | `~/Library/Application Support/<app-name>/config.json` |
| Windows  | `%APPDATA%\kaulan\config.json`                     | `%APPDATA%\<app-name>\config.json`                     |

**Config Format:**

```json
{
  "music_directory": "/path/to/music",
  "device_id": "550e8400-e29b-41d4-a716-446655440000",
  "device_name": "Living Room Player"
}
```

**Music Directory Priority (highest to lowest):**

1. CLI argument (if provided) - **Overrides config file**
2. Config file (if exists)
3. Environment variable `KAULAN_MUSIC_DIR`
4. **Application aborts** if none of the above are configured

**Note:** The application will no longer fall back to a default directory. If no music directory is configured via CLI argument, config file, or environment variable, the application will abort with an error message.

### Backend Configuration

| Setting         | Default                | Description                        |
| --------------- | ---------------------- | ---------------------------------- |
| HTTP Port       | 2080                   | HTTP API server port               |
| Discovery Port  | 2082                   | UDP device discovery request/reply |
| Bind Address    | 0.0.0.0                | Server bind address                |
| Music Directory | `~/Music` or `./music` | Path to music files                |
| Database        | `music.db`             | SQLite database in music directory |

### Frontend Configuration

The frontend uses Vite with a proxy to the backend:

```typescript
// vite.config.ts
server: {
  port: 3000,
  proxy: {
    '/api': 'http://localhost:2080'
  }
}
```

## Android Build

Kaulan can be built as a native Android app using Tauri's mobile support. The Android version uses the MediaStore API to access music files on the device.

### Building for Android

```bash
cd frontend

# Build Android APK
npm run tauri android build

# For development with live reload
npm run tauri android dev
```

### Android-Specific Features

- **MediaStore Integration** - Uses Android's MediaStore API to scan and play music
- **Plugin-Owned Playback Session** - Android queue, index, runtime state, and play mode are stored in the foreground service plugin
- **Scoped Storage Support** - Compatible with Android 10+ scoped storage restrictions
- **Content URI Streaming** - Reads audio files via content URIs instead of file paths
- **Permission Handling** - Requests `READ_MEDIA_AUDIO` permission on Android 13+

See [`docs/android/mediastore-integration.md`](docs/android/mediastore-integration.md) for detailed technical documentation.
See [`docs/android/playback-session.md`](docs/android/playback-session.md) for Android playback/session behavior.

### Desktop Webview Cookies

The desktop Tauri build can export the live webview cookie jar for YouTube/Google into a Netscape cookie file. This is useful for browser-login flows that need `HttpOnly` cookies.

The export command is implemented in the Rust backend and writes a temporary jar file under the system temp directory.

### Embedded YouTube Solver Assets

Tauri desktop and Android builds embed the browser-side YouTube solver assets during the Tauri build step.

- `frontend/src-tauri/build.rs` downloads fixed versions of `meriyah` and `astring`
- the generated files are bundled into the desktop Tauri build and Android `android_asset` tree
- the runtime hidden webview uses those local files instead of loading a CDN or running `npm install`

Standalone backend mode is unchanged here: it still uses the vendored `ytdl-audio/js/solver.mjs` Node.js helper when no Tauri `JsRunner` is registered.

### Android Permissions

The app requires the following permission:

```xml
<uses-permission android:name="android.permission.READ_MEDIA_AUDIO" />
```

This permission is automatically requested when the app first launches.

## Development

### Backend Development

```bash
cd backend

# Type check without building
cargo check

# Run tests
cargo test

# Build release version
cargo build --release

# Run with debug logging
RUST_LOG=debug cargo run run ~/Music
```

### Frontend Development

```bash
cd frontend

# Development server with hot reload
npm run dev

# Type checking
npm run check

# Build for production
npm run build

# Preview production build
npm run preview

# Run tests
npm run test
```

## Architecture

### Backend (Rust/Actix Web)

```
backend/src/
├── main.rs          # HTTP server, API endpoints, file scanning
├── database/mod.rs  # SQLite connection, table creation
├── entities/
│   ├── music.rs     # Music table entity definition
│   └── mod.rs
└── lufsgen.rs       # FFmpeg-based LUFS calculation
```

### Frontend (Vue.js/TypeScript)

```
frontend/src/
├── main.ts          # Application entry point
├── App.vue          # Main music player component
├── router/
│   └── index.ts     # Vue Router configuration
└── views/
    ├── Home.vue     # Dashboard with statistics
    ├── Library.vue  # Music library view
    └── Playlists.vue # Playlist management view
```

## LUFS Volume Normalization

LUFS (Loudness Units Full Scale) values are calculated using FFmpeg and stored in the database. The application uses these values to normalize playback volume across tracks with different mastering levels.

During the `update` command, LUFS is calculated for:

- New files being added to the database
- Existing files with missing or default (0.5) LUFS values

## Troubleshooting

### Audio files not showing up

1. Ensure files have supported extensions (`.mp3`, `.ogg`, `.wav`, `.aac`, `.flac`)
2. Run the update command: `cargo run update ~/Music`
3. Check server logs for database errors

### Database errors

The database file is created automatically in your music directory. If you encounter corruption:

```bash
# Delete and recreate the database
rm ~/Music/music.db
cargo run run ~/Music
```

### FFmpeg not found

LUFS calculation requires FFmpeg. Install it:

```bash
# Arch Linux
sudo pacman -S ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# macOS
brew install ffmpeg
```

On Windows, use the repo-managed `vcpkg` bootstrap instead of installing FFmpeg
and LLVM manually:

```powershell
pwsh -File .\scripts\setup-windows-vcpkg.ps1
$env:VCPKG_ROOT = (Resolve-Path .\.cache\vcpkg)
$env:VCPKGRS_DYNAMIC = '1'
```
