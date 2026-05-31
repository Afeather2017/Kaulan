<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <h3>在线查找</h3>

      <div class="search-section">
        <div class="search-input-row">
          <input
            v-model="searchInput"
            type="text"
            placeholder="搜索歌曲、视频或歌词关键字..."
            @keyup.enter="handleSearch"
          />
          <button
            class="search-btn"
            @click="handleSearch"
            :disabled="isSearching || !searchInput.trim() || selectedSources.length === 0"
          >
            {{ isSearching ? '搜索中...' : '搜索' }}
          </button>
        </div>

        <div class="source-row">
          <label v-for="source in sourceOptions" :key="source.value" class="source-checkbox">
            <input
              type="checkbox"
              :checked="selectedSources.includes(source.value)"
              :disabled="!isSourceAvailable(source.value)"
              @change="toggleSource(source.value)"
            />
            <span>{{ source.label }}</span>
          </label>
        </div>
      </div>

      <div class="provider-statuses">
        <div
          v-for="provider in providerOptions"
          :key="provider.value"
          class="provider-card"
        >
          <div class="provider-title">{{ provider.label }}</div>
          <div class="provider-summary">{{ providerStatus[provider.value].summary }}</div>
          <div class="provider-actions">
            <template v-if="!providerStatus[provider.value].is_logged_in">
              <button class="login-btn" @click="openLogin(provider.value)">打开登录</button>
              <button class="login-btn" @click="captureLogin(provider.value)">读取登录</button>
            </template>
            <button
              v-if="providerStatus[provider.value].is_logged_in"
              class="secondary-btn"
              @click="logout(provider.value)"
            >
              退出
            </button>
          </div>
        </div>
      </div>

      <div class="directory-section">
        <label class="setting-label">保存到目录</label>
        <div class="directory-tree">
          <DirectoryTreeNode
            v-if="directoryTree"
            :node="directoryTree"
            :selected-path="selectedPath"
            @select="selectDirectory"
          />
        </div>
        <p class="permission-message">
          Android 下载会保存到应用目录，试听文件会在下次启动时自动清理。
        </p>
      </div>

      <div class="results-section" v-if="searchResults.length > 0">
        <div class="results-list">
          <div
            v-for="result in searchResults"
            :key="result.source + ':' + result.id"
            class="result-item"
          >
            <img
              v-if="result.thumbnail_url"
              :src="result.thumbnail_url"
              class="result-thumbnail"
              loading="lazy"
            />
            <div v-else class="result-thumbnail placeholder"></div>

            <div class="result-info">
              <div class="result-header">
                <div class="result-title">{{ result.title }}</div>
                <span class="source-badge">{{ sourceLabel(result.source) }}</span>
              </div>
              <div class="result-meta">
                {{ result.artist }}
                <span v-if="result.duration" class="result-duration">{{ result.duration }}</span>
              </div>
              <div v-if="selectedLyrics[resultKey(result)]" class="selected-lyric">
                歌词: {{ selectedLyrics[resultKey(result)]?.title }} / {{ selectedLyrics[resultKey(result)]?.artist }}
              </div>
            </div>

            <div class="result-actions">
              <button
                class="action-btn preview-btn"
                @click="handlePreview(result)"
                :disabled="previewingKey === resultKey(result)"
              >
                {{ previewingKey === resultKey(result) ? '准备中' : '试听' }}
              </button>
              <button
                class="action-btn lyric-btn"
                @click="toggleLyrics(result)"
                :disabled="loadingLyricsKey === resultKey(result)"
              >
                {{ loadingLyricsKey === resultKey(result) ? '读取中' : '歌词' }}
              </button>
              <button
                class="action-btn download-btn"
                @click="handleDownload(result)"
                :disabled="downloadingKey === resultKey(result)"
              >
                {{ downloadingKey === resultKey(result) ? '下载中' : '下载' }}
              </button>
            </div>

            <div
              v-if="expandedLyricsKey === resultKey(result)"
              class="lyrics-candidates"
            >
              <div class="lyric-tip">
                选择歌词后，点击这一行的“下载”会同时保存歌曲和歌词。
              </div>
              <div
                v-for="candidate in lyricCandidates[resultKey(result)] || []"
                :key="candidate.id"
                :class="['lyric-candidate', { selected: selectedLyrics[resultKey(result)]?.id === candidate.id }]"
                @click="selectLyric(result, candidate)"
              >
                <div class="candidate-title">{{ candidate.title }}</div>
                <div class="candidate-meta">{{ candidate.artist }}</div>
              </div>
              <div
                v-if="(lyricCandidates[resultKey(result)] || []).length === 0"
                class="empty-candidate"
              >
                未找到可选歌词
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-if="statusMessage" :class="['status-message', statusType]">
        {{ statusMessage }}
      </div>

      <div class="modal-actions">
        <button @click="$emit('close')" class="close-btn">关闭</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, onMounted } from 'vue'
import { getApiBase } from '@/utils/api'
import { checkIsAndroid } from '@/utils/platform'

type DownloadSource = 'youtube' | 'netease' | 'bilibili'
type OnlineProvider = DownloadSource

interface SearchResult {
  source: DownloadSource
  id: string
  title: string
  artist: string
  duration: string | null
  thumbnail_url: string | null
  can_preview: boolean
  can_download: boolean
  requires_login: boolean
}

interface DirectoryNode {
  name: string
  path: string
  type: string
  children?: DirectoryNode[]
}

interface ProviderStatus {
  provider: string
  is_logged_in: boolean
  session_path: string
  summary: string
}

interface LyricCandidate {
  source: DownloadSource
  id: string
  title: string
  artist: string
  album?: string | null
}

interface PreviewSong {
  id: number
  name: string
  path: string
  stream_url: string
  lufs: number | null
  cover_url?: string | null
  source: DownloadSource
  is_temporary: boolean
}

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'downloadComplete'): void
  (e: 'previewTrack', song: PreviewSong): void
}>()

const sourceOptions: Array<{ value: DownloadSource; label: string }> = [
  { value: 'youtube', label: 'YouTube' },
  { value: 'netease', label: '网易云' },
  { value: 'bilibili', label: 'Bilibili' }
]

const providerOptions: Array<{ value: OnlineProvider; label: string }> = [
  { value: 'youtube', label: 'YouTube' },
  { value: 'netease', label: '网易云' },
  { value: 'bilibili', label: 'Bilibili' }
]

const searchInput = ref('')
const isSearching = ref(false)
const searchResults = ref<SearchResult[]>([])
const downloadingKey = ref<string | null>(null)
const previewingKey = ref<string | null>(null)
const loadingLyricsKey = ref<string | null>(null)
const expandedLyricsKey = ref<string | null>(null)
const statusMessage = ref('')
const statusType = ref<'info' | 'success' | 'error'>('info')
const directoryTree = ref<DirectoryNode | null>(null)
const selectedPath = ref('')
const selectedSources = ref<DownloadSource[]>(['youtube', 'netease', 'bilibili'])
const lyricCandidates = reactive<Record<string, LyricCandidate[]>>({})
const selectedLyrics = reactive<Record<string, LyricCandidate | null>>({})
const providerStatus = reactive<Record<OnlineProvider, ProviderStatus>>({
  youtube: {
    provider: 'youtube',
    is_logged_in: false,
    session_path: '',
    summary: '未读取登录状态'
  },
  netease: {
    provider: 'netease',
    is_logged_in: false,
    session_path: '',
    summary: '未读取登录状态'
  },
  bilibili: {
    provider: 'bilibili',
    is_logged_in: false,
    session_path: '',
    summary: '未读取登录状态'
  }
})

onMounted(async () => {
  await Promise.all([
    loadDirectoryTree(),
    loadProviderStatus('youtube'),
    loadProviderStatus('netease'),
    loadProviderStatus('bilibili'),
    checkIsAndroid()
  ])
  syncSelectedSources()
})

const resultKey = (result: SearchResult): string => `${result.source}:${result.id}`

const sourceLabel = (source: DownloadSource): string => {
  switch (source) {
    case 'youtube':
      return 'YouTube'
    case 'netease':
      return '网易云'
    case 'bilibili':
      return 'Bilibili'
  }
}

const isSourceAvailable = (source: DownloadSource): boolean => providerStatus[source].is_logged_in

const syncSelectedSources = () => {
  selectedSources.value = selectedSources.value.filter(source => isSourceAvailable(source))
}

const toggleSource = (source: DownloadSource) => {
  if (!isSourceAvailable(source)) {
    return
  }
  if (selectedSources.value.includes(source)) {
    selectedSources.value = selectedSources.value.filter(item => item !== source)
    return
  }
  selectedSources.value = [...selectedSources.value, source]
}

const selectDirectory = (path: string) => {
  selectedPath.value = path
}

const loadDirectoryTree = async () => {
  try {
    const response = await fetch(getApiBase() + '/download/directory-tree')
    if (response.ok) {
      directoryTree.value = await response.json()
    }
  } catch (error) {
    console.error('Failed to load download directory tree:', error)
  }
}

const providerLabel = (provider: OnlineProvider): string => {
  switch (provider) {
    case 'youtube':
      return 'YouTube'
    case 'netease':
      return '网易云'
    case 'bilibili':
      return 'Bilibili'
  }
}

const loadProviderStatus = async (provider: OnlineProvider) => {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status = await invoke<ProviderStatus>('online_login_status', { provider })
    providerStatus[provider] = status
    syncSelectedSources()
  } catch (error) {
    providerStatus[provider].summary = '当前环境不支持读取登录状态'
    console.warn('Failed to load provider status:', provider, error)
  }
}

const openLogin = async (provider: OnlineProvider) => {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('online_open_login', { provider })
    statusType.value = 'info'
    statusMessage.value = `已打开 ${providerLabel(provider)} 登录页面`
  } catch (error) {
    statusType.value = 'error'
    statusMessage.value = `打开登录失败: ${error}`
  }
}

const captureLogin = async (provider: OnlineProvider) => {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status = await invoke<ProviderStatus>('online_capture_login', { provider })
    providerStatus[provider] = status
    if (!selectedSources.value.includes(provider)) {
      selectedSources.value = [...selectedSources.value, provider]
    }
    statusType.value = 'success'
    statusMessage.value = `${providerLabel(provider)} 登录信息已保存`
  } catch (error) {
    statusType.value = 'error'
    statusMessage.value = `读取登录信息失败: ${error}`
  }
}

const logout = async (provider: OnlineProvider) => {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const status = await invoke<ProviderStatus>('online_logout', { provider })
    providerStatus[provider] = status
    syncSelectedSources()
    statusType.value = 'success'
    statusMessage.value = `${providerLabel(provider)} 已退出`
  } catch (error) {
    statusType.value = 'error'
    statusMessage.value = `退出失败: ${error}`
  }
}

const handleSearch = async () => {
  const enabledSources = selectedSources.value.filter(source => isSourceAvailable(source))
  if (!searchInput.value.trim() || enabledSources.length === 0) {
    statusType.value = 'error'
    statusMessage.value = '请先登录至少一个可用来源'
    return
  }

  isSearching.value = true
  statusMessage.value = ''
  try {
    const response = await fetch(getApiBase() + '/download/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        query: searchInput.value.trim(),
        max_results: 8,
        sources: enabledSources
      })
    })
    if (!response.ok) {
      const errorText = await response.text()
      throw new Error(errorText || '搜索失败')
    }

    searchResults.value = await response.json()
  } catch (error) {
    statusType.value = 'error'
    statusMessage.value = `搜索失败: ${error}`
  } finally {
    isSearching.value = false
  }
}

const handlePreview = async (result: SearchResult) => {
  previewingKey.value = resultKey(result)
  statusType.value = 'info'
  statusMessage.value = `正在准备试听: ${result.title}`

  try {
    const response = await fetch(getApiBase() + '/download/preview', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        source: result.source,
        id: result.id,
        title: result.title,
        artist: result.artist
      })
    })
    const payload = await response.json()
    if (!response.ok || !payload.success || !payload.song) {
      throw new Error(payload.message || '试听准备失败')
    }

    emit('previewTrack', {
      ...payload.song,
      lufs: null
    })
    statusType.value = 'success'
    statusMessage.value = `已开始试听: ${result.title}`
  } catch (error) {
    statusType.value = 'error'
    statusMessage.value = `试听失败: ${error}`
  } finally {
    previewingKey.value = null
  }
}

const fetchLyrics = async (result: SearchResult) => {
  loadingLyricsKey.value = resultKey(result)
  try {
    const response = await fetch(getApiBase() + '/download/lyrics/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        query: `${result.title} ${result.artist}`.trim()
      })
    })
    if (!response.ok) {
      const text = await response.text()
      throw new Error(text || '歌词搜索失败')
    }
    lyricCandidates[resultKey(result)] = await response.json()
  } finally {
    loadingLyricsKey.value = null
  }
}

const toggleLyrics = async (result: SearchResult) => {
  const key = resultKey(result)
  if (expandedLyricsKey.value === key) {
    expandedLyricsKey.value = null
    return
  }
  expandedLyricsKey.value = key
  if (lyricCandidates[key] === undefined) {
    try {
      await fetchLyrics(result)
    } catch (error) {
      statusType.value = 'error'
      statusMessage.value = `歌词搜索失败: ${error}`
    }
  }
}

const selectLyric = (result: SearchResult, candidate: LyricCandidate) => {
  selectedLyrics[resultKey(result)] = candidate
}

const handleDownload = async (result: SearchResult) => {
  downloadingKey.value = resultKey(result)
  statusType.value = 'info'
  statusMessage.value = `正在下载: ${result.title}`
  try {
    const selectedLyric = selectedLyrics[resultKey(result)]
    const response = await fetch(getApiBase() + '/download/track', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        source: result.source,
        id: result.id,
        title: result.title,
        artist: result.artist,
        target_subdir: selectedPath.value || '',
        lyric_selection: selectedLyric?.id ?? null
      })
    })
    const data = await response.json()
    console.log('[online-search] download response:', {
      status: response.status,
      ok: response.ok,
      source: result.source,
      id: result.id,
      target_subdir: selectedPath.value || '',
      payload: data
    })
    if (!response.ok || !data.success) {
      throw new Error(data.message || '下载失败')
    }
    statusType.value = data.warning ? 'info' : 'success'
    statusMessage.value = data.warning
      ? `下载完成: ${data.filename}，${data.warning}`
      : `下载完成: ${data.filename}`
    emit('downloadComplete')
  } catch (error) {
    statusType.value = 'error'
    statusMessage.value = `下载失败: ${error}`
  } finally {
    downloadingKey.value = null
  }
}
</script>

<script lang="ts">
import { defineComponent, PropType } from 'vue'

interface DirectoryNode {
  name: string
  path: string
  type: string
  children?: DirectoryNode[]
}

export const DirectoryTreeNode = defineComponent({
  name: 'DirectoryTreeNode',
  props: {
    node: {
      type: Object as PropType<DirectoryNode>,
      required: true
    },
    selectedPath: {
      type: String,
      default: ''
    }
  },
  emits: ['select'],
  template: `
    <div class="directory-node">
      <div
        :class="['directory-name', { selected: node.path === selectedPath }]"
        @click="$emit('select', node.path)"
      >
        <span class="folder-icon">📁</span>
        <span class="node-text">{{ node.name || '根目录' }}</span>
      </div>
      <div v-if="node.children && node.children.length > 0" class="directory-children">
        <DirectoryTreeNode
          v-for="child in node.children"
          :key="child.path"
          :node="child"
          :selected-path="selectedPath"
          @select="$emit('select', $event)"
        />
      </div>
    </div>
  `
})
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
  padding: 20px;
  border-radius: 12px;
  width: min(92vw, 760px);
  max-height: 90vh;
  overflow-y: auto;
}

.modal-content h3 {
  margin: 0 0 18px;
  text-align: center;
}

.search-section,
.directory-section,
.results-section {
  margin-bottom: 18px;
}

.search-input-row {
  display: flex;
  gap: 10px;
}

.search-input-row input {
  flex: 1;
  padding: 10px 12px;
  border: 1px solid #d0d7de;
  border-radius: 8px;
}

.search-btn,
.action-btn,
.login-btn,
.secondary-btn,
.close-btn {
  border: none;
  border-radius: 8px;
  padding: 10px 14px;
  cursor: pointer;
}

.search-btn,
.download-btn,
.preview-btn,
.login-btn {
  background: #1db954;
  color: #fff;
}

.lyric-btn,
.secondary-btn,
.close-btn {
  background: #eceff3;
  color: #223;
}

.search-btn:disabled,
.action-btn:disabled {
  opacity: 0.65;
  cursor: not-allowed;
}

.source-row {
  display: flex;
  gap: 14px;
  flex-wrap: wrap;
  margin-top: 12px;
}

.source-checkbox {
  display: flex;
  align-items: center;
  gap: 6px;
}

.provider-statuses {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 12px;
  margin-bottom: 18px;
}

.provider-card {
  border: 1px solid #e4e8ee;
  border-radius: 10px;
  padding: 12px;
  background: #f9fbfc;
}

.provider-title {
  font-weight: 700;
  margin-bottom: 6px;
}

.provider-summary {
  font-size: 13px;
  color: #4a5565;
  min-height: 34px;
}

.provider-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 10px;
}

.setting-label {
  display: block;
  margin-bottom: 8px;
  font-weight: 600;
}

.directory-tree {
  border: 1px solid #e4e8ee;
  border-radius: 8px;
  padding: 10px;
  max-height: 180px;
  overflow-y: auto;
  background: #fbfcfd;
}

.permission-message {
  margin: 10px 0 0;
  font-size: 13px;
  color: #556372;
}

.directory-node {
  margin-left: 0;
}

.directory-children {
  margin-left: 20px;
}

.directory-name {
  display: flex;
  align-items: center;
  padding: 6px 10px;
  border-radius: 6px;
  cursor: pointer;
}

.directory-name.selected {
  background: #1db954;
  color: #fff;
}

.folder-icon {
  margin-right: 8px;
}

.results-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.result-item {
  border: 1px solid #e5e7eb;
  border-radius: 10px;
  padding: 12px;
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  align-items: flex-start;
}

.result-thumbnail {
  width: 72px;
  height: 72px;
  flex-shrink: 0;
  object-fit: cover;
  border-radius: 8px;
  background: #e5e7eb;
}

.result-thumbnail.placeholder {
  background: linear-gradient(135deg, #dae3e8, #eef3f6);
}

.result-info {
  flex: 1;
  min-width: 0;
}

.result-header {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.result-title {
  font-weight: 700;
  color: #1f2937;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-badge {
  font-size: 11px;
  background: #d8f4e1;
  color: #126b37;
  padding: 2px 8px;
  border-radius: 999px;
}

.result-meta,
.selected-lyric,
.candidate-meta,
.empty-candidate {
  font-size: 13px;
  color: #576475;
  margin-top: 4px;
}

.result-duration {
  margin-left: 8px;
}

.result-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.lyrics-candidates {
  width: calc(100% - 84px);
  margin-left: 84px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.lyric-tip {
  font-size: 13px;
  color: #576475;
}

.lyric-candidate {
  border: 1px solid #dde3ea;
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
}

.lyric-candidate.selected {
  border-color: #1db954;
  background: #eefaf2;
}

.candidate-title {
  font-weight: 600;
}

.status-message {
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 14px;
  margin-top: 12px;
}

.status-message.info {
  background: #e8f3ff;
  color: #0e4f96;
}

.status-message.success {
  background: #eaf8ef;
  color: #1b6a39;
}

.status-message.error {
  background: #fdecec;
  color: #a12626;
}

.modal-actions {
  display: flex;
  justify-content: center;
  margin-top: 18px;
}

@media (max-width: 640px) {
  .result-thumbnail {
    width: 56px;
    height: 56px;
  }

  .result-info {
    flex: 1;
    min-width: 0;
  }

  .result-actions {
    flex-basis: 100%;
    flex-direction: row;
    flex-wrap: wrap;
  }

  .lyrics-candidates {
    flex-basis: 100%;
    width: 100%;
    margin-left: 0;
  }
}
</style>
