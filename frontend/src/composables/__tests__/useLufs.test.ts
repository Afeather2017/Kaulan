import { describe, it, expect, beforeEach, vi } from "vitest";
import { ref } from "vue";
import { useLufs } from "../useLufs";
import type { MusicInfo } from "@/types/music";
import type { LibrarySourceGroup } from "@/types/library";

vi.mock("@/utils/api", () => ({
  resolveSourceApiBase: (sourceKey?: string | null) =>
    sourceKey || "http://localhost:2080/api",
}));

const createSong = (
  id: number,
  sourceKey: string | null,
  lufs: number | null = null,
  deviceId: string | null = sourceKey,
): MusicInfo => ({
  id,
  name: `Song ${id}`,
  path: `/music/${id}.mp3`,
  lufs,
  source_key: sourceKey,
  device_id: deviceId,
});

const createUseLufs = () => {
  return useLufs({
    currentSong: ref<MusicInfo | null>(null),
    activeQueue: ref<MusicInfo[]>([]),
    searchPlaybackSongs: ref<MusicInfo[]>([]),
    selectedPlaylist: ref<{ name: string; songs: MusicInfo[] } | null>(null),
    sourceGroups: ref<LibrarySourceGroup[]>([]),
    isAndroidPlayer: ref(false),
    syncAndroidQueueState: vi.fn(() => Promise.resolve()),
    syncSelectedLibraryPlaylist: vi.fn(),
    currentView: ref("library"),
  });
};

describe("useLufs queue pre-cache", () => {
  beforeEach(() => {
    global.fetch = vi.fn(async () => ({
      ok: true,
      json: async () => ({
        success: true,
        lufs: -11.8,
        cached: true,
      }),
    })) as unknown as typeof fetch;
  });

  it("requests songs from the current sequential position in order", async () => {
    const { requestQueueLufs } = createUseLufs();
    const queue = [
      createSong(1, "http://localhost:2080/api"),
      createSong(2, "http://localhost:2080/api"),
      createSong(3, "http://localhost:2080/api"),
    ];

    await requestQueueLufs(queue, 1, 2, "sequential");

    expect(vi.mocked(global.fetch).mock.calls).toHaveLength(2);
    expect(vi.mocked(global.fetch).mock.calls[0]?.[0]).toBe(
      "http://localhost:2080/api/music/2/precache-lufs",
    );
    expect(vi.mocked(global.fetch).mock.calls[1]?.[0]).toBe(
      "http://localhost:2080/api/music/3/precache-lufs",
    );
  });

  it("limits loop mode queue pre-cache to the current song", async () => {
    const { requestQueueLufs } = createUseLufs();
    const queue = [
      createSong(1, "http://localhost:2080/api"),
      createSong(2, "http://localhost:2080/api"),
      createSong(3, "http://localhost:2080/api"),
    ];

    await requestQueueLufs(queue, 1, 3, "loop");

    expect(vi.mocked(global.fetch).mock.calls).toHaveLength(1);
    expect(vi.mocked(global.fetch).mock.calls[0]?.[0]).toBe(
      "http://localhost:2080/api/music/2/precache-lufs",
    );
  });

  it("de-dupes repeated queue entries from the same source", async () => {
    const { requestQueueLufs } = createUseLufs();
    const sharedSource = "http://localhost:2080/api";
    const queue = [
      createSong(1, sharedSource),
      createSong(1, sharedSource),
      createSong(2, sharedSource),
    ];

    await requestQueueLufs(queue, 0, 3, "sequential");

    expect(vi.mocked(global.fetch).mock.calls).toHaveLength(2);
    expect(vi.mocked(global.fetch).mock.calls[0]?.[0]).toBe(
      "http://localhost:2080/api/music/1/precache-lufs",
    );
    expect(vi.mocked(global.fetch).mock.calls[1]?.[0]).toBe(
      "http://localhost:2080/api/music/2/precache-lufs",
    );
  });

  it("treats identical song ids from different sources as distinct requests", async () => {
    const { requestQueueLufs } = createUseLufs();
    const queue = [
      createSong(1, "http://localhost:2080/api"),
      createSong(1, "http://remote.example/api"),
    ];

    await requestQueueLufs(queue, 0, 2, "sequential");

    expect(vi.mocked(global.fetch).mock.calls).toHaveLength(2);
    expect(vi.mocked(global.fetch).mock.calls[0]?.[0]).toBe(
      "http://localhost:2080/api/music/1/precache-lufs",
    );
    expect(vi.mocked(global.fetch).mock.calls[1]?.[0]).toBe(
      "http://remote.example/api/music/1/precache-lufs",
    );
  });

  it("de-dupes an identical device song after its API URL changes", async () => {
    const { requestQueueLufs } = createUseLufs();
    const queue = [
      createSong(1, "http://192.168.1.10/api", null, "same-device"),
      createSong(1, "http://192.168.1.11/api", null, "same-device"),
    ];

    await requestQueueLufs(queue, 0, 2, "sequential");

    expect(vi.mocked(global.fetch).mock.calls).toHaveLength(1);
  });
});
