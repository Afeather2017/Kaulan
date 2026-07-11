# Web Playback `isPlaying` State

## Overview

On desktop and in the browser the frontend plays audio with a detached
`HTMLAudioElement` (`new Audio()`). The play/pause icon in
[`PlayerControls.vue`](../frontend/src/components/PlayerControls.vue) and
[`AppPlayerView.vue`](../frontend/src/components/AppPlayerView.vue) is bound to
the `isPlaying` ref exposed by
[`frontend/src/composables/useAudioPlayer.ts`](../frontend/src/composables/useAudioPlayer.ts).

This document explains why `isPlaying` must follow the audio element's real
media events rather than the `play()` promise, and why a rejected `play()`
promise is **not** an autoplay block.

Related Android behavior is documented separately in
[`docs/android/playback-session.md`](android/playback-session.md).

## The Bug (browser only)

Symptom: tapping a song from a playlist played the audio, but the play/pause
icon stayed on "play" (`isPlaying` stayed `false`). It reproduced only in the
browser, not in the Android app or the desktop app.

Root cause, captured with the `[web-playback]` console logs:

1. `playSong` creates the element and calls `await newAudio.play()` while the
   remote source is still loading (`readyState` 0, `networkState` 2).
2. The browser interrupts that pending `play()` and the promise rejects with
   `AbortError` ("The play() request was interrupted ... removed from the
   document").
3. The element then loads and reaches the `playing` state on its own.
4. The old catch treated **every** rejection as "autoplay blocked": it set
   `isPlaying = false` and threw `PlaybackStartError`. So the UI read "paused"
   even though audio was playing.

Desktop/local loads finish fast enough that `play()` resolves first, so the
interrupt never happens there — which is why the bug was browser-only and
appeared even with a fast remote server.

## The Fix

Two changes in the `web` backend inside `useAudioPlayer.ts`:

1. **`AbortError` is not fatal.** In the `play()` catch, only
   `NotAllowedError` is treated as an autoplay block. `AbortError` reconciles
   `isPlaying` with the element's real `paused` state instead of forcing
   `false` / throwing.

2. **`isPlaying` mirrors media events.** `playing` and `pause` listeners set
   `isPlaying` from the element's actual state, so the UI can never disagree
   with the audio regardless of how the `play()` promise resolves. Each listener
   is scoped to the current element with `toRaw(audioElement.value) !==
   toRaw(newAudio)` so a stale/old element (kept around after a song switch)
   cannot flip the UI. `toRaw` is required because `ref()` wraps plain-object
   values in a reactive proxy; real `HTMLAudioElement` values are host objects
   and are not proxied, but `toRaw` keeps the identity check correct in both
   production and tests.

## Sequence Diagram

```mermaid
sequenceDiagram
    participant UI as Browser
    participant Player as useAudioPlayer.ts (web)
    participant Audio as HTMLAudioElement

    UI->>Player: Tap song (playSong)
    Player->>Audio: new Audio(); src = url
    Player->>Audio: await play()  (source still loading)
    Audio-->>Player: media:play, media:waiting
    Audio-->>Player: media:pause (browser interrupts)
    Audio-->>Player: play() rejects (AbortError)
    Player->>Player: catch: AbortError -> reconcile, do NOT throw
    Audio-->>Player: media:playing (recovered)
    Player->>Player: playing listener -> isPlaying = true
    Player-->>UI: Play/pause icon flips to "playing"
```

## Logging

The web path logs under the `[web-playback]` prefix: `playSong` entry,
`calling newAudio.play()`, `play() resolved` / `play() interrupted
(AbortError)` / `play() REJECTED`, the `isPlaying changed` watch, and the
`media:playing` / `media:pause` reconciliation.

## Related Source and Tests

- [`frontend/src/composables/useAudioPlayer.ts`](../frontend/src/composables/useAudioPlayer.ts) - `webBackend.playSong`, the `playing`/`pause` listeners, and the `isPlaying` watch
- [`frontend/src/composables/__tests__/useAudioPlayer.isplaying.test.ts`](../frontend/src/composables/__tests__/useAudioPlayer.isplaying.test.ts) - web click-to-play and `AbortError` recovery regression tests
