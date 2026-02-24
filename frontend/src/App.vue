<template>
  <div id="app" class="music-player">
    <div class="app-window">
      <div class="top-bar">
        <button class="icon-btn settings-btn" aria-label="Settings" @click="handleShowSettingsModal">
          <i class="fas fa-cog"></i>
        </button>
        <SearchBar
          v-model="searchQuery"
          @search="handleSearch"
        />
      </div>

      <div class="action-bar">
        <button v-if="showBackButton" class="action-btn" @click="handleActionBack">
          返回
        </button>
        <div class="action-spacer"></div>
        <button v-if="showChooseButton" class="action-btn" @click="handleChooseAction">
          选择
        </button>
      </div>

      <!-- Scanning Message -->
      <div v-if="isScanning && playlistNames.length === 0" class="scanning-message">
        扫描中...
      </div>

      <div class="main-area" :class="{ 'wide-layout': isWideLayout }">
        <div class="list-panel" v-show="!showLyric || isWideLayout">
          <div class="content-area">
            <!-- Playlist List -->
            <PlaylistListView
              v-if="currentView === 'playlists'"
              :title="viewModeLabels[viewMode]"
              :view-mode="viewMode"
              :playlist-names="playlistNames"
              :playlists="playlists"
              :select-mode="collectionSelectMode"
              :selected-playlists="selectedCollectionsList"
              :show-select-button="false"
              :has-selected-non-all-music="hasSelectedNonAllMusicCollection()"
              :show-header="false"
              @toggle-select-mode="toggleCollectionSelectMode"
              @toggle-selection="toggleCollectionSelection"
              @select="handleSelectPlaylist"
              @show-create-modal="handleShowCreateModal"
              @delete-selected="handleDeleteSelectedCollections"
            />

            <!-- Song List -->
            <SongListView
              v-if="currentView === 'songs'"
              :title="selectedPlaylist?.name || ''"
              :songs="currentSongs"
              :select-mode="selectMode"
              :selected-songs="selectedSongs"
              :current-song-name="currentSong?.name"
              :show-remove-button="viewMode === 'collection' && selectedPlaylist?.name !== '所有音乐'"
              :show-add-button="viewMode === 'folder' || selectedPlaylist?.name === '所有音乐'"
              :show-header="false"
              @back="handleBackToPlaylists"
              @toggle-select-mode="toggleSelectMode"
              @toggle-selection="toggleSongSelection"
              @play="handlePlaySong"
              @remove="handleRemoveFromCollection"
              @show-add-modal="handleShowAddToCollectionModal"
            />

            <!-- Search Results -->
            <SongListView
              v-if="currentView === 'search'"
              title="搜索结果"
              :songs="searchResults"
              :select-mode="false"
              :selected-songs="new Set()"
              :show-remove-button="false"
              :show-add-button="false"
              :show-header="false"
              @back="handleBackToPlaylists"
              @play="handlePlaySong"
            />
          </div>
        </div>

        <div class="right-panel" v-if="isWideLayout || showLyric">
          <div class="right-panel-content">
            <div v-if="showLyric" class="lyric-panel">
              <div class="lyric-placeholder">LYRIC</div>
            </div>
            <div v-else class="cover-panel"></div>

            <PlayerControls
              v-if="isWideLayout && !selectMode"
              :current-time="currentTime"
              :duration="audioElement?.duration || 0"
              :is-playing="isPlaying"
              :play-mode="playMode"
              :current-song-name="currentSong?.name"
              @seek="seekToTime"
              @toggle-play-mode="togglePlayMode"
              @previous="previousSong"
              @toggle-play="togglePlay"
              @next="nextSong"
              @show-current-playlist="handleShowCurrentPlaylist"
              @toggle-lyric="handleToggleLyric"
            />
          </div>
        </div>
      </div>

      <!-- Player Controls (mobile) -->
      <PlayerControls
        v-if="!isWideLayout && !selectMode"
        :current-time="currentTime"
        :duration="audioElement?.duration || 0"
        :is-playing="isPlaying"
        :play-mode="playMode"
        :current-song-name="currentSong?.name"
        @seek="seekToTime"
        @toggle-play-mode="togglePlayMode"
        @previous="previousSong"
        @toggle-play="togglePlay"
        @next="nextSong"
        @show-current-playlist="handleShowCurrentPlaylist"
        @toggle-lyric="handleToggleLyric"
      />
    </div>

    <!-- Settings Modal -->
    <SettingsModal
      v-if="showSettings"
      :view-mode="viewMode"
      :volume-mode="volumeMode"
      :manual-volume="manualVolume"
      :manual-volume-input="manualVolumeInput"
      :fixed-lufs="fixedLufs"
      :fixed-lufs-input="fixedLufsInput"
      :timer-minutes="timerMinutes"
      :timer-minutes-input="timerMinutesInput"
      :timer-active="timerActive"
      :timer-status-display="timerStatusDisplay"
      :view-mode-labels="viewModeLabels"
      :volume-mode-labels="volumeModeLabels"
      @close="hideSettingsModal"
      @toggle-view-mode="handleToggleViewMode"
      @toggle-volume-mode="handleToggleVolumeMode"
      @update:manual-volume="manualVolume = $event"
      @update:manual-volume-input="manualVolumeInput = $event"
      @update:fixed-lufs="fixedLufs = $event"
      @update:fixed-lufs-input="fixedLufsInput = $event"
      @update:timer-minutes="timerMinutes = $event"
      @update:timer-minutes-input="timerMinutesInput = $event"
      @set-timer-preset="setTimerPreset"
      @start-timer="handleStartTimer"
      @cancel-timer="cancelTimer"
      @directory-changed="handleDirectoryChanged"
      @database-updated="handleDatabaseUpdated"
      @open-upload-modal="showUploadModal = true"
    />

    <!-- Add to Collection Modal -->
    <AddToCollectionModal
      v-if="showAddToCollection"
      :collections="collections"
      :selected-collection-ids="selectedCollections"
      @close="hideAddToCollectionModal"
      @confirm="addToCollection"
      @toggle-selection="handleToggleCollectionSelection"
    />

    <!-- Create Collection Modal -->
    <CreateCollectionModal
      v-if="showCreateCollection"
      v-model="newCollectionName"
      @close="hideCreateCollectionModal"
      @confirm="handleCreateCollection"
    />

    <!-- Upload Modal -->
    <UploadModal
      v-if="showUploadModal"
      @close="showUploadModal = false"
      @upload-complete="handleUploadComplete"
    />

    <!-- Current Playlist Modal -->
    <CurrentPlaylistModal
      v-if="showCurrentPlaylistModal"
      :playlist="selectedPlaylist"
      :current-song-name="currentSong?.name"
      @close="showCurrentPlaylistModal = false"
      @play="handlePlaySong"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue'
import SearchBar from '@/components/SearchBar.vue'
import PlaylistListView from '@/components/PlaylistListView.vue'
import SongListView, { type SongInfo } from '@/components/SongListView.vue'
import PlayerControls from '@/components/PlayerControls.vue'
import SettingsModal from '@/components/modals/SettingsModal.vue'
import CurrentPlaylistModal from '@/components/modals/CurrentPlaylistModal.vue'
import AddToCollectionModal from '@/components/modals/AddToCollectionModal.vue'
import CreateCollectionModal from '@/components/modals/CreateCollectionModal.vue'
import UploadModal from '@/components/modals/UploadModal.vue'
import { useAudioPlayer } from '@/composables/useAudioPlayer'
import { usePlaylist } from '@/composables/usePlaylist'
import { useSelection } from '@/composables/useSelection'
import { useTimer } from '@/composables/useTimer'
import { useVolume } from '@/composables/useVolume'
import { usePermissions } from '@/composables/usePermissions'

// Use composables
const {
  audioElement,
  currentSong,
  isPlaying,
  currentTime,
  playMode,
  playSong,
  playSongAtIndex,
  togglePlay,
  togglePlayMode,
  previousSong,
  nextSong,
  seekToTime,
  setVolume,
  resetPlaylist,
  initAudio
} = useAudioPlayer({
  songs: () => currentPlaylistSongs.value,
  onSongEnd: () => {}
})

const {
  viewMode,
  playlists,
  collections,
  searchQuery,
  currentView,
  selectedPlaylist,
  playlistNames,
  currentSongs,
  searchResults,
  viewModeLabels,
  refreshData,
  toggleViewMode: playlistToggleViewMode,
  selectPlaylist,
  backToPlaylists,
  showSearchResults,
  createCollection: apiCreateCollection,
  deleteCollection,
  addToCollection: apiAddToCollection,
  removeFromCollection: apiRemoveFromCollection,
  getAllMusic,
  isScanning
} = usePlaylist()

const {
  selectMode,
  selectedSongs,
  collectionSelectMode,
  selectedCollectionsList,
  toggleSelectMode,
  toggleSongSelection,
  toggleCollectionSelectMode,
  toggleCollectionSelection,
  hasSelectedNonAllMusicCollection
} = useSelection()

const {
  timerMinutes,
  timerMinutesInput,
  timerActive,
  timerStatusDisplay,
  setTimerPreset,
  startTimer,
  cancelTimer
} = useTimer(() => {
  // Timer complete callback
  if (audioElement.value) {
    audioElement.value.pause()
  }
  isPlaying.value = false
  currentTime.value = 0
})

const {
  volumeMode,
  manualVolume,
  manualVolumeInput,
  fixedLufs,
  fixedLufsInput,
  volumeModeLabels,
  calculateVolume,
  toggleVolumeMode
} = useVolume(currentSong, currentSongs)

// Permissions composable for Android file access
const { requestPermissions } = usePermissions()

// Additional state
const showSettings = ref(false)
const showAddToCollection = ref(false)
const selectedCollections = ref<number[]>([])
const newCollectionName = ref('')
const showCreateCollection = ref(false)
const showUploadModal = ref(false)
const showCurrentPlaylistModal = ref(false)
const showLyric = ref(false)
const isWideLayout = ref(false)
const hasUserToggledLyric = ref(false)

// Computed helper for audio player
const currentPlaylistSongs = computed(() => {
  return selectedPlaylist.value?.songs || []
})

const showBackButton = computed(() => {
  return showLyric.value || currentView.value !== 'playlists'
})

const showChooseButton = computed(() => {
  if (showLyric.value) return false
  if (currentView.value === 'playlists') return viewMode.value === 'collection'
  if (currentView.value === 'songs') return true
  return false
})

const updateLayoutMode = () => {
  if (typeof window === 'undefined') return
  const isWide = window.matchMedia('(min-aspect-ratio: 1/1)').matches
  isWideLayout.value = isWide
  if (isWide && !hasUserToggledLyric.value) {
    showLyric.value = true
  }
}

// Watch for volume changes and update audio
watch([volumeMode, manualVolume, fixedLufs, currentSong], () => {
  setVolume(calculateVolume())
}, { deep: true })

// Watch for view mode changes
watch(viewMode, async () => {
  backToPlaylists()
  await refreshData()
})

// Event handlers
const handleSearch = () => {
  showSearchResults()
}

const handleSelectPlaylist = (name: string) => {
  selectPlaylist(name)
  resetPlaylist()
}

const handleBackToPlaylists = () => {
  backToPlaylists()
  selectMode.value = false
  selectedSongs.value.clear()
  collectionSelectMode.value = false
  selectedCollectionsList.value.clear()
}

const handleActionBack = () => {
  if (showLyric.value) {
    showLyric.value = false
    return
  }
  handleBackToPlaylists()
}

const handleChooseAction = () => {
  if (currentView.value === 'playlists') {
    toggleCollectionSelectMode()
    return
  }
  if (currentView.value === 'songs') {
    toggleSelectMode()
  }
}

const handlePlaySong = async (song: SongInfo, index?: number) => {
  if (index !== undefined) {
    await playSongAtIndex(song, index)
  } else {
    await playSong(song)
  }
}

const handleShowSettingsModal = () => {
  showSettings.value = true
}

const handleShowCurrentPlaylist = () => {
  showCurrentPlaylistModal.value = true
}

const handleToggleLyric = () => {
  showLyric.value = !showLyric.value
  hasUserToggledLyric.value = true
}

const hideSettingsModal = () => {
  showSettings.value = false
}

const handleToggleViewMode = () => {
  playlistToggleViewMode()
}

const handleToggleVolumeMode = () => {
  toggleVolumeMode()
}

const handleStartTimer = () => {
  startTimer()
}

const handleDirectoryChanged = () => {
  // Refresh data when directory changes
  refreshData()
}

const handleDatabaseUpdated = async () => {
  // Refresh data when database is updated
  await refreshData()
}

const handleUploadComplete = async () => {
  // Refresh data after upload completes
  showUploadModal.value = false
  await refreshData()

  // If a playlist is currently selected, update its songs with the refreshed data
  if (selectedPlaylist.value) {
    const playlistName = selectedPlaylist.value.name
    selectPlaylist(playlistName)
  }
}

// Collection management handlers
const handleShowAddToCollectionModal = () => {
  selectedCollections.value = []
  showAddToCollection.value = true
}

const hideAddToCollectionModal = () => {
  showAddToCollection.value = false
  selectedCollections.value = []
}

const handleToggleCollectionSelection = (id: number) => {
  const index = selectedCollections.value.indexOf(id)
  if (index > -1) {
    selectedCollections.value.splice(index, 1)
  } else {
    selectedCollections.value.push(id)
  }
}

const addToCollection = async () => {
  if (selectedCollections.value.length === 0) {
    alert('请选择至少一个收藏夹')
    return
  }

  const allMusic = await getAllMusic()
  const selectedMusicIds = allMusic
    .filter((m: { filename: string; id: number }) => selectedSongs.value.has(m.filename))
    .map((m: { id: number }) => m.id)

  if (selectedMusicIds.length === 0) {
    alert('没有选中的歌曲')
    return
  }

  for (const collectionId of selectedCollections.value) {
    await apiAddToCollection(collectionId, selectedMusicIds)
  }

  alert('添加成功')
  hideAddToCollectionModal()
  selectMode.value = false
  selectedSongs.value.clear()
  await refreshData()
}

const handleRemoveFromCollection = async () => {
  if (!selectedPlaylist.value) return

  const collection = collections.value.find((c: { id: number; name: string }) => c.name === selectedPlaylist.value?.name)
  if (!collection || collection.id === -1) {
    alert('无法从"所有音乐"中移除')
    return
  }

  const allMusic = await getAllMusic()
  const selectedMusicIds = allMusic
    .filter((m: { filename: string; id: number }) => selectedSongs.value.has(m.filename))
    .map((m: { id: number }) => m.id)

  if (selectedMusicIds.length === 0) {
    alert('没有选中的歌曲')
    return
  }

  const success = await apiRemoveFromCollection(collection.id, selectedMusicIds)
  if (success) {
    alert('移除成功')
    selectMode.value = false
    selectedSongs.value.clear()
    await refreshData()
  } else {
    alert('移除失败')
  }
}

const handleShowCreateModal = () => {
  newCollectionName.value = ''
  showCreateCollection.value = true
}

const hideCreateCollectionModal = () => {
  showCreateCollection.value = false
  newCollectionName.value = ''
}

const handleCreateCollection = async () => {
  if (!newCollectionName.value.trim()) {
    alert('请输入收藏夹名称')
    return
  }

  await apiCreateCollection(newCollectionName.value.trim())
  hideCreateCollectionModal()
}

const handleDeleteSelectedCollections = async () => {
  if (selectedCollectionsList.value.size === 0) {
    alert('请选择要删除的收藏夹')
    return
  }

  if (!confirm(`确定要删除选中的 ${selectedCollectionsList.value.size} 个收藏夹吗？`)) {
    return
  }

  let deletedCount = 0
  for (const collectionName of selectedCollectionsList.value) {
    if (collectionName === '所有音乐') continue

    const collection = collections.value.find((c: { id: number; name: string }) => c.name === collectionName)
    if (collection) {
      const success = await deleteCollection(collection.id)
      if (success) deletedCount++
    }
  }

  alert(`已删除 ${deletedCount} 个收藏夹`)
  collectionSelectMode.value = false
  selectedCollectionsList.value.clear()
  await refreshData()
}

// Initialize
onMounted(async () => {
  // Request storage permissions on Android before accessing music files
  // On web this is a no-op
  const granted = await requestPermissions()
  if (!granted) {
    console.warn('Storage permissions not granted - music access may fail')
  }

  initAudio()
  refreshData()
  updateLayoutMode()
  window.addEventListener('resize', updateLayoutMode)
})

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('resize', updateLayoutMode)
  }
})
</script>

<style scoped>
.music-player {
  min-height: 100vh;
  display: flex;
  align-items: stretch;
  justify-content: center;
  background-color: #f5f5f5;
  color: #333;
  font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
  overflow: hidden;
}

.app-window {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background-color: #fff;
  overflow: hidden;
}

.top-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  background-color: #fff;
  border-bottom: 1px solid #eee;
}

.icon-btn {
  width: 36px;
  height: 36px;
  border: none;
  background: #f0f0f0;
  border-radius: 8px;
  cursor: pointer;
  font-size: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #333;
}

.icon-btn:hover {
  background-color: #e6e6e6;
}

.action-bar {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  background-color: #fff;
  border-bottom: 1px solid #eee;
}

.action-spacer {
  flex: 1;
}

.action-btn {
  background: none;
  border: none;
  color: #1db954;
  font-size: 14px;
  cursor: pointer;
  padding: 6px 8px;
}

.scanning-message {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 120px;
  font-size: 18px;
  color: #888;
}

.main-area {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background-color: #fff;
}

.main-area.wide-layout {
  flex-direction: row;
}

.list-panel {
  flex: 1;
  min-height: 0;
  background-color: #fff;
}

.right-panel {
  flex: 1;
  min-height: 0;
  background-color: #fafafa;
  border-left: 1px solid #eee;
}

.right-panel-content {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: flex-start;
  gap: 0;
  padding: 0;
  box-sizing: border-box;
}

.lyric-panel {
  flex: 1;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  border-top: 1px solid #eee;
  border-bottom: 1px solid #eee;
  background-color: #fff;
}

.lyric-placeholder {
  font-size: 20px;
  color: #888;
  letter-spacing: 2px;
}

.cover-panel {
  flex: 1;
  width: 100%;
  border-top: 1px solid #eee;
  border-bottom: 1px solid #eee;
  background-color: #fff;
}

.content-area {
  height: 100%;
  overflow-y: auto;
  padding: 15px;
  background-color: #fff;
  position: relative;
}

@media (min-width: 900px) and (min-aspect-ratio: 1/1) {
  .music-player {
    align-items: stretch;
  }

  .app-window {
    width: 100%;
    height: 100vh;
    border-radius: 0;
    border: none;
    box-shadow: none;
  }
}
</style>
