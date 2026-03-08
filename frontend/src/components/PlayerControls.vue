<template>
  <div class="player-controls">
    <button
      v-if="currentSongName"
      class="current-song"
      @click="$emit('toggleLyric')"
    >
      {{ currentSongName }}
    </button>
    <div v-else class="current-song placeholder">
      无正在播放
    </div>
    <!-- Progress Bar -->
    <div class="progress-container">
      <div
        class="progress-bar"
        ref="progressBar"
        @click="handleSeek"
      >
        <div class="progress" :style="{ width: progressPercent + '%' }"></div>
      </div>
      <div class="play-info">
        <span class="progress-time">{{ formatTime(currentTime) }}</span>
        <span class="progress-time">{{ formatTime(duration) }}</span>
      </div>
    </div>

    <!-- Control Buttons -->
    <div class="control-buttons">
      <button class="control-btn" @click="$emit('togglePlayMode')">
        <i v-if="playMode === 'sequential'" class="fas fa-redo"></i>
        <i v-else-if="playMode === 'shuffle'" class="fas fa-random"></i>
        <i v-else class="fas fa-redo-alt"></i>
      </button>
      <button class="control-btn" @click="$emit('previous')">
        <i class="fas fa-step-backward"></i>
      </button>
      <button class="control-btn" @click="$emit('togglePlay')">
        <i v-if="isPlaying" class="fas fa-pause"></i>
        <i v-else class="fas fa-play"></i>
      </button>
      <button class="control-btn" @click="$emit('next')">
        <i class="fas fa-step-forward"></i>
      </button>
      <button class="control-btn" @click="$emit('showCurrentPlaylist')">
        <i class="fas fa-list"></i>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

const props = defineProps<{
  currentTime: number
  duration: number
  isPlaying: boolean
  playMode: 'sequential' | 'shuffle' | 'loop'
  currentSongName?: string
}>()

const emit = defineEmits<{
  (e: 'seek', time: number): void
  (e: 'togglePlayMode'): void
  (e: 'previous'): void
  (e: 'togglePlay'): void
  (e: 'next'): void
  (e: 'showCurrentPlaylist'): void
  (e: 'toggleLyric'): void
}>()

const progressBar = ref<HTMLElement | null>(null)

// Computed progress percentage
const progressPercent = computed(() => {
  if (props.duration === 0) return 0
  return (props.currentTime / props.duration) * 100
})

// Handle click on progress bar to seek
const handleSeek = (event: MouseEvent) => {
  if (!progressBar.value || props.duration === 0) return

  const rect = progressBar.value.getBoundingClientRect()
  const clickX = event.clientX - rect.left
  const percent = Math.max(0, Math.min(1, clickX / rect.width))
  const seekTime = percent * props.duration

  emit('seek', seekTime)
}

const formatTime = (seconds: number) => {
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}
</script>

<style scoped>
.player-controls {
  background-color: #fff;
  border-top: 1px solid #eee;
  padding: 15px;
  box-shadow: 0 -2px 10px rgba(0,0,0,0.05);
  position: relative;
  z-index: 10;
}

.current-song {
  width: 100%;
  text-align: center;
  font-size: 14px;
  font-weight: 600;
  color: #333;
  background: none;
  border: none;
  margin-bottom: 10px;
  cursor: pointer;
  padding: 4px 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.current-song.placeholder {
  cursor: default;
  color: #888;
}

.progress-container {
  margin-bottom: 15px;
}

.progress-bar {
  width: 100%;
  height: 4px;
  background-color: #e0e0e0;
  border-radius: 2px;
  overflow: hidden;
  cursor: pointer;
}

.progress {
  height: 100%;
  background-color: #1db954;
  width: 0%;
}

.play-info {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: #888;
  margin-top: 5px;
}

.progress-time {
  font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
  font-variant-numeric: tabular-nums;
}

.control-buttons {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.control-btn {
  background: none;
  border: none;
  font-size: 20px;
  cursor: pointer;
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #333;
  transition: background-color 0.2s;
}

.control-btn:hover {
  background-color: #f0f0f0;
}
</style>
