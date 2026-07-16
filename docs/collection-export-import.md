# Collection Export / Import

Collections live in the browser's `localStorage` (`kaulan_local_collections`),
which is wiped when the app is re-installed. The export / import feature lets
users back up their collections to a JSON file before re-install (or when
moving to another device) and restore them afterward.

The export deliberately **drops volatile fields** (`id`, `lufs`, `path`,
`stream_url`, `cover_url`, …) and keys each song by `(source, song_name)`.
The reason: the local SQLite DB is rebuilt on re-install, so song `id`
values change. Remote server IDs can change too. The API base URL of each
source is stable across re-installs (the local server is always
`http://localhost:2080/api`; remote servers persist via
`kaulan_manual_devices`), and song filenames are stable as long as the user's
files don't move — so that pair is the right durable key.

Related source files:
- Frontend utility: `frontend/src/utils/collectionTransfer.ts` (`buildCollectionsExport`, `parseCollectionsExport`, `mergeCollectionsFromImport`)
- Frontend tests: `frontend/src/utils/__tests__/collectionTransfer.test.ts`
- UI entry point: `frontend/src/components/modals/SettingsModal.vue` (备份收藏夹 section in 个人, runtime-branched handlers)
- Runtime detection: `frontend/src/utils/api.ts` (`isTauriWebview`)
- Persistence: `frontend/src/stores/collections.ts` (`replaceLocalCollections`)
- Library lookup source: `frontend/src/composables/useLibrarySources.ts` (`allLibrarySongs`, `buildSongRowKey`)
- Browser download helper: `frontend/src/utils/browserDownload.ts` (`downloadBlob`)
- Tauri plugins: `@tauri-apps/plugin-dialog` (`save`, `open`), `@tauri-apps/plugin-fs` (`readTextFile`, `writeTextFile`)
- Tauri setup: `frontend/src-tauri/src/lib.rs` (`tauri_plugin_fs::init()`), `frontend/src-tauri/capabilities/default.json`

## Export format

```json
{
  "version": 1,
  "exported_at": "2026-07-16T12:00:00.000Z",
  "collections": [
    {
      "name": "My Favorites",
      "created_at": "2026-01-15T08:30:00.000Z",
      "songs": [
        { "source": "http://localhost:2080/api", "name": "Artist - Song.mp3" },
        { "source": "http://192.168.1.10:2080/api", "name": "Other.flac" }
      ]
    }
  ]
}
```

The format is per-collection (songs nested inside each collection), not the
normalized `songs: { local: [...], server_1: [...] }` shape suggested in the
original issue. Per-collection matches the in-memory `StoredLocalCollection`
shape, is easy to hand-edit, and the size overhead is negligible for a
personal library.

## How matching works on import

The matcher builds a lookup table from `allLibrarySongs` (the flat list of
every song in every currently-loaded source), keyed by
`${source_key} ${name}` → library song. Each payload entry is an O(1)
lookup. There are **no per-server queries at import time** — only the data
that was already fetched on library load.

First match wins: if two songs on the same source share a filename, only the
first one encountered (in source-then-playlist order) is used.

If a payload song's `(source, name)` isn't present in the current library
(because the remote server hasn't been re-added, the file was renamed, or the
library hasn't finished loading), the song is **skipped and counted** in the
summary. No partial entry is stored — the collection either has the full
current metadata for a song, or it doesn't list it at all.

## Merge-on-name-conflict semantics

When the imported collection's name matches an existing local collection:

```mermaid
sequenceDiagram
    participant U as User
    participant SM as SettingsModal
    participant CT as collectionTransfer
    participant CS as Collections store
    participant LS as Library store

    U->>SM: click 导入收藏夹, pick JSON
    SM->>CT: parseCollectionsExport(text)
    SM->>LS: read allLibrarySongs
    SM->>CS: read localCollections
    SM->>CT: mergeCollectionsFromImport(payload, current, allSongs)
    CT-->>SM: { collections, result }
    loop per payload collection
        alt name unknown
            CT->>CT: create new collection, id=Date.now()
            CT->>CT: resolve songs; skip unmatched
        else name exists
            CT->>CT: union songs, dedupe by rowKey
        end
    end
    SM->>CS: replaceLocalCollections(collections)
    CS->>CS: localCollections := next, persist to localStorage
    SM->>U: alert summary (new / merged / matched / skipped)
```

Dedupe is by the same row-key shape used elsewhere in the app:
`${source_key || "local"}:${id}:${name}`. Re-importing the same file is
idempotent — every payload song either matches an existing entry (deduped) or
appends fresh.

## Re-import recovers skipped songs

If the first import skipped songs because a source was unavailable, those
songs are simply absent from the resulting collection — they are not stored
as placeholders. Re-importing the same file **after the missing source comes
online** recovers them: the previously-matched songs dedupe against existing
entries, and the previously-skipped songs now resolve and append. Final state
converges to the full set.

**Keep the export file** until all songs import cleanly. Once the export is
deleted, the skipped songs cannot be reconstructed.

## User-facing flow

1. **Export:** Open 设置 → 个人 → 备份收藏夹 → 导出收藏夹. Downloads
   `kaulan-collections-YYYY-MM-DD.json` via the browser's download manager
   (`downloadBlob`).
2. **Re-install or move devices.** Re-add any remote servers via 设备与来源
   and wait for the library to load.
3. **Import:** Open 设置 → 个人 → 备份收藏夹 → 导入收藏夹. Pick the JSON
   file. A summary alert reports new collections, merged collections, matched
   songs, and skipped songs.

If the library is still loading when the user imports, songs from not-yet-
loaded sources will be counted as skipped. The user should wait for the
library to finish loading before importing.

## Why not `tauri-plugin-file-access`?

The original issue suggested `tauri-plugin-file-access`. We deliberately
avoid it: CLAUDE.md forbids Tauri features in ts/js/html ("we need to provide
a web"). The browser's native `<input type="file">` for reading and
`<a download>` for writing work identically on web, desktop, and the Android
webview, so no plugin is needed.

## Runtime-conditional file IO

The HTML `<input type="file">` + `<a download>` path only works inside a real
browser — Android and desktop Tauri webviews block the download anchor and
(surprisingly) the desktop webview also fails to surface `<input type="file">.
So the SettingsModal branches at the call site on `isTauriWebview()`
(`frontend/src/utils/api.ts:31`):

| Runtime | Export | Import |
| --- | --- | --- |
| Browser | `downloadBlob` (hidden `<a download>`) | hidden `<input type="file">` |
| Tauri (Android + desktop) | `plugin-dialog` `save()` + `plugin-fs` `writeTextFile()` | `plugin-dialog` `open()` + `plugin-fs` `readTextFile()` |

The Tauri plugins (`@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-fs`) are
dynamically imported only from the Tauri branch, so the browser bundle
doesn't pull them in. Capabilities are granted in
`frontend/src-tauri/capabilities/default.json` (`dialog:default`, `fs:default`,
plus `fs:allow-read-text-file` and `fs:allow-write-text-file`). plugin-dialog
adds user-picked paths to the fs scope at runtime, so no per-path scope config
is needed.

The pure export/parse/merge logic in `frontend/src/utils/collectionTransfer.ts`
is runtime-agnostic: it just produces/consumes JSON strings. The runtime
dispatch lives entirely in `SettingsModal.vue`.

**Android risk note:** plugin-fs on Android uses content URIs returned by
plugin-dialog's SAF-backed picker. If this turns out not to work for
read/write, the contingency is a pair of custom `#[tauri::command]`s in
`frontend/src-tauri/src/lib.rs` that use ContentResolver via JNI, with the
frontend branching on `checkIsAndroid()` (`frontend/src/utils/platform.ts:125`)
to call invoke instead of plugin-fs.
