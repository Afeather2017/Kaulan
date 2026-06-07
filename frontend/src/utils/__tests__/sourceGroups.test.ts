import { describe, expect, it, vi } from "vitest";

import { loadItemsIncrementally, upsertSortedItem } from "@/utils/sourceGroups";

interface FakeSource {
  sourceKey: string;
  isLoading: boolean;
  isOnline: boolean;
}

function indexBySourceKey(items: FakeSource[]): Record<string, FakeSource> {
  return Object.fromEntries(items.map((item) => [item.sourceKey, item]));
}

function flushMicrotasks(): Promise<void> {
  return new Promise((resolve) => {
    queueMicrotask(resolve);
  });
}

describe("source group incremental loading", () => {
  it("updates localhost before a fake slow server finishes", async () => {
    const fakeServerResolvers: Array<(value: FakeSource) => void> = [];
    const fakeServerPromise = new Promise<FakeSource>((resolve) => {
      fakeServerResolvers.push(resolve);
    });
    const updates: FakeSource[][] = [];

    const run = loadItemsIncrementally({
      keys: ["http://localhost:2080/api", "http://10.255.255.1:2080/api"],
      buildLoadingItem: (key) => ({
        sourceKey: key,
        isLoading: true,
        isOnline: false,
      }),
      fetchItem: async (key) => {
        if (key === "http://localhost:2080/api") {
          return {
            sourceKey: key,
            isLoading: false,
            isOnline: true,
          };
        }

        return fakeServerPromise;
      },
      getItemKey: (item) => item.sourceKey,
      sortItems: (items) => items,
      onUpdate: (items) => {
        updates.push(items.map((item) => ({ ...item })));
      },
    });

    expect(updates).toHaveLength(1);
    expect(updates[0]).toEqual([
      {
        sourceKey: "http://localhost:2080/api",
        isLoading: true,
        isOnline: false,
      },
      {
        sourceKey: "http://10.255.255.1:2080/api",
        isLoading: true,
        isOnline: false,
      },
    ]);

    await flushMicrotasks();

    expect(updates).toHaveLength(2);
    expect(indexBySourceKey(updates[1])).toEqual({
      "http://localhost:2080/api": {
        sourceKey: "http://localhost:2080/api",
        isLoading: false,
        isOnline: true,
      },
      "http://10.255.255.1:2080/api": {
        sourceKey: "http://10.255.255.1:2080/api",
        isLoading: true,
        isOnline: false,
      },
    });

    const fakeServerResolver = fakeServerResolvers[0];
    if (!fakeServerResolver) {
      throw new Error("Expected fake server resolver");
    }

    fakeServerResolver({
      sourceKey: "http://10.255.255.1:2080/api",
      isLoading: false,
      isOnline: false,
    });

    await run;

    expect(updates).toHaveLength(3);
    expect(indexBySourceKey(updates[2])).toEqual({
      "http://localhost:2080/api": {
        sourceKey: "http://localhost:2080/api",
        isLoading: false,
        isOnline: true,
      },
      "http://10.255.255.1:2080/api": {
        sourceKey: "http://10.255.255.1:2080/api",
        isLoading: false,
        isOnline: false,
      },
    });
  });

  it("ignores stale updates after the active run changes", async () => {
    let active = true;
    const staleResolvers: Array<(value: FakeSource) => void> = [];
    const stalePromise = new Promise<FakeSource>((resolve) => {
      staleResolvers.push(resolve);
    });
    const onUpdate = vi.fn();

    const run = loadItemsIncrementally({
      keys: ["http://10.255.255.1:2080/api"],
      buildLoadingItem: (key) => ({
        sourceKey: key,
        isLoading: true,
        isOnline: false,
      }),
      fetchItem: async () => stalePromise,
      getItemKey: (item) => item.sourceKey,
      sortItems: (items) => items,
      onUpdate,
      isActive: () => active,
    });

    expect(onUpdate).toHaveBeenCalledTimes(1);

    active = false;
    const staleResolver = staleResolvers[0];
    if (!staleResolver) {
      throw new Error("Expected stale resolver");
    }

    staleResolver({
      sourceKey: "http://10.255.255.1:2080/api",
      isLoading: false,
      isOnline: false,
    });

    await run;

    expect(onUpdate).toHaveBeenCalledTimes(1);
  });
});

describe("upsertSortedItem", () => {
  it("replaces an item by key and re-sorts the result", () => {
    const result = upsertSortedItem(
      [
        { sourceKey: "b", isLoading: true, isOnline: false },
        { sourceKey: "a", isLoading: false, isOnline: true },
      ],
      { sourceKey: "b", isLoading: false, isOnline: true },
      (item) => item.sourceKey,
      (items) =>
        [...items].sort((left, right) =>
          left.sourceKey.localeCompare(right.sourceKey),
        ),
    );

    expect(result).toEqual([
      { sourceKey: "a", isLoading: false, isOnline: true },
      { sourceKey: "b", isLoading: false, isOnline: true },
    ]);
  });

  it("keeps localhost first even when another source is loading", () => {
    const result = upsertSortedItem(
      [
        {
          sourceKey: "http://localhost:2080/api",
          isLoading: false,
          isOnline: true,
        },
        {
          sourceKey: "http://10.255.255.1:2080/api",
          isLoading: true,
          isOnline: false,
        },
      ],
      {
        sourceKey: "http://10.255.255.1:2080/api",
        isLoading: true,
        isOnline: false,
      },
      (item) => item.sourceKey,
      (items) =>
        [...items].sort((left, right) => {
          const leftIsLocalhost =
            left.sourceKey === "http://localhost:2080/api";
          const rightIsLocalhost =
            right.sourceKey === "http://localhost:2080/api";

          if (leftIsLocalhost && !rightIsLocalhost) {
            return -1;
          }
          if (rightIsLocalhost && !leftIsLocalhost) {
            return 1;
          }
          if (left.isLoading && !right.isLoading) {
            return -1;
          }
          if (!left.isLoading && right.isLoading) {
            return 1;
          }

          return left.sourceKey.localeCompare(right.sourceKey);
        }),
    );

    expect(result).toEqual([
      {
        sourceKey: "http://localhost:2080/api",
        isLoading: false,
        isOnline: true,
      },
      {
        sourceKey: "http://10.255.255.1:2080/api",
        isLoading: true,
        isOnline: false,
      },
    ]);
  });
});
