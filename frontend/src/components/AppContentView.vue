<template>
  <div ref="contentAreaRef" class="content-area">
    <div
      v-if="showTopNavigation"
      :class="['content-tabs', { 'compact-content-tabs': !isWideLayout }]"
    >
      <div class="content-nav-group">
        <button
          :class="['content-tab', { active: isLibraryNavActive }]"
          @click="$emit('setActiveTab', 'library')"
        >
          曲库
        </button>
        <button
          :class="['content-tab', { active: isCollectionsNavActive }]"
          @click="$emit('setActiveTab', 'collections')"
        >
          {{ isWideLayout ? "我的收藏" : "收藏" }}
        </button>
        <button
          :class="['content-tab', { active: currentView === 'downloads' }]"
          @click="handleOpenDownloads"
        >
          下载
        </button>
      </div>

      <button
        v-if="hasPageActions"
        class="page-actions-trigger"
        aria-label="页面操作"
        @click="showPageActionsSheet = true"
      >
        ⋮
      </button>
    </div>

    <div
      v-if="showPageActionsSheet"
      class="page-actions-overlay"
      @click="showPageActionsSheet = false"
    >
      <div class="page-actions-sheet" @click.stop>
        <div class="page-actions-title">{{ pageActionsTitle }}</div>
        <button
          v-for="action in pageActions"
          :key="action.key"
          class="page-actions-item"
          @click="runPageAction(action.key)"
        >
          {{ action.label }}
        </button>
      </div>
    </div>

    <DownloadJobsView
      v-if="currentView === 'downloads'"
      :jobs="activeDownloadJobs"
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
      <div>
        {{
          isWideLayout
            ? "还没有个人收藏夹，点击上方“新建收藏夹”开始。"
            : "还没有个人收藏夹，从“操作”里新建一个开始。"
        }}
      </div>
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
import { computed, nextTick, ref, watch } from "vue";
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
  isWideLayout: boolean;
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

const emit = defineEmits<{
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

type PageActionKey = "filter" | "add-device" | "create-collection";

const contentAreaRef = ref<HTMLDivElement | null>(null);
const songsViewRef = ref<ScrollableSongList | null>(null);
const searchViewRef = ref<ScrollableSongList | null>(null);
const showPageActionsSheet = ref(false);
const playlistScrollPositions = {
  library: 0,
  collections: 0,
};
const detailScrollPositions = new Map<string, number>();

const showTopNavigation = computed(
  () => props.currentView === "playlists" || props.currentView === "downloads",
);

const isLibraryNavActive = computed(
  () => props.currentView === "playlists" && props.activeTab === "library",
);

const isCollectionsNavActive = computed(
  () => props.currentView === "playlists" && props.activeTab === "collections",
);

const pageActions = computed<{ key: PageActionKey; label: string }[]>(() => {
  if (props.currentView !== "playlists") {
    return [];
  }

  if (props.activeTab === "library") {
    return [
      { key: "filter", label: "筛选曲库" },
      { key: "add-device", label: "添加设备" },
    ];
  }

  return [{ key: "create-collection", label: "新建收藏夹" }];
});

const hasPageActions = computed(() => pageActions.value.length > 0);

const pageActionsTitle = computed(() =>
  props.activeTab === "library" ? "曲库操作" : "收藏操作",
);

const handleOpenDownloads = () => {
  if (props.currentView === "downloads") {
    return;
  }
  emit("openDownloads");
};

const runPageAction = (action: PageActionKey) => {
  showPageActionsSheet.value = false;
  switch (action) {
    case "filter":
      emit("openFilterSheet");
      return;
    case "add-device":
      emit("addDevice");
      return;
    case "create-collection":
      emit("showCreateModal");
  }
};

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
    showPageActionsSheet.value = false;
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
  justify-content: space-between;
  gap: 12px;
  padding: 14px 0 10px;
  position: sticky;
  top: 0;
  background: #fff;
  z-index: 5;
}

.content-nav-group {
  min-width: 0;
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

.compact-content-tabs {
  gap: 10px;
}

.compact-content-tabs .content-nav-group {
  flex: 1;
  min-width: 0;
}

.compact-content-tabs .content-tab {
  flex: 1;
  min-width: 0;
  padding: 8px 10px;
}

.page-actions-trigger {
  border: none;
  background: transparent;
  color: #31414f;
  border-radius: 10px;
  padding: 6px 8px;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
  flex-shrink: 0;
}

.page-actions-overlay {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: flex-end;
  z-index: 1000;
}

.page-actions-sheet {
  width: 100%;
  background: #fff;
  border-radius: 18px 18px 0 0;
  padding: 12px 16px 24px;
  box-sizing: border-box;
}

.page-actions-title {
  padding: 8px 4px 14px;
  font-size: 16px;
  font-weight: 700;
  color: #222;
}

.page-actions-item {
  width: 100%;
  border: none;
  background: transparent;
  color: #222;
  text-align: left;
  padding: 14px 4px;
  font-size: 15px;
  cursor: pointer;
  border-top: 1px solid #f1f1f1;
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
