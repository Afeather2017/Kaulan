# Search

## Overview

The frontend provides a search box that lets users find songs by name. The search runs when the user presses Enter or clicks the search button.

## Behavior

- If a playlist is open, search only within that playlist.
- If the user is in the playlists view, search uses the "所有音乐" playlist (all music) when available.
- Otherwise, search across all songs in all playlists.
- In narrow layout, opening search collapses the player panel first so the result list is immediately visible.
- Back from search restores the last visible panel instead of forcing a jump to playlists.
- If no results are found, the UI shows a clear empty-state message.
- When a user plays a song from search results, the search results become the playback list until a playlist is selected.

## Data Flow

1. User types a query in the search box.
2. User triggers search (Enter or Search button).
3. The app computes search results in-memory and switches to the search view.
4. The search view renders results or the empty-state message.

## Related Source Files

- `frontend/src/components/SearchBar.vue`
- `frontend/src/composables/useAppShell.ts`
- `frontend/src/App.vue`
