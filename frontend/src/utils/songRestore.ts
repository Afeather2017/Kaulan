/**
 * Helpers that turn persisted song entries back into playable MusicInfo by
 * resolving the device's current apiBase from the live source list.
 *
 * Lookups run through an index memoized per source-groups snapshot (WeakMap
 * keyed on array identity — see `indexCache` below), so restoring a whole
 * collection or queue costs one O(library) index build per snapshot plus O(1)
 * per stored song.
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
 * Index over one source-groups snapshot so restore lookups stay O(1) per
 * song. Songs are keyed by the composite `apiBase + song_id`: ids are
 * per-device SQLite row ids and collide across devices, so a bare-id map
 * would mix libraries.
 */
type LibraryIndex = {
  apiBaseByDeviceId: Map<string, string>;
  songByCompositeKey: Map<string, MusicInfo>;
};

const songKey = (apiBase: string, songId: number): string =>
  `${apiBase}\n${songId}`;

const buildLibraryIndex = (
  sourceGroups: LibrarySourceGroup[],
): LibraryIndex => {
  const apiBaseByDeviceId = new Map<string, string>();
  const songByCompositeKey = new Map<string, MusicInfo>();
  for (const group of sourceGroups) {
    if (group.device_id && !apiBaseByDeviceId.has(group.device_id)) {
      apiBaseByDeviceId.set(group.device_id, group.apiBase);
    }
    for (const playlist of group.playlists) {
      for (const song of playlist.songs) {
        const key = songKey(group.apiBase, song.id);
        if (!songByCompositeKey.has(key)) {
          songByCompositeKey.set(key, song);
        }
      }
    }
  }
  return { apiBaseByDeviceId, songByCompositeKey };
};

// Safe only because source-groups arrays are replaced wholesale on every
// update (see `upsertSortedItem` / `loadItemsIncrementally`) and never
// mutated in place — array identity is therefore a valid cache key. If that
// discipline ever breaks, this cache serves stale entries and restores
// silently fall back to basename/HTTP playback.
const indexCache = new WeakMap<LibrarySourceGroup[], LibraryIndex>();

const getLibraryIndex = (sourceGroups: LibrarySourceGroup[]): LibraryIndex => {
  let index = indexCache.get(sourceGroups);
  if (!index) {
    index = buildLibraryIndex(sourceGroups);
    indexCache.set(sourceGroups, index);
  }
  return index;
};

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
  return getLibraryIndex(sourceGroups).apiBaseByDeviceId.get(deviceId) ?? null;
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
 * Find the live library entry for a stored song via the shared snapshot
 * index: composite `apiBase + song_id` lookup, first match wins (same
 * convention as the collection import matcher in `collectionTransfer.ts`).
 * Returns null when the group is not loaded or the song is no longer in the
 * library.
 */
const findLiveLibrarySong = (
  apiBase: string,
  songId: number,
  sourceGroups: LibrarySourceGroup[],
): MusicInfo | null => {
  const live = getLibraryIndex(sourceGroups).songByCompositeKey.get(
    songKey(apiBase, songId),
  );
  return live ? { ...live } : null;
};

/**
 * Build an O(1) adopter over the live library for a batch of songs whose
 * references may predate the initial library load (e.g. collection entries
 * captured before the first `/playlists` fetch settled, still carrying their
 * basename/HTTP fallback shape). Backed by the shared per-snapshot index, so
 * adopting a whole queue stays linear. The returned function keys each song
 * by `(device_id, song_id)` — blank `device_id` resolves to the local group,
 * same rule as the restore helpers. Online (`source`) and temporary songs
 * pass through unchanged; so do songs whose device group is not loaded or no
 * longer lists them.
 */
export const createSongAdopter = (
  sourceGroups: LibrarySourceGroup[],
): ((song: MusicInfo) => MusicInfo) => {
  const index = getLibraryIndex(sourceGroups);

  return (song: MusicInfo): MusicInfo => {
    if (song.source || (song.is_temporary && song.id <= 0)) {
      return song;
    }
    const apiBase = song.device_id
      ? (index.apiBaseByDeviceId.get(song.device_id) ?? null)
      : getLocalApiBase();
    if (!apiBase) {
      return song;
    }
    const live = index.songByCompositeKey.get(songKey(apiBase, song.id));
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
