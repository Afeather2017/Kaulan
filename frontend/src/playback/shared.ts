import type { LibrarySourceGroup } from "@/types/library";
import type { MusicInfo } from "@/types/music";
import {
  getStoredPlaybackSession,
  removeStoredPlaybackSession,
  setStoredPlaybackSession,
  toStoredPlaybackQueueSong,
} from "@/utils/storage";
import { storedQueueSongToMusicInfo } from "@/utils/songRestore";
import type { PlayMode } from "@/playback/types";

export function getSongIdentity(song: MusicInfo): string {
  if (song.source) {
    return `online:${song.source}:${song.id}:${song.name}`;
  }
  return `${song.device_id ?? "local"}:${song.id}`;
}

export function songsMatch(left: MusicInfo, right: MusicInfo): boolean {
  return getSongIdentity(left) === getSongIdentity(right);
}

export interface ResolvedQueueState {
  queue: MusicInfo[];
  currentIndex: number;
  currentSong: MusicInfo | null;
}

/**
 * Rebuild the current-song pointer for a restored/received queue by preferred
 * device-id + song id, then song id, then a raw index fallback.
 */
export function resolveQueueState(
  queue: MusicInfo[],
  preferredCurrentDeviceId: string | null,
  preferredCurrentSongId: number | null,
  preferredIndex: number | null,
): ResolvedQueueState {
  let resolvedIndex = -1;

  if (preferredCurrentDeviceId !== null && preferredCurrentSongId !== null) {
    resolvedIndex = queue.findIndex(
      (song) =>
        (song.device_id ?? "") === preferredCurrentDeviceId &&
        song.id === preferredCurrentSongId,
    );
  }

  if (resolvedIndex < 0 && preferredCurrentSongId !== null) {
    resolvedIndex = queue.findIndex(
      (song) => song.id === preferredCurrentSongId,
    );
  }

  if (
    resolvedIndex < 0 &&
    preferredIndex !== null &&
    preferredIndex >= 0 &&
    preferredIndex < queue.length
  ) {
    resolvedIndex = preferredIndex;
  }

  const currentSong =
    resolvedIndex >= 0 ? (queue[resolvedIndex] ?? null) : null;
  return { queue, currentIndex: resolvedIndex, currentSong };
}

export function shuffleOrder(queue: MusicInfo[]): MusicInfo[] {
  const shuffled = queue.slice();
  for (let i = shuffled.length - 1; i > 0; i -= 1) {
    const j = Math.floor(Math.random() * (i + 1));
    const temp = shuffled[i];
    shuffled[i] = shuffled[j];
    shuffled[j] = temp;
  }
  return shuffled;
}

/**
 * Build the effective queue for playing `selectedSong`: normalize the selected
 * entry into the base queue, and in shuffle mode rotate the queue so the
 * selected song leads a shuffled remainder.
 */
export function buildQueueForMode(
  baseQueue: MusicInfo[],
  selectedSong: MusicInfo,
  selectedIndex: number | undefined,
  playMode: PlayMode,
): { queue: MusicInfo[]; index: number } {
  if (baseQueue.length === 0) {
    return { queue: [selectedSong], index: 0 };
  }

  const normalizedQueue = baseQueue.map((song) =>
    songsMatch(song, selectedSong) ? selectedSong : song,
  );

  const resolvedIndex =
    selectedIndex ??
    normalizedQueue.findIndex((song) => songsMatch(song, selectedSong));
  const clampedIndex = resolvedIndex >= 0 ? resolvedIndex : 0;

  if (playMode !== "shuffle") {
    return { queue: normalizedQueue, index: clampedIndex };
  }

  const current = normalizedQueue[clampedIndex] ?? selectedSong;
  const remaining = normalizedQueue.filter(
    (_song, index) => index !== clampedIndex,
  );
  return { queue: [current, ...shuffleOrder(remaining)], index: 0 };
}

/**
 * Shuffle pick avoiding repeats; resets once every index has been played.
 * Mirrors the previous randomSongIndexNoRepeat behavior.
 */
export function randomSongIndexNoRepeat(
  queueLength: number,
  playedIndexes: Set<number>,
): number {
  if (queueLength === 0) return 0;

  const notPlayed = queueLength - playedIndexes.size;
  if (notPlayed === 0) {
    playedIndexes.clear();
    return Math.floor(Math.random() * queueLength);
  }

  let count = Math.ceil(Math.random() * notPlayed);
  for (let i = 0; i < queueLength; i += 1) {
    if (!playedIndexes.has(i)) {
      count -= 1;
      if (count === 0) return i;
    }
  }

  return 0;
}

export function persistPlaybackSession(
  queue: MusicInfo[],
  currentSongInfo: MusicInfo | null,
): void {
  const storedQueue = queue
    .map(toStoredPlaybackQueueSong)
    .filter((entry): entry is NonNullable<typeof entry> => entry !== null);

  if (storedQueue.length === 0) {
    removeStoredPlaybackSession();
    return;
  }

  const currentIsPersistable =
    !!currentSongInfo &&
    !currentSongInfo.source &&
    !currentSongInfo.is_temporary;

  setStoredPlaybackSession({
    currentDeviceId: currentIsPersistable
      ? (currentSongInfo!.device_id ?? null)
      : null,
    currentSongId: currentIsPersistable ? currentSongInfo!.id : null,
    queue: storedQueue,
    timestamp: Date.now(),
  });
}

export function loadStoredQueue(sourceGroups: LibrarySourceGroup[]): {
  queue: MusicInfo[];
  currentDeviceId: string | null;
  currentSongId: number | null;
} | null {
  const stored = getStoredPlaybackSession();
  if (!stored || stored.queue.length === 0) {
    return null;
  }
  return {
    queue: stored.queue.map((song) =>
      storedQueueSongToMusicInfo(song, sourceGroups),
    ),
    currentDeviceId: stored.currentDeviceId,
    currentSongId: stored.currentSongId,
  };
}
