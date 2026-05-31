/**
 * Lyrics composable for Kaulan music player
 *
 * This composable handles parsing timed lyric files and synchronizing lyrics with audio playback.
 * Supports LRC and WebVTT formats, including bilingual lyric grouping.
 *
 * @module composables/useLyrics
 *
 * Related documentation:
 * - `docs/lyric-sync-timing.md`
 */

import {
  ref,
  computed,
  watch,
  onScopeDispose,
  getCurrentScope,
  type Ref,
} from "vue";
import { getApiBase } from "@/utils/api";

/**
 * Represents a single lyric line with timestamp and text(s)
 */
export interface LyricLine {
  /** Time in seconds when this lyric should be displayed */
  time: number;
  /** Array of lyric texts - single language has 1 element, bilingual has 2 */
  texts: string[];
}

/**
 * Song info interface (must match the one in SongListView)
 */
export interface SongInfo {
  id: number;
  name: string;
  lufs: number | null;
  path: string;
}

const LYRIC_RESYNC_THRESHOLD_SECONDS = 0.1;

/**
 * Merge parsed lyric lines by timestamp while preserving text order.
 */
function mergeLyricLines(lines: LyricLine[]): LyricLine[] {
  const merged = new Map<number, string[]>();

  for (const line of lines) {
    const existing = merged.get(line.time) ?? [];
    existing.push(...line.texts);
    merged.set(line.time, existing);
  }

  return Array.from(merged.entries())
    .sort(([a], [b]) => a - b)
    .map(([time, texts]) => ({ time, texts }));
}

/**
 * Parse LRC content into LyricLine array.
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
  const lines: LyricLine[] = [];
  const timeRegex = /^\[(\d{2}):(\d{2})\.(\d{2,3})\]/;
  const metadataRegex = /^\[(ti|ar|al|by|offset|kana):.*\]$/i;
  let currentTime: number | null = null;
  let currentTexts: string[] = [];

  const pushCurrent = () => {
    if (currentTime === null) {
      return;
    }
    lines.push({ time: currentTime, texts: [...currentTexts] });
  };

  content.split("\n").forEach((rawLine) => {
    const line = rawLine.replace(/\r$/, "");
    if (metadataRegex.test(line.trim())) {
      return;
    }
    const match = timeRegex.exec(line);

    if (match) {
      const minutes = parseInt(match[1], 10);
      const seconds = parseInt(match[2], 10);
      const milliseconds = parseInt(match[3].padEnd(3, "0"), 10);
      const timestamp = minutes * 60 + seconds + milliseconds / 1000;
      const text = line.substring(match[0].length).trim();

      if (currentTime !== null && timestamp !== currentTime) {
        pushCurrent();
        currentTexts = [];
      }

      if (currentTime === null || timestamp !== currentTime) {
        currentTime = timestamp;
        currentTexts = [];
      }

      if (text) {
        currentTexts.push(text);
      }
      return;
    }

    if (currentTime !== null && line.trim()) {
      currentTexts.push(line.trim());
    }
  });

  pushCurrent();

  return mergeLyricLines(lines);
}

function parseVttTimestamp(timestamp: string): number | null {
  const parts = timestamp.trim().split(":");
  if (parts.length < 2 || parts.length > 3) {
    return null;
  }

  let hours = 0;
  let minutes: number;
  let secondsPart: string;

  if (parts.length === 3) {
    hours = Number.parseInt(parts[0], 10);
    minutes = Number.parseInt(parts[1], 10);
    secondsPart = parts[2];
  } else {
    minutes = Number.parseInt(parts[0], 10);
    secondsPart = parts[1];
  }

  const [secondsText, millisecondsText = "0"] = secondsPart.split(".");
  const seconds = Number.parseInt(secondsText, 10);
  const milliseconds = Number.parseInt(
    millisecondsText.padEnd(3, "0").slice(0, 3),
    10,
  );

  if ([hours, minutes, seconds, milliseconds].some(Number.isNaN)) {
    return null;
  }

  return hours * 3600 + minutes * 60 + seconds + milliseconds / 1000;
}

/**
 * Parse WEBVTT content into LyricLine array.
 *
 * Uses cue start times for lyric sync and preserves cue text exactly.
 */
export function parseVtt(content: string): LyricLine[] {
  const normalizedContent = content.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const blocks = normalizedContent
    .split(/\n{2,}/)
    .map((block) => block.trim())
    .filter(Boolean);

  const lines: LyricLine[] = [];

  for (const block of blocks) {
    const blockLines = block.split("\n");
    if (blockLines.length === 0) {
      continue;
    }

    let timingLineIndex = 0;
    if (
      blockLines[0] === "WEBVTT" ||
      blockLines[0].startsWith("NOTE") ||
      blockLines[0].startsWith("STYLE") ||
      blockLines[0].startsWith("REGION")
    ) {
      continue;
    }

    if (!blockLines[0].includes("-->")) {
      timingLineIndex = 1;
    }

    const timingLine = blockLines[timingLineIndex];
    if (!timingLine || !timingLine.includes("-->")) {
      continue;
    }

    const [startTimeText] = timingLine.split("-->");
    const startTime = parseVttTimestamp(startTimeText);
    if (startTime === null) {
      continue;
    }

    const texts = blockLines
      .slice(timingLineIndex + 1)
      .map((line) => line.trim())
      .filter(Boolean);

    lines.push({
      time: startTime,
      texts,
    });
  }

  return mergeLyricLines(lines);
}

/**
 * Parse timed lyric content into LyricLine array.
 */
export function parseLyrics(content: string): LyricLine[] {
  const trimmed = content.trimStart();
  if (trimmed.startsWith("WEBVTT")) {
    return parseVtt(content);
  }

  return parseLrc(content);
}

/**
 * Find the active lyric line for a playback time.
 *
 * Returns the last lyric line whose timestamp is <= time.
 */
export function findLyricIndex(lines: LyricLine[], time: number): number {
  let low = 0;
  let high = lines.length - 1;
  let index = -1;

  while (low <= high) {
    const mid = Math.floor((low + high) / 2);

    if (lines[mid].time <= time) {
      index = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }

  return index;
}

/**
 * Load lyrics from the backend API
 *
 * @param id - Music ID
 * @returns Promise resolving to the raw lyric content, or null if not found
 */
async function loadLyrics(id: number): Promise<string | null> {
  try {
    const apiBase = getApiBase();
    const response = await fetch(`${apiBase}/lyrics/id/${id}`);

    if (response.status === 404) {
      // No lyrics available for this song
      return null;
    }

    if (!response.ok) {
      console.error(
        `Failed to load lyrics: ${response.status} ${response.statusText}`,
      );
      return null;
    }

    return await response.text();
  } catch (error) {
    console.error("Error loading lyrics:", error);
    return null;
  }
}

/**
 * Composable for lyrics management and synchronization
 *
 * @param currentSong - Ref containing the currently playing song
 * @returns Lyrics state and management functions
 */
export function useLyrics(
  currentSong: Ref<SongInfo | null>,
  currentTime: Ref<number>,
  isPlaying: Ref<boolean>,
) {
  /** Parsed lyrics lines */
  const lyrics = ref<LyricLine[]>([]);
  /** Index of the currently active lyric line (-1 if no active lyric) */
  const currentLyricIndex = ref(-1);
  /** Loading state for lyrics fetch */
  const isLoading = ref(false);
  /** Whether lyrics are available for the current song */
  const hasLyrics = computed(() => lyrics.value.length > 0);
  let lyricTimer: number | null = null;
  let scheduledPlaybackTime = 0;
  let scheduledAtMs = 0;

  function clearLyricTimer(): void {
    if (lyricTimer !== null) {
      clearTimeout(lyricTimer);
      lyricTimer = null;
    }
  }

  function getExpectedPlaybackTime(): number {
    if (!isPlaying.value) {
      return currentTime.value;
    }

    if (lyricTimer === null) {
      return currentTime.value;
    }

    const elapsedSeconds = (Date.now() - scheduledAtMs) / 1000;
    return scheduledPlaybackTime + elapsedSeconds;
  }

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
      currentLyricIndex.value = -1;
      return;
    }
    currentLyricIndex.value = findLyricIndex(lyrics.value, time);
  }

  function scheduleFromTime(time: number): void {
    clearLyricTimer();
    updateCurrentLyric(time);

    if (!isPlaying.value || lyrics.value.length === 0) {
      return;
    }

    const nextLine = lyrics.value[currentLyricIndex.value + 1];
    if (!nextLine) {
      return;
    }

    scheduledPlaybackTime = time;
    scheduledAtMs = Date.now();
    const delayMs = Math.max(0, Math.round((nextLine.time - time) * 1000));
    lyricTimer = window.setTimeout(() => {
      lyricTimer = null;
      scheduleFromTime(nextLine.time);
    }, delayMs);
  }

  function resyncLyrics(time: number): void {
    const correctedIndex = findLyricIndex(lyrics.value, time);
    const drift = Math.abs(time - getExpectedPlaybackTime());
    const indexChanged = correctedIndex !== currentLyricIndex.value;

    if (
      drift > LYRIC_RESYNC_THRESHOLD_SECONDS ||
      indexChanged ||
      lyricTimer === null
    ) {
      scheduleFromTime(time);
    }
  }

  /**
   * Load and parse lyrics for the current song
   */
  async function fetchLyrics(): Promise<void> {
    if (!currentSong.value) {
      clearLyricTimer();
      lyrics.value = [];
      currentLyricIndex.value = -1;
      return;
    }

    isLoading.value = true;
    const content = await loadLyrics(currentSong.value.id);

    if (content) {
      lyrics.value = parseLyrics(content);
      scheduleFromTime(currentTime.value);
    } else {
      clearLyricTimer();
      lyrics.value = [];
      currentLyricIndex.value = -1;
    }

    isLoading.value = false;
  }

  // Auto-load lyrics only when the song identity changes.
  watch(
    () => currentSong.value?.id ?? null,
    () => {
      void fetchLyrics();
    },
    { immediate: true },
  );

  watch(currentTime, (time) => {
    if (!hasLyrics.value) {
      return;
    }

    resyncLyrics(time);
  });

  watch(isPlaying, (playing) => {
    if (!hasLyrics.value) {
      return;
    }

    if (!playing) {
      clearLyricTimer();
      updateCurrentLyric(currentTime.value);
      return;
    }

    scheduleFromTime(currentTime.value);
  });

  if (getCurrentScope()) {
    onScopeDispose(() => {
      clearLyricTimer();
    });
  }

  return {
    lyrics,
    currentLyricIndex,
    hasLyrics,
    isLoading,
    updateCurrentLyric,
    clearLyricTimer,
    scheduleFromTime,
  };
}
