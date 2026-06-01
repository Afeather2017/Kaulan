<template>
  <div class="song-list">
    <div v-if="showHeader" class="list-header">
      <button v-if="showBackButton" class="back-button" @click="$emit('back')">
        ← 返回
      </button>
      <h2>{{ title }}</h2>
      <button
        v-if="showHeaderActionButton"
        class="header-action-btn"
        @click="$emit('headerAction')"
      >
        {{ headerActionLabel }}
      </button>
      <button
        v-if="showSelectButton"
        class="select-mode-btn"
        @click="$emit('toggleSelectMode')"
      >
        {{ selectMode ? "取消勾选" : "选择" }}
      </button>
    </div>
    <div
      v-for="(song, index) in songs"
      :key="song.rowKey || `${song.id}:${song.name}`"
      class="song-item"
      :class="{ active: currentSongName === song.name }"
      @click="
        selectMode
          ? $emit('toggleSelection', song.rowKey || `${song.id}:${song.name}`)
          : $emit('play', song, index)
      "
    >
      <div v-if="selectMode" class="song-checkbox">
        <input
          type="checkbox"
          :checked="selectedSongs.has(song.rowKey || `${song.id}:${song.name}`)"
          @click.stop="
            $emit('toggleSelection', song.rowKey || `${song.id}:${song.name}`)
          "
        />
      </div>
      <div class="song-info">
        <h3>{{ song.name }}</h3>
        <span v-if="showLufs" class="song-lufs">
          {{ song.lufs !== null ? `${song.lufs} LUFS` : "-" }}
        </span>
      </div>
      <button
        v-if="!selectMode && showSongActionButton"
        class="song-action-btn"
        @click.stop="$emit('songAction', song)"
      >
        {{ songActionLabel || "⋮" }}
      </button>
    </div>

    <!-- Selection Mode Actions -->
    <div v-if="selectMode" class="selection-actions selection-actions-floating">
      <button
        v-if="showRemoveButton"
        class="action-btn remove-btn"
        @click="$emit('remove')"
      >
        从收藏夹移除
      </button>
      <button
        v-if="showAddButton"
        class="action-btn add-btn"
        @click="$emit('showAddModal')"
      >
        添加到收藏夹
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
export interface SongInfo {
  id: number;
  name: string;
  lufs: number | null;
  path: string;
  sourceLabel?: string;
  rowKey?: string;
}

withDefaults(
  defineProps<{
    title: string;
    songs: SongInfo[];
    selectMode: boolean;
    selectedSongs: Set<string>;
    currentSongName?: string;
    showRemoveButton: boolean;
    showAddButton: boolean;
    showSongActionButton?: boolean;
    songActionLabel?: string;
    showHeader?: boolean;
    showLufs?: boolean;
    showBackButton?: boolean;
    showSelectButton?: boolean;
    showHeaderActionButton?: boolean;
    headerActionLabel?: string;
  }>(),
  {
    showHeader: true,
    showLufs: false,
    showSongActionButton: false,
    songActionLabel: "⋮",
    showBackButton: true,
    showSelectButton: true,
    showHeaderActionButton: false,
    headerActionLabel: "⋮",
  },
);

defineEmits<{
  (e: "back"): void;
  (e: "toggleSelectMode"): void;
  (e: "toggleSelection", name: string): void;
  (e: "play", song: SongInfo, index: number): void;
  (e: "remove"): void;
  (e: "showAddModal"): void;
  (e: "songAction", song: SongInfo): void;
  (e: "headerAction"): void;
}>();
</script>

<style scoped>
.song-list {
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
}

.list-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: bold;
  color: #333;
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}

.back-button {
  background: none;
  border: none;
  color: #1db954;
  font-size: 14px;
  cursor: pointer;
  padding: 5px;
  white-space: nowrap;
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

.header-action-btn {
  flex: none;
  width: 38px;
  height: 38px;
  border: none;
  border-radius: 10px;
  background: #f0f0f0;
  color: #333;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}

.song-item {
  padding: 12px 15px;
  border-bottom: 1px solid #f0f0f0;
  cursor: pointer;
  display: flex;
  align-items: center;
  transition: background-color 0.2s;
}

.song-item:hover {
  background-color: #f9f9f9;
}

.song-item.active {
  color: #1db954;
}

.song-checkbox {
  margin-right: 10px;
}

.song-checkbox input[type="checkbox"] {
  width: 20px;
  height: 20px;
  cursor: pointer;
}

.song-info {
  flex: 1;
  min-width: 0;
}

.song-info h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 500;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.song-lufs {
  display: inline-block;
  margin-top: 4px;
  font-size: 13px;
  color: #999;
  font-weight: 400;
  white-space: nowrap;
}

.song-action-btn {
  flex: none;
  width: 38px;
  height: 38px;
  border: none;
  border-radius: 10px;
  background: #f0f0f0;
  color: #333;
  font-size: 20px;
  line-height: 1;
  cursor: pointer;
}

.song-action-btn:hover {
  background: #e4e4e4;
}

.selection-actions {
  padding: 20px;
  display: flex;
  gap: 10px;
  justify-content: center;
}

.selection-actions-floating {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 15px;
  background-color: #fff;
  box-shadow: 0 -2px 10px rgba(0, 0, 0, 0.1);
  z-index: 20;
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
