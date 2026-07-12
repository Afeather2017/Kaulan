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
});
