/**
 * Pure helpers that turn persisted song entries back into playable MusicInfo by
 * resolving the device's current apiBase from the live source list.
 *
 * Songs are persisted as `{ device_id, song_id, filename }` (see
 * `utils/storage.ts`). Stream and cover URLs are never stored — they are
 * rebuilt here so a device IP change does not rot stored collections or queue
 * entries.
 *
 * When the owning source group is already loaded, the stored song is matched
 * against the live library (same device group, same `song_id`) and the live
 * MusicInfo is adopted wholesale: for localhost sources it carries the raw
 * `content://` / absolute path, so Android playback classifies it as
 * `local_raw` and the native MediaPlayer opens the URI directly instead of
 * streaming over loopback HTTP. The basename + HTTP rebuild below is the
 * fallback for groups that are still loading or songs that no longer exist.
 *
 * When `device_id` is not yet in the source list (cold start before sources
 * fetch, or the device is offline), the helper returns a *skeleton* MusicInfo
 * with `stream_url: null`. The caller keeps skeleton entries in place; they
 * re-materialize reactively once the library answers.
 *
 * @module utils/songRestore
 */

import { getLocalApiBase } from "@/utils/api";
import type { MusicInfo } from "@/types/music";
import type { LibrarySourceGroup } from "@/types/library";
import type {
  StoredCollectionSong,
  StoredPlaybackQueueSong,
} from "@/utils/storage";

/**
 * Look up the current apiBase for a device by its stable id.
 *
 * Empty `device_id` resolves to the local source (the device running this
 * browser). Returns null when the device is not currently in the source list.
 */
export const resolveDeviceApiBase = (
  deviceId: string,
  sourceGroups: LibrarySourceGroup[],
): string | null => {
  if (!deviceId) {
    return getLocalApiBase();
  }
  const match = sourceGroups.find((group) => group.device_id === deviceId);
  return match?.apiBase ?? null;
};

/**
 * Re-stamp `source_key`, `stream_url`, `cover_url` on a MusicInfo after the
 * source list changed (e.g. a device came back online with a new IP, or a
 * skeleton song's device finally appeared). Returns the original song
 * unchanged when the device is still unknown.
 */
export const rehydrateSongUrls = (
  song: MusicInfo,
  sourceGroups: LibrarySourceGroup[],
): MusicInfo => {
  const apiBase = resolveDeviceApiBase(song.device_id ?? "", sourceGroups);
  if (!apiBase) {
    return song;
  }
  return {
    ...song,
    source_key: apiBase,
    stream_url: `${apiBase}/music/id/${song.id}`,
    cover_url: `${apiBase}/music/id/${song.id}/cover`,
  };
};

/**
 * Find the live library entry for a stored song. Lookup is scoped to the
 * device's own source group: resolve the group by apiBase, then flatten its
 * playlists and match by `song_id` (first match wins — same convention as the
 * collection import matcher in `collectionTransfer.ts`). Returns null when the
 * group is not loaded or the song is no longer in the library.
 */
const findLiveLibrarySong = (
  apiBase: string,
  songId: number,
  sourceGroups: LibrarySourceGroup[],
): MusicInfo | null => {
  const group = sourceGroups.find((item) => item.apiBase === apiBase);
  if (!group) {
    return null;
  }
  for (const playlist of group.playlists) {
    const match = playlist.songs.find((song) => song.id === songId);
    if (match) {
      return { ...match };
    }
  }
  return null;
};

/**
 * Build an O(1) adopter over the live library for a batch of songs whose
 * references may predate the initial library load (e.g. collection entries
 * captured before the first `/playlists` fetch settled, still carrying their
 * basename/HTTP fallback shape). Indexes live songs by `apiBase + song_id`
 * once, so adopting a whole queue stays linear. The returned function keys
 * each song by `(device_id, song_id)` — blank `device_id` resolves to the
 * local group, same rule as the restore helpers. Online (`source`) and
 * temporary songs pass through unchanged; so do songs whose device group is
 * not loaded or no longer lists them.
 */
export const createSongAdopter = (
  sourceGroups: LibrarySourceGroup[],
): ((song: MusicInfo) => MusicInfo) => {
  const index = new Map<string, MusicInfo>();
  for (const group of sourceGroups) {
    for (const playlist of group.playlists) {
      for (const song of playlist.songs) {
        const key = `${group.apiBase}\n${song.id}`;
        if (!index.has(key)) {
          index.set(key, song);
        }
      }
    }
  }

  return (song: MusicInfo): MusicInfo => {
    if (song.source || (song.is_temporary && song.id <= 0)) {
      return song;
    }
    const apiBase = resolveDeviceApiBase(song.device_id ?? "", sourceGroups);
    if (!apiBase) {
      return song;
    }
    const live = index.get(`${apiBase}\n${song.id}`);
    return live ? { ...live } : song;
  };
};

const buildRestoredSong = (
  stored: StoredPlaybackQueueSong | StoredCollectionSong,
  apiBase: string,
): MusicInfo => ({
  id: stored.song_id,
  name: stored.name ?? stored.filename,
  path: stored.filename,
  lufs: stored.lufs ?? null,
  device_id: stored.device_id,
  source_key: apiBase,
  stream_url: `${apiBase}/music/id/${stored.song_id}`,
  cover_url: `${apiBase}/music/id/${stored.song_id}/cover`,
  mediaType: stored.mediaType,
  source: stored.source,
  is_temporary: false,
});

/**
 * Build a placeholder MusicInfo for a stored song whose device apiBase is not
 * yet known. The skeleton keeps `device_id` (and `source` for online songs) so
 * the runtime can match it later, and marks itself temporary so playback can
 * surface a "source not yet available" state.
 */
const buildSkeletonSong = (
  stored: StoredPlaybackQueueSong | StoredCollectionSong,
): MusicInfo => ({
  id: stored.song_id,
  name: stored.name ?? stored.filename,
  path: stored.filename,
  lufs: stored.lufs ?? null,
  device_id: stored.device_id,
  source_key: null,
  stream_url: null,
  cover_url: null,
  mediaType: stored.mediaType,
  source: stored.source,
  is_temporary: true,
});

/**
 * Restore a queued song. Always returns a MusicInfo — either fully resolved
 * (when the device is known) or a skeleton (when it is not). Online songs
 * (those with `source` set) always come back as skeletons keyed off `source`.
 */
export const storedQueueSongToMusicInfo = (
  stored: StoredPlaybackQueueSong,
  sourceGroups: LibrarySourceGroup[],
): MusicInfo => {
  if (stored.source) {
    return buildSkeletonSong(stored);
  }
  const apiBase = resolveDeviceApiBase(stored.device_id, sourceGroups);
  if (!apiBase) {
    return buildSkeletonSong(stored);
  }
  const liveSong = findLiveLibrarySong(apiBase, stored.song_id, sourceGroups);
  if (liveSong) {
    return liveSong;
  }
  return buildRestoredSong(stored, apiBase);
};

/**
 * Restore a collection song. Same restore rules as the queue helper.
 */
export const storedCollectionSongToMusicInfo = (
  stored: StoredCollectionSong,
  sourceGroups: LibrarySourceGroup[],
): MusicInfo => {
  if (stored.source) {
    return buildSkeletonSong(stored);
  }
  const apiBase = resolveDeviceApiBase(stored.device_id, sourceGroups);
  if (!apiBase) {
    return buildSkeletonSong(stored);
  }
  const liveSong = findLiveLibrarySong(apiBase, stored.song_id, sourceGroups);
  if (liveSong) {
    return liveSong;
  }
  return buildRestoredSong(stored, apiBase);
};
