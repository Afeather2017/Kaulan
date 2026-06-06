import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getDefaultOnlineSearchApiBase,
  removeDefaultOnlineSearchApiBase,
  setDefaultOnlineSearchApiBase,
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
