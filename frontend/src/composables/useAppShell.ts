import { storeToRefs } from "pinia";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useAndroidBackNavigation } from "@/composables/useAndroidBackNavigation";
import { useAppShellLayout } from "@/composables/useAppShellLayout";
import { useCollectionsStore } from "@/stores/collections";
import { usePlayerStore } from "@/stores/player";
import { useUiStore, type PlaylistSelection } from "@/stores/ui";
import { useLibraryStore } from "@/stores/library";
import {
  buildSongRowKey,
  inferMediaType,
} from "@/composables/useLibrarySources";
import { useSelection } from "@/composables/useSelection";
import { useLyrics } from "@/composables/useLyrics";
import { useLufs } from "@/composables/useLufs";
import { useQueueEditing } from "@/composables/useQueueEditing";
import type { MusicInfo, PlayMode } from "@/composables/useAudioPlayer";
import type {
  LibrarySourceGroup,
  LibrarySourceGroupSummary,
} from "@/types/library";

// Related documentation:
// - `docs/runtime-platform-capabilities.md`
// - `docs/android/playback-session.md`

export function useAppShell() {
  const uiStore = useUiStore();
  const libraryStore = useLibraryStore();
  const playerStore = usePlayerStore();
  const collectionsStore = useCollectionsStore();

  const ui = storeToRefs(uiStore);
  const library = storeToRefs(libraryStore);
  const player = storeToRefs(playerStore);
  const collections = storeToRefs(collectionsStore);

  const selection = useSelection();
  const currentSongs = computed<MusicInfo[]>(
    () => ui.selectedPlaylist.value?.songs || [],
  );

  const selectedSourceMenuGroup = ref<LibrarySourceGroup | null>(null);
  const selectedSongMenuSong = ref<MusicInfo | null>(null);

  const { lyrics, currentLyricIndex, hasLyrics } = useLyrics(
    player.currentSong,
    player.currentTime,
    player.isPlaying,
  );

  const {
    requestSongLufs,
    resolveSongForPlayback,
    syncPlaybackMetadataFromBackend,
  } = useLufs({
    currentSong: player.currentSong,
    activeQueue: player.activeQueue,
    searchPlaybackSongs: player.searchPlaybackSongs,
    selectedPlaylist: ui.selectedPlaylist,
    sourceGroups: library.sourceGroups,
    isAndroidPlayer: player.isAndroidPlayer,
    syncAndroidQueueState: playerStore.syncAndroidQueueState,
    syncSelectedLibraryPlaylist: libraryStore.syncSelectedLibraryPlaylist,
    currentView: ui.currentView,
  });

  playerStore.setPrepareSongHandler(async (song) => {
    return await resolveSongForPlayback(song);
  });

  const shellLayout = useAppShellLayout({
    currentSong: player.currentSong,
    currentView: ui.currentView,
    playerPanelMode: ui.playerPanelMode,
  });

  const clearLibrarySelection = () => {
    library.selectedLibrarySourceKey.value = null;
    library.selectedLibraryPlaylistName.value = null;
  };

  const clearSongSelection = () => {
    selection.selectMode.value = false;
    selection.selectedSongs.value.clear();
  };

  const resetSelectionModes = () => {
    selection.selectMode.value = false;
    selection.selectedSongs.value.clear();
    selection.collectionSelectMode.value = false;
    selection.selectedCollectionsList.value.clear();
  };

  const backToPlaylists = () => {
    uiStore.backToPlaylists();
    clearLibrarySelection();
    library.searchQuery.value = "";
  };

  const handleBackToPlaylists = () => {
    collectionsStore.closeCollectionMenu();
    closeSongMenu();
    backToPlaylists();
    resetSelectionModes();
  };

  const androidBackNavigation = useAndroidBackNavigation({
    showFilterSheet: library.showFilterSheet,
    selectedSourceMenuGroup,
    selectedCollectionMenuName: collections.selectedCollectionMenuName,
    selectedSongMenuSong,
    showActiveQueueModal: ui.showActiveQueueModal,
    showAddDeviceModal: ui.showAddDeviceModal,
    showUploadModal: ui.showUploadModal,
    showOnlineSearchModal: ui.showOnlineSearchModal,
    showCreateCollection: collections.showCreateCollection,
    showAddToCollection: collections.showAddToCollection,
    showSettings: ui.showSettings,
    selectMode: selection.selectMode,
    selectedSongs: selection.selectedSongs,
    collectionSelectMode: selection.collectionSelectMode,
    selectedCollectionsList: selection.selectedCollectionsList,
    playerPanelMode: ui.playerPanelMode,
    isWideLayout: shellLayout.isWideLayout,
    currentView: ui.currentView,
    closeSourceMenu: () => {
      selectedSourceMenuGroup.value = null;
    },
    closeCollectionMenu: collectionsStore.closeCollectionMenu,
    closeSongMenu: () => {
      selectedSongMenuSong.value = null;
    },
    hideCreateCollectionModal: collectionsStore.hideCreateCollectionModal,
    hideAddToCollectionModal: collectionsStore.hideAddToCollectionModal,
    closeSettings: uiStore.closeSettings,
    backToPlaylists: handleBackToPlaylists,
  });

  const handleSearch = () => {
    uiStore.showSearchResults(library.searchQuery.value);
  };

  const openOnlineSearchFromQuery = () => {
    if (!library.trimmedSearchQuery.value) {
      return;
    }
    libraryStore.ensureOnlineSearchSourceExists();
    ui.showOnlineSearchModal.value = true;
  };

  const syncSelectedPlaylistIntoPlayer = (playlist: PlaylistSelection) => {
    ui.selectedPlaylist.value = playlist;
    playerStore.setPlaylistSongs(playlist.songs);
    playerStore.resetPlaybackSourceContext();
  };

  const handleSelectCollection = (name: string) => {
    const songs = collections.collectionPlaylists.value[name] || [];
    syncSelectedPlaylistIntoPlayer({
      name,
      songs,
    });
    clearLibrarySelection();
    ui.currentView.value = "songs";
  };

  const handleSelectLibraryPlaylist = (
    group: LibrarySourceGroupSummary,
    playlistName: string,
  ) => {
    const resolved = libraryStore.getLibraryPlaylist(
      group.sourceKey,
      playlistName,
    );
    if (!resolved) {
      return;
    }

    syncSelectedPlaylistIntoPlayer({
      name: `曲库 / ${playlistName} [${resolved.source.name}]`,
      songs: resolved.playlist.songs,
    });
    library.selectedLibrarySourceKey.value = resolved.source.sourceKey;
    library.selectedLibraryPlaylistName.value = playlistName;
    ui.currentView.value = "songs";
  };

  const handleLyricLineClick = async (time: number) => {
    if (!player.currentSong.value) {
      return;
    }

    if (!player.audioElement.value || player.duration.value === 0) {
      await playerStore.playSong(player.currentSong.value, time);
      return;
    }

    await playerStore.seekToTime(time);
    if (!player.isPlaying.value) {
      await playerStore.play();
    }
  };

  const handlePlaySong = async (song: MusicInfo, index?: number) => {
    if (ui.currentView.value === "search") {
      await playerStore.playSongFromSearch(
        song,
        library.searchResults.value.slice(),
        index,
      );
      return;
    }

    await playerStore.playSongFromPlaylist(
      song,
      currentSongs.value.slice(),
      index,
    );
  };

  const handleSongStart = async (
    currentSongInfo: { id: number },
    nextSongInfo: { id: number } | null,
  ) => {
    console.log(
      "[app] onSongStart called: currentSongId =",
      currentSongInfo.id,
      ", nextSongInfo =",
      nextSongInfo,
    );
    if (player.isAndroidPlayer.value) {
      console.log(
        "[app] Android playback backend handles next-track LUFS pre-cache",
      );
      return;
    }
    const allSongs = player.playbackSongs.value;

    if (!nextSongInfo) {
      console.log("[app] No next song, skipping pre-cache");
      return;
    }

    const nextSong = allSongs.find((song) => song.id === nextSongInfo.id);
    if (!nextSong) {
      console.log(
        "[app] Next song metadata missing in current playback list, skipping pre-cache",
      );
      return;
    }
    if (nextSong.lufs !== null) {
      console.log(
        "[app] Next song already has LUFS:",
        nextSong.lufs,
        ", skipping pre-cache",
      );
      return;
    }

    if (
      player.playMode.value === ("loop" as PlayMode) &&
      currentSongInfo.id === nextSongInfo.id
    ) {
      console.log("[app] Loop mode with same song, skipping pre-cache");
      return;
    }

    console.log("[app] Pre-caching LUFS for next song ID:", nextSongInfo.id);
    await requestSongLufs(nextSong, "next");
  };

  playerStore.setSongStartHandler(handleSongStart);

  const handleManageCollections = () => {
    uiStore.closeSettings();
    ui.activeTab.value = "collections";
    uiStore.backToPlaylists();
    clearLibrarySelection();
  };

  const handleShowSettingsModal = () => {
    closeSourceMenu();
    collectionsStore.closeCollectionMenu();
    closeSongMenu();
    uiStore.openSettings();
  };

  const handleDirectoryChanged = () => {
    void libraryStore.refreshSourceGroups();
    void playerStore.refreshAndroidSession();
  };

  const handleDatabaseUpdated = async () => {
    await libraryStore.refreshSourceGroups();
    await playerStore.refreshAndroidSession();
  };

  const handleUploadComplete = async () => {
    ui.showUploadModal.value = false;
    await libraryStore.refreshSourceGroups();
  };

  const handleSourcesUpdated = async () => {
    await libraryStore.refreshSourceGroups();
    libraryStore.ensureOnlineSearchSourceExists();
  };

  const handleDeviceConnected = async (apiBase: string) => {
    await libraryStore.refreshSingleSource(apiBase);
    libraryStore.ensureOnlineSearchSourceExists();

    const connectedSource = library.sourceGroups.value.find(
      (group) => group.apiBase === apiBase,
    );
    if (!connectedSource?.capabilities.canUseForOnlineSearch) {
      return;
    }

    const shouldUseForOnlineSearch = window.confirm(
      `设备 “${connectedSource.name}” 已连接。是否将它设为默认在线搜索来源？`,
    );
    if (shouldUseForOnlineSearch) {
      libraryStore.setOnlineSearchSource(apiBase);
    }
  };

  const handleOnlineDownloadComplete = async () => {
    await libraryStore.refreshSourceGroups();
    await playerStore.refreshAndroidSession();
  };

  const handlePreviewTrack = async (song: MusicInfo) => {
    await playerStore.playPreviewTrack(song);
  };

  const handleAddDevice = () => {
    selectedSourceMenuGroup.value = null;
    collectionsStore.closeCollectionMenu();
    closeSongMenu();
    void libraryStore.refreshSourceGroups();
    ui.showAddDeviceModal.value = true;
  };

  const handleOpenSourceMenu = (group: LibrarySourceGroupSummary) => {
    selectedSourceMenuGroup.value =
      library.sourceGroups.value.find(
        (item) => item.sourceKey === group.sourceKey,
      ) || null;
  };

  const closeSourceMenu = () => {
    selectedSourceMenuGroup.value = null;
  };

  const handleRetrySourceConnection = async (apiBase: string) => {
    await libraryStore.retrySourceConnection(apiBase);
  };

  const handleShowSourceDetails = (group: LibrarySourceGroup) => {
    closeSourceMenu();
    libraryStore.showSourceDetails(group);
  };

  const handleSetOnlineSearchSourceFromMenu = (group: LibrarySourceGroup) => {
    closeSourceMenu();
    libraryStore.setOnlineSearchSourceFromMenu(group);
  };

  const handleDeleteSource = (group: LibrarySourceGroup) => {
    closeSourceMenu();
    const deleted = libraryStore.deleteSource(group);
    if (!deleted) {
      return;
    }

    if (library.selectedLibrarySourceKey.value === group.sourceKey) {
      handleBackToPlaylists();
    }
  };

  const handleOpenCollectionMenu = (collectionName: string) => {
    selectedSongMenuSong.value = null;
    collectionsStore.openCollectionMenu(collectionName);
  };

  const openSongMenu = (song: MusicInfo) => {
    collections.selectedCollectionMenuName.value = null;
    selectedSongMenuSong.value = song;
  };

  const closeSongMenu = () => {
    selectedSongMenuSong.value = null;
  };

  const { queueSongNextFromMenu, addSongToQueueFromMenu } = useQueueEditing({
    activeQueue: player.activeQueue,
    playbackSongs: player.playbackSongs,
    currentSong: player.currentSong,
    selectedSongMenuSong,
    replaceQueue: playerStore.replaceQueue,
    handlePlaySong,
    closeSongMenu,
  });

  const addSongToCollectionFromMenu = () => {
    const song = selectedSongMenuSong.value;
    if (!song) {
      return;
    }

    collectionsStore.addSongToCollectionFromMenu(song);
    closeSongMenu();
  };

  const removeSongFromCollectionFromMenu = () => {
    const song = selectedSongMenuSong.value;
    if (!song || !ui.selectedPlaylist.value) {
      return;
    }

    collectionsStore.removeSingleSongFromCollection(
      ui.selectedPlaylist.value.name,
      song,
      buildSongRowKey,
    );
    handleSelectCollection(ui.selectedPlaylist.value.name);
    closeSongMenu();
  };

  const openUploadForSource = (group: LibrarySourceGroup) => {
    uiStore.openUploadForSource(group.apiBase);
    closeSourceMenu();
  };

  const handleUpdateSourceDatabase = async (group: LibrarySourceGroup) => {
    closeSourceMenu();
    ui.isScanning.value = true;
    try {
      await libraryStore.updateSourceDatabase(group);
    } catch (error) {
      console.error("Failed to update source database:", error);
      alert(`更新失败: ${error}`);
    } finally {
      ui.isScanning.value = false;
    }
  };

  const handleChangeSourceDirectory = async (group: LibrarySourceGroup) => {
    closeSourceMenu();
    try {
      await libraryStore.changeSourceDirectory(group);
    } catch (error) {
      console.error("Failed to change source directory:", error);
      alert(`更改目录失败: ${error}`);
    }
  };

  const handleSongCollectionAction = (song: MusicInfo) => {
    openSongMenu(song);
  };

  const handleDeleteSelectedCollections = async () => {
    if (selection.selectedCollectionsList.value.size === 0) {
      alert("请选择要删除的收藏夹");
      return;
    }

    if (
      !confirm(
        `确定要删除选中的 ${selection.selectedCollectionsList.value.size} 个收藏夹吗？`,
      )
    ) {
      return;
    }

    let deletedCount = 0;
    for (const collectionName of selection.selectedCollectionsList.value) {
      if (collectionsStore.deleteCollectionByName(collectionName)) {
        deletedCount += 1;
      }
    }

    alert(`已删除 ${deletedCount} 个收藏夹`);
    selection.collectionSelectMode.value = false;
    selection.selectedCollectionsList.value.clear();
  };

  const handleShowAddToCollectionModal = () => {
    collectionsStore.showAddToCollectionModal();
  };

  const handleRemoveFromCollection = async () => {
    if (!ui.selectedPlaylist.value) {
      return;
    }

    const removed = collectionsStore.removeSongsFromCollection(
      ui.selectedPlaylist.value.name,
      selection.selectedSongs.value,
      buildSongRowKey,
    );
    if (!removed) {
      return;
    }

    handleSelectCollection(ui.selectedPlaylist.value.name);
    clearSongSelection();
  };

  const handleShowCreateModal = () => {
    collectionsStore.showCreateCollectionModal();
  };

  const handleCreateCollectionFromAddModal = () => {
    collectionsStore.createCollectionFromAddModal();
  };

  const handleCreateCollection = () => {
    collectionsStore.createCollection();
  };

  const addToCollection = async () => {
    collectionsStore.addToCollection(
      currentSongs.value,
      selection.selectedSongs.value,
      buildSongRowKey,
      clearSongSelection,
    );
  };

  const renameCollection = () => {
    const previousName = collections.selectedCollectionMenuName.value;
    const nextName = collectionsStore.renameCollection();
    if (!previousName || !nextName || previousName === nextName) {
      return;
    }

    if (ui.selectedPlaylist.value?.name === previousName) {
      handleSelectCollection(nextName);
    }
  };

  const deleteCollectionFromMenu = () => {
    const deletedName = collectionsStore.deleteCollectionFromMenu();
    if (!deletedName) {
      return;
    }

    if (ui.selectedPlaylist.value?.name === deletedName) {
      handleBackToPlaylists();
    }
  };

  watch(
    [
      player.volumeMode,
      player.manualVolume,
      player.manualVolumeInput,
      player.fixedLufs,
      player.fixedLufsInput,
      () => player.currentSong.value?.id ?? null,
      () => player.currentSong.value?.lufs ?? null,
      () =>
        player.playbackSongs.value
          .map((song) => `${song.id}:${song.lufs ?? "null"}`)
          .join("|"),
    ],
    () => {
      void playerStore.syncNormalization();
    },
    { deep: true, immediate: true },
  );

  watch(
    () =>
      player.activeQueue.value
        .map((song) => `${song.id}:${song.lufs ?? "null"}`)
        .join("|"),
    () => {
      syncPlaybackMetadataFromBackend();
    },
  );

  watch(
    library.sourceGroups,
    () => {
      libraryStore.syncSelectedLibraryPlaylist(
        ui.currentView.value,
        ui.selectedPlaylist,
      );
    },
    { deep: true },
  );

  watch(ui.activeTab, () => {
    if (ui.currentView.value !== "playlists") {
      backToPlaylists();
    }
    resetSelectionModes();
  });

  onMounted(async () => {
    await libraryStore.triggerDatabaseUpdate(ui.isScanning);
    await libraryStore.initRuntimeCapabilities();
    collectionsStore.loadLocalCollections(buildSongRowKey, inferMediaType);
    void libraryStore.refreshDiscoveryState();
    await libraryStore.refreshSourceGroups();
    await playerStore.initAudio();
    await androidBackNavigation.registerAndroidBackHandler();
    shellLayout.updateLayoutMode();
    window.addEventListener("resize", shellLayout.updateLayoutMode);
  });

  onBeforeUnmount(() => {
    if (typeof window !== "undefined") {
      window.removeEventListener("resize", shellLayout.updateLayoutMode);
    }
    androidBackNavigation.cleanupAndroidBackHandler();
  });

  return {
    searchQuery: library.searchQuery,
    sourceGroups: library.sourceGroups,
    showFilterSheet: library.showFilterSheet,
    draftSourceFilterKey: library.draftSourceFilterKey,
    draftMediaTypes: library.draftMediaTypes,
    selectedLibrarySourceKey: library.selectedLibrarySourceKey,
    selectedLibraryPlaylistName: library.selectedLibraryPlaylistName,
    onlineSearchApiBase: library.onlineSearchApiBase,
    libraryGroupSummaries: library.libraryGroupSummaries,
    searchResults: library.searchResults,
    filterSources: library.filterSources,
    trimmedSearchQuery: library.trimmedSearchQuery,
    onlineSearchSourceName: library.onlineSearchSourceName,
    ensureOnlineSearchSourceExists: libraryStore.ensureOnlineSearchSourceExists,
    setOnlineSearchSource: libraryStore.setOnlineSearchSource,
    openFilterSheet: libraryStore.openFilterSheet,
    toggleDraftMediaType: libraryStore.toggleDraftMediaType,
    applyLibraryFilter: libraryStore.applyLibraryFilter,
    resetLibraryFilter: libraryStore.resetLibraryFilter,
    currentView: ui.currentView,
    activeTab: ui.activeTab,
    selectedPlaylist: ui.selectedPlaylist,
    showSettings: ui.showSettings,
    showAddDeviceModal: ui.showAddDeviceModal,
    showUploadModal: ui.showUploadModal,
    showOnlineSearchModal: ui.showOnlineSearchModal,
    showActiveQueueModal: ui.showActiveQueueModal,
    playerPanelMode: ui.playerPanelMode,
    isScanning: ui.isScanning,
    uploadTargetApiBase: ui.uploadTargetApiBase,
    playbackSource: player.playbackSource,
    searchPlaybackSongs: player.searchPlaybackSongs,
    currentSongs,
    audioElement: player.audioElement,
    activeQueue: player.activeQueue,
    currentSong: player.currentSong,
    isPlaying: player.isPlaying,
    currentTime: player.currentTime,
    duration: player.duration,
    playMode: player.playMode,
    showLufs: player.showLufs,
    volumeMode: player.volumeMode,
    manualVolume: player.manualVolume,
    manualVolumeInput: player.manualVolumeInput,
    fixedLufs: player.fixedLufs,
    fixedLufsInput: player.fixedLufsInput,
    volumeModeLabels: playerStore.volumeModeLabels,
    timerMinutes: player.timerMinutes,
    timerMinutesInput: player.timerMinutesInput,
    timerActive: player.timerActive,
    timerStatusDisplay: player.timerStatusDisplay,
    isAndroidPlayer: player.isAndroidPlayer,
    localCollections: collections.localCollections,
    collectionNames: collections.collectionNames,
    collectionPlaylists: collections.collectionPlaylists,
    showAddToCollection: collections.showAddToCollection,
    selectedCollections: collections.selectedCollections,
    newCollectionName: collections.newCollectionName,
    showCreateCollection: collections.showCreateCollection,
    selectedCollectionMenuName: collections.selectedCollectionMenuName,
    selectMode: selection.selectMode,
    selectedSongs: selection.selectedSongs,
    collectionSelectMode: selection.collectionSelectMode,
    selectedCollectionsList: selection.selectedCollectionsList,
    hasSelectedNonAllMusicCollection:
      selection.hasSelectedNonAllMusicCollection,
    lyrics,
    currentLyricIndex,
    hasLyrics,
    selectedSourceMenuGroup,
    selectedSongMenuSong,
    showBackButton: shellLayout.showBackButton,
    showActionBar: shellLayout.showActionBar,
    isWideLayout: shellLayout.isWideLayout,
    isPlayerPanelVisible: shellLayout.isPlayerPanelVisible,
    isLyricPanelVisible: shellLayout.isLyricPanelVisible,
    resolveSongCoverUrl: shellLayout.resolveSongCoverUrl,
    handleSearch,
    handleActionBack: androidBackNavigation.handleActionBack,
    handleShowSettingsModal,
    handleShowLufsChange: playerStore.setShowLufsState,
    handleSetTimerPreset: playerStore.setTimerPreset,
    handleStartTimer: playerStore.startSleepTimer,
    handleCancelTimer: playerStore.cancelSleepTimer,
    handleDirectoryChanged,
    handleDatabaseUpdated,
    handleDatabaseUpdateStart: () => {
      ui.isScanning.value = true;
    },
    handleDatabaseUpdateEnd: () => {
      ui.isScanning.value = false;
    },
    handleManageCollections,
    handleSourcesUpdated,
    handleDeviceConnected,
    handleToggleCollectionSelection: collectionsStore.toggleCollectionSelection,
    handleCreateCollectionFromAddModal,
    handleCreateCollection,
    addToCollection,
    hideAddToCollectionModal: collectionsStore.hideAddToCollectionModal,
    hideCreateCollectionModal: collectionsStore.hideCreateCollectionModal,
    handleUploadComplete,
    handleOnlineDownloadComplete,
    handlePreviewTrack,
    handlePlayQueueSong: playerStore.playQueueSong,
    handleUpdateSourceDatabase,
    handleSetOnlineSearchSourceFromMenu,
    handleChangeSourceDirectory,
    handleRetrySourceConnection,
    handleShowSourceDetails,
    handleDeleteSource,
    closeSourceMenu,
    closeCollectionMenu: collectionsStore.closeCollectionMenu,
    renameCollection,
    deleteCollectionFromMenu,
    closeSongMenu,
    queueSongNextFromMenu,
    addSongToQueueFromMenu,
    addSongToCollectionFromMenu,
    removeSongFromCollectionFromMenu,
    openOnlineSearchFromQuery,
    handleSelectCollection,
    handleSelectLibraryPlaylist,
    handleBackToPlaylists,
    toggleSelectMode: selection.toggleSelectMode,
    toggleSongSelection: selection.toggleSongSelection,
    toggleCollectionSelectMode: selection.toggleCollectionSelectMode,
    toggleCollectionSelection: selection.toggleCollectionSelection,
    handlePlaySong,
    handleRemoveFromCollection,
    handleShowAddToCollectionModal,
    handleSongCollectionAction,
    handleLyricLineClick,
    handleShowActiveQueue: uiStore.showActiveQueue,
    togglePlayerPanelMode: shellLayout.togglePlayerPanelMode,
    showCoverPanel: shellLayout.showCoverPanel,
    showLyricsPanel: shellLayout.showLyricsPanel,
    handleCoverLoadError: shellLayout.handleCoverLoadError,
    seekToTime: playerStore.seekToTime,
    play: playerStore.play,
    pause: playerStore.pause,
    previousSong: playerStore.previousSong,
    nextSong: playerStore.nextSong,
    togglePlayMode: playerStore.togglePlayMode,
    openUploadForSource,
    handleAddDevice,
    handleOpenSourceMenu,
    handleOpenCollectionMenu,
    handleDeleteSelectedCollections,
    handleShowCreateModal,
    hideSettingsModal: uiStore.closeSettings,
  };
}
