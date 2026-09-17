# Playback Interface

The unified playback contract shared by the web backend (HTMLAudioElement)
and the Android backend (native `MusicPlayerService` via the
`tauri-plugin-music-notification` submodule). Introduced by the
`refactor/playback-state-interface` refactor, which replaced the old
poll-and-patch layer in `composables/useAudioPlayer.ts`.

This document supersedes the authority rules described in
`docs/web-playback-isplaying.md` and `docs/android/playback-session.md`
(those remain for history; their watcher/drift-guard mechanics no longer exist).

## Modules

| File | Role |
|---|---|
| `frontend/src/playback/types.ts` | The `PlaybackBackend` interface, `PlaybackSnapshot`, `BackendEvent` |
| `frontend/src/playback/shared.ts` | Song identity, queue building, session persistence |
| `frontend/src/playback/engine.ts` | The single owner of frontend playback state |
| `frontend/src/playback/web.ts` | Web backend over one reused HTMLAudioElement |
| `frontend/src/playback/android.ts` | Android backend over the plugin event stream |
| `frontend/src/playback/__tests__/conformance.test.ts` | Same-scenarios-both-backends suite |

## Contract rules

1. **Single authority per domain.** The backend owns transport state: status,
   position, duration, the loaded song, and auto-advance. The engine owns the
   queue order, play mode, and target index.
2. **One-way flow.** UI → engine commands → backend commands → backend events
   → `engine.applySnapshot()` → reactive refs → UI. Nobody writes engine refs
   from outside `applySnapshot`, and backends never read app state.
3. **Events, not polling.** Android pushes `playback:event` snapshots on every
   native transition plus a 250 ms position tick while playing (JNI → Rust
   `app.emit`). Web maps element media events to the same stream. The old 1 s
   `getPlaybackSession` poll is gone; `getSession()` remains as a one-shot
   resync (init, visibility change, unknown song).
4. **Explicit intents.** Commands carry their semantics in their signature —
   e.g. `seek(sec, { playWhenReady = true })`. There is no `if (isPlaying)`
   feature logic anywhere.

## State model

```ts
type PlaybackStatus = "idle" | "loading" | "playing" | "paused" | "error";

interface PlaybackSnapshot {
  status: PlaybackStatus;
  song: MusicInfo | null;   // resolved metadata of the loaded song
  positionSec: number;
  positionAtMs: number;     // wall clock sample time → UI interpolates
  durationSec: number;
  currentIndex: number;
  playMode: PlayMode;
  error?: PlaybackError;
  queue?: MusicInfo[];      // Android only: adopt converted native queue
}
```

`isPlaying` is a derived value (`status === "playing"`). Every snapshot
carries `positionAtMs` so the engine's 250 ms ticker interpolates the
displayed position between updates on both platforms.

### Event derivation

Backends emit `snapshot` and `songEnded` events. The engine derives song
starts from identity changes between snapshots and fires `onSongStart`
(identity-deduped) which drives LUFS pre-caching. Completion handling differs
by platform *inside* the backends but is identical to the UI:

- **Web**: element `ended` → `songEnded(completed)` → the engine computes the
  next index (play-mode aware) and issues `skipTo`. The webview is always
  alive, so a round trip is fine.
- **Android**: the service must advance on its own (the webview can be
  backgrounded or the process restarted), so completion advances natively and
  the engine simply reconciles the arriving snapshots.

## Product rules encoded here

- **Seek while paused resumes.** `seek(sec)` defaults to
  `playWhenReady: true` on both backends. Lyric clicks and progress-bar seeks
  are the same one command; no caller passes `playWhenReady: false` yet.
- **Seek before metadata** falls back to a full reload from the target time
  (`engine.seekToTime` → `playSong(song, time)` when `duration <= 0`); web
  additionally parks `pendingPlaySeekTime` for `loadedmetadata`.
- **Seek pinning.** After a seek, the displayed position holds the target for
  up to 1.5 s until a snapshot within 0.35 s acks it (replaces the old
  Android-only drift guard).
- **Lyric edit** pauses on entry and never auto-resumes (unchanged UX).
- **Replaying the current song** (`playSong` on the active song with no seek
  target) restarts from the top on both backends.

## Android plugin surface (see submodule `feat/playback-event-stream`)

- `onPlaybackEvent(cb)` — subscribes to `playback:event` (JSON snapshots with
  `reason: tick|transition|error`).
- `playTrackAtIndex(index, autoPlay, startAtMs)` — targeted switch reusing the
  native queue; `autoPlay=false` prepares the track paused
  (`pendingResumeAfterPrepare` in Kotlin).
- `seek(position, { autoPlay })` — seeking no longer implies playback;
  `seekAndPlay` remains for compatibility.
- `getPlaybackSession().runtime.status` — native transport status so a
  restored session (player dead) reports `idle`, not `paused`.

## Testing

`conformance.test.ts` runs identical scenarios against both backends using a
stateful `FakeMediaElement` and a stateful `FakeNativeService`: load/play,
seek-while-paused resumes, lyric-click, pre-metadata seeks, restore + resume,
pause/play, rapid skips, sequential wrap, song-start callbacks, web
ended/loop/error advance, Android native advance and skip-without-re-push.
When adding a playback feature, extend the conformance suite first — both
backends must pass the same scenarios.
