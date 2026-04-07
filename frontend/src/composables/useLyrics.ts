/**
 * Lyrics composable for Kaulan music player
 *
 * This composable handles parsing LRC files and synchronizing lyrics with audio playback.
 * Supports both single-language and bilingual LRC formats.
 *
 * @module composables/useLyrics
 */

import { ref, computed, watch, type Ref } from 'vue'
import { getApiBase } from '@/utils/api'

/**
 * Represents a single lyric line with timestamp and text(s)
 */
export interface LyricLine {
  /** Time in seconds when this lyric should be displayed */
  time: number
  /** Array of lyric texts - single language has 1 element, bilingual has 2 */
  texts: string[]
}

/**
 * Song info interface (must match the one in SongListView)
 */
export interface SongInfo {
  id: number
  name: string
  lufs: number | null
  path: string
}

/**
 * Parse LRC content into LyricLine array
 *
 * Handles both single-language format:
 * ```
 * [00:00.54]First line
 * [00:02.52]Second line
 * ```
 *
 * And bilingual format (consecutive lines with same timestamp):
 * ```
 * [00:00.64]Japanese text
 * [00:00.64]Chinese translation
 * [00:02.14]Another Japanese line
 * [00:02.14]Another Chinese translation
 * ```
 *
 * @param content - Raw LRC file content
 * @returns Array of parsed LyricLine objects sorted by timestamp
 */
export function parseLrc(content: string): LyricLine[] {
  const lines: LyricLine[] = []
  const timeRegex = /^\[(\d{2}):(\d{2})\.(\d{2,3})\]/
  let currentTime: number | null = null
  let currentTexts: string[] = []

  const pushCurrent = () => {
    if (currentTime === null) {
      return
    }
    lines.push({ time: currentTime, texts: [...currentTexts] })
  }

  content.split('\n').forEach((rawLine) => {
    const line = rawLine.replace(/\r$/, '')
    const match = timeRegex.exec(line)

    if (match) {
      const minutes = parseInt(match[1], 10)
      const seconds = parseInt(match[2], 10)
      const milliseconds = parseInt(match[3].padEnd(3, '0'), 10)
      const timestamp = minutes * 60 + seconds + milliseconds / 1000
      const text = line.substring(match[0].length).trim()

      if (currentTime !== null && timestamp !== currentTime) {
        pushCurrent()
        currentTexts = []
      }

      if (currentTime === null || timestamp !== currentTime) {
        currentTime = timestamp
        currentTexts = []
      }

      if (text) {
        currentTexts.push(text)
      }
      return
    }

    if (currentTime !== null && line.trim()) {
      currentTexts.push(line.trim())
    }
  })

  pushCurrent()

  lines.sort((a, b) => a.time - b.time)

  return lines
}

/**
 * Load lyrics from the backend API
 *
 * @param id - Music ID
 * @returns Promise resolving to the raw LRC content, or null if not found
 */
async function loadLyrics(id: number): Promise<string | null> {
  try {
    const apiBase = getApiBase()
    const response = await fetch(`${apiBase}/lyrics/id/${id}`)

    if (response.status === 404) {
      // No lyrics available for this song
      return null
    }

    if (!response.ok) {
      console.error(`Failed to load lyrics: ${response.status} ${response.statusText}`)
      return null
    }

    return await response.text()
  } catch (error) {
    console.error('Error loading lyrics:', error)
    return null
  }
}

/**
 * Composable for lyrics management and synchronization
 *
 * @param currentSong - Ref containing the currently playing song
 * @returns Lyrics state and management functions
 */
export function useLyrics(currentSong: Ref<SongInfo | null>) {
  /** Parsed lyrics lines */
  const lyrics = ref<LyricLine[]>([])
  /** Index of the currently active lyric line (-1 if no active lyric) */
  const currentLyricIndex = ref(-1)
  /** Loading state for lyrics fetch */
  const isLoading = ref(false)
  /** Whether lyrics are available for the current song */
  const hasLyrics = computed(() => lyrics.value.length > 0)

  /**
   * Update the current lyric index based on playback time
   *
   * Finds the last lyric line whose timestamp is <= current time.
   * This handles the case where the user seeks to a different position.
   *
   * @param time - Current playback time in seconds
   */
  function updateCurrentLyric(time: number): void {
    if (lyrics.value.length === 0) {
      currentLyricIndex.value = -1
      return
    }

    // Find the last lyric that should be displayed at current time
    let index = -1
    for (let i = 0; i < lyrics.value.length; i++) {
      if (lyrics.value[i].time <= time) {
        index = i
      } else {
        break
      }
    }
    currentLyricIndex.value = index
  }

  /**
   * Load and parse lyrics for the current song
   */
  async function fetchLyrics(): Promise<void> {
    if (!currentSong.value) {
      lyrics.value = []
      currentLyricIndex.value = -1
      return
    }

    isLoading.value = true
    const content = await loadLyrics(currentSong.value.id)

    if (content) {
      lyrics.value = parseLrc(content)
      // Reset current lyric index
      currentLyricIndex.value = -1
    } else {
      lyrics.value = []
      currentLyricIndex.value = -1
    }

    isLoading.value = false
  }

  // Auto-load lyrics only when the song identity changes.
  watch(
    () => currentSong.value?.id ?? null,
    () => {
      void fetchLyrics()
    },
    { immediate: true }
  )

  return {
    lyrics,
    currentLyricIndex,
    hasLyrics,
    isLoading,
    updateCurrentLyric
  }
}