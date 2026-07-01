<template>
  <div ref="contentAreaRef" class="content-area">
    <div v-if="currentView === 'playlists'" class="content-tabs">
      <button
        :class="['content-tab', { active: activeTab === 'library' }]"
        @click="$emit('setActiveTab', 'library')"
      >
        曲库
      </button>
      <button
        :class="['content-tab', { active: activeTab === 'collections' }]"
        @click="$emit('setActiveTab', 'collections')"
      >
        我的收藏
      </button>
      <button class="content-tab filter-tab" @click="$emit('openFilterSheet')">
        筛选
      </button>
      <button
        v-if="activeTab === 'library'"
        class="content-tab"
        @click="$emit('addDevice')"
      >
        添加设备
      </button>
      <button
        v-if="activeTab === 'collections'"
        class="content-tab"
        @click="$emit('showCreateModal')"
      >
        新建收藏夹
      </button>
      <button class="content-tab" @click="$emit('openDownloads')">
        下载中
      </button>
    </div>

    <DownloadJobsView
      v-if="currentView === 'downloads'"
      :jobs="activeDownloadJobs"
      @show-library="$emit('showLibraryHome')"
      @show-collections="$emit('showCollectionsHome')"
    />

    <LibrarySourceListView
      v-if="
        currentView === 'playlists' &&
        activeTab === 'library' &&
        libraryGroupSummaries.length > 0
      "
      :groups="libraryGroupSummaries"
      @select-playlist="
        (group, playlistName) =>
          $emit('selectLibraryPlaylist', group, playlistName)
      "
      @open-menu="$emit('openSourceMenu', $event)"
      @retry="$emit('retrySourceConnection', $event.sourceKey)"
    />
    <div
      v-if="
        currentView === 'playlists' &&
        activeTab === 'library' &&
        libraryGroupSummaries.length === 0
      "
      class="empty-state"
    >
      <div>当前筛选条件下没有可显示的曲库内容</div>
      <button
        class="empty-online-search-btn"
        @click="$emit('resetLibraryFilter')"
      >
        重置筛选
      </button>
    </div>

    <PlaylistListView
      v-if="
        currentView === 'playlists' &&
        activeTab === 'collections' &&
        collectionNames.length > 0
      "
      title="我的收藏"
      view-mode="collection"
      :playlist-names="collectionNames"
      :playlists="collectionPlaylists"
      :select-mode="collectionSelectMode"
      :selected-playlists="selectedCollectionsList"
      :show-select-button="false"
      :has-selected-non-all-music="hasSelectedNonAllMusic"
      :show-header="false"
      :show-playlist-action-button="true"
      playlist-action-label="⋮"
      @toggle-select-mode="$emit('toggleCollectionSelectMode')"
      @toggle-selection="$emit('toggleCollectionSelection', $event)"
      @select="$emit('selectCollection', $event)"
      @show-create-modal="$emit('showCreateModal')"
      @delete-selected="$emit('deleteSelectedCollections')"
      @playlist-action="$emit('openCollectionMenu', $event)"
    />
    <div
      v-if="
        currentView === 'playlists' &&
        activeTab === 'collections' &&
        collectionNames.length === 0
      "
      class="empty-state"
    >
      <div>还没有个人收藏夹，点击上方“新建收藏夹”开始。</div>
    </div>

    <SongListView
      v-if="currentView === 'songs'"
      ref="songsViewRef"
      :title="selectedPlaylistTitle"
      :songs="currentSongs"
      :select-mode="selectMode"
      :selected-songs="selectedSongs"
      :current-song-name="currentSongName"
      :show-remove-button="activeTab === 'collections'"
      :show-add-button="activeTab === 'library'"
      :show-header="true"
      :show-lufs="showLufs"
      :show-select-button="false"
      :show-header-action-button="true"
      header-action-label="⋮"
      :selection-count="selectedSongs.size"
      :selection-action-label="songSelectionActionLabel"
      @back="$emit('backToPlaylists')"
      @toggle-select-mode="$emit('toggleSelectMode')"
      @toggle-selection="$emit('toggleSongSelection', $event)"
      @play="(song, index) => $emit('playSong', song, index)"
      @selection-action="$emit('performSongSelectionAction')"
      @header-action="$emit('openSongListMenu', selectedPlaylistTitle)"
    />

    <div v-if="currentView === 'search'">
      <SongListView
        v-if="searchResults.length > 0 || selectMode"
        ref="searchViewRef"
        title="库内结果"
        :songs="searchResults"
        :select-mode="selectMode"
        :selected-songs="selectedSongs"
        :selection-count="selectedSongs.size"
        :selection-action-label="songSelectionActionLabel"
        :show-remove-button="false"
        :show-add-button="false"
        :show-header="true"
        :show-back-button="false"
        :show-select-button="false"
        :show-header-action-button="searchResults.length > 0"
        header-action-label="⋮"
        :show-lufs="showLufs"
        @toggle-select-mode="$emit('toggleSelectMode')"
        @toggle-selection="$emit('toggleSongSelection', $event)"
        @play="(song, index) => $emit('playSong', song, index)"
        @selection-action="$emit('performSongSelectionAction')"
        @header-action="$emit('openSongListMenu', '搜索结果')"
      />
      <div class="search-results-actions">
        <button
          class="online-search-entry"
          @click="$emit('openOnlineSearchFromQuery')"
        >
          在线搜索 “{{ trimmedSearchQuery }}”
        </button>
        <button
          class="online-search-entry secondary"
          @click="$emit('openDownloads')"
        >
          查看下载进度
        </button>
      </div>
      <div v-if="searchResults.length === 0 && !selectMode" class="empty-state">
        <div>未找到库内结果</div>
        <button
          class="empty-online-search-btn"
          @click="$emit('openOnlineSearchFromQuery')"
        >
          在线搜索 “{{ trimmedSearchQuery }}”
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import DownloadJobsView from "@/components/DownloadJobsView.vue";
import PlaylistListView from "@/components/PlaylistListView.vue";
import LibrarySourceListView from "@/components/LibrarySourceListView.vue";
import SongListView from "@/components/SongListView.vue";
import type { ActiveDownloadJob } from "@/stores/downloads";
import type { MusicInfo } from "@/types/music";
import type { LibrarySourceGroupSummary } from "@/types/library";

const props = defineProps<{
  currentView: "playlists" | "songs" | "search" | "downloads";
  activeTab: "library" | "collections";
  libraryGroupSummaries: LibrarySourceGroupSummary[];
  collectionNames: string[];
  collectionPlaylists: Record<string, MusicInfo[]>;
  collectionSelectMode: boolean;
  selectedCollectionsList: Set<string>;
  hasSelectedNonAllMusic: boolean;
  selectedPlaylistTitle: string;
  currentSongs: MusicInfo[];
  selectMode: boolean;
  selectedSongs: Set<string>;
  currentSongName?: string;
  showLufs: boolean;
  trimmedSearchQuery: string;
  searchResults: MusicInfo[];
  songSelectionActionLabel: string;
  activeDownloadJobs: ActiveDownloadJob[];
}>();

defineEmits<{
  (e: "setActiveTab", tab: "library" | "collections"): void;
  (e: "openFilterSheet"): void;
  (e: "addDevice"): void;
  (
    e: "selectLibraryPlaylist",
    group: LibrarySourceGroupSummary,
    playlistName: string,
  ): void;
  (e: "openSourceMenu", group: LibrarySourceGroupSummary): void;
  (e: "retrySourceConnection", sourceKey: string): void;
  (e: "toggleCollectionSelectMode"): void;
  (e: "toggleCollectionSelection", name: string): void;
  (e: "selectCollection", name: string): void;
  (e: "showCreateModal"): void;
  (e: "deleteSelectedCollections"): void;
  (e: "openCollectionMenu", name: string): void;
  (e: "openSongListMenu", title: string): void;
  (e: "backToPlaylists"): void;
  (e: "toggleSelectMode"): void;
  (e: "toggleSongSelection", key: string): void;
  (e: "playSong", song: MusicInfo, index: number): void;
  (e: "performSongSelectionAction"): void;
  (e: "openOnlineSearchFromQuery"): void;
  (e: "resetLibraryFilter"): void;
  (e: "openDownloads"): void;
  (e: "showLibraryHome"): void;
  (e: "showCollectionsHome"): void;
}>();

type ScrollableSongList = {
  getScrollTop: () => number;
  setScrollTop: (scrollTop: number) => void;
};

const contentAreaRef = ref<HTMLDivElement | null>(null);
const songsViewRef = ref<ScrollableSongList | null>(null);
const searchViewRef = ref<ScrollableSongList | null>(null);
const playlistScrollPositions = {
  library: 0,
  collections: 0,
};
const detailScrollPositions = new Map<string, number>();

const getPlaylistScrollKey = (playlistTitle: string) =>
  `playlist:${playlistTitle}`;

const saveScrollPosition = (
  currentView: "playlists" | "songs" | "search" | "downloads",
  activeTab: "library" | "collections",
  selectedPlaylistTitle: string,
) => {
  if (currentView === "playlists") {
    playlistScrollPositions[activeTab] = contentAreaRef.value?.scrollTop ?? 0;
    return;
  }

  if (currentView === "downloads") {
    return;
  }

  if (currentView === "search") {
    detailScrollPositions.set(
      "search",
      searchViewRef.value?.getScrollTop() ?? 0,
    );
    return;
  }

  detailScrollPositions.set(
    getPlaylistScrollKey(selectedPlaylistTitle),
    songsViewRef.value?.getScrollTop() ?? 0,
  );
};

const restoreScrollPosition = async (
  currentView: "playlists" | "songs" | "search" | "downloads",
  activeTab: "library" | "collections",
  selectedPlaylistTitle: string,
) => {
  await nextTick();

  if (currentView === "downloads") {
    return;
  }

  if (currentView === "playlists") {
    if (contentAreaRef.value) {
      contentAreaRef.value.scrollTop = playlistScrollPositions[activeTab];
    }
    return;
  }

  if (currentView === "search") {
    searchViewRef.value?.setScrollTop(detailScrollPositions.get("search") ?? 0);
    return;
  }

  songsViewRef.value?.setScrollTop(
    detailScrollPositions.get(getPlaylistScrollKey(selectedPlaylistTitle)) ?? 0,
  );
};

watch(
  () =>
    [props.currentView, props.activeTab, props.selectedPlaylistTitle] as const,
  ([currentView, activeTab, selectedPlaylistTitle], previousState) => {
    if (previousState) {
      saveScrollPosition(...previousState);
    }
    void restoreScrollPosition(currentView, activeTab, selectedPlaylistTitle);
  },
  { flush: "pre", immediate: true },
);
</script>

<style scoped>
.content-area {
  height: 100%;
  overflow-y: auto;
  padding: 0 15px;
  background-color: #fff;
  position: relative;
  box-sizing: border-box;
}

.content-tabs {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 14px 0 10px;
  position: sticky;
  top: 0;
  background: #fff;
  z-index: 5;
}

.content-tab {
  border: 1px solid #d8e1e8;
  background: #f7fafc;
  color: #31414f;
  border-radius: 999px;
  padding: 8px 14px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}

.content-tab.active {
  background: #1db954;
  border-color: #1db954;
  color: #fff;
}

.filter-tab {
  margin-left: auto;
}

.empty-state {
  padding: 24px 16px;
  text-align: center;
  color: #888;
  font-size: 14px;
}

.search-results-actions {
  padding: 16px 0 8px;
}

.online-search-entry,
.empty-online-search-btn {
  border: 1px solid #1db954;
  background: #eefaf2;
  color: #126b37;
  border-radius: 10px;
  padding: 12px 14px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}

.online-search-entry {
  width: 100%;
  text-align: left;
}

.online-search-entry.secondary {
  margin-top: 10px;
  border-color: #d7b36a;
  background: #fff7e6;
  color: #8a5a11;
}

.empty-online-search-btn {
  margin-top: 12px;
}
</style>
