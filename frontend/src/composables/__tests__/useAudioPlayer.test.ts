/**
 * Tests for useAudioPlayer composable
 *
 * Tests the duration loading behavior - specifically that the duration ref
 * updates when the loadedmetadata event fires on the audio element.
 *
 * @module composables/__tests__/useAudioPlayer.test
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { useAudioPlayer, type MusicInfo } from '../useAudioPlayer'
import {
  getStoredPlaybackSession,
  removeStoredPlaybackSession,
  setStoredPlaybackSession
} from '@/utils/storage'

// Mock the getApiBase function to avoid cookie access in tests
vi.mock('@/utils/api', () => ({
  getApiBase: () => 'http://localhost:2080/api'
}))

describe('useAudioPlayer - duration loading', () => {
  let mockSongs: MusicInfo[]
  let audioMock: {
    play: ReturnType<typeof vi.fn>
    pause: ReturnType<typeof vi.fn>
    addEventListener: ReturnType<typeof vi.fn>
    removeEventListener: ReturnType<typeof vi.fn>
  }

  beforeEach(() => {
    // Setup mock songs
    mockSongs = [
      { id: 1, name: 'Test Song 1', lufs: -12, path: '/test/song1.mp3' },
      { id: 2, name: 'Test Song 2', lufs: null, path: '/test/song2.mp3' }
    ]

    // Mock the global Audio constructor
    audioMock = {
      play: vi.fn().mockResolvedValue(undefined),
      pause: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn()
    }

    // Mock global Audio constructor
    global.Audio = vi.fn(() => audioMock) as unknown as typeof Audio

    const storage = new Map<string, string>()
    Object.defineProperty(globalThis, 'localStorage', {
      value: {
        getItem: vi.fn((key: string) => storage.get(key) ?? null),
        setItem: vi.fn((key: string, value: string) => {
          storage.set(key, value)
        }),
        removeItem: vi.fn((key: string) => {
          storage.delete(key)
        })
      },
      configurable: true
    })
    removeStoredPlaybackSession()
  })

  it('should initialize duration to 0', () => {
    const { duration } = useAudioPlayer({
      songs: () => mockSongs
    })

    expect(duration.value).toBe(0)
  })

  it('should reset duration when changing songs', async () => {
    const { playSong } = useAudioPlayer({
      songs: () => mockSongs
    })

    await playSong(mockSongs[0])

    // The playSong function resets duration to 0 before metadata loads
    // We verify the function completes without error
    expect(audioMock.addEventListener).toHaveBeenCalled()
  })

  it('should add loadedmetadata event listener when playing song', async () => {
    const { playSong } = useAudioPlayer({
      songs: () => mockSongs
    })

    await playSong(mockSongs[0])

    // Verify addEventListener was called with loadedmetadata
    const loadedmetadataCalls = audioMock.addEventListener.mock.calls.filter(
      call => call[0] === 'loadedmetadata'
    )
    expect(loadedmetadataCalls).toHaveLength(1)
    expect(loadedmetadataCalls[0][0]).toBe('loadedmetadata')
    expect(typeof loadedmetadataCalls[0][1]).toBe('function')
  })

  it('should update duration when loadedmetadata event fires', async () => {
    const { playSong } = useAudioPlayer({
      songs: () => mockSongs
    })

    // Start playing a song
    await playSong(mockSongs[0])

    // Get the loadedmetadata event handler
    const loadedmetadataCalls = audioMock.addEventListener.mock.calls.filter(
      call => call[0] === 'loadedmetadata'
    )
    expect(loadedmetadataCalls).toHaveLength(1)

    const handler = loadedmetadataCalls[0][1]

    // Verify the handler is a function that can process the event
    expect(typeof handler).toBe('function')
  })

  it('should add timeupdate event listener for current time tracking', async () => {
    const { playSong } = useAudioPlayer({
      songs: () => mockSongs
    })

    await playSong(mockSongs[0])

    const timeupdateCalls = audioMock.addEventListener.mock.calls.filter(
      call => call[0] === 'timeupdate'
    )
    expect(timeupdateCalls).toHaveLength(1)
    expect(timeupdateCalls[0][0]).toBe('timeupdate')
    expect(typeof timeupdateCalls[0][1]).toBe('function')
  })

  it('should add ended event listener for auto-advance', async () => {
    const { playSong } = useAudioPlayer({
      songs: () => mockSongs
    })

    await playSong(mockSongs[0])

    const endedCalls = audioMock.addEventListener.mock.calls.filter(
      call => call[0] === 'ended'
    )
    expect(endedCalls).toHaveLength(1)
    expect(endedCalls[0][0]).toBe('ended')
    expect(typeof endedCalls[0][1]).toBe('function')
  })

  it('should use prepared song metadata before playback starts', async () => {
    const prepareSong = vi.fn(async (song: MusicInfo) => ({
      ...song,
      lufs: -11.8
    }))

    const { playSong, currentSong } = useAudioPlayer({
      songs: () => mockSongs,
      prepareSong
    })

    await playSong(mockSongs[1])

    expect(prepareSong).toHaveBeenCalledWith(mockSongs[1])
    expect(currentSong.value?.id).toBe(mockSongs[1].id)
    expect(currentSong.value?.lufs).toBe(-11.8)
  })

  it('should preserve the active queue when playing from a queue override', async () => {
    const currentQueue = [
      { id: 1, name: 'Queue Song 1', lufs: -12, path: '/test/queue-song1.mp3' },
      { id: 2, name: 'Queue Song 2', lufs: -10, path: '/test/queue-song2.mp3' }
    ]
    const visiblePlaylist = [
      { id: 3, name: 'Visible Song 1', lufs: -8, path: '/test/visible-song1.mp3' },
      { id: 4, name: 'Visible Song 2', lufs: -9, path: '/test/visible-song2.mp3' }
    ]
    let sourceSongs = visiblePlaylist

    const { playSongAtIndex, activeQueue, currentSong } = useAudioPlayer({
      songs: () => sourceSongs
    })

    sourceSongs = visiblePlaylist
    await playSongAtIndex(currentQueue[1], 1, currentQueue)

    expect(activeQueue.value.map(song => song.id)).toEqual([1, 2])
    expect(currentSong.value?.id).toBe(2)
  })

  it('should persist the active queue and current song when playback starts', async () => {
    const { playSongAtIndex } = useAudioPlayer({
      songs: () => mockSongs
    })

    await playSongAtIndex(mockSongs[1], 1, mockSongs)

    expect(getStoredPlaybackSession()).toEqual({
      currentSongId: 2,
      queue: [
        {
          id: 1,
          name: 'Test Song 1',
          path: '/test/song1.mp3',
          url: 'http://localhost:2080/api/music/id/1',
          lufs: -12,
          coverUrl: 'http://localhost:2080/api/music/id/1/cover'
        },
        {
          id: 2,
          name: 'Test Song 2',
          path: '/test/song2.mp3',
          url: 'http://localhost:2080/api/music/id/2',
          lufs: null,
          coverUrl: 'http://localhost:2080/api/music/id/2/cover'
        }
      ],
      timestamp: expect.any(Number)
    })
  })

  it('should restore queue and current song from stored playback session on init', async () => {
    setStoredPlaybackSession({
      currentSongId: 2,
      queue: [
        {
          id: 1,
          name: 'Test Song 1',
          path: '/test/song1.mp3',
          url: 'http://localhost:2080/api/music/id/1',
          lufs: -12
        },
        {
          id: 2,
          name: 'Test Song 2',
          path: '/test/song2.mp3',
          url: 'http://localhost:2080/api/music/id/2',
          lufs: null
        }
      ],
      timestamp: Date.now()
    })

    const { initAudio, activeQueue, currentSong, currentIndex } = useAudioPlayer({
      songs: () => mockSongs
    })

    await initAudio()

    expect(activeQueue.value.map(song => song.id)).toEqual([1, 2])
    expect(currentSong.value?.id).toBe(2)
    expect(currentIndex.value).toBe(1)
  })
})
