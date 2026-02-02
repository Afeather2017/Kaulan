<template>
  <div id="app" class="music-player">
    <!-- Search Bar -->
    <SearchBar
      v-model="searchQuery"
      @search="handleSearch"
    />

    <!-- Content Area -->
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
        :show-select-button="viewMode === 'collection'"
        :has-selected-non-all-music="hasSelectedNonAllMusicCollection()"
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
        @back="handleBackToPlaylists"
        @play="handlePlaySong"
      />
    </div>

    <!-- Player Controls -->
    <PlayerControls
      v-if="!selectMode"
      :current-time="currentTime"
      :duration="audioElement?.duration || 0"
      :is-playing="isPlaying"
      :play-mode="playMode"
      @seek="seekToTime"
      @toggle-play-mode="togglePlayMode"
      @previous="previousSong"
      @toggle-play="togglePlay"
      @next="nextSong"
      @show-settings="handleShowSettingsModal"
    />

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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import SearchBar from '@/components/SearchBar.vue'
import PlaylistListView from '@/components/PlaylistListView.vue'
import SongListView, { type SongInfo } from '@/components/SongListView.vue'
import PlayerControls from '@/components/PlayerControls.vue'
import SettingsModal from '@/components/modals/SettingsModal.vue'
import AddToCollectionModal from '@/components/modals/AddToCollectionModal.vue'
import CreateCollectionModal from '@/components/modals/CreateCollectionModal.vue'
import { useAudioPlayer } from '@/composables/useAudioPlayer'
import { usePlaylist } from '@/composables/usePlaylist'
import { useSelection } from '@/composables/useSelection'
import { useTimer } from '@/composables/useTimer'
import { useVolume } from '@/composables/useVolume'

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
  getAllMusic
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

// Additional state
const showSettings = ref(false)
const showAddToCollection = ref(false)
const selectedCollections = ref<number[]>([])
const newCollectionName = ref('')
const showCreateCollection = ref(false)

// Computed helper for audio player
const currentPlaylistSongs = computed(() => {
  return selectedPlaylist.value?.songs || []
})

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
onMounted(() => {
  initAudio()
  refreshData()
})
</script>

<style scoped>
.music-player {
  height: 100vh;
  display: flex;
  flex-direction: column;
  background-color: #f5f5f5;
  color: #333;
  font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
  overflow: hidden;
}

.content-area {
  flex: 1;
  overflow-y: auto;
  padding: 15px;
  background-color: #fff;
  position: relative;
}

@media (min-width: 768px) {
  .music-player {
    max-width: 500px;
    margin: 0 auto;
    box-shadow: 0 0 20px rgba(0,0,0,0.1);
  }
}
</style>
