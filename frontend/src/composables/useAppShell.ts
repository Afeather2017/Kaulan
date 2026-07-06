import { storeToRefs } from "pinia";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useAndroidBackNavigation } from "@/composables/useAndroidBackNavigation";
import { useAppShellLayout } from "@/composables/useAppShellLayout";
import { useCollectionsStore } from "@/stores/collections";
import { useDownloadsStore } from "@/stores/downloads";
import { usePlayerStore } from "@/stores/player";
import { useUiStore, type PlaylistSelection } from "@/stores/ui";
import { useLibraryStore } from "@/stores/library";
import {
  buildSongRowKey,
  inferMediaType,
} from "@/composables/useLibrarySources";
import {
  useSelection,
  type SongSelectionAction,
} from "@/composables/useSelection";
import { useLyrics } from "@/composables/useLyrics";
import { useLufs } from "@/composables/useLufs";
import {
  PlaybackStartError,
  type MusicInfo,
  type PlayMode,
} from "@/composables/useAudioPlayer";
import type {
  LibrarySourceGroup,
  LibrarySourceGroupSummary,
} from "@/types/library";
import { resolveSourceApiBase } from "@/utils/api";
import {
  applySharedLinkApiBase,
  buildSharedSongUrl,
  consumeSharedLinkQuery,
  parseSharedLinkIntent,
} from "@/utils/sharedLink";

// Related documentation:
// - `docs/runtime-platform-capabilities.md`
// - `docs/android/playback-session.md`

export function useAppShell() {
  const uiStore = useUiStore();
  const libraryStore = useLibraryStore();
  const playerStore = usePlayerStore();
  const collectionsStore = useCollectionsStore();
  const downloadsStore = useDownloadsStore();

  const ui = storeToRefs(uiStore);
  const library = storeToRefs(libraryStore);
  const player = storeToRefs(playerStore);
  const collections = storeToRefs(collectionsStore);
  const downloads = storeToRefs(downloadsStore);

  const selection = useSelection();
  const currentSongs = computed<MusicInfo[]>(
    () => ui.selectedPlaylist.value?.songs || [],
  );
  const visibleSongs = computed<MusicInfo[]>(() =>
    ui.currentView.value === "search"
      ? library.searchResults.value
      : currentSongs.value,
  );

  const selectedSourceMenuGroup = ref<LibrarySourceGroup | null>(null);
  const selectedSongListMenuTitle = ref<string | null>(null);
  const startupStatusMessage = ref("");
  const showSharedPlayPrompt = ref(false);
  const songMenuTab = computed<"library" | "collections">(() =>
    ui.currentView.value === "search" ? "library" : ui.activeTab.value,
  );
  const songSelectionActionLabel = computed(() => {
    switch (selection.songSelectionAction.value) {
      case "collection":
        return "添加到收藏夹";
      case "remove":
        return "移除收藏夹";
      case "delete":
        return "删除";
      default:
        return "";
    }
  });

  const {
    lyrics,
    rawLyricsContent,
    currentLyricIndex,
    hasLyrics,
    isLoading: isLyricsLoading,
    reloadLyrics,
  } = useLyrics(player.currentSong, player.currentTime, player.isPlaying);
  const onlineSearchInitialQuery = ref("");

  const {
    requestSongLufs,
    requestQueueLufs,
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

  playerStore.setQueuePrecacheHandler(async (queue, index, mode) => {
    void requestQueueLufs(queue, index, player.lufsPrecacheCount.value, mode);
  });

  const shellLayout = useAppShellLayout({
    currentSong: player.currentSong,
    playerPanelMode: ui.playerPanelMode,
    canGoBack: ui.canGoBack,
    currentView: ui.currentView,
  });

  watch(
    () => shellLayout.isWideLayout.value,
    (isWide) => {
      uiStore.normalizeForLayout(isWide);
    },
    { immediate: true },
  );

  const clearLibrarySelection = () => {
    library.selectedLibrarySourceKey.value = null;
    library.selectedLibraryPlaylistName.value = null;
  };

  const clearSongSelection = () => {
    selection.stopSongSelection();
  };

  const resetSelectionModes = () => {
    selection.stopSongSelection();
    selection.collectionSelectMode.value = false;
    selection.selectedCollectionsList.value.clear();
  };

  const backToPlaylists = () => {
    uiStore.backToPlaylists();
    clearLibrarySelection();
    library.searchQuery.value = "";
  };

  const handleBackToPlaylists = () => {
    closeSongListMenu();
    collectionsStore.closeCollectionMenu();
    backToPlaylists();
    resetSelectionModes();
  };

  const androidBackNavigation = useAndroidBackNavigation({
    showFilterSheet: library.showFilterSheet,
    selectedSourceMenuGroup,
    selectedSongListMenuTitle,
    selectedCollectionMenuName: collections.selectedCollectionMenuName,
    showActiveQueueModal: ui.showActiveQueueModal,
    showAddDeviceModal: ui.showAddDeviceModal,
    showUploadModal: ui.showUploadModal,
    showOnlineSearchModal: ui.showOnlineSearchModal,
    showLyricSearchModal: ui.showLyricSearchModal,
    showCreateCollection: collections.showCreateCollection,
    showAddToCollection: collections.showAddToCollection,
    showSettings: ui.showSettings,
    selectMode: selection.selectMode,
    clearSongSelection,
    collectionSelectMode: selection.collectionSelectMode,
    selectedCollectionsList: selection.selectedCollectionsList,
    playerPanelMode: ui.playerPanelMode,
    currentView: ui.currentView,
    closeSourceMenu: () => {
      selectedSourceMenuGroup.value = null;
    },
    closeSongListMenu: () => {
      selectedSongListMenuTitle.value = null;
    },
    closeCollectionMenu: collectionsStore.closeCollectionMenu,
    hideCreateCollectionModal: collectionsStore.hideCreateCollectionModal,
    hideAddToCollectionModal: collectionsStore.hideAddToCollectionModal,
    closeSettings: uiStore.closeSettings,
    goBack: uiStore.goBack,
    backToPlaylists: handleBackToPlaylists,
  });

  const handleSearch = () => {
    const trimmedQuery = library.searchQuery.value.trim();
    if (!trimmedQuery) {
      return;
    }
    uiStore.showSearchResults(trimmedQuery);
  };

  const handleSetActiveTab = (tab: "library" | "collections") => {
    uiStore.showTabHome(tab);
    resetSelectionModes();
  };

  const openDownloads = () => {
    uiStore.openDownloads();
  };

  const showLibraryHome = () => {
    uiStore.showTabHome("library");
  };

  const showCollectionsHome = () => {
    uiStore.showTabHome("collections");
  };

  const setStartupStatusMessage = (message: string) => {
    startupStatusMessage.value = message;
  };

  const clearStartupStatusMessage = () => {
    startupStatusMessage.value = "";
  };

  const openOnlineSearch = (query: string) => {
    const trimmedQuery = query.trim();
    if (!trimmedQuery) {
      return;
    }
    libraryStore.ensureOnlineSearchSourceExists();
    onlineSearchInitialQuery.value = trimmedQuery;
    ui.showOnlineSearchModal.value = true;
  };

  const openOnlineSearchFromQuery = () => {
    openOnlineSearch(library.trimmedSearchQuery.value);
  };

  const openOnlineLyricSearch = () => {
    if (!player.currentSong.value?.name?.trim()) {
      return;
    }
    ui.showLyricSearchModal.value = true;
  };

  const currentSongLyricApiBase = computed(() =>
    resolveSourceApiBase(player.currentSong.value?.source_key),
  );
  const currentSongShareUrl = computed(() =>
    player.currentSong.value
      ? buildSharedSongUrl(player.currentSong.value)
      : "",
  );

  const handleLyricApplied = async () => {
    await reloadLyrics();
  };

  const setVisiblePlayerPanel = (mode: "cover" | "lyrics") => {
    if (!player.currentSong.value) {
      return;
    }

    shellLayout.hasUserToggledLyric.value = true;
    if (shellLayout.isWideLayout.value) {
      ui.playerPanelMode.value = mode;
      return;
    }

    uiStore.enterPlayerPanel(mode);
  };

  const toggleVisiblePlayerPanel = () => {
    if (!player.currentSong.value) {
      return;
    }

    if (ui.playerPanelMode.value === "collapsed") {
      setVisiblePlayerPanel("cover");
      return;
    }

    setVisiblePlayerPanel(
      ui.playerPanelMode.value === "cover" ? "lyrics" : "cover",
    );
  };

  const syncSelectedPlaylistIntoPlayer = (playlist: PlaylistSelection) => {
    ui.selectedPlaylist.value = playlist;
    playerStore.setPlaylistSongs(playlist.songs);
    playerStore.resetPlaybackSourceContext();
  };

  const openSharedSongPlayer = async () => {
    const intent = parseSharedLinkIntent(window.location);
    applySharedLinkApiBase(intent);

    if (!intent.hasShareIntent) {
      return;
    }

    consumeSharedLinkQuery();
    clearStartupStatusMessage();
    showSharedPlayPrompt.value = false;

    if (intent.error) {
      setStartupStatusMessage("分享链接无效。");
      return;
    }

    const resolvedGroup = library.sourceGroups.value.find(
      (group) =>
        group.isOnline &&
        group.playlists.some((playlist) =>
          playlist.songs.some((song) => song.id === intent.songId),
        ),
    );

    if (!resolvedGroup || intent.songId === null) {
      setStartupStatusMessage("当前服务器上未找到这首分享歌曲。");
      return;
    }

    const resolvedPlaylist = resolvedGroup.playlists.find((playlist) =>
      playlist.songs.some((song) => song.id === intent.songId),
    );
    const songIndex =
      resolvedPlaylist?.songs.findIndex((song) => song.id === intent.songId) ??
      -1;
    const resolvedSong =
      songIndex >= 0 ? (resolvedPlaylist?.songs[songIndex] ?? null) : null;

    if (!resolvedPlaylist || !resolvedSong || songIndex < 0) {
      setStartupStatusMessage("当前服务器上未找到这首分享歌曲。");
      return;
    }

    syncSelectedPlaylistIntoPlayer({
      name: `曲库 / ${resolvedPlaylist.name} [${resolvedGroup.name}]`,
      songs: resolvedPlaylist.songs,
    });
    library.selectedLibrarySourceKey.value = resolvedGroup.sourceKey;
    library.selectedLibraryPlaylistName.value = resolvedPlaylist.name;
    ui.activeTab.value = "library";
    uiStore.openLibraryPlaylist({
      name: `曲库 / ${resolvedPlaylist.name} [${resolvedGroup.name}]`,
      songs: resolvedPlaylist.songs,
    });
    uiStore.enterPlayerPanel("cover");

    try {
      await playerStore.playSongFromPlaylist(
        resolvedSong,
        resolvedPlaylist.songs.slice(),
        songIndex,
      );
    } catch (error) {
      if (error instanceof PlaybackStartError) {
        showSharedPlayPrompt.value = true;
        setStartupStatusMessage("浏览器阻止了自动播放，请点击播放按钮继续。");
        return;
      }

      console.error("Failed to start shared song playback:", error);
      setStartupStatusMessage("播放分享歌曲失败。");
    }
  };

  const handleStartSharedPlayback = async () => {
    if (!player.currentSong.value) {
      return;
    }

    clearStartupStatusMessage();
    showSharedPlayPrompt.value = false;
    try {
      await playerStore.play();
    } catch (error) {
      if (error instanceof PlaybackStartError) {
        showSharedPlayPrompt.value = true;
        setStartupStatusMessage("浏览器阻止了自动播放，请点击播放按钮继续。");
        return;
      }

      console.error("Failed to resume shared playback:", error);
      setStartupStatusMessage("播放分享歌曲失败。");
    }
  };

  const handleSelectCollection = (name: string) => {
    const songs = collections.collectionPlaylists.value[name] || [];
    syncSelectedPlaylistIntoPlayer({
      name,
      songs,
    });
    clearLibrarySelection();
    uiStore.openCollectionPlaylist({
      name,
      songs,
    });
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
    uiStore.openLibraryPlaylist({
      name: `曲库 / ${playlistName} [${resolved.source.name}]`,
      songs: resolved.playlist.songs,
    });
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
    closeSongListMenu();
    collectionsStore.closeCollectionMenu();
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
    closeSongListMenu();
    collectionsStore.closeCollectionMenu();
    void libraryStore.refreshSourceGroups();
    ui.showAddDeviceModal.value = true;
  };

  const handleOpenSourceMenu = (group: LibrarySourceGroupSummary) => {
    closeSongListMenu();
    selectedSourceMenuGroup.value =
      library.sourceGroups.value.find(
        (item) => item.sourceKey === group.sourceKey,
      ) || null;
  };

  const closeSourceMenu = () => {
    selectedSourceMenuGroup.value = null;
  };

  const handleOpenSongListMenu = (title: string) => {
    closeSourceMenu();
    collectionsStore.closeCollectionMenu();
    selectedSongListMenuTitle.value = title;
  };

  const closeSongListMenu = () => {
    selectedSongListMenuTitle.value = null;
  };

  const startSongSelection = (action: SongSelectionAction) => {
    closeSongListMenu();
    selection.startSongSelection(action);
  };

  const startSongListCollectionSelection = () => {
    const action =
      ui.currentView.value === "songs" && ui.activeTab.value === "collections"
        ? "remove"
        : "collection";
    startSongSelection(action);
  };

  const startSongListDeleteSelection = () => {
    startSongSelection("delete");
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
    closeSongListMenu();
    collectionsStore.openCollectionMenu(collectionName);
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

  const deleteSongsBySource = async (apiBase: string, ids: number[]) => {
    const response = await fetch(`${apiBase}/music/batch`, {
      method: "DELETE",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ ids }),
    });
    const payload = await response.json();
    if (!response.ok) {
      throw new Error(payload?.message || "删除失败");
    }
    return payload as {
      deleted_ids: number[];
      failed: Array<{ id: number; reason: string }>;
      message: string;
    };
  };

  const deleteSelectedSongs = async () => {
    const selectedVisibleSongs = visibleSongs.value.filter((song) =>
      selection.selectedSongs.value.has(song.rowKey || buildSongRowKey(song)),
    );
    if (selectedVisibleSongs.length === 0) {
      alert("没有选中的歌曲");
      return;
    }

    if (
      !window.confirm(
        `确定要删除选中的 ${selectedVisibleSongs.length} 首歌曲吗？`,
      )
    ) {
      return;
    }

    const songsByApiBase = new Map<string, MusicInfo[]>();
    for (const song of selectedVisibleSongs) {
      const apiBase = resolveSourceApiBase(song.source_key);
      const bucket = songsByApiBase.get(apiBase);
      if (bucket) {
        bucket.push(song);
      } else {
        songsByApiBase.set(apiBase, [song]);
      }
    }

    const deletedKeys = new Set<string>();
    const failureMessages: string[] = [];

    for (const [apiBase, songs] of songsByApiBase) {
      const payload = await deleteSongsBySource(
        apiBase,
        songs.map((song) => song.id),
      );
      const deletedIds = new Set(payload.deleted_ids);
      for (const song of songs) {
        if (deletedIds.has(song.id)) {
          deletedKeys.add(song.rowKey || buildSongRowKey(song));
        }
      }
      for (const failure of payload.failed) {
        const failedSong = songs.find((song) => song.id === failure.id);
        failureMessages.push(
          `${failedSong?.name || failure.id}: ${failure.reason}`,
        );
      }
    }

    if (deletedKeys.size > 0) {
      collectionsStore.pruneSongsByKeys(deletedKeys, buildSongRowKey);
      await Promise.all(
        Array.from(songsByApiBase.keys()).map((apiBase) =>
          libraryStore.refreshSingleSource(apiBase),
        ),
      );
      await playerStore.refreshAndroidSession();
      if (
        ui.currentView.value === "songs" &&
        ui.activeTab.value === "collections" &&
        ui.selectedPlaylist.value
      ) {
        handleSelectCollection(ui.selectedPlaylist.value.name);
      }
    }

    clearSongSelection();

    if (failureMessages.length > 0) {
      alert(`部分歌曲删除失败：\n${failureMessages.join("\n")}`);
      return;
    }

    alert("删除成功");
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
      visibleSongs.value,
      selection.selectedSongs.value,
      buildSongRowKey,
      clearSongSelection,
    );
  };

  const handleSongSelectionAction = async () => {
    switch (selection.songSelectionAction.value) {
      case "collection":
        collectionsStore.showAddToCollectionModal();
        return;
      case "remove":
        await handleRemoveFromCollection();
        return;
      case "delete":
        await deleteSelectedSongs();
        return;
      default:
        return;
    }
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
      player.lufsPrecacheCount,
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
    const sharedLinkIntent =
      typeof window !== "undefined"
        ? parseSharedLinkIntent(window.location)
        : null;
    if (sharedLinkIntent) {
      applySharedLinkApiBase(sharedLinkIntent);
    }
    await libraryStore.triggerDatabaseUpdate(ui.isScanning);
    await libraryStore.initRuntimeCapabilities();
    collectionsStore.loadLocalCollections(buildSongRowKey, inferMediaType);
    void libraryStore.refreshDiscoveryState();
    await libraryStore.refreshSourceGroups();
    await playerStore.initAudio();
    if (sharedLinkIntent?.hasShareIntent) {
      await openSharedSongPlayer();
    }
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
    onlineSearchInitialQuery,
    libraryGroupSummaries: library.libraryGroupSummaries,
    searchResults: library.searchResults,
    filterSources: library.filterSources,
    onlineSearchSources: library.onlineSearchSources,
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
    activeDownloadJobs: downloads.activeJobs,
    selectedPlaylist: ui.selectedPlaylist,
    showSettings: ui.showSettings,
    showAddDeviceModal: ui.showAddDeviceModal,
    showUploadModal: ui.showUploadModal,
    showOnlineSearchModal: ui.showOnlineSearchModal,
    showLyricSearchModal: ui.showLyricSearchModal,
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
    currentSongLyricApiBase,
    currentSongShareUrl,
    isPlaying: player.isPlaying,
    currentTime: player.currentTime,
    duration: player.duration,
    playMode: player.playMode,
    showLufs: player.showLufs,
    lufsPrecacheCount: player.lufsPrecacheCount,
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
    songSelectionActionLabel,
    collectionSelectMode: selection.collectionSelectMode,
    selectedCollectionsList: selection.selectedCollectionsList,
    hasSelectedNonAllMusicCollection:
      selection.hasSelectedNonAllMusicCollection,
    lyrics,
    rawLyricsContent,
    currentLyricIndex,
    isLyricsLoading,
    hasLyrics,
    selectedSourceMenuGroup,
    selectedSongListMenuTitle,
    startupStatusMessage,
    showSharedPlayPrompt,
    songMenuTab,
    showBackButton: shellLayout.showBackButton,
    isWideLayout: shellLayout.isWideLayout,
    isPlayerPanelVisible: shellLayout.isPlayerPanelVisible,
    isLyricPanelVisible: shellLayout.isLyricPanelVisible,
    resolveSongCoverUrl: shellLayout.resolveSongCoverUrl,
    handleSearch,
    handleSetActiveTab,
    openDownloads,
    showLibraryHome,
    showCollectionsHome,
    handleActionBack: androidBackNavigation.handleActionBack,
    handleShowSettingsModal,
    handleShowLufsChange: playerStore.setShowLufsState,
    handleLufsPrecacheCountChange: playerStore.setLufsPrecacheCountState,
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
    handleOpenSongListMenu,
    closeSongListMenu,
    startSongListCollectionSelection,
    startSongListDeleteSelection,
    closeCollectionMenu: collectionsStore.closeCollectionMenu,
    renameCollection,
    deleteCollectionFromMenu,
    openOnlineSearchFromQuery,
    openOnlineLyricSearch,
    handleLyricApplied,
    handleSelectCollection,
    handleSelectLibraryPlaylist,
    handleBackToPlaylists,
    toggleSelectMode: selection.toggleSelectMode,
    toggleSongSelection: selection.toggleSongSelection,
    toggleCollectionSelectMode: selection.toggleCollectionSelectMode,
    toggleCollectionSelection: selection.toggleCollectionSelection,
    handlePlaySong,
    handleRemoveFromCollection,
    handleSongSelectionAction,
    handleLyricLineClick,
    handleShowActiveQueue: uiStore.showActiveQueue,
    handleStartSharedPlayback,
    togglePlayerPanelMode: toggleVisiblePlayerPanel,
    showCoverPanel: () => {
      setVisiblePlayerPanel("cover");
    },
    showLyricsPanel: () => {
      setVisiblePlayerPanel("lyrics");
    },
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
