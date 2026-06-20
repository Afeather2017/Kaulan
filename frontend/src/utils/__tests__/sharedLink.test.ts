import { beforeEach, describe, expect, it, vi } from "vitest";

import { clearSessionLocalApiBaseOverride, getLocalApiBase } from "@/utils/api";
import {
  applySharedLinkApiBase,
  consumeSharedLinkQuery,
  parseSharedLinkIntent,
} from "@/utils/sharedLink";

describe("sharedLink", () => {
  beforeEach(() => {
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
});
