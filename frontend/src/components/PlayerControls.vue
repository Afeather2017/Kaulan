<template>
  <div class="player-controls">
    <div class="player-top">
      <div class="cover-thumb-wrapper" @click="$emit('toggleLyric')">
        <img v-if="coverUrl && !coverFailed"
             :src="coverUrl"
             :key="coverUrl"
             class="cover-thumb"
             @error="coverFailed = true"
             alt="" />
        <i v-else class="fas fa-music cover-placeholder-icon"></i>
      </div>
      <div class="player-info">
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
      </div>
    </div>

    <!-- Control Buttons -->
    <div class="control-buttons">
      <button class="control-btn" :title="playModeLabel" @click="$emit('togglePlayMode')">
        <i v-if="playMode === 'sequential'" class="fas fa-repeat"></i>
        <i v-else-if="playMode === 'shuffle'" class="fas fa-shuffle"></i>
        <span v-else class="play-mode-icon play-mode-icon-loop">
          <i class="fas fa-repeat"></i>
          <span class="play-mode-loop-badge">1</span>
        </span>
      </button>
      <button class="control-btn" @click="$emit('previous')">
        <i class="fas fa-step-backward"></i>
      </button>
      <button class="control-btn" @click="handlePlayPause">
        <i v-if="isPlaying" class="fas fa-pause"></i>
        <i v-else class="fas fa-play"></i>
      </button>
      <button class="control-btn" @click="$emit('next')">
        <i class="fas fa-step-forward"></i>
      </button>
      <button class="control-btn" @click="$emit('showActiveQueue')">
        <i class="fas fa-list"></i>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { getApiBase } from '@/utils/api'

const props = defineProps<{
  currentTime: number
  duration: number
  isPlaying: boolean
  playMode: 'sequential' | 'shuffle' | 'loop'
  currentSongName?: string
  songId?: number
}>()

const emit = defineEmits<{
  (e: 'seek', time: number): void
  (e: 'togglePlayMode'): void
  (e: 'previous'): void
  (e: 'play'): void
  (e: 'pause'): void
  (e: 'next'): void
  (e: 'showActiveQueue'): void
  (e: 'toggleLyric'): void
}>()

const progressBar = ref<HTMLElement | null>(null)
const coverFailed = ref(false)

const coverUrl = computed(() => {
  if (!props.songId) return null
  coverFailed.value = false
  return `${getApiBase()}/music/id/${props.songId}/cover`
})

const playModeLabel = computed(() => {
  if (props.playMode === 'sequential') return 'Sequential playback'
  if (props.playMode === 'shuffle') return 'Shuffle playback'
  return 'Single track loop'
})

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

const handlePlayPause = () => {
  if (props.isPlaying) {
    emit('pause')
    return
  }
  emit('play')
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

.player-top {
  display: flex;
  align-items: center;
  margin-bottom: 10px;
}

.cover-thumb-wrapper {
  width: 48px;
  height: 48px;
  border-radius: 4px;
  flex-shrink: 0;
  background-color: #e0e0e0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  margin-right: 10px;
}

.cover-thumb {
  width: 48px;
  height: 48px;
  object-fit: cover;
}

.cover-placeholder-icon {
  font-size: 20px;
  color: #999;
}

.player-info {
  flex: 1;
  min-width: 0;
}

.current-song {
  width: 100%;
  text-align: left;
  font-size: 14px;
  font-weight: 600;
  color: #333;
  background: none;
  border: none;
  margin-bottom: 6px;
  cursor: pointer;
  padding: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.current-song.placeholder {
  cursor: default;
  color: #888;
}

.progress-container {
  margin-bottom: 0;
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

.play-mode-icon {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.play-mode-icon-loop {
  width: 1.25em;
  height: 1.25em;
}

.play-mode-loop-badge {
  position: absolute;
  right: -0.15em;
  bottom: -0.2em;
  font-size: 0.55em;
  font-weight: 700;
  line-height: 1;
}

.control-btn:hover {
  background-color: #f0f0f0;
}
</style>
