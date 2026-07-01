import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";
import { getLocalApiBase } from "@/utils/api";

export type MainView = "playlists" | "songs" | "search" | "downloads";
export type MainTab = "library" | "collections";
export type PlayerPanelMode = "collapsed" | "cover" | "lyrics";

export interface PlaylistSelection {
  name: string;
  songs: MusicInfo[];
}

type ContentPanelState = {
  kind: "content";
  view: MainView;
  activeTab: MainTab;
  selectedPlaylist: PlaylistSelection | null;
};

type PlayerPanelState = {
  kind: "player";
  mode: PlayerPanelMode;
};

type NavigationState = ContentPanelState | PlayerPanelState;

export const useUiStore = defineStore("ui", () => {
  const currentView = ref<MainView>("playlists");
  const activeTab = ref<MainTab>("library");
  const selectedPlaylist = ref<PlaylistSelection | null>(null);
  const showSettings = ref(false);
  const showAddDeviceModal = ref(false);
  const showUploadModal = ref(false);
  const showOnlineSearchModal = ref(false);
  const showLyricSearchModal = ref(false);
  const showActiveQueueModal = ref(false);
  const playerPanelMode = ref<PlayerPanelMode>("collapsed");
  const isScanning = ref(false);
  const uploadTargetApiBase = ref<string>(getLocalApiBase());
  const navigationStack = ref<NavigationState[]>([
    {
      kind: "content",
      view: "playlists",
      activeTab: "library",
      selectedPlaylist: null,
    },
  ]);

  const isPlaylistView = computed(() => currentView.value === "playlists");
  const canGoBack = computed(() => navigationStack.value.length > 1);

  const syncCurrentStateFromTop = () => {
    const topState = navigationStack.value[navigationStack.value.length - 1];
    if (!topState) {
      currentView.value = "playlists";
      activeTab.value = "library";
      selectedPlaylist.value = null;
      playerPanelMode.value = "collapsed";
      return;
    }

    if (topState.kind === "content") {
      currentView.value = topState.view;
      activeTab.value = topState.activeTab;
      selectedPlaylist.value = topState.selectedPlaylist;
      if (topState.view !== "songs" && topState.view !== "search") {
        selectedPlaylist.value = null;
      }
      playerPanelMode.value = "collapsed";
      return;
    }

    playerPanelMode.value = topState.mode;
  };

  const pushContentState = (view: MainView, activeTabValue?: MainTab) => {
    navigationStack.value.push({
      kind: "content",
      view,
      activeTab: activeTabValue ?? activeTab.value,
      selectedPlaylist: selectedPlaylist.value,
    });
    currentView.value = view;
    if (activeTabValue) {
      activeTab.value = activeTabValue;
    }
    if (view !== "songs" && view !== "search") {
      selectedPlaylist.value = null;
    }
    playerPanelMode.value = "collapsed";
  };

  const pushPlayerState = (mode: PlayerPanelMode) => {
    navigationStack.value.push({
      kind: "player",
      mode,
    });
    playerPanelMode.value = mode;
  };

  const replaceTopContentState = (view: MainView, activeTabValue?: MainTab) => {
    const topState = navigationStack.value[navigationStack.value.length - 1];
    if (topState?.kind === "content") {
      topState.view = view;
      topState.activeTab = activeTabValue ?? topState.activeTab;
      topState.selectedPlaylist = selectedPlaylist.value;
    } else {
      navigationStack.value.push({
        kind: "content",
        view,
        activeTab: activeTabValue ?? activeTab.value,
        selectedPlaylist: selectedPlaylist.value,
      });
    }

    currentView.value = view;
    if (activeTabValue) {
      activeTab.value = activeTabValue;
    }
    if (view !== "songs" && view !== "search") {
      selectedPlaylist.value = null;
    }
    playerPanelMode.value = "collapsed";
  };

  const setPlayerPanelMode = (mode: PlayerPanelMode) => {
    const topState = navigationStack.value[navigationStack.value.length - 1];
    if (topState?.kind === "player") {
      topState.mode = mode;
      playerPanelMode.value = mode;
      return;
    }

    if (mode === "collapsed") {
      playerPanelMode.value = "collapsed";
      return;
    }

    pushPlayerState(mode);
  };

  const openLibraryPlaylist = (playlist: PlaylistSelection) => {
    selectedPlaylist.value = playlist;
    pushContentState("songs");
  };

  const openCollectionPlaylist = (playlist: PlaylistSelection) => {
    selectedPlaylist.value = playlist;
    pushContentState("songs");
  };

  const showSearchResults = (searchQuery: string) => {
    if (!searchQuery.trim()) {
      return;
    }
    pushContentState("search");
  };

  const openDownloads = () => {
    pushContentState("downloads");
  };

  const showTabHome = (tab: MainTab) => {
    selectedPlaylist.value = null;
    replaceTopContentState("playlists", tab);
  };

  const resetSelectedPlaylist = () => {
    selectedPlaylist.value = null;
  };

  const backToPlaylists = () => {
    navigationStack.value = [
      {
        kind: "content",
        view: "playlists",
        activeTab: activeTab.value,
        selectedPlaylist: null,
      },
    ];
    currentView.value = "playlists";
    resetSelectedPlaylist();
    playerPanelMode.value = "collapsed";
  };

  const goBack = () => {
    if (navigationStack.value.length <= 1) {
      return false;
    }

    navigationStack.value.pop();
    syncCurrentStateFromTop();
    return true;
  };

  const enterPlayerPanel = (mode: PlayerPanelMode) => {
    setPlayerPanelMode(mode);
  };

  const normalizeForLayout = (isWideLayout: boolean) => {
    if (isWideLayout) {
      navigationStack.value = navigationStack.value.filter(
        (state) => state.kind === "content",
      );
      if (navigationStack.value.length === 0) {
        navigationStack.value.push({
          kind: "content",
          view: "playlists",
          activeTab: activeTab.value,
          selectedPlaylist: null,
        });
      }
      syncCurrentStateFromTop();
      return;
    }

    const topState = navigationStack.value[navigationStack.value.length - 1];
    if (topState?.kind === "player") {
      return;
    }

    if (playerPanelMode.value !== "collapsed") {
      pushPlayerState(playerPanelMode.value);
      return;
    }

    pushPlayerState("cover");
  };

  const openSettings = () => {
    showSettings.value = true;
  };

  const closeSettings = () => {
    showSettings.value = false;
  };

  const showActiveQueue = () => {
    showActiveQueueModal.value = true;
  };

  const resetLibrarySelection = () => {
    resetSelectedPlaylist();
    replaceTopContentState("playlists");
  };

  const openUploadForSource = (apiBase: string) => {
    uploadTargetApiBase.value = apiBase;
    showUploadModal.value = true;
  };

  return {
    currentView,
    activeTab,
    selectedPlaylist,
    showSettings,
    showAddDeviceModal,
    showUploadModal,
    showOnlineSearchModal,
    showLyricSearchModal,
    showActiveQueueModal,
    playerPanelMode,
    isScanning,
    uploadTargetApiBase,
    isPlaylistView,
    canGoBack,
    openLibraryPlaylist,
    openCollectionPlaylist,
    showSearchResults,
    openDownloads,
    showTabHome,
    resetSelectedPlaylist,
    backToPlaylists,
    goBack,
    enterPlayerPanel,
    setPlayerPanelMode,
    normalizeForLayout,
    openSettings,
    closeSettings,
    showActiveQueue,
    resetLibrarySelection,
    openUploadForSource,
  };
});
