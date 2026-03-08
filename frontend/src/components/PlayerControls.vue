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
        :value="sliderValue"
        @input="handleSeekInput"
        @change="handleSeekChange"
        class="progress-slider"
        :style="progressStyle"
      />
      <div class="progress-time">{{ formatTime(duration) }}</div>
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
import { ref, watch, computed } from 'vue'

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

// Local ref for slider value to prevent reactivity conflicts during dragging
const sliderValue = ref(props.currentTime)
const isDragging = ref(false)

// Sync slider value with currentTime when not dragging
watch(() => props.currentTime, (newTime) => {
  if (!isDragging.value) {
    sliderValue.value = newTime
  }
})

const handleSeekInput = (event: Event) => {
  isDragging.value = true
  const target = event.target as HTMLInputElement
  sliderValue.value = parseFloat(target.value)
}

const handleSeekChange = (event: Event) => {
  isDragging.value = false
  const target = event.target as HTMLInputElement
  const time = parseFloat(target.value)
  emit('seek', time)
}

// Computed style for progress bar gradient (green for played, gray for unplayed)
const progressStyle = computed(() => {
  if (props.duration === 0) return {}
  const percentage = (sliderValue.value / props.duration) * 100
  return {
    background: `linear-gradient(to right, #1db954 0%, #1db954 ${percentage}%, #e0e0e0 ${percentage}%, #e0e0e0 100%)`
  }
})

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
  border-radius: 2px;
  appearance: none;
  outline: none;
  cursor: pointer;
}

/* Hide the thumb (round indicator) */
.progress-slider::-webkit-slider-thumb {
  appearance: none;
  width: 0;
  height: 0;
  opacity: 0;
  cursor: pointer;
}

.progress-slider::-moz-range-thumb {
  width: 0;
  height: 0;
  opacity: 0;
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
