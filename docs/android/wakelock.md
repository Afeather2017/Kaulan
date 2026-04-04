# Android Wake Lock

## Problem

On Android, the backend HTTP server would randomly freeze during music playback. The server runs on localhost and streams audio files to `MediaPlayer`. When the device enters deep sleep (CPU off), the server thread stops responding, causing playback to stall or fail to prepare new tracks.

## Root Cause

Android aggressively suspends the CPU when the screen is off to save battery. Even though the app runs a foreground service for the music notification, the CPU can still enter a low-power state between media session callbacks. When this happens, the local HTTP server cannot serve audio data, and `MediaPlayer.prepareAsync()` hangs until the CPU wakes up — observable as multi-second or indefinite freezes.

## Solution

A **partial wake lock** is acquired when the playback foreground service starts, and released when it stops. A partial wake lock keeps the CPU running while allowing the screen to turn off normally.

### How it works

```
Foreground Service starts (playback)
  └─> Wake lock acquired (PARTIAL_WAKE_LOCK)
        └─> CPU stays on → server stays responsive → streaming works
Foreground Service stops
  └─> Wake lock released
        └─> CPU can sleep normally
```

### Components

1. **`wakelock.rs`** — Android-only module that uses JNI to call `PowerManager.newWakeLock()` directly. Creates a partial wake lock (`PARTIAL_WAKE_LOCK = 0x00000001`) tagged `kaulan:playback`.

2. **`lib.rs`** — Acquires the wake lock in `Server::start()` (called by the foreground service), releases it in `Server::stop()`.

3. **`AndroidManifest.xml`** — Declares `android.permission.WAKE_LOCK` permission.

4. **`ndk-context`** crate — Provides access to the JVM and Android context from native Rust code, used to obtain `PowerManager`.

### Why partial wake lock

- **Partial wake lock** (`PARTIAL_WAKE_LOCK`) keeps the CPU on but lets the screen turn off. Correct for background music playback.
- **Full wake lock** (`SCREEN_BRIGHT_WAKE_LOCK`) keeps the screen on at full brightness. Deprecated and would drain battery — not suitable for a music player.

### Why no extra notification

A partial wake lock does not require a notification. Only foreground services require notifications, and the music notification plugin already provides one. The wake lock is independent and silent.

### Source files

- `frontend/src-tauri/src/wakelock.rs` — Wake lock implementation
- `frontend/src-tauri/src/lib.rs` — Integration with server lifecycle
- `frontend/src-tauri/Cargo.toml` — `ndk-context` dependency
- `frontend/src-tauri/gen/android/app/src/main/AndroidManifest.xml` — WAKE_LOCK permission

## Performance measurement

Timing instrumentation was added to `MusicPlayerService.kt` using the `KaulanPerf` log tag. Filter with:

```bash
adb logcat -s KaulanPerf
```

This shows three metrics per track switch:

| Metric | Meaning |
|---|---|
| `prepareAsync` | Time for `MediaPlayer.prepareAsync()` to buffer/decode audio |
| `playTrackInternal` | Time from entering `playTrackInternal()` to `onPrepared()` |
| `LUFS resolution` | Time spent resolving LUFS before playback starts |

Source: `tauri-plugin-music-notification/android/src/main/java/MusicPlayerService.kt`
