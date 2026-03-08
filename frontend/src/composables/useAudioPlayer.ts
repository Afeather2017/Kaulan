import { ref, watch, onUnmounted } from 'vue'
import { getApiBase } from '@/utils/api'

export interface MusicInfo {
  id: number
  name: string
  lufs: number | null
  path: string
}

export type PlayMode = 'sequential' | 'shuffle' | 'loop'

interface UseAudioPlayerOptions {
  songs: () => MusicInfo[]
  onSongEnd?: () => void
  onSongStart?: (currentSong: MusicInfo, nextSong: MusicInfo | null) => void
}

export function useAudioPlayer(options: UseAudioPlayerOptions) {
  const { songs, onSongEnd, onSongStart } = options

  // State
  const audioElement = ref<HTMLAudioElement | null>(null)
  const currentSong = ref<MusicInfo | null>(null)
  const isPlaying = ref(false)
  const currentTime = ref(0)
  const duration = ref(0)
  const playMode = ref<PlayMode>('sequential')
  const playedSongIndexes = ref<Set<number>>(new Set())
  const currentIndex = ref(-1)
  const apiBase = getApiBase()

  // Random song index with no repeat (ported from swplayer)
  const randomSongIndexNoRepeat = (): number => {
    const allSongs = songs()
    if (allSongs.length === 0) return 0

    const notPlayed = allSongs.length - playedSongIndexes.value.size
    if (notPlayed === 0) {
      // All songs played, reset and start over
      playedSongIndexes.value = new Set()
      return Math.floor(Math.random() * allSongs.length)
    }

    // Select a random unplayed song
    let count = Math.ceil(Math.random() * notPlayed)
    for (let i = 0; i < allSongs.length; i++) {
      if (!playedSongIndexes.value.has(i)) {
        count--
        if (count === 0) return i
      }
    }

    // Fallback (shouldn't reach here)
    return 0
  }

  // Get next song index based on play mode (used for pre-caching LUFS)
  const getNextSongIndex = (currentIndexValue: number): number | null => {
    const allSongs = songs()
    if (allSongs.length === 0) return null

    if (playMode.value === 'loop') {
      // Loop mode: same song
      return currentIndexValue
    } else if (playMode.value === 'shuffle') {
      // Shuffle mode: random unplayed song
      const notPlayed = allSongs.length - playedSongIndexes.value.size
      if (notPlayed <= 1) {
        // Only current song left or all played, will reset
        return null
      }
      // Find a random unplayed song that's not current
      let count = Math.ceil(Math.random() * (notPlayed - 1))
      for (let i = 0; i < allSongs.length; i++) {
        if (!playedSongIndexes.value.has(i) && i !== currentIndexValue) {
          count--
          if (count === 0) return i
        }
      }
      return null
    } else {
      // Sequential mode: next song
      return currentIndexValue === allSongs.length - 1 ? 0 : currentIndexValue + 1
    }
  }

  const playSongAtIndex = async (song: MusicInfo, index: number) => {
    currentIndex.value = index
    playedSongIndexes.value.add(index)
    await playSong(song)
  }

  // Flag to prevent double-play from watch triggering during playSong
  let isPlayingInternal = false

  const playSong = async (song: MusicInfo) => {
    // Pause and cleanup any existing audio
    if (audioElement.value && !audioElement.value.paused) {
      audioElement.value.pause()
    }

    // Create a fresh audio element for each song (prevents AbortError from src changes)
    const newAudio = new Audio()
    newAudio.src = `${apiBase}/music/id/${song.id}`
    newAudio.preload = 'auto'

    // Copy over any event listeners from the old element
    newAudio.addEventListener('loadedmetadata', () => {
      duration.value = newAudio.duration || 0
    })

    newAudio.addEventListener('timeupdate', () => {
      currentTime.value = newAudio.currentTime || 0
    })

    newAudio.addEventListener('seeked', () => {
      currentTime.value = newAudio.currentTime || 0
    })

    newAudio.addEventListener('seeking', () => {
      currentTime.value = newAudio.currentTime || 0
    })

    newAudio.addEventListener('ended', () => {
      if (playMode.value === 'loop') {
        if (currentSong.value) {
          playSong(currentSong.value)
        }
      } else {
        nextSong()
      }
      onSongEnd?.()
    })

    // Replace the old audio element
    audioElement.value = newAudio
    currentSong.value = song
    duration.value = 0  // Reset duration until metadata loads
    isPlayingInternal = true

    // Trigger onSongStart callback for LUFS pre-caching
    if (onSongStart) {
      const nextIndex = getNextSongIndex(currentIndex.value)
      const allSongs = songs()
      const nextSong = nextIndex !== null ? allSongs[nextIndex] : null
      console.log('[useAudioPlayer] Calling onSongStart with currentSong:', song.name, ', nextSong:', nextSong?.name)
      onSongStart(song, nextSong)
    } else {
      console.log('[useAudioPlayer] onSongStart callback not registered')
    }

    // Small delay to ensure src is loaded
    await new Promise(resolve => setTimeout(resolve, 50))

    try {
      await newAudio.play()
      isPlaying.value = true
    } catch (error) {
      console.error('Failed to play audio:', error)
      isPlaying.value = false
    } finally {
      isPlayingInternal = false
    }
  }

  const togglePlay = async () => {
    if (!audioElement.value) return

    if (isPlaying.value) {
      audioElement.value.pause()
      isPlaying.value = false
    } else {
      const allSongs = songs()
      if (!currentSong.value && allSongs.length > 0) {
        await playSongAtIndex(allSongs[0], 0)
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
    const allSongs = songs()
    if (!currentSong.value || allSongs.length === 0) return

    let newIndex: number

    if (playMode.value === 'loop') {
      // Loop mode: play same song
      newIndex = currentIndex.value
    } else if (playMode.value === 'shuffle') {
      // Shuffle mode: no-repeat random
      newIndex = randomSongIndexNoRepeat()
    } else {
      // Sequential mode: go to previous
      newIndex = currentIndex.value === 0 ? allSongs.length - 1 : currentIndex.value - 1
    }

    playSongAtIndex(allSongs[newIndex], newIndex)
  }

  const nextSong = () => {
    const allSongs = songs()
    if (!currentSong.value || allSongs.length === 0) return

    let newIndex: number

    if (playMode.value === 'loop') {
      // Loop mode: play same song
      newIndex = currentIndex.value
    } else if (playMode.value === 'shuffle') {
      // Shuffle mode: no-repeat random
      newIndex = randomSongIndexNoRepeat()
    } else {
      // Sequential mode: go to next
      newIndex = currentIndex.value === allSongs.length - 1 ? 0 : currentIndex.value + 1
    }

    playSongAtIndex(allSongs[newIndex], newIndex)
  }

  const seekToTime = (time: number) => {
    if (!audioElement.value) return

    // Use fastSeek if available (more efficient for streaming)
    if (typeof (audioElement.value as any).fastSeek === 'function') {
      (audioElement.value as any).fastSeek(time)
    } else {
      audioElement.value.currentTime = time
    }
  }

  const setVolume = (volume: number) => {
    if (audioElement.value) {
      audioElement.value.volume = Math.min(1, Math.max(0, volume))
    }
  }

  const resetPlaylist = () => {
    playedSongIndexes.value = new Set()
    currentIndex.value = -1
  }

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60)
    const secs = Math.floor(seconds % 60)
    return `${mins}:${secs.toString().padStart(2, '0')}`
  }

  // Initialize audio element
  const initAudio = () => {
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
      onSongEnd?.()
    })
  }

  // Watch for play state changes
  watch(isPlaying, (playing) => {
    if (!audioElement.value) return
    // Skip if playSong is internally handling playback (prevents double-play)
    if (isPlayingInternal) return

    if (playing) {
      audioElement.value.play()
    } else {
      audioElement.value.pause()
    }
  })

  // Cleanup
  onUnmounted(() => {
    if (audioElement.value) {
      audioElement.value.pause()
      audioElement.value = null
    }
  })

  return {
    // State
    audioElement,
    currentSong,
    isPlaying,
    currentTime,
    duration,
    playMode,
    currentIndex,
    // Methods
    playSong,
    playSongAtIndex,
    togglePlay,
    togglePlayMode,
    previousSong,
    nextSong,
    seekToTime,
    setVolume,
    resetPlaylist,
    formatTime,
    initAudio
  }
}
