import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";

import {
  resolveSongMenuTab,
  useUiStore,
  type PlaylistSelection,
} from "@/stores/ui";

describe("ui store navigation stack", () => {
  const playlist: PlaylistSelection = {
    name: "Test Playlist",
    songs: [],
  };

  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("returns to the previous panel after leaving search", () => {
    const store = useUiStore();

    store.openLibraryPlaylist(playlist);
    store.enterPlayerPanel("cover");
    store.showSearchResults("test");

    expect(store.currentView).toBe("search");
    expect(store.playerPanelMode).toBe("collapsed");

    expect(store.goBack()).toBe(true);
    expect(store.playerPanelMode).toBe("cover");

    expect(store.goBack()).toBe(true);
    expect(store.currentView).toBe("songs");
    expect(store.selectedPlaylist?.name).toBe("Test Playlist");
  });

  it("uses search as a real screen with a back target", () => {
    const store = useUiStore();

    store.showSearchResults("shared song");

    expect(store.currentView).toBe("search");
    expect(store.canGoBack).toBe(true);

    expect(store.goBack()).toBe(true);
    expect(store.currentView).toBe("playlists");
    expect(store.playerPanelMode).toBe("collapsed");
  });

  it("preserves collection actions for search within a collection", () => {
    const store = useUiStore();

    store.showTabHome("collections");
    store.openCollectionPlaylist(playlist);
    store.showSearchResults("shared song");

    expect(store.currentView).toBe("search");
    expect(store.activeTab).toBe("collections");
    expect(store.selectedPlaylist).toEqual(playlist);
    expect(
      resolveSongMenuTab(
        store.currentView,
        store.activeTab,
        store.selectedPlaylist,
      ),
    ).toBe("collections");
  });

  it("keeps global search actions in library mode", () => {
    const store = useUiStore();

    store.showTabHome("collections");
    store.showSearchResults("shared song");

    expect(
      resolveSongMenuTab(
        store.currentView,
        store.activeTab,
        store.selectedPlaylist,
      ),
    ).toBe("library");
  });

  it("normalizes player entries across layout changes", () => {
    const store = useUiStore();

    store.openLibraryPlaylist(playlist);
    store.enterPlayerPanel("lyrics");

    store.normalizeForLayout(true);

    expect(store.currentView).toBe("songs");
    expect(store.playerPanelMode).toBe("collapsed");

    store.normalizeForLayout(false);

    expect(store.playerPanelMode).toBe("cover");
    expect(store.canGoBack).toBe(true);
  });

  it("treats downloads as a top-level destination", () => {
    const store = useUiStore();

    store.openDownloads();

    expect(store.currentView).toBe("downloads");
    expect(store.canGoBack).toBe(false);

    store.showTabHome("library");

    expect(store.currentView).toBe("playlists");
    expect(store.activeTab).toBe("library");
  });
});
