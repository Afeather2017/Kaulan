/**
 * Export / import helpers for local collections.
 *
 * Collections live in localStorage (`kaulan_local_collections`) and are wiped
 * when the app is re-installed. These helpers serialize collections to a
 * portable JSON shape keyed by `(device_id, song_name)` so they can be restored
 * after a re-install on the same instance, even though the local SQLite DB
 * (and therefore song `id` values) are rebuilt fresh.
 *
 * Matching on import uses the in-memory `allLibrarySongs` list — no per-server
 * queries. Unresolvable songs are skipped and counted; re-importing the same
 * file after the missing source comes online recovers them.
 *
 * Cross-instance imports (e.g. exporting from one Kaulan device and importing
 * on another) will not match because `device_id` values differ — that is by
 * design; the user must add the matching source first.
 *
 * Related documentation: `docs/collection-export-import.md`
 *
 * @module utils/collectionTransfer
 *
 * Keep this source file UTF-8 text so export-format changes remain reviewable.
 */

import type { MusicInfo } from "@/types/music";
import {
  toStoredCollectionSong,
  type StoredCollectionSong,
  type StoredLocalCollection,
} from "@/utils/storage";

/** Export format version. Bump on breaking shape changes. */
export const COLLECTION_EXPORT_VERSION = 2;

export interface CollectionExportSong {
  /** Stable UUID of the source device. Empty for local/unidentified. */
  device_id: string;
  /** Song name as stored on the source. */
  name: string;
}

export interface CollectionExportEntry {
  name: string;
  created_at: string;
  songs: CollectionExportSong[];
}

export interface CollectionExportPayload {
  /** Export format version. Runtime-validated against COLLECTION_EXPORT_VERSION. */
  version: number;
  exported_at: string;
  collections: CollectionExportEntry[];
}

export interface CollectionImportResult {
  /** Brand-new collections created by this import. */
  importedCollections: number;
  /** Existing same-named collections that received merged songs. */
  mergedCollections: number;
  /** Payload songs successfully resolved against the current library. */
  matchedSongs: number;
  /** Payload songs whose `(device_id, name)` wasn't present in the library. */
  skippedSongs: number;
}

/** Build the portable JSON payload from persisted collections. */
export function buildCollectionsExport(
  collections: StoredLocalCollection[],
  now: () => Date = () => new Date(),
): CollectionExportPayload {
  return {
    version: COLLECTION_EXPORT_VERSION,
    exported_at: now().toISOString(),
    collections: collections.map((collection) => ({
      name: collection.name,
      created_at: collection.created_at,
      songs: collection.songs.map((song) => ({
        device_id: song.device_id,
        name: song.name ?? song.filename,
      })),
    })),
  };
}

/**
 * Parse and validate a JSON string. Throws when the payload is malformed or
 * the version is unsupported — callers should surface the error to the user.
 */
export function parseCollectionsExport(raw: string): CollectionExportPayload {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    throw new Error("文件不是有效的 JSON");
  }

  if (!parsed || typeof parsed !== "object") {
    throw new Error("文件内容格式不正确");
  }

  const root = parsed as Record<string, unknown>;
  if (root.version !== COLLECTION_EXPORT_VERSION) {
    throw new Error(
      `不支持的导出版本：${String(root.version)}（当前仅支持版本 ${COLLECTION_EXPORT_VERSION}）`,
    );
  }

  if (!Array.isArray(root.collections)) {
    throw new Error("文件缺少 collections 列表");
  }

  const collections: CollectionExportEntry[] = [];
  for (const entry of root.collections) {
    if (!entry || typeof entry !== "object") {
      throw new Error("收藏夹条目格式不正确");
    }
    const obj = entry as Record<string, unknown>;
    if (typeof obj.name !== "string" || typeof obj.created_at !== "string") {
      throw new Error("收藏夹条目缺少名称或创建时间");
    }
    if (obj.name.trim().length === 0) {
      throw new Error("收藏夹条目名称不能为空");
    }
    if (!Array.isArray(obj.songs)) {
      throw new Error(`收藏夹“${obj.name}”缺少 songs 列表`);
    }

    const songs: CollectionExportSong[] = [];
    for (const song of obj.songs) {
      if (!song || typeof song !== "object") {
        throw new Error(`收藏夹“${obj.name}”包含无效的歌曲条目`);
      }
      const songObj = song as Record<string, unknown>;
      if (
        typeof songObj.device_id !== "string" ||
        typeof songObj.name !== "string"
      ) {
        throw new Error(`收藏夹“${obj.name}”包含无效的歌曲条目`);
      }
      if (songObj.name.trim().length === 0) {
        throw new Error(`收藏夹“${obj.name}”包含空名称的歌曲`);
      }
      // `device_id` is allowed to be empty — it's the normalized form of a
      // library song whose source device is the local/unidentified one.
      songs.push({ device_id: songObj.device_id, name: songObj.name });
    }

    collections.push({
      name: obj.name,
      created_at: obj.created_at,
      songs,
    });
  }

  return {
    version: COLLECTION_EXPORT_VERSION,
    exported_at: typeof root.exported_at === "string" ? root.exported_at : "",
    collections,
  };
}

const buildMatchKey = (deviceId: string, name: string): string =>
  `${deviceId} ${name}`;

/**
 * Build a lookup from `(device_id, name)` → current library song. The first
 * match wins on duplicate names within a source — consistent with how the
 * rest of the app treats filename uniqueness.
 */
const buildSongLookup = (allSongs: MusicInfo[]): Map<string, MusicInfo> => {
  const lookup = new Map<string, MusicInfo>();
  for (const song of allSongs) {
    const deviceId = song.device_id ?? "";
    const key = buildMatchKey(deviceId, song.name);
    if (!lookup.has(key)) {
      lookup.set(key, song);
    }
  }
  return lookup;
};

const buildExistingDedupeKey = (song: StoredCollectionSong): string =>
  `${song.device_id || "local"}:${song.song_id}:${song.name ?? song.filename}`;

const buildResolvedDedupeKey = (song: MusicInfo): string =>
  `${song.device_id || "local"}:${song.id}:${song.name}`;

/**
 * Merge an import payload into the current collections state.
 *
 * - Existing collection with the same name → songs are unioned in, deduped by
 *   the existing rowKey shape. Re-importing the same file is idempotent.
 * - Unknown collection name → a new collection is created with a fresh id.
 * - Payload songs that don't resolve in `allSongs` are skipped and counted.
 *
 * Returns the next collections array (caller persists it) and a summary.
 */
export function mergeCollectionsFromImport(
  payload: CollectionExportPayload,
  currentCollections: StoredLocalCollection[],
  allSongs: MusicInfo[],
  now: () => number = Date.now,
): { collections: StoredLocalCollection[]; result: CollectionImportResult } {
  const songLookup = buildSongLookup(allSongs);

  // Work on a shallow copy so we can mutate per-collection entries freely.
  const next: StoredLocalCollection[] = currentCollections.map(
    (collection) => ({
      ...collection,
      songs: [...collection.songs],
    }),
  );

  let importedCollections = 0;
  let mergedCollections = 0;
  let matchedSongs = 0;
  let skippedSongs = 0;

  for (const entry of payload.collections) {
    const existingIndex = next.findIndex(
      (collection) => collection.name === entry.name,
    );

    if (existingIndex === -1) {
      const resolvedSongs: StoredCollectionSong[] = [];
      for (const payloadSong of entry.songs) {
        const matched = songLookup.get(
          buildMatchKey(payloadSong.device_id, payloadSong.name),
        );
        if (!matched) {
          skippedSongs += 1;
          continue;
        }
        matchedSongs += 1;
        resolvedSongs.push(toStoredCollectionSong(matched));
      }

      next.push({
        id: now(),
        name: entry.name,
        created_at: entry.created_at,
        songs: resolvedSongs,
      });
      importedCollections += 1;
      continue;
    }

    const target = next[existingIndex];
    const existingKeys = new Set(
      target.songs.map((song) => buildExistingDedupeKey(song)),
    );
    let appended = false;

    for (const payloadSong of entry.songs) {
      const matched = songLookup.get(
        buildMatchKey(payloadSong.device_id, payloadSong.name),
      );
      if (!matched) {
        skippedSongs += 1;
        continue;
      }
      matchedSongs += 1;
      const dedupeKey = buildResolvedDedupeKey(matched);
      if (existingKeys.has(dedupeKey)) {
        continue;
      }
      existingKeys.add(dedupeKey);
      target.songs.push(toStoredCollectionSong(matched));
      appended = true;
    }

    if (appended) {
      mergedCollections += 1;
    }
  }

  return {
    collections: next,
    result: {
      importedCollections,
      mergedCollections,
      matchedSongs,
      skippedSongs,
    },
  };
}
