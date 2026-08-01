//! Unit tests for Android playback correction behavior in useAudioPlayer.
//!
//! Related documentation:
//! - `docs/lyric-sync-timing.md`

import { beforeEach, describe, expect, it, vi } from "vitest";
import { setStoredPlaybackSession } from "@/utils/storage";
import type { LibrarySourceGroup } from "@/types/library";

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

  it("should persist raw playback paths for localhost Android queues", async () => {
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
      queue: { songs: [], currentIndex: null },
      currentSongId: null,
      runtime: { isPlaying: false, positionMs: 0, durationMs: 0 },
      playMode: "sequential",
    });

    const { initAudio, playSongAtIndex } = useAudioPlayer({
      songs: () => songs,
    });

    await initAudio();
    await playSongAtIndex(songs[0], 0, songs);

    expect(plugin.setPlayingQueue).toHaveBeenCalled();
    const queueCalls = plugin.setPlayingQueue.mock.calls as Array<
      Array<unknown>
    >;
    const latestCall = queueCalls[queueCalls.length - 1];
    const latestQueue = (latestCall?.[0] ?? undefined) as
      | {
          songs?: Array<{
            path?: string | null;
            url?: string | null;
            sourceKind?: string | null;
          }>;
        }
      | undefined;
    expect(latestQueue?.songs?.[0]?.path).toBe(
      "content://media/external/audio/media/1",
    );
    expect(latestQueue?.songs?.[0]?.sourceKind).toBe("local_raw");
    expect(latestQueue?.songs?.[0]?.url).toBeUndefined();
  });

  it("should migrate an active legacy native queue when Resume follows discovery", async () => {
    const oldApiBase = "http://192.168.20.222:2080/api";
    const newApiBase = "http://192.168.20.221:2080/api";
    const deviceId = "stable-device-id";
    const legacySongs = [
      {
        id: 537,
        name: "Moved Server Song",
        path: "/music/moved-server-song.mp3",
        url: `${oldApiBase}/music/id/537`,
        lufs: -12,
      },
    ];
    const sourceGroups: LibrarySourceGroup[] = [
      {
        apiBase: newApiBase,
        sourceKey: newApiBase,
        device_id: deviceId,
        name: "Moved server",
        isLoading: false,
        isOnline: true,
        playlists: [],
        onlineProviderStatuses: [],
        capabilities: {
          canRefresh: true,
          canUpload: false,
          canChangeDirectory: false,
          canUseForOnlineSearch: false,
          isCurrentOnlineSearchSource: false,
          canRetryConnection: true,
          canShowSourceDetails: true,
          canDeleteSource: true,
        },
      },
    ];

    setStoredPlaybackSession({
      // Reproduces the broken upgrade where one legacy session poll had
      // already overwritten the shadow's stable device identity.
      currentDeviceId: null,
      currentSongId: 537,
      queue: [
        {
          device_id: "",
          song_id: 537,
          filename: "moved-server-song.mp3",
          name: "Moved Server Song",
          lufs: -12,
        },
      ],
      timestamp: Date.now(),
    });
    plugin.getPlaybackSession.mockResolvedValue({
      queue: { songs: legacySongs, currentIndex: 0 },
      currentSongId: 537,
      runtime: { isPlaying: true, positionMs: 0, durationMs: 180000 },
      playMode: "sequential",
    });

    const { initAudio, play, currentSong, currentIndex } = useAudioPlayer({
      songs: () => [],
      sourceGroups: () => sourceGroups,
    });

    await initAudio();
    sourceGroups[0].playlists.push({
      name: "All",
      songs: [
        {
          id: 537,
          name: "Moved Server Song",
          path: "/music/moved-server-song.mp3",
          lufs: -12,
          device_id: deviceId,
          source_key: newApiBase,
        },
      ],
    });
    await play();

    expect(plugin.stop).not.toHaveBeenCalled();
    expect(plugin.setPlayingQueue).toHaveBeenCalledTimes(2);
    const queueCalls = plugin.setPlayingQueue.mock.calls as unknown as Array<
      Array<unknown>
    >;
    const payload = queueCalls[queueCalls.length - 1]?.[0] as {
      songs: Array<{
        id: number;
        deviceId?: string | null;
        sourceKind?: string | null;
        url?: string;
      }>;
      currentIndex: number | null;
    };
    expect(payload.currentIndex).toBe(0);
    expect(payload.songs).toEqual([
      expect.objectContaining({
        id: 537,
        deviceId,
        sourceKind: "kaulan",
      }),
    ]);
    expect(payload.songs[0]).not.toHaveProperty("url");
    expect(currentIndex.value).toBe(0);
    expect(currentSong.value?.id).toBe(537);
    expect(currentSong.value?.device_id).toBe(deviceId);
  });
});
