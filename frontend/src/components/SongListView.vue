<template>
  <div ref="songListRef" class="song-list">
    <div v-if="showHeader" class="list-header">
      <template v-if="selectMode">
        <button class="back-button" @click="$emit('toggleSelectMode')">
          取消
        </button>
        <h2>已选择 {{ selectionCount }}</h2>
        <button
          v-if="selectionActionLabel"
          class="select-mode-btn"
          @click="$emit('selectionAction')"
        >
          {{ selectionActionLabel }}
        </button>
        <div v-else class="header-action-placeholder"></div>
      </template>
      <template v-else>
        <BackButton v-if="showBackButton" @back="$emit('back')" />
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
      </template>
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
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import BackButton from "@/components/BackButton.vue";
import type { MusicInfo } from "@/types/music";

withDefaults(
  defineProps<{
    title: string;
    songs: MusicInfo[];
    selectMode: boolean;
    selectedSongs: Set<string>;
    selectionCount?: number;
    selectionActionLabel?: string;
    currentSongName?: string;
    showRemoveButton: boolean;
    showAddButton: boolean;
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
  (e: "play", song: MusicInfo, index: number): void;
  (e: "remove"): void;
  (e: "showAddModal"): void;
  (e: "headerAction"): void;
  (e: "selectionAction"): void;
}>();

const songListRef = ref<HTMLDivElement | null>(null);

defineExpose({
  getScrollTop: () => songListRef.value?.scrollTop ?? 0,
  setScrollTop: (scrollTop: number) => {
    if (songListRef.value) {
      songListRef.value.scrollTop = scrollTop;
    }
  },
});
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
  min-width: 0;
  overflow-wrap: anywhere;
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

.header-action-placeholder {
  width: 80px;
  flex: none;
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
</style>
