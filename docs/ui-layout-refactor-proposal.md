# UI Layout Refactor Proposal

Related source files:

- `frontend/src/App.vue`
- `frontend/src/components/modals/AddDeviceModal.vue`
- `frontend/src/components/modals/SettingsModal.vue`
- `frontend/src/components/SearchBar.vue`
- `frontend/src/components/PlaylistListView.vue`
- `frontend/src/components/SongListView.vue`

## Core Direction

The UI should stop asking normal users to choose a single active server.

Instead:

- `Library` shows server-grouped folder-based playlists from all music servers
- each server section can show its current reachability status
- opening a library row shows the songs in that folder-based playlist
- search works across all servers by default
- collections are stored locally per browser/user, not shared through the backend

This matches the family-use case better:

- the music library is shared
- personal collections remain private

## Product Model

Normal users should understand the app like this:

```text
Kaulan shows one music library page.
That page lists folder-based playlists from different devices or servers.
I can open one playlist to see its songs.
My collections are mine only.
```

Not like this:

```text
First choose one server.
Then browse that server.
Collections are shared with everyone.
```

## Main Concepts

### Source Resolution Rules

The frontend should not keep a global "current source" for normal operation.

Instead:

- local maintenance actions always target `http://localhost:2080/api`
- source-browsing actions use the explicit source URL stored on the item being acted on
- a source key that is an absolute `http://` or `https://` URL is treated as a remote API base
- any non-HTTP source key is treated as local and resolved to `http://localhost:2080/api`

This means:

- startup scan is localhost-only
- discovery scan and local device naming are localhost-only
- playlist, song, lyrics, cover, and LUFS requests are routed by the song or source group itself
- adding a device should add a source to the library, not replace an app-global active server

### 1. Shared Library

The visible library is an aggregated view of folder-based playlists returned by all reachable servers and local sources.

Examples of sources:

- `This Device`
- `Living Room PC`
- `Downloads`

Important detail:

- this is not a recursive directory tree
- this is not a flat merged song list by default
- it is a grouped list of backend playlist buckets keyed by folder name
- this matches the current `/api/playlists` behavior

Primary layout:

- group playlists under each server/device
- show server status in the group header
- show a per-source `⋮` menu in the group header
- show playlist rows inside that group

This is better than a flat merged list when a server goes offline, because the UI can show the problem once at the group level.

### 2. Personal Collections

Collections should be stored in frontend local storage or IndexedDB.

That means:

- each browser/user gets their own collections
- family members do not overwrite each other
- a collection can mix songs from multiple servers

### 3. Add Device Flow

Source onboarding should be a first-class library action, not hidden in settings.

That means:

- `Library` shows an `Add device` action in the top action row
- tapping it opens a dedicated device sheet or modal
- the sheet includes both nearby-device discovery and manual address entry
- tapping a discovered device connects immediately
- manual devices remain stored locally for the current browser/user

### 4. Advanced Settings

Advanced settings should keep low-frequency admin controls only.

That means:

- local device name
- scan file types
- playback and Android-specific toggles
- source diagnostics or advanced details

Not:

- nearby-device discovery
- manual source onboarding

## Terminology

Recommended user-facing labels:

| Technical Term    | User-facing Label                               |
| ----------------- | ----------------------------------------------- |
| Server            | Music Source or Device                          |
| Active Server     | Remove from normal UI and internal architecture |
| Source List       | Music Sources                                   |
| Database Update   | Refresh Library                                 |
| Collection        | My Collection                                   |
| Shared Collection | Avoid by default                                |
| LUFS              | Hide in advanced settings                       |

## Narrow Mode

Narrow mode should show one main task at a time.

The main screen should focus on:

- one content list
- one mini player
- one obvious way into search

### Mobile Main Screen

`Library` tab:

```text
+--------------------------------------------------+
| Kaulan                             [Search] [⋮] |
+--------------------------------------------------+
| [Library] [My Collections] [Filter] [Add device] |
+--------------------------------------------------+
| This Device                      [Online]    [⋮]|
|  所有音乐                                        |
|  mp3                                             |
|  RedmiNote7                                      |
|                                                  |
| Living Room PC                   [Online]    [⋮]|
|  Downloads                                      |
|  Anime                                          |
|                                                  |
+--------------------------------------------------+
| [Cover] Song Name                                |
| [Cover]      progress bar                        |
| [Shuffle/Seq/...] [<<] [Play/Pause] [>>] [Queue] |
+--------------------------------------------------+
```

`My Collections` tab:

```text
+------------------------------------------------------+
| Kaulan                               [Search] [⋮]   |
+------------------------------------------------------+
| [Library] [My Collections] [Filter] [New Collection] |
+------------------------------------------------------+
| No personal collections yet.                         |
| Tap "New Collection" to create one.                  |
+------------------------------------------------------+
| [Cover] Song Name                                    |
| [Cover]      progress bar                            |
| [Shuffle/Seq/...] [<<] [Play/Pause] [>>] [Queue]     |
+------------------------------------------------------+
```

### Mobile Expanded Player

When the user taps the mini-player cover or song name, the player expands upward while keeping the lower control block stable.

Default expanded state:

```text
+--------------------------------------------------+
|           [ Cover ]                              |
|           [ Cover ]                              |
|           [ Cover ]                              |
|           [ Cover ]                              |
|           [ Cover ]                              |
|           [ Cover ]                              |
|           [ Cover ]                              |
+--------------------------------------------------+
| [Cover] Song Name                                |
| [Cover]      progress bar                        |
| [Shuffle/Seq/...] [<<] [Play/Pause] [>>] [Queue] |
+--------------------------------------------------+
```

Lyric state:

```text
+--------------------------------------------------+
|           [ Lyric ]                              |
|           [ Lyric ]                              |
|           [ Lyric ]                              |
|           [ Lyric ]                              |
|           [ Lyric ]                              |
|           [ Lyric ]                              |
|           [ Lyric ]                              |
+--------------------------------------------------+
| [Cover] Song Name                                |
| [Cover]      progress bar                        |
| [Shuffle/Seq/...] [<<] [Play/Pause] [>>] [Queue] |
+--------------------------------------------------+
```

### Narrow Mode Rules

- `Library` shows server-grouped folder-based playlists from all servers
- `My Collections` shows only local personal collections
- `Filter` opens a sheet, not a full new permanent panel
- `Add device` is shown only while `Library` is active
- `New collection` is shown only while `My Collections` is active
- only one of these is active at a time
- lyrics should open in the expandable upper player panel
- the lower player block should keep the same layout in cover and lyric states
- song sharing should live in the current queue sheet header, not in a narrow-only top action bar

### Mobile Offline Example

```text
+--------------------------------------------------+
| This Device                      [Online]    [⋮]|
|  所有音乐                                        |
|  mp3                                             |
|                                                  |
| Living Room PC                  [Offline]   [⋮]|
|  Cannot reach this source right now              |
|  [Retry]                                         |
+--------------------------------------------------+
```

### Mobile Filter Sheet

```text
+--------------------------------------------------+
| Filter Library                                   |
+--------------------------------------------------+
| Source                                           |
|  (x) All sources                                 |
|  ( ) This Device                                 |
|  ( ) Living Room PC                              |
|  ( ) Downloads                                   |
|                                                  |
| Type                                             |
|  [x] Songs                                       |
|  [ ] Videos                                      |
|                                                  |
| [Reset]                              [Apply]     |
+--------------------------------------------------+
```

### Mobile Add Device Sheet

```text
+--------------------------------------------------+
| Add Device                                       |
+--------------------------------------------------+
| Manual address                                   |
| [ 192.168.1.10:2080                        ] [Connect] |
|                                                  |
| Nearby devices                          [Refresh] |
|                                                  |
| localhost(self)                         [This Device] |
| http://localhost:2080/api                          |
|                                                  |
| Kaulan Player b31a7e                    [0 sec ago]  |
| http://192.168.136.124:2080/api                    |
+--------------------------------------------------+
```

Rules:

- show both discovered and manually added devices in one list
- manual address connect and discovery connect should share the same destination flow
- manual devices can be removed from the sheet
- this sheet replaces device onboarding inside settings

### Mobile Search

Search should support two behaviors in one flow:

- show local library results directly
- offer a clear `Search online` action in the result area

The top search entry should not force the user to choose online/offline first.

```text
+--------------------------------------------------+
| [< Back] Search                             [⋮]  |
+--------------------------------------------------+
| [ Search songs, playlists...                 ] |
+--------------------------------------------------+
| [ Search online for "anime ost" ]               |
|                                                  |
| Library results                                  |
|                                                  |
| Song A                              [This Device]|
| Song B                              [Living Room]|
| Song C                              [Downloads]  |
|                                                  |
+--------------------------------------------------+
```

If there are no local results:

```text
+--------------------------------------------------+
| [< Back] Search                             [⋮]  |
+--------------------------------------------------+
| [ Search songs, playlists...                 ] |
+--------------------------------------------------+
| No library results.                              |
|                                                  |
| [ Search online for "anime ost" ]               |
+--------------------------------------------------+
```

Online search should open the existing online-search panel, not replace it.

That panel already handles:

- server-scoped provider selection
- provider readiness status
- save directory selection for the selected search source
- preview
- lyrics selection
- download to the selected source library
- optional extra local copy when the selected source is not localhost

Search result list actions should match library detail actions:

- `⋮` opens a list action menu
- `Multi-select` enters batch selection mode for the current result list
- `Delete` is best-effort only and may be unavailable or fail for some sources

Search selection mode:

```text
+--------------------------------------------------+
| [Cancel]      Selected 3      [Add to Collection] |
+--------------------------------------------------+
| [x] Song A                           [This Device]|
| [x] Song B                           [Living Room]|
| [ ] Song C                           [Downloads]  |
+--------------------------------------------------+
```

### Online Search Panel

The online-search panel should keep the search flow visible and collapse provider management by default.

Recommended default layout:

```text
+--------------------------------------------------+
| Online Search                                    |
+--------------------------------------------------+
| [ Search songs, videos, lyrics...            ]   |
| [Search]                                         |
|                                                  |
| Current source: Living Room PC                   |
| Sources: [ ] YouTube [x] 网易云 [ ] Bilibili     |
| [Source Status ▾]                                |
|                                                  |
| Save to: [Selected Folder]                       |
+--------------------------------------------------+
| Results...                                       |
+--------------------------------------------------+
```

Rules:

- provider checkboxes default to none selected
- only providers already usable on the selected search source can be checked
- unavailable providers stay visible but disabled
- the search button stays disabled until at least one enabled provider is selected
- the selected source still comes from the source-group `⋮` menu, not from this panel

Expanded provider status:

```text
+--------------------------------------------------+
| Source Status ▴                                  |
+--------------------------------------------------+
| YouTube    Cookies configured   [Manage]         |
| 网易云      Session available    [Manage]         |
| Bilibili   Login required       [Manage]         |
+--------------------------------------------------+
```

Provider-specific management sheet:

```text
+--------------------------------------------------+
| 网易云 Account                                   |
+--------------------------------------------------+
| Status: Not logged in                            |
| [Login]                                          |
| [Sync Login]                                     |
| [Logout]                                         |
+--------------------------------------------------+
```

Default rules:

- keep provider selection visible in the main online-search panel
- collapse login and account operations under `Source Status`
- do not show large provider cards before search results
- keep directory selection visible because it affects download destination
- when downloading from a non-localhost source, ask whether the user also wants a local copy

### Source Group Action Menu

Library-management actions should live in each source group header, not as one shared global action area.

Example:

```text
Living Room PC                  [Offline]   [⋮]
```

Opening `⋮` shows actions for that source only:

```text
Refresh library
Upload music
Use for Online Search
Change directory
Retry connection
Source details
Delete source
```

Visibility should depend on source capabilities, not the current client device type.

Examples:

- show `Change directory` only if that source type supports directory switching
- show `Upload music` only if that source supports uploads
- show `Retry connection` only when the source is offline or reconnectable
- show `Use for Online Search` only if that source currently has at least one usable provider
- show a disabled `Current Online Search Source` state when that source is already selected

### Mobile Library Detail

```text
+-------------------------------------------------+
| [< Back] Library / RedmiNote7 [This Device] [⋮] |
+-------------------------------------------------+
| Song A                                          |
| Song B                                          |
| Song C                                          |
|                                                 |
| Song row menu still supports add to collection  |
+-------------------------------------------------+
```

## Wide Mode

Wide mode can show browsing and playback together, but the library should still be aggregated at the folder-playlist level.

### Desktop Main Screen

`Library` tab:

```text
+------------------------------------------------------------------------------------------+
| Kaulan                  [ Search all music sources...                 ] [⋮]              |
+------------------------------------------------------------------------------------------+
| [Library] [My Collections] [Filter] [Add device] | Now Playing / Lyrics                  |
+---------------------------------------+--------------------------------------------------+
| This Device          [Online]    [⋮]  |              [ Cover ]                           |
|  所有音乐                             |              [ Cover ]                           |
|  mp3                                  |              [ Cover ]                           |
|  RedmiNote7                           |              [ Cover ]                           |
|                                       |--------------------------------------------------|
| Living Room PC       [Online]    [⋮]  | [Cover] Song Name                                |
|  Downloads                            | [Cover]      progress bar                        |
|  Anime                                | [Shuffle/Seq/...] [<<] [Play/Pause] [>>] [Queue] |
+---------------------------------------+--------------------------------------------------+
```

`My Collections` tab:

```text
+-----------------------------------------------------------------------------+
| Kaulan                  [ Search all music sources...                 ] [⋮] |
+-----------------------------------------------------------------------------+
| [Library] [My Collections] [Filter] [New Collection] | Now Playing / Queue  |
+------------------------------------------------------+----------------------+
| No personal collections yet.                                                |
| Use "New Collection" to create your first one.                              |
+------------------------------------------------------+----------------------+
```

### Desktop Offline Example

```text
+------------------------------------------------------------------------------------------+
| [Library] [My Collections] [Filter]   | Now Playing / Lyrics                             |
+---------------------------------------+--------------------------------------------------+
| This Device          [Online]    [⋮]  |                                                  |
|  所有音乐                             |                                                  |
|  mp3                                  |                                                  |
|                                       |--------------------------------------------------|
| Living Room PC       [Offline]   [⋮]  | [Cover] Song Name                                |
|  Cannot reach this source right now   | [Cover]      progress bar                        |
|  [Retry]                              | [Shuffle/Seq/...] [<<] [Play/Pause] [>>] [Queue] |
+---------------------------------------+--------------------------------------------------+
```

### Desktop With Library Detail Open

```text
+----------------------------------------------------------------------------------------+
| Kaulan                  [ Search all music sources...                 ] [⋮]            |
+----------------------------------------------------------------------------------------+
| Library / RedmiNote7 [This Device]  | Now Playing / Lyrics                             |
+-------------------------------------+--------------------------------------------------+
| Song A                              |              [ Cover ]                           |
| Song B                              |              [ Cover ]                           |
| Song C                              |              [ Cover ]                           |
|                                     |              [ Cover ]                           |
|                                     |              [ Cover ]                           |
|                                     |--------------------------------------------------|
|                                     | [Cover] Song Name                                |
|                                     | [Cover]      progress bar                        |
|                                     | [Shuffle/Seq/...] [<<] [Play/Pause] [>>] [Queue] |
+-------------------------------------+--------------------------------------------------+
```

### Desktop With Lyrics Open

```text
+----------------------------------------------------------------------------------------+
| Kaulan                  [ Search all music sources...                 ] [⋮]            |
+----------------------------------------------------------------------------------------+
| Library / RedmiNote7 [This Device]  | Now Playing / Lyrics                             |
+-------------------------------------+--------------------------------------------------+
| Song A                              |                    [ Lyric ]                     |
| Song B                              |                    [ Lyric ]                     |
| Song C                              |                    [ Lyric ]                     |
|                                     |                    [ Lyric ]                     |
|                                     |                    [ Lyric ]                     |
|                                     |--------------------------------------------------|
|                                     | [Cover] Song Name                                |
|                                     | [Cover]      progress bar                        |
|                                     | [Shuffle/Seq/...] [<<] [Play/Pause] [>>] [Queue] |
+-------------------------------------+--------------------------------------------------+
```

### Desktop With Collections Active

```text
+------------------------------------------------------------------------------------------+
| Kaulan                  [ Search all music sources...                 ] [⋮]              |
+------------------------------------------------------------------------------------------+
| [Library] [My Collections] [Filter]   | Now Playing / Queue                              |
+---------------------------------------+--------------------------------------------------+
| My Collections                        |              [ Cover ]                           |
| Favorites                             |              [ Cover ]                           |
| Sleep                                 |              [ Cover ]                           |
| Driving                               |              [ Cover ]                           |
|                                       |              [ Cover ]                           |
| Favorites Contents                    |--------------------------------------------------|
| Song A                                | [Cover] Song Name                                |
| Song C                                | [Cover]      progress bar                        |
| Song F                                | [Shuffle/Seq/...] [<<] [Play/Pause] [>>] [Queue] |
+---------------------------------------+--------------------------------------------------+
```

## Server Visibility

The user should know where music came from, but should not be forced to manage a current active server all the time.

Use clear source presentation:

```text
This Device                    [Online]    [⋮]
  mp3
  RedmiNote7

Living Room                    [Offline]   [⋮]
  Downloads
```

Recommended badge behavior:

- always visible in server group headers
- always visible in search results (because search is cross-source)
- not shown on song rows in library detail or collection detail — the top bar already identifies the source
- optional in queue and now playing
- tap or click badge to filter by that source

Recommended playback-utility placement:

- current-queue-specific actions such as song sharing belong in the queue sheet header
- the trigger should use a share icon instead of a raw link icon because the user intent is an action, not URL inspection

### Source Capability Rules

Per-source menu actions should be capability-driven.

Suggested capability flags:

- canRefresh
- canUpload
- canChangeDirectory
- canUseForOnlineSearch
- isCurrentOnlineSearchSource
- canRetryConnection
- canShowSourceDetails
- canDeleteSource

The UI should render actions from these capabilities instead of assuming all servers behave the same.

Implemented interaction rules:

- `Use for Online Search` only appears when `canUseForOnlineSearch` is true
- `Current Online Search Source` is shown as a disabled state, not a second action
- when a newly added source supports online use, the app asks whether it should become the default online source
- when the selected online source is removed, the default immediately falls back to `http://localhost:2080/api`

### Library Row Identity

Two servers may expose the same folder name, so a library row should be identified by:

```text
sourceKey + playlistName
```

not only:

```text
playlistName
```

Example:

```text
This Device / Downloads
Living Room / Downloads
```

These are two different library rows.

## Collection Design

Collections should be personal and local by default.

### Why Local Collections

- a family member should not modify another person’s collections
- collections are closer to bookmarks than shared library structure
- local storage avoids backend synchronization conflicts

### Local Collection Data Shape

Collections should not identify songs by display name only.

Bad:

```json
{ "name": "song.mp3" }
```

Better:

```json
{
  "source": "living-room-pc",
  "songId": 123
}
```

Or:

```json
{
  "serverUrl": "http://192.168.1.20:2080/api",
  "songId": 123
}
```

Best option is a stable source key plus song id:

```json
{
  "sourceKey": "living-room-pc",
  "songId": 123
}
```

This avoids collisions when two servers have the same file names.

## Collection Actions

Batch song actions should use the same pattern in library detail, search results, and collection detail.

### Recommended Entry Points

- library detail header `⋮` menu
- search results header `⋮` menu
- collection detail header `⋮` menu

### List Action Menu

```text
+--------------------------------------------------+
| [< Back] Library / RedmiNote7                [⋮] |
+--------------------------------------------------+
| Song A                                           |
| Song B                                           |
| Song C                                           |
+--------------------------------------------------+
| Multi-select                                     |
| Delete                                           |
+--------------------------------------------------+
```

Tap the header `⋮` button to open this menu.

### Library Or Search Selection Mode

```text
+--------------------------------------------------+
| [Cancel]      Selected 2      [Add to Collection] |
+--------------------------------------------------+
| [x] Song A                                       |
| [x] Song B                                       |
| [ ] Song C                                       |
+--------------------------------------------------+
```

### Collection Selection Mode

```text
+--------------------------------------------------+
| [Cancel]   Selected 2   [Remove from Collection] |
+--------------------------------------------------+
| [x] Song A                                       |
| [x] Song C                                       |
| [ ] Song F                                       |
+--------------------------------------------------+
```

Deletion note:

- `Delete` should be documented as best-effort
- some sources may disable it entirely
- Android MediaStore-backed sources do not currently implement deletion, so delete may fail there

### Add To Collection Modal

```text
+--------------------------------------------------+
| Add to My Collection                             |
+--------------------------------------------------+
| [ ] Favorites                                    |
| [x] Driving                                      |
| [ ] Sleep                                        |
|                                                  |
| [Create New Collection]                          |
|                                                  |
| [Cancel]                           [Confirm]     |
+--------------------------------------------------+
```

### My Collections Screen

```text
+--------------------------------------------------+
| My Collections                            [ + ]  |
+--------------------------------------------------+
| Favorites                                        |
| Driving                                          |
| Sleep                                            |
|                                                  |
| Select one collection to view its songs          |
+--------------------------------------------------+
```

### Collection Detail

```text
+--------------------------------------------------+
| [< Back] Favorites                         [⋮]   |
+--------------------------------------------------+
| Song A                                           |
| Song C                                           |
| Song F                                           |
+--------------------------------------------------+
```

Collection overflow menu:

```text
Multi-select
Rename collection
Delete collection
```

## Settings Design

Normal settings should be short and non-technical.

### Settings

```text
+--------------------------------------------------+
| Settings                                         |
+--------------------------------------------------+
| Playback                                         |
|  - Loudness mode                                 |
|  - Target LUFS / manual volume                   |
|  - Lyrics on/off                                 |
|  - Sleep timer                                   |
|                                                  |
| Personal                                         |
|  - Manage my collections                         |
|                                                  |
| [Advanced Settings]                              |
+--------------------------------------------------+
```

### Advanced Settings

```text
+--------------------------------------------------+
| Advanced Settings                                |
+--------------------------------------------------+
| Device / source                                  |
|  - Local device name                             |
|  - Connected sources list                        |
|  - Source diagnostics                            |
|                                                  |
| Scan / backend                                   |
|  - Media type filter                             |
|  - Refresh internals                             |
|                                                  |
| Android / device integration                     |
|  - Local lyrics permission                       |
|  - Headset media button toggle                   |
+--------------------------------------------------+
```

Playback detail for LUFS-related controls:

- loudness mode stays in normal settings because it directly changes playback behavior
- `Auto` uses the current playlist LUFS values to normalize against the quietest track
- `Fixed LUFS` exposes a target loudness numeric input
- `Manual` exposes direct volume tuning when LUFS normalization is not wanted
- optional per-song LUFS number display remains advanced because it is diagnostic, not a primary playback control

## What Moves Out Of The Main Screen

Move away from default main UI:

- choosing one active server
- manual server URL editing
- device discovery list
- raw source details
- music directory path
- LUFS number display
- loudness internals
- backend/database terminology
- shared global library-management action block

Keep visible in normal use:

- grouped folder-based library
- source badges
- personal collections
- search
- queue
- song sharing from the queue / now playing context
- lyrics entry
- source-group `⋮` actions
- header-based batch song actions

## Suggested Frontend Refactor

### `App.vue`

Reduce `App.vue` to page orchestration and shared playback state.

It should no longer directly mix:

- settings internals
- library management internals
- lyric panel logic
- collection editing mode
- search mode

### Suggested High-level Components

- `LibraryView`
- `CollectionView`
- `CollectionDetailView`
- `SearchView`
- `NowPlayingView`
- `SettingsView`
- `AdvancedSettingsView`
- `LibraryFilterSheet`

### Existing Component Reuse

- `SongListView.vue`
  - reuse for library detail songs and personal collection detail lists
- `PlaylistListView.vue`
  - likely repurpose into merged folder-based library rows or personal collection lists
- `SettingsModal.vue`
  - split into normal settings and advanced settings
- `SearchBar.vue`
  - keep as a search entry, but use global cross-server semantics

## Summary

The proposed design changes the app from:

```text
pick one server
then browse that server
then share collections with everyone
```

to:

```text
see one combined library page
see one combined library page grouped by server
open folder-based playlists from any server
notice immediately when one server is offline
keep my own collections private on this device/browser
```

That model is simpler for normal users and better aligned with multi-server family usage.
