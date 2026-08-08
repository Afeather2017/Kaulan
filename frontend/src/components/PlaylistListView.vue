<template>
  <div class="playlist-list">
    <div v-if="showHeader" class="list-header">
      <h2>{{ title }}</h2>
      <button
        v-if="showSelectButton"
        class="select-mode-btn"
        @click="$emit('toggleSelectMode')"
      >
        {{ selectMode ? "取消勾选" : "选择" }}
      </button>
    </div>
    <div
      v-for="playlistName in playlistNames"
      :key="playlistName"
      class="playlist-item"
    >
      <button
        class="playlist-main-button"
        @click="
          selectMode
            ? $emit('toggleSelection', playlistName)
            : $emit('select', playlistName)
        "
      >
        <div v-if="selectMode" class="playlist-checkbox">
          <input
            type="checkbox"
            :checked="selectedPlaylists.has(playlistName)"
            @click.stop="$emit('toggleSelection', playlistName)"
          />
        </div>
        <div class="playlist-cover">
          <i class="fas fa-music"></i>
        </div>
        <div class="playlist-info">
          <h3>{{ playlistName }}</h3>
          <p>{{ playlists[playlistName]?.length || 0 }} 首歌曲</p>
        </div>
      </button>
      <button
        v-if="!selectMode && showPlaylistActionButton"
        class="playlist-action-btn"
        @click.stop="$emit('playlistAction', playlistName)"
      >
        {{ playlistActionLabel }}
      </button>
    </div>

    <!-- Collection Management Actions (floating) -->
    <div v-if="selectMode" class="collection-actions-floating">
      <button class="action-btn add-btn" @click="$emit('showCreateModal')">
        添加收藏夹
      </button>
      <button
        v-if="hasSelectedNonAllMusic"
        class="action-btn remove-btn"
        @click="$emit('deleteSelected')"
      >
        删除收藏夹
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    title: string;
    viewMode: "folder" | "collection";
    playlistNames: string[];
    playlists: Record<string, any[]>;
    selectMode: boolean;
    selectedPlaylists: Set<string>;
    showSelectButton: boolean;
    hasSelectedNonAllMusic: boolean;
    showHeader?: boolean;
    showPlaylistActionButton?: boolean;
    playlistActionLabel?: string;
  }>(),
  {
    showHeader: true,
    showPlaylistActionButton: false,
    playlistActionLabel: "⋮",
  },
);

defineEmits<{
  (e: "toggleSelectMode"): void;
  (e: "toggleSelection", name: string): void;
  (e: "select", name: string): void;
  (e: "showCreateModal"): void;
  (e: "deleteSelected"): void;
  (e: "playlistAction", name: string): void;
}>();
</script>

<style scoped>
.playlist-list {
  height: 100%;
  overflow-y: auto;
}

.list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 0;
  font-size: 18px;
  font-weight: bold;
  color: #333;
  border-bottom: 1px solid #eee;
  margin-bottom: 10px;
  gap: 10px;
  position: sticky;
  top: 0;
  background: #fff;
  z-index: 5;
}

.list-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: bold;
  color: #333;
  flex: 1;
}

.select-mode-btn {
  background-color: #1db954;
  color: white;
  border: none;
  border-radius: 5px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 14px;
  white-space: nowrap;
}

.select-mode-btn:hover {
  background-color: #1ed760;
}

.playlist-item {
  display: flex;
  align-items: center;
  gap: 10px;
  border-bottom: 1px solid #f0f0f0;
}

.playlist-main-button {
  flex: 1;
  padding: 12px 15px;
  cursor: pointer;
  display: flex;
  align-items: center;
  width: 100%;
  border: none;
  background: transparent;
  transition: background-color 0.2s;
  text-align: left;
}

.playlist-main-button:hover {
  background-color: #f9f9f9;
}

.playlist-cover {
  width: 40px;
  height: 40px;
  background-color: #e0e0e0;
  border-radius: 5px;
  margin-right: 15px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #666;
}

.playlist-checkbox {
  margin-right: 10px;
}

.playlist-checkbox input[type="checkbox"] {
  width: 20px;
  height: 20px;
  cursor: pointer;
}

.playlist-info {
  flex: 1;
  min-width: 0;
}

.playlist-info h3 {
  margin: 0 0 5px 0;
  font-size: 16px;
  font-weight: 500;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.playlist-info p {
  margin: 0;
  color: #666;
  font-size: 12px;
}

.playlist-action-btn {
  flex: none;
  width: 38px;
  height: 38px;
  margin-right: 12px;
  border: none;
  border-radius: 10px;
  background: #f0f0f0;
  color: #333;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}

.collection-actions-floating {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 15px;
  background-color: #fff;
  box-shadow: 0 -2px 10px rgba(0, 0, 0, 0.1);
  z-index: 20;
  display: flex;
  gap: 10px;
  justify-content: center;
}

.action-btn {
  padding: 12px 24px;
  border: none;
  border-radius: 8px;
  font-size: 16px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.add-btn {
  background-color: #1db954;
  color: white;
}

.add-btn:hover {
  background-color: #1ed760;
}

.remove-btn {
  background-color: #e74c3c;
  color: white;
}

.remove-btn:hover {
  background-color: #c0392b;
}
</style>
