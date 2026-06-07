interface FakeSong {
  mediaType?: "audio" | "video";
}

interface FakePlaylist {
  name: string;
  songs: FakeSong[];
}

interface FakeSourceGroup {
  sourceKey: string;
  isOnline: boolean;
  playlists: FakePlaylist[];
}

function filterVisibleGroups(
  groups: FakeSourceGroup[],
  sourceKey: string,
  mediaTypes: Array<"audio" | "video">,
): FakeSourceGroup[] {
  return groups
    .filter((group) => sourceKey === "all" || group.sourceKey === sourceKey)
    .map((group) => ({
      ...group,
      playlists: group.playlists
        .map((playlist) => ({
          ...playlist,
          songs: playlist.songs.filter((song) =>
            mediaTypes.includes(song.mediaType || "audio"),
          ),
        }))
        .filter((playlist) => playlist.songs.length > 0 || !group.isOnline),
    }));
}

import { describe, expect, it } from "vitest";

describe("library source visibility", () => {
  it("keeps an online source visible when it has no playlists", () => {
    const result = filterVisibleGroups(
      [
        {
          sourceKey: "http://192.168.1.20:2080/api",
          isOnline: true,
          playlists: [],
        },
      ],
      "all",
      ["audio", "video"],
    );

    expect(result).toEqual([
      {
        sourceKey: "http://192.168.1.20:2080/api",
        isOnline: true,
        playlists: [],
      },
    ]);
  });

  it("keeps an online source visible when filters remove every song", () => {
    const result = filterVisibleGroups(
      [
        {
          sourceKey: "http://192.168.1.20:2080/api",
          isOnline: true,
          playlists: [
            {
              name: "Videos",
              songs: [{ mediaType: "video" }],
            },
          ],
        },
      ],
      "all",
      ["audio"],
    );

    expect(result).toEqual([
      {
        sourceKey: "http://192.168.1.20:2080/api",
        isOnline: true,
        playlists: [],
      },
    ]);
  });

  it("still hides offline sources without visible playlists", () => {
    const result = filterVisibleGroups(
      [
        {
          sourceKey: "http://192.168.1.20:2080/api",
          isOnline: false,
          playlists: [],
        },
      ],
      "all",
      ["audio", "video"],
    );

    expect(result).toEqual([
      {
        sourceKey: "http://192.168.1.20:2080/api",
        isOnline: false,
        playlists: [],
      },
    ]);
  });
});
