import { ref, computed } from 'vue'
import { getApiBase } from '@/utils/api'
import { getViewMode, setViewMode } from '@/utils/cookies'

export interface MusicInfo {
  id: number
  name: string
  lufs: number
  path: string
}

export interface Playlist {
  name: string
  songs: MusicInfo[]
}

export interface Collection {
  id: number
  name: string
  created_at: string
}

export type ViewMode = 'folder' | 'collection'

const viewModeLabels: Record<ViewMode, string> = {
  folder: '文件夹',
  collection: '收藏夹'
}

export function usePlaylist() {
  // State - initialize viewMode from cookie (default: collection)
  const viewMode = ref<ViewMode>((getViewMode() as ViewMode) || 'collection')
  const playlists = ref<Record<string, MusicInfo[]>>({})
  const collections = ref<Collection[]>([])
  const searchQuery = ref('')
  const currentView = ref<'playlists' | 'songs' | 'search'>('playlists')
  const selectedPlaylist = ref<Playlist | null>(null)

  // Computed
  const playlistNames = computed(() => {
    return Object.keys(playlists.value)
  })

  const currentSongs = computed(() => {
    return selectedPlaylist.value?.songs || []
  })

  // Search behavior docs: docs/search.md
  const searchResults = computed(() => {
    if (!searchQuery.value) return []
    const query = searchQuery.value.toLowerCase()
    let scopeSongs: MusicInfo[] = []
    if (selectedPlaylist.value) {
      scopeSongs = selectedPlaylist.value.songs
    } else {
      const allMusic = playlists.value['所有音乐']
      scopeSongs = allMusic || Object.values(playlists.value).flat()
    }
    return scopeSongs.filter(song =>
      song.name.toLowerCase().includes(query)
    )
  })

  // Fetch playlists from backend (folder mode)
  const fetchPlaylists = async () => {
    try {
      const response = await fetch(`${getApiBase()}/playlists`)
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
      const response = await fetch(`${getApiBase()}/collections`)
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
      const response = await fetch(`${getApiBase()}/playlists/collection-mode`)
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
    const newMode = viewMode.value === 'folder' ? 'collection' : 'folder'
    viewMode.value = newMode
    setViewMode(newMode)
  }

  const selectPlaylist = (playlistName: string) => {
    selectedPlaylist.value = {
      name: playlistName,
      songs: playlists.value[playlistName] || []
    }
    currentView.value = 'songs'
  }

  const backToPlaylists = () => {
    currentView.value = 'playlists'
    selectedPlaylist.value = null
    searchQuery.value = ''
  }

  const showSearchResults = () => {
    if (!searchQuery.value) return
    currentView.value = 'search'
  }

  // Collection API methods
  const createCollection = async (name: string): Promise<boolean> => {
    try {
      const response = await fetch(`${getApiBase()}/collections`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: name.trim() })
      })

      if (response.ok) {
        await refreshData()
        return true
      } else {
        const error = await response.text()
        alert(error)
        return false
      }
    } catch (error) {
      console.error('Failed to create collection:', error)
      alert('创建失败')
      return false
    }
  }

  const deleteCollection = async (collectionId: number): Promise<boolean> => {
    try {
      const response = await fetch(`${getApiBase()}/collections/${collectionId}`, {
        method: 'DELETE'
      })
      return response.ok
    } catch (error) {
      console.error('Failed to delete collection:', error)
      return false
    }
  }

  const addToCollection = async (collectionId: number, musicIds: number[]): Promise<boolean> => {
    try {
      const response = await fetch(`${getApiBase()}/collections/${collectionId}/items`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ music_ids: musicIds })
      })
      return response.ok
    } catch (error) {
      console.error('Failed to add to collection:', error)
      return false
    }
  }

  const removeFromCollection = async (collectionId: number, musicIds: number[]): Promise<boolean> => {
    try {
      const response = await fetch(`${getApiBase()}/collections/${collectionId}/items`, {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ music_ids: musicIds })
      })
      return response.ok
    } catch (error) {
      console.error('Failed to remove from collection:', error)
      return false
    }
  }

  const getAllMusic = async (): Promise<any[]> => {
    const response = await fetch(`${getApiBase()}/music`)
    if (response.ok) {
      return await response.json()
    }
    return []
  }

  return {
    // State
    viewMode,
    playlists,
    collections,
    searchQuery,
    currentView,
    selectedPlaylist,
    // Computed
    playlistNames,
    currentSongs,
    searchResults,
    viewModeLabels,
    // Methods
    fetchPlaylists,
    fetchCollections,
    fetchPlaylistsCollectionMode,
    refreshData,
    toggleViewMode,
    selectPlaylist,
    backToPlaylists,
    showSearchResults,
    createCollection,
    deleteCollection,
    addToCollection,
    removeFromCollection,
    getAllMusic
  }
}
