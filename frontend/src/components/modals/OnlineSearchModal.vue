<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <h3>在线查找</h3>

      <!-- Search Input -->
      <div class="search-section">
        <div class="search-input-row">
          <input
            v-model="searchInput"
            type="text"
            placeholder="搜索YouTube音乐..."
            @keyup.enter="handleSearch"
          />
          <button
            class="search-btn"
            @click="handleSearch"
            :disabled="isSearching || !searchInput.trim()"
          >
            {{ isSearching ? '搜索中...' : '搜索' }}
          </button>
        </div>
      </div>

      <!-- Directory Tree -->
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
        <p v-if="permissionMessage" :class="['permission-message', { error: !storagePermissionGranted }]">
          {{ permissionMessage }}
        </p>
        <button
          v-if="isAndroid && !storagePermissionGranted"
          class="permission-btn"
          @click="requestStoragePermission"
          :disabled="isRequestingPermission"
        >
          {{ isRequestingPermission ? '请求权限中...' : '授予存储权限后允许下载' }}
        </button>
      </div>

      <!-- Search Results -->
      <div class="results-section" v-if="searchResults.length > 0">
        <div class="results-list">
          <div
            v-for="result in searchResults"
            :key="result.id"
            class="result-item"
          >
            <img
              :src="result.thumbnail_url"
              class="result-thumbnail"
              loading="lazy"
            />
            <div class="result-info" @click="handleDownload(result)">
              <div class="result-title">{{ result.title }}</div>
              <div class="result-meta">
                {{ result.channel }}
                <span v-if="result.duration" class="result-duration">
                  {{ result.duration }}
                </span>
              </div>
            </div>
            <button
              class="download-btn"
              @click.stop="handleDownload(result)"
              :disabled="downloadingId === result.id || (isAndroid && !storagePermissionGranted)"
            >
              <i
                :class="downloadingId === result.id ? 'fas fa-spinner fa-spin' : 'fas fa-download'"
              ></i>
            </button>
          </div>
        </div>
      </div>

      <!-- Status Message -->
      <div v-if="statusMessage" :class="['status-message', statusType]">
        {{ statusMessage }}
      </div>

      <!-- Close Button -->
      <div class="modal-actions">
        <button @click="$emit('close')" class="close-btn">关闭</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getApiBase } from '@/utils/api'
import { checkIsAndroid } from '@/utils/platform'

interface SearchResult {
  id: string
  title: string
  channel: string
  duration: string | null
  thumbnail_url: string
}

interface DirectoryNode {
  name: string
  path: string
  type: string
  children?: DirectoryNode[]
}

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'downloadComplete'): void
}>()

const searchInput = ref('')
const isSearching = ref(false)
const searchResults = ref<SearchResult[]>([])
const downloadingId = ref<string | null>(null)
const statusMessage = ref('')
const statusType = ref('')
const directoryTree = ref<DirectoryNode | null>(null)
const selectedPath = ref('')
const isAndroid = ref(false)
const storagePermissionGranted = ref(true)
const isRequestingPermission = ref(false)
const permissionMessage = ref('')

onMounted(async () => {
  isAndroid.value = await checkIsAndroid()
  if (isAndroid.value) {
    await loadPermissionState()
  }
  await loadDirectoryTree()
})

const loadPermissionState = async () => {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    storagePermissionGranted.value = await invoke<boolean>('check_external_storage_permission')
    permissionMessage.value = storagePermissionGranted.value
      ? '已获得外部存储权限，下载会保存到 /sdcard/Music'
      : '下载到 /sdcard/Music 需要外部存储权限'
  } catch (error) {
    console.error('Failed to check external storage permission:', error)
    storagePermissionGranted.value = false
    permissionMessage.value = '无法检查外部存储权限'
  }
}

const loadDirectoryTree = async () => {
  try {
    const response = await fetch(getApiBase() + '/download/directory-tree')
    if (response.ok) {
      directoryTree.value = await response.json()
      return
    }
    if (response.status === 403 && isAndroid.value) {
      directoryTree.value = null
      permissionMessage.value = '下载目录需要外部存储权限后才能浏览'
    }
  } catch (error) {
    console.error('Failed to load directory tree:', error)
  }
}

const requestStoragePermission = async () => {
  if (!isAndroid.value) {
    storagePermissionGranted.value = true
    return
  }

  isRequestingPermission.value = true
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    storagePermissionGranted.value = await invoke<boolean>('request_external_storage_permission')
    permissionMessage.value = storagePermissionGranted.value
      ? '已获得外部存储权限，下载会保存到 /sdcard/Music'
      : '权限未授予，无法下载到 /sdcard/Music'
    if (storagePermissionGranted.value) {
      await loadDirectoryTree()
    }
  } catch (error) {
    console.error('Failed to request external storage permission:', error)
    storagePermissionGranted.value = false
    permissionMessage.value = '请求外部存储权限失败'
  } finally {
    isRequestingPermission.value = false
  }
}

const selectDirectory = (path: string) => {
  selectedPath.value = path
}

const handleSearch = async () => {
  if (!searchInput.value.trim()) return
  isSearching.value = true
  statusMessage.value = ''
  try {
    const response = await fetch(getApiBase() + '/download/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        query: searchInput.value.trim(),
        max_results: 10,
      }),
    })
    if (!response.ok) {
      const err = await response.json()
      throw new Error(err.message || '搜索失败')
    }
    searchResults.value = await response.json()
  } catch (error) {
    statusType.value = 'error'
    statusMessage.value = '搜索失败: ' + error
  } finally {
    isSearching.value = false
  }
}

const handleDownload = async (result: SearchResult) => {
  if (isAndroid.value && !storagePermissionGranted.value) {
    statusType.value = 'error'
    statusMessage.value = '请先授予外部存储权限'
    return
  }
  downloadingId.value = result.id
  statusType.value = 'info'
  statusMessage.value = '正在下载: ' + result.title + '...'
  try {
    const response = await fetch(getApiBase() + '/download/track', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        video_id: result.id,
        title: result.title,
        target_subdir: selectedPath.value || '',
      }),
    })
    const data = await response.json()
    if (data.success) {
      statusType.value = 'success'
      statusMessage.value = '下载完成: ' + data.filename
      emit('downloadComplete')
    } else {
      statusType.value = 'error'
      statusMessage.value = data.message || '下载失败'
    }
  } catch (error) {
    statusType.value = 'error'
    statusMessage.value = '下载失败: ' + error
  } finally {
    downloadingId.value = null
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
      required: true,
    },
    selectedPath: {
      type: String,
      default: '',
    },
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
  `,
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
  padding: 25px;
  border-radius: 10px;
  width: 90%;
  max-width: 500px;
  max-height: 85vh;
  overflow-y: auto;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
}

.modal-content h3 {
  text-align: center;
  margin-bottom: 20px;
  font-size: 22px;
  font-weight: bold;
  color: #333;
  padding-bottom: 15px;
  border-bottom: 1px solid #eee;
}

.search-section {
  margin-bottom: 20px;
}

.search-input-row {
  display: flex;
  gap: 10px;
}

.search-input-row input {
  flex: 1;
  padding: 10px 14px;
  border: 1px solid #ddd;
  border-radius: 5px;
  font-size: 15px;
  outline: none;
  transition: border-color 0.2s;
}

.search-input-row input:focus {
  border-color: #1db954;
}

.search-btn {
  padding: 10px 20px;
  border: none;
  border-radius: 5px;
  background-color: #1db954;
  color: white;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: background-color 0.2s;
  white-space: nowrap;
}

.search-btn:hover:not(:disabled) {
  background-color: #1ed760;
}

.search-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.permission-message {
  margin-top: 10px;
  font-size: 13px;
  color: #2f6f3e;
}

.permission-message.error {
  color: #b42318;
}

.permission-btn {
  margin-top: 10px;
  padding: 8px 14px;
  border: none;
  border-radius: 5px;
  background-color: #1db954;
  color: #fff;
  font-size: 14px;
  cursor: pointer;
}

.permission-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.directory-section {
  margin-bottom: 20px;
}

.setting-label {
  display: block;
  margin-bottom: 10px;
  font-weight: 500;
  font-size: 15px;
  color: #555;
}

.directory-tree {
  background-color: #f9f9f9;
  border: 1px solid #ddd;
  border-radius: 5px;
  padding: 15px;
  max-height: 150px;
  overflow-y: auto;
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
  cursor: pointer;
  border-radius: 5px;
  transition: background-color 0.2s;
}

.directory-name:hover {
  background-color: #e8f5e9;
}

.directory-name.selected {
  background-color: #1db954;
  color: white;
}

.folder-icon {
  margin-right: 8px;
  font-size: 14px;
}

.node-text {
  font-size: 14px;
}

.results-section {
  margin-bottom: 15px;
}

.results-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.result-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px;
  border-radius: 8px;
  background-color: #f9f9f9;
  transition: background-color 0.2s;
}

.result-item:hover {
  background-color: #f0f0f0;
}

.result-thumbnail {
  width: 64px;
  height: 48px;
  border-radius: 4px;
  object-fit: cover;
  flex-shrink: 0;
  background-color: #e0e0e0;
}

.result-info {
  flex: 1;
  min-width: 0;
  cursor: pointer;
}

.result-title {
  font-size: 14px;
  font-weight: 500;
  color: #333;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.result-meta {
  font-size: 12px;
  color: #888;
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.result-duration {
  margin-left: 8px;
  color: #aaa;
}

.download-btn {
  flex-shrink: 0;
  width: 36px;
  height: 36px;
  border: none;
  border-radius: 50%;
  background-color: #1db954;
  color: white;
  font-size: 14px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.2s;
}

.download-btn:hover:not(:disabled) {
  background-color: #1ed760;
}

.download-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.status-message {
  padding: 10px;
  border-radius: 5px;
  font-weight: 500;
  text-align: center;
  font-size: 14px;
  margin-bottom: 10px;
}

.status-message.info {
  background-color: #d1ecf1;
  color: #0c5460;
}

.status-message.success {
  background-color: #d4edda;
  color: #155724;
}

.status-message.error {
  background-color: #f8d7da;
  color: #721c24;
}

.modal-actions {
  display: flex;
  justify-content: center;
  margin-top: 20px;
}

.close-btn {
  padding: 10px 30px;
  border: none;
  border-radius: 5px;
  background-color: #6c757d;
  color: white;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: background-color 0.2s;
}

.close-btn:hover {
  background-color: #5a6268;
}
</style>
