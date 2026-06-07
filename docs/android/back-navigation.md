# Android Back Navigation

## Overview

Kaulan handles the Android system back button in the frontend so it behaves like the visible `返回` buttons instead of closing the webview immediately.

- Related source: [`frontend/src/App.vue`](../../frontend/src/App.vue)
- Related source: [`frontend/src/utils/platform.ts`](../../frontend/src/utils/platform.ts)

The implementation is Android-only. Desktop browsers and non-Android Tauri targets do not register this handler.

## Why This Is Needed

Kaulan's main UI is mostly a single-page Vue app driven by reactive state:

- modal visibility flags such as `showSettings`
- panel visibility flags such as `showLyric`
- mode flags such as `selectMode`
- view state such as `currentView`

Without custom handling, Android back exits the Tauri webview directly. That skips the app's own UI state transitions.

## Frontend Flow

On mount, the app reads runtime capabilities from `frontend/src/utils/platform.ts`. Only when `supportsAndroidBackHandler` is true does it load Tauri's Android back listener and register `onBackButtonPress`.

```mermaid
sequenceDiagram
    participant Android as Android System Back
    participant Tauri as Tauri onBackButtonPress
    participant App as frontend/src/App.vue

    Android->>Tauri: back press
    Tauri->>App: handleAndroidBackPress()
    alt modal is open
        App->>App: closeTopOverlay()
    else song selection mode is active
        App->>App: disable selectMode
    else collection selection mode is active
        App->>App: disable collectionSelectMode
    else mobile lyric panel is open
        App->>App: hide lyric panel
    else current view is not playlists
        App->>App: back to playlists
    else webview has history
        App->>App: window.history.back()
    else root state
        App->>Tauri: close current window
    end
```

## Ordered Back Priority

Each back event closes only one layer. The handler returns immediately after the first matching state.

Current priority in [`frontend/src/App.vue`](../../frontend/src/App.vue):

1. Filter sheet
2. Source action sheet
3. Collection action sheet
4. Song action sheet
5. Active queue modal
6. Add device modal
7. Upload modal
8. Online search modal
9. Create collection modal
10. Add-to-collection modal
11. Settings modal
12. Song selection mode
13. Collection selection mode
14. Mobile player panel
15. Non-playlist view back to playlist view
16. Browser history back
17. Close Tauri window

This ordered check is what makes one back event affect only one visible page or panel.

## Modal Handling Pattern

Modal overlays are handled by a dedicated helper:

```ts
const closeTopOverlay = () => {
  if (showFilterSheet.value) {
    showFilterSheet.value = false
    return true
  }

  if (selectedSourceMenuGroup.value) {
    closeSourceMenu()
    return true
  }

  if (selectedCollectionMenuName.value) {
    closeCollectionMenu()
    return true
  }

  if (selectedSongMenuSong.value) {
    closeSongMenu()
    return true
  }

  if (showActiveQueueModal.value) {
    showActiveQueueModal.value = false
    return true
  }

  if (showAddDeviceModal.value) {
    showAddDeviceModal.value = false
    return true
  }

  if (showUploadModal.value) {
    showUploadModal.value = false
    return true
  }

  if (showOnlineSearchModal.value) {
    showOnlineSearchModal.value = false
    return true
  }

  if (showCreateCollection.value) {
    hideCreateCollectionModal()
    return true
  }

  if (showAddToCollection.value) {
    hideAddToCollectionModal()
    return true
  }

  if (showSettings.value) {
    hideSettingsModal()
    return true
  }

  return false
}
```

The important rule is: check from topmost overlay to lowest overlay, and stop after the first match.

## State-Based Page Handling Pattern

For non-modal pages, the app uses boolean flags and `currentView` instead of full route navigation.

Examples:

- `selectMode.value = false` exits song multi-select state
- `collectionSelectMode.value = false` exits collection multi-select state
- `showLyric.value = false` closes the mobile lyric panel
- `handleBackToPlaylists()` returns from songs or search to the playlist list

This is a good fit for mobile UIs with stacked panels inside one Vue page.

## Android-Only Safety

The Android back listener is registered only when the runtime capability `supportsAndroidBackHandler` is `true`.

This avoids:

- calling Tauri Android APIs in normal browser development
- crashing non-Android targets due to unavailable back-button APIs
- changing desktop web behavior

## Design Rule For New Panels

When adding a new page, panel, or modal that should respond to Android back:

1. Represent its visibility with explicit reactive state.
2. Add a close or reset function for that state.
3. Insert that check in `handleAndroidBackPress()` at the correct priority.
4. Return immediately after handling it.

If the new UI is a modal overlay, prefer adding it to `closeTopOverlay()`.

## Summary

Kaulan's Android back behavior is implemented as a single ordered state machine in the frontend. One back event does one thing:

- close the topmost modal,
- or exit the current mode,
- or move back one in-app view,
- or finally leave the app.
