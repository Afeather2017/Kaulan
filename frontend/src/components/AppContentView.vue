<template>
  <div class="content-area">
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
    </div>

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
      v-if="currentView === 'playlists' && activeTab === 'collections'"
      class="collection-actions-bar"
    >
      <button class="collection-create-btn" @click="$emit('showCreateModal')">
        新建收藏夹
      </button>
    </div>
    <div
      v-if="
        currentView === 'playlists' &&
        activeTab === 'collections' &&
        collectionNames.length === 0
      "
      class="empty-state"
    >
      <div>还没有个人收藏夹</div>
      <button class="empty-online-search-btn" @click="$emit('showCreateModal')">
        创建第一个收藏夹
      </button>
    </div>

    <SongListView
      v-if="currentView === 'songs'"
      :title="selectedPlaylistTitle"
      :songs="currentSongs"
      :select-mode="selectMode"
      :selected-songs="selectedSongs"
      :current-song-name="currentSongName"
      :show-remove-button="activeTab === 'collections'"
      :show-add-button="activeTab === 'library'"
      :show-song-action-button="true"
      song-action-label="⋮"
      :show-header="true"
      :show-lufs="showLufs"
      :show-select-button="false"
      :show-header-action-button="activeTab === 'collections'"
      header-action-label="⋮"
      @back="$emit('backToPlaylists')"
      @toggle-select-mode="$emit('toggleSelectMode')"
      @toggle-selection="$emit('toggleSongSelection', $event)"
      @play="(song, index) => $emit('playSong', song, index)"
      @remove="$emit('removeFromCollection')"
      @show-add-modal="$emit('showAddToCollectionModal')"
      @song-action="$emit('songAction', $event)"
      @header-action="$emit('openCollectionMenu', selectedPlaylistTitle)"
    />

    <div v-if="currentView === 'search'">
      <div class="search-results-actions">
        <button
          class="online-search-entry"
          @click="$emit('openOnlineSearchFromQuery')"
        >
          在线搜索 “{{ trimmedSearchQuery }}”
        </button>
      </div>
      <SongListView
        v-if="searchResults.length > 0"
        title="库内结果"
        :songs="searchResults"
        :select-mode="false"
        :selected-songs="new Set()"
        :show-remove-button="false"
        :show-add-button="false"
        :show-header="false"
        :show-lufs="showLufs"
        @back="$emit('backToPlaylists')"
        @play="(song, index) => $emit('playSong', song, index)"
      />
      <div v-else class="empty-state">
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
import PlaylistListView from "@/components/PlaylistListView.vue";
import LibrarySourceListView, {
  type LibrarySourceGroupSummary,
} from "@/components/LibrarySourceListView.vue";
import SongListView, { type SongInfo } from "@/components/SongListView.vue";

defineProps<{
  currentView: "playlists" | "songs" | "search";
  activeTab: "library" | "collections";
  libraryGroupSummaries: LibrarySourceGroupSummary[];
  collectionNames: string[];
  collectionPlaylists: Record<string, SongInfo[]>;
  collectionSelectMode: boolean;
  selectedCollectionsList: Set<string>;
  hasSelectedNonAllMusic: boolean;
  selectedPlaylistTitle: string;
  currentSongs: SongInfo[];
  selectMode: boolean;
  selectedSongs: Set<string>;
  currentSongName?: string;
  showLufs: boolean;
  trimmedSearchQuery: string;
  searchResults: SongInfo[];
}>();

defineEmits<{
  (e: "setActiveTab", tab: "library" | "collections"): void;
  (e: "openFilterSheet"): void;
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
  (e: "backToPlaylists"): void;
  (e: "toggleSelectMode"): void;
  (e: "toggleSongSelection", key: string): void;
  (e: "playSong", song: SongInfo, index: number): void;
  (e: "removeFromCollection"): void;
  (e: "showAddToCollectionModal"): void;
  (e: "songAction", song: SongInfo): void;
  (e: "openOnlineSearchFromQuery"): void;
  (e: "resetLibraryFilter"): void;
}>();
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

.collection-actions-bar {
  display: flex;
  justify-content: flex-end;
  padding: 12px 0 8px;
}

.collection-create-btn,
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

.collection-create-btn {
  padding: 10px 14px;
}

.online-search-entry {
  width: 100%;
  text-align: left;
}

.empty-online-search-btn {
  margin-top: 12px;
}
</style>
