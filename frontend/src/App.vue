<template>
  <div id="app" class="music-player">
    <div class="app-window">
      <div class="top-bar">
        <button
          class="icon-btn settings-btn"
          aria-label="Settings"
          @click="handleShowSettingsModal"
        >
          <i class="fas fa-cog"></i>
        </button>
        <SearchBar v-model="searchQuery" @search="handleSearch" />
      </div>

      <div v-if="showActionBar" class="action-bar">
        <button
          v-if="showBackButton"
          class="action-btn"
          @click="handleActionBack"
        >
          返回
        </button>
        <div class="action-spacer"></div>
      </div>

      <!-- Scanning Message -->
      <div v-if="isScanning" class="scanning-message">扫描中...</div>

      <div class="main-area" :class="{ 'wide-layout': isWideLayout }">
        <div class="list-panel" v-show="!isPlayerPanelVisible || isWideLayout">
          <AppContentView
            :current-view="currentView"
            :active-tab="activeTab"
            :library-group-summaries="libraryGroupSummaries"
            :collection-names="collectionNames"
            :collection-playlists="collectionPlaylists"
            :collection-select-mode="collectionSelectMode"
            :selected-collections-list="selectedCollectionsList"
            :has-selected-non-all-music="hasSelectedNonAllMusicCollection()"
            :selected-playlist-title="selectedPlaylist?.name || ''"
            :current-songs="currentSongs"
            :select-mode="selectMode"
            :selected-songs="selectedSongs"
            :current-song-name="currentSong?.name"
            :show-lufs="showLufs"
            :trimmed-search-query="trimmedSearchQuery"
            :search-results="searchResults"
            @set-active-tab="activeTab = $event"
            @open-filter-sheet="openFilterSheet"
            @add-device="handleAddDevice"
            @select-library-playlist="handleSelectLibraryPlaylist"
            @open-source-menu="handleOpenSourceMenu"
            @retry-source-connection="handleRetrySourceConnection"
            @toggle-collection-select-mode="toggleCollectionSelectMode"
            @toggle-collection-selection="toggleCollectionSelection"
            @select-collection="handleSelectCollection"
            @show-create-modal="handleShowCreateModal"
            @delete-selected-collections="handleDeleteSelectedCollections"
            @open-collection-menu="handleOpenCollectionMenu"
            @back-to-playlists="handleBackToPlaylists"
            @toggle-select-mode="toggleSelectMode"
            @toggle-song-selection="toggleSongSelection"
            @play-song="handlePlaySong"
            @remove-from-collection="handleRemoveFromCollection"
            @show-add-to-collection-modal="handleShowAddToCollectionModal"
            @song-action="handleSongCollectionAction"
            @open-online-search-from-query="openOnlineSearchFromQuery"
            @reset-library-filter="resetLibraryFilter"
          />
        </div>
        <AppPlayerView
          :is-player-panel-visible="isPlayerPanelVisible"
          :is-wide-layout="isWideLayout"
          :is-lyric-panel-visible="isLyricPanelVisible"
          :select-mode="selectMode"
          :has-lyrics="hasLyrics"
          :lyrics="lyrics"
          :current-lyric-index="currentLyricIndex"
          :current-song-id="currentSong?.id ?? null"
          :current-song-name="currentSong?.name"
          :cover-url="resolveSongCoverUrl(currentSong)"
          :current-time="currentTime"
          :duration="duration"
          :is-playing="isPlaying"
          :play-mode="playMode"
          @lyric-line-click="handleLyricLineClick"
          @show-cover-panel="showCoverPanel"
          @show-lyrics-panel="showLyricsPanel"
          @cover-load-error="handleCoverLoadError(currentSong)"
          @seek="seekToTime"
          @toggle-play-mode="togglePlayMode"
          @previous="previousSong"
          @play="play"
          @pause="pause"
          @next="nextSong"
          @show-active-queue="handleShowActiveQueue"
          @toggle-panel-mode="togglePlayerPanelMode"
        />
      </div>
    </div>

    <!-- Settings Modal -->
    <SettingsModal
      v-if="showSettings"
      :volume-mode="volumeMode"
      :manual-volume="manualVolume"
      :manual-volume-input="manualVolumeInput"
      :fixed-lufs="fixedLufs"
      :fixed-lufs-input="fixedLufsInput"
      :show-lufs="showLufs"
      :timer-minutes="timerMinutes"
      :timer-minutes-input="timerMinutesInput"
      :timer-active="timerActive"
      :timer-status-display="timerStatusDisplay"
      :volume-mode-labels="volumeModeLabels"
      @close="hideSettingsModal"
      @update:volume-mode="volumeMode = $event"
      @update:manual-volume="manualVolume = $event"
      @update:manual-volume-input="manualVolumeInput = $event"
      @update:fixed-lufs="fixedLufs = $event"
      @update:fixed-lufs-input="fixedLufsInput = $event"
      @update:show-lufs="handleShowLufsChange"
      @update:timer-minutes="timerMinutes = $event"
      @update:timer-minutes-input="timerMinutesInput = $event"
      @set-timer-preset="handleSetTimerPreset"
      @start-timer="handleStartTimer"
      @cancel-timer="handleCancelTimer"
      @directory-changed="handleDirectoryChanged"
      @database-updated="handleDatabaseUpdated"
      @database-update-start="handleDatabaseUpdateStart"
      @database-update-end="handleDatabaseUpdateEnd"
      @open-upload-modal="showUploadModal = true"
      @manage-collections="handleManageCollections"
    />

    <AddDeviceModal
      v-if="showAddDeviceModal"
      @close="showAddDeviceModal = false"
      @sources-updated="handleSourcesUpdated"
      @device-connected="handleDeviceConnected"
    />

    <!-- Add to Collection Modal -->
    <AddToCollectionModal
      v-if="showAddToCollection"
      :collections="localCollections"
      :selected-collection-ids="selectedCollections"
      @close="hideAddToCollectionModal"
      @confirm="addToCollection"
      @toggle-selection="handleToggleCollectionSelection"
      @create-new="handleCreateCollectionFromAddModal"
    />

    <!-- Create Collection Modal -->
    <CreateCollectionModal
      v-if="showCreateCollection"
      v-model="newCollectionName"
      @close="hideCreateCollectionModal"
      @confirm="handleCreateCollection"
    />

    <!-- Upload Modal -->
    <UploadModal
      v-if="showUploadModal"
      :api-base="uploadTargetApiBase"
      @close="showUploadModal = false"
      @upload-complete="handleUploadComplete"
    />

    <LibraryFilterSheet
      v-if="showFilterSheet"
      :sources="filterSources"
      :draft-source-key="draftSourceFilterKey"
      :draft-media-types="draftMediaTypes"
      @close="showFilterSheet = false"
      @apply="applyLibraryFilter"
      @reset="resetLibraryFilter"
      @update:draft-source-key="draftSourceFilterKey = $event"
      @toggle-media-type="toggleDraftMediaType"
    />

    <OnlineSearchModal
      v-if="showOnlineSearchModal"
      :initial-query="searchQuery"
      :api-base="onlineSearchApiBase"
      :source-name="onlineSearchSourceName"
      @close="showOnlineSearchModal = false"
      @download-complete="handleOnlineDownloadComplete"
      @preview-track="handlePreviewTrack"
    />

    <!-- Active Queue Modal -->
    <ActiveQueueModal
      v-if="showActiveQueueModal"
      :songs="activeQueue"
      :current-song-name="currentSong?.name"
      @close="showActiveQueueModal = false"
      @play="handlePlayQueueSong"
    />

    <AppActionSheets
      :active-tab="activeTab"
      :selected-source-menu-group="selectedSourceMenuGroup"
      :selected-collection-menu-name="selectedCollectionMenuName"
      :selected-song-menu-song="selectedSongMenuSong"
      @close-source-menu="closeSourceMenu"
      @refresh-source="handleUpdateSourceDatabase"
      @upload-to-source="openUploadForSource"
      @set-online-search-source="handleSetOnlineSearchSourceFromMenu"
      @change-source-directory="handleChangeSourceDirectory"
      @retry-source-connection="handleRetrySourceConnection"
      @show-source-details="handleShowSourceDetails"
      @delete-source="handleDeleteSource"
      @close-collection-menu="closeCollectionMenu"
      @rename-collection="renameCollection"
      @delete-collection="deleteCollectionFromMenu"
      @close-song-menu="closeSongMenu"
      @queue-song-next="queueSongNextFromMenu"
      @add-song-to-queue="addSongToQueueFromMenu"
      @add-song-to-collection="addSongToCollectionFromMenu"
      @remove-song-from-collection="removeSongFromCollectionFromMenu"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from "vue";
import SearchBar from "@/components/SearchBar.vue";
import AppActionSheets from "@/components/AppActionSheets.vue";
import AppContentView from "@/components/AppContentView.vue";
import AppPlayerView from "@/components/AppPlayerView.vue";
import LibraryFilterSheet from "@/components/LibraryFilterSheet.vue";
import { type SongInfo } from "@/components/SongListView.vue";
import AddDeviceModal from "@/components/modals/AddDeviceModal.vue";
import SettingsModal from "@/components/modals/SettingsModal.vue";
import ActiveQueueModal from "@/components/modals/ActiveQueueModal.vue";
import AddToCollectionModal from "@/components/modals/AddToCollectionModal.vue";
import CreateCollectionModal from "@/components/modals/CreateCollectionModal.vue";
import OnlineSearchModal from "@/components/modals/OnlineSearchModal.vue";
import UploadModal from "@/components/modals/UploadModal.vue";
import { useAudioPlayer, type MusicInfo } from "@/composables/useAudioPlayer";
import {
  buildSongRowKey,
  inferMediaType,
  useLibrarySources,
} from "@/composables/useLibrarySources";
import { useSelection } from "@/composables/useSelection";
import { useTimer } from "@/composables/useTimer";
import { useVolume } from "@/composables/useVolume";
import { useLyrics } from "@/composables/useLyrics";
import { useCollections } from "@/composables/useCollections";
import { useLufs } from "@/composables/useLufs";
import { LOCALHOST_API_BASE, resolveSourceApiBase } from "@/utils/api";
import {
  getShowLufs,
  getTimerExitAppOnAndroid,
  setShowLufs,
} from "@/utils/storage";
import { checkIsAndroid } from "@/utils/platform";
import type {
  LibrarySourceGroup,
  LibrarySourceGroupSummary,
} from "@/types/library";

type MainView = "playlists" | "songs" | "search";
type MainTab = "library" | "collections";
type PlayerPanelMode = "collapsed" | "cover" | "lyrics";

interface PlaylistSelection {
  name: string;
  songs: MusicInfo[];
}

const currentView = ref<MainView>("playlists");
const activeTab = ref<MainTab>("library");
const selectedPlaylist = ref<PlaylistSelection | null>(null);
const isAndroidRuntime = ref(false);

const {
  searchQuery,
  sourceGroups,
  showFilterSheet,
  draftSourceFilterKey,
  draftMediaTypes,
  selectedLibrarySourceKey,
  selectedLibraryPlaylistName,
  onlineSearchApiBase,
  libraryGroupSummaries,
  searchResults,
  filterSources,
  trimmedSearchQuery,
  onlineSearchSourceName,
  ensureOnlineSearchSourceExists,
  setOnlineSearchSource,
  refreshSourceGroups,
  refreshSingleSource,
  refreshDiscoveryState,
  openFilterSheet,
  toggleDraftMediaType,
  applyLibraryFilter,
  resetLibraryFilter,
  getLibraryPlaylist,
  syncSelectedLibraryPlaylist,
  retrySourceConnection,
  showSourceDetails,
  setOnlineSearchSourceFromMenu,
  deleteSource,
  updateSourceDatabase,
  changeSourceDirectory,
  triggerDatabaseUpdate,
} = useLibrarySources({
  isAndroidRuntime,
});

const playbackSource = ref<"playlist" | "search">("playlist");
const searchPlaybackSongs = ref<SongInfo[]>([]);
const currentSongs = computed<MusicInfo[]>(
  () => selectedPlaylist.value?.songs || [],
);

// Handler for song start event - trigger LUFS pre-caching for next song
// Defined as ref to allow useAudioPlayer to reference it, but implemented below
const handleSongStartRef = ref<
  | ((
      currentSongInfo: { id: number },
      nextSongInfo: { id: number } | null,
    ) => void)
  | null
>(null);

const {
  audioElement,
  activeQueue,
  currentSong,
  isPlaying,
  currentTime,
  duration,
  playMode,
  play,
  pause,
  playSong,
  playSongAtIndex,
  togglePlayMode,
  previousSong,
  nextSong,
  seekToTime,
  setTimedPause,
  resetPlaylist,
  initAudio,
  refreshAndroidSession,
  isAndroidPlayer,
  syncAndroidQueueState,
  syncNormalizationConfig,
} = useAudioPlayer({
  songs: () => {
    if (playbackSource.value === "search") {
      return searchPlaybackSongs.value;
    }
    if (selectedPlaylist.value) {
      return selectedPlaylist.value.songs;
    }
    return [];
  },
  onSongEnd: () => {},
  onSongStart: (currentSongInfo, nextSongInfo) => {
    handleSongStartRef.value?.(currentSongInfo, nextSongInfo);
  },
  prepareSong: async (song) => await resolveSongForPlayback(song),
});

const playbackSongs = computed(() => {
  return activeQueue.value;
});

const {
  selectMode,
  selectedSongs,
  collectionSelectMode,
  selectedCollectionsList,
  toggleSelectMode,
  toggleSongSelection,
  toggleCollectionSelectMode,
  toggleCollectionSelection,
  hasSelectedNonAllMusicCollection,
} = useSelection();

const {
  timerMinutes,
  timerMinutesInput,
  timerActive,
  timerStatusDisplay,
  startTimer,
  cancelTimer,
} = useTimer(() => {
  // Timer complete callback
  if (isAndroidPlayer.value) {
    void handleAndroidTimerComplete();
  } else if (isPlaying.value) {
    void pause();
  } else if (audioElement.value) {
    audioElement.value.pause();
  }
});

// Lyrics composable for synchronized lyrics display
const { lyrics, currentLyricIndex, hasLyrics } = useLyrics(
  currentSong,
  currentTime,
  isPlaying,
);

// Additional state
const showSettings = ref(false);
const showAddDeviceModal = ref(false);
const showLufs = ref(getShowLufs());
const showUploadModal = ref(false);
const showOnlineSearchModal = ref(false);
const showActiveQueueModal = ref(false);
const playerPanelMode = ref<PlayerPanelMode>("collapsed");
const isWideLayout = ref(false);
const hasUserToggledLyric = ref(false);
const isScanning = ref(false);
const failedCoverUrls = ref<Set<string>>(new Set());
const selectedSourceMenuGroup = ref<LibrarySourceGroup | null>(null);
const selectedSongMenuSong = ref<MusicInfo | null>(null);
const uploadTargetApiBase = ref<string>(LOCALHOST_API_BASE);
let androidBackListener: { unregister(): Promise<void> } | null = null;

const resolveSongCoverUrl = (song: MusicInfo | null): string | null => {
  if (!song) {
    return null;
  }

  const coverUrl =
    song.cover_url ||
    `${resolveSourceApiBase(song.source_key)}/music/id/${song.id}/cover`;
  return failedCoverUrls.value.has(coverUrl) ? null : coverUrl;
};

const getSongMenuIdentity = (song: {
  id: number;
  name: string;
  stream_url?: string | null;
  source_key?: string | null;
}): string => {
  return song.stream_url || buildSongRowKey(song);
};

// Computed helper for audio player
const {
  volumeMode,
  manualVolume,
  manualVolumeInput,
  fixedLufs,
  fixedLufsInput,
  volumeModeLabels,
  calculateVolume,
} = useVolume(currentSong, playbackSongs);

const showBackButton = computed(() => {
  return (
    currentView.value === "search" ||
    (playerPanelMode.value !== "collapsed" && !isWideLayout.value)
  );
});

const showActionBar = computed(() => showBackButton.value);

const isPlayerPanelVisible = computed(
  () => isWideLayout.value || playerPanelMode.value !== "collapsed",
);

const isLyricPanelVisible = computed(() => playerPanelMode.value === "lyrics");

const updateLayoutMode = () => {
  if (typeof window === "undefined") return;
  const isWide = window.matchMedia("(min-aspect-ratio: 1/1)").matches;
  const wasWide = isWideLayout.value;
  isWideLayout.value = isWide;
  if (isWide && (!wasWide || playerPanelMode.value === "collapsed")) {
    playerPanelMode.value = "cover";
    return;
  }
  if (!isWide && wasWide && !hasUserToggledLyric.value) {
    playerPanelMode.value = "collapsed";
  }
};

const clearSongSelection = () => {
  selectMode.value = false;
  selectedSongs.value.clear();
};

const {
  localCollections,
  collectionNames,
  collectionPlaylists,
  showAddToCollection,
  selectedCollections,
  newCollectionName,
  showCreateCollection,
  selectedCollectionMenuName,
  loadLocalCollections,
  handleShowAddToCollectionModal,
  hideAddToCollectionModal,
  handleToggleCollectionSelection,
  addSongToCollectionFromMenu: queueSongToCollectionFromMenu,
  addToCollection,
  handleRemoveFromCollection,
  removeSingleSongFromCollection,
  handleShowCreateModal,
  handleCreateCollectionFromAddModal,
  hideCreateCollectionModal,
  handleCreateCollection,
  openCollectionMenu,
  closeCollectionMenu,
  renameCollection,
  deleteCollectionByName,
  deleteCollectionFromMenu,
} = useCollections({
  currentSongs,
  selectedSongs,
  selectedPlaylist,
  buildSongRowKey,
  inferMediaType,
  onSelectCollection: (name) => {
    const songs = collectionPlaylists.value[name] || [];
    selectedPlaylist.value = {
      name,
      songs,
    };
    selectedLibrarySourceKey.value = null;
    selectedLibraryPlaylistName.value = null;
    currentView.value = "songs";
    playbackSource.value = "playlist";
    searchPlaybackSongs.value = [];
    resetPlaylist();
  },
  onBackToPlaylists: () => {
    currentView.value = "playlists";
    selectedPlaylist.value = null;
    selectedLibrarySourceKey.value = null;
    selectedLibraryPlaylistName.value = null;
    searchQuery.value = "";
  },
  clearSongSelection,
});

const {
  requestSongLufs,
  resolveSongForPlayback,
  syncPlaybackMetadataFromBackend,
} = useLufs({
  currentSong,
  activeQueue,
  searchPlaybackSongs,
  selectedPlaylist,
  sourceGroups,
  isAndroidPlayer,
  syncAndroidQueueState,
  syncSelectedLibraryPlaylist,
  currentView,
});

watch(
  [
    volumeMode,
    manualVolume,
    manualVolumeInput,
    fixedLufs,
    fixedLufsInput,
    () => currentSong.value?.id ?? null,
    () => currentSong.value?.lufs ?? null,
    () =>
      playbackSongs.value
        .map((song) => `${song.id}:${song.lufs ?? "null"}`)
        .join("|"),
  ],
  () => {
    void syncNormalizationConfig(
      volumeMode.value,
      manualVolume.value,
      fixedLufs.value,
      calculateVolume(),
    );
  },
  { deep: true, immediate: true },
);

watch(
  () =>
    activeQueue.value
      .map((song) => `${song.id}:${song.lufs ?? "null"}`)
      .join("|"),
  () => {
    syncPlaybackMetadataFromBackend();
  },
);

watch(
  sourceGroups,
  () => {
    syncSelectedLibraryPlaylist(currentView.value, selectedPlaylist);
  },
  { deep: true },
);

watch(activeTab, () => {
  if (currentView.value !== "playlists") {
    backToPlaylists();
  }
  selectMode.value = false;
  selectedSongs.value.clear();
  collectionSelectMode.value = false;
  selectedCollectionsList.value.clear();
});

// Event handlers
const backToPlaylists = () => {
  currentView.value = "playlists";
  selectedPlaylist.value = null;
  selectedLibrarySourceKey.value = null;
  selectedLibraryPlaylistName.value = null;
  searchQuery.value = "";
};

const showSearchResults = () => {
  if (!searchQuery.value.trim()) {
    return;
  }
  currentView.value = "search";
};

const handleLyricLineClick = async (time: number) => {
  if (!currentSong.value) {
    return;
  }

  if (!audioElement.value || duration.value === 0) {
    await playSong(currentSong.value, time);
    return;
  }

  await seekToTime(time);
  if (!isPlaying.value) {
    await play();
  }
};

const handleSearch = () => {
  showSearchResults();
};

const openOnlineSearchFromQuery = () => {
  if (!trimmedSearchQuery.value) {
    return;
  }
  ensureOnlineSearchSourceExists();
  showOnlineSearchModal.value = true;
};

const handleSelectCollection = (name: string) => {
  const songs = collectionPlaylists.value[name] || [];
  selectedPlaylist.value = {
    name,
    songs,
  };
  selectedLibrarySourceKey.value = null;
  selectedLibraryPlaylistName.value = null;
  currentView.value = "songs";
  playbackSource.value = "playlist";
  searchPlaybackSongs.value = [];
  resetPlaylist();
};

const handleSelectLibraryPlaylist = (
  group: LibrarySourceGroupSummary,
  playlistName: string,
) => {
  const resolved = getLibraryPlaylist(group.sourceKey, playlistName);
  if (!resolved) {
    return;
  }

  selectedPlaylist.value = {
    name: `曲库 / ${playlistName} [${resolved.source.name}]`,
    songs: resolved.playlist.songs,
  };
  selectedLibrarySourceKey.value = resolved.source.sourceKey;
  selectedLibraryPlaylistName.value = playlistName;
  currentView.value = "songs";
  playbackSource.value = "playlist";
  searchPlaybackSongs.value = [];
  resetPlaylist();
};

const handleBackToPlaylists = () => {
  closeCollectionMenu();
  closeSongMenu();
  backToPlaylists();
  selectMode.value = false;
  selectedSongs.value.clear();
  collectionSelectMode.value = false;
  selectedCollectionsList.value.clear();
};

const handleActionBack = () => {
  if (playerPanelMode.value !== "collapsed" && !isWideLayout.value) {
    playerPanelMode.value = "collapsed";
    return;
  }
  handleBackToPlaylists();
};

const closeTopOverlay = () => {
  if (showFilterSheet.value) {
    showFilterSheet.value = false;
    return true;
  }

  if (selectedSourceMenuGroup.value) {
    closeSourceMenu();
    return true;
  }

  if (selectedCollectionMenuName.value) {
    closeCollectionMenu();
    return true;
  }

  if (selectedSongMenuSong.value) {
    closeSongMenu();
    return true;
  }

  if (showActiveQueueModal.value) {
    showActiveQueueModal.value = false;
    return true;
  }

  if (showAddDeviceModal.value) {
    showAddDeviceModal.value = false;
    return true;
  }

  if (showUploadModal.value) {
    showUploadModal.value = false;
    return true;
  }

  if (showOnlineSearchModal.value) {
    showOnlineSearchModal.value = false;
    return true;
  }

  if (showCreateCollection.value) {
    hideCreateCollectionModal();
    return true;
  }

  if (showAddToCollection.value) {
    hideAddToCollectionModal();
    return true;
  }

  if (showSettings.value) {
    hideSettingsModal();
    return true;
  }

  return false;
};

const handleAndroidBackPress = () => {
  if (closeTopOverlay()) {
    return true;
  }

  if (selectMode.value) {
    selectMode.value = false;
    selectedSongs.value.clear();
    return true;
  }

  if (collectionSelectMode.value) {
    collectionSelectMode.value = false;
    selectedCollectionsList.value.clear();
    return true;
  }

  // Match the visible "返回" button behavior on mobile.
  if (playerPanelMode.value !== "collapsed" && !isWideLayout.value) {
    playerPanelMode.value = "collapsed";
    return true;
  }

  if (currentView.value !== "playlists") {
    handleBackToPlaylists();
    return true;
  }

  return false;
};

const registerAndroidBackHandler = async () => {
  const isAndroid = await checkIsAndroid();
  if (!isAndroid) {
    return;
  }

  try {
    const [{ onBackButtonPress }, { getCurrentWindow }] = await Promise.all([
      import("@tauri-apps/api/app"),
      import("@tauri-apps/api/window"),
    ]);

    androidBackListener = await onBackButtonPress(async ({ canGoBack }) => {
      if (handleAndroidBackPress()) {
        return;
      }

      if (canGoBack) {
        window.history.back();
        return;
      }

      await getCurrentWindow().close();
    });
  } catch (error) {
    console.warn("[app] Failed to register Android back handler:", error);
  }
};

const handlePlayQueueSong = async (song: MusicInfo, index: number) => {
  await playSongAtIndex(song, index, activeQueue.value);
};

const handlePlaySong = async (song: SongInfo, index?: number) => {
  if (currentView.value === "search") {
    playbackSource.value = "search";
    searchPlaybackSongs.value = searchResults.value.slice();
  } else {
    playbackSource.value = "playlist";
    searchPlaybackSongs.value = [];
  }

  const visibleQueue =
    currentView.value === "search"
      ? searchResults.value.slice()
      : currentSongs.value.slice();

  if (index !== undefined) {
    await playSongAtIndex(song, index, visibleQueue);
  } else {
    await playSong(song, undefined, visibleQueue);
  }
};

// Handle song start event - trigger LUFS pre-caching for next song
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
  if (isAndroidPlayer.value) {
    console.log(
      "[app] Android playback backend handles next-track LUFS pre-cache",
    );
    return;
  }
  const allSongs = playbackSongs.value;

  if (!nextSongInfo) {
    console.log("[app] No next song, skipping pre-cache");
    return;
  }

  // Skip pre-caching if next song already has LUFS calculated
  const nextSong = allSongs.find(
    (s: { id: number }) => s.id === nextSongInfo.id,
  );
  if (!nextSong) {
    console.log(
      "[app] Next song metadata missing in current playback list, skipping pre-cache",
    );
    return;
  }
  if (nextSong && nextSong.lufs !== null) {
    console.log(
      "[app] Next song already has LUFS:",
      nextSong.lufs,
      ", skipping pre-cache",
    );
    return;
  }

  // Skip pre-caching in loop mode (same song) if it already has LUFS or will be re-calculated anyway
  if (playMode.value === "loop" && currentSongInfo.id === nextSongInfo.id) {
    console.log("[app] Loop mode with same song, skipping pre-cache");
    return;
  }

  console.log("[app] Pre-caching LUFS for next song ID:", nextSongInfo.id);
  await requestSongLufs(nextSong, "next");
};

// Assign the handler to the ref so useAudioPlayer can call it
handleSongStartRef.value = handleSongStart;

const handleShowSettingsModal = () => {
  closeSourceMenu();
  closeCollectionMenu();
  closeSongMenu();
  showSettings.value = true;
};

const handleManageCollections = () => {
  hideSettingsModal();
  activeTab.value = "collections";
  currentView.value = "playlists";
  selectedPlaylist.value = null;
  selectedLibrarySourceKey.value = null;
  selectedLibraryPlaylistName.value = null;
};

const handleShowActiveQueue = () => {
  showActiveQueueModal.value = true;
};

const togglePlayerPanelMode = () => {
  if (!currentSong.value) {
    return;
  }

  if (playerPanelMode.value === "collapsed") {
    playerPanelMode.value = "cover";
  } else if (playerPanelMode.value === "cover") {
    playerPanelMode.value = "lyrics";
  } else {
    playerPanelMode.value = "cover";
  }

  hasUserToggledLyric.value = true;
};

const showCoverPanel = () => {
  if (!currentSong.value) {
    return;
  }
  playerPanelMode.value = "cover";
  hasUserToggledLyric.value = true;
};

const showLyricsPanel = () => {
  if (!currentSong.value) {
    return;
  }
  playerPanelMode.value = "lyrics";
  hasUserToggledLyric.value = true;
};

const hideSettingsModal = () => {
  showSettings.value = false;
};

const handleShowLufsChange = (value: boolean) => {
  showLufs.value = value;
  setShowLufs(value);
};

const handleAndroidTimerComplete = async () => {
  if (getTimerExitAppOnAndroid()) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("exit_android_app");
      return;
    } catch (error) {
      console.error("Failed to exit Android app from timer:", error);
    }
  }

  await refreshAndroidSession();
};

const handleStartTimer = async () => {
  if (isAndroidPlayer.value) {
    await setTimedPause(timerMinutes.value * 60 * 1000);
  }
  startTimer();
};

const handleSetTimerPreset = async (minutes: number) => {
  timerMinutes.value = minutes;
  timerMinutesInput.value = minutes;
  await handleStartTimer();
};

const handleCancelTimer = async () => {
  cancelTimer();
  if (isAndroidPlayer.value) {
    await setTimedPause(0);
  }
};

const handleDirectoryChanged = () => {
  void refreshSourceGroups();
  void refreshAndroidSession();
};

const handleDatabaseUpdated = async () => {
  await refreshSourceGroups();
  await refreshAndroidSession();
};

const handleDatabaseUpdateStart = () => {
  isScanning.value = true;
};

const handleDatabaseUpdateEnd = () => {
  isScanning.value = false;
};

const handleUploadComplete = async () => {
  showUploadModal.value = false;
  await refreshSourceGroups();
};

const handleSourcesUpdated = async () => {
  await refreshSourceGroups();
  ensureOnlineSearchSourceExists();
};

const handleDeviceConnected = async (apiBase: string) => {
  await refreshSingleSource(apiBase);
  ensureOnlineSearchSourceExists();

  const connectedSource = sourceGroups.value.find(
    (group) => group.apiBase === apiBase,
  );
  if (!connectedSource?.capabilities.canUseForOnlineSearch) {
    return;
  }

  const shouldUseForOnlineSearch = window.confirm(
    `设备 “${connectedSource.name}” 已连接。是否将它设为默认在线搜索来源？`,
  );
  if (shouldUseForOnlineSearch) {
    setOnlineSearchSource(apiBase);
  }
};

const handleOnlineDownloadComplete = async () => {
  await refreshSourceGroups();
  await refreshAndroidSession();
};

const handlePreviewTrack = async (song: MusicInfo) => {
  playbackSource.value = "search";
  searchPlaybackSongs.value = [song];
  await playSongAtIndex(song, 0, [song]);
};

const handleAddDevice = () => {
  closeSourceMenu();
  closeCollectionMenu();
  closeSongMenu();
  void refreshSourceGroups();
  showAddDeviceModal.value = true;
};

const handleOpenSourceMenu = (group: LibrarySourceGroupSummary) => {
  selectedSourceMenuGroup.value =
    sourceGroups.value.find((item) => item.sourceKey === group.sourceKey) ||
    null;
};

const closeSourceMenu = () => {
  selectedSourceMenuGroup.value = null;
};

const handleRetrySourceConnection = async (apiBase: string) => {
  await retrySourceConnection(apiBase);
};

const handleShowSourceDetails = (group: LibrarySourceGroup) => {
  closeSourceMenu();
  showSourceDetails(group);
};

const handleSetOnlineSearchSourceFromMenu = (group: LibrarySourceGroup) => {
  closeSourceMenu();
  setOnlineSearchSourceFromMenu(group);
};

const handleDeleteSource = (group: LibrarySourceGroup) => {
  closeSourceMenu();
  const deleted = deleteSource(group);
  if (!deleted) {
    return;
  }

  if (selectedLibrarySourceKey.value === group.sourceKey) {
    handleBackToPlaylists();
  }
};

const handleOpenCollectionMenu = (collectionName: string) => {
  selectedSongMenuSong.value = null;
  openCollectionMenu(collectionName);
};

const openSongMenu = (song: SongInfo) => {
  selectedCollectionMenuName.value = null;
  selectedSongMenuSong.value = song as MusicInfo;
};

const closeSongMenu = () => {
  selectedSongMenuSong.value = null;
};

const buildQueueWithSongInserted = (song: MusicInfo, insertIndex: number) => {
  const baseQueue =
    activeQueue.value.length > 0
      ? activeQueue.value.slice()
      : playbackSongs.value.slice();
  const songIdentity = getSongMenuIdentity(song);
  const existingQueue = baseQueue.filter(
    (item) => getSongMenuIdentity(item) !== songIdentity,
  );

  if (existingQueue.length === 0) {
    return [song];
  }

  const safeInsertIndex = Math.max(
    0,
    Math.min(insertIndex, existingQueue.length),
  );
  existingQueue.splice(safeInsertIndex, 0, song);
  return existingQueue;
};

const updateQueueState = async (nextQueue: MusicInfo[]) => {
  activeQueue.value = nextQueue;
  if (isAndroidPlayer.value) {
    await syncAndroidQueueState();
  }
};

const queueSongNextFromMenu = async () => {
  const song = selectedSongMenuSong.value;
  if (!song) {
    return;
  }

  if (!currentSong.value) {
    closeSongMenu();
    await handlePlaySong(song);
    return;
  }

  const currentQueue =
    activeQueue.value.length > 0 ? activeQueue.value : playbackSongs.value;
  const currentIndex = currentQueue.findIndex(
    (item) =>
      getSongMenuIdentity(item) ===
      getSongMenuIdentity(currentSong.value as MusicInfo),
  );
  const nextQueue = buildQueueWithSongInserted(
    song,
    currentIndex >= 0 ? currentIndex + 1 : 1,
  );
  await updateQueueState(nextQueue);
  closeSongMenu();
};

const addSongToQueueFromMenu = async () => {
  const song = selectedSongMenuSong.value;
  if (!song) {
    return;
  }

  const nextQueue = buildQueueWithSongInserted(song, activeQueue.value.length);
  await updateQueueState(nextQueue);
  closeSongMenu();
};

const addSongToCollectionFromMenu = () => {
  const song = selectedSongMenuSong.value;
  if (!song) {
    return;
  }

  queueSongToCollectionFromMenu(song);
  closeSongMenu();
};

const removeSongFromCollectionFromMenu = () => {
  const song = selectedSongMenuSong.value;
  if (!song) {
    return;
  }

  removeSingleSongFromCollection(song);
  closeSongMenu();
};

const handleCoverLoadError = (song: MusicInfo | null) => {
  const coverUrl = resolveSongCoverUrl(song);
  if (!coverUrl) {
    return;
  }

  failedCoverUrls.value = new Set(failedCoverUrls.value).add(coverUrl);
};

const openUploadForSource = (group: LibrarySourceGroup) => {
  uploadTargetApiBase.value = group.apiBase;
  showUploadModal.value = true;
  closeSourceMenu();
};

const handleUpdateSourceDatabase = async (group: LibrarySourceGroup) => {
  closeSourceMenu();
  isScanning.value = true;
  try {
    await updateSourceDatabase(group);
  } catch (error) {
    console.error("Failed to update source database:", error);
    alert(`更新失败: ${error}`);
  } finally {
    isScanning.value = false;
  }
};

const handleChangeSourceDirectory = async (group: LibrarySourceGroup) => {
  closeSourceMenu();
  try {
    await changeSourceDirectory(group);
  } catch (error) {
    console.error("Failed to change source directory:", error);
    alert(`更改目录失败: ${error}`);
  }
};

const handleSongCollectionAction = (song: SongInfo) => {
  openSongMenu(song);
};

const handleDeleteSelectedCollections = async () => {
  if (selectedCollectionsList.value.size === 0) {
    alert("请选择要删除的收藏夹");
    return;
  }

  if (
    !confirm(
      `确定要删除选中的 ${selectedCollectionsList.value.size} 个收藏夹吗？`,
    )
  ) {
    return;
  }

  let deletedCount = 0;
  for (const collectionName of selectedCollectionsList.value) {
    if (deleteCollectionByName(collectionName)) {
      deletedCount += 1;
    }
  }

  alert(`已删除 ${deletedCount} 个收藏夹`);
  collectionSelectMode.value = false;
  selectedCollectionsList.value.clear();
};

// Initialize
onMounted(async () => {
  // Startup scan flow: docs/startup-scan.md
  await triggerDatabaseUpdate(isScanning);

  isAndroidRuntime.value = await checkIsAndroid();
  loadLocalCollections();
  void refreshDiscoveryState();
  await refreshSourceGroups();
  await initAudio();
  await registerAndroidBackHandler();
  updateLayoutMode();
  window.addEventListener("resize", updateLayoutMode);
});

onBeforeUnmount(() => {
  if (typeof window !== "undefined") {
    window.removeEventListener("resize", updateLayoutMode);
  }
  if (androidBackListener) {
    void androidBackListener.unregister();
    androidBackListener = null;
  }
});
</script>

<style scoped>
.music-player {
  min-height: 100vh;
  display: flex;
  align-items: stretch;
  justify-content: center;
  background-color: #f5f5f5;
  color: #333;
  font-family: "Segoe UI", Tahoma, Geneva, Verdana, sans-serif;
  overflow: hidden;
}

.app-window {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background-color: #fff;
  overflow: hidden;
}

.top-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  background-color: #fff;
  border-bottom: 1px solid #eee;
}

.icon-btn {
  width: 36px;
  height: 36px;
  border: none;
  background: #f0f0f0;
  border-radius: 8px;
  cursor: pointer;
  font-size: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #333;
}

.icon-btn:hover {
  background-color: #e6e6e6;
}

.action-bar {
  display: flex;
  align-items: center;
  padding: 0 12px;
  background-color: #fff;
  border-bottom: 1px solid #eee;
}

.action-spacer {
  flex: 1;
}

.action-btn {
  background: none;
  border: none;
  color: #1db954;
  font-size: 14px;
  cursor: pointer;
  padding: 12px;
}

.icon-action-btn {
  font-size: 22px;
  line-height: 1;
  color: #31414f;
}

.scanning-message {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 120px;
  font-size: 18px;
  color: #888;
}

.main-area {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: #fff;
}

.main-area.wide-layout {
  flex-direction: row;
}

.list-panel {
  flex: 1;
  min-height: 0;
  background-color: #fff;
}

@media (min-width: 900px) and (min-aspect-ratio: 1/1) {
  .music-player {
    align-items: stretch;
  }

  .app-window {
    width: 100%;
    height: 100vh;
    border-radius: 0;
    border: none;
    box-shadow: none;
  }
}
</style>
