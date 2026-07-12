/**
 * Regression tests for web LUFS volume normalization.
 *
 * Related documentation: docs/lufs-playback-flow.md
 *
 * @module composables/__tests__/useAudioPlayer-volume
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import { useAudioPlayer, type MusicInfo } from "../useAudioPlayer";
import { useVolume } from "../useVolume";
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

describe("useAudioPlayer - web LUFS volume normalization", () => {
  let songs: MusicInfo[];
  let audioMock: {
    src: string;
    paused: boolean;
    ended: boolean;
    readyState: number;
    networkState: number;
    duration: number;
    currentTime: number;
    volume: number;
    error: MediaError | null;
    play: ReturnType<typeof vi.fn>;
    pause: ReturnType<typeof vi.fn>;
    addEventListener: ReturnType<typeof vi.fn>;
    removeEventListener: ReturnType<typeof vi.fn>;
  };

  beforeEach(() => {
    songs = [
      { id: 1, name: "Normalized Song", lufs: -12, path: "/test/song1.mp3" },
    ];

    audioMock = {
      src: "",
      paused: true,
      ended: false,
      readyState: 4,
      networkState: 0,
      duration: 0,
      currentTime: 0,
      volume: 1,
      error: null,
      play: vi.fn(() => {
        audioMock.paused = false;
        return Promise.resolve(undefined);
      }),
      pause: vi.fn(() => {
        audioMock.paused = true;
      }),
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

  it("applies the LUFS-derived current volume, not the pre-cache count", async () => {
    const player = useAudioPlayer({
      songs: () => songs,
    });
    const volume = useVolume(player.currentSong, player.activeQueue);

    await player.playSong(songs[0]);

    const currentVolume = volume.calculateVolume();
    await player.syncNormalizationConfig(
      volume.volumeMode.value,
      volume.manualVolume.value,
      volume.fixedLufs.value,
      5,
      currentVolume,
    );

    expect(currentVolume).toBeLessThan(1);
    expect(audioMock.volume).toBeCloseTo(currentVolume, 6);
  });

  it("keeps normalized volume when loop replay reloads the same song", async () => {
    const player = useAudioPlayer({
      songs: () => songs,
    });
    const volume = useVolume(player.currentSong, player.activeQueue);

    await player.playSong(songs[0]);
    await player.syncNormalizationConfig(
      volume.volumeMode.value,
      volume.manualVolume.value,
      volume.fixedLufs.value,
      5,
      volume.calculateVolume(),
    );
    const normalizedVolume = audioMock.volume;

    await player.playSong(songs[0]);

    expect(global.Audio).toHaveBeenCalledTimes(1);
    expect(audioMock.volume).toBeCloseTo(normalizedVolume, 6);
  });
});
