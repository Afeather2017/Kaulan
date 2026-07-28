# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Kaulan is a music player with a Rust Actix Web backend and Vue.js (TypeScript) frontend. The app features mobile-first UI, audio streaming, folder-based playlists, user-defined collections (favorites), and LUFS volume normalization.

## Develop steps

### New feature

A new feature need change backend and frontend. You should:

#### Check the feature

1. Think if the feature should be provided in the project, if not, refuse.
2. If we don't provide the way we use UI, you should stop from generate. Unless it is nothing to do with UI.

#### Get things done.

1. Figure out what API should be provided, the JSON format, etc. And, use RESTful mostly.
2. Design the database tables.
3. Generate the unit test for the backend. This makes sure you make the API works.
4. After backend done, the test pass, implement the frontend.
5. Brief check. Use curl to access APIs, to check if things work.

### Documentations

Documentations should always in English. When feature is done, you should generate the documentation.

1. README.md, briefly introduced the project, say key features.
2. docs/ , which introduce the feature, how features works(in sequence diagram), how user to use the UI. The features introducton should include all API.
3. API reference, use rust-builtin reference system, this should be done in comments.
4. CLAUDE.md, Introduce the project framework by path.
5. In source code and test files, comment the documentation file. And documentation should contains the related source file path.
6. External code brought in as a git submodule should live under `vendor/`.

### Log

The log system of rust should be tracing. and the ts/js, console.log is enough.

Rules:

| Scenario / Purpose | Recommended Log Level | Explanation & Example |
| ------------------ | --------------------- | --------------------- |
| 1. Development Environment Debugging | DEBUG or TRACE | Used during development to track the program's execution path, internal state, and variable values. Essential for debugging. |
| 2. Production Environment Routine Operation | INFO or WARN | Higher levels are used in production to reduce performance overhead and prevent important messages from being drowned out by less important ones. INFO records key system status and business processes. |
| 3. Recording Key Business Information | INFO | Used for important business milestones such as user login, order creation, or successful payment. This is useful for business analysis and auditing. Example: `User ID:{} logged in successfully`. Music files are large, log if someone access it. |
| 4. Recording Potential Issues | WARN | Used when the system encounters an abnormal situation that can be automatically recovered from or does not immediately affect core functionality. Examples: using a default configuration value, an API call timeout that was retried successfully. Alerts developers to a situation that may need attention but does not require immediate intervention. |
| 5. Recording Errors and Exceptions | ERROR | Used when a system function is impacted and a manual intervention is required. Key point: When logging exceptions, always output the complete exception stack trace, not just `e.getMessage()`, for quick root cause analysis. Example: `An exception occurred while processing user ID:{}`. |
| 6. Controlling Log Volume (in loops or batch operations) | INFO (with conditional control) | Avoid logging on every iteration within a loop. Use conditional checks (e.g., log every 1000 records) or concatenate messages with `StringBuilder`and log once after the loop to reduce I/O pressure. |
| 7. Logging Method Entry/Exit and Call Chains | DEBUG / INFO | For important methods, log parameters at the entry and results at the exit (DEBUG). For long operations, ensure each step in the chain has a log (INFO) to trace the problematic link. The article suggests using AOP for uniform implementation. |
| 8. Avoiding Sensitive Information | All Levels | General Rule: Regardless of the level, log content must never contain sensitive information like user passwords or ID numbers to prevent security issues in case of a log leak. |
| 9. Dynamic Log Level Adjustment | Adjustable | If a problem occurs in production but the current log level (e.g., INFO) doesn't provide enough information, you can temporarily lower the level (e.g., to DEBUG) to get more detailed logs for troubleshooting. |

### Bug fix

Update documentation when fixed

## Rules

1. In the ts/js/html, use tauri feature is strictly forbidden! We need to provide a web.
2. If it is android, use @build-android.sh or @build-android.bat . Ignore the failure if there is no phone found.
3. If it is desktop, use cargo check and npm run build to check.

## Development Commands

### Backend (Rust)
```bash
cd backend

# Run development server (with custom or default music directory)
cargo run /path/to/music/directory  # or just `cargo run` for ./music
cargo build                         # Build
cargo check                         # Type check without building
cargo test                          # Run tests
```

### Frontend (Vue.js/TypeScript)
**CSS Compatibility Notes:**
- **Do not use the `inset` property.** The build process must not convert `top: 0; left: 0; right: 0; bottom: 0;` to `inset: 0` due to old Android webview compatibility.
- **Font Awesome version:** Use v6.7.2 or lower. Font Awesome v7 uses modern CSS features (`:is()` pseudo-class, `font-synthesis` property) not supported in Android WebView 84.x. The caret notation `^6.7.2` in package.json allows patch/minor updates within v6 but prevents accidental upgrade to v7.
- **JavaScript target:** The frontend bundle must stay compatible with Android WebView 84.x. Do not rely on modern syntax such as logical assignment operators (`??=`, `||=`, `&&=`). Keep Vite `build.target` pinned to `chrome84`.
```bash
cd frontend

npm install         # Install dependencies
npm run dev         # Development server (runs on port 3000)
npm run build       # Production build
npm run preview     # Preview production build
npm run test        # Run tests (vitest)
```

### Full Stack Development
- Backend API runs on `http://localhost:2080`
- When `frontend/dist/index.html` exists, the backend also serves the built web app from `http://localhost:2080/`
- Frontend dev server runs on `http://localhost:3000`
- Vite proxy forwards `/api` requests to backend (see `frontend/vite.config.ts`)
- On Android, the Tauri app starts the backend during app setup; the music notification foreground service reuses that server to keep it alive in background.

### Versioning
- The **git tag** (`vX.Y.Z`) is the single source of truth for the release version. Tags are picked up automatically by the Android build scripts (`build-android.sh`, `build-android.bat`), which strip the `v` prefix and inject the version via Tauri's `--config` flag so the bundled APK/AAB reports the tag's version.
- The `version` fields in `frontend/src-tauri/Cargo.toml`, `frontend/src-tauri/tauri.conf.json`, and `frontend/package.json` are **dev-build fallbacks only**. They do not need to be hand-bumped per release — they're used when no git tag is reachable (e.g., local dev builds before any tag exists).
- To cut a release: `git tag vX.Y.Z && git push --tags`, then run `./build-android.sh` (or `build-android.bat` on Windows). The script logs `Injecting release version from git tag: X.Y.Z` before invoking Tauri.
- Tags that don't match `X.Y.Z` (after stripping the `v` prefix) are ignored, and the build falls back to the source-file version.

## Architecture

### Backend Structure
```
backend/src/
├── main.rs          # Binary entry point
├── lib.rs           # Library re-exports and module declarations
├── config/
│   └── mod.rs       # Configuration file management
├── types/
│   └── mod.rs       # Shared request/response types
├── handlers/
│   ├── mod.rs       # Handler exports
│   ├── music.rs     # Music API endpoints
│   ├── playlists.rs # Playlist API endpoints (folder-based)
│   ├── settings.rs  # Settings API endpoints
│   ├── upload.rs    # File upload API endpoints
│   ├── library_import.rs # Remote-library import endpoint (pull tracks from a remote Kaulan server into local download_root)
│   └── database.rs  # Database update API endpoints
├── services/
│   ├── mod.rs       # Service exports
│   └── scanner.rs   # Database update operations driven by registered scan backends
├── server/
│   └── mod.rs       # Server startup logic and static frontend hosting
├── database/mod.rs  # SQLite connection, table creation
├── entities/        # SeaORM entities
│   ├── music.rs           # Music table (id, filename, file_path, lufs, created_at)
│   ├── mod.rs
│   └── prelude.rs
├── file_ops/
│   └── mod.rs       # Source registry, scan-backend registry, path resolver, and backend file operations
└── lufsgen.rs       # FFmpeg-based LUFS analysis utility
```

**Key Architecture Points:**
- Uses SeaORM with SQLite for persistence
- Database file: `music.db` (auto-created)
- `music` table stores all audio files with LUFS values
- Static frontend hosting serves `frontend/dist` at `/` after `npm run build`; `/api/...` remains reserved for backend routes
- Backend scans registered `ScanBackend`s on startup via `initialize_database()`
- Two view modes: folder-based playlists and user-defined collections
- User-defined collections are frontend-only state stored in browser localStorage
- **Source-resolved file operations** - `file_ops` resolves each stored raw path to a source implementation:
  - `StdFs` handles normal filesystem paths on desktop and Android app-private storage
  - `AndroidMediaStoreContent` handles `content://` URIs on Android
  - Callers keep using backend file operation helpers, but dispatch is source-aware
  - See [`docs/android/mediastore-integration.md`](docs/android/mediastore-integration.md) for Android implementation
- **Backend-based library scanning** - `file_ops` keeps library population separate from I/O dispatch:
  - `StdFsScanBackend` scans one configured filesystem root
  - Android registers `MediaStoreScanBackend` to query all device audio through MediaStore
  - Backends live in a per-server `ScanRegistry` owned by `AppState` (not a global) — each `start_server` invocation and each test gets its own
  - `scanner::initialize_database(db, &registry)` and `scanner::update_database(db, &registry)` iterate the registry via `ScanRegistry::scan_all`

### Frontend Structure
```
frontend/src/
├── main.ts                 # App entry point and Pinia registration
├── App.vue                 # Thin root shell that renders layout and binds the app-shell composable
├── components/
│   ├── AppContentView.vue     # Library / collection / search content area
│   ├── AppPlayerView.vue      # Cover, lyrics, and desktop player panel
│   ├── AppActionSheets.vue    # Source, collection, and song action sheets
│   ├── LibrarySourceListView.vue # Source-aware playlist list
│   ├── PlaylistListView.vue   # Collection playlist list
│   ├── PlayerControls.vue     # Shared playback controls
│   ├── SearchBar.vue          # Top search input
│   ├── SongListView.vue       # Shared song list renderer
│   └── modals/
│       ├── ActiveQueueModal.vue     # Current playback queue
│       ├── AddDeviceModal.vue       # Manual device/source management
│       ├── AddToCollectionModal.vue # Add songs to local collections
│       ├── CreateCollectionModal.vue # Create collection dialog
│       ├── OnlineSearchModal.vue    # Remote provider search and download
│       ├── SettingsModal.vue        # Settings panel and database actions
│       └── UploadModal.vue          # File upload modal
├── composables/
│   ├── useAppShell.ts       # Top-level shell orchestration for the root component
│   ├── useAppShellLayout.ts # Wide-layout state, player panel presentation, and cover fallback handling
│   ├── useAndroidBackNavigation.ts # Android back-button state machine and listener registration
│   ├── useAudioPlayer.ts      # Playback state, queue persistence, Android player integration
│   ├── useCollections.ts      # Local collection persistence, CRUD, and collection modal state
│   ├── useLibrarySources.ts   # Multi-source library loading, filtering, and search
│   ├── useLufs.ts             # LUFS pre-cache requests, polling, and metadata patching
│   ├── useLyrics.ts           # LRC loading and lyric sync state
│   ├── useQueueEditing.ts     # Queue insertion helpers for song action-sheet operations
│   ├── useSelection.ts        # Shared multi-select behavior
│   ├── useTimer.ts            # Sleep timer behavior
│   └── useVolume.ts           # Volume and normalization settings
├── stores/
│   ├── collections.ts       # Shared local collection state and collection modal/menu state
│   ├── library.ts           # Shared library source, filter, and search state
│   ├── player.ts            # Shared playback, normalization, timer, and LUFS visibility state
│   └── ui.ts                # Shared shell navigation, modal visibility, and selected playlist state
├── types/
│   └── library.ts             # Source-group and capability models for the UI
└── utils/
    ├── api.ts                 # API base resolution for local and remote sources
    ├── discovery.ts           # Manual/discovered device source helpers
    ├── platform.ts            # Runtime profile and capability checks for web and Android
    ├── sourceGroups.ts        # Incremental source-group loading helpers
    ├── storage.ts             # localStorage persistence for settings and collections
    └── validation.ts          # URL validation utilities

frontend/src-tauri/src/
├── lib.rs              # Tauri app setup, MediaStore adapter initialization
└── mediastore_adapter.rs # MediaStore implementations for Android (FileReader, MusicFileLister)
```

Desktop Tauri builds hide the main window to the system tray on close/minimize
and expose Show/Quit through the tray menu. See
[`docs/system-tray.md`](docs/system-tray.md) for the desktop behavior and source
flow.

**Note:** The active frontend is still mounted directly from `App.vue`. Navigation is implemented as internal view state and source-aware lists, not Vue Router pages, but most shared state now lives in Pinia stores and shell composables instead of the root component.

### Data Flow

**Folder Mode (default):**
1. Backend scans registered backends on startup → populates `music` table
2. Frontend calls `GET /api/playlists` → gets folder-based playlist structure
3. Localhost callers receive raw playback paths; remote callers receive HTTP stream URLs
4. Remote playback uses `GET /api/music/{filename}` or `GET /api/music/id/{id}` → backend resolves raw path to source and streams audio
5. LUFS values are stored in DB

**Collection Mode:**
1. Frontend reads user-defined collections from browser localStorage
2. Frontend builds collection playlists locally, including a virtual "所有音乐" entry when needed
3. Users create, rename, delete, and update collections entirely on the client
4. Playback still uses the backend music streaming endpoints for the selected songs

## Code Standards (from AGENTS.md)

1. Avoid deeply nested code
2. For long calling chains, draw Sequence Diagrams in Mermaid
3. Use English for code/comments unless instructed to translate
4. Always use strict mode in TypeScript and JavaScript
5. Ensure backend compiles with cargo and frontend compiles with npm build.
6. Frontend has cargo part, we only use it as a "browser that run the web", so mostly we don't need to ensure it compiles with cargo.

## Logging Features

### Access Logging
- All HTTP API requests are automatically logged with detailed information
- Logs include: method, path, query string, client IP, user-agent, status code, response size, and processing time
- See [`docs/access-logging.md`](docs/access-logging.md) for detailed documentation
- Access logs are always at INFO level and cannot be disabled

### File Scanning Debug Logging
- Debug level logs show detailed information about file scanning process
- When enabled, logs show:
  - Directories being scanned
  - Music files found with full paths
  - Non-audio files that are skipped
  - Progress counters for file processing
- Enable with: `RUST_LOG=debug cargo run`

## API Endpoints

### Music Endpoints
- `GET /api/music/{filename}` - Stream audio file
- `GET /api/music` - Get all music from database
- `GET /api/music/id/{id}` - Stream audio by ID; `?position=` seeks (0.0–1.0) and `?download=1` sends `Content-Disposition: attachment` (RFC 6266 `filename` + UTF-8 `filename*=`) so the browser saves instead of plays — used by the browser "download to local" flow; see [`docs/library-import.md`](docs/library-import.md)
- `GET /api/music/path` - Stream an arbitrary audio file by absolute path (no DB lookup). Query: `?p={url-encoded path}`. Used by the "open file as default app" flow; extension-gated for safety. See [`docs/default-music-app.md`](docs/default-music-app.md)
- `GET /api/music/id/{id}/cover` - Extract embedded cover art by music ID (FFmpeg pipeline; materializes non-filesystem sources like Android `content://` URIs into a temp file first)
- `GET /api/music/path/cover` - Extract embedded cover art by absolute path. Query: `?p={url-encoded path}`. Same security gating as `/api/music/path`. Used by the launch handoff so the click-open player shows cover art for files not in the DB. See [`docs/default-music-app.md`](docs/default-music-app.md)

### Launch Handoff Endpoints (open-as-default-app flow)
- `GET /api/launch/pending` - Atomically take the pending launch file path the OS launched Kaulan with. Returns `{path: string | null}` and clears the stash.
- `GET /api/launch/events` - Server-Sent Events stream; pushes `data: {}` each time a new launch file is stashed (warm-start case). Browser auto-reconnects.
- See [`docs/default-music-app.md`](docs/default-music-app.md) for the full flow.

### Lyrics Endpoints
- `GET /api/lyrics/{filename}` - Stream LRC lyrics file
  - Looks up music by filename in database
  - Constructs corresponding `.lrc` file path
  - Returns 404 if LRC file doesn't exist (graceful degradation)
  - See [`docs/lyrics-display.md`](docs/lyrics-display.md) for details
- `GET /api/lyrics/id/{id}` - Stream LRC or WEBVTT lyrics by music ID
- `GET /api/lyrics/path` - Stream LRC or WEBVTT sidecar lyrics by absolute path. Query: `?p={url-encoded path}`. Same security gating as `/api/music/path`. Used by the launch handoff so a double-clicked file picks up same-directory sidecar lyrics without a DB row. See [`docs/default-music-app.md`](docs/default-music-app.md)
- `PUT /api/lyrics/id/{id}` - Update an existing writable LRC or WEBVTT sidecar file after lyric timing edits
  - Request body: `{ "content": "..." }`
  - Returns 409 when the source is not writable, such as Android MediaStore
  - See [`docs/lyric-editing.md`](docs/lyric-editing.md) for details

### Playlist Endpoints (Folder-based)
- `GET /api/playlists` - Get all playlists (folder-based)
- `GET /api/playlists/{name}` - Get specific playlist by name

### Settings Endpoints
- `GET /api/settings/music-directory` - Get current music directory path
- `POST /api/database/update` - Trigger database update (scan for new files, update LUFS, remove deleted files)

### Other Endpoints
- `POST /api/generate-lufs` - Generate LUFS values via FFmpeg (debug mode only)
- `POST /api/music/{id}/precache-lufs` - Pre-cache LUFS for next song (non-blocking)
- `POST /api/library/import-from-remote` - (Tauri runtimes) Pull selected tracks (audio + lyrics) from a remote Kaulan server into the local `download_root`; returns a job id pollable via `GET /api/download/jobs/{id}`. See [`docs/library-import.md`](docs/library-import.md)

### Static Frontend Routes
- `GET /` - Serve the built Vue app from `frontend/dist/index.html`
- `GET /assets/...` - Serve production frontend assets
- `GET /<browser-route>` - Fall back to `index.html` for SPA navigation
- `GET /api/...` - Reserved for API routes and never handled by the SPA fallback
- See [`docs/static-frontend-serving.md`](docs/static-frontend-serving.md)

**Important: LUFS Pre-cache Non-blocking Behavior**
- When LUFS value is already cached: returns `200 OK` immediately with the LUFS value
- When LUFS needs calculation: returns `202 Accepted` immediately and calculates in background via `tokio::spawn`
- The background task updates the database when calculation completes
- Frontend only refreshes data when receiving a cached LUFS value (not for 202 responses)
- This prevents browser connection limits from blocking audio playback when switching songs quickly

## Dependencies

**Backend:**
- actix-web 4
- sea-orm 0.12 with SQLite
- serde, tokio, chrono
- actix-cors
- async-trait (for pluggable file operations)

**Frontend:**
- Vue 3 with Composition API
- TypeScript
- Vite
- localStorage-backed frontend persistence for settings, collections, and device sources

**Android (Tauri):**
- tauri-plugin-android-mediastore - MediaStore API integration for Android music scanning
- See [`docs/android/mediastore-integration.md`](docs/android/mediastore-integration.md) for details

**External Tools:**
- FFmpeg required for LUFS generation

## Configurable Server URL

### Overview
The frontend supports a configurable backend server URL. Users can set a custom server address through the settings panel, which is saved to browser cookies and persists across page reloads.

### How It Works

1. **LocalStorage**: Server URL is stored in localStorage with key `kaulan_server_url`
2. **Default URL**: `http://localhost:2080/api` is used if no localStorage value is set
3. **Dynamic API Base**: All API calls use `getApiBase()` to get the current server URL
4. **URL Normalization**: URLs are automatically normalized to end with `/api`

### Key Files
- `frontend/src/utils/api.ts` - Dynamic API base with localStorage support
- `frontend/src/utils/storage.ts` - LocalStorage operations
- `frontend/src/utils/validation.ts` - URL validation
- `frontend/src/components/modals/SettingsModal.vue` - Server URL UI

### Usage
See [`docs/configurable-server-url.md`](docs/configurable-server-url.md) for full documentation including user instructions, validation behavior, and technical details.

### git commit

A commit message should contains:

1. What did you do?
2. Why you do that?

Example:

```
Fix lyric panel behavior in desktop mode

- Prevent lyric toggle in wide-layout (desktop) mode
- Auto-scroll to current lyric when panel opens
- Add helper function scrollToCurrentLyric for reusability

Co-Authored-By: Claude/Codex/Gemini...
```

### Icon

There is icon, you should use this to generate icons for the app
```
# cd to the root of the project
tauri icon ./favicon.ico
```
