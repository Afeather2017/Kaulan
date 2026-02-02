<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <h3>播放器设置</h3>

      <!-- View Mode Toggle -->
      <div class="mode-toggle" @click="$emit('toggleViewMode')">
        <div class="mode-label">分类方式</div>
        <div class="mode-value">{{ viewModeLabels[viewMode] }}</div>
      </div>

      <hr class="settings-divider" />

      <!-- Volume Mode Toggle -->
      <div class="mode-toggle" @click="$emit('toggleVolumeMode')">
        <div class="mode-label">音量模式</div>
        <div class="mode-value">{{ volumeModeLabels[volumeMode] }}</div>
      </div>

      <!-- Manual Volume Panel -->
      <div v-if="volumeMode === 'manual'" class="setting-panel active">
        <div class="setting-item">
          <label class="setting-label">音量设置 (%)</label>
          <div class="slider-container">
            <input
              type="range"
              class="volume-slider"
              :model-value="manualVolume"
              @input="$emit('update:manualVolume', Number(($event.target as HTMLInputElement).value))"
              min="0"
              max="1"
              step="0.01"
            />
            <input
              type="number"
              class="volume-input"
              :model-value="manualVolumeInput"
              @input="$emit('update:manualVolumeInput', Number(($event.target as HTMLInputElement).value))"
              min="0"
              max="1"
              step="0.01"
            />
          </div>
        </div>
      </div>

      <!-- Fixed LUFS Volume Panel -->
      <div v-if="volumeMode === 'fixed'" class="setting-panel active">
        <div class="setting-item">
          <label
            class="setting-label"
            title="如果音频音量过小，那么此选项可能无法设置为目标音量大小"
          >
            目标音量 (LUFS)
          </label>
          <div class="slider-container">
            <input
              type="range"
              class="volume-slider"
              :model-value="fixedLufs"
              @input="$emit('update:fixedLufs', Number(($event.target as HTMLInputElement).value))"
              min="-100"
              max="0"
              step="1"
            />
            <input
              type="number"
              class="volume-input"
              :model-value="fixedLufsInput"
              @input="$emit('update:fixedLufsInput', Number(($event.target as HTMLInputElement).value))"
              min="-100"
              max="0"
              step="1"
            />
            <span class="suffix">LUFS</span>
          </div>
        </div>
      </div>

      <!-- Music Directory -->
      <hr class="settings-divider" />
      <div class="mode-toggle">
        <div class="mode-label">音乐目录</div>
      </div>
      <div class="setting-item">
        <div class="directory-display">{{ musicDirectory }}</div>
        <button @click="selectDirectory" class="select-directory-btn">
          更改目录
        </button>
      </div>
      <div class="setting-item">
        <button @click="updateDatabase" class="update-database-btn" :disabled="isUpdating">
          {{ isUpdating ? '更新中...' : '更新数据库' }}
        </button>
      </div>
      <div class="setting-item">
        <button @click="$emit('openUploadModal')" class="upload-music-btn">
          上传音乐文件
        </button>
      </div>

      <!-- Sleep Timer -->
      <hr class="settings-divider" />
      <div class="setting-item">
        <label class="setting-label">定时停止播放</label>

        <!-- Timer Status Display -->
        <div class="timer-status">{{ timerStatusDisplay }}</div>

        <!-- Timer Slider -->
        <div class="slider-container">
          <input
            type="range"
            class="volume-slider"
            :model-value="timerMinutes"
            @input="$emit('update:timerMinutes', Number(($event.target as HTMLInputElement).value))"
            min="0"
            max="360"
            step="1"
          />
          <input
            type="number"
            class="volume-input"
            :model-value="timerMinutesInput"
            @input="$emit('update:timerMinutesInput', Number(($event.target as HTMLInputElement).value))"
            min="0"
            max="360"
            step="1"
          />
          <span class="suffix">分钟</span>
        </div>

        <!-- Timer Presets -->
        <div class="timer-presets">
          <button
            v-for="preset in [15, 30, 45, 60]"
            :key="preset"
            class="timer-preset-btn"
            @click="$emit('setTimerPreset', preset)"
          >
            {{ preset }}分钟
          </button>
        </div>

        <!-- Timer Action Buttons -->
        <div class="timer-actions">
          <button v-if="timerActive" @click="$emit('cancelTimer')" class="cancel-timer-btn">
            取消定时
          </button>
          <button v-else @click="$emit('startTimer')" class="start-timer-btn">
            开始定时
          </button>
        </div>
      </div>

      <div class="modal-actions">
        <button @click="$emit('close')" class="confirm-btn">确认</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

type VolumeMode = 'auto' | 'manual' | 'fixed'
type ViewMode = 'folder' | 'collection'

defineProps<{
  viewMode: ViewMode
  volumeMode: VolumeMode
  manualVolume: number
  manualVolumeInput: number
  fixedLufs: number
  fixedLufsInput: number
  timerMinutes: number
  timerMinutesInput: number
  timerActive: boolean
  timerStatusDisplay: string
  viewModeLabels: Record<ViewMode, string>
  volumeModeLabels: Record<VolumeMode, string>
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'toggleViewMode'): void
  (e: 'toggleVolumeMode'): void
  (e: 'update:manualVolume', value: number): void
  (e: 'update:manualVolumeInput', value: number): void
  (e: 'update:fixedLufs', value: number): void
  (e: 'update:fixedLufsInput', value: number): void
  (e: 'update:timerMinutes', value: number): void
  (e: 'update:timerMinutesInput', value: number): void
  (e: 'setTimerPreset', minutes: number): void
  (e: 'startTimer'): void
  (e: 'cancelTimer'): void
  (e: 'directoryChanged'): void
  (e: 'databaseUpdated'): void
  (e: 'openUploadModal'): void
}>()

const musicDirectory = ref<string>('Loading...')
const isUpdating = ref<boolean>(false)

onMounted(async () => {
  try {
    const path = await invoke<string>('get_music_directory')
    musicDirectory.value = path
  } catch (error) {
    console.error('Failed to get music directory:', error)
    musicDirectory.value = 'Unknown'
  }
})

const selectDirectory = async () => {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      recursive: true,
    })

    if (selected && typeof selected === 'string') {
      await invoke('set_music_directory', { newPath: selected })
      musicDirectory.value = selected
      emit('directoryChanged')
      alert('音乐目录已更新，正在重新加载...')
      // Reload the page to refresh the data
      setTimeout(() => {
        window.location.reload()
      }, 1000)
    }
  } catch (error) {
    console.error('Failed to select directory:', error)
    alert('更改目录失败: ' + error)
  }
}

const updateDatabase = async () => {
  isUpdating.value = true
  try {
    const response = await fetch('/api/database/update', {
      method: 'POST',
    })
    const result = await response.json()
    if (result.success) {
      alert('数据库更新成功！')
      emit('databaseUpdated')
    } else {
      alert('数据库更新失败: ' + result.message)
    }
  } catch (error) {
    console.error('Failed to update database:', error)
    alert('数据库更新失败: ' + error)
  } finally {
    isUpdating.value = false
  }
}
</script>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0,0,0,0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.modal-content {
  background-color: #fff;
  padding: 25px;
  border-radius: 10px;
  width: 90%;
  max-width: 400px;
  box-shadow: 0 4px 20px rgba(0,0,0,0.15);
  max-height: 80vh;
  overflow-y: auto;
}

.modal-content h3 {
  text-align: center;
  margin-bottom: 25px;
  font-size: 22px;
  font-weight: bold;
  color: #333;
  padding-bottom: 15px;
  border-bottom: 1px solid #eee;
}

.mode-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
  padding: 12px 15px;
  background-color: #f9f9f9;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.3s;
}

.mode-toggle:hover {
  background-color: #f0f0f0;
}

.mode-label {
  font-size: 17px;
  font-weight: 600;
}

.mode-value {
  font-size: 16px;
  color: #1db954;
  font-weight: 500;
  min-width: 100px;
  text-align: right;
}

.settings-divider {
  border: none;
  border-top: 1px solid #eee;
  margin: 20px 0;
}

.setting-panel {
  background-color: #f9f9f9;
  border-radius: 8px;
  padding: 20px;
  margin-top: 15px;
  display: none;
}

.setting-panel.active {
  display: block;
}

.setting-item {
  margin-bottom: 20px;
}

.setting-item:last-child {
  margin-bottom: 0;
}

.setting-label {
  display: block;
  margin-bottom: 8px;
  font-weight: 500;
  font-size: 15px;
  color: #555;
}

.slider-container {
  display: flex;
  align-items: center;
  gap: 15px;
}

.volume-slider {
  flex: 1;
  height: 8px;
  appearance: none;
  background: #e0e0e0;
  border-radius: 4px;
  outline: none;
}

.volume-slider::-webkit-slider-thumb {
  appearance: none;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  transition: all 0.2s;
}

.volume-slider::-webkit-slider-thumb:hover {
  transform: scale(1.2);
  box-shadow: 0 0 0 4px rgba(29, 185, 84, 0.2);
}

.volume-slider::-moz-range-thumb {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  border: none;
}

.volume-input {
  width: 70px;
  padding: 8px 12px;
  border: 1px solid #ddd;
  border-radius: 5px;
  font-size: 15px;
  text-align: center;
  transition: border-color 0.2s;
}

.volume-input:focus {
  border-color: #1db954;
  outline: none;
  box-shadow: 0 0 0 2px rgba(29, 185, 84, 0.2);
}

.suffix {
  font-size: 15px;
  color: #777;
  min-width: 30px;
}

.timer-status {
  margin-bottom: 10px;
  color: #1db954;
  font-weight: 500;
}

.timer-presets {
  display: flex;
  gap: 10px;
  margin-top: 10px;
}

.timer-preset-btn {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid #ddd;
  border-radius: 5px;
  background-color: #fff;
  cursor: pointer;
  font-size: 14px;
  transition: all 0.2s;
}

.timer-preset-btn:hover {
  background-color: #1db954;
  color: white;
  border-color: #1db954;
}

.timer-actions {
  margin-top: 15px;
}

.start-timer-btn, .cancel-timer-btn {
  width: 100%;
  padding: 10px 20px;
  border: none;
  border-radius: 5px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  background-color: #1db954;
  color: white;
}

.start-timer-btn:hover, .cancel-timer-btn:hover {
  background-color: #1ed760;
}

.cancel-timer-btn {
  background-color: #e74c3c;
}

.cancel-timer-btn:hover {
  background-color: #c0392b;
}

.modal-actions {
  display: flex;
  gap: 12px;
  justify-content: center;
  margin-top: 25px;
}

.confirm-btn {
  padding: 10px 20px;
  border: none;
  border-radius: 5px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
  background-color: #1db954;
  color: white;
}

.confirm-btn:hover {
  background-color: #1ed760;
}

.directory-display {
  background-color: #f9f9f9;
  border: 1px solid #ddd;
  border-radius: 5px;
  padding: 12px;
  margin-bottom: 10px;
  font-size: 14px;
  color: #555;
  word-break: break-all;
  max-height: 100px;
  overflow-y: auto;
}

.select-directory-btn {
  width: 100%;
  padding: 10px 20px;
  border: 1px solid #ddd;
  border-radius: 5px;
  background-color: #fff;
  color: #333;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.select-directory-btn:hover {
  background-color: #f0f0f0;
  border-color: #ccc;
}

.select-directory-btn:active {
  background-color: #e0e0e0;
}

.update-database-btn {
  width: 100%;
  padding: 10px 20px;
  border: 1px solid #1db954;
  border-radius: 5px;
  background-color: #1db954;
  color: white;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.update-database-btn:hover:not(:disabled) {
  background-color: #1ed760;
}

.update-database-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.upload-music-btn {
  width: 100%;
  padding: 10px 20px;
  border: 1px solid #3498db;
  border-radius: 5px;
  background-color: #3498db;
  color: white;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.upload-music-btn:hover {
  background-color: #2980b9;
}
</style>
