# Online Search and Download

## Overview

Kaulan supports online search, preview, lyric selection, and download for three providers:

- YouTube
- Netease
- Bilibili

The feature is exposed through the `在线查找` modal in the frontend. Provider login state is managed by the Tauri shell, while search, preview, and download requests are handled by the Rust backend.

This document describes the current behavior, request flow, and provider-specific rules.

## Standalone Server Auth Import

Standalone backend mode can import provider auth from one file per source at process startup.

CLI flags:

- `--youtube-cookie-file <path>` points to a Netscape cookie jar file for YouTube
- `--netease-session-file <path>` points to a JSON file that matches `session.json`
- `--bilibili-session-file <path>` points to a JSON file that matches `bilibili_session.json`

Example:

```bash
cargo run -- run /path/to/music \
  --youtube-cookie-file /path/to/youtube-cookies.txt \
  --netease-session-file /path/to/netease-session.json \
  --bilibili-session-file /path/to/bilibili-session.json
```

Import behavior:

- YouTube keeps using the file directly through `KAULAN_YOUTUBE_COOKIE_HEADER_PATH`
- Netease copies the provided file into `$NCMDUMP_CONFIG_DIR/session.json`
- Bilibili copies the provided file into `$NCMDUMP_CONFIG_DIR/bilibili_session.json`
- If `NCMDUMP_CONFIG_DIR` is not set, Kaulan uses `~/.config/ncmdump/`

## User-Facing Behavior

### Provider gating

- A provider is considered available only when Kaulan detects a saved login/session for that provider.
- In the modal, logged-out providers:
  - hide login capture actions after login is already present
  - are disabled as search sources when login is missing
- During search, the frontend only sends enabled sources.
- The backend also re-checks provider availability and drops logged-out sources defensively.

Current provider availability checks:

- `youtube`: saved raw cookie header exists
- `netease`: saved `MUSIC_U` session exists
- `bilibili`: saved session cookies exist

### Search behavior

- Search is merged across the selected enabled providers.
- Backend provider requests are fanned out concurrently, not serially.
- Result rows show:
  - title
  - artist/uploader
  - duration
  - source badge
- Users can request:
  - `试听` for temporary playback
  - `歌词` for Netease lyric candidates
  - `下载` for a full download
- The lyric list is for selection only. After selecting a lyric candidate, the user still uses the song row's `下载` button to save both the audio file and the selected `.lrc`.

### Preview behavior

- Preview downloads a temporary local file first.
- The frontend replaces the current playback queue with a single temporary track.
- Temporary preview tracks are not inserted into the main library database.
- On Android, preview files are stored in the app download root under `.preview-cache`.
- On Android, Bilibili preview downloads keep the provider's raw DASH audio container instead of running FFmpeg conversion. Related source: `backend/src/handlers/download.rs`.

### Download behavior

- Full downloads are saved under the configured online download root.
- On Android, Kaulan uses the app external files music directory:
  - `/sdcard/Android/data/afeather.kaulan/files/Music`
- On Android, Bilibili full downloads are saved as raw DASH audio files with the `.m4s` extension because FFmpeg is not integrated in the Android path yet. Related source: `backend/src/handlers/download.rs`.
- If the user selected a lyric candidate, Kaulan tries to save a matching `.lrc` file beside the audio file.
- After a successful full download, Kaulan refreshes the music database across both library roots:
  - the configured music directory
  - the configured online download root

## API

### `POST /api/download/search`

Search online providers and return a merged result list.

Request:

```json
{
  "query": "keyword",
  "max_results": 8,
  "sources": ["youtube", "netease", "bilibili"]
}
```

Response item:

```json
{
  "source": "netease",
  "id": "2015001195",
  "title": "Song Title",
  "artist": "Artist Name",
  "duration": "3:28",
  "thumbnail_url": "https://...",
  "can_preview": true,
  "can_download": true,
  "requires_login": false
}
```

### `POST /api/download/preview`

Download a temporary preview file and return a one-track playback item.

Request:

```json
{
  "source": "netease",
  "id": "2015001195",
  "title": "Song Title",
  "artist": "Artist Name"
}
```

Response:

```json
{
  "success": true,
  "message": "试听准备完成",
  "song": {
    "id": -123456,
    "name": "Song Title [Artist Name]",
    "path": "/absolute/path/to/temp-file.mp3",
    "stream_url": "http://localhost:2080/api/download/preview/preview-....mp3",
    "cover_url": "https://...",
    "source": "netease",
    "is_temporary": true
  }
}
```

### `GET /api/download/preview/{filename}`

Stream a previously prepared preview file.

### `POST /api/download/lyrics/search`

Search Netease lyric candidates for the selected result.

Request:

```json
{
  "query": "Song Title Artist Name"
}
```

Response item:

```json
{
  "source": "netease",
  "id": "2015001195",
  "title": "Song Title",
  "artist": "Artist Name",
  "album": "Album Name"
}
```

### `POST /api/download/track`

Download the selected track into the online download root and optionally save lyrics.

Request:

```json
{
  "source": "netease",
  "id": "2015001195",
  "title": "Song Title",
  "artist": "Artist Name",
  "target_subdir": "",
  "lyric_selection": "2015001195"
}
```

Response:

```json
{
  "success": true,
  "message": "下载完成",
  "filename": "Song Title.mp3",
  "lyric_filename": "Song Title.lrc",
  "warning": null
}
```

### `GET /api/download/directory-tree`

Return the selectable directory tree under the online download root.

## Sequence Diagrams

### Provider login gating

```mermaid
sequenceDiagram
    participant User
    participant FE as Frontend Modal
    participant Tauri as Tauri Provider Commands
    participant Store as Session/Cookie Store

    User->>FE: Open 在线查找 modal
    FE->>Tauri: online_login_status(youtube)
    FE->>Tauri: online_login_status(netease)
    FE->>Tauri: online_login_status(bilibili)
    Tauri->>Store: Read saved provider sessions
    Store-->>Tauri: Session status
    Tauri-->>FE: ProviderStatus per source
    FE->>FE: Disable unchecked logged-out sources
    FE->>FE: Hide login actions when already logged in
```

### Concurrent search flow

```mermaid
sequenceDiagram
    participant User
    participant FE as Frontend Modal
    participant BE as Backend /api/download/search
    participant YT as YouTube Search
    participant NE as Netease Search
    participant BI as Bilibili Search

    User->>FE: Enter keyword and click 搜索
    FE->>FE: Keep only enabled logged-in sources
    FE->>BE: POST /api/download/search
    BE->>BE: Filter logged-out providers again
    par Enabled provider search fan-out
        BE->>YT: search_youtube(query, max_results)
        BE->>NE: search_netease(query, max_results)
        BE->>BI: search_bilibili(query, max_results)
    end
    YT-->>BE: Result list / error
    NE-->>BE: Result list / error
    BI-->>BE: Result list / error
    BE->>BE: Merge successful result lists
    BE-->>FE: Combined search results
    FE-->>User: Render merged result rows with source badges
```

### Preview flow

```mermaid
sequenceDiagram
    participant User
    participant FE as Frontend Modal
    participant BE as Backend /api/download/preview
    participant Provider as Provider Client
    participant FS as Preview Cache
    participant Player as Audio Player

    User->>FE: Click 试听
    FE->>BE: POST /api/download/preview
    BE->>Provider: Download provider preview file
    Provider-->>BE: Temporary audio file
    BE->>FS: Save under preview root
    BE-->>FE: PreviewSong with stream_url and temp status
    FE->>Player: Replace queue with single temporary track
    Player->>BE: GET /api/download/preview/{filename}
    BE-->>Player: Stream preview file
```

### Full download and lyric flow

```mermaid
sequenceDiagram
    participant User
    participant FE as Frontend Modal
    participant BE as Backend /api/download/track
    participant Provider as Provider Client
    participant FS as Download Root
    participant Lyrics as Netease Lyric API
    participant DB as Music Database

    User->>FE: Select target directory
    User->>FE: Optionally select lyric candidate
    User->>FE: Click 下载
    FE->>BE: POST /api/download/track
    BE->>BE: Validate target_subdir stays within download root
    BE->>Provider: Download full track
    Provider-->>BE: Audio file
    BE->>FS: Save final file into target directory
    opt lyric_selection provided
        BE->>Lyrics: track_lyric(selected lyric id)
        Lyrics-->>BE: LRC / translated lyric
        BE->>FS: Save .lrc beside audio file
    end
    BE->>DB: Refresh music database
    Note over DB: Scan music directory and download root
    DB-->>BE: Updated scan result
    BE-->>FE: success / warning / failure response
    FE-->>User: Show final status message
```

### Netease quality fallback flow

```mermaid
sequenceDiagram
    participant BE as Backend
    participant NE as Netease Client

    BE->>NE: download_track(track_id, Exhigh)
    alt Exhigh succeeds
        NE-->>BE: Audio file
    else Exhigh fails
        BE->>NE: download_track(track_id, Higher)
        alt Higher succeeds
            NE-->>BE: Audio file
        else Higher fails
            BE->>NE: download_track(track_id, Standard)
            NE-->>BE: Audio file or final error
        end
    end
```

## Implementation Notes

### Search source availability

- `youtube` search/download requires a saved cookie header.
- `netease` search/download requires a saved `MUSIC_U` session.
- `bilibili` search/download requires saved login cookies.

The current backend behavior intentionally ignores logged-out providers instead of returning partial provider login errors from `/api/download/search`.

### Temporary preview tracks and LUFS

- Preview tracks use synthetic negative IDs and are not in the main music table.
- They are streamed by `/api/download/preview/{filename}` instead of `/api/music/id/{id}`.
- They should be treated as temporary playback-only items.

### Target directory validation

- `target_subdir` is always interpreted as a path relative to the online download root.
- Kaulan rejects parent traversal and paths outside the configured root.
- The root directory itself is valid when `target_subdir` is empty.

## Related Source Files

- `frontend/src/components/modals/OnlineSearchModal.vue`
- `frontend/src/App.vue`
- `frontend/src/composables/useAudioPlayer.ts`
- `frontend/src-tauri/src/lib.rs`
- `backend/src/handlers/download.rs`
- `backend/src/handlers/lufs.rs`
- `backend/src/server/mod.rs`
- `backend/src/types/mod.rs`
- `vendor/ytdl-audio/src/lib.rs`
- `vendor/ncmdump-rs/netease-api/src/track.rs`
- `vendor/ncmdump-rs/bilibili-api`
