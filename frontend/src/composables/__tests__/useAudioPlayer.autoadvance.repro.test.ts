/**
 * Regression: web auto-advance (song ends -> next song) must keep playing.
 *
 * Safari/WebKit only let play() start on an HTMLAudioElement that a user
 * gesture has "unlocked". Auto-advance has no gesture, so a brand-new element
 * created for the next song is blocked with NotAllowedError and sits paused.
 * The fix reuses a single unlocked element and swaps its `src`, which Safari
 * allows. This fake models that policy: play() on a fresh, unlocked element
 * rejects with NotAllowedError; play() on an element a gesture unlocked
 * succeeds.
 *
 * Related documentation: docs/web-playback-isplaying.md
 *
 * @module composables/__tests__/useAudioPlayer.autoadvance-repro
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

/**
 * Models Safari's autoplay policy. A user gesture "unlocks" whichever element
 * play() is called on during the gesture; an unlocked element can be played
 * again later (e.g. on auto-advance), but a never-unlocked element rejects
 * play() with NotAllowedError. Distinct element per `new Audio()`.
 */
function createSafariLikeAudioFactory() {
  const created: FakeAudio[] = [];
  let gestureActive = false;

  const withUserGesture = async <T>(fn: () => Promise<T>): Promise<T> => {
    gestureActive = true;
    try {
      return await fn();
    } finally {
      gestureActive = false;
    }
  };

  class FakeAudio {
    src = "";
    paused = true;
    ended = false;
    readyState = 0;
    networkState = 0;
    duration = 0;
    currentTime = 0;
    volume = 1;
    error: MediaError | null = null;
    unlocked = false;
    private listeners: Record<string, Array<(event?: unknown) => void>> = {};

    constructor() {
      created.push(this);
    }

    addEventListener(event: string, cb: (event?: unknown) => void) {
      if (!this.listeners[event]) this.listeners[event] = [];
      this.listeners[event].push(cb);
    }

    removeEventListener() {}

    fire(event: string) {
      (this.listeners[event] ?? []).forEach((cb) => cb());
    }

    play(): Promise<void> {
      if (gestureActive) {
        this.unlocked = true;
      }
      if (!this.unlocked) {
        const err = new Error(
          "play() can only be initiated by a user gesture.",
        );
        err.name = "NotAllowedError";
        return Promise.reject(err);
      }
      return new Promise<void>((resolve) => {
        setTimeout(() => {
          this.paused = false;
          this.ended = false;
          this.readyState = 4;
          this.fire("playing");
          resolve();
        }, 0);
      });
    }

    pause() {
      const wasPlaying = !this.paused;
      this.paused = true;
      if (wasPlaying) this.fire("pause");
    }
  }

  return { FakeAudio, created, withUserGesture };
}

describe("useAudioPlayer - web auto-advance on song end", () => {
  let mockSongs: MusicInfo[];

  beforeEach(() => {
    mockSongs = [
      { id: 1, name: "Test Song 1", lufs: -12, path: "/test/song1.mp3" },
      { id: 2, name: "Test Song 2", lufs: null, path: "/test/song2.mp3" },
      { id: 3, name: "Test Song 3", lufs: null, path: "/test/song3.mp3" },
    ];
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

  const flush = () => new Promise((resolve) => setTimeout(resolve, 120));

  it("reuses the unlocked element so auto-advance is not blocked", async () => {
    const factory = createSafariLikeAudioFactory();
    global.Audio = vi.fn(
      () => new factory.FakeAudio(),
    ) as unknown as typeof Audio;

    const { playSongAtIndex, isPlaying, currentSong } = useAudioPlayer({
      songs: () => mockSongs,
    });

    // User taps song 1 (gesture) -> unlocks the element and plays.
    await factory.withUserGesture(() =>
      playSongAtIndex(mockSongs[0], 0, mockSongs),
    );
    await flush();
    expect(currentSong.value?.id).toBe(1);
    expect(isPlaying.value).toBe(true);
    expect(factory.created.length).toBe(1);

    // Track ends -> auto-advance. No gesture, but the element is reused.
    factory.created[0].fire("pause");
    factory.created[0].fire("ended");
    await flush();

    expect(currentSong.value?.id).toBe(2);
    expect(isPlaying.value).toBe(true);
    // Still the same single element (no fresh element to block).
    expect(factory.created.length).toBe(1);
  });

  it("keeps playing across a second auto-advance", async () => {
    const factory = createSafariLikeAudioFactory();
    global.Audio = vi.fn(
      () => new factory.FakeAudio(),
    ) as unknown as typeof Audio;

    const { playSongAtIndex, isPlaying, currentSong } = useAudioPlayer({
      songs: () => mockSongs,
    });

    await factory.withUserGesture(() =>
      playSongAtIndex(mockSongs[0], 0, mockSongs),
    );
    await flush();

    factory.created[0].fire("pause");
    factory.created[0].fire("ended");
    await flush();
    expect(currentSong.value?.id).toBe(2);
    expect(isPlaying.value).toBe(true);

    factory.created[0].fire("pause");
    factory.created[0].fire("ended");
    await flush();
    expect(currentSong.value?.id).toBe(3);
    expect(isPlaying.value).toBe(true);
    expect(factory.created.length).toBe(1);
  });
});
