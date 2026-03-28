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
      <div v-if="isScanning" class="scanning-message">
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
              :show-lufs="showLufs"
              @back="handleBackToPlaylists"
              @toggle-select-mode="toggleSelectMode"
              @toggle-selection="toggleSongSelection"
              @play="handlePlaySong"
              @remove="handleRemoveFromCollection"
              @show-add-modal="handleShowAddToCollectionModal"
            />

            <!-- Search Results -->
            <div v-if="currentView === 'search'">
              <SongListView
                v-if="searchResults.length > 0"
                title="搜索结果"
                :songs="searchResults"
                :select-mode="false"
                :selected-songs="new Set()"
                :show-remove-button="false"
                :show-add-button="false"
                :show-header="false"
                :show-lufs="showLufs"
                @back="handleBackToPlaylists"
                @play="handlePlaySong"
              />
              <div v-else class="empty-state">
                未找到匹配的歌曲
              </div>
            </div>
          </div>
        </div>

        <div class="right-panel" v-if="isWideLayout || showLyric">
          <div class="right-panel-content">
            <div v-if="showLyric" class="lyric-panel">
              <div v-if="!hasLyrics" class="lyric-empty">暂无歌词</div>
              <div v-else class="lyric-container" ref="lyricContainerRef">
                <div v-for="(line, index) in lyrics"
                     :key="index"
                     :class="['lyric-line', { active: index === currentLyricIndex }]">
                  <!-- Display all language versions for this timestamp -->
                  <template v-for="(text, textIndex) in line.texts" :key="textIndex">
                    <div :class="['lyric-text', `lyric-lang-${textIndex}`]">
                      {{ text || '\u00A0' }}
                    </div>
                  </template>
                </div>
              </div>
            </div>
            <div v-else class="cover-panel"></div>

            <PlayerControls
              v-if="isWideLayout && !selectMode"
              :current-time="currentTime"
              :duration="duration"
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
        :duration="duration"
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
      :show-lufs="showLufs"
      @close="hideSettingsModal"
      @toggle-view-mode="handleToggleViewMode"
      @toggle-volume-mode="handleToggleVolumeMode"
      @update:manual-volume="manualVolume = $event"
      @update:manual-volume-input="manualVolumeInput = $event"
      @update:fixed-lufs="fixedLufs = $event"
      @update:fixed-lufs-input="fixedLufsInput = $event"
      @update:timer-minutes="timerMinutes = $event"
      @update:timer-minutes-input="timerMinutesInput = $event"
      @set-timer-preset="handleSetTimerPreset"
      @start-timer="handleStartTimer"
      @cancel-timer="handleCancelTimer"
      @directory-changed="handleDirectoryChanged"
      @database-updated="handleDatabaseUpdated"
      @database-update-start="handleDatabaseUpdateStart"
      @database-update-end="handleDatabaseUpdateEnd"
      @open-upload-modal="showUploadModal = true"
      @update:show-lufs="showLufs = $event"
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
      :playlist="playbackPlaylist"
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
import { useLyrics } from '@/composables/useLyrics'
import { getApiBase } from '@/utils/api'
import { getShowLufs } from '@/utils/storage'
import { checkIsAndroid } from '@/utils/platform'

// Search behavior docs: docs/search.md
// Use composables
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

const playbackSource = ref<'playlist' | 'search'>('playlist')
const searchPlaybackSongs = ref<SongInfo[]>([])
const lastPlayedPlaylist = ref<{ name: string; songs: SongInfo[] } | null>(null)

const sourcePlaybackSongs = computed(() => {
  if (playbackSource.value === 'search') {
    return searchPlaybackSongs.value
  }
  return lastPlayedPlaylist.value?.songs || []
})

const playbackQueueTitle = computed(() => {
  if (lastPlayedPlaylist.value?.name) {
    return lastPlayedPlaylist.value.name
  }
  if (playbackSource.value === 'search') {
    return '搜索结果'
  }
  return '当前播放列表'
})

const playbackPlaylist = computed(() => {
  if (activeQueue.value.length === 0 && sourcePlaybackSongs.value.length === 0) {
    return null
  }
  return {
    name: playbackQueueTitle.value,
    songs: activeQueue.value.length > 0 ? activeQueue.value : sourcePlaybackSongs.value
  }
})

// Handler for song start event - trigger LUFS pre-caching for next song
// Defined as ref to allow useAudioPlayer to reference it, but implemented below
const handleSongStartRef = ref<((currentSongInfo: { id: number }, nextSongInfo: { id: number } | null) => void) | null>(null)

const {
  audioElement,
  activeQueue,
  currentSong,
  isPlaying,
  currentTime,
  duration,
  playMode,
  playSong,
  playSongAtIndex,
  togglePlay,
  togglePlayMode,
  previousSong,
  nextSong,
  seekToTime,
  setVolume,
  setTimedPause,
  resetPlaylist,
  initAudio,
  refreshAndroidSession,
  isAndroidPlayer
} = useAudioPlayer({
  songs: () => sourcePlaybackSongs.value,
  onSongEnd: () => {},
  onSongStart: (currentSongInfo, nextSongInfo) => {
    handleSongStartRef.value?.(currentSongInfo, nextSongInfo)
  }
})

const playbackSongs = computed(() => {
  return activeQueue.value.length > 0 ? activeQueue.value : sourcePlaybackSongs.value
})

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
  startTimer,
  cancelTimer
} = useTimer(() => {
  // Timer complete callback
  if (isAndroidPlayer.value) {
    void refreshAndroidSession()
  } else if (isPlaying.value) {
    void togglePlay()
  } else if (audioElement.value) {
    audioElement.value.pause()
  }
})

// Lyrics composable for synchronized lyrics display
const { lyrics, currentLyricIndex, hasLyrics, updateCurrentLyric } = useLyrics(currentSong)

// Additional state
const showSettings = ref(false)
const showLufs = ref(getShowLufs())
const showAddToCollection = ref(false)
const selectedCollections = ref<number[]>([])
const newCollectionName = ref('')
const showCreateCollection = ref(false)
const showUploadModal = ref(false)
const showCurrentPlaylistModal = ref(false)
const showLyric = ref(false)
const lyricContainerRef = ref<HTMLElement | null>(null)
const isWideLayout = ref(false)
const hasUserToggledLyric = ref(false)
const isScanning = ref(false)
let androidBackListener: { unregister(): Promise<void> } | null = null

const triggerDatabaseUpdate = async () => {
  try {
    isScanning.value = true
    console.log('[app] onMounted: triggering startup database scan')
    const response = await fetch(`${getApiBase()}/database/update?startup=true`, { method: 'POST' })
    if (!response.ok) {
      const errorText = await response.text()
      console.warn('[app] onMounted: database update failed:', response.status, errorText)
      return
    }
    const result = await response.json()
    if (!result.success) {
      console.warn('[app] onMounted: database update returned failure:', result.message)
    } else {
      console.log('[app] onMounted: database update completed')
    }
  } catch (error) {
    console.error('[app] onMounted: database update error:', error)
  } finally {
    isScanning.value = false
  }
}

// Computed helper for audio player
const {
  volumeMode,
  manualVolume,
  manualVolumeInput,
  fixedLufs,
  fixedLufsInput,
  volumeModeLabels,
  calculateVolume,
  toggleVolumeMode
} = useVolume(currentSong, playbackSongs)

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

// Watch for volume changes and update audio.
// Track the current song by ID so Android polling does not retrigger volume updates every second.
watch([volumeMode, manualVolume, fixedLufs, () => currentSong.value?.id ?? null], () => {
  void setVolume(calculateVolume())
}, { deep: true })

// Watch for view mode changes
watch(viewMode, async () => {
  backToPlaylists()
  await refreshData()
})

// Watch currentTime to update lyrics
watch(currentTime, (newTime) => {
  updateCurrentLyric(newTime)
})

// Helper function to scroll to current lyric
const scrollToCurrentLyric = () => {
  if (!lyricContainerRef.value || currentLyricIndex.value < 0) return

  // Get all lyric line elements
  const lyricLines = lyricContainerRef.value.querySelectorAll('.lyric-line')
  if (lyricLines.length === 0) return

  const activeLine = lyricLines[currentLyricIndex.value] as HTMLElement
  if (!activeLine) return

  // Scroll the active line to center of container
  activeLine.scrollIntoView({
    behavior: 'smooth',
    block: 'center'
  })
}

// Auto-scroll to current lyric when index changes
watch(currentLyricIndex, scrollToCurrentLyric)

// Scroll to current lyric when lyric panel is shown
watch(showLyric, (isShown) => {
  if (isShown) {
    // Use setTimeout to ensure the DOM is rendered after v-show takes effect
    setTimeout(scrollToCurrentLyric, 50)
  }
})

// Event handlers
const handleSearch = () => {
  showSearchResults()
}

const handleSelectPlaylist = (name: string) => {
  selectPlaylist(name)
  lastPlayedPlaylist.value = {
    name: name,
    songs: playlists.value[name] || []
  }
  playbackSource.value = 'playlist'
  searchPlaybackSongs.value = []
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
  // In wide layout, lyrics are always visible - don't toggle them off
  if (showLyric.value && !isWideLayout.value) {
    showLyric.value = false
    return
  }
  handleBackToPlaylists()
}

const closeTopOverlay = () => {
  if (showCurrentPlaylistModal.value) {
    showCurrentPlaylistModal.value = false
    return true
  }

  if (showUploadModal.value) {
    showUploadModal.value = false
    return true
  }

  if (showCreateCollection.value) {
    hideCreateCollectionModal()
    return true
  }

  if (showAddToCollection.value) {
    hideAddToCollectionModal()
    return true
  }

  if (showSettings.value) {
    hideSettingsModal()
    return true
  }

  return false
}

const handleAndroidBackPress = () => {
  if (closeTopOverlay()) {
    return true
  }

  if (selectMode.value) {
    selectMode.value = false
    selectedSongs.value.clear()
    return true
  }

  if (collectionSelectMode.value) {
    collectionSelectMode.value = false
    selectedCollectionsList.value.clear()
    return true
  }

  // Match the visible "返回" button behavior on mobile.
  if (showLyric.value && !isWideLayout.value) {
    showLyric.value = false
    return true
  }

  if (currentView.value !== 'playlists') {
    handleBackToPlaylists()
    return true
  }

  return false
}

const registerAndroidBackHandler = async () => {
  const isAndroid = await checkIsAndroid()
  if (!isAndroid) {
    return
  }

  try {
    const [{ onBackButtonPress }, { getCurrentWindow }] = await Promise.all([
      import('@tauri-apps/api/app'),
      import('@tauri-apps/api/window')
    ])

    androidBackListener = await onBackButtonPress(async ({ canGoBack }) => {
      if (handleAndroidBackPress()) {
        return
      }

      if (canGoBack) {
        window.history.back()
        return
      }

      await getCurrentWindow().close()
    })
  } catch (error) {
    console.warn('[app] Failed to register Android back handler:', error)
  }
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
  if (currentView.value === 'search') {
    playbackSource.value = 'search'
    searchPlaybackSongs.value = searchResults.value.slice()
    lastPlayedPlaylist.value = null
  } else {
    playbackSource.value = 'playlist'
    searchPlaybackSongs.value = []
    if (selectedPlaylist.value) {
      lastPlayedPlaylist.value = {
        name: selectedPlaylist.value.name,
        songs: selectedPlaylist.value.songs
      }
    }
  }
  if (index !== undefined) {
    await playSongAtIndex(song, index)
  } else {
    await playSong(song)
  }
}

// Handle song start event - trigger LUFS pre-caching for current and next song
const handleSongStart = async (currentSongInfo: { id: number }, nextSongInfo: { id: number } | null) => {
  console.log('[app] onSongStart called: currentSongId =', currentSongInfo.id, ', nextSongInfo =', nextSongInfo)

  // Fire-and-forget: Pre-cache LUFS for CURRENT song if it has no LUFS
  const allSongs = playbackSongs.value
  const currentSong = allSongs.find((s: { id: number }) => s.id === currentSongInfo.id)
  if (currentSong && currentSong.lufs === null) {
    console.log('[app] Current song has no LUFS, triggering fire-and-forget calculation for ID:', currentSongInfo.id)
    // Fire without await - don't block playback
    fetch(`${getApiBase()}/music/${currentSongInfo.id}/precache-lufs`, { method: 'POST' })
      .catch(error => console.warn('[app] Fire-and-forget LUFS request failed:', error))
  }

  if (!nextSongInfo) {
    console.log('[app] No next song, skipping pre-cache')
    return
  }

  // Skip pre-caching if next song already has LUFS calculated
  const nextSong = allSongs.find((s: { id: number }) => s.id === nextSongInfo.id)
  if (nextSong && nextSong.lufs !== null) {
    console.log('[app] Next song already has LUFS:', nextSong.lufs, ', skipping pre-cache')
    return
  }

  // Skip pre-caching in loop mode (same song) if it already has LUFS or will be re-calculated anyway
  if (playMode.value === 'loop' && currentSongInfo.id === nextSongInfo.id) {
    console.log('[app] Loop mode with same song, skipping pre-cache')
    return
  }

  console.log('[app] Pre-caching LUFS for next song ID:', nextSongInfo.id)

  try {
    const response = await fetch(`${getApiBase()}/music/${nextSongInfo.id}/precache-lufs`, {
      method: 'POST'
    })
    if (response.ok) {
      const result = await response.json()
      if (result.success && result.lufs !== null) {
        console.log('[app] LUFS pre-cache complete (already cached):', result.lufs)
        // Refresh the current playlist data to get the updated LUFS value
        await refreshData()
        // If a playlist is currently selected, update its songs
        if (selectedPlaylist.value) {
          const playlistName = selectedPlaylist.value.name
          selectPlaylist(playlistName)
        }
      } else if (result.success && result.cached === false) {
        console.log('[app] LUFS pre-cache started in background (non-blocking)')
      }
    } else {
      console.warn('[app] LUFS pre-cache failed:', response.status)
    }
  } catch (error) {
    console.error('[app] LUFS pre-cache error:', error)
  }
}

// Assign the handler to the ref so useAudioPlayer can call it
handleSongStartRef.value = handleSongStart

const handleShowSettingsModal = () => {
  showSettings.value = true
}

const handleShowCurrentPlaylist = () => {
  showCurrentPlaylistModal.value = true
}

const handleToggleLyric = () => {
  // In wide layout, lyrics are always visible - don't allow toggle
  if (isWideLayout.value) {
    return
  }
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

const handleStartTimer = async () => {
  if (isAndroidPlayer.value) {
    await setTimedPause(timerMinutes.value * 60 * 1000)
  }
  startTimer()
}

const handleSetTimerPreset = async (minutes: number) => {
  timerMinutes.value = minutes
  timerMinutesInput.value = minutes
  await handleStartTimer()
}

const handleCancelTimer = async () => {
  cancelTimer()
  if (isAndroidPlayer.value) {
    await setTimedPause(0)
  }
}

const handleDirectoryChanged = () => {
  // Refresh data when directory changes
  refreshData()
  void refreshAndroidSession()
}

const handleDatabaseUpdated = async () => {
  // Refresh data when database is updated
  await refreshData()
  await refreshAndroidSession()
}

const handleDatabaseUpdateStart = () => {
  isScanning.value = true
}

const handleDatabaseUpdateEnd = () => {
  isScanning.value = false
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
  // Startup scan flow: docs/startup-scan.md
  await triggerDatabaseUpdate()

  await refreshData()
  await initAudio()
  await registerAndroidBackHandler()
  updateLayoutMode()
  window.addEventListener('resize', updateLayoutMode)
})

onBeforeUnmount(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('resize', updateLayoutMode)
  }
  if (androidBackListener) {
    void androidBackListener.unregister()
    androidBackListener = null
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
  padding: 0 12px;
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
  padding: 12px;
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

.empty-state {
  padding: 24px 16px;
  text-align: center;
  color: #888;
  font-size: 14px;
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
  overflow-y: auto;
  background-color: #fafafa;
}

.lyric-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  font-size: 16px;
  color: #999;
}

.lyric-container {
  padding: 40px 20px;
  display: flex;
  flex-direction: column;
  gap: 24px;
  align-items: center;
}

.lyric-line {
  display: flex;
  flex-direction: column;
  gap: 4px;
  text-align: center;
  transition: all 0.3s ease;
  padding: 8px 16px;
}

.lyric-text {
  font-size: 16px;
  color: #999;
  line-height: 1.6;
}

/* Original language (first text) */
.lyric-lang-0 {
  font-weight: 500;
}

/* Translation language (second text) - smaller */
.lyric-lang-1 {
  font-size: 14px;
  opacity: 0.8;
}

.lyric-line.active .lyric-text {
  color: #1db954;
  font-weight: 600;
}

.lyric-line.active .lyric-lang-0 {
  font-size: 20px;
}

.lyric-line.active .lyric-lang-1 {
  font-size: 16px;
  opacity: 0.9;
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
  padding: 0 15px;
  background-color: #fff;
  position: relative;
  box-sizing: border-box;
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
