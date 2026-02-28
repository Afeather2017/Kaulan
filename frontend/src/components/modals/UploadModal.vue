<template>
  <div class="upload-panel">
    <div class="panel-transparent-top"></div>
    <div class="upload-panel-content">
      <div class="panel-top-bar">
        <button class="top-back-btn" @click="$emit('close')">
          <i class="fas fa-arrow-left"></i>
          返回
        </button>
        <h3 class="panel-title">上传音乐文件</h3>
      </div>
      <div class="panel-body">
        <!-- Directory Tree -->
        <div class="directory-section">
          <label class="setting-label">选择目标目录</label>
          <div class="directory-tree">
            <DirectoryTreeNode
              v-if="directoryTree"
              :node="directoryTree"
              :selected-path="selectedPath"
              @select="selectDirectory"
            />
          </div>
        </div>

        <!-- File Upload -->
        <div class="upload-section">
          <label class="setting-label">选择文件</label>
          <input
            ref="fileInput"
            type="file"
            accept=".mp3,.ogg,.wav,.aac,.flac"
            @change="handleFileSelect"
            style="display: none"
          />
          <button @click="openFileSelector" class="select-files-btn">
            点击选择文件
          </button>
          <div v-if="selectedFile" class="selected-files">
            <div class="files-count">已选择文件</div>
            <div class="files-list">
              <div class="file-item">
                {{ selectedFile.name }}
              </div>
            </div>
          </div>
        </div>

        <!-- Upload Button -->
        <div class="upload-actions">
          <button
            @click="uploadFiles"
            class="upload-btn"
            :disabled="!selectedFile || isUploading"
          >
            {{ isUploading ? '上传中...' : '上传到 ' + (selectedPath || '根目录') }}
          </button>
        </div>

        <!-- Upload Progress -->
        <div v-if="isUploading" class="upload-progress">
          <div class="progress-text">正在上传...</div>
        </div>

        <!-- Upload Result -->
        <div v-if="uploadResult" class="upload-result">
          <div :class="['result-message', uploadResult.success ? 'success' : 'error']">
            {{ uploadResult.message }}
          </div>
          <div v-if="uploadResult.uploaded.length > 0" class="result-files">
            <div class="result-label">成功上传 ({{ uploadResult.uploaded.length }}):</div>
            <div class="result-list">
              <div v-for="file in uploadResult.uploaded" :key="file" class="result-item success">
                {{ file }}
              </div>
            </div>
          </div>
          <div v-if="uploadResult.failed.length > 0" class="result-files">
            <div class="result-label">失败 ({{ uploadResult.failed.length }}):</div>
            <div class="result-list">
              <div v-for="file in uploadResult.failed" :key="file" class="result-item error">
                {{ file }}
              </div>
            </div>
          </div>
        </div>

        <!-- Close Button -->
        <div class="panel-actions">
          <button @click="$emit('close')" class="close-btn">关闭</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getApiBase } from '@/utils/api'

interface DirectoryNode {
  name: string
  path: string
  type: string
  children?: DirectoryNode[]
}

interface UploadResponse {
  success: boolean
  message: string
  uploaded: string[]
  failed: string[]
}

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'uploadComplete'): void
}>()

const directoryTree = ref<DirectoryNode | null>(null)
const selectedPath = ref<string>('')
const selectedFile = ref<File | null>(null)
const isUploading = ref<boolean>(false)
const uploadResult = ref<UploadResponse | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)

onMounted(async () => {
  await loadDirectoryTree()
})

const loadDirectoryTree = async () => {
  try {
    const response = await fetch(`${getApiBase()}/files/directory-tree`)
    if (response.ok) {
      directoryTree.value = await response.json()
    } else {
      alert('获取目录列表失败')
    }
  } catch (error) {
    console.error('Failed to load directory tree:', error)
    alert('获取目录列表失败: ' + error)
  }
}

const selectDirectory = (path: string) => {
  selectedPath.value = path
}

const openFileSelector = () => {
  fileInput.value?.click()
}

const handleFileSelect = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    selectedFile.value = target.files[0]
    uploadResult.value = null
  }
}

const uploadFiles = async () => {
  if (!selectedFile.value) {
    alert('请先选择文件')
    return
  }

  isUploading.value = true
  uploadResult.value = null

  try {
    const formData = new FormData()
    formData.append('targetPath', selectedPath.value)
    formData.append('files', selectedFile.value)

    const response = await fetch(`${getApiBase()}/files/upload`, {
      method: 'POST',
      body: formData,
    })

    const result: UploadResponse = await response.json()
    uploadResult.value = result

    if (result.success) {
      // Clear file selection on success
      selectedFile.value = null
      if (fileInput.value) {
        fileInput.value.value = ''
      }
      // Emit upload complete event to refresh data in parent component
      emit('uploadComplete')
    }
  } catch (error) {
    console.error('Failed to upload files:', error)
    uploadResult.value = {
      success: false,
      message: '上传失败: ' + error,
      uploaded: [],
      failed: [selectedFile.value?.name || 'unknown'],
    }
  } finally {
    isUploading.value = false
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

// Directory Tree Node Component
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
.upload-panel {
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

.upload-panel-content {
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
  padding: 20px;
  overflow-y: auto;
}

.directory-section,
.upload-section {
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
  max-height: 200px;
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
  padding: 8px 10px;
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
  font-size: 16px;
}

.node-text {
  font-size: 15px;
}

.select-files-btn {
  width: 100%;
  padding: 12px 20px;
  border: 2px dashed #ddd;
  border-radius: 5px;
  background-color: #f9f9f9;
  color: #555;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.select-files-btn:hover {
  border-color: #1db954;
  background-color: #f0fff4;
  color: #1db954;
}

.selected-files {
  margin-top: 15px;
}

.files-count {
  font-size: 14px;
  color: #1db954;
  font-weight: 500;
  margin-bottom: 8px;
}

.files-list {
  background-color: #f9f9f9;
  border-radius: 5px;
  padding: 10px;
  max-height: 120px;
  overflow-y: auto;
}

.file-item {
  font-size: 13px;
  color: #555;
  padding: 4px 0;
}

.upload-actions {
  margin-top: 15px;
}

.upload-btn {
  width: 100%;
  padding: 12px 20px;
  border: none;
  border-radius: 5px;
  background-color: #1db954;
  color: white;
  font-size: 16px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.upload-btn:hover:not(:disabled) {
  background-color: #1ed760;
}

.upload-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.upload-progress {
  margin-top: 15px;
  text-align: center;
}

.progress-text {
  color: #1db954;
  font-weight: 500;
}

.upload-result {
  margin-top: 15px;
}

.result-message {
  padding: 10px;
  border-radius: 5px;
  font-weight: 500;
  text-align: center;
  margin-bottom: 10px;
}

.result-message.success {
  background-color: #d4edda;
  color: #155724;
}

.result-message.error {
  background-color: #f8d7da;
  color: #721c24;
}

.result-files {
  margin-top: 10px;
}

.result-label {
  font-size: 13px;
  font-weight: 500;
  color: #555;
  margin-bottom: 5px;
}

.result-list {
  background-color: #f9f9f9;
  border-radius: 5px;
  padding: 10px;
  max-height: 100px;
  overflow-y: auto;
}

.result-item {
  font-size: 13px;
  padding: 3px 0;
}

.result-item.success {
  color: #155724;
}

.result-item.error {
  color: #721c24;
}

.panel-actions {
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
  transition: all 0.2s;
}

.close-btn:hover {
  background-color: #5a6268;
}
</style>
