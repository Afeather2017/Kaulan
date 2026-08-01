/**
 * Repro tests: Android click-to-play should flip isPlaying to true even when
 * the immediately-polled session snapshot lags behind plugin.play().
 *
 * @module composables/__tests__/useAudioPlayer.android-isplaying-repro
 */

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
  getRuntimeCapabilities: () =>
    Promise.resolve({
      usesAndroidPlaybackBackend: true,
      supportsAndroidBackHandler: true,
      supportsForegroundMusicService: true,
      supportsExitAppOnTimer: true,
      supportsLocalLyricsPermission: true,
      supportsHeadsetMediaButtonControl: true,
      supportsRawContentPlayback: true,
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

vi.mock("music-notification-api", () => plugin);

describe("useAudioPlayer - Android click flips isPlaying", () => {
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

  it("sets isPlaying=true after playSongAtIndex even if session lags", async () => {
    const songs = [
      {
        id: 1,
        name: "Test Song",
        lufs: -12,
        path: "content://media/external/audio/media/1",
        stream_url: "content://media/external/audio/media/1",
        source_key: "http://localhost:2080/api",
      },
    ];

    // Simulate the race: right after plugin.play() the media session still
    // reports isPlaying=false (ExoPlayer not yet transitioned to playing).
    plugin.getPlaybackSession.mockResolvedValue({
      queue: { songs, currentIndex: 0 },
      currentSongId: 1,
      runtime: { isPlaying: false, positionMs: 0, durationMs: 180000 },
      playMode: "sequential",
    });

    const { initAudio, playSongAtIndex, isPlaying } = useAudioPlayer({
      songs: () => songs,
    });

    await initAudio();
    await playSongAtIndex(songs[0], 0, songs);

    expect(plugin.seekAndPlay).toHaveBeenCalledWith(0);
    // The explicit play command must flip isPlaying even when the lagging
    // session snapshot reports not-playing.
    expect(isPlaying.value).toBe(true);
  });

  it("flips isPlaying back to false after an explicit pause", async () => {
    const songs = [
      {
        id: 1,
        name: "Test Song",
        lufs: -12,
        path: "content://media/external/audio/media/1",
        stream_url: "content://media/external/audio/media/1",
        source_key: "http://localhost:2080/api",
      },
    ];

    plugin.getPlaybackSession.mockResolvedValue({
      queue: { songs, currentIndex: 0 },
      currentSongId: 1,
      runtime: { isPlaying: false, positionMs: 0, durationMs: 180000 },
      playMode: "sequential",
    });

    const { initAudio, playSongAtIndex, pause, isPlaying } = useAudioPlayer({
      songs: () => songs,
    });

    await initAudio();
    await playSongAtIndex(songs[0], 0, songs);
    expect(isPlaying.value).toBe(true);

    await pause();
    expect(plugin.pause).toHaveBeenCalled();
    expect(isPlaying.value).toBe(false);
  });
});
