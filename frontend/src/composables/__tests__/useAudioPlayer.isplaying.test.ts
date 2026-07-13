/**
 * Repro tests: clicking a song should flip isPlaying to true.
 *
 * Related documentation: docs/web-playback-isplaying.md
 *
 * @module composables/__tests__/useAudioPlayer.isplaying-repro
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import { useAudioPlayer, type MusicInfo } from "../useAudioPlayer";
import { removeStoredPlaybackSession } from "@/utils/storage";

vi.mock("@/utils/platform", () => ({
  getRuntimeCapabilities: () =>
    Promise.resolve({
      usesAndroidPlaybackBackend: false,
      supportsAndroidBackHandler: false,
      supportsForegroundMusicService: false,
      supportsExitAppOnTimer: false,
      supportsLocalLyricsPermission: false,
      supportsHeadsetMediaButtonControl: false,
      supportsRawContentPlayback: false,
    }),
  isLocalhostApiBase: (apiBase: string) => {
    try {
      const hostname = new URL(apiBase).hostname;
      return (
        hostname === "localhost" ||
        hostname === "127.0.0.1" ||
        hostname === "::1"
      );
    } catch {
      return false;
    }
  },
}));

vi.mock("@/utils/api", () => ({
  getLocalApiBase: () => "http://localhost:2080/api",
  resolveSourceApiBase: (sourceKey?: string | null) =>
    sourceKey || "http://localhost:2080/api",
}));

describe("useAudioPlayer - click from playlist flips isPlaying (web)", () => {
  let mockSongs: MusicInfo[];
  let audioMock: {
    play: ReturnType<typeof vi.fn>;
    pause: ReturnType<typeof vi.fn>;
    addEventListener: ReturnType<typeof vi.fn>;
    removeEventListener: ReturnType<typeof vi.fn>;
    src?: string;
  };

  beforeEach(() => {
    mockSongs = [
      { id: 1, name: "Test Song 1", lufs: -12, path: "/test/song1.mp3" },
      { id: 2, name: "Test Song 2", lufs: null, path: "/test/song2.mp3" },
    ];

    audioMock = {
      play: vi.fn().mockResolvedValue(undefined),
      pause: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    };
    global.Audio = vi.fn(() => audioMock) as unknown as typeof Audio;
    global.fetch = vi.fn(async () => ({
      ok: true,
      json: async () => ({}),
    })) as unknown as typeof fetch;

    const storage = new Map<string, string>();
    Object.defineProperty(globalThis, "localStorage", {
      value: {
        getItem: vi.fn((key: string) => storage.get(key) ?? null),
        setItem: vi.fn((key: string, value: string) => {
          storage.set(key, value);
        }),
        removeItem: vi.fn((key: string) => {
          storage.delete(key);
        }),
      },
      configurable: true,
    });
    removeStoredPlaybackSession();
  });

  it("sets isPlaying=true after playSong (single click)", async () => {
    const { playSong, isPlaying } = useAudioPlayer({
      songs: () => mockSongs,
    });

    expect(isPlaying.value).toBe(false);
    await playSong(mockSongs[0]);
    expect(isPlaying.value).toBe(true);
  });

  it("sets isPlaying=true after playSongAtIndex with queue+index (playlist click)", async () => {
    const { playSongAtIndex, isPlaying, currentSong } = useAudioPlayer({
      songs: () => mockSongs,
    });

    await playSongAtIndex(mockSongs[1], 1, mockSongs);

    expect(currentSong.value?.id).toBe(2);
    expect(isPlaying.value).toBe(true);
  });

  // Reproduces the browser-only bug: when the source is still loading the
  // browser interrupts play() with AbortError ("removed from the document").
  // isPlaying must follow the real "playing" state, not the rejected play()
  // promise. The catch must not throw, and on auto-advance (no user gesture to
  // resume the interrupted element) it must wait for the source to be ready and
  // start playback explicitly so the song does not sit paused.
  it("recovers and plays when play() rejects with AbortError", async () => {
    const abortError = new Error(
      "The play() request was interrupted by a call to pause().",
    );
    abortError.name = "AbortError";

    const listeners: Record<string, Array<(event?: unknown) => void>> = {};
    const rejectingAudio = {
      src: "",
      paused: true,
      ended: false,
      readyState: 0,
      networkState: 0,
      duration: 0,
      currentTime: 0,
      volume: 1,
      error: null as MediaError | null,
      play: vi.fn(() => {
        playCallCount += 1;
        if (playCallCount === 1) {
          // First attempt is interrupted while the source is still loading.
          // The source finishes loading shortly after, signaling "canplay".
          setTimeout(() => {
            rejectingAudio.readyState = 4;
            (listeners["canplay"] ?? []).forEach((cb) => cb());
          }, 0);
          return Promise.reject(abortError);
        }
        // Retry once the source is ready: playback actually starts.
        rejectingAudio.paused = false;
        (listeners["playing"] ?? []).forEach((cb) => cb());
        return Promise.resolve(undefined);
      }),
      pause: vi.fn(),
      addEventListener: vi.fn(
        (event: string, cb: (event?: unknown) => void) => {
          if (!listeners[event]) listeners[event] = [];
          listeners[event].push(cb);
        },
      ),
      removeEventListener: vi.fn(),
    };
    let playCallCount = 0;
    global.Audio = vi.fn(() => rejectingAudio) as unknown as typeof Audio;

    const { playSong, isPlaying } = useAudioPlayer({
      songs: () => mockSongs,
    });

    // Must not throw: AbortError is an interrupt, not an autoplay block.
    await playSong(mockSongs[0]);

    // The first attempt was interrupted, then retried once the source loaded.
    expect(rejectingAudio.play).toHaveBeenCalledTimes(2);
    expect(isPlaying.value).toBe(true);
  });

  // Reproduces the bug where seeking while paused left isPlaying=false after
  // the user pressed resume. After a seek the element sits at HAVE_CURRENT_DATA
  // and Chrome emits `play` (paused flipped) but never `playing` (data not yet
  // buffered for continuous advance). The UI must reflect that the user pressed
  // play, so the `play` event — not just `playing` — drives isPlaying=true.
  it("flips isPlaying=true on the `play` event after seek-while-paused (no `playing`)", async () => {
    const listeners: Record<string, Array<(event?: unknown) => void>> = {};
    const audio = {
      src: "",
      paused: true,
      ended: false,
      readyState: 0,
      networkState: 0,
      duration: 100,
      currentTime: 0,
      volume: 1,
      error: null as MediaError | null,
      play: vi.fn().mockResolvedValue(undefined),
      pause: vi.fn(),
      addEventListener: vi.fn(
        (event: string, cb: (event?: unknown) => void) => {
          if (!listeners[event]) listeners[event] = [];
          listeners[event].push(cb);
        },
      ),
      removeEventListener: vi.fn(),
    };
    global.Audio = vi.fn(() => audio) as unknown as typeof Audio;

    const { playSong, play, pause, seekToTime, isPlaying, duration } =
      useAudioPlayer({
        songs: () => mockSongs,
      });

    // 1. Start the song. play() resolves, then `playing` fires.
    await playSong(mockSongs[0]);
    audio.paused = false;
    audio.readyState = 4;
    audio.duration = 100;
    (listeners["loadedmetadata"] ?? []).forEach((cb) => cb());
    (listeners["playing"] ?? []).forEach((cb) => cb());
    expect(isPlaying.value).toBe(true);

    // 2. User pauses.
    await pause();
    audio.paused = true;
    (listeners["pause"] ?? []).forEach((cb) => cb());
    expect(isPlaying.value).toBe(false);

    // 3. User clicks the progress bar while paused.
    await seekToTime(40);
    expect(audio.currentTime).toBe(40);
    // After the seek the element is still paused but now at HAVE_CURRENT_DATA.
    audio.readyState = 2;
    expect(isPlaying.value).toBe(false);

    // 4. User clicks resume. The browser fires `play` but, because the seek
    //    left the element with only current-frame data, `playing` never fires.
    await play();
    audio.paused = false;
    (listeners["play"] ?? []).forEach((cb) => cb());

    expect(isPlaying.value).toBe(true);

    // 5. Pause must still work — the user can actually stop playback.
    await pause();
    audio.paused = true;
    (listeners["pause"] ?? []).forEach((cb) => cb());
    expect(isPlaying.value).toBe(false);
    expect(duration.value).toBe(100);
  });

  // Regression: rapid skips overlap playSong calls on the single reused
  // HTMLAudioElement. When a newer switch changes `src` while an older switch's
  // play() is still pending, the older promise rejects with AbortError. The
  // older switch must NOT then retry play() (it would act on the newer song's
  // src) or flip isPlaying/isPlayingInternal — it must bail, leaving the newer
  // switch in charge. Without the generation guard the stale retry issues a 3rd
  // play() (2 calls -> 3); with the guard it stays at 2.
  it("does not retry play() on a switch superseded mid-retry (rapid skip)", async () => {
    const abortError = new Error("The play() request was interrupted.");
    abortError.name = "AbortError";

    const listeners: Record<string, Array<(event?: unknown) => void>> = {};
    let playCallCount = 0;
    const deferredPlays: Array<{
      resolve: () => void;
      reject: (error: unknown) => void;
    }> = [];
    const controllableAudio = {
      src: "",
      paused: true,
      ended: false,
      readyState: 0,
      networkState: 0,
      duration: 0,
      currentTime: 0,
      volume: 1,
      error: null as MediaError | null,
      play: () => {
        playCallCount += 1;
        return new Promise<void>((resolve, reject) => {
          deferredPlays.push({
            resolve: () => {
              controllableAudio.paused = false;
              (listeners["playing"] ?? []).forEach((cb) => cb());
              resolve();
            },
            reject,
          });
        });
      },
      pause: () => {
        const wasPlaying = !controllableAudio.paused;
        controllableAudio.paused = true;
        if (wasPlaying) (listeners["pause"] ?? []).forEach((cb) => cb());
      },
      addEventListener: (event: string, cb: (event?: unknown) => void) => {
        (listeners[event] ??= []).push(cb);
      },
      removeEventListener: () => {},
    };
    global.Audio = vi.fn(() => controllableAudio) as unknown as typeof Audio;

    const { playSong, isPlaying, currentSong } = useAudioPlayer({
      songs: () => mockSongs,
    });

    // Song A: pass the pre-play wait so it calls play() (call 1), then leave its
    // play() promise pending.
    const aPromise = playSong(mockSongs[0]);
    await new Promise((resolve) => setTimeout(resolve, 70));
    expect(playCallCount).toBe(1);

    // A's play() is interrupted (as a newer src change would) -> AbortError. A
    // enters waitForAudioReady and suspends on canplay.
    deferredPlays[0].reject(abortError);
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Song B supersedes A while A waits for canplay. B calls play() (call 2).
    const bPromise = playSong(mockSongs[1]);
    await new Promise((resolve) => setTimeout(resolve, 70));
    expect(playCallCount).toBe(2);

    // B actually starts playing.
    deferredPlays[1].resolve();
    await new Promise((resolve) => setTimeout(resolve, 10));

    // The source finishes loading; this also unblocks A's waitForAudioReady.
    controllableAudio.readyState = 4;
    (listeners["canplay"] ?? []).forEach((cb) => cb());
    await new Promise((resolve) => setTimeout(resolve, 10));

    // A was superseded, so it must NOT have retried play() (would be 3 without
    // the generation guard). B is the active song and is playing.
    expect(playCallCount).toBe(2);
    expect(currentSong.value?.id).toBe(2);
    expect(isPlaying.value).toBe(true);

    // Drain any later deferred so the test never hangs if the guard is removed.
    for (const deferred of deferredPlays.slice(2)) {
      deferred.resolve();
    }
    await Promise.allSettled([aPromise, bPromise]);
  });

  // Reproduces the unhandled-rejection side effect of seek-while-paused: when
  // the user resumes and play() is still pending, pressing pause before the
  // seek-target buffer fills makes Chrome reject play() with AbortError. That is
  // benign — the pause listener already mirrored the state — so the resume
  // webBackend.play() must swallow AbortError instead of surfacing an unhandled
  // rejection. This exercises the play() catch (not playSong's), so a current
  // song + audio element must exist first.
  it("does not throw when resume play() rejects with AbortError because the user paused mid-resume", async () => {
    const abortError = new Error("The operation was aborted.");
    abortError.name = "AbortError";

    const listeners: Record<string, Array<(event?: unknown) => void>> = {};
    const audio = {
      src: "",
      paused: true,
      ended: false,
      readyState: 4,
      networkState: 4,
      duration: 100,
      currentTime: 40,
      volume: 1,
      error: null as MediaError | null,
      play: vi.fn().mockResolvedValue(undefined),
      pause: vi.fn(),
      addEventListener: vi.fn(
        (event: string, cb: (event?: unknown) => void) => {
          if (!listeners[event]) listeners[event] = [];
          listeners[event].push(cb);
        },
      ),
      removeEventListener: vi.fn(),
    };
    global.Audio = vi.fn(() => audio) as unknown as typeof Audio;

    const { playSong, play, pause, isPlaying } = useAudioPlayer({
      songs: () => mockSongs,
    });

    // Start playback, then pause. This establishes a current song and an audio
    // element so the call below takes the resume path (webBackend.play), not
    // playSong.
    await playSong(mockSongs[0]);
    await pause();
    expect(isPlaying.value).toBe(false);

    // The user clicks resume, then immediately pauses before the buffer fills.
    // play() rejects with AbortError; the resume handler must not propagate the
    // rejection and must reconcile isPlaying from the element's real state.
    audio.play = vi.fn(() => Promise.reject(abortError));
    audio.paused = true;
    await expect(play()).resolves.toBeUndefined();
    expect(isPlaying.value).toBe(false);
  });
});
