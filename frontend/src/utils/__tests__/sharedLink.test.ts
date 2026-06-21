import { beforeEach, describe, expect, it, vi } from "vitest";

import { clearSessionLocalApiBaseOverride, getLocalApiBase } from "@/utils/api";
import {
  applySharedLinkApiBase,
  buildSharedSongUrl,
  consumeSharedLinkQuery,
  parseSharedLinkIntent,
} from "@/utils/sharedLink";

// Related documentation: `docs/shared-song-links.md`

describe("sharedLink", () => {
  beforeEach(() => {
    vi.unstubAllGlobals();
    clearSessionLocalApiBaseOverride();
  });

  it("parses a valid shared song id from the current url", () => {
    expect(
      parseSharedLinkIntent({
        origin: "http://192.168.1.20:2080",
        search: "?id=42",
      }),
    ).toEqual({
      apiBase: "http://192.168.1.20:2080/api",
      hasShareIntent: true,
      songId: 42,
    });
  });

  it("marks a non-numeric id as invalid", () => {
    expect(
      parseSharedLinkIntent({
        origin: "http://192.168.1.20:2080",
        search: "?id=abc",
      }),
    ).toEqual({
      apiBase: "http://192.168.1.20:2080/api",
      hasShareIntent: true,
      songId: null,
      error: "invalid_id",
    });
  });

  it("does not create share intent when id is absent", () => {
    expect(
      parseSharedLinkIntent({
        origin: "http://192.168.1.20:2080",
        search: "",
      }),
    ).toEqual({
      apiBase: "http://192.168.1.20:2080/api",
      hasShareIntent: false,
      songId: null,
    });
  });

  it("applies the session api base override for shared links", () => {
    applySharedLinkApiBase({
      apiBase: "http://192.168.1.20:2080/api",
      hasShareIntent: true,
      songId: 42,
    });

    expect(getLocalApiBase()).toBe("http://192.168.1.20:2080/api");
  });

  it("removes the id query after consuming the shared link", () => {
    const replaceState = vi.fn();
    const originalWindow = window;
    const nextWindow = {
      ...originalWindow,
      location: {
        ...originalWindow.location,
        href: "http://192.168.1.20:2080/?id=42&foo=bar#player",
      },
      history: {
        ...originalWindow.history,
        replaceState,
      },
    };

    vi.stubGlobal("window", nextWindow);
    consumeSharedLinkQuery();

    expect(replaceState).toHaveBeenCalledWith({}, "", "/?foo=bar#player");
  });

  it("builds a shared song url from the song source api base", () => {
    expect(
      buildSharedSongUrl({
        id: 42,
        source_key: "http://192.168.1.20:2080/api",
      }),
    ).toBe("http://192.168.1.20:2080/?id=42");
  });

  it("builds a shared song url from the local api base when source is absent", () => {
    vi.stubGlobal("window", {
      ...window,
      location: {
        ...window.location,
        origin: "http://192.168.1.20:2080",
      },
    });

    expect(
      buildSharedSongUrl({
        id: 7,
        source_key: null,
      }),
    ).toBe("http://192.168.1.20:2080/?id=7");
  });

  it("does not build a shared song url for temporary preview tracks", () => {
    expect(
      buildSharedSongUrl({
        id: 42,
        source_key: "http://192.168.1.20:2080/api",
        is_temporary: true,
      }),
    ).toBe("");
  });
});
