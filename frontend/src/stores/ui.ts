import { defineStore } from "pinia";
import { computed, ref } from "vue";
import type { MusicInfo } from "@/composables/useAudioPlayer";
import { getLocalApiBase } from "@/utils/api";

export type MainView = "playlists" | "songs" | "search";
export type MainTab = "library" | "collections";
export type PlayerPanelMode = "collapsed" | "cover" | "lyrics";

export interface PlaylistSelection {
  name: string;
  songs: MusicInfo[];
}

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

  const isPlaylistView = computed(() => currentView.value === "playlists");

  const openLibraryPlaylist = (playlist: PlaylistSelection) => {
    selectedPlaylist.value = playlist;
    currentView.value = "songs";
  };

  const openCollectionPlaylist = (playlist: PlaylistSelection) => {
    selectedPlaylist.value = playlist;
    currentView.value = "songs";
  };

  const showSearchResults = (searchQuery: string) => {
    if (!searchQuery.trim()) {
      return;
    }
    currentView.value = "search";
  };

  const resetSelectedPlaylist = () => {
    selectedPlaylist.value = null;
  };

  const backToPlaylists = () => {
    currentView.value = "playlists";
    resetSelectedPlaylist();
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
    currentView.value = "playlists";
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
    openLibraryPlaylist,
    openCollectionPlaylist,
    showSearchResults,
    resetSelectedPlaylist,
    backToPlaylists,
    openSettings,
    closeSettings,
    showActiveQueue,
    resetLibrarySelection,
    openUploadForSource,
  };
});
