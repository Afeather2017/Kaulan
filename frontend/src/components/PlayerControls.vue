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
    <div class="progress-bar">
      <div class="progress-time">{{ formatTime(currentTime) }}</div>
      <input
        type="range"
        min="0"
        :max="duration"
        :value="currentTime"
        @input="$emit('seek', $event)"
        class="progress-slider"
      />
      <div class="progress-time">{{ formatTime(duration) }}</div>
    </div>

    <!-- Control Buttons -->
    <div class="control-buttons">
      <button class="control-btn" @click="$emit('togglePlayMode')">
        <span v-if="playMode === 'sequential'">↻</span>
        <span v-else-if="playMode === 'shuffle'">⤮</span>
        <span v-else>①</span>
      </button>
      <button class="control-btn" @click="$emit('previous')">⏮</button>
      <button class="control-btn" @click="$emit('togglePlay')">
        <span v-if="isPlaying">⏸</span>
        <span v-else>▶</span>
      </button>
      <button class="control-btn" @click="$emit('next')">⏭</button>
      <button class="control-btn" @click="$emit('showCurrentPlaylist')">≡</button>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  currentTime: number
  duration: number
  isPlaying: boolean
  playMode: 'sequential' | 'shuffle' | 'loop'
  currentSongName?: string
}>()

defineEmits<{
  (e: 'seek', event: Event): void
  (e: 'togglePlayMode'): void
  (e: 'previous'): void
  (e: 'togglePlay'): void
  (e: 'next'): void
  (e: 'showCurrentPlaylist'): void
  (e: 'toggleLyric'): void
}>()

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

.progress-bar {
  display: flex;
  align-items: center;
  margin-bottom: 15px;
}

.progress-slider {
  flex: 1;
  height: 4px;
  background-color: #e0e0e0;
  border-radius: 2px;
  appearance: none;
  outline: none;
}

.progress-slider::-webkit-slider-thumb {
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
}

.progress-slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  border: none;
}

.progress-time {
  font-size: 12px;
  color: #888;
  min-width: 45px;
  font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
  font-variant-numeric: tabular-nums;
  text-align: center;
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

.play-btn {
  font-size: 24px;
  background: none;
  color: #333;
}
</style>
