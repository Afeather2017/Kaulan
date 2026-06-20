import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  clearSessionLocalApiBaseOverride,
  getLocalApiBase,
  setSessionLocalApiBaseOverride,
} from "@/utils/api";
import {
  DEFAULT_LUFS_PRECACHE_COUNT,
  MAX_LUFS_PRECACHE_COUNT,
  getDefaultOnlineSearchApiBase,
  getLufsPrecacheCount,
  normalizeLufsPrecacheCount,
  removeDefaultOnlineSearchApiBase,
  setDefaultOnlineSearchApiBase,
  setLufsPrecacheCount,
} from "@/utils/storage";

describe("online search source storage", () => {
  beforeEach(() => {
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => {
        store.set(key, value);
      },
      removeItem: (key: string) => {
        store.delete(key);
      },
      clear: () => {
        store.clear();
      },
    });
    removeDefaultOnlineSearchApiBase();
  });

  it("defaults to localhost when unset", () => {
    expect(getDefaultOnlineSearchApiBase()).toBe("http://localhost:2080/api");
  });

  it("persists a selected online search source", () => {
    setDefaultOnlineSearchApiBase("http://192.168.1.20:2080/api");

    expect(getDefaultOnlineSearchApiBase()).toBe(
      "http://192.168.1.20:2080/api",
    );
  });

  it("falls back to localhost when saving an empty value", () => {
    setDefaultOnlineSearchApiBase("   ");

    expect(getDefaultOnlineSearchApiBase()).toBe("http://localhost:2080/api");
  });
});

describe("LUFS pre-cache count storage", () => {
  beforeEach(() => {
    const store = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => store.get(key) ?? null,
      setItem: (key: string, value: string) => {
        store.set(key, value);
      },
      removeItem: (key: string) => {
        store.delete(key);
      },
      clear: () => {
        store.clear();
      },
    });
  });

  it("defaults to the queue pre-cache count", () => {
    expect(getLufsPrecacheCount()).toBe(DEFAULT_LUFS_PRECACHE_COUNT);
  });

  it("clamps and persists the queue pre-cache count", () => {
    setLufsPrecacheCount(MAX_LUFS_PRECACHE_COUNT + 4);

    expect(getLufsPrecacheCount()).toBe(MAX_LUFS_PRECACHE_COUNT);
  });

  it("normalizes invalid input to the default", () => {
    expect(normalizeLufsPrecacheCount(Number.NaN)).toBe(
      DEFAULT_LUFS_PRECACHE_COUNT,
    );
  });
});

describe("session local api base override", () => {
  beforeEach(() => {
    clearSessionLocalApiBaseOverride();
  });

  it("defaults to localhost when no session override exists", () => {
    expect(getLocalApiBase()).toBe("http://localhost:2080/api");
  });

  it("uses the session override when one is applied", () => {
    setSessionLocalApiBaseOverride("http://192.168.1.20:2080");

    expect(getLocalApiBase()).toBe("http://192.168.1.20:2080/api");
  });
});
