<template>
  <div id="app" class="music-player">
    <!-- Search Bar -->
    <div class="search-bar">
      <input
        v-model="searchQuery"
        type="text"
        placeholder="搜索歌曲或歌单"
        class="search-input"
        @keyup.enter="showSearchResults"
      />
      <button class="search-button" @click="showSearchResults">
        搜索
      </button>
    </div>

    <!-- Content Area -->
    <div class="content-area">
      <!-- Playlist List -->
      <div v-if="currentView === 'playlists'" class="playlist-list">
        <div class="list-header">
          <h2>{{ viewMode === 'folder' ? '我的歌单' : '收藏夹' }}</h2>
        </div>
        <div
          v-for="playlistName in playlistNames"
          :key="playlistName"
          class="playlist-item"
          @click="selectPlaylist(playlistName)"
        >
          <div class="playlist-cover">
            <span>♪</span>
          </div>
          <div class="playlist-info">
            <h3>{{ playlistName }}</h3>
            <p>{{ playlists[playlistName]?.length || 0 }} 首歌曲</p>
          </div>
        </div>
      </div>

      <!-- Song List -->
      <div v-if="currentView === 'songs'" class="song-list">
        <div class="list-header">
          <button class="back-button" @click="backToPlaylists">
            ← 返回
          </button>
          <h2>{{ selectedPlaylist?.name }}</h2>
          <button class="select-mode-btn" @click="toggleSelectMode">
            {{ selectMode ? '取消勾选' : '勾选模式' }}
          </button>
        </div>
        <div
          v-for="(song, index) in currentSongs"
          :key="song.name"
          class="song-item"
          :class="{ 'active': currentSong?.name === song.name }"
          @click="selectMode ? toggleSongSelection(song.name) : playSongAtIndex(song, index)"
        >
          <div v-if="selectMode" class="song-checkbox">
            <input type="checkbox" :checked="selectedSongs.has(song.name)" @click.stop="toggleSongSelection(song.name)" />
          </div>
          <div class="song-info">
            <h3>{{ song.name }}</h3>
            <p>LUFS: {{ song.lufs.toFixed(2) }}</p>
          </div>
          <div class="song-duration">
            --:--
          </div>
        </div>

        <!-- Selection Mode Actions -->
        <div v-if="selectMode" class="selection-actions">
          <button
            v-if="viewMode === 'collection' && selectedPlaylist?.name !== '所有音乐'"
            class="action-btn remove-btn"
            @click="removeFromCollection"
          >
            从收藏夹移除
          </button>
          <button
            v-if="viewMode === 'folder' || selectedPlaylist?.name === '所有音乐'"
            class="action-btn add-btn"
            @click="showAddToCollectionModal"
          >
            添加到收藏夹
          </button>
        </div>
      </div>

      <!-- Search Results -->
      <div v-if="currentView === 'search'" class="search-results">
        <div class="list-header">
          <button class="back-button" @click="backToPlaylists">
            ← 返回
          </button>
          <h2>搜索结果</h2>
        </div>
        <div
          v-for="song in searchResults"
          :key="song.name"
          class="song-item"
          :class="{ 'active': currentSong?.name === song.name }"
          @click="playSong(song)"
        >
          <div class="song-info">
            <h3>{{ song.name }}</h3>
            <p>LUFS: {{ song.lufs.toFixed(2) }}</p>
          </div>
          <div class="song-duration">
            --:--
          </div>
        </div>
      </div>
    </div>

    <!-- Player Controls -->
    <div v-if="!selectMode" class="player-controls">
      <!-- Progress Bar -->
      <div class="progress-bar">
        <div class="progress-time">{{ formatTime(currentTime) }}</div>
        <input
          type="range"
          min="0"
          :max="audioElement?.duration || 0"
          :value="currentTime"
          @input="seekToTime"
          class="progress-slider"
        />
        <div class="progress-time">{{ formatTime(audioElement?.duration || 0) }}</div>
      </div>

      <!-- Control Buttons -->
      <div class="control-buttons">
        <button class="control-btn" @click="togglePlayMode">
          <span v-if="playMode === 'sequential'">↻</span>
          <span v-else-if="playMode === 'shuffle'">⤮</span>
          <span v-else>①</span>
        </button>
        <button class="control-btn" @click="previousSong">⏮</button>
        <button class="control-btn" @click="togglePlay">
          <span v-if="isPlaying">⏸</span>
          <span v-else>▶</span>
        </button>
        <button class="control-btn" @click="nextSong">⏭</button>
        <button class="control-btn" @click="showSettingsModal">≡</button>
      </div>
    </div>

    <!-- Settings Modal -->
    <div v-if="showSettings" class="modal-overlay" @click="hideSettingsModal">
      <div class="modal-content" @click.stop>
        <h3>播放器设置</h3>

        <!-- View Mode Toggle -->
        <div class="mode-toggle" @click="toggleViewMode">
          <div class="mode-label">分类方式</div>
          <div class="mode-value">{{ viewModeLabels[viewMode] }}</div>
        </div>

        <!-- Collection Management (only in collection mode) -->
        <div v-if="viewMode === 'collection'" class="collection-management">
          <h4>收藏夹管理</h4>
          <div class="create-collection">
            <input
              type="text"
              v-model="newCollectionName"
              placeholder="新收藏夹名称"
              class="collection-input"
            />
            <button class="create-btn" @click="createCollection">创建</button>
          </div>
          <div class="collection-list">
            <div
              v-for="collection in collections.filter(c => c.name !== '所有音乐')"
              :key="collection.id"
              class="collection-item-small"
            >
              <span>{{ collection.name }}</span>
              <button class="delete-collection-btn" @click="deleteCollection(collection.id)">删除</button>
            </div>
          </div>
        </div>

        <hr class="settings-divider" />

        <!-- Volume Mode Toggle -->
        <div class="mode-toggle" @click="toggleVolumeMode">
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
                v-model.number="manualVolume"
                min="0"
                max="1"
                step="0.01"
              />
              <input
                type="number"
                class="volume-input"
                v-model.number="manualVolumeInput"
                min="0"
                max="1"
                step="0.01"
                @input="manualVolume = Number(($event.target as HTMLInputElement).value)"
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
                v-model.number="fixedLufs"
                min="-100"
                max="0"
                step="1"
              />
              <input
                type="number"
                class="volume-input"
                v-model.number="fixedLufsInput"
                min="-100"
                max="0"
                step="1"
                @input="fixedLufs = Number(($event.target as HTMLInputElement).value)"
              />
              <span class="suffix">LUFS</span>
            </div>
          </div>
        </div>

        <!-- Auto volume mode has no additional controls -->

        <!-- Sleep Timer -->
        <div class="setting-item">
          <label class="setting-label">定时停止播放</label>

          <!-- Timer Status Display -->
          <div class="timer-status">{{ timerStatusDisplay }}</div>

          <!-- Timer Slider -->
          <div class="slider-container">
            <input
              type="range"
              class="volume-slider"
              v-model.number="timerMinutes"
              min="0"
              max="360"
              step="1"
            />
            <input
              type="number"
              class="volume-input"
              v-model.number="timerMinutesInput"
              min="0"
              max="360"
              step="1"
              @input="timerMinutes = Number(($event.target as HTMLInputElement).value)"
            />
            <span class="suffix">分钟</span>
          </div>

          <!-- Timer Presets -->
          <div class="timer-presets">
            <button
              v-for="preset in [15, 30, 45, 60]"
              :key="preset"
              class="timer-preset-btn"
              @click="setTimerPreset(preset)"
            >
              {{ preset }}分钟
            </button>
          </div>

          <!-- Timer Action Buttons -->
          <div class="timer-actions">
            <button v-if="timerActive" @click="cancelTimer" class="cancel-timer-btn">
              取消定时
            </button>
            <button v-else @click="startTimer" class="start-timer-btn">
              开始定时
            </button>
          </div>
        </div>

        <div class="modal-actions">
          <button @click="hideSettingsModal" class="confirm-btn">确认</button>
        </div>
      </div>
    </div>

    <!-- Add to Collection Modal -->
    <div v-if="showAddToCollection" class="modal-overlay" @click="hideAddToCollectionModal">
      <div class="modal-content" @click.stop>
        <h3>添加到收藏夹</h3>
        <div class="collection-select-list">
          <div
            v-for="collection in collections.filter(c => c.name !== '所有音乐')"
            :key="collection.id"
            class="collection-checkbox-item"
          >
            <input
              type="checkbox"
              :id="'collection-' + collection.id"
              :value="collection.id"
              v-model="selectedCollections"
            />
            <label :for="'collection-' + collection.id">{{ collection.name }}</label>
          </div>
        </div>
        <div class="modal-actions">
          <button @click="hideAddToCollectionModal" class="cancel-btn">取消</button>
          <button @click="addToCollection" class="confirm-btn">确定</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'

interface MusicInfo {
  name: string
  lufs: number
  path: string
}

interface Playlist {
  name: string
  songs: MusicInfo[]
}

interface Collection {
  id: number
  name: string
  created_at: string
}

type VolumeMode = 'auto' | 'manual' | 'fixed'
type ViewMode = 'folder' | 'collection'

// State
const searchQuery = ref('')
const currentView = ref<'playlists' | 'songs' | 'search'>('playlists')
const selectedPlaylist = ref<Playlist | null>(null)
const currentSong = ref<MusicInfo | null>(null)
const isPlaying = ref(false)
const currentTime = ref(0)
const playMode = ref<'sequential' | 'shuffle' | 'loop'>('sequential')
const showSettings = ref(false)
const playlists = ref<Record<string, MusicInfo[]>>({})
const audioElement = ref<HTMLAudioElement | null>(null)
const playedSongIndexes = ref<Set<number>>(new Set())
const currentIndex = ref(-1)
const selectMode = ref(false)
const selectedSongs = ref<Set<string>>(new Set())

// View mode state
const viewMode = ref<ViewMode>('folder')
const collections = ref<Collection[]>([])
const viewModeLabels: Record<ViewMode, string> = {
  folder: '文件夹',
  collection: '收藏夹'
}

// Add to collection modal state
const showAddToCollection = ref(false)
const selectedCollections = ref<number[]>([])
const newCollectionName = ref('')

// Volume mode state
const volumeMode = ref<VolumeMode>('auto')
const manualVolume = ref(0.5)
const manualVolumeInput = ref(0.5)
const fixedLufs = ref(-27)
const fixedLufsInput = ref(-27)
const volumeModeLabels: Record<VolumeMode, string> = {
  auto: '自动音量平衡',
  manual: '手动设置音量',
  fixed: '固定音量大小'
}

// Timer state
const timerMinutes = ref(30)
const timerMinutesInput = ref(30)
const timerActive = ref(false)
const timerRemaining = ref(0)
const timerInterval = ref<number | null>(null)

// API Configuration
const API_BASE = 'http://localhost:2080/api'

// Computed
const playlistNames = computed(() => {
  return Object.keys(playlists.value)
})

const currentSongs = computed(() => {
  if (searchQuery.value) return []
  return selectedPlaylist.value?.songs || []
})

const searchResults = computed(() => {
  if (!searchQuery.value) return []
  const query = searchQuery.value.toLowerCase()
  const allSongs = Object.values(playlists.value).flat()
  return allSongs.filter(song =>
    song.name.toLowerCase().includes(query)
  )
})

const timerStatusDisplay = computed(() => {
  if (timerActive.value) {
    const minutes = Math.floor(timerRemaining.value / 60)
    const seconds = timerRemaining.value % 60
    return `已定时: ${minutes}分${seconds.toString().padStart(2, '0')}秒后停止`
  }
  return '未启用定时'
})

// Fetch playlists from backend (folder mode)
const fetchPlaylists = async () => {
  try {
    const response = await fetch(`${API_BASE}/playlists`)
    if (response.ok) {
      playlists.value = await response.json()
    }
  } catch (error) {
    console.error('Failed to fetch playlists:', error)
  }
}

// Fetch collections from backend
const fetchCollections = async () => {
  try {
    const response = await fetch(`${API_BASE}/collections`)
    if (response.ok) {
      collections.value = await response.json()
      // Add virtual "所有音乐" collection
      collections.value.unshift({
        id: -1,
        name: '所有音乐',
        created_at: new Date().toISOString()
      })
    }
  } catch (error) {
    console.error('Failed to fetch collections:', error)
  }
}

// Fetch playlists in collection mode
const fetchPlaylistsCollectionMode = async () => {
  try {
    const response = await fetch(`${API_BASE}/playlists/collection-mode`)
    if (response.ok) {
      playlists.value = await response.json()
    }
  } catch (error) {
    console.error('Failed to fetch collection playlists:', error)
  }
}

// Refresh data based on view mode
const refreshData = async () => {
  if (viewMode.value === 'folder') {
    await fetchPlaylists()
  } else {
    await fetchCollections()
    await fetchPlaylistsCollectionMode()
  }
}

// Toggle view mode
const toggleViewMode = () => {
  viewMode.value = viewMode.value === 'folder' ? 'collection' : 'folder'
  backToPlaylists()
  refreshData()
}

// Toggle select mode
const toggleSelectMode = () => {
  selectMode.value = !selectMode.value
  selectedSongs.value.clear()
}

// Toggle song selection
const toggleSongSelection = (songName: string) => {
  if (selectedSongs.value.has(songName)) {
    selectedSongs.value.delete(songName)
  } else {
    selectedSongs.value.add(songName)
  }
}

// Show add to collection modal
const showAddToCollectionModal = () => {
  selectedCollections.value = []
  showAddToCollection.value = true
}

// Hide add to collection modal
const hideAddToCollectionModal = () => {
  showAddToCollection.value = false
  selectedCollections.value = []
}

// Add selected songs to collections
const addToCollection = async () => {
  if (selectedCollections.value.length === 0) {
    alert('请选择至少一个收藏夹')
    return
  }

  // Get music IDs for selected songs
  const allMusicResponse = await fetch(`${API_BASE}/music`)
  if (!allMusicResponse.ok) {
    alert('获取音乐列表失败')
    return
  }
  const allMusic = await allMusicResponse.json()

  const selectedMusicIds = allMusic
    .filter((m: any) => selectedSongs.value.has(m.filename))
    .map((m: any) => m.id)

  if (selectedMusicIds.length === 0) {
    alert('没有选中的歌曲')
    return
  }

  // Add to each selected collection
  for (const collectionId of selectedCollections.value) {
    try {
      await fetch(`${API_BASE}/collections/${collectionId}/items`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ music_ids: selectedMusicIds })
      })
    } catch (error) {
      console.error('Failed to add to collection:', error)
    }
  }

  alert('添加成功')
  hideAddToCollectionModal()
  selectMode.value = false
  selectedSongs.value.clear()
  await refreshData()
}

// Remove selected songs from collection
const removeFromCollection = async () => {
  if (!selectedPlaylist.value) return

  // Find the collection ID
  const collection = collections.value.find(c => c.name === selectedPlaylist.value?.name)
  if (!collection || collection.id === -1) {
    alert('无法从"所有音乐"中移除')
    return
  }

  // Get music IDs for selected songs
  const allMusicResponse = await fetch(`${API_BASE}/music`)
  if (!allMusicResponse.ok) {
    alert('获取音乐列表失败')
    return
  }
  const allMusic = await allMusicResponse.json()

  const selectedMusicIds = allMusic
    .filter((m: any) => selectedSongs.value.has(m.filename))
    .map((m: any) => m.id)

  if (selectedMusicIds.length === 0) {
    alert('没有选中的歌曲')
    return
  }

  try {
    await fetch(`${API_BASE}/collections/${collection.id}/items`, {
      method: 'DELETE',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ music_ids: selectedMusicIds })
    })
    alert('移除成功')
    selectMode.value = false
    selectedSongs.value.clear()
    await refreshData()
  } catch (error) {
    console.error('Failed to remove from collection:', error)
    alert('移除失败')
  }
}

// Create new collection
const createCollection = async () => {
  if (!newCollectionName.value.trim()) {
    alert('请输入收藏夹名称')
    return
  }

  try {
    const response = await fetch(`${API_BASE}/collections`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: newCollectionName.value.trim() })
    })

    if (response.ok) {
      newCollectionName.value = ''
      await refreshData()
    } else {
      const error = await response.text()
      alert(error)
    }
  } catch (error) {
    console.error('Failed to create collection:', error)
    alert('创建失败')
  }
}

// Delete collection
const deleteCollection = async (collectionId: number) => {
  if (!confirm('确定要删除这个收藏夹吗？')) {
    return
  }

  try {
    const response = await fetch(`${API_BASE}/collections/${collectionId}`, {
      method: 'DELETE'
    })

    if (response.ok) {
      await refreshData()
    } else {
      alert('删除失败')
    }
  } catch (error) {
    console.error('Failed to delete collection:', error)
    alert('删除失败')
  }
}

// Initialize audio element
onMounted(() => {
  audioElement.value = new Audio()
  audioElement.value.addEventListener('timeupdate', () => {
    currentTime.value = audioElement.value?.currentTime || 0
  })
  audioElement.value.addEventListener('ended', () => {
    // Auto-play next song based on mode
    if (playMode.value === 'loop') {
      // Loop mode: replay same song
      if (currentSong.value) {
        playSong(currentSong.value)
      }
    } else {
      // Sequential or shuffle mode: go to next song
      nextSong()
    }
  })

  refreshData()
})

// Methods
const selectPlaylist = (playlistName: string) => {
  selectedPlaylist.value = {
    name: playlistName,
    songs: playlists.value[playlistName] || []
  }
  // Reset played indexes when changing playlist
  playedSongIndexes.value = new Set()
  currentIndex.value = -1
  currentView.value = 'songs'
}

const backToPlaylists = () => {
  currentView.value = 'playlists'
  selectedPlaylist.value = null
  searchQuery.value = ''
  selectMode.value = false
  selectedSongs.value.clear()
}

const showSearchResults = () => {
  if (searchQuery.value && searchResults.value.length > 0) {
    currentView.value = 'search'
  }
}

// Random song index with no repeat (ported from swplayer)
const randomSongIndexNoRepeat = (): number => {
  const songs = currentSongs.value
  if (songs.length === 0) return 0

  const notPlayed = songs.length - playedSongIndexes.value.size
  if (notPlayed === 0) {
    // All songs played, reset and start over
    playedSongIndexes.value = new Set()
    return Math.floor(Math.random() * songs.length)
  }

  // Select a random unplayed song
  let count = Math.ceil(Math.random() * notPlayed)
  for (let i = 0; i < songs.length; i++) {
    if (!playedSongIndexes.value.has(i)) {
      count--
      if (count === 0) return i
    }
  }

  // Fallback (shouldn't reach here)
  return 0
}

// Set volume based on current mode
const setVolume = () => {
  if (!audioElement.value || !currentSong.value) return

  const song = currentSong.value
  let volume = 0.5

  if (volumeMode.value === 'auto') {
    // Find minimum LUFS in current playlist
    let minLufs = 1000
    for (const s of currentSongs.value) {
      minLufs = Math.min(s.lufs, minLufs)
    }
    volume = 10 ** ((minLufs - song.lufs) / 20)
  } else if (volumeMode.value === 'fixed') {
    volume = 10 ** ((fixedLufs.value - song.lufs) / 20)
  } else if (volumeMode.value === 'manual') {
    volume = manualVolume.value
  }

  audioElement.value.volume = Math.min(1, Math.max(0, volume))
}

const playSongAtIndex = async (song: MusicInfo, index: number) => {
  currentIndex.value = index
  playedSongIndexes.value.add(index)
  await playSong(song)
}

const playSong = async (song: MusicInfo) => {
  currentSong.value = song

  if (audioElement.value) {
    audioElement.value.src = `${API_BASE}/music/${encodeURIComponent(song.name)}`
    setVolume()
    try {
      await audioElement.value.play()
      isPlaying.value = true
    } catch (error) {
      console.error('Failed to play audio:', error)
    }
  }
}

const togglePlay = async () => {
  if (!audioElement.value) return

  if (isPlaying.value) {
    audioElement.value.pause()
    isPlaying.value = false
  } else {
    if (!currentSong.value && currentSongs.value.length > 0) {
      await playSongAtIndex(currentSongs.value[0], 0)
    } else if (currentSong.value) {
      await audioElement.value.play()
      isPlaying.value = true
    }
  }
}

const togglePlayMode = () => {
  if (playMode.value === 'sequential') {
    playMode.value = 'shuffle'
  } else if (playMode.value === 'shuffle') {
    playMode.value = 'loop'
  } else {
    playMode.value = 'sequential'
  }
}

const previousSong = () => {
  if (!currentSong.value || !selectedPlaylist.value) return

  const songs = selectedPlaylist.value.songs
  let newIndex: number

  if (playMode.value === 'loop') {
    // Loop mode: play same song
    newIndex = currentIndex.value
  } else if (playMode.value === 'shuffle') {
    // Shuffle mode: no-repeat random
    newIndex = randomSongIndexNoRepeat()
  } else {
    // Sequential mode: go to previous
    newIndex = currentIndex.value === 0 ? songs.length - 1 : currentIndex.value - 1
  }

  playSongAtIndex(songs[newIndex], newIndex)
}

const nextSong = () => {
  if (!currentSong.value || !selectedPlaylist.value) return

  const songs = selectedPlaylist.value.songs
  let newIndex: number

  if (playMode.value === 'loop') {
    // Loop mode: play same song
    newIndex = currentIndex.value
  } else if (playMode.value === 'shuffle') {
    // Shuffle mode: no-repeat random
    newIndex = randomSongIndexNoRepeat()
  } else {
    // Sequential mode: go to next
    newIndex = currentIndex.value === songs.length - 1 ? 0 : currentIndex.value + 1
  }

  playSongAtIndex(songs[newIndex], newIndex)
}

const seekToTime = (event: Event) => {
  const target = event.target as HTMLInputElement
  const time = parseInt(target.value)

  if (audioElement.value) {
    audioElement.value.currentTime = time
    currentTime.value = time
  }
}

const showSettingsModal = () => {
  showSettings.value = true
  // Sync input values
  manualVolumeInput.value = manualVolume.value
  fixedLufsInput.value = fixedLufs.value
}

const hideSettingsModal = () => {
  showSettings.value = false
}

const toggleVolumeMode = () => {
  const modes: VolumeMode[] = ['auto', 'manual', 'fixed']
  const currentIndex_mode = modes.indexOf(volumeMode.value)
  volumeMode.value = modes[(currentIndex_mode + 1) % modes.length]
  setVolume()
}

// Timer management
const setTimerPreset = (minutes: number) => {
  timerMinutes.value = minutes
  timerMinutesInput.value = minutes
  startTimer()
}

const startTimer = () => {
  if (timerMinutes.value > 0) {
    // Clear existing interval
    if (timerInterval.value) {
      clearInterval(timerInterval.value)
    }

    timerActive.value = true
    timerRemaining.value = timerMinutes.value * 60

    timerInterval.value = window.setInterval(() => {
      timerRemaining.value--

      if (timerRemaining.value <= 0) {
        cancelTimer()
        if (audioElement.value) {
          audioElement.value.pause()
        }
        isPlaying.value = false
        currentTime.value = 0
      }
    }, 1000)
  }
}

const cancelTimer = () => {
  if (timerInterval.value) {
    clearInterval(timerInterval.value)
    timerInterval.value = null
  }
  timerActive.value = false
  timerRemaining.value = 0
}

// Sync slider and input for volume
watch(manualVolume, (val) => {
  manualVolumeInput.value = val
  setVolume()
})

watch(fixedLufs, (val) => {
  fixedLufsInput.value = val
  setVolume()
})

watch(timerMinutes, (val) => {
  timerMinutesInput.value = val
})

// Watch for play state changes
watch(isPlaying, (playing) => {
  if (!audioElement.value) return

  if (playing) {
    audioElement.value.play()
  } else {
    audioElement.value.pause()
  }
})

onUnmounted(() => {
  if (audioElement.value) {
    audioElement.value.pause()
    audioElement.value = null
  }
  if (timerInterval.value) {
    clearInterval(timerInterval.value)
  }
})

const formatTime = (seconds: number) => {
  const mins = Math.floor(seconds / 60)
  const secs = Math.floor(seconds % 60)
  return `${mins}:${secs.toString().padStart(2, '0')}`
}
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

/* 搜索区域样式 */
.search-bar {
  padding: 15px;
  background-color: #fff;
  box-shadow: 0 2px 5px rgba(0,0,0,0.1);
  position: relative;
  z-index: 10;
  display: flex;
  gap: 8px;
  align-items: center;
}

.search-input {
  flex: 1;
  padding: 10px 15px;
  border: 1px solid #ddd;
  border-radius: 20px 0 0 20px;
  font-size: 16px;
  outline: none;
}

.search-button {
  padding: 10px 20px;
  background-color: #1db954;
  color: white;
  border: none;
  border-radius: 0 20px 20px 0;
  cursor: pointer;
  font-weight: bold;
}

.search-button:hover {
  background-color: #1ed760;
}

.search-input:focus {
  border-color: #1db954;
}

/* 内容区域样式 */
.content-area {
  flex: 1;
  overflow-y: auto;
  padding: 15px;
  background-color: #fff;
  position: relative;
}

.list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 0;
  font-size: 18px;
  font-weight: bold;
  color: #333;
  border-bottom: 1px solid #eee;
  margin-bottom: 10px;
  gap: 10px;
}

.list-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: bold;
  color: #333;
  flex: 1;
}

.back-button {
  background: none;
  border: none;
  color: #1db954;
  font-size: 14px;
  cursor: pointer;
  padding: 5px;
  white-space: nowrap;
}

.select-mode-btn {
  background-color: #1db954;
  color: white;
  border: none;
  border-radius: 5px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 14px;
  white-space: nowrap;
}

.select-mode-btn:hover {
  background-color: #1ed760;
}

.playlist-item, .song-item {
  padding: 12px 15px;
  border-bottom: 1px solid #f0f0f0;
  cursor: pointer;
  display: flex;
  align-items: center;
  transition: background-color 0.2s;
}

.playlist-item:hover, .song-item:hover {
  background-color: #f9f9f9;
}

.playlist-cover {
  width: 40px;
  height: 40px;
  background-color: #e0e0e0;
  border-radius: 5px;
  margin-right: 15px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #666;
}

.song-checkbox {
  margin-right: 10px;
}

.song-checkbox input[type="checkbox"] {
  width: 20px;
  height: 20px;
  cursor: pointer;
}

.playlist-info h3, .song-info h3 {
  margin: 0 0 5px 0;
  font-size: 16px;
  font-weight: 500;
}

.playlist-info p, .song-info p {
  margin: 0;
  color: #666;
  font-size: 12px;
}

.song-item.active {
  color: #1db954;
}

.selection-actions {
  padding: 20px;
  display: flex;
  gap: 10px;
  justify-content: center;
}

.action-btn {
  padding: 12px 24px;
  border: none;
  border-radius: 8px;
  font-size: 16px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.add-btn {
  background-color: #1db954;
  color: white;
}

.add-btn:hover {
  background-color: #1ed760;
}

.remove-btn {
  background-color: #e74c3c;
  color: white;
}

.remove-btn:hover {
  background-color: #c0392b;
}

/* 播放器控制栏样式 */
.player-controls {
  background-color: #fff;
  border-top: 1px solid #eee;
  padding: 15px;
  box-shadow: 0 -2px 10px rgba(0,0,0,0.05);
  position: relative;
  z-index: 10;
}

.progress-bar {
  display: flex;
  align-items: center;
  margin-bottom: 15px;
}

.progress-slider {
  flex: 1;
  height: 4px;
  background-color: #e0e0e0;
  border-radius: 2px;
  appearance: none;
  outline: none;
}

.progress-slider::-webkit-slider-thumb {
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
}

.progress-slider::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #1db954;
  cursor: pointer;
  border: none;
}

.progress-time {
  font-size: 12px;
  color: #888;
  min-width: 45px;
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

/* Modal Styles */
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

.modal-content h4 {
  margin: 20px 0 10px 0;
  font-size: 18px;
  font-weight: 600;
  color: #333;
}

/* Mode Toggle */
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

/* Collection Management */
.collection-management {
  margin-top: 20px;
}

.create-collection {
  display: flex;
  gap: 10px;
  margin-bottom: 15px;
}

.collection-input {
  flex: 1;
  padding: 10px 15px;
  border: 1px solid #ddd;
  border-radius: 5px;
  font-size: 14px;
  outline: none;
}

.collection-input:focus {
  border-color: #1db954;
}

.create-btn {
  padding: 10px 20px;
  background-color: #1db954;
  color: white;
  border: none;
  border-radius: 5px;
  cursor: pointer;
  font-weight: 500;
}

.create-btn:hover {
  background-color: #1ed760;
}

.collection-list {
  max-height: 200px;
  overflow-y: auto;
}

.collection-item-small {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 15px;
  background-color: #f9f9f9;
  border-radius: 5px;
  margin-bottom: 8px;
}

.delete-collection-btn {
  padding: 5px 10px;
  background-color: #e74c3c;
  color: white;
  border: none;
  border-radius: 3px;
  cursor: pointer;
  font-size: 12px;
}

.delete-collection-btn:hover {
  background-color: #c0392b;
}

.settings-divider {
  border: none;
  border-top: 1px solid #eee;
  margin: 20px 0;
}

/* Collection Select List */
.collection-select-list {
  max-height: 300px;
  overflow-y: auto;
  margin-bottom: 20px;
}

.collection-checkbox-item {
  display: flex;
  align-items: center;
  padding: 12px 15px;
  border-bottom: 1px solid #f0f0f0;
}

.collection-checkbox-item input[type="checkbox"] {
  width: 20px;
  height: 20px;
  margin-right: 10px;
  cursor: pointer;
}

.collection-checkbox-item label {
  cursor: pointer;
  flex: 1;
  font-size: 16px;
}

/* Setting Panel */
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

/* Timer Styles */
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

/* Modal Actions */
.modal-actions {
  display: flex;
  gap: 12px;
  justify-content: center;
  margin-top: 25px;
}

.confirm-btn, .cancel-btn {
  padding: 10px 20px;
  border: none;
  border-radius: 5px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.confirm-btn {
  background-color: #1db954;
  color: white;
}

.confirm-btn:hover {
  background-color: #1ed760;
}

.cancel-btn {
  background-color: #f0f0f0;
  color: #555;
}

.cancel-btn:hover {
  background-color: #e5e5e5;
}

.search-results {
  margin-top: 16px;
}

@media (min-width: 768px) {
  .music-player {
    max-width: 500px;
    margin: 0 auto;
    box-shadow: 0 0 20px rgba(0,0,0,0.1);
  }
}
</style>
