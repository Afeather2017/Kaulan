/**
 * Tests for useLyrics composable
 *
 * @module composables/__tests__/useLyrics.test
 *
 * Related documentation:
 * - `docs/lyric-sync-timing.md`
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { nextTick, ref } from 'vue'
import { parseLrc, parseLyrics, parseVtt, useLyrics } from '../useLyrics'

vi.mock('@/utils/api', () => ({
  getApiBase: () => 'http://localhost:2080/api'
}))

async function flushLyricsLoad(): Promise<void> {
  await Promise.resolve()
  await nextTick()
}

describe('parseLrc', () => {
  it('should parse single-language LRC format', () => {
    const content = `[00:00.54]First lyric line
[00:02.52]Second lyric line
[00:05.00]Third lyric line`

    const result = parseLrc(content)

    expect(result).toHaveLength(3)
    expect(result[0]).toEqual({
      time: 0.54,
      texts: ['First lyric line']
    })
    expect(result[1]).toEqual({
      time: 2.52,
      texts: ['Second lyric line']
    })
    expect(result[2]).toEqual({
      time: 5.0,
      texts: ['Third lyric line']
    })
  })

  it('should parse bilingual LRC format (consecutive lines with same timestamp)', () => {
    const content = `[00:00.64]Japanese original text
[00:00.64]Chinese translation
[00:02.14]Another Japanese line
[00:02.14]Another Chinese translation`

    const result = parseLrc(content)

    expect(result).toHaveLength(2)
    expect(result[0]).toEqual({
      time: 0.64,
      texts: ['Japanese original text', 'Chinese translation']
    })
    expect(result[1]).toEqual({
      time: 2.14,
      texts: ['Another Japanese line', 'Another Chinese translation']
    })
  })

  it('should handle empty lyric lines gracefully', () => {
    const content = `[00:00.54]
[00:02.52]Has text
[00:05.00]`

    const result = parseLrc(content)

    expect(result).toHaveLength(3)
    // Empty strings should not be added to texts array
    expect(result[0].texts).toHaveLength(0)
    expect(result[1].texts).toEqual(['Has text'])
    expect(result[2].texts).toHaveLength(0)
  })

  it('should preserve empty lines with non-breaking space placeholder', () => {
    const content = `[00:00.54]First line
[00:02.52]   \t
[00:05.00]Third line`

    const result = parseLrc(content)

    // Lines with only whitespace are treated as empty after trim
    expect(result).toHaveLength(3)
  })

  it('should sort lyrics by timestamp if unsorted', () => {
    const content = `[00:05.00]Third
[00:00.54]First
[00:02.52]Second`

    const result = parseLrc(content)

    expect(result).toHaveLength(3)
    expect(result[0].time).toBe(0.54)
    expect(result[1].time).toBe(2.52)
    expect(result[2].time).toBe(5.0)
  })

  it('should handle mixed bilingual/monolingual entries', () => {
    const content = `[00:00.10]Only Japanese
[00:02.20]Japanese with translation
[00:02.20]Chinese translation
[00:05.00]Another single line`

    const result = parseLrc(content)

    expect(result).toHaveLength(3)
    expect(result[0].texts).toHaveLength(1)
    expect(result[0].texts[0]).toBe('Only Japanese')
    expect(result[1].texts).toHaveLength(2)
    expect(result[1].texts).toEqual(['Japanese with translation', 'Chinese translation'])
    expect(result[2].texts).toHaveLength(1)
    expect(result[2].texts[0]).toBe('Another single line')
  })

  it('should handle milliseconds with 2 or 3 digits', () => {
    const content = `[00:00.54]Two digit milliseconds
[00:01.123]Three digit milliseconds`

    const result = parseLrc(content)

    expect(result).toHaveLength(2)
    expect(result[0].time).toBe(0.54)
    expect(result[1].time).toBe(1.123)
  })

  it('should handle UTF-8 content (Chinese/Japanese)', () => {
    const content = `[00:00.54]欢迎光临。
[00:02.52]是美容店的店员茉子哦♪
[00:05.00]ときめきもどぎまぎも`

    const result = parseLrc(content)

    expect(result).toHaveLength(3)
    expect(result[0].texts[0]).toBe('欢迎光临。')
    expect(result[1].texts[0]).toBe('是美容店的店员茉子哦♪')
    expect(result[2].texts[0]).toBe('ときめきもどぎまぎも')
  })

  it('should return empty array for empty content', () => {
    const result = parseLrc('')
    expect(result).toEqual([])
  })

  it('should return empty array for content without valid LRC tags', () => {
    const content = `Just some text
Without timestamps
No [brackets] here`

    const result = parseLrc(content)
    expect(result).toEqual([])
  })

  it('should handle multiple consecutive lines without timestamp (all treated as continuation)', () => {
    const content = `[00:00.10]First line
Second continuation line
Third continuation line
[00:02.00]New timestamp`

    const result = parseLrc(content)

    expect(result).toHaveLength(2)
    expect(result[0].texts).toHaveLength(3)
    expect(result[0].texts).toEqual([
      'First line',
      'Second continuation line',
      'Third continuation line'
    ])
    expect(result[1].texts).toEqual(['New timestamp'])
  })

  it('should merge duplicated timestamps across separated bilingual LRC blocks', () => {
    const content = `[ti:Sample]
[00:01.00]I love the way you lie
[00:03.00]I can't tell you what it really is
[00:05.00]Right now
[00:01.00]我喜欢你的谎言
[00:03.00]我无法告诉你这到底是什么
[00:05.00]现在`

    const result = parseLrc(content)

    expect(result).toHaveLength(3)
    expect(result[0]).toEqual({
      time: 1,
      texts: ['I love the way you lie', '我喜欢你的谎言']
    })
    expect(result[1]).toEqual({
      time: 3,
      texts: ["I can't tell you what it really is", '我无法告诉你这到底是什么']
    })
    expect(result[2]).toEqual({
      time: 5,
      texts: ['Right now', '现在']
    })
  })

  it('should ignore LRC metadata tags', () => {
    const content = `[ti:Song Title]
[ar:Artist]
[offset:0]
[00:01.00]Actual lyric`

    const result = parseLrc(content)

    expect(result).toEqual([
      {
        time: 1,
        texts: ['Actual lyric']
      }
    ])
  })

  it('should preserve punctuation in LRC text', () => {
    const content = `[00:01.00]that's "my" line, right?`

    const result = parseLrc(content)

    expect(result[0].texts[0]).toBe(`that's "my" line, right?`)
  })
})

describe('parseVtt', () => {
  it('should parse standard WEBVTT cues', () => {
    const content = `WEBVTT

1
00:00:02.900 --> 00:00:06.700
哥哥 哥哥

2
00:00:08.875 --> 00:00:12.100
终于醒了呀`

    const result = parseVtt(content)

    expect(result).toEqual([
      {
        time: 2.9,
        texts: ['哥哥 哥哥']
      },
      {
        time: 8.875,
        texts: ['终于醒了呀']
      }
    ])
  })

  it('should preserve punctuation in VTT text', () => {
    const content = `WEBVTT

00:00:01.000 --> 00:00:03.000
that's "my" line, right?`

    const result = parseVtt(content)

    expect(result[0].texts[0]).toBe(`that's "my" line, right?`)
  })

  it('should support multi-line VTT cues', () => {
    const content = `WEBVTT

00:00:01.000 --> 00:00:03.000
Hello
你好`

    const result = parseVtt(content)

    expect(result).toEqual([
      {
        time: 1,
        texts: ['Hello', '你好']
      }
    ])
  })
})

describe('useLyrics scheduling', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-01-01T00:00:00.000Z'))
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
    vi.stubGlobal('fetch', vi.fn(() => Promise.resolve({
      ok: true,
      status: 200,
      text: () => Promise.resolve(`[00:00.05]Line one
[00:00.12]Line two
[00:00.40]Line three`)
    })))
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  it('should advance lyrics using sub-second timers', async () => {
    const currentSong = ref({ id: 1, name: 'Song', lufs: null, path: '/song.mp3' })
    const currentTime = ref(0)
    const isPlaying = ref(true)

    const { currentLyricIndex } = useLyrics(currentSong, currentTime, isPlaying)
    await flushLyricsLoad()

    expect(currentLyricIndex.value).toBe(-1)

    await vi.advanceTimersByTimeAsync(55)
    expect(currentLyricIndex.value).toBe(0)

    await vi.advanceTimersByTimeAsync(70)
    expect(currentLyricIndex.value).toBe(1)
  })

  it('should rebuild the lyric timer after a seek-like correction', async () => {
    const currentSong = ref({ id: 1, name: 'Song', lufs: null, path: '/song.mp3' })
    const currentTime = ref(0)
    const isPlaying = ref(true)

    const { currentLyricIndex } = useLyrics(currentSong, currentTime, isPlaying)
    await flushLyricsLoad()

    currentTime.value = 0.35
    await nextTick()

    expect(currentLyricIndex.value).toBe(1)

    await vi.advanceTimersByTimeAsync(60)
    expect(currentLyricIndex.value).toBe(2)
  })
})

describe('parseLyrics', () => {
  it('should auto-detect WEBVTT content', () => {
    const content = `WEBVTT

00:00:01.000 --> 00:00:03.000
Detected automatically`

    const result = parseLyrics(content)

    expect(result).toEqual([
      {
        time: 1,
        texts: ['Detected automatically']
      }
    ])
  })
})
