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
      <div v-if="startupStatusMessage" class="startup-message">
        <span>{{ startupStatusMessage }}</span>
        <button
          v-if="showSharedPlayPrompt"
          class="startup-play-btn"
          @click="handleStartSharedPlayback"
        >
          播放
        </button>
      </div>

      <div class="main-area" :class="{ 'wide-layout': isWideLayout }">
        <div class="list-panel" v-show="!isPlayerPanelVisible || isWideLayout">
          <AppContentView
            :current-view="currentView"
            :active-tab="activeTab"
            :is-wide-layout="isWideLayout"
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
            :song-selection-action-label="songSelectionActionLabel"
            :active-download-jobs="activeDownloadJobs"
            @set-active-tab="handleSetActiveTab"
            @open-filter-sheet="openFilterSheet"
            @add-device="handleAddDevice"
            @open-downloads="openDownloads"
            @show-library-home="showLibraryHome"
            @show-collections-home="showCollectionsHome"
            @select-library-playlist="handleSelectLibraryPlaylist"
            @open-source-menu="handleOpenSourceMenu"
            @retry-source-connection="handleRetrySourceConnection"
            @toggle-collection-select-mode="toggleCollectionSelectMode"
            @toggle-collection-selection="toggleCollectionSelection"
            @select-collection="handleSelectCollection"
            @show-create-modal="handleShowCreateModal"
            @delete-selected-collections="handleDeleteSelectedCollections"
            @open-collection-menu="handleOpenCollectionMenu"
            @open-song-list-menu="handleOpenSongListMenu"
            @back-to-playlists="handleBackToPlaylists"
            @toggle-select-mode="toggleSelectMode"
            @toggle-song-selection="toggleSongSelection"
            @play-song="handlePlaySong"
            @perform-song-selection-action="handleSongSelectionAction"
            @open-online-search-from-query="openOnlineSearchFromQuery"
            @reset-library-filter="resetLibraryFilter"
          />
        </div>
        <AppPlayerView
          :is-player-panel-visible="isPlayerPanelVisible"
          :is-wide-layout="isWideLayout"
          :is-lyric-panel-visible="isLyricPanelVisible"
          :select-mode="selectMode"
          :is-lyrics-loading="isLyricsLoading"
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
          @open-online-lyric-search="openOnlineLyricSearch"
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

    <div
      v-if="isShareModalVisible"
      class="share-modal-overlay"
      @click="closeShareModal"
    >
      <div class="share-modal" @click.stop>
        <div class="share-modal-header">
          <h2>分享歌曲</h2>
          <button
            type="button"
            class="share-close-btn"
            aria-label="关闭"
            @click="closeShareModal"
          >
            <i class="fas fa-xmark"></i>
          </button>
        </div>
        <div class="share-song-name">{{ currentSong?.name }}</div>
        <input
          ref="shareInputRef"
          class="share-url-input"
          :value="currentSongShareUrl"
          readonly
          @focus="selectShareUrl"
        />
        <div class="share-modal-footer">
          <span class="share-copy-status">{{ shareCopyStatus }}</span>
          <button type="button" class="share-copy-btn" @click="copyShareUrl">
            复制链接
          </button>
        </div>
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
      :lufs-precache-count="lufsPrecacheCount"
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
      @update:lufs-precache-count="handleLufsPrecacheCountChange"
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
      :initial-query="onlineSearchInitialQuery"
      :api-base="onlineSearchApiBase"
      :source-name="onlineSearchSourceName"
      :source-options="onlineSearchSources"
      @close="showOnlineSearchModal = false"
      @change-source="setOnlineSearchSource"
      @download-complete="handleOnlineDownloadComplete"
      @preview-track="handlePreviewTrack"
    />

    <LyricSearchModal
      v-if="showLyricSearchModal && currentSong"
      :api-base="currentSongLyricApiBase"
      :initial-query="currentSong.name"
      mode="apply"
      :song-id="currentSong.id"
      @close="showLyricSearchModal = false"
      @applied="handleLyricApplied"
    />

    <!-- Active Queue Modal -->
    <ActiveQueueModal
      v-if="showActiveQueueModal"
      :songs="activeQueue"
      :current-song-name="currentSong?.name"
      :show-share-button="Boolean(currentSongShareUrl)"
      @close="showActiveQueueModal = false"
      @play="handlePlayQueueSong"
      @share="openShareModal"
    />

    <AppActionSheets
      :active-tab="songMenuTab"
      :selected-source-menu-group="selectedSourceMenuGroup"
      :selected-song-list-menu-title="selectedSongListMenuTitle"
      :selected-collection-menu-name="selectedCollectionMenuName"
      @close-source-menu="closeSourceMenu"
      @refresh-source="handleUpdateSourceDatabase"
      @upload-to-source="openUploadForSource"
      @set-online-search-source="handleSetOnlineSearchSourceFromMenu"
      @change-source-directory="handleChangeSourceDirectory"
      @retry-source-connection="handleRetrySourceConnection"
      @show-source-details="handleShowSourceDetails"
      @delete-source="handleDeleteSource"
      @close-song-list-menu="closeSongListMenu"
      @start-song-list-collection-selection="startSongListCollectionSelection"
      @start-song-list-delete-selection="startSongListDeleteSelection"
      @close-collection-menu="closeCollectionMenu"
      @rename-collection="renameCollection"
      @delete-collection="deleteCollectionFromMenu"
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import SearchBar from "@/components/SearchBar.vue";
import AppActionSheets from "@/components/AppActionSheets.vue";
import AppContentView from "@/components/AppContentView.vue";
import AppPlayerView from "@/components/AppPlayerView.vue";
import LibraryFilterSheet from "@/components/LibraryFilterSheet.vue";
import AddDeviceModal from "@/components/modals/AddDeviceModal.vue";
import SettingsModal from "@/components/modals/SettingsModal.vue";
import ActiveQueueModal from "@/components/modals/ActiveQueueModal.vue";
import AddToCollectionModal from "@/components/modals/AddToCollectionModal.vue";
import CreateCollectionModal from "@/components/modals/CreateCollectionModal.vue";
import LyricSearchModal from "@/components/modals/LyricSearchModal.vue";
import OnlineSearchModal from "@/components/modals/OnlineSearchModal.vue";
import UploadModal from "@/components/modals/UploadModal.vue";
import { useAppShell } from "@/composables/useAppShell";

// Related documentation: `docs/shared-song-links.md`

const {
  searchQuery,
  showFilterSheet,
  draftSourceFilterKey,
  draftMediaTypes,
  onlineSearchApiBase,
  onlineSearchInitialQuery,
  libraryGroupSummaries,
  searchResults,
  filterSources,
  onlineSearchSources,
  trimmedSearchQuery,
  onlineSearchSourceName,
  currentView,
  activeTab,
  activeDownloadJobs,
  selectedPlaylist,
  showSettings,
  showAddDeviceModal,
  showUploadModal,
  showOnlineSearchModal,
  showLyricSearchModal,
  showActiveQueueModal,
  isScanning,
  uploadTargetApiBase,
  currentSongs,
  activeQueue,
  currentSong,
  currentSongLyricApiBase,
  currentSongShareUrl,
  isPlaying,
  currentTime,
  duration,
  playMode,
  showLufs,
  lufsPrecacheCount,
  volumeMode,
  manualVolume,
  manualVolumeInput,
  fixedLufs,
  fixedLufsInput,
  volumeModeLabels,
  timerMinutes,
  timerMinutesInput,
  timerActive,
  timerStatusDisplay,
  localCollections,
  collectionNames,
  collectionPlaylists,
  showAddToCollection,
  selectedCollections,
  newCollectionName,
  showCreateCollection,
  selectedCollectionMenuName,
  selectMode,
  selectedSongs,
  songSelectionActionLabel,
  collectionSelectMode,
  selectedCollectionsList,
  hasSelectedNonAllMusicCollection,
  lyrics,
  currentLyricIndex,
  isLyricsLoading,
  hasLyrics,
  selectedSourceMenuGroup,
  selectedSongListMenuTitle,
  startupStatusMessage,
  showSharedPlayPrompt,
  songMenuTab,
  showBackButton,
  showActionBar,
  isWideLayout,
  isPlayerPanelVisible,
  isLyricPanelVisible,
  resolveSongCoverUrl,
  handleSearch,
  handleSetActiveTab,
  openDownloads,
  showLibraryHome,
  showCollectionsHome,
  handleActionBack,
  handleShowSettingsModal,
  handleShowLufsChange,
  handleLufsPrecacheCountChange,
  handleSetTimerPreset,
  handleStartTimer,
  handleCancelTimer,
  handleDirectoryChanged,
  handleDatabaseUpdated,
  handleDatabaseUpdateStart,
  handleDatabaseUpdateEnd,
  handleManageCollections,
  handleSourcesUpdated,
  handleDeviceConnected,
  handleToggleCollectionSelection,
  handleCreateCollectionFromAddModal,
  handleCreateCollection,
  addToCollection,
  hideAddToCollectionModal,
  hideCreateCollectionModal,
  handleUploadComplete,
  handleOnlineDownloadComplete,
  handleLyricApplied,
  handlePreviewTrack,
  setOnlineSearchSource,
  handlePlayQueueSong,
  handleUpdateSourceDatabase,
  handleSetOnlineSearchSourceFromMenu,
  handleChangeSourceDirectory,
  handleRetrySourceConnection,
  handleShowSourceDetails,
  handleDeleteSource,
  closeSourceMenu,
  closeSongListMenu,
  startSongListCollectionSelection,
  startSongListDeleteSelection,
  closeCollectionMenu,
  renameCollection,
  deleteCollectionFromMenu,
  openOnlineSearchFromQuery,
  openOnlineLyricSearch,
  handleSelectCollection,
  handleSelectLibraryPlaylist,
  handleBackToPlaylists,
  toggleSelectMode,
  toggleSongSelection,
  toggleCollectionSelectMode,
  toggleCollectionSelection,
  handlePlaySong,
  handleSongSelectionAction,
  handleLyricLineClick,
  handleShowActiveQueue,
  handleStartSharedPlayback,
  togglePlayerPanelMode,
  showCoverPanel,
  showLyricsPanel,
  handleCoverLoadError,
  seekToTime,
  play,
  pause,
  previousSong,
  nextSong,
  togglePlayMode,
  openUploadForSource,
  handleAddDevice,
  handleOpenSourceMenu,
  handleOpenCollectionMenu,
  handleOpenSongListMenu,
  handleDeleteSelectedCollections,
  handleShowCreateModal,
  hideSettingsModal,
  openFilterSheet,
  toggleDraftMediaType,
  applyLibraryFilter,
  resetLibraryFilter,
} = useAppShell();

const shareInputRef = ref<HTMLInputElement | null>(null);
const isShareModalVisible = ref(false);
const shareCopyStatus = ref("");

const copyTextToClipboard = async (text: string) => {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.top = "0";
  textarea.style.left = "0";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand("copy");
  document.body.removeChild(textarea);
};

const openShareModal = () => {
  shareCopyStatus.value = "";
  isShareModalVisible.value = true;
  setTimeout(() => shareInputRef.value?.select(), 0);
};

const closeShareModal = () => {
  isShareModalVisible.value = false;
};

const selectShareUrl = () => {
  shareInputRef.value?.select();
};

const copyShareUrl = async () => {
  if (!currentSongShareUrl.value) {
    return;
  }

  try {
    await copyTextToClipboard(currentSongShareUrl.value);
    shareCopyStatus.value = "已复制分享链接";
  } catch (error) {
    console.error("Failed to copy shared song link:", error);
    shareCopyStatus.value = "复制失败，请手动复制";
  }
};
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

.share-modal-overlay {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  z-index: 1100;
  background: rgba(0, 0, 0, 0.42);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  box-sizing: border-box;
}

.share-modal {
  width: 100%;
  max-width: 520px;
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.2);
  padding: 18px;
  box-sizing: border-box;
}

.share-modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.share-modal-header h2 {
  margin: 0;
  font-size: 18px;
  color: #20252a;
}

.share-close-btn {
  width: 34px;
  height: 34px;
  border: none;
  border-radius: 8px;
  background: #f1f3f4;
  color: #31414f;
  cursor: pointer;
}

.share-song-name {
  margin-bottom: 10px;
  color: #31414f;
  font-size: 14px;
  font-weight: 600;
  overflow-wrap: anywhere;
}

.share-url-input {
  width: 100%;
  height: 40px;
  border: 1px solid #ccd6dd;
  border-radius: 6px;
  padding: 0 10px;
  box-sizing: border-box;
  color: #20252a;
  font-size: 14px;
}

.share-modal-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 14px;
}

.share-copy-status {
  min-width: 0;
  color: #176b3a;
  font-size: 13px;
  overflow-wrap: anywhere;
}

.share-copy-btn {
  flex: none;
  border: none;
  border-radius: 6px;
  background: #176b3a;
  color: #fff;
  padding: 10px 16px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}

.share-copy-btn:hover,
.share-copy-btn:focus-visible {
  background: #145a31;
  outline: none;
}

.scanning-message {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 120px;
  font-size: 18px;
  color: #888;
}

.startup-message {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 16px;
  background-color: #fff4d6;
  border-bottom: 1px solid #f1d08a;
  color: #7c4a03;
  font-size: 14px;
}

.startup-play-btn {
  flex-shrink: 0;
  border: none;
  border-radius: 999px;
  padding: 8px 16px;
  background-color: #d97706;
  color: #fff;
  font-size: 14px;
  cursor: pointer;
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
