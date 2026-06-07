# App Shell Architecture

This document describes how Kaulan's frontend app shell is split between Pinia stores, shell composables, and `App.vue`.

## Related Source Files

- `frontend/src/App.vue`
- `frontend/src/composables/useAppShell.ts`
- `frontend/src/stores/ui.ts`
- `frontend/src/stores/library.ts`
- `frontend/src/stores/player.ts`
- `frontend/src/stores/collections.ts`

## Overview

`App.vue` is now a thin shell component.

It is responsible for:

- rendering the top-level layout
- binding props and events to child components
- delegating behavior to `useAppShell()`

It no longer owns the majority of application state directly.

## State Ownership

### Pinia stores

The frontend uses Pinia for shared app and domain state:

- `ui.ts`
  - current view
  - active tab
  - selected playlist
  - shell modal visibility
  - player panel mode
  - upload target
  - scan state
- `library.ts`
  - source groups
  - library filters
  - search query
  - selected library source and playlist identifiers
  - online search source selection
  - runtime-driven raw content playback capability
- `player.ts`
  - playback queue and current song state
  - web and Android playback runtime bridge
  - volume mode and normalization inputs
  - sleep timer state
  - LUFS visibility toggle
- `collections.ts`
  - local collections
  - collection modal state
  - collection menu state

### Shell composables

The app shell still uses focused composables for orchestration and presentation:

- `useAppShell.ts`
  - coordinates stores and feature composables
  - handles startup sequence
  - provides the template-facing API used by `App.vue`
- `useAppShellLayout.ts`
  - wide-layout detection
  - player panel presentation state
  - cover fallback handling
- `useAndroidBackNavigation.ts`
  - Android back button registration
  - ordered close/back state machine
- `useQueueEditing.ts`
  - queue insertion helpers for song action-sheet operations

### Feature composables kept in place

The following feature composables remain active and are consumed by the player or shell layer:

- `useAudioPlayer.ts`
- `useLufs.ts`
- `useLyrics.ts`
- `useSelection.ts`
- `useTimer.ts`
- `useVolume.ts`

## Design Rules

### Put state in Pinia when

- multiple parts of the shell need it
- it represents shared app or feature state
- it should survive view switches inside the running app

### Keep state local when

- it is DOM-specific, such as scroll position or element handles
- it is a modal draft input
- it is tightly coupled to one rendered component

Examples that stay local:

- scroll restoration in `AppContentView.vue`
- `scrollTop` handling in `SongListView.vue`
- temporary modal text inputs

## Playback Boundary

The player store does not replace the existing playback architecture.

- web playback still uses `HTMLAudioElement`
- Android playback still uses the notification plugin session as the runtime source of truth

The store exposes reactive playback state to the shell, while `useAudioPlayer.ts` remains the engine boundary.
