# Lyrics Display Feature

## Overview

The lyrics display feature adds synchronized lyrics display to the Kaulan music player. Timed lyric sidecar files stored alongside music files are served via a backend API endpoint, parsed on the frontend, and displayed with real-time highlighting and auto-scrolling.

On Android, lyric updates are still frontend-driven. The frontend polls the playback session from the Android plugin, updates the current song and playback position, and the lyric panel reacts to those values.

For the timing model used to keep lyric switching accurate across playback backends, see [`docs/lyric-sync-timing.md`](./lyric-sync-timing.md).

## Related Files

**Backend:**
- `backend/src/file_ops/mod.rs` - `LyricReader` trait, `StdLyricReader`, `set_lyric_reader()`/`get_lyric_reader()`
- `backend/src/handlers/lyrics.rs` - Lyrics API endpoint handler
- `backend/src/handlers/mod.rs` - Module exports
- `backend/src/server/mod.rs` - Route registration

**Tauri (Android adapters):**
- `frontend/src-tauri/src/android_media_adapter.rs` - `AndroidLyricReader` implementation, `MediaStoreFileReader`, `MediaStoreMusicFileLister`
- `frontend/src-tauri/src/lib.rs` - Plugin registration, `request_external_storage_permission`, `check_external_storage_permission` commands

**Frontend:**
- `frontend/src/composables/useLyrics.ts` - LRC/WEBVTT parsing and sync logic
- `frontend/src/App.vue` - Lyric panel UI and integration
- `frontend/src/components/modals/SettingsModal.vue` - "使用本地歌词" checkbox (Android only)

## Supported Formats

The backend looks for sidecar lyric files in this order:

1. `song.lrc`
2. `song.vtt`

The sidecar file name is stem-based, so `song.mp3` maps to `song.lrc` or `song.vtt`.

## LRC Format

### Standard Single-Language Format

```
[mm:ss.xx]Lyric text here
```

Example:
```
[00:00.54]First line of lyrics
[00:02.52]Second line of lyrics
[00:05.00]Third line of lyrics
```

### Bilingual Format

The player supports bilingual lyrics where lines with the same timestamp are treated as one lyric group with original + translation. This works whether the duplicate timestamps are consecutive or split into separate blocks.

```
[00:00.64]Japanese original text
[00:00.64]Chinese translation
[00:02.14]Another Japanese line
[00:02.14]Another Chinese translation
```

### Format Details

- **Timestamp format:** `[mm:ss.xx]` or `[mm:ss.xxx]`
  - `mm` = minutes (00-59)
  - `ss` = seconds (00-59)
  - `xx/xxx` = milliseconds (2 or 3 digits)
- **UTF-8 encoding:** Supports Chinese, Japanese, Korean, and other Unicode characters
- **File naming:** LRC files must have the same base name as the audio file:
  - `song.mp3` → `song.lrc`
  - `album/track.flac` → `album/track.lrc`

### WEBVTT Format

Standard `WEBVTT` subtitle files are also supported as a fallback when no `.lrc` file exists.

Example:
```text
WEBVTT

00:00:02.900 --> 00:00:06.700
哥哥 哥哥
```

## API Reference

### GET /api/lyrics/{filename}

Retrieve the lyric file content for a given music filename.

**Path Parameters:**
- `filename` - The music filename to look up (e.g., `song.mp3`)

**Response:**
- **200 OK** - Returns lyric file content as `text/plain; charset=utf-8`
- **404 Not Found** - Music not in database or no supported sidecar lyric file exists

**Example:**
```bash
curl http://localhost:2080/api/lyrics/song.mp3
```

Response:
```
[00:00.54]First line
[00:02.52]Second line
```

### POST /api/download/lyrics/search

Search online lyric candidates by free-text query.

### POST /api/download/lyrics/apply

Save the selected online lyric as a sidecar `.lrc` file for an existing local song.

## User Instructions

### Viewing Lyrics

1. **Mobile/Tall Layout:**
   - Tap the lyrics icon in the player controls to toggle lyrics display
   - The lyrics panel will replace the song list

2. **Desktop/Wide Layout:**
   - Lyrics are displayed automatically in the right panel
   - Toggle lyrics display using the lyrics icon in player controls

### Lyrics Behavior

- **Auto-scroll:** The current lyric line is automatically centered as the song plays
- **Highlighting:** The active lyric line is highlighted in green with larger text
- **Bilingual display:** Original language is shown larger, translation is shown smaller below, and both lines are highlighted together when their timestamp is active
- **Missing lyrics:** if no supported lyric file exists, the Lyric tab shows "暂无歌词" together with a `search online` action that opens a lyric-only online search dialog prefilled with the current song name

### Adding Lyrics

1. Create a `.lrc` or `.vtt` file with the same stem as your audio file
2. Place it in the same directory as the audio file
3. Use the standard LRC timestamp format
4. On desktop: lyrics are available immediately
5. On Android: see [Local Lyrics on Android](#local-lyrics-on-android) for setup

### Local Lyrics on Android (Android only)

Reading `.lrc` files from the filesystem requires the `MANAGE_EXTERNAL_STORAGE` permission, which is granted through system settings.

1. Open Settings
2. Check "使用本地歌词" under display settings
3. The app will request `MANAGE_EXTERNAL_STORAGE` permission — this opens Android system settings
4. Grant the permission
5. Return to the app — the checkbox reflects the actual permission state

The checkbox directly reflects the system permission state. To disable local lyrics, revoke the permission in Android Settings > Apps > Kaulan > Permissions.

## Implementation Details

### Backend Flow

```mermaid
sequenceDiagram
    participant Frontend
    participant API
    participant Database
    participant FileSystem

    Frontend->>API: GET /api/lyrics/{filename}
    API->>Database: Find music by filename
    Database-->>API: Music record (file_path)
    API->>API: Construct `.lrc` path, then `.vtt` path
    API->>FileSystem: Read first matching lyric file
    FileSystem-->>API: File contents or error
    API-->>Frontend: 200 + content or 404
```

### Frontend Parsing

The lyric parser handles:
1. Format detection for LRC vs `WEBVTT`
2. Line-by-line parsing of timestamps
3. Merging all lines with the same timestamp into one lyric group (bilingual support)
4. Sorting lyrics by timestamp
4. Returning an array of `LyricLine` objects:
   ```typescript
   interface LyricLine {
     time: number      // Time in seconds
     texts: string[]   // Array of lyric texts
   }
   ```

### Sync Logic

The `updateCurrentLyric(time)` function:
1. Finds the last lyric line with `time <= current_time`
2. Updates `currentLyricIndex` to highlight the active line
3. Triggers auto-scroll to center the active line

### Android Behavior

On Android:

1. playback queue and runtime state come from the plugin session
2. the frontend polls the plugin every second
3. `currentSong` changes trigger lyric file reloads
4. `currentTime` updates drive lyric highlighting

#### Android Lyrics Resolution

On Android, the database stores content URIs (`content://media/external/audio/media/...`) instead of filesystem paths. The `AndroidLyricReader` resolves these to real paths using `resolve_media_path()` from `tauri-plugin-android-mediastore`, then reads `.lrc` files via `std::fs` (requires `MANAGE_EXTERNAL_STORAGE` permission).

```mermaid
sequenceDiagram
    participant Frontend
    participant API
    participant DB
    participant MediaStore
    participant FileSystem

    Frontend->>API: GET /api/lyrics/id/{id}
    API->>DB: Find music by ID
    DB-->>API: file_path = content://...
    API->>MediaStore: find sidecar lyric for content URI
    MediaStore-->>API: lyric text if readable
    API-->>Frontend: 200 + content or 404
```

See [`docs/android/playback-session.md`](./android/playback-session.md) for the playback/session side of this flow.

## Testing

### Backend Tests

```bash
cd backend
cargo test lyrics
```

Tests cover:
- LRC file present → 200 with content
- VTT fallback present → 200 with content
- No sidecar lyric file → 404
- Music not in database → 404
- UTF-8 content (Chinese/Japanese characters)

### Frontend Tests

```bash
cd frontend
npm test useLyrics
```

Tests cover:
- Single-language LRC parsing
- Bilingual LRC parsing across repeated timestamps
- Empty lyric lines handling
- Timestamp sorting
- UTF-8 character support
- VTT parsing
- Punctuation preservation

## Troubleshooting

### Lyrics not displaying

1. **Check LRC file exists:**
   ```bash
   ls -l /path/to/music/song.lrc
   ```

2. **Check backend is serving the file:**
   ```bash
   curl http://localhost:2080/api/lyrics/song.mp3
   ```

3. **Check frontend console for errors:**
   - Open browser DevTools
   - Look for fetch errors or parsing issues

4. **Android: Check MANAGE_EXTERNAL_STORAGE permission is granted in system settings**

5. **Android: Check logs** — search for `AndroidLyricReader` entries showing the resolved path and whether the `.lrc` file was found

### Sync issues

- **Lyrics ahead/behind audio:** Check the timestamp format in the LRC file
- **Missing lyrics:** Verify timestamps match the audio version (different recordings may have different timings)

### Encoding issues

- Ensure LRC files are saved with UTF-8 encoding
- Most text editors default to UTF-8, but some may use other encodings
