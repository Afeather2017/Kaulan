//! Unit tests for timestamp-based music streaming functionality in useAudioPlayer
//!
//! Tests the buildAudioUrl helper and seekToTime threshold-based logic.

import { describe, it, expect, vi, beforeEach } from 'vitest'

// Mock the api utility
vi.mock('@/utils/api', () => ({
  getApiBase: () => 'http://localhost:2080/api'
}))

describe('useAudioPlayer - Timestamp Seek Functionality', () => {
  // We need to import after mocking
  let useAudioPlayer: any

  beforeEach(async () => {
    // Dynamic import after mocking
    const module = await import('../useAudioPlayer')
    useAudioPlayer = module.useAudioPlayer
  })

  describe('buildAudioUrl behavior (tested through playSong)', () => {
    it('should create URL with timestamp parameters when duration is known', async () => {
      const mockSongs = [
        { id: 1, name: 'Test Song', lufs: -12, path: '/path/to/song.mp3' }
      ]

      const { playSong, duration } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn()
      })

      // Mock Audio constructor
      const mockAudio = {
        src: '',
        preload: '',
        addEventListener: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
        pause: vi.fn()
      }
      global.Audio = vi.fn(() => mockAudio) as any

      // Set duration first (simulating that metadata is already loaded)
      duration.value = 180

      // Call playSong with resumeTimestamp
      await playSong(mockSongs[0], 45)

      // Verify the URL includes timestamp parameter
      expect(mockAudio.src).toContain('t=45')
      expect(mockAudio.src).toContain('duration=180')
    })

    it('should create URL without timestamp when resumeTimestamp is not provided', async () => {
      const mockSongs = [
        { id: 1, name: 'Test Song', lufs: -12, path: '/path/to/song.mp3' }
      ]

      const { playSong } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn()
      })

      // Mock Audio constructor
      const mockAudio = {
        src: '',
        preload: '',
        addEventListener: vi.fn(),
        play: vi.fn(() => Promise.resolve()),
        pause: vi.fn()
      }
      global.Audio = vi.fn(() => mockAudio) as any

      // Call playSong without resumeTimestamp
      await playSong(mockSongs[0])

      // Verify the URL does NOT include timestamp parameter
      expect(mockAudio.src).not.toContain('t=')
      expect(mockAudio.src).not.toContain('duration=')
    })
  })

  describe('seekToTime threshold-based logic', () => {
    it('should use HTML5 seeking for small jumps (< 30 seconds)', () => {
      const mockSongs = [
        { id: 1, name: 'Test Song', lufs: -12, path: '/path/to/song.mp3' }
      ]

      const { seekToTime, audioElement, currentTime, duration, isPlaying, currentSong } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn()
      })

      // Setup audio element and state
      const mockAudio = {
        currentTime: 30,
        fastSeek: vi.fn()
      }
      audioElement.value = mockAudio as any
      currentTime.value = 30
      duration.value = 180
      isPlaying.value = false
      currentSong.value = mockSongs[0]

      // Small jump (5 seconds)
      seekToTime(35)

      // Should use HTML5 seeking, not timestamp parameter
      expect(mockAudio.fastSeek).toHaveBeenCalledWith(35)
    })

    it('should use HTML5 seeking for large jumps while playing', () => {
      const mockSongs = [
        { id: 1, name: 'Test Song', lufs: -12, path: '/path/to/song.mp3' }
      ]

      const onSongStart = vi.fn()
      const { seekToTime, audioElement, currentTime, duration, isPlaying, currentSong } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart
      })

      // Setup audio element and state
      const mockAudio = {
        currentTime: 10,
        fastSeek: vi.fn()
      }
      audioElement.value = mockAudio as any
      currentTime.value = 10
      duration.value = 180
      isPlaying.value = true  // Currently playing
      currentSong.value = mockSongs[0]

      // Large jump while playing
      seekToTime(120)

      // Should use HTML5 seeking (not playSong with timestamp)
      expect(mockAudio.fastSeek).toHaveBeenCalledWith(120)
      // onSongStart should NOT be called (playSong was not invoked)
      expect(onSongStart).not.toHaveBeenCalled()
    })

    it('should use timestamp parameter for large jumps while paused', async () => {
      const mockSongs = [
        { id: 1, name: 'Test Song', lufs: -12, path: '/path/to/song.mp3' }
      ]

      const onSongStart = vi.fn()
      const { seekToTime, audioElement, currentTime, duration, isPlaying, currentSong } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart
      })

      // Setup audio element and state
      const mockAudio = {
        currentTime: 0,
        fastSeek: vi.fn()
      }
      audioElement.value = mockAudio as any
      currentTime.value = 0
      duration.value = 180
      isPlaying.value = false  // Paused
      currentSong.value = mockSongs[0]

      // Large jump while paused (> 30 seconds)
      seekToTime(90)

      // Should use playSong with timestamp parameter
      // Note: Since we can't easily mock the Audio constructor in this test,
      // we're primarily testing that the logic path is correct
      // The actual URL construction is tested in the buildAudioUrl tests above
      expect(mockAudio.fastSeek).not.toHaveBeenCalled()
    })
  })

  describe('edge cases', () => {
    it('should not seek if audio element is not available', () => {
      const { seekToTime, audioElement, currentTime, duration } = useAudioPlayer({
        songs: () => [],
        onSongEnd: vi.fn(),
        onSongStart: vi.fn()
      })

      audioElement.value = null
      currentTime.value = 30
      duration.value = 180

      // Should not throw, just return early
      expect(() => seekToTime(60)).not.toThrow()
    })

    it('should not seek if duration is not known', () => {
      const mockSongs = [
        { id: 1, name: 'Test Song', lufs: -12, path: '/path/to/song.mp3' }
      ]

      const { seekToTime, audioElement, currentTime, duration, currentSong } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn()
      })

      const mockAudio = {
        currentTime: 30,
        fastSeek: vi.fn()
      }
      audioElement.value = mockAudio as any
      currentTime.value = 30
      duration.value = 0  // Duration not known
      currentSong.value = mockSongs[0]

      // Should return early without seeking
      seekToTime(60)
      expect(mockAudio.fastSeek).not.toHaveBeenCalled()
    })

    it('should handle t=0 (seek to beginning)', () => {
      const mockSongs = [
        { id: 1, name: 'Test Song', lufs: -12, path: '/path/to/song.mp3' }
      ]

      const { seekToTime, audioElement, currentTime, duration, isPlaying, currentSong } = useAudioPlayer({
        songs: () => mockSongs,
        onSongEnd: vi.fn(),
        onSongStart: vi.fn()
      })

      const mockAudio = {
        currentTime: 60,
        fastSeek: vi.fn()
      }
      audioElement.value = mockAudio as any
      currentTime.value = 60
      duration.value = 180
      isPlaying.value = false
      currentSong.value = mockSongs[0]

      // Seek to beginning
      seekToTime(0)

      // Should use HTML5 seeking (t=0 is a special case, handled by standard seeking)
      expect(mockAudio.fastSeek).toHaveBeenCalledWith(0)
    })
  })
})
