import { describe, expect, it } from "vitest";

import type { MusicInfo } from "@/types/music";
import type { LibrarySourceGroup } from "@/types/library";
import type {
  StoredCollectionSong,
  StoredPlaybackQueueSong,
} from "@/utils/storage";
import {
  createSongAdopter,
  storedCollectionSongToMusicInfo,
  storedQueueSongToMusicInfo,
} from "@/utils/songRestore";

// Related documentation:
// - `docs/android/playback-session.md` (local_raw vs kaulan classification)
// - `docs/collection-export-import.md`

const LOCAL = "http://localhost:2080/api";
const REMOTE = "http://192.168.1.10:2080/api";
const LOCAL_DEVICE = "local-device";
const REMOTE_DEVICE = "remote-device";

const buildLiveSong = (
  id: number,
  name: string,
  apiBase: string,
  deviceId: string,
  path: string,
): MusicInfo => ({
  id,
  name,
  lufs: -12,
  path,
  stream_url: `${apiBase}/music/id/${id}`,
  cover_url: `${apiBase}/music/id/${id}/cover`,
  source_key: apiBase,
  device_id: deviceId,
  sourceLabel: apiBase,
  rowKey: `${deviceId}:${id}:${name}`,
  mediaType: "audio",
});

const buildGroup = (
  apiBase: string,
  deviceId: string,
  songs: MusicInfo[],
): LibrarySourceGroup => ({
  apiBase,
  sourceKey: apiBase,
  device_id: deviceId,
  name: apiBase,
  isLoading: false,
  isOnline: true,
  playlists: [{ name: "All", songs }],
  onlineProviderStatuses: [],
  capabilities: {
    canRefresh: true,
    canUpload: true,
    canChangeDirectory: true,
    canUseForOnlineSearch: false,
    isCurrentOnlineSearchSource: false,
    canRetryConnection: false,
    canShowSourceDetails: true,
    canDeleteSource: false,
  },
});

// Local group serves raw locators to localhost callers (see
// `resolve_playback_path` in backend/src/types/mod.rs) — on Android that is
// the content:// URI from MediaStore.
const LOCAL_SONG = buildLiveSong(
  42,
  "song.mp3",
  LOCAL,
  LOCAL_DEVICE,
  "content://media/external/audio/media/42",
);
const REMOTE_SONG = buildLiveSong(7, "remote.flac", REMOTE, REMOTE_DEVICE, "");

const SOURCE_GROUPS = [
  buildGroup(LOCAL, LOCAL_DEVICE, [LOCAL_SONG]),
  buildGroup(REMOTE, REMOTE_DEVICE, [REMOTE_SONG]),
];

describe("storedCollectionSongToMusicInfo", () => {
  it("adopts the live library entry with its raw content:// path for a loaded local device", () => {
    const stored: StoredCollectionSong = {
      device_id: LOCAL_DEVICE,
      song_id: 42,
      filename: "song.mp3",
      name: "song.mp3",
      lufs: -10,
    };

    const restored = storedCollectionSongToMusicInfo(stored, SOURCE_GROUPS);

    expect(restored.path).toBe("content://media/external/audio/media/42");
    expect(restored.stream_url).toBe(`${LOCAL}/music/id/42`);
    expect(restored.cover_url).toBe(`${LOCAL}/music/id/42/cover`);
    expect(restored.source_key).toBe(LOCAL);
    expect(restored.device_id).toBe(LOCAL_DEVICE);
    expect(restored.is_temporary).toBeFalsy();
  });

  it("adopts the live entry for a remote device (empty path, HTTP stream_url)", () => {
    const stored: StoredCollectionSong = {
      device_id: REMOTE_DEVICE,
      song_id: 7,
      filename: "remote.flac",
      name: "remote.flac",
    };

    const restored = storedCollectionSongToMusicInfo(stored, SOURCE_GROUPS);

    expect(restored.path).toBe("");
    expect(restored.stream_url).toBe(`${REMOTE}/music/id/7`);
    expect(restored.source_key).toBe(REMOTE);
  });

  it("falls back to basename + HTTP stream when the song is no longer in the library", () => {
    const stored: StoredCollectionSong = {
      device_id: LOCAL_DEVICE,
      song_id: 999,
      filename: "deleted.mp3",
      name: "deleted.mp3",
    };

    const restored = storedCollectionSongToMusicInfo(stored, SOURCE_GROUPS);

    expect(restored.path).toBe("deleted.mp3");
    expect(restored.stream_url).toBe(`${LOCAL}/music/id/999`);
    expect(restored.source_key).toBe(LOCAL);
  });

  it("returns a skeleton when the device is not in the source list", () => {
    const stored: StoredCollectionSong = {
      device_id: "unknown-device",
      song_id: 42,
      filename: "song.mp3",
    };

    const restored = storedCollectionSongToMusicInfo(stored, SOURCE_GROUPS);

    expect(restored.stream_url).toBeNull();
    expect(restored.cover_url).toBeNull();
    expect(restored.source_key).toBeNull();
    expect(restored.is_temporary).toBe(true);
  });

  it("empty device_id resolves to the local group", () => {
    const stored: StoredCollectionSong = {
      device_id: "",
      song_id: 42,
      filename: "song.mp3",
    };

    const restored = storedCollectionSongToMusicInfo(stored, SOURCE_GROUPS);

    expect(restored.path).toBe("content://media/external/audio/media/42");
    expect(restored.source_key).toBe(LOCAL);
  });
});

describe("storedQueueSongToMusicInfo", () => {
  it("adopts the live library entry with its raw content:// path for a loaded local device", () => {
    const stored: StoredPlaybackQueueSong = {
      device_id: LOCAL_DEVICE,
      song_id: 42,
      filename: "song.mp3",
      name: "song.mp3",
    };

    const restored = storedQueueSongToMusicInfo(stored, SOURCE_GROUPS);

    expect(restored.path).toBe("content://media/external/audio/media/42");
    expect(restored.stream_url).toBe(`${LOCAL}/music/id/42`);
    expect(restored.is_temporary).toBeFalsy();
  });

  it("falls back to basename + HTTP stream when the song is missing from a loaded group", () => {
    const stored: StoredPlaybackQueueSong = {
      device_id: REMOTE_DEVICE,
      song_id: 999,
      filename: "gone.flac",
    };

    const restored = storedQueueSongToMusicInfo(stored, SOURCE_GROUPS);

    expect(restored.path).toBe("gone.flac");
    expect(restored.stream_url).toBe(`${REMOTE}/music/id/999`);
  });

  it("returns a skeleton when the device is not in the source list", () => {
    const stored: StoredPlaybackQueueSong = {
      device_id: "unknown-device",
      song_id: 42,
      filename: "song.mp3",
    };

    const restored = storedQueueSongToMusicInfo(stored, SOURCE_GROUPS);

    expect(restored.stream_url).toBeNull();
    expect(restored.is_temporary).toBe(true);
  });
});

describe("createSongAdopter", () => {
  it("re-adopts a pre-load fallback song into its live content:// entry", () => {
    const adopt = createSongAdopter(SOURCE_GROUPS);
    // What a collection tap looks like before the library settles: basename
    // path, rebuilt HTTP stream_url (buildRestoredSong output shape).
    const preLoadTap: MusicInfo = {
      id: 42,
      name: "song.mp3",
      lufs: -10,
      path: "song.mp3",
      stream_url: `${LOCAL}/music/id/42`,
      cover_url: `${LOCAL}/music/id/42/cover`,
      source_key: LOCAL,
      device_id: LOCAL_DEVICE,
      is_temporary: false,
    };

    const adopted = adopt(preLoadTap);

    expect(adopted.path).toBe("content://media/external/audio/media/42");
    expect(adopted.stream_url).toBe(`${LOCAL}/music/id/42`);
  });

  it("re-adopts by song_id when device_id is blank (local device)", () => {
    const adopt = createSongAdopter(SOURCE_GROUPS);
    const preLoadTap: MusicInfo = {
      id: 42,
      name: "song.mp3",
      lufs: null,
      path: "song.mp3",
      stream_url: `${LOCAL}/music/id/42`,
      cover_url: `${LOCAL}/music/id/42/cover`,
      source_key: LOCAL,
      device_id: "",
      is_temporary: false,
    };

    const adopted = adopt(preLoadTap);

    expect(adopted.path).toBe("content://media/external/audio/media/42");
  });

  it("passes through songs it cannot adopt (missing, unknown device, online)", () => {
    const adopt = createSongAdopter(SOURCE_GROUPS);
    const missingFromLibrary: MusicInfo = {
      id: 999,
      name: "deleted.mp3",
      lufs: null,
      path: "deleted.mp3",
      stream_url: `${LOCAL}/music/id/999`,
      source_key: LOCAL,
      device_id: LOCAL_DEVICE,
      is_temporary: false,
    };
    const unknownDevice: MusicInfo = {
      id: 1,
      name: "x.mp3",
      lufs: null,
      path: "x.mp3",
      device_id: "stranger",
      is_temporary: false,
    };
    const online: MusicInfo = {
      id: 5,
      name: "yt track",
      lufs: null,
      path: "yt",
      device_id: "",
      source: "youtube",
      is_temporary: true,
    };

    expect(adopt(missingFromLibrary)).toBe(missingFromLibrary);
    expect(adopt(unknownDevice)).toBe(unknownDevice);
    expect(adopt(online)).toBe(online);
  });
});
