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
            @retry-source-connection="retrySourceConnection"
            @toggle-collection-select-mode="toggleCollectionSelectMode"
            @toggle-collection-selection="toggleCollectionSelection"
            @select-collection="handleSelectCollection"
            @show-create-modal="handleShowCreateModal"
            @delete-selected-collections="handleDeleteSelectedCollections"
            @open-collection-menu="openCollectionMenu"
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
      @sources-updated="refreshSourceGroups"
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
      @refresh-source="updateSourceDatabase"
      @upload-to-source="openUploadForSource"
      @open-online-search-for-source="openOnlineSearchForSource"
      @change-source-directory="changeSourceDirectory"
      @retry-source-connection="retrySourceConnection"
      @show-source-details="showSourceDetails"
      @delete-source="deleteSource"
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
import { type LibrarySourceGroupSummary } from "@/components/LibrarySourceListView.vue";
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
import { useSelection } from "@/composables/useSelection";
import { useTimer } from "@/composables/useTimer";
import { useVolume } from "@/composables/useVolume";
import { useLyrics } from "@/composables/useLyrics";
import {
  LOCALHOST_API_BASE,
  getLocalApiBase,
  resolveSourceApiBase,
} from "@/utils/api";
import {
  refreshDiscoveredDevices,
  refreshStoredManualDevices,
} from "@/utils/discovery";
import {
  getLocalCollections,
  getManualDevices,
  getShowLufs,
  getTimerExitAppOnAndroid,
  setLocalCollections,
  setManualDevices,
  setShowLufs,
  type StoredLocalCollection,
} from "@/utils/storage";
import { checkIsAndroid, isLocalhostApiBase } from "@/utils/platform";
import { loadItemsIncrementally, upsertSortedItem } from "@/utils/sourceGroups";

type MainView = "playlists" | "songs" | "search";
type MainTab = "library" | "collections";
type PlayerPanelMode = "collapsed" | "cover" | "lyrics";

interface PlaylistSelection {
  name: string;
  songs: MusicInfo[];
}

interface LibraryPlaylistGroup {
  name: string;
  songs: MusicInfo[];
}

interface SourceCapabilities {
  canRefresh: boolean;
  canUpload: boolean;
  canChangeDirectory: boolean;
  canOnlineDownload: boolean;
  canRetryConnection: boolean;
  canShowSourceDetails: boolean;
  canDeleteSource: boolean;
}

interface LibrarySourceGroup {
  apiBase: string;
  sourceKey: string;
  name: string;
  isLoading: boolean;
  isOnline: boolean;
  playlists: LibraryPlaylistGroup[];
  capabilities: SourceCapabilities;
}

const searchQuery = ref("");
const currentView = ref<MainView>("playlists");
const activeTab = ref<MainTab>("library");
const selectedPlaylist = ref<PlaylistSelection | null>(null);
const selectedLibrarySourceKey = ref<string | null>(null);
const selectedLibraryPlaylistName = ref<string | null>(null);
const localCollections = ref<StoredLocalCollection[]>([]);
const sourceGroups = ref<LibrarySourceGroup[]>([]);
const showFilterSheet = ref(false);
const appliedSourceFilterKey = ref("all");
const draftSourceFilterKey = ref("all");
const appliedMediaTypes = ref<Array<"audio" | "video">>(["audio", "video"]);
const draftMediaTypes = ref<Array<"audio" | "video">>(["audio", "video"]);

const playbackSource = ref<"playlist" | "search">("playlist");
const searchPlaybackSongs = ref<SongInfo[]>([]);
const currentSongs = computed<MusicInfo[]>(
  () => selectedPlaylist.value?.songs || [],
);
const collectionNames = computed(() =>
  localCollections.value.map((collection) => collection.name),
);
const collectionPlaylists = computed<Record<string, SongInfo[]>>(() =>
  Object.fromEntries(
    localCollections.value.map((collection) => [
      collection.name,
      collection.songs,
    ]),
  ),
);

const filteredSourceGroups = computed<LibrarySourceGroup[]>(() =>
  sourceGroups.value
    .filter(
      (group) =>
        appliedSourceFilterKey.value === "all" ||
        group.sourceKey === appliedSourceFilterKey.value,
    )
    .map((group) => ({
      ...group,
      playlists: group.playlists
        .map((playlist) => ({
          ...playlist,
          songs: playlist.songs.filter((song) =>
            appliedMediaTypes.value.includes(song.mediaType || "audio"),
          ),
        }))
        .filter((playlist) => playlist.songs.length > 0 || !group.isOnline),
    }))
    .filter((group) => group.playlists.length > 0 || !group.isOnline),
);

const allLibrarySongs = computed<SongInfo[]>(() =>
  filteredSourceGroups.value.flatMap((group) =>
    group.playlists.flatMap((playlist) => playlist.songs),
  ),
);

const libraryGroupSummaries = computed<LibrarySourceGroupSummary[]>(() =>
  filteredSourceGroups.value.map((group) => ({
    sourceKey: group.sourceKey,
    name: group.name,
    isLoading: group.isLoading,
    isOnline: group.isOnline,
    playlists: group.playlists.map((playlist) => ({
      name: playlist.name,
      songCount: playlist.songs.length,
    })),
  })),
);

const searchResults = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  if (!query) {
    return [];
  }

  return allLibrarySongs.value.filter((song) =>
    song.name.toLowerCase().includes(query),
  );
});

const filterSources = computed(() =>
  sourceGroups.value.map((group) => ({
    sourceKey: group.sourceKey,
    name: group.name,
  })),
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
const showAddToCollection = ref(false);
const selectedCollections = ref<number[]>([]);
const pendingSongsForCollection = ref<MusicInfo[]>([]);
const newCollectionName = ref("");
const showCreateCollection = ref(false);
const showUploadModal = ref(false);
const showOnlineSearchModal = ref(false);
const showActiveQueueModal = ref(false);
const playerPanelMode = ref<PlayerPanelMode>("collapsed");
const isWideLayout = ref(false);
const hasUserToggledLyric = ref(false);
const isScanning = ref(false);
const failedCoverUrls = ref<Set<string>>(new Set());
const selectedSourceMenuGroup = ref<LibrarySourceGroup | null>(null);
const selectedCollectionMenuName = ref<string | null>(null);
const selectedSongMenuSong = ref<MusicInfo | null>(null);
const uploadTargetApiBase = ref<string>(LOCALHOST_API_BASE);
const isAndroidRuntime = ref(false);
let androidBackListener: { unregister(): Promise<void> } | null = null;

const SOURCE_REQUEST_TIMEOUT_MS = 3000;
const onlineSearchApiBase = ref<string>(LOCALHOST_API_BASE);
let sourceRefreshToken = 0;

const buildSongApiUrl = (apiBase: string, suffix: string): string => {
  return `${apiBase}${suffix}`;
};

const buildSongRowKey = (song: {
  id: number;
  name: string;
  source_key?: string | null;
}): string => {
  return `${song.source_key || "local"}:${song.id}:${song.name}`;
};

const resolveSongCoverUrl = (song: MusicInfo | null): string | null => {
  if (!song) {
    return null;
  }

  const coverUrl =
    song.cover_url ||
    buildSongApiUrl(
      resolveSourceApiBase(song.source_key),
      `/music/id/${song.id}/cover`,
    );
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

const inferMediaType = (song: {
  name: string;
  path: string;
}): "audio" | "video" => {
  const candidate = `${song.name} ${song.path}`.toLowerCase();
  const audioExtensions = [
    ".mp3",
    ".flac",
    ".wav",
    ".ogg",
    ".m4a",
    ".aac",
    ".opus",
  ];
  return audioExtensions.some((extension) => candidate.includes(extension))
    ? "audio"
    : "video";
};

const buildSourceLabel = (apiBase: string): string => {
  const manualMatch = getManualDevices().find(
    (device) => device.api_url === apiBase,
  );
  if (manualMatch?.device_name?.trim()) {
    return manualMatch.device_name.trim();
  }

  try {
    const parsed = new URL(apiBase);
    return parsed.hostname === "localhost" || parsed.hostname === "127.0.0.1"
      ? "This Device"
      : parsed.hostname;
  } catch {
    return apiBase;
  }
};

const normalizeSourceSong = (
  apiBase: string,
  sourceLabel: string,
  song: MusicInfo,
): MusicInfo => ({
  ...song,
  stream_url:
    song.stream_url || buildSongApiUrl(apiBase, `/music/id/${song.id}`),
  cover_url:
    (song as MusicInfo).cover_url ||
    buildSongApiUrl(apiBase, `/music/id/${song.id}/cover`),
  source_key: apiBase,
  sourceLabel,
  rowKey: `${apiBase}:${song.id}:${song.name}`,
  mediaType: song.mediaType || inferMediaType(song),
});

const buildPlaylistRequestUrl = (apiBase: string): string => {
  const shouldRequestRawPlaybackPath =
    isAndroidRuntime.value && isLocalhostApiBase(apiBase);
  return shouldRequestRawPlaybackPath
    ? buildSongApiUrl(apiBase, "/playlists?stream=content")
    : buildSongApiUrl(apiBase, "/playlists");
};

const syncLocalCollections = () => {
  setLocalCollections(localCollections.value);
};

const loadLocalCollections = () => {
  localCollections.value = getLocalCollections().map((collection) => ({
    ...collection,
    songs: collection.songs.map((song) => ({
      ...song,
      rowKey: buildSongRowKey(song),
      mediaType: song.mediaType || inferMediaType(song),
    })),
  }));
};

const getSourceApiBases = (): string[] => {
  const manual = getManualDevices().map((device) => device.api_url);
  return Array.from(new Set([LOCALHOST_API_BASE, ...manual]));
};

const sortSourceGroups = (groups: LibrarySourceGroup[]): LibrarySourceGroup[] =>
  [...groups].sort((left, right) => {
    if (left.isLoading && !right.isLoading) return -1;
    if (!left.isLoading && right.isLoading) return 1;
    if (
      isLocalhostApiBase(left.apiBase) &&
      !isLocalhostApiBase(right.apiBase)
    ) {
      return -1;
    }
    if (
      isLocalhostApiBase(right.apiBase) &&
      !isLocalhostApiBase(left.apiBase)
    ) {
      return 1;
    }
    return left.name.localeCompare(right.name);
  });

const buildLoadingSourceGroup = (apiBase: string): LibrarySourceGroup => ({
  sourceKey: apiBase,
  apiBase,
  name: buildSourceLabel(apiBase),
  isLoading: true,
  isOnline: false,
  playlists: [],
  capabilities: {
    canRefresh: false,
    canUpload: false,
    canChangeDirectory: false,
    canOnlineDownload: false,
    canRetryConnection: false,
    canShowSourceDetails: true,
    canDeleteSource: !isLocalhostApiBase(apiBase),
  },
});

const fetchWithTimeout = async (
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> => {
  const controller = new AbortController();
  const timeoutId = window.setTimeout(() => {
    controller.abort();
  }, SOURCE_REQUEST_TIMEOUT_MS);

  try {
    return await fetch(input, {
      ...init,
      signal: controller.signal,
    });
  } finally {
    window.clearTimeout(timeoutId);
  }
};

const fetchSourceGroup = async (
  apiBase: string,
): Promise<LibrarySourceGroup> => {
  const fallbackName = buildSourceLabel(apiBase);

  try {
    const [
      selfResponse,
      playlistsResponse,
      directoryTreeResponse,
      musicDirectoryResponse,
    ] = await Promise.all([
      fetchWithTimeout(buildSongApiUrl(apiBase, "/discovery/self"), {
        cache: "no-store",
      }),
      fetchWithTimeout(buildPlaylistRequestUrl(apiBase), {
        cache: "no-store",
      }),
      fetchWithTimeout(buildSongApiUrl(apiBase, "/files/directory-tree"), {
        cache: "no-store",
      }).catch(() => null),
      fetchWithTimeout(buildSongApiUrl(apiBase, "/settings/music-directory"), {
        cache: "no-store",
      }).catch(() => null),
    ]);

    if (!selfResponse.ok || !playlistsResponse.ok) {
      throw new Error("source unavailable");
    }

    const selfData = await selfResponse.json();
    const playlistMap = (await playlistsResponse.json()) as Record<
      string,
      SongInfo[]
    >;
    const sourceLabel = selfData.device_name || fallbackName;
    const canUpload = !!directoryTreeResponse?.ok;
    const canChangeDirectory = !!musicDirectoryResponse?.ok;
    const isLocalDownloadTarget = apiBase === LOCALHOST_API_BASE;

    const playlists = Object.entries(playlistMap).map(([name, songs]) => ({
      name,
      songs: songs.map((song) =>
        normalizeSourceSong(apiBase, sourceLabel, song),
      ),
    }));

    return {
      sourceKey: apiBase,
      apiBase,
      name: sourceLabel,
      isLoading: false,
      isOnline: true,
      playlists,
      capabilities: {
        canRefresh: true,
        canUpload,
        canChangeDirectory,
        canOnlineDownload: isLocalDownloadTarget && canUpload,
        canRetryConnection: false,
        canShowSourceDetails: true,
        canDeleteSource: !isLocalhostApiBase(apiBase),
      },
    };
  } catch (error) {
    console.warn("Failed to load source group:", apiBase, error);
    return {
      sourceKey: apiBase,
      apiBase,
      name: fallbackName,
      isLoading: false,
      isOnline: false,
      playlists: [],
      capabilities: {
        canRefresh: false,
        canUpload: false,
        canChangeDirectory: false,
        canOnlineDownload: false,
        canRetryConnection: true,
        canShowSourceDetails: true,
        canDeleteSource: !isLocalhostApiBase(apiBase),
      },
    };
  }
};

const syncSelectedLibraryPlaylist = () => {
  if (
    currentView.value !== "songs" ||
    !selectedLibrarySourceKey.value ||
    !selectedLibraryPlaylistName.value
  ) {
    return;
  }

  const source = sourceGroups.value.find(
    (group) => group.sourceKey === selectedLibrarySourceKey.value,
  );
  const playlist = source?.playlists.find(
    (item) => item.name === selectedLibraryPlaylistName.value,
  );
  if (!source || !playlist) {
    return;
  }

  selectedPlaylist.value = {
    name: `曲库 / ${playlist.name} [${source.name}]`,
    songs: playlist.songs,
  };
};

const refreshSourceGroups = async () => {
  const apiBases = getSourceApiBases();
  const refreshToken = sourceRefreshToken + 1;
  sourceRefreshToken = refreshToken;

  await loadItemsIncrementally({
    keys: apiBases,
    buildLoadingItem: buildLoadingSourceGroup,
    fetchItem: fetchSourceGroup,
    getItemKey: (group) => group.sourceKey,
    sortItems: sortSourceGroups,
    isActive: () => sourceRefreshToken === refreshToken,
    onUpdate: (groups) => {
      sourceGroups.value = groups;
      syncSelectedLibraryPlaylist();
    },
  });
};

const triggerDatabaseUpdate = async () => {
  try {
    isScanning.value = true;
    console.log("[app] onMounted: triggering startup database scan");
    const response = await fetch(
      `${getLocalApiBase()}/database/update?startup=true`,
      { method: "POST" },
    );
    if (!response.ok) {
      const errorText = await response.text();
      console.warn(
        "[app] onMounted: database update failed:",
        response.status,
        errorText,
      );
      return;
    }
    const result = await response.json();
    if (!result.success) {
      console.warn(
        "[app] onMounted: database update returned failure:",
        result.message,
      );
    } else {
      console.log("[app] onMounted: database update completed");
    }
  } catch (error) {
    console.error("[app] onMounted: database update error:", error);
  } finally {
    isScanning.value = false;
  }
};

const refreshDiscoveryState = async () => {
  const previousManualDevices = getManualDevices();

  try {
    const discoveredDevices = await refreshDiscoveredDevices();
    const updatedManualDevices =
      await refreshStoredManualDevices(discoveredDevices);

    const previousByDeviceId = new Map(
      previousManualDevices
        .filter((device) => device.device_id)
        .map((device) => [device.device_id!, device.api_url]),
    );

    const previousByApiUrl = new Map(
      previousManualDevices.map((device) => [device.api_url, device.api_url]),
    );

    for (const device of updatedManualDevices) {
      const previousApiBase = device.device_id
        ? previousByDeviceId.get(device.device_id)
        : previousByApiUrl.get(device.api_url);

      if (!previousApiBase || previousApiBase === device.api_url) {
        continue;
      }

      sourceGroups.value = sourceGroups.value.filter(
        (group) => group.sourceKey !== previousApiBase,
      );

      if (selectedLibrarySourceKey.value === previousApiBase) {
        selectedLibrarySourceKey.value = device.api_url;
      }

      await refreshSingleSource(device.api_url);
    }
  } catch (error) {
    console.warn("[app] startup discovery refresh failed:", error);
  }
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

const patchSongLufsInList = (
  songs: SongInfo[],
  songId: number,
  lufs: number,
): SongInfo[] => {
  let changed = false;
  const updatedSongs = songs.map((song) => {
    if (song.id !== songId || song.lufs === lufs) {
      return song;
    }
    changed = true;
    return {
      ...song,
      lufs,
    };
  });

  return changed ? updatedSongs : songs;
};

const patchSongLufs = (songId: number, lufs: number) => {
  if (currentSong.value?.id === songId && currentSong.value.lufs !== lufs) {
    currentSong.value = {
      ...currentSong.value,
      lufs,
    };
  }

  activeQueue.value = patchSongLufsInList(activeQueue.value, songId, lufs);
  searchPlaybackSongs.value = patchSongLufsInList(
    searchPlaybackSongs.value,
    songId,
    lufs,
  );

  if (selectedPlaylist.value) {
    selectedPlaylist.value = {
      ...selectedPlaylist.value,
      songs: patchSongLufsInList(selectedPlaylist.value.songs, songId, lufs),
    };
  }

  let groupsChanged = false;
  const nextGroups = sourceGroups.value.map((group) => {
    let groupChanged = false;
    const playlists = group.playlists.map((playlist) => {
      const updatedSongs = patchSongLufsInList(playlist.songs, songId, lufs);
      if (updatedSongs !== playlist.songs) {
        groupChanged = true;
      }
      return groupChanged
        ? {
            ...playlist,
            songs: updatedSongs,
          }
        : playlist;
    });

    if (!groupChanged) {
      return group;
    }

    groupsChanged = true;
    return {
      ...group,
      playlists,
    };
  });
  if (groupsChanged) {
    sourceGroups.value = nextGroups;
    syncSelectedLibraryPlaylist();
  }
};

interface PrecacheLufsResult {
  success: boolean;
  lufs: number | null;
  cached?: boolean;
  error?: string;
}

const LUFS_POLL_DELAY_MS = 1000;
const LUFS_POLL_MAX_ATTEMPTS = 8;
const pendingLufsPolls = new Set<number>();

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const pollSongLufs = async (
  songId: number,
  sourceKey: string | null | undefined,
  context: "current" | "next",
) => {
  if (pendingLufsPolls.has(songId)) {
    console.log(
      `[app] LUFS ${context} poll already in flight for song ID:`,
      songId,
    );
    return;
  }

  pendingLufsPolls.add(songId);

  try {
    for (let attempt = 1; attempt <= LUFS_POLL_MAX_ATTEMPTS; attempt++) {
      await wait(LUFS_POLL_DELAY_MS);

      try {
        console.log(
          `[app] LUFS ${context} poll request attempt ${attempt} for song ID:`,
          songId,
        );
        const response = await fetch(
          `${resolveSourceApiBase(sourceKey)}/music/${songId}/precache-lufs`,
          {
            method: "POST",
          },
        );

        if (!response.ok) {
          console.warn(`[app] LUFS ${context} poll failed:`, response.status);
          return;
        }

        const result: PrecacheLufsResult = await response.json();
        if (result.success && result.lufs !== null) {
          console.log(
            `[app] LUFS ${context} resolved from poll attempt ${attempt}:`,
            result.lufs,
          );
          patchSongLufs(songId, result.lufs);
          if (isAndroidPlayer.value) {
            await syncAndroidQueueState();
          }
          return;
        }

        if (result.cached !== false) {
          return;
        }
      } catch (error) {
        console.error(
          `[app] LUFS ${context} poll error on attempt ${attempt}:`,
          error,
        );
        return;
      }
    }
  } finally {
    pendingLufsPolls.delete(songId);
  }
};

const requestSongLufs = async (
  song: MusicInfo,
  context: "current" | "next",
): Promise<MusicInfo> => {
  if (song.lufs !== null) {
    console.log(
      `[app] LUFS ${context} already cached for song ID:`,
      song.id,
      "value:",
      song.lufs,
    );
    return song;
  }

  try {
    console.log(`[app] LUFS ${context} request for song ID:`, song.id);
    const response = await fetch(
      `${resolveSourceApiBase(song.source_key)}/music/${song.id}/precache-lufs`,
      {
        method: "POST",
      },
    );

    if (!response.ok) {
      console.warn(`[app] LUFS ${context} pre-cache failed:`, response.status);
      return song;
    }

    const result: PrecacheLufsResult = await response.json();
    if (result.success && result.lufs !== null) {
      console.log(`[app] LUFS ${context} resolved immediately:`, result.lufs);
      patchSongLufs(song.id, result.lufs);
      if (context === "next" && isAndroidPlayer.value) {
        await syncAndroidQueueState();
      }
      return {
        ...song,
        lufs: result.lufs,
      };
    }

    if (result.success && result.cached === false) {
      console.log(`[app] LUFS ${context} started in background (non-blocking)`);
      void pollSongLufs(song.id, song.source_key, context);
    }
  } catch (error) {
    console.error(`[app] LUFS ${context} pre-cache error:`, error);
  }

  return song;
};

const resolveSongForPlayback = async (song: SongInfo): Promise<SongInfo> => {
  if (isAndroidPlayer.value) {
    return song;
  }
  return await requestSongLufs(song, "current");
};

const syncPlaybackMetadataFromBackend = () => {
  if (!isAndroidPlayer.value || activeQueue.value.length === 0) {
    return;
  }

  for (const song of activeQueue.value) {
    if (song.lufs !== null) {
      patchSongLufs(song.id, song.lufs);
    }
  }
};

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

const trimmedSearchQuery = computed(() => searchQuery.value.trim());

const openOnlineSearchFromQuery = () => {
  if (!trimmedSearchQuery.value) {
    return;
  }
  onlineSearchApiBase.value = LOCALHOST_API_BASE;
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
  const source = sourceGroups.value.find(
    (item) => item.sourceKey === group.sourceKey,
  );
  const playlist = source?.playlists.find((item) => item.name === playlistName);
  if (!source || !playlist) {
    return;
  }

  selectedPlaylist.value = {
    name: `曲库 / ${playlistName} [${source.name}]`,
    songs: playlist.songs,
  };
  selectedLibrarySourceKey.value = source.sourceKey;
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
  if (index !== undefined) {
    await playSongAtIndex(song, index);
  } else {
    await playSong(song);
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

const handleOnlineDownloadComplete = async () => {
  await refreshSourceGroups();
  await refreshAndroidSession();
};

const handlePreviewTrack = async (song: MusicInfo) => {
  playbackSource.value = "search";
  searchPlaybackSongs.value = [song];
  await playSongAtIndex(song, 0, [song]);
};

const openFilterSheet = () => {
  draftSourceFilterKey.value = appliedSourceFilterKey.value;
  draftMediaTypes.value = [...appliedMediaTypes.value];
  showFilterSheet.value = true;
};

const toggleDraftMediaType = (
  mediaType: "audio" | "video",
  enabled: boolean,
) => {
  const next = new Set(draftMediaTypes.value);
  if (enabled) {
    next.add(mediaType);
  } else if (next.size > 1) {
    next.delete(mediaType);
  }
  draftMediaTypes.value = Array.from(next) as Array<"audio" | "video">;
};

const applyLibraryFilter = () => {
  appliedSourceFilterKey.value = draftSourceFilterKey.value;
  appliedMediaTypes.value =
    draftMediaTypes.value.length > 0 ? [...draftMediaTypes.value] : ["audio"];
  showFilterSheet.value = false;
};

const resetLibraryFilter = () => {
  draftSourceFilterKey.value = "all";
  draftMediaTypes.value = ["audio", "video"];
  appliedSourceFilterKey.value = "all";
  appliedMediaTypes.value = ["audio", "video"];
  showFilterSheet.value = false;
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

const openCollectionMenu = (collectionName: string) => {
  if (!collectionName) {
    return;
  }
  selectedSongMenuSong.value = null;
  selectedCollectionMenuName.value = collectionName;
};

const closeCollectionMenu = () => {
  selectedCollectionMenuName.value = null;
};

const renameCollection = () => {
  const currentName = selectedCollectionMenuName.value;
  if (!currentName) {
    return;
  }

  const nextName = prompt("请输入新的收藏夹名称:", currentName);
  if (nextName === null) {
    return;
  }

  const trimmedName = nextName.trim();
  if (!trimmedName) {
    alert("请输入收藏夹名称");
    return;
  }

  if (
    trimmedName !== currentName &&
    localCollections.value.some((collection) => collection.name === trimmedName)
  ) {
    alert("已存在同名收藏夹");
    return;
  }

  localCollections.value = localCollections.value.map((collection) =>
    collection.name === currentName
      ? {
          ...collection,
          name: trimmedName,
        }
      : collection,
  );
  syncLocalCollections();
  if (selectedPlaylist.value?.name === currentName) {
    selectedPlaylist.value = {
      ...selectedPlaylist.value,
      name: trimmedName,
    };
  }
  selectedCollectionMenuName.value = trimmedName;
};

const deleteCollectionByName = (collectionName: string) => {
  const before = localCollections.value.length;
  localCollections.value = localCollections.value.filter(
    (collection) => collection.name !== collectionName,
  );
  if (localCollections.value.length === before) {
    return false;
  }

  syncLocalCollections();
  if (selectedPlaylist.value?.name === collectionName) {
    handleBackToPlaylists();
  }
  return true;
};

const deleteCollectionFromMenu = () => {
  const collectionName = selectedCollectionMenuName.value;
  if (!collectionName) {
    return;
  }

  if (!confirm(`确定要删除收藏夹 “${collectionName}” 吗？`)) {
    return;
  }

  if (deleteCollectionByName(collectionName)) {
    closeCollectionMenu();
  }
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

  selectedCollections.value = [];
  pendingSongsForCollection.value = [song];
  showAddToCollection.value = true;
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

const refreshSingleSource = async (apiBase: string) => {
  sourceGroups.value = upsertSortedItem(
    sourceGroups.value,
    buildLoadingSourceGroup(apiBase),
    (group) => group.sourceKey,
    sortSourceGroups,
  );
  syncSelectedLibraryPlaylist();

  const updated = await fetchSourceGroup(apiBase);
  sourceGroups.value = upsertSortedItem(
    sourceGroups.value,
    updated,
    (group) => group.sourceKey,
    sortSourceGroups,
  );
  syncSelectedLibraryPlaylist();
};

const retrySourceConnection = async (apiBase: string) => {
  await refreshSingleSource(apiBase);
};

const showSourceDetails = (group: LibrarySourceGroup) => {
  const lines = [
    `Name: ${group.name}`,
    `API: ${group.apiBase}`,
    `Status: ${group.isOnline ? "Online" : "Offline"}`,
    `Playlists: ${group.playlists.length}`,
  ];
  closeSourceMenu();
  alert(lines.join("\n"));
};

const deleteSource = (group: LibrarySourceGroup) => {
  closeSourceMenu();

  if (isLocalhostApiBase(group.apiBase)) {
    alert("本机来源不能删除");
    return;
  }

  const confirmed = window.confirm(`删除来源 “${group.name}” 吗？`);
  if (!confirmed) {
    return;
  }

  setManualDevices(
    getManualDevices().filter((device) => device.api_url !== group.apiBase),
  );

  sourceGroups.value = sourceGroups.value.filter(
    (item) => item.sourceKey !== group.sourceKey,
  );
  syncSelectedLibraryPlaylist();

  if (selectedLibrarySourceKey.value === group.sourceKey) {
    handleBackToPlaylists();
  }
};

const handleCoverLoadError = (song: MusicInfo | null) => {
  const coverUrl = resolveSongCoverUrl(song);
  if (!coverUrl) {
    return;
  }

  failedCoverUrls.value = new Set(failedCoverUrls.value).add(coverUrl);
};

const updateSourceDatabase = async (group: LibrarySourceGroup) => {
  closeSourceMenu();
  isScanning.value = true;
  try {
    const response = await fetch(`${group.apiBase}/database/update`, {
      method: "POST",
    });
    const result = await response.json();
    if (!response.ok || !result.success) {
      throw new Error(result.message || "更新失败");
    }
    await refreshSingleSource(group.apiBase);
  } catch (error) {
    console.error("Failed to update source database:", error);
    alert(`更新失败: ${error}`);
  } finally {
    isScanning.value = false;
  }
};

const openUploadForSource = (group: LibrarySourceGroup) => {
  uploadTargetApiBase.value = group.apiBase;
  showUploadModal.value = true;
  closeSourceMenu();
};

const openOnlineSearchForSource = (group: LibrarySourceGroup) => {
  onlineSearchApiBase.value = group.apiBase;
  closeSourceMenu();
  showOnlineSearchModal.value = true;
};

const changeSourceDirectory = async (group: LibrarySourceGroup) => {
  closeSourceMenu();
  const newPath = prompt("请输入新的音乐目录路径:");
  if (!newPath || !newPath.trim()) {
    return;
  }

  try {
    const response = await fetch(`${group.apiBase}/settings/music-directory`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ path: newPath.trim() }),
    });
    const result = await response.json();
    if (!response.ok || !result.success) {
      throw new Error(result.message || "更改目录失败");
    }
    await refreshSingleSource(group.apiBase);
  } catch (error) {
    console.error("Failed to change source directory:", error);
    alert(`更改目录失败: ${error}`);
  }
};

// Collection management handlers
const handleShowAddToCollectionModal = () => {
  selectedCollections.value = [];
  pendingSongsForCollection.value = [];
  showAddToCollection.value = true;
};

const hideAddToCollectionModal = () => {
  showAddToCollection.value = false;
  selectedCollections.value = [];
  pendingSongsForCollection.value = [];
};

const handleToggleCollectionSelection = (id: number) => {
  const index = selectedCollections.value.indexOf(id);
  if (index > -1) {
    selectedCollections.value.splice(index, 1);
  } else {
    selectedCollections.value.push(id);
  }
};

const addToCollection = async () => {
  if (selectedCollections.value.length === 0) {
    alert("请选择至少一个收藏夹");
    return;
  }

  const selectedVisibleSongs =
    pendingSongsForCollection.value.length > 0
      ? pendingSongsForCollection.value
      : currentSongs.value.filter((song) =>
          selectedSongs.value.has(song.rowKey || buildSongRowKey(song)),
        );

  if (selectedVisibleSongs.length === 0) {
    alert("没有选中的歌曲");
    return;
  }

  localCollections.value = localCollections.value.map((collection) => {
    if (!selectedCollections.value.includes(collection.id)) {
      return collection;
    }

    const existingKeys = new Set(
      collection.songs.map(
        (song) => `${song.source_key || "local"}:${song.id}:${song.name}`,
      ),
    );
    const nextSongs = collection.songs.slice();

    for (const song of selectedVisibleSongs) {
      const songKey = `${(song as MusicInfo).source_key || "local"}:${song.id}:${song.name}`;
      const normalizedSong = {
        ...(song as MusicInfo),
        rowKey: song.rowKey || songKey,
      };
      if (existingKeys.has(songKey)) {
        continue;
      }
      existingKeys.add(songKey);
      nextSongs.push(normalizedSong);
    }

    return {
      ...collection,
      songs: nextSongs,
    };
  });
  syncLocalCollections();

  alert("添加成功");
  hideAddToCollectionModal();
  selectMode.value = false;
  selectedSongs.value.clear();
};

const handleRemoveFromCollection = async () => {
  if (!selectedPlaylist.value) return;

  const selectedNames = new Set(selectedSongs.value);
  if (selectedNames.size === 0) {
    alert("没有选中的歌曲");
    return;
  }

  localCollections.value = localCollections.value.map((collection) => {
    if (collection.name !== selectedPlaylist.value?.name) {
      return collection;
    }

    return {
      ...collection,
      songs: collection.songs.filter(
        (song) => !selectedNames.has(song.rowKey || buildSongRowKey(song)),
      ),
    };
  });
  syncLocalCollections();
  handleSelectCollection(selectedPlaylist.value.name);
  alert("移除成功");
  selectMode.value = false;
  selectedSongs.value.clear();
};

const removeSingleSongFromCollection = (song: MusicInfo) => {
  if (!selectedPlaylist.value) {
    return;
  }

  const songKey = song.rowKey || buildSongRowKey(song);
  localCollections.value = localCollections.value.map((collection) => {
    if (collection.name !== selectedPlaylist.value?.name) {
      return collection;
    }

    return {
      ...collection,
      songs: collection.songs.filter(
        (item) => (item.rowKey || buildSongRowKey(item)) !== songKey,
      ),
    };
  });
  syncLocalCollections();
  handleSelectCollection(selectedPlaylist.value.name);
};

const handleSongCollectionAction = (song: SongInfo) => {
  openSongMenu(song);
};

const handleShowCreateModal = () => {
  newCollectionName.value = "";
  showCreateCollection.value = true;
};

const handleCreateCollectionFromAddModal = () => {
  showAddToCollection.value = false;
  handleShowCreateModal();
};

const hideCreateCollectionModal = () => {
  showCreateCollection.value = false;
  newCollectionName.value = "";
};

const handleCreateCollection = async () => {
  if (!newCollectionName.value.trim()) {
    alert("请输入收藏夹名称");
    return;
  }

  const shouldReturnToAddModal = pendingSongsForCollection.value.length > 0;
  localCollections.value = [
    ...localCollections.value,
    {
      id: Date.now(),
      name: newCollectionName.value.trim(),
      created_at: new Date().toISOString(),
      songs: [],
    },
  ];
  syncLocalCollections();
  hideCreateCollectionModal();
  if (shouldReturnToAddModal) {
    showAddToCollection.value = true;
  }
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
  await triggerDatabaseUpdate();

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
