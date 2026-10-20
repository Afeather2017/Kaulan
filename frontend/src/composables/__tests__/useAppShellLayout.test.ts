import { afterEach, describe, expect, it, vi } from "vitest";
import { effectScope, ref } from "vue";
import { useAppShellLayout } from "../useAppShellLayout";
import type { MusicInfo } from "@/types/music";

// Related documentation: `docs/android/playback-session.md`.

vi.mock("@/utils/api", () => ({
  resolveSourceApiBase: (sourceKey?: string | null) =>
    sourceKey || "http://localhost:2080/api",
}));

const song = (id: number): MusicInfo => ({
  id,
  name: `Song ${id}`,
  path: `/music/${id}.mp3`,
  lufs: null,
  source_key: "http://localhost:2080/api",
  cover_url: null,
});

describe("useAppShellLayout cover recovery", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("retries an Android localhost cover twice, then keeps the placeholder", () => {
    vi.useFakeTimers();
    const currentSong = ref<MusicInfo | null>(song(51));
    const scope = effectScope();
    const layout = scope.run(() =>
      useAppShellLayout({
        currentSong,
        playerPanelMode: ref("cover"),
        canGoBack: ref(false),
        currentView: ref("playlists"),
      }),
    );
    if (!layout) throw new Error("layout scope did not initialize");

    const coverUrl = "http://localhost:2080/api/music/id/51/cover";
    expect(layout.resolveSongCoverUrl(currentSong.value)).toBe(coverUrl);

    layout.handleCoverLoadError(currentSong.value);
    expect(layout.resolveSongCoverUrl(currentSong.value)).toBeNull();
    vi.advanceTimersByTime(1_000);
    expect(layout.resolveSongCoverUrl(currentSong.value)).toBe(coverUrl);

    layout.handleCoverLoadError(currentSong.value);
    vi.advanceTimersByTime(2_000);
    expect(layout.resolveSongCoverUrl(currentSong.value)).toBe(coverUrl);

    layout.handleCoverLoadError(currentSong.value);
    vi.advanceTimersByTime(10_000);
    expect(layout.resolveSongCoverUrl(currentSong.value)).toBeNull();
    scope.stop();
  });

  it("clears a prior failure when the current cover changes", async () => {
    const currentSong = ref<MusicInfo | null>(song(51));
    const scope = effectScope();
    const layout = scope.run(() =>
      useAppShellLayout({
        currentSong,
        playerPanelMode: ref("cover"),
        canGoBack: ref(false),
        currentView: ref("playlists"),
      }),
    );
    if (!layout) throw new Error("layout scope did not initialize");

    layout.handleCoverLoadError(currentSong.value);
    currentSong.value = song(52);
    await Promise.resolve();

    expect(layout.resolveSongCoverUrl(currentSong.value)).toBe(
      "http://localhost:2080/api/music/id/52/cover",
    );
    scope.stop();
  });
});
