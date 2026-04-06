<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <div class="modal-top-bar">
        <button class="top-back-btn" @click="$emit('close')">
          <i class="fas fa-arrow-left"></i>
          返回
        </button>
      </div>
      <div class="modal-body">
        <h3>播放器设置</h3>

        <!-- Device Discovery -->
        <div class="mode-toggle">
          <div class="mode-label">设备发现</div>
        </div>

        <!-- Device Name Setting -->
        <div class="setting-item">
          <label class="setting-label">设备名称</label>
          <div class="url-input-container">
            <input
              type="text"
              class="url-input"
              :value="deviceNameInput"
              @input="deviceNameInput = ($event.target as HTMLInputElement).value"
              placeholder="My Kaulan Player"
              maxlength="64"
            />
          </div>
          <div class="url-actions">
            <button
              @click="saveDeviceName"
              class="save-url-btn"
              :disabled="isSavingDeviceName"
            >
              {{ isSavingDeviceName ? '保存中...' : '保存名称' }}
            </button>
          </div>
        </div>

        <!-- Discovered Devices -->
        <div class="setting-item">
          <label class="setting-label">局域网中的设备</label>
          <div v-if="isLoadingDevices" class="loading-state">
            扫描中...
          </div>
          <div v-else-if="displayDevices.length === 0" class="empty-state">
            未发现其他设备
          </div>
          <div v-else class="device-list">
            <div
              v-for="device in displayDevices"
              :key="device.device_id"
              class="device-item"
              @click="connectToDevice(device)"
            >
              <div class="device-info">
                <div class="device-header">
                  <div class="device-name-row">
                    <div class="device-name">{{ device.device_name }}</div>
                    <span v-if="device.isManual" class="manual-badge">手动添加</span>
                  </div>
                  <div class="device-actions">
                    <div class="device-last-seen">
                      {{ isLocalhostDevice(device) ? '本机' : formatLastSeen(device.last_seen_secs_ago) }}
                    </div>
                    <button
                      v-if="device.isManual"
                      class="remove-device-btn"
                      @click.stop="removeManualDevice(device.api_url)"
                    >
                      <i class="fas fa-times"></i>
                    </button>
                  </div>
                </div>
                <div class="device-url">{{ device.api_url }}</div>
              </div>
            </div>
          </div>
          <div class="url-actions">
            <button @click="refreshDevices" class="refresh-devices-btn">
              刷新设备
            </button>
            <button @click="openManualAddressDialog" class="manual-url-btn">
              手动指定地址
            </button>
          </div>
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
            <label class="setting-label">音量设置</label>
            <div class="slider-container">
              <input
                type="range"
                class="volume-slider"
                :model-value="manualVolume"
                @input="handleManualVolumeSlider"
                min="0"
                max="1"
                step="0.01"
              />
              <input
                type="text"
                class="value-input"
                :value="manualVolumeDisplay"
                @input="handleManualVolumeInput"
                @blur="handleManualVolumeBlur"
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
              目标音量
            </label>
            <div class="slider-container">
              <input
                type="range"
                class="volume-slider"
                :model-value="fixedLufs"
                @input="handleFixedLufsSlider"
                min="-100"
                max="0"
                step="1"
              />
              <input
                type="text"
                class="value-input"
                :value="fixedLufsDisplay"
                @input="handleFixedLufsInput"
                @blur="handleFixedLufsBlur"
              />
            </div>
          </div>
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
              :model-value="Math.min(timerMinutes, 120)"
              @input="handleTimerMinutesSlider"
              min="1"
              max="120"
              step="1"
            />
            <input
              type="text"
              class="value-input"
              :value="timerMinutesDisplay"
              @input="handleTimerMinutesInput"
              @blur="handleTimerMinutesBlur"
            />
            <span class="value-suffix">分钟</span>
          </div>

          <!-- Timer Presets -->
          <div class="timer-presets">
            <button
              v-for="preset in [15, 30, 60]"
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

        <!-- View Mode Toggle -->
        <hr class="settings-divider" />
        <div class="mode-toggle" @click="$emit('toggleViewMode')">
          <div class="mode-label">分类方式</div>
          <div class="mode-value">{{ viewModeLabels[viewMode] }}</div>
        </div>

        <!-- Display Settings -->
        <hr class="settings-divider" />
        <div class="mode-toggle">
          <div class="mode-label">显示设置</div>
        </div>
        <div class="setting-item">
          <label class="checkbox-label">
            <input
              type="checkbox"
              :checked="showLufsLocal"
              @change="handleShowLufsChange"
              class="setting-checkbox"
            />
            <span>显示 LUFS 值</span>
          </label>
        </div>

        <!-- Android: Local Lyrics Setting -->
        <div v-if="isAndroid" class="setting-item">
          <label class="checkbox-label">
            <input
              type="checkbox"
              :checked="useLocalLyrics"
              @change="handleUseLocalLyricsChange"
              :disabled="isRequestingPermission"
              class="setting-checkbox"
            />
            <span>使用本地歌词</span>
          </label>
          <p v-if="permissionStatus" class="setting-hint" :class="{ 'setting-error': !permissionGranted }">
            {{ permissionStatus }}
          </p>
        </div>

        <hr class="settings-divider" />
        <div class="mode-toggle">
          <div class="mode-label">媒体类型过滤</div>
        </div>
        <div class="setting-item">
          <label class="checkbox-label">
            <input
              type="checkbox"
              class="setting-checkbox"
              :checked="selectedMediaTypes.includes('audio')"
              :disabled="isMediaTypeDisabled('audio') || isSavingMediaTypes || isLoadingMediaTypes"
              @change="toggleMediaType('audio', ($event.target as HTMLInputElement).checked)"
            />
            <span>扫描音频文件</span>
          </label>
        </div>
        <div class="setting-item">
          <label class="checkbox-label">
            <input
              type="checkbox"
              class="setting-checkbox"
              :checked="selectedMediaTypes.includes('video')"
              :disabled="isMediaTypeDisabled('video') || isSavingMediaTypes || isLoadingMediaTypes"
              @change="toggleMediaType('video', ($event.target as HTMLInputElement).checked)"
            />
            <span>扫描视频文件</span>
          </label>
          <p class="setting-hint warning-hint">
            视频文件不会执行 LUFS 音量标准化，保存后需要重新扫描数据库才会生效。
          </p>
          <p v-if="mediaTypesMessage" class="setting-hint" :class="{ 'setting-error': mediaTypesError }">
            {{ mediaTypesMessage }}
          </p>
          <div class="url-actions">
            <button
              @click="saveMediaTypes"
              class="save-url-btn"
              :disabled="isSavingMediaTypes || isLoadingMediaTypes"
            >
              {{ isSavingMediaTypes ? '保存中...' : '保存媒体类型' }}
            </button>
          </div>
        </div>

        <div class="modal-actions">
          <button @click="$emit('close')" class="confirm-btn">确认</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { getApiBase, normalizeApiBase, setApiBase } from '@/utils/api'
import { validateServerUrl } from '@/utils/validation'
import { getMediaTypes, setMediaTypes, setShowLufs } from '@/utils/storage'
import { useDeviceDiscovery, type DiscoveredDevice } from '@/composables/useDeviceDiscovery'

type VolumeMode = 'auto' | 'manual' | 'fixed'
type ViewMode = 'folder' | 'collection'

const props = defineProps<{
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
  showLufs: boolean
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
  (e: 'databaseUpdateStart'): void
  (e: 'databaseUpdateEnd'): void
  (e: 'openUploadModal'): void
  (e: 'update:showLufs', value: boolean): void
}>()

const musicDirectory = ref<string>('Loading...')
const isUpdating = ref<boolean>(false)

// Server URL configuration state
const serverUrlInput = ref<string>('')
const serverUrlError = ref<string>('')
const isSavingServerUrl = ref<boolean>(false)
const serverUrlValid = ref<boolean>(true)

// Device discovery state
const {
  devices: discoveredDevices,
  selfDevice,
  isLoading: isLoadingDevices,
  fetchDevices,
  refreshDevices: runDiscoveryRefresh,
  fetchSelfDevice,
  setDeviceName,
  connectToDevice,
  formatLastSeen,
} = useDeviceDiscovery()

const deviceNameInput = ref<string>('')
const isSavingDeviceName = ref<boolean>(false)
const LOCALHOST_API_URL = 'http://localhost:2080/api'

// Manual devices storage (localStorage)
const MANUAL_DEVICES_KEY = 'kaulan_manual_devices'

interface ManualDevice {
  api_url: string
  device_name?: string
  added_at: number
  last_fetched?: number
}

const manualDevices = ref<ManualDevice[]>([])

// Fetch device name from a manual device by calling its API
const fetchDeviceName = async (url: string): Promise<string | null> => {
  try {
    const normalizedUrl = normalizeApiBase(url)
    const response = await fetch(`${normalizedUrl}/discovery/self`)
    if (response.ok) {
      const data = await response.json()
      return data.device_name || null
    }
  } catch (e) {
    console.warn(`Failed to fetch device name from ${url}:`, e)
  }
  return null
}

// Refresh names for all manual devices
const refreshManualDeviceNames = async () => {
  const promises = manualDevices.value.map(async (device) => {
    const name = await fetchDeviceName(device.api_url)
    if (name) {
      device.device_name = name
      device.last_fetched = Date.now()
    }
  })
  await Promise.all(promises)
  saveManualDevices()
}

const loadManualDevices = () => {
  try {
    const stored = localStorage.getItem(MANUAL_DEVICES_KEY)
    if (stored) {
      manualDevices.value = JSON.parse(stored)
      // Refresh device names in background
      refreshManualDeviceNames()
    }
  } catch (e) {
    console.error('Failed to load manual devices:', e)
  }
}

const saveManualDevices = () => {
  try {
    localStorage.setItem(MANUAL_DEVICES_KEY, JSON.stringify(manualDevices.value))
  } catch (e) {
    console.error('Failed to save manual devices:', e)
  }
}

const addManualDevice = async (url: string) => {
  const normalizedUrl = normalizeApiBase(url)

  // Check if already exists
  const existing = manualDevices.value.find(m => m.api_url === normalizedUrl)
  if (existing) {
    return // Already exists, don't add duplicate
  }

  // Fetch device name
  const deviceName = await fetchDeviceName(normalizedUrl)

  manualDevices.value.push({
    api_url: normalizedUrl,
    device_name: deviceName || undefined,
    added_at: Date.now(),
    last_fetched: deviceName ? Date.now() : undefined
  })
  saveManualDevices()
}

const removeManualDevice = (url: string) => {
  manualDevices.value = manualDevices.value.filter(m => m.api_url !== url)
  saveManualDevices()
}

// Local show LUFS setting (synced with prop)
const showLufsLocal = ref<boolean>(props.showLufs)
const selectedMediaTypes = ref<string[]>(getMediaTypes())
const isLoadingMediaTypes = ref<boolean>(false)
const isSavingMediaTypes = ref<boolean>(false)
const mediaTypesMessage = ref<string>('')
const mediaTypesError = ref<boolean>(false)

// Watch for prop changes (from parent/App.vue)
watch(() => props.showLufs, (newValue) => {
  showLufsLocal.value = newValue
})

// Handle show LUFS checkbox change
const handleShowLufsChange = (e: Event) => {
  const checked = (e.target as HTMLInputElement).checked
  showLufsLocal.value = checked
  setShowLufs(checked)
  emit('update:showLufs', checked)
}

// Local lyrics setting (Android only)
const useLocalLyrics = ref<boolean>(false)
const isRequestingPermission = ref<boolean>(false)
const permissionGranted = ref<boolean>(false)
const permissionStatus = ref<string>('')

// Check if running on Android
const isAndroid = ref<boolean>(false)

// Handle use local lyrics checkbox change
const handleUseLocalLyricsChange = async (e: Event) => {
  const checked = (e.target as HTMLInputElement).checked

  if (!checked) {
    useLocalLyrics.value = false
    permissionStatus.value = ''
    return
  }

  isRequestingPermission.value = true
  permissionStatus.value = '正在请求权限...'

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const granted = await invoke<boolean>('request_external_storage_permission')

    permissionGranted.value = granted
    useLocalLyrics.value = granted

    if (granted) {
      permissionStatus.value = '权限已授予，可以读取本地歌词文件'
    } else {
      permissionStatus.value = '权限未授予，无法读取本地歌词文件'
      useLocalLyrics.value = false
    }
  } catch (error) {
    console.error('Failed to request external storage permission:', error)
    permissionStatus.value = '请求权限失败: ' + error
    useLocalLyrics.value = false
  } finally {
    isRequestingPermission.value = false
  }
}

const sortMediaTypes = (mediaTypes: string[]): string[] => {
  return ['audio', 'video'].filter(type => mediaTypes.includes(type))
}

const isMediaTypeDisabled = (mediaType: string): boolean => {
  return selectedMediaTypes.value.length === 1 && selectedMediaTypes.value.includes(mediaType)
}

const toggleMediaType = (mediaType: string, enabled: boolean) => {
  const next = new Set(selectedMediaTypes.value)

  if (enabled) {
    next.add(mediaType)
  } else if (!(next.size === 1 && next.has(mediaType))) {
    next.delete(mediaType)
  }

  selectedMediaTypes.value = sortMediaTypes(Array.from(next))
  mediaTypesMessage.value = ''
  mediaTypesError.value = false
}

const loadMediaTypes = async () => {
  isLoadingMediaTypes.value = true
  mediaTypesMessage.value = ''
  mediaTypesError.value = false

  try {
    const response = await fetch(`${getApiBase()}/settings/media-types`)
    if (!response.ok) {
      throw new Error(`Request failed with status ${response.status}`)
    }

    const data = await response.json()
    const mediaTypes = Array.isArray(data.media_types) ? sortMediaTypes(data.media_types) : ['audio']
    selectedMediaTypes.value = mediaTypes.length > 0 ? mediaTypes : ['audio']
    setMediaTypes(selectedMediaTypes.value)
  } catch (error) {
    console.error('Failed to load media types:', error)
    selectedMediaTypes.value = sortMediaTypes(getMediaTypes())
    mediaTypesMessage.value = '读取媒体类型失败，已使用本地缓存。'
    mediaTypesError.value = true
  } finally {
    isLoadingMediaTypes.value = false
  }
}

const saveMediaTypes = async () => {
  isSavingMediaTypes.value = true
  mediaTypesMessage.value = ''
  mediaTypesError.value = false

  try {
    const response = await fetch(`${getApiBase()}/settings/media-types`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ media_types: selectedMediaTypes.value }),
    })
    const result = await response.json()

    if (!response.ok || !result.success) {
      throw new Error(result.message || `Request failed with status ${response.status}`)
    }

    setMediaTypes(selectedMediaTypes.value)
    mediaTypesMessage.value = '媒体类型已保存，重新扫描数据库后生效。'
  } catch (error) {
    console.error('Failed to save media types:', error)
    mediaTypesMessage.value = `保存媒体类型失败: ${error}`
    mediaTypesError.value = true
  } finally {
    isSavingMediaTypes.value = false
  }
}

const localhostDevice = computed<DiscoveredDevice>(() => ({
  device_id: 'localhost-self',
  device_name: 'localhost(self)',
  api_url: LOCALHOST_API_URL,
  last_seen_secs_ago: 0,
}))

const displayDevices = computed<DiscoveredDevice[]>(() => {
  const manualDeviceEntries: DiscoveredDevice[] = manualDevices.value.map((m, idx) => ({
    device_id: `manual-${idx}-${m.added_at}`,
    device_name: m.device_name || '手动添加',
    api_url: m.api_url,
    last_seen_secs_ago: 0,
    isManual: true
  }))

  const merged = [localhostDevice.value, ...discoveredDevices.value, ...manualDeviceEntries]
  const unique = new Map<string, DiscoveredDevice>()
  for (const device of merged) {
    if (!unique.has(device.api_url)) {
      unique.set(device.api_url, device)
    }
  }
  return Array.from(unique.values())
})

const isLocalhostDevice = (device: DiscoveredDevice): boolean => device.api_url === LOCALHOST_API_URL

// Temporary state for user input (before blur/commit)
const manualVolumeInputTemp = ref('')
const fixedLufsInputTemp = ref('')
const timerMinutesInputTemp = ref('')

// Computed display values
const manualVolumeDisplay = computed(() => `${Math.round(props.manualVolume * 100)}%`)
const fixedLufsDisplay = computed(() => `${props.fixedLufs} LUFS`)
const timerMinutesDisplay = computed(() => `${props.timerMinutes}`)

// Manual Volume handlers
const handleManualVolumeSlider = (e: Event) => {
  const value = Number((e.target as HTMLInputElement).value)
  emit('update:manualVolume', value)
}

const handleManualVolumeInput = (e: Event) => {
  manualVolumeInputTemp.value = (e.target as HTMLInputElement).value
}

const handleManualVolumeBlur = () => {
  let valueStr = manualVolumeInputTemp.value.trim()
  // Remove % suffix if present
  if (valueStr.endsWith('%')) {
    valueStr = valueStr.slice(0, -1)
  }
  const value = Number(valueStr)
  if (!isNaN(value) && value >= 0 && value <= 100) {
    emit('update:manualVolume', value / 100)
  }
  // Reset temp to current display value
  manualVolumeInputTemp.value = ''
}

// Fixed LUFS handlers
const handleFixedLufsSlider = (e: Event) => {
  const value = Number((e.target as HTMLInputElement).value)
  emit('update:fixedLufs', value)
}

const handleFixedLufsInput = (e: Event) => {
  fixedLufsInputTemp.value = (e.target as HTMLInputElement).value
}

const handleFixedLufsBlur = () => {
  let valueStr = fixedLufsInputTemp.value.trim()
  // Remove "LUFS" suffix if present
  if (valueStr.endsWith('LUFS') || valueStr.endsWith('lufs')) {
    valueStr = valueStr.slice(0, -4).trim()
  }
  const value = Number(valueStr)
  if (!isNaN(value) && value >= -100 && value <= 0) {
    emit('update:fixedLufs', value)
  }
  // Reset temp to current display value
  fixedLufsInputTemp.value = ''
}

// Timer handlers
const handleTimerMinutesSlider = (e: Event) => {
  const value = Number((e.target as HTMLInputElement).value)
  emit('update:timerMinutes', value)
}

const handleTimerMinutesInput = (e: Event) => {
  timerMinutesInputTemp.value = (e.target as HTMLInputElement).value
}

const handleTimerMinutesBlur = () => {
  let valueStr = timerMinutesInputTemp.value.trim()
  // Remove Chinese time units if present
  valueStr = valueStr.replace(/分钟/g, '').replace(/小时/g, ' ').trim()

  // Parse hours if present
  const parts = valueStr.split(' ')
  let totalMinutes = 0
  if (parts.length === 2) {
    const hours = Number(parts[0])
    const mins = Number(parts[1])
    if (!isNaN(hours)) totalMinutes += hours * 60
    if (!isNaN(mins)) totalMinutes += mins
  } else {
    totalMinutes = Number(valueStr)
  }

  // Allow any positive value (slider is limited to 1-120, but manual input can be larger)
  if (!isNaN(totalMinutes) && totalMinutes >= 1) {
    emit('update:timerMinutes', Math.min(totalMinutes, 999))
  }
  // Reset temp to current display value
  timerMinutesInputTemp.value = ''
}

onMounted(async () => {
  // Initialize server URL input
  serverUrlInput.value = getApiBase()

  // Load manual devices from localStorage
  loadManualDevices()

  // Initialize device name from localStorage first (local device's name)
  const localDeviceName = localStorage.getItem('kaulan_local_device_name')
  if (localDeviceName) {
    deviceNameInput.value = localDeviceName
  } else {
    // Fallback: fetch from current API and save as local device name
    await fetchSelfDevice()
    if (selfDevice.value) {
      deviceNameInput.value = selfDevice.value.device_name
      localStorage.setItem('kaulan_local_device_name', selfDevice.value.device_name)
    }
  }

  // Load current committed discovery list.
  try {
    await fetchDevices()
  } catch (err) {
    console.error('Failed to load discovered devices:', err)
  }

  try {
    const response = await fetch(`${getApiBase()}/settings/music-directory`)
    if (response.ok) {
      const data = await response.json()
      musicDirectory.value = data.path
    } else {
      console.error('Failed to get music directory')
      musicDirectory.value = 'Unknown'
    }
  } catch (error) {
    console.error('Failed to get music directory:', error)
    musicDirectory.value = 'Unknown'
  }

  await loadMediaTypes()

  // Check if running on Android
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const platform = await invoke<string>('get_platform')
    isAndroid.value = platform === 'android'
  } catch {
    isAndroid.value = false
  }

  // Load local lyrics setting (Android only) - query actual permission state
  if (isAndroid.value) {
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const granted = await invoke<boolean>('check_external_storage_permission')
      useLocalLyrics.value = granted
      if (granted) {
        permissionStatus.value = '本地歌词已启用'
      }
    } catch (e) {
      console.error('Failed to check permission status:', e)
    }
  }
})

const selectDirectory = async () => {
  // For now, use the same prompt approach for all platforms
  // SAF integration on Android requires additional work to bypass ACL restrictions
  let newPath = prompt('请输入新的音乐目录路径:', musicDirectory.value)

  if (!newPath || newPath.trim() === '') {
    return
  }

  try {
    const response = await fetch(`${getApiBase()}/settings/music-directory`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ path: newPath.trim() }),
    })

    if (response.ok) {
      const result = await response.json()
      if (result.success) {
        musicDirectory.value = newPath.trim()
        emit('directoryChanged')
        alert('音乐目录已更新，正在重新加载...')
        // Reload the page to refresh the data
        setTimeout(() => {
          window.location.reload()
        }, 1000)
      } else {
        alert('更改目录失败: ' + result.message)
      }
    } else {
      const error = await response.json()
      alert('更改目录失败: ' + error.message)
    }
  } catch (error) {
    console.error('Failed to select directory:', error)
    alert('更改目录失败: ' + error)
  }
}

const updateDatabase = async () => {
  isUpdating.value = true
  emit('databaseUpdateStart')
  try {
    const response = await fetch(`${getApiBase()}/database/update`, {
      method: 'POST',
    })
    const result = await response.json()
    if (result.success) {
      alert('数据库更新成功，正在重新加载...')
      // Reload the page to refresh all data
      setTimeout(() => {
        window.location.reload()
      }, 1000)
    } else {
      alert('数据库更新失败: ' + result.message)
    }
  } catch (error) {
    console.error('Failed to update database:', error)
    alert('数据库更新失败: ' + error)
  } finally {
    isUpdating.value = false
    emit('databaseUpdateEnd')
  }
}

// Server URL handlers
const saveServerUrl = async () => {
  const validation = validateServerUrl(serverUrlInput.value)
  if (!validation.valid) {
    serverUrlError.value = validation.error || 'Invalid URL'
    serverUrlValid.value = false
    return
  }

  isSavingServerUrl.value = true
  serverUrlError.value = ''
  serverUrlValid.value = true

  try {
    setApiBase(serverUrlInput.value)
    alert('服务器地址已保存，正在重新加载...')
    setTimeout(() => {
      window.location.reload()
    }, 500)
  } catch (error) {
    serverUrlError.value = '保存失败: ' + error
    serverUrlValid.value = false
  } finally {
    isSavingServerUrl.value = false
  }
}

// Device discovery functions
const saveDeviceName = async () => {
  if (!deviceNameInput.value.trim()) return

  isSavingDeviceName.value = true
  const success = await setDeviceName(deviceNameInput.value.trim())
  isSavingDeviceName.value = false

  if (success) {
    // Save to localStorage so it persists when connecting to other devices
    localStorage.setItem('kaulan_local_device_name', deviceNameInput.value.trim())
    alert('设备名称已更新')
  } else {
    alert('保存设备名称失败')
  }
}

const refreshDevices = async () => {
  await runDiscoveryRefresh()
}

const openManualAddressDialog = async () => {
  const input = prompt('请输入服务器地址，可直接填写 IP、域名或带端口地址:', serverUrlInput.value)
  if (input === null) return

  const trimmed = input.trim()
  serverUrlInput.value = trimmed

  // Add to manual devices list before connecting (fetches device name)
  await addManualDevice(trimmed)

  await saveServerUrl()
}
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
  align-items: flex-start;
  justify-content: flex-start;
  z-index: 100;
}

.modal-content {
  background-color: #fff;
  width: 500px;
  max-width: 85vw;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 2px 0 10px rgba(0, 0, 0, 0.1);
}

.modal-top-bar {
  flex: none;
  padding: 12px 20px;
  border-bottom: 1px solid #eee;
  display: flex;
  align-items: center;
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

.modal-body {
  flex: 1;
  overflow-y: auto;
  padding: 24px 28px 32px;
}

.modal-body h3 {
  margin: 0 0 20px 0;
  font-size: 22px;
  font-weight: 600;
  color: #333;
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

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 15px;
  color: #333;
  cursor: pointer;
}

.setting-checkbox {
  width: 18px;
  height: 18px;
  accent-color: #1db954;
}

.setting-hint {
  margin: 10px 0 0;
  font-size: 13px;
  line-height: 1.5;
  color: #666;
}

.warning-hint {
  color: #b45f06;
}

.setting-error {
  color: #c0392b;
}

.slider-container {
  display: flex;
  align-items: center;
  gap: 15px;
  min-width: 0;
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

.value-input {
  width: 72px;
  flex: 0 0 72px;
  min-width: 0;
  padding: 8px 12px;
  border: 1px solid #1db954;
  border-radius: 5px;
  font-size: 18px;
  font-weight: bold;
  color: #1db954;
  text-align: center;
  transition: all 0.2s;
  background-color: #f0fff4;
}

.value-input:focus {
  outline: none;
  box-shadow: 0 0 0 3px rgba(29, 185, 84, 0.3);
  border-color: #1db954;
}

.value-input::placeholder {
  color: #1db954;
  opacity: 0.5;
}

.value-suffix {
  flex: 0 0 auto;
  white-space: nowrap;
  font-size: 15px;
  color: #777;
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

/* Server URL styles */
.url-input-container {
  margin-bottom: 10px;
}

.url-input {
  width: 100%;
  padding: 12px 15px;
  border: 1px solid #ddd;
  border-radius: 5px;
  font-size: 14px;
  font-family: monospace;
  transition: all 0.2s;
  box-sizing: border-box;
}

.url-input:focus {
  border-color: #1db954;
  outline: none;
  box-shadow: 0 0 0 3px rgba(29, 185, 84, 0.2);
}

.url-input.url-invalid {
  border-color: #e74c3c;
}

.url-input.url-invalid:focus {
  box-shadow: 0 0 0 3px rgba(231, 76, 60, 0.2);
}

.url-error {
  color: #e74c3c;
  font-size: 13px;
  margin-top: -8px;
  margin-bottom: 10px;
  padding-left: 2px;
}

.url-actions {
  display: flex;
  gap: 10px;
  margin-top: 15px;
}

.save-url-btn {
  flex: 1;
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

.save-url-btn:hover:not(:disabled) {
  background-color: #1ed760;
}

.save-url-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.reset-url-btn {
  flex: 1;
  padding: 10px 20px;
  border: 1px solid #e74c3c;
  border-radius: 5px;
  background-color: #fff;
  color: #e74c3c;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.reset-url-btn:hover {
  background-color: #e74c3c;
  color: white;
}

.manual-url-btn {
  flex: 1;
  padding: 10px 20px;
  border: 1px solid #ddd;
  border-radius: 5px;
  background-color: #fff;
  color: #555;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.manual-url-btn:hover {
  background-color: #f0f0f0;
  border-color: #ccc;
}

.manual-address-panel {
  margin-top: 10px;
}

.url-changed {
  color: #e67e22 !important;
}

.mode-value {
  font-size: 16px;
  color: #1db954;
  font-weight: 500;
  min-width: 100px;
  text-align: right;
}

/* Device discovery styles */
.loading-state,
.empty-state {
  color: #777;
  font-size: 14px;
  padding: 10px 0;
  text-align: center;
}

.device-list {
  margin-bottom: 10px;
}

.device-item {
  display: flex;
  align-items: flex-start;
  padding: 12px 15px;
  background-color: #f9f9f9;
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  margin-bottom: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.device-item:hover {
  background-color: #f0f0f0;
  border-color: #1db954;
}

.device-item:last-child {
  margin-bottom: 0;
}

.device-info {
  flex: 1;
  min-width: 0;
}

.device-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 4px;
}

.device-name-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex-wrap: wrap;
}

.device-name {
  font-size: 15px;
  font-weight: 500;
  color: #333;
  min-width: 0;
}

.device-url {
  font-size: 12px;
  color: #777;
  font-family: monospace;
  overflow-wrap: anywhere;
  word-break: break-word;
}

.device-last-seen {
  font-size: 12px;
  color: #999;
  white-space: nowrap;
}

.manual-badge {
  font-size: 11px;
  background-color: #ff9800;
  color: white;
  padding: 2px 6px;
  border-radius: 4px;
  margin-left: 6px;
}

.device-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.remove-device-btn {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 50%;
  background-color: #e74c3c;
  color: white;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
  flex-shrink: 0;
}

.remove-device-btn:hover {
  background-color: #c0392b;
  transform: scale(1.1);
}

.refresh-devices-btn {
  flex: 1;
  padding: 8px 15px;
  border: 1px solid #ddd;
  border-radius: 5px;
  background-color: #fff;
  color: #555;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.refresh-devices-btn:hover {
  background-color: #f0f0f0;
  border-color: #ccc;
}

/* Checkbox styles */
.checkbox-label {
  display: flex;
  align-items: center;
  cursor: pointer;
  font-size: 15px;
  color: #555;
  user-select: none;
}

.setting-checkbox {
  width: 20px;
  height: 20px;
  margin-right: 10px;
  cursor: pointer;
  accent-color: #1db954;
}

.checkbox-label:hover .setting-checkbox {
  transform: scale(1.1);
}
</style>
