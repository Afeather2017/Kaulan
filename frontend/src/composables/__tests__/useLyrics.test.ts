/**
 * Tests for useLyrics composable
 *
 * @module composables/__tests__/useLyrics.test
 */

import { describe, it, expect } from 'vitest'
import { parseLrc } from '../useLyrics'

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
})
