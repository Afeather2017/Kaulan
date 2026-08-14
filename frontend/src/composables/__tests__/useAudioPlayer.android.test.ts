//! Unit tests for Android playback correction behavior in useAudioPlayer.
//!
//! Related documentation:
//! - `docs/lyric-sync-timing.md`
//! - `docs/android/playback-session.md`

import { beforeEach, describe, expect, it, vi } from "vitest";
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

const buildLocalGroup = (
  overrides: Partial<LibrarySourceGroup> = {},
): LibrarySourceGroup => ({
  apiBase: "http://localhost:2080/api",
  sourceKey: "http://localhost:2080/api",
  device_id: "local-device",
  name: "Local",
  isLoading: false,
  isOnline: true,
  playlists: [],
  onlineProviderStatuses: [],
  capabilities: {
    canRefresh: true,
    canUpload: true,
    canChangeDirectory: true,
    canUseForOnlineSearch: false,
    isCurrentOnlineSearchSource: false,
    canRetryConnection: true,
    canShowSourceDetails: true,
    canDeleteSource: false,
  },
  ...overrides,
});

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
            localUri?: string | null;
            tempSongUrl?: string | null;
            sourceKind?: string | null;
          }>;
        }
      | undefined;
    expect(latestQueue?.songs?.[0]?.localUri).toBe(
      "content://media/external/audio/media/1",
    );
    expect(latestQueue?.songs?.[0]?.sourceKind).toBe("local_raw");
    expect(latestQueue?.songs?.[0]?.tempSongUrl).toBeNull();
  });

  it("keeps absolute filesystem paths as local_raw for localhost queues", async () => {
    // App-private files (e.g. library-import downloads under download_root)
    // are absolute filesystem paths the native MediaPlayer can open directly,
    // unlike the basename-only paths of restored songs.
    const songs = [
      {
        id: 2,
        name: "Downloaded Song",
        lufs: -11,
        path: "/data/user/0/com.kaulan.app/files/downloads/song.mp3",
        stream_url: "http://localhost:2080/api/music/id/2",
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

    const queueCalls = plugin.setPlayingQueue.mock.calls as Array<
      Array<unknown>
    >;
    const latestQueue = queueCalls[queueCalls.length - 1]?.[0] as {
      songs?: Array<{
        sourceKind?: string;
        deviceId?: string | null;
        localUri?: string | null;
      }>;
    };
    expect(latestQueue.songs?.[0]?.sourceKind).toBe("local_raw");
    expect(latestQueue.songs?.[0]?.localUri).toBe(songs[0].path);
    expect(latestQueue.songs?.[0]?.deviceId).toBeNull();
  });

  it("falls back to kaulan streaming for restored songs with basename-only paths", async () => {
    // Restored collection/queue songs keep only the basename in `path`
    // (songRestore.buildRestoredSong) and may store device_id "" for the
    // local device. The queue must stream via the localhost backend with the
    // source group's device id instead of handing the native player an
    // unopenable basename as a raw local URI.
    const restoredSongs = [
      {
        id: 3,
        name: "One day of Pokke village",
        lufs: -13.7,
        path: "1000013114",
        stream_url: "http://localhost:2080/api/music/id/3",
        source_key: "http://localhost:2080/api",
        device_id: "",
      },
    ];
    const sourceGroups = [buildLocalGroup()];

    plugin.getPlaybackSession.mockResolvedValue({
      queue: { songs: [], currentIndex: null },
      currentSongId: null,
      runtime: { isPlaying: false, positionMs: 0, durationMs: 0 },
      playMode: "sequential",
    });

    const { initAudio, playSongAtIndex } = useAudioPlayer({
      songs: () => restoredSongs,
      sourceGroups: () => sourceGroups,
    });

    await initAudio();
    await playSongAtIndex(restoredSongs[0], 0, restoredSongs);

    const queueCalls = plugin.setPlayingQueue.mock.calls as Array<
      Array<unknown>
    >;
    const latestQueue = queueCalls[queueCalls.length - 1]?.[0] as {
      songs?: Array<{
        sourceKind?: string;
        deviceId?: string | null;
        localUri?: string | null;
      }>;
    };
    expect(latestQueue.songs?.[0]?.sourceKind).toBe("kaulan");
    expect(latestQueue.songs?.[0]?.deviceId).toBe("local-device");
    expect(latestQueue.songs?.[0]?.localUri).toBeNull();
  });

  it("maps a still-loading source group to a null device id, not blank", async () => {
    // buildLoadingSourceGroup stamps device_id "" until /discovery/self
    // resolves. The native queue must receive null (skippable entry) instead
    // of an unresolvable "" deviceId.
    const restoredSongs = [
      {
        id: 3,
        name: "One day of Pokke village",
        lufs: -13.7,
        path: "1000013114",
        stream_url: "http://localhost:2080/api/music/id/3",
        source_key: "http://localhost:2080/api",
        device_id: "",
      },
    ];
    const sourceGroups = [
      buildLocalGroup({ device_id: "", isLoading: true, isOnline: false }),
    ];

    plugin.getPlaybackSession.mockResolvedValue({
      queue: { songs: [], currentIndex: null },
      currentSongId: null,
      runtime: { isPlaying: false, positionMs: 0, durationMs: 0 },
      playMode: "sequential",
    });

    const { initAudio, playSongAtIndex } = useAudioPlayer({
      songs: () => restoredSongs,
      sourceGroups: () => sourceGroups,
    });

    await initAudio();
    await playSongAtIndex(restoredSongs[0], 0, restoredSongs);

    const queueCalls = plugin.setPlayingQueue.mock.calls as Array<
      Array<unknown>
    >;
    const latestQueue = queueCalls[queueCalls.length - 1]?.[0] as {
      songs?: Array<{
        sourceKind?: string;
        deviceId?: string | null;
        localUri?: string | null;
      }>;
    };
    expect(latestQueue.songs?.[0]?.sourceKind).toBe("kaulan");
    expect(latestQueue.songs?.[0]?.deviceId).toBeNull();
    expect(latestQueue.songs?.[0]?.localUri).toBeNull();
  });

  it("should resolve HTTP paths by identity despite a localhost source key", async () => {
    const songs = [
      {
        id: 416,
        name: "Remote Song",
        lufs: -12,
        path: "http://192.168.20.23:2080/api/music/id/416",
        stream_url: "http://192.168.20.23:2080/api/music/id/416",
        source_key: "http://localhost:2080/api",
        device_id: "remote-device",
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

    const queueCalls = plugin.setPlayingQueue.mock.calls as Array<
      Array<unknown>
    >;
    const latestQueue = queueCalls[queueCalls.length - 1]?.[0] as {
      songs?: Array<{
        sourceKind?: string;
        deviceId?: string | null;
        localUri?: string | null;
        tempSongUrl?: string | null;
      }>;
    };
    expect(latestQueue.songs?.[0]?.sourceKind).toBe("kaulan");
    expect(latestQueue.songs?.[0]?.deviceId).toBe("remote-device");
    expect(latestQueue.songs?.[0]?.localUri).toBeNull();
    expect(latestQueue.songs?.[0]?.tempSongUrl).toBeNull();
  });

  it("uses a dedicated URL only for temporary Android tracks", async () => {
    const songs = [
      {
        id: -1,
        name: "Preview",
        lufs: null,
        path: "https://media.example/preview.mp3",
        stream_url: "https://media.example/preview.mp3",
        is_temporary: true,
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

    const queueCalls = plugin.setPlayingQueue.mock.calls as Array<
      Array<unknown>
    >;
    const latestQueue = queueCalls[queueCalls.length - 1]?.[0] as {
      songs?: Array<Record<string, unknown>>;
    };
    expect(latestQueue.songs?.[0]).toMatchObject({
      sourceKind: "temporary",
      deviceId: null,
      localUri: null,
      tempSongUrl: "https://media.example/preview.mp3",
    });
    expect(latestQueue.songs?.[0]).not.toHaveProperty("path");
    expect(latestQueue.songs?.[0]).not.toHaveProperty("url");
  });

  it("keeps explicit temporary tracks ephemeral even with a raw local path", async () => {
    const songs = [
      {
        id: -1,
        name: "Opened file",
        lufs: null,
        path: "/storage/emulated/0/Music/opened.mp3",
        is_temporary: true,
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

    const queueCalls = plugin.setPlayingQueue.mock.calls as Array<
      Array<unknown>
    >;
    const latestQueue = queueCalls[queueCalls.length - 1]?.[0] as {
      songs?: Array<Record<string, unknown>>;
    };
    expect(latestQueue.songs?.[0]).toMatchObject({
      sourceKind: "temporary",
      localUri: null,
      tempSongUrl: "/storage/emulated/0/Music/opened.mp3",
    });
  });
});
