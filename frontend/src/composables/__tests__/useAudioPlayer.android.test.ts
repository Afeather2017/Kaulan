//! Unit tests for Android playback correction behavior in useAudioPlayer.
//!
//! Related documentation:
//! - `docs/lyric-sync-timing.md`

import { beforeEach, describe, expect, it, vi } from "vitest";

const plugin = {
  getPlaybackSession: vi.fn(),
  seek: vi.fn(() => Promise.resolve()),
  seekAndPlay: vi.fn(() => Promise.resolve()),
  setPlayingQueue: vi.fn(() => Promise.resolve()),
  stop: vi.fn(() => Promise.resolve()),
  play: vi.fn(() => Promise.resolve()),
  pause: vi.fn(() => Promise.resolve()),
  setPlayMode: vi.fn(() => Promise.resolve()),
  setNormalizationConfig: vi.fn(() => Promise.resolve()),
};

vi.mock("@/utils/api", () => ({
  getLocalApiBase: () => "http://localhost:2080/api",
  resolveSourceApiBase: (sourceKey?: string | null) =>
    sourceKey || "http://localhost:2080/api",
}));

vi.mock("@/utils/platform", () => ({
  checkIsAndroid: () => Promise.resolve(true),
}));

vi.mock("music-notification-api", () => plugin);

describe("useAudioPlayer - Android correction guard", () => {
  let useAudioPlayer: typeof import("../useAudioPlayer").useAudioPlayer;

  beforeEach(async () => {
    plugin.getPlaybackSession.mockReset();
    plugin.seek.mockClear();
    plugin.seekAndPlay.mockClear();
    plugin.setPlayingQueue.mockClear();
    plugin.stop.mockClear();
    plugin.play.mockClear();
    plugin.pause.mockClear();
    plugin.setPlayMode.mockClear();
    plugin.setNormalizationConfig.mockClear();

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

    const module = await import("../useAudioPlayer");
    useAudioPlayer = module.useAudioPlayer;
  });

  it("should ignore a stale position snapshot after Android seek", async () => {
    const songs = [
      { id: 1, name: "Test Song", lufs: -12, path: "/path/to/song.mp3" },
    ];

    plugin.getPlaybackSession
      .mockResolvedValueOnce({
        queue: { songs, currentIndex: 0 },
        currentSongId: 1,
        runtime: { isPlaying: true, positionMs: 0, durationMs: 180000 },
        playMode: "sequential",
      })
      .mockResolvedValueOnce({
        queue: { songs, currentIndex: 0 },
        currentSongId: 1,
        runtime: { isPlaying: true, positionMs: 50000, durationMs: 180000 },
        playMode: "sequential",
      });

    const {
      seekToTime,
      currentTime,
      duration,
      isPlaying,
      currentSong,
      isAndroidPlayer,
      refreshAndroidSession,
    } = useAudioPlayer({
      songs: () => songs,
      onSongEnd: vi.fn(),
      onSongStart: vi.fn(),
    });

    isAndroidPlayer.value = true;
    currentSong.value = songs[0];
    duration.value = 180;
    isPlaying.value = true;

    await seekToTime(50);

    expect(plugin.seek).toHaveBeenCalledWith(50000);
    expect(currentTime.value).toBe(50);

    await refreshAndroidSession("confirmed-seek");
    expect(currentTime.value).toBe(50);
  });
});
