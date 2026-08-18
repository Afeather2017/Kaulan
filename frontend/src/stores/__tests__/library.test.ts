import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import type { Ref } from "vue";

import type { LibrarySourceGroup } from "@/types/library";
import * as librarySourcesModule from "@/composables/useLibrarySources";
import { useLibraryStore } from "@/stores/library";

// Related documentation:
// - `docs/android/playback-session.md` (initial-load gate before adoption)

type LibraryMockState = {
  resolvers: Array<() => void>;
  failNext: boolean;
  calls: number;
  setGroups: (groups: LibrarySourceGroup[]) => void;
};

// vi.mock factories are hoisted above every declaration in vitest 0.30 (no
// vi.hoisted yet), so shared state must live inside the factory closure and
// be exposed as a test-only module export.
vi.mock("@/composables/useLibrarySources", async () => {
  const { ref } = await import("vue");
  const groupsRef = ref<LibrarySourceGroup[]>([]) as Ref<LibrarySourceGroup[]>;
  const state: LibraryMockState = {
    resolvers: [],
    failNext: false,
    calls: 0,
    setGroups: (groups: LibrarySourceGroup[]) => {
      groupsRef.value = groups;
    },
  };
  return {
    __mockState: state,
    useLibrarySources: () => ({
      sourceGroups: groupsRef,
      refreshSourceGroups: () => {
        state.calls += 1;
        if (state.failNext) {
          state.failNext = false;
          return Promise.reject(new Error("source fetch failed"));
        }
        return new Promise<void>((resolve) => {
          state.resolvers.push(resolve);
        });
      },
    }),
  };
});

const mock = (
  librarySourcesModule as unknown as {
    __mockState: LibraryMockState;
  }
).__mockState;

// True when `promise` is still unsettled after a short real-time window.
const isPending = (promise: Promise<unknown>): Promise<boolean> =>
  Promise.race([
    promise.then(
      () => false,
      () => false,
    ),
    new Promise<boolean>((resolve) => setTimeout(() => resolve(true), 10)),
  ]);

describe("library store initial-load gate", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mock.resolvers = [];
    mock.failNext = false;
    mock.calls = 0;
    mock.setGroups([]);
  });

  it("resolves immediately before the first refresh arms the gate", async () => {
    const store = useLibraryStore();

    await expect(store.waitForInitialSourceGroups()).resolves.toBeUndefined();
    expect(mock.calls).toBe(0);
  });

  it("arms the gate on the first refresh and settles it when that load settles", async () => {
    const store = useLibraryStore();
    const load = store.refreshSourceGroups();
    const gate = store.waitForInitialSourceGroups();

    expect(await isPending(gate)).toBe(true);

    mock.resolvers[0]();
    await Promise.all([load, gate]);
  });

  it("settles the gate even when the caller drops the load promise", async () => {
    const store = useLibraryStore();
    void store.refreshSourceGroups();
    const gate = store.waitForInitialSourceGroups();

    mock.resolvers[0]();
    await expect(gate).resolves.toBeUndefined();
  });

  it("keeps the gate tied to the first load when a later refresh overlaps it", async () => {
    const store = useLibraryStore();
    const first = store.refreshSourceGroups();
    const second = store.refreshSourceGroups();
    const gate = store.waitForInitialSourceGroups();

    mock.resolvers[0]();
    await expect(gate).resolves.toBeUndefined();
    expect(await isPending(second)).toBe(true);

    mock.resolvers[1]();
    await Promise.all([first, second]);
  });

  it("does not re-arm the gate after the initial load settled", async () => {
    const store = useLibraryStore();
    const first = store.refreshSourceGroups();
    mock.resolvers[0]();
    await first;

    const second = store.refreshSourceGroups();
    await expect(store.waitForInitialSourceGroups()).resolves.toBeUndefined();

    mock.resolvers[1]();
    await second;
  });

  it("never rejects the gate when the initial load fails (caller still sees the error)", async () => {
    const store = useLibraryStore();
    mock.failNext = true;
    const attempt = store.refreshSourceGroups();

    await expect(attempt).rejects.toThrow("source fetch failed");
    await expect(store.waitForInitialSourceGroups()).resolves.toBeUndefined();
  });
});
