import { ref, watch, onUnmounted } from 'vue'
import { getApiBase } from '@/utils/api'
import { checkIsAndroid } from '@/utils/platform'
import type {
  PlaybackSession,
  PlayMode as AndroidPlayMode,
  PlayingQueue,
  NormalizationMode
} from 'music-notification-api'

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
  prepareSong?: (song: MusicInfo) => Promise<MusicInfo>
}

type MusicNotificationApi = typeof import('music-notification-api')

export function useAudioPlayer(options: UseAudioPlayerOptions) {
  const { songs, onSongEnd, onSongStart, prepareSong } = options

  const audioElement = ref<HTMLAudioElement | null>(null)
  const currentSong = ref<MusicInfo | null>(null)
  const isPlaying = ref(false)
  const currentTime = ref(0)
  const duration = ref(0)
  const playMode = ref<PlayMode>('sequential')
  const playedSongIndexes = ref<Set<number>>(new Set())
  const currentIndex = ref(-1)
  const activeQueue = ref<MusicInfo[]>([])
  const isAndroidPlayer = ref(false)
  const apiBase = getApiBase()

  const POLL_INTERVAL_MS = 1000

  let isPlayingInternal = false
  let pollingTimer: ReturnType<typeof setInterval> | null = null
  let pluginApiPromise: Promise<MusicNotificationApi> | null = null
  let lastStartedSongId: number | null = null

  const loadPluginApi = async (): Promise<MusicNotificationApi> => {
    if (pluginApiPromise === null) {
      pluginApiPromise = import('music-notification-api')
    }
    return pluginApiPromise
  }

  const buildAudioUrl = (songId: number, seekTime?: number): string => {
    let url: URL
    try {
      url = new URL(`${apiBase}/music/id/${songId}`)
    } catch {
      url = new URL(`${apiBase}/music/id/${songId}`, window.location.origin)
    }
    if (seekTime !== undefined && duration.value > 0) {
      const position = seekTime / duration.value
      url.searchParams.set('position', position.toString())
    }
    return url.toString()
  }

  const toQueueSong = (song: MusicInfo) => ({
    id: song.id,
    name: song.name,
    path: song.path,
    url: buildAudioUrl(song.id),
    lufs: song.lufs
  })

  const toMusicInfo = (song: { id: number; name: string; path: string; lufs: number | null }): MusicInfo => ({
    id: song.id,
    name: song.name,
    path: song.path,
    lufs: song.lufs
  })

  const getBaseQueue = (): MusicInfo[] => {
    const sourceSongs = songs()
    if (sourceSongs.length > 0) {
      return sourceSongs.slice()
    }
    return activeQueue.value.slice()
  }

  const shuffleSongs = (queue: MusicInfo[]): MusicInfo[] => {
    const shuffled = queue.slice()
    for (let i = shuffled.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1))
      const temp = shuffled[i]
      shuffled[i] = shuffled[j]
      shuffled[j] = temp
    }
    return shuffled
  }

  const buildQueueForMode = (selectedSong: MusicInfo, selectedIndex?: number): { queue: MusicInfo[]; index: number } => {
    const baseQueue = getBaseQueue()
    if (baseQueue.length === 0) {
      return { queue: [selectedSong], index: 0 }
    }

    const normalizedQueue = baseQueue.map(song => {
      if (song.id !== selectedSong.id) {
        return song
      }
      return selectedSong
    })

    const resolvedIndex = selectedIndex ?? normalizedQueue.findIndex(song => song.id === selectedSong.id)
    const clampedIndex = resolvedIndex >= 0 ? resolvedIndex : 0

    if (playMode.value !== 'shuffle') {
      return { queue: normalizedQueue, index: clampedIndex }
    }

    const current = normalizedQueue[clampedIndex] ?? selectedSong
    const remaining = normalizedQueue.filter((_song, index) => index !== clampedIndex)
    return {
      queue: [current, ...shuffleSongs(remaining)],
      index: 0
    }
  }

  const prepareSongForPlayback = async (song: MusicInfo): Promise<MusicInfo> => {
    if (!prepareSong) {
      return song
    }
    return await prepareSong(song)
  }

  const randomSongIndexNoRepeat = (): number => {
    const allSongs = activeQueue.value.length > 0 ? activeQueue.value : songs()
    if (allSongs.length === 0) return 0

    const notPlayed = allSongs.length - playedSongIndexes.value.size
    if (notPlayed === 0) {
      playedSongIndexes.value = new Set()
      return Math.floor(Math.random() * allSongs.length)
    }

    let count = Math.ceil(Math.random() * notPlayed)
    for (let i = 0; i < allSongs.length; i++) {
      if (!playedSongIndexes.value.has(i)) {
        count--
        if (count === 0) return i
      }
    }

    return 0
  }


  const maybeEmitSongStart = (queue: MusicInfo[], song: MusicInfo | null, index: number) => {
    if (!song) {
      lastStartedSongId = null
      return
    }
    if (lastStartedSongId === song.id) {
      return
    }
    lastStartedSongId = song.id
    if (!onSongStart) {
      return
    }

    const nextIndex = playMode.value === 'loop'
      ? index
      : (index >= 0 && index < queue.length - 1 ? index + 1 : (queue.length > 0 ? 0 : -1))
    const nextSong = nextIndex >= 0 && nextIndex < queue.length ? queue[nextIndex] : null
    onSongStart(song, nextSong)
  }

  const applyAndroidSession = (session: PlaybackSession, source = 'unknown') => {
    activeQueue.value = session.queue.songs.map(song => toMusicInfo(song))
    currentIndex.value = session.queue.currentIndex ?? -1
    const nextSong = currentIndex.value >= 0 ? activeQueue.value[currentIndex.value] ?? null : null
    if (!nextSong) {
      currentSong.value = null
    } else if (
      currentSong.value?.id !== nextSong.id ||
      currentSong.value?.lufs !== nextSong.lufs ||
      currentSong.value?.name !== nextSong.name ||
      currentSong.value?.path !== nextSong.path
    ) {
      currentSong.value = nextSong
    }
    isPlaying.value = session.runtime.isPlaying
    currentTime.value = session.runtime.positionMs / 1000
    duration.value = session.runtime.durationMs / 1000
    playMode.value = session.playMode as PlayMode
    console.log('[useAudioPlayer] applyAndroidSession', {
      source,
      currentIndex: currentIndex.value,
      currentSongId: currentSong.value?.id ?? null,
      currentSongName: currentSong.value?.name ?? null,
      queueSize: activeQueue.value.length,
      isPlaying: isPlaying.value,
      positionMs: session.runtime.positionMs,
      durationMs: session.runtime.durationMs,
      playMode: playMode.value
    })
    maybeEmitSongStart(activeQueue.value, currentSong.value, currentIndex.value)
  }

  const refreshAndroidSession = async (source = 'manual') => {
    if (!isAndroidPlayer.value) return
    const plugin = await loadPluginApi()
    const session = await plugin.getPlaybackSession()
    applyAndroidSession(session, source)
  }

  const getAndroidSessionSnapshot = async (source = 'manual'): Promise<PlaybackSession | null> => {
    if (!isAndroidPlayer.value) return null
    const plugin = await loadPluginApi()
    const session = await plugin.getPlaybackSession()
    applyAndroidSession(session, source)
    return session
  }

  const startAndroidPolling = () => {
    if (pollingTimer) return
    pollingTimer = setInterval(() => {
      void refreshAndroidSession('poll').catch(error => {
        console.warn('[useAudioPlayer] Failed to poll Android playback session:', error)
      })
    }, POLL_INTERVAL_MS)
  }

  const stopAndroidPolling = () => {
    if (!pollingTimer) return
    clearInterval(pollingTimer)
    pollingTimer = null
  }

  const syncAndroidQueue = async (selectedSong: MusicInfo, selectedIndex?: number) => {
    const plugin = await loadPluginApi()
    const { queue, index } = buildQueueForMode(selectedSong, selectedIndex)
    console.log('[useAudioPlayer] syncAndroidQueue', {
      selectedSongId: selectedSong.id,
      selectedSongName: selectedSong.name,
      selectedIndex: selectedIndex ?? null,
      resolvedIndex: index,
      queueSize: queue.length,
      playMode: playMode.value
    })
    const payload: PlayingQueue = {
      songs: queue.map(song => toQueueSong(song)),
      currentIndex: index
    }
    await plugin.setPlayingQueue(payload, playMode.value as AndroidPlayMode)
    activeQueue.value = queue
    currentIndex.value = index
    currentSong.value = queue[index] ?? selectedSong
  }

  const syncAndroidQueueState = async () => {
    if (!isAndroidPlayer.value || activeQueue.value.length === 0) {
      return
    }

    const plugin = await loadPluginApi()
    const payload: PlayingQueue = {
      songs: activeQueue.value.map(song => toQueueSong(song)),
      currentIndex: currentIndex.value >= 0 ? currentIndex.value : null
    }

    await plugin.setPlayingQueue(payload, playMode.value as AndroidPlayMode)
  }

  const restartAndroidPlayback = async (
    queue: MusicInfo[],
    index: number,
    source: string,
    seekTime?: number
  ) => {
    if (queue.length === 0) return

    const plugin = await loadPluginApi()
    const resolvedIndex = Math.min(Math.max(index, 0), queue.length - 1)
    const preparedTargetSong = await prepareSongForPlayback(queue[resolvedIndex])
    const preparedQueue = queue.map(song => {
      if (song.id !== preparedTargetSong.id) {
        return song
      }
      return preparedTargetSong
    })
    const targetSong = preparedQueue[resolvedIndex]
    const payload: PlayingQueue = {
      songs: preparedQueue.map(song => toQueueSong(song)),
      currentIndex: resolvedIndex
    }

    console.log('[useAudioPlayer] restartAndroidPlayback', {
      source,
      resolvedIndex,
      targetSongId: targetSong.id,
      targetSongName: targetSong.name,
      queueSize: preparedQueue.length,
      playMode: playMode.value
    })

    await plugin.stop()
    await plugin.setPlayingQueue(payload, playMode.value as AndroidPlayMode)

    activeQueue.value = preparedQueue
    currentIndex.value = resolvedIndex
    currentSong.value = targetSong

    await plugin.play({
      url: buildAudioUrl(targetSong.id, seekTime),
      title: targetSong.name
    })

    await refreshAndroidSession(source)
  }

  const syncAndroidPlayMode = async () => {
    if (!isAndroidPlayer.value) return
    const plugin = await loadPluginApi()
    if (playMode.value === 'shuffle' && currentSong.value) {
      await syncAndroidQueue(currentSong.value, currentIndex.value)
      return
    }
    await plugin.setPlayMode(playMode.value as AndroidPlayMode)
    await refreshAndroidSession()
  }

  const playSongAtIndex = async (song: MusicInfo, index: number) => {
    currentIndex.value = index
    playedSongIndexes.value.add(index)
    await playSong(song)
  }

  const playSong = async (song: MusicInfo, seekTime?: number) => {
    if (isAndroidPlayer.value) {
      const { queue, index } = buildQueueForMode(song, currentIndex.value >= 0 ? currentIndex.value : undefined)
      await restartAndroidPlayback(queue, index, 'playSong', seekTime)
      return
    }

    const preparedSong = await prepareSongForPlayback(song)
    activeQueue.value = songs().map(sourceSong => {
      if (sourceSong.id !== preparedSong.id) {
        return sourceSong
      }
      return preparedSong
    })

    if (audioElement.value && !audioElement.value.paused) {
      audioElement.value.pause()
    }

    const newAudio = new Audio()
    const sourceUrl = buildAudioUrl(preparedSong.id, seekTime)
    newAudio.src = sourceUrl
    newAudio.preload = 'auto'

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
          void playSong(currentSong.value)
        }
      } else {
        void nextSong()
      }
      onSongEnd?.()
    })

    audioElement.value = newAudio
    currentSong.value = preparedSong
    duration.value = 0
    isPlayingInternal = true

    maybeEmitSongStart(activeQueue.value, preparedSong, currentIndex.value)

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

  const play = async () => {
    if (isAndroidPlayer.value) {
      const plugin = await loadPluginApi()
      if (currentSong.value) {
        await plugin.play({
          url: buildAudioUrl(currentSong.value.id),
          title: currentSong.value.name
        })
      } else if (activeQueue.value.length > 0) {
        const index = currentIndex.value >= 0 ? currentIndex.value : 0
        await playSongAtIndex(activeQueue.value[index], index)
        return
      } else {
        const sourceSongs = songs()
        if (sourceSongs.length > 0) {
          await playSongAtIndex(sourceSongs[0], 0)
          return
        }
      }
      await refreshAndroidSession()
      return
    }

    if (!audioElement.value) return

    const allSongs = songs()
    if (!currentSong.value && allSongs.length > 0) {
      await playSongAtIndex(allSongs[0], 0)
      return
    }

    if (currentSong.value) {
      await audioElement.value.play()
      isPlaying.value = true
    }
  }

  const pause = async () => {
    if (isAndroidPlayer.value) {
      const plugin = await loadPluginApi()
      await plugin.pause()
      await refreshAndroidSession()
      return
    }

    if (!audioElement.value) return

    audioElement.value.pause()
    isPlaying.value = false
  }

  const togglePlayMode = async () => {
    if (playMode.value === 'sequential') {
      playMode.value = 'shuffle'
    } else if (playMode.value === 'shuffle') {
      playMode.value = 'loop'
    } else {
      playMode.value = 'sequential'
    }

    if (isAndroidPlayer.value) {
      await syncAndroidPlayMode()
    }
  }

  const previousSong = async () => {
    if (isAndroidPlayer.value) {
      const session = await getAndroidSessionSnapshot('previousSong:before')
      if (!session) return

      const queue = session.queue.songs.map(song => toMusicInfo(song))
      if (queue.length === 0) return

      const sessionIndex = session.queue.currentIndex ?? 0
      const newIndex = playMode.value === 'loop'
        ? sessionIndex
        : (sessionIndex <= 0 ? queue.length - 1 : sessionIndex - 1)

      await restartAndroidPlayback(queue, newIndex, 'previousSong')
      return
    }

    const allSongs = activeQueue.value.length > 0 ? activeQueue.value : songs()
    if (!currentSong.value || allSongs.length === 0) return

    let newIndex: number

    if (playMode.value === 'loop') {
      newIndex = currentIndex.value
    } else if (playMode.value === 'shuffle') {
      newIndex = randomSongIndexNoRepeat()
    } else {
      newIndex = currentIndex.value === 0 ? allSongs.length - 1 : currentIndex.value - 1
    }

    await playSongAtIndex(allSongs[newIndex], newIndex)
  }

  const nextSong = async () => {
    if (isAndroidPlayer.value) {
      const session = await getAndroidSessionSnapshot('nextSong:before')
      if (!session) return

      const queue = session.queue.songs.map(song => toMusicInfo(song))
      if (queue.length === 0) return

      const sessionIndex = session.queue.currentIndex ?? 0
      const newIndex = playMode.value === 'loop'
        ? sessionIndex
        : (sessionIndex >= queue.length - 1 ? 0 : sessionIndex + 1)

      await restartAndroidPlayback(queue, newIndex, 'nextSong')
      return
    }

    const allSongs = activeQueue.value.length > 0 ? activeQueue.value : songs()
    if (!currentSong.value || allSongs.length === 0) return

    let newIndex: number

    if (playMode.value === 'loop') {
      newIndex = currentIndex.value
    } else if (playMode.value === 'shuffle') {
      newIndex = randomSongIndexNoRepeat()
    } else {
      newIndex = currentIndex.value === allSongs.length - 1 ? 0 : currentIndex.value + 1
    }

    await playSongAtIndex(allSongs[newIndex], newIndex)
  }

  const seekToTime = async (time: number) => {
    if (isAndroidPlayer.value) {
      if (duration.value === 0) return
      const plugin = await loadPluginApi()
      console.log('[useAudioPlayer] seekToTime(android): invoking plugin.seek()', {
        targetTimeSeconds: time,
        targetPositionMs: Math.max(0, Math.floor(time * 1000)),
        currentIndex: currentIndex.value,
        currentSongId: currentSong.value?.id ?? null
      })
      await plugin.seek(Math.max(0, Math.floor(time * 1000)))
      await refreshAndroidSession('seekToTime')
      return
    }

    if (!audioElement.value || duration.value === 0) return

    const clampedTime = Math.max(0, Math.min(time, duration.value))
    audioElement.value.currentTime = clampedTime
    currentTime.value = clampedTime
  }

  const applyWebVolume = (volume: number) => {
    const normalizedVolume = Math.min(1, Math.max(0, volume))
    if (audioElement.value) {
      audioElement.value.volume = normalizedVolume
    }
  }

  const syncNormalizationConfig = async (
    mode: NormalizationMode,
    manualVolume: number,
    fixedLufs: number,
    currentVolume: number
  ) => {
    if (isAndroidPlayer.value) {
      const plugin = await loadPluginApi()
      await plugin.setNormalizationConfig({
        mode,
        manualVolume: Math.min(1, Math.max(0, manualVolume)),
        fixedLufs
      })
      return
    }

    applyWebVolume(currentVolume)
  }

  const setTimedPause = async (delayMs: number) => {
    if (!isAndroidPlayer.value) {
      return
    }
    const plugin = await loadPluginApi()
    await plugin.pauseAfter(Math.max(0, Math.floor(delayMs)))
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

  const initAudio = async () => {
    isAndroidPlayer.value = await checkIsAndroid()
    if (isAndroidPlayer.value) {
      startAndroidPolling()
      await refreshAndroidSession('initAudio')
      return
    }

    audioElement.value = new Audio()
    audioElement.value.addEventListener('timeupdate', () => {
      currentTime.value = audioElement.value?.currentTime || 0
    })
    audioElement.value.addEventListener('ended', () => {
      if (playMode.value === 'loop') {
        if (currentSong.value) {
          void playSong(currentSong.value)
        }
      } else {
        void nextSong()
      }
      onSongEnd?.()
    })
  }

  watch(isPlaying, (playing) => {
    if (isAndroidPlayer.value) return
    if (!audioElement.value) return
    if (isPlayingInternal) return

    if (playing) {
      void audioElement.value.play()
    } else {
      audioElement.value.pause()
    }
  })

  onUnmounted(() => {
    stopAndroidPolling()
    if (audioElement.value) {
      audioElement.value.pause()
      audioElement.value = null
    }
  })

  return {
    audioElement,
    activeQueue,
    currentSong,
    isPlaying,
    currentTime,
    duration,
    playMode,
    currentIndex,
    play,
    pause,
    playSong,
    playSongAtIndex,
    togglePlayMode,
    previousSong,
    nextSong,
    seekToTime,
    setTimedPause,
    resetPlaylist,
    formatTime,
    initAudio,
    refreshAndroidSession,
    isAndroidPlayer,
    syncAndroidQueueState,
    syncNormalizationConfig
  }
}
