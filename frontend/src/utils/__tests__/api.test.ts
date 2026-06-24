import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  clearSessionLocalApiBaseOverride,
  getLocalApiBase,
  LOCALHOST_API_BASE,
} from "@/utils/api";

describe("getLocalApiBase", () => {
  beforeEach(() => {
    vi.unstubAllGlobals();
    clearSessionLocalApiBaseOverride();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    clearSessionLocalApiBaseOverride();
  });

  it("falls back to localhost when no browser origin is available", () => {
    vi.stubGlobal("window", {
      location: { origin: "http://localhost:2080" },
    });

    expect(getLocalApiBase()).toBe(LOCALHOST_API_BASE);
  });

  it("uses the browser origin inside a regular desktop browser", () => {
    vi.stubGlobal("window", {
      location: { origin: "http://192.168.1.20:2080" },
    });

    expect(getLocalApiBase()).toBe("http://192.168.1.20:2080/api");
  });

  it("ignores the browser origin inside a Tauri Android webview", () => {
    vi.stubGlobal("window", {
      __TAURI_INTERNALS__: { invoke: () => undefined },
      location: { origin: "http://tauri.localhost" },
    });

    expect(getLocalApiBase()).toBe(LOCALHOST_API_BASE);
  });

  it("ignores https tauri.localhost origins on Tauri webviews", () => {
    vi.stubGlobal("window", {
      __TAURI_INTERNALS__: { invoke: () => undefined },
      location: { origin: "https://tauri.localhost" },
    });

    expect(getLocalApiBase()).toBe(LOCALHOST_API_BASE);
  });
});
