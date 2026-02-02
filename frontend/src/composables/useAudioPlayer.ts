import { ref, watch, onUnmounted } from 'vue'

export interface MusicInfo {
  name: string
  lufs: number
  path: string
}

export type PlayMode = 'sequential' | 'shuffle' | 'loop'

interface UseAudioPlayerOptions {
  songs: () => MusicInfo[]
  onSongEnd?: () => void
}

export function useAudioPlayer(options: UseAudioPlayerOptions) {
  const { songs, onSongEnd } = options

  // State
  const audioElement = ref<HTMLAudioElement | null>(null)
  const currentSong = ref<MusicInfo | null>(null)
  const isPlaying = ref(false)
  const currentTime = ref(0)
  const playMode = ref<PlayMode>('sequential')
  const playedSongIndexes = ref<Set<number>>(new Set())
  const currentIndex = ref(-1)
  const apiBase = 'http://localhost:2080/api'

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

  const playSongAtIndex = async (song: MusicInfo, index: number) => {
    currentIndex.value = index
    playedSongIndexes.value.add(index)
    await playSong(song)
  }

  const playSong = async (song: MusicInfo) => {
    currentSong.value = song

    if (audioElement.value) {
      audioElement.value.src = `${apiBase}/music/${encodeURIComponent(song.name)}`
      isPlaying.value = true
      try {
        await audioElement.value.play()
      } catch (error) {
        console.error('Failed to play audio:', error)
        isPlaying.value = false
      }
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

  const seekToTime = (event: Event) => {
    const target = event.target as HTMLInputElement
    const time = parseInt(target.value)

    if (audioElement.value) {
      audioElement.value.currentTime = time
      currentTime.value = time
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
