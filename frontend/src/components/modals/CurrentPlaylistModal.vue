<template>
  <div class="playlist-panel">
    <div class="panel-transparent-top"></div>
    <div class="playlist-panel-content">
      <div class="panel-top-bar">
        <button class="top-back-btn" @click="$emit('close')">
          <i class="fas fa-arrow-left"></i>
          返回
        </button>
        <h3 class="panel-title">{{ playlist?.name || '当前播放列表' }}</h3>
      </div>
      <div class="panel-body">
        <div v-if="playlist && playlist.songs.length > 0" class="song-list">
          <div
            v-for="(song, index) in playlist.songs"
            :key="song.name"
            class="song-item"
            :class="{ 'active': currentSongName === song.name }"
            @click="$emit('play', song, index)"
          >
            <div class="song-index">
              <i v-if="currentSongName === song.name" class="fas fa-volume-up"></i>
              <span v-else>{{ index + 1 }}</span>
            </div>
            <div class="song-info">
              <h4>{{ song.name }}</h4>
              <p>LUFS: {{ song.lufs.toFixed(2) }}</p>
            </div>
          </div>
        </div>

        <div v-else class="empty-state">
          <i class="fas fa-music"></i>
          <p>暂无歌曲</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { SongInfo } from '@/components/SongListView.vue'

export interface Playlist {
  name: string
  songs: SongInfo[]
}

defineProps<{
  playlist: Playlist | null
  currentSongName?: string
}>()

defineEmits<{
  (e: 'close'): void
  (e: 'play', song: SongInfo, index: number): void
}>()
</script>

<style scoped>
.playlist-panel {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  width: 100%;
  background-color: transparent;
  z-index: 100;
  animation: slideIn 0.3s ease-out;
  display: flex;
  flex-direction: column;
}

@keyframes slideIn {
  from {
    transform: translateY(100%);
  }
  to {
    transform: translateY(0);
  }
}

.playlist-panel-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  background-color: #fff;
  border-top: 1px solid #eee;
}

.panel-transparent-top {
  flex: none;
  height: 30vh;
  background-color: transparent;
  pointer-events: none;
}

.panel-top-bar {
  flex: none;
  padding: 12px 20px;
  border-bottom: 1px solid #eee;
  display: flex;
  align-items: center;
  gap: 12px;
  background-color: #fff;
}

.top-back-btn {
  border: 1px solid #ddd;
  background-color: #f8f8f8;
  color: #333;
  font-size: 15px;
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  border-radius: 999px;
  padding: 6px 12px;
  transition: all 0.2s;
}

.top-back-btn:hover {
  background-color: #f0f0f0;
  border-color: #ccc;
}

.panel-title {
  margin: 0;
  flex: 1;
  font-size: 18px;
  font-weight: 600;
  color: #333;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.panel-body {
  flex: 1;
  padding: 0 20px 20px;
  display: flex;
  flex-direction: column;
}

.song-list {
  flex: 1;
  overflow-y: auto;
  padding: 10px 0;
}

.song-item {
  display: flex;
  align-items: center;
  padding: 12px 25px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.song-item:hover {
  background-color: #f9f9f9;
}

.song-item.active {
  background-color: #f0fff4;
  color: #1db954;
}

.song-item.active h4 {
  color: #1db954;
}

.song-index {
  width: 30px;
  text-align: center;
  font-size: 14px;
  color: #888;
  margin-right: 15px;
}

.song-index i {
  font-size: 16px;
}

.song-info {
  flex: 1;
  min-width: 0;
}

.song-info h4 {
  margin: 0 0 4px 0;
  font-size: 15px;
  font-weight: 500;
  color: #333;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.song-info p {
  margin: 0;
  color: #888;
  font-size: 12px;
}

.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  color: #888;
}

.empty-state i {
  font-size: 48px;
  margin-bottom: 15px;
  opacity: 0.5;
}

.empty-state p {
  font-size: 16px;
  margin: 0;
}
</style>
