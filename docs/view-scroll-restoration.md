# View Scroll Restoration

This document describes how Kaulan preserves in-session scroll position when users move between library lists and playlist detail views.

## Related Source Files

- `frontend/src/components/AppContentView.vue`
- `frontend/src/components/SongListView.vue`

## Behavior

- The library tab keeps its own scroll offset.
- The collections tab keeps its own scroll offset.
- Each playlist detail view keeps its own scroll offset, keyed by playlist title.
- Search results keep their own scroll offset.

When the user opens a playlist and then returns to the previous list, the UI restores the last position instead of jumping back to the top.

## Why This Behavior

In a single-page app, list/detail navigation should usually preserve the user's place within the current session. Resetting to the top is reasonable only when the underlying dataset meaningfully changes, such as:

- Applying a new filter
- Switching to a different source
- Replacing the playlist content entirely

This matches common mobile music and file-browser behavior and reduces re-scrolling friction in long lists.
