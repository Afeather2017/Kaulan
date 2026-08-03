import { describe, expect, it } from "vitest";
import { filterSongsBySearchQuery } from "@/composables/useLibrarySources";
import type { MusicInfo } from "@/composables/useAudioPlayer";

const song = (name: string, id: number): MusicInfo => ({
  id,
  name,
  lufs: null,
  path: `/music/${name}`,
});

describe("filterSongsBySearchQuery", () => {
  it("filters only the supplied playlist when a search scope is provided", () => {
    const playlistSongs = [song("Only In Playlist.mp3", 1)];
    const allSongs = [...playlistSongs, song("Global Match.mp3", 2)];

    expect(filterSongsBySearchQuery(playlistSongs, "match")).toEqual([]);
    expect(filterSongsBySearchQuery(allSongs, "match")).toEqual([allSongs[1]]);
  });

  it("deduplicates matching songs by row key", () => {
    const duplicate = song("Same.mp3", 1);
    expect(filterSongsBySearchQuery([duplicate, duplicate], "same")).toEqual([
      duplicate,
    ]);
  });
});
