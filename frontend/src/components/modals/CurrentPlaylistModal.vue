<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <div class="modal-header">
        <h3>{{ playlist?.name || '当前播放列表' }}</h3>
        <button class="close-btn" @click="$emit('close')">
          <i class="fas fa-times"></i>
        </button>
      </div>

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
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.modal-content {
  background-color: #fff;
  border-radius: 10px;
  width: 90%;
  max-width: 500px;
  max-height: 70vh;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 20px 25px;
  border-bottom: 1px solid #eee;
}

.modal-header h3 {
  margin: 0;
  font-size: 20px;
  font-weight: bold;
  color: #333;
}

.close-btn {
  width: 32px;
  height: 32px;
  border: none;
  background: #f0f0f0;
  border-radius: 50%;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #666;
  transition: all 0.2s;
}

.close-btn:hover {
  background-color: #e0e0e0;
  color: #333;
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
