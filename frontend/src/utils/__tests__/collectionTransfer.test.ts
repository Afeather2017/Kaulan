import { describe, expect, it } from "vitest";

import type { MusicInfo } from "@/types/music";
import type { StoredLocalCollection } from "@/utils/storage";
import {
  buildCollectionsExport,
  COLLECTION_EXPORT_VERSION,
  mergeCollectionsFromImport,
  parseCollectionsExport,
} from "@/utils/collectionTransfer";

// Related documentation: `docs/collection-export-import.md`

const buildLibrarySong = (
  id: number,
  name: string,
  source: string,
): MusicInfo => ({
  id,
  name,
  lufs: -10,
  path: `/music/${name}`,
  stream_url: `${source}/music/id/${id}`,
  cover_url: `${source}/music/id/${id}/cover`,
  source_key: source,
  sourceLabel: source,
  rowKey: `${source}:${id}:${name}`,
  mediaType: "audio",
});

const LOCAL = "http://localhost:2080/api";
const REMOTE = "http://192.168.1.10:2080/api";

describe("buildCollectionsExport", () => {
  it("strips volatile fields and keeps (source, name) pairs", () => {
    const collections: StoredLocalCollection[] = [
      {
        id: 1,
        name: "Favorites",
        created_at: "2026-01-01T00:00:00.000Z",
        songs: [
          {
            id: 42,
            name: "track.mp3",
            lufs: -10,
            path: "/music/track.mp3",
            stream_url: `${LOCAL}/music/id/42`,
            cover_url: `${LOCAL}/music/id/42/cover`,
            source_key: LOCAL,
            sourceLabel: LOCAL,
            rowKey: `${LOCAL}:42:track.mp3`,
            mediaType: "audio",
          },
        ],
      },
    ];

    const payload = buildCollectionsExport(
      collections,
      () => new Date("2026-07-16T00:00:00.000Z"),
    );

    expect(payload).toEqual({
      version: COLLECTION_EXPORT_VERSION,
      exported_at: "2026-07-16T00:00:00.000Z",
      collections: [
        {
          name: "Favorites",
          created_at: "2026-01-01T00:00:00.000Z",
          songs: [{ source: LOCAL, name: "track.mp3" }],
        },
      ],
    });
  });

  it("normalizes a missing source_key to an empty string", () => {
    const collections: StoredLocalCollection[] = [
      {
        id: 1,
        name: "F",
        created_at: "2026-01-01T00:00:00.000Z",
        songs: [
          {
            id: 1,
            name: "n.mp3",
            lufs: null,
            path: "p",
            source_key: null,
          },
        ],
      },
    ];

    const payload = buildCollectionsExport(collections);

    expect(payload.collections[0].songs[0]).toEqual({
      source: "",
      name: "n.mp3",
    });
  });
});

describe("parseCollectionsExport", () => {
  it("rejects non-JSON input", () => {
    expect(() => parseCollectionsExport("not json")).toThrowError(/JSON/);
  });

  it("rejects unsupported versions", () => {
    expect(() =>
      parseCollectionsExport(JSON.stringify({ version: 999, collections: [] })),
    ).toThrowError(/版本/);
  });

  it("rejects a missing collections array", () => {
    expect(() =>
      parseCollectionsExport(
        JSON.stringify({ version: COLLECTION_EXPORT_VERSION }),
      ),
    ).toThrowError(/collections/);
  });

  it("rejects a song entry missing source or name", () => {
    const bad = {
      version: COLLECTION_EXPORT_VERSION,
      collections: [
        {
          name: "F",
          created_at: "x",
          songs: [{ source: LOCAL }],
        },
      ],
    };
    expect(() => parseCollectionsExport(JSON.stringify(bad))).toThrowError();
  });
});

describe("mergeCollectionsFromImport", () => {
  it("round-trips songs when the library matches the export", () => {
    const allSongs: MusicInfo[] = [
      buildLibrarySong(1, "a.mp3", LOCAL),
      buildLibrarySong(2, "b.flac", REMOTE),
    ];
    const payload = {
      version: COLLECTION_EXPORT_VERSION,
      exported_at: "2026-07-16T00:00:00.000Z",
      collections: [
        {
          name: "Mix",
          created_at: "2026-01-01T00:00:00.000Z",
          songs: [
            { source: LOCAL, name: "a.mp3" },
            { source: REMOTE, name: "b.flac" },
          ],
        },
      ],
    };

    const { collections, result } = mergeCollectionsFromImport(
      payload,
      [],
      allSongs,
    );

    expect(result).toEqual({
      importedCollections: 1,
      mergedCollections: 0,
      matchedSongs: 2,
      skippedSongs: 0,
    });
    expect(collections).toHaveLength(1);
    const songs = collections[0].songs;
    expect(songs.map((s) => s.name).sort()).toEqual(["a.mp3", "b.flac"]);
    // Resolved songs carry current library metadata (id, lufs, rowKey, etc).
    expect(songs[0]).toMatchObject({
      id: expect.any(Number),
      lufs: -10,
      rowKey: expect.any(String),
      source_key: expect.any(String),
    });
  });

  it("counts unmatched songs as skipped and omits them from the result", () => {
    const allSongs: MusicInfo[] = [buildLibrarySong(1, "a.mp3", LOCAL)];
    const payload = {
      version: COLLECTION_EXPORT_VERSION,
      exported_at: "",
      collections: [
        {
          name: "Mix",
          created_at: "x",
          songs: [
            { source: LOCAL, name: "a.mp3" },
            { source: REMOTE, name: "missing.flac" },
          ],
        },
      ],
    };

    const { collections, result } = mergeCollectionsFromImport(
      payload,
      [],
      allSongs,
    );

    expect(result.matchedSongs).toBe(1);
    expect(result.skippedSongs).toBe(1);
    expect(collections[0].songs.map((s) => s.name)).toEqual(["a.mp3"]);
  });

  it("merges songs into a same-named collection without duplicates", () => {
    const songA = buildLibrarySong(1, "a.mp3", LOCAL);
    const songB = buildLibrarySong(2, "b.mp3", LOCAL);
    const existing: StoredLocalCollection[] = [
      {
        id: 100,
        name: "Mix",
        created_at: "x",
        songs: [
          {
            id: songA.id,
            name: songA.name,
            lufs: songA.lufs,
            path: songA.path,
            source_key: songA.source_key,
            rowKey: songA.rowKey,
            mediaType: "audio",
          },
        ],
      },
    ];
    const payload = {
      version: COLLECTION_EXPORT_VERSION,
      exported_at: "",
      collections: [
        {
          name: "Mix",
          created_at: "x",
          songs: [
            { source: LOCAL, name: "a.mp3" },
            { source: LOCAL, name: "b.mp3" },
          ],
        },
      ],
    };

    const { collections, result } = mergeCollectionsFromImport(
      payload,
      existing,
      [songA, songB],
    );

    // songA is already present (deduped), songB appended, merged count is 1.
    expect(result).toEqual({
      importedCollections: 0,
      mergedCollections: 1,
      matchedSongs: 2,
      skippedSongs: 0,
    });
    expect(collections).toHaveLength(1);
    expect(collections[0].id).toBe(100);
    expect(collections[0].songs.map((s) => s.name).sort()).toEqual([
      "a.mp3",
      "b.mp3",
    ]);
  });

  it("is idempotent when the same file is imported twice", () => {
    const songA = buildLibrarySong(1, "a.mp3", LOCAL);
    const payload = {
      version: COLLECTION_EXPORT_VERSION,
      exported_at: "",
      collections: [
        {
          name: "Mix",
          created_at: "x",
          songs: [{ source: LOCAL, name: "a.mp3" }],
        },
      ],
    };

    const first = mergeCollectionsFromImport(payload, [], [songA]);
    const second = mergeCollectionsFromImport(payload, first.collections, [
      songA,
    ]);

    expect(second.result.matchedSongs).toBe(1);
    expect(second.result.mergedCollections).toBe(0);
    expect(second.collections[0].songs).toHaveLength(1);
  });

  it("recovers previously-skipped songs on re-import once the source is available", () => {
    const payload = {
      version: COLLECTION_EXPORT_VERSION,
      exported_at: "",
      collections: [
        {
          name: "Mix",
          created_at: "x",
          songs: [
            { source: LOCAL, name: "a.mp3" },
            { source: REMOTE, name: "b.flac" },
          ],
        },
      ],
    };

    // First import: remote is unavailable, b.flac skipped.
    const first = mergeCollectionsFromImport(
      payload,
      [],
      [buildLibrarySong(1, "a.mp3", LOCAL)],
    );
    expect(first.result.skippedSongs).toBe(1);
    expect(first.collections[0].songs.map((s) => s.name)).toEqual(["a.mp3"]);

    // User re-adds the remote server, re-imports the same file.
    const second = mergeCollectionsFromImport(payload, first.collections, [
      buildLibrarySong(1, "a.mp3", LOCAL),
      buildLibrarySong(2, "b.flac", REMOTE),
    ]);
    expect(second.result).toEqual({
      importedCollections: 0,
      mergedCollections: 1,
      matchedSongs: 2,
      skippedSongs: 0,
    });
    expect(second.collections[0].songs.map((s) => s.name).sort()).toEqual([
      "a.mp3",
      "b.flac",
    ]);
  });

  it("resolves duplicate filenames in the same source to the first match", () => {
    const first = buildLibrarySong(1, "dup.mp3", LOCAL);
    const second = buildLibrarySong(2, "dup.mp3", LOCAL);
    const payload = {
      version: COLLECTION_EXPORT_VERSION,
      exported_at: "",
      collections: [
        {
          name: "Mix",
          created_at: "x",
          songs: [{ source: LOCAL, name: "dup.mp3" }],
        },
      ],
    };

    const { collections, result } = mergeCollectionsFromImport(
      payload,
      [],
      [first, second],
    );

    expect(result.matchedSongs).toBe(1);
    expect(collections[0].songs).toHaveLength(1);
    expect(collections[0].songs[0].id).toBe(1);
  });

  it("creates separate collections for distinct names", () => {
    const songA = buildLibrarySong(1, "a.mp3", LOCAL);
    const payload = {
      version: COLLECTION_EXPORT_VERSION,
      exported_at: "",
      collections: [
        {
          name: "One",
          created_at: "x",
          songs: [{ source: LOCAL, name: "a.mp3" }],
        },
        {
          name: "Two",
          created_at: "x",
          songs: [{ source: LOCAL, name: "a.mp3" }],
        },
      ],
    };

    const { collections, result } = mergeCollectionsFromImport(
      payload,
      [],
      [songA],
    );

    expect(result.importedCollections).toBe(2);
    expect(collections.map((c) => c.name)).toEqual(["One", "Two"]);
  });
});
