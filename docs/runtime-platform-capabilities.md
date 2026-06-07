# Runtime Platform Capabilities

This document describes how Kaulan centralizes frontend runtime detection and platform-specific feature flags.

## Related Source Files

- `frontend/src/utils/platform.ts`
- `frontend/src/main.ts`
- `frontend/src/App.vue`
- `frontend/src/composables/useAudioPlayer.ts`
- `frontend/src/components/modals/SettingsModal.vue`

## Overview

Kaulan supports two frontend runtime environments:

- `web` - normal browser and desktop web usage
- `android` - Android Tauri WebView with the Android playback backend

Instead of scattering direct `isAndroid` checks across components, the frontend now resolves a cached runtime profile from `frontend/src/utils/platform.ts`.

The runtime profile contains:

- `platform` - `web` or `android`
- `capabilities` - explicit booleans describing what the runtime supports

## Why This Exists

Direct platform checks spread runtime policy across unrelated components. That causes duplication such as:

- deciding whether to start the foreground music service
- deciding whether Android back handling is available
- deciding whether localhost playlist requests should expose raw `content://` playback paths
- deciding whether Android-only settings should be shown

Centralizing these rules makes the UI ask "what is supported here?" instead of repeating "am I on Android?"

## Runtime Profile

The frontend resolves a `RuntimeProfile` once and caches it for reuse.

Current platform values:

- `android`
- `web`

Current capability values:

- `usesAndroidPlaybackBackend`
- `supportsAndroidBackHandler`
- `supportsForegroundMusicService`
- `supportsExitAppOnTimer`
- `supportsLocalLyricsPermission`
- `supportsHeadsetMediaButtonControl`
- `supportsRawContentPlayback`

## Current Usage

### App bootstrap

`frontend/src/main.ts` reads the runtime profile before mounting the app.

It uses capabilities to:

- start the Android foreground music service only when supported
- apply Android WebView touch behavior only when the Android playback runtime is active

### Android back handling

The app shell registers the Android back listener only when `supportsAndroidBackHandler` is true. The actual registration now lives behind `useAndroidBackNavigation.ts`, and `App.vue` consumes it through `useAppShell.ts`.

### Playback initialization

`frontend/src/stores/player.ts` and `frontend/src/composables/useAudioPlayer.ts` use `usesAndroidPlaybackBackend` to choose the playback engine:

- Android plugin session polling
- browser `HTMLAudioElement`

This does not change the playback architecture. It only moves the runtime decision into a shared helper.

### Raw localhost playback

`supportsRawContentPlayback` controls when playlist requests may use `?stream=content` for Android localhost direct-play behavior.

This keeps the Android-specific raw-path rule in one place.

### Settings UI

`frontend/src/components/modals/SettingsModal.vue` uses capability flags instead of a local `isAndroid` ref to decide whether to render:

- exit-app-on-timer
- local lyrics permission
- headset media button control

## Design Rule

When adding new platform-specific behavior:

1. Add or reuse a named capability in `frontend/src/utils/platform.ts`.
2. Let callers depend on that capability instead of checking `android` directly.
3. Keep direct runtime branching only where the underlying implementation truly differs, such as playback engine behavior.

## Non-Goals

This runtime capability layer does not eliminate all platform-specific code.

It does **not** replace:

- Android playback session polling
- browser `HTMLAudioElement`
- Android-only Tauri APIs

Those differences remain real implementation boundaries. The goal is to centralize the decision, not pretend the runtimes behave the same.
