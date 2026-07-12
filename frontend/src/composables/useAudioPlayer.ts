import { ref, watch, getCurrentScope, onScopeDispose, toRaw } from "vue";
import { resolveSourceApiBase } from "@/utils/api";
import { getRuntimeCapabilities, isLocalhostApiBase } from "@/utils/platform";
import {
  getStoredPlaybackSession,
  removeStoredPlaybackSession,
  setStoredPlaybackSession,
  type StoredPlaybackQueueSong,
} from "@/utils/storage";
import type { MusicInfo } from "@/types/music";
import type {
  PlaybackSession,
  PlayMode as AndroidPlayMode,
  PlayingQueue,
  NormalizationMode,
} from "music-notification-api";

// Related documentation:
// - `docs/lyric-sync-timing.md`
// - `docs/web-playback-isplaying.md` (web isPlaying / play() AbortError handling)
// - `docs/android/playback-session.md` (Android isPlaying authority + optimistic play)

export type { MusicInfo } from "@/types/music";

export type PlayMode = "sequential" | "shuffle" | "loop";

export class PlaybackStartError extends Error {
  readonly code: "autoplay_blocked";

  constructor(message: string) {
    super(message);
    this.name = "PlaybackStartError";
    this.code = "autoplay_blocked";
  }
}

interface UseAudioPlayerOptions {
  songs: () => MusicInfo[];
  onSongEnd?: () => void;
  onSongStart?: (currentSong: MusicInfo, nextSong: MusicInfo | null) => void;
  onPlaybackQueueStart?: (
    queue: MusicInfo[],
    currentIndex: number,
    playMode: PlayMode,
  ) => Promise<void> | void;
  prepareSong?: (song: MusicInfo) => Promise<MusicInfo>;
}

type MusicNotificationApi = typeof import("music-notification-api");

interface ResolvedQueueState {
  queue: MusicInfo[];
  currentIndex: number;
  currentSong: MusicInfo | null;
  currentSongId: number | null;
  currentSongUrl: string | null;
}

interface PlaybackBackend {
  kind: "web" | "android";
  init: () => Promise<void>;
  cleanup: () => void;
  usesRawPlaybackPath: (song: MusicInfo) => boolean;
  playSong: (
    song: MusicInfo,
    seekTime?: number,
    queueOverride?: MusicInfo[],
    selectedIndex?: number,
  ) => Promise<void>;
  play: () => Promise<void>;
  pause: () => Promise<void>;
  previousSong: () => Promise<void>;
  nextSong: () => Promise<void>;
  seekToTime: (time: number) => Promise<void>;
  syncQueueState: () => Promise<void>;
  handlePlayModeChange: () => Promise<void>;
  handleQueuePruned: (
    queue: MusicInfo[],
    index: number,
    shouldResumePlayback: boolean,
    source: string,
  ) => Promise<void>;
  refreshSession: (source?: string) => Promise<void>;
  syncNormalizationConfig: (
    mode: NormalizationMode,
    manualVolume: number,
    fixedLufs: number,
    lufsPrecacheCount: number,
    currentVolume: number,
  ) => Promise<void>;
  setTimedPause: (delayMs: number) => Promise<void>;
  clearPlaybackState: () => Promise<void>;
}

export function useAudioPlayer(options: UseAudioPlayerOptions) {
  const { songs, onSongEnd, onSongStart, onPlaybackQueueStart, prepareSong } =
    options;

  const audioElement = ref<HTMLAudioElement | null>(null);
  const currentSong = ref<MusicInfo | null>(null);
  const isPlaying = ref(false);
  const currentTime = ref(0);
  const duration = ref(0);
  const playMode = ref<PlayMode>("sequential");
  const playedSongIndexes = ref<Set<number>>(new Set());
  const currentIndex = ref(-1);
  const activeQueue = ref<MusicInfo[]>([]);
  const isAndroidPlayer = ref(false);

  const POLL_INTERVAL_MS = 1000;
  const SOURCE_POLL_INTERVAL_MS = 5000;
  const SEEK_CONFIRMATION_THRESHOLD_SECONDS = 0.1;

  let isPlayingInternal = false;
  let pollingTimer: ReturnType<typeof setInterval> | null = null;
  let sourcePollingTimer: ReturnType<typeof setInterval> | null = null;
  let pluginApiPromise: Promise<MusicNotificationApi> | null = null;
  let lastStartedSongIdentity: string | null = null;
  let pendingSeekTargetMs: number | null = null;
  let pendingPlaySeekTime: number | undefined;
  let sourceCheckInFlight = false;

  const loadPluginApi = async (): Promise<MusicNotificationApi> => {
    if (pluginApiPromise === null) {
      pluginApiPromise = import("music-notification-api");
    }
    return pluginApiPromise;
  };

  const buildAudioUrl = (
    songId: number,
    sourceKey?: string | null,
    seekTime?: number,
  ): string => {
    const apiBase = resolveSourceApiBase(sourceKey);
    let url: URL;
    try {
      url = new URL(`${apiBase}/music/id/${songId}`);
    } catch {
      url = new URL(`${apiBase}/music/id/${songId}`, window.location.origin);
    }
    if (seekTime !== undefined && duration.value > 0) {
      const position = seekTime / duration.value;
      url.searchParams.set("position", position.toString());
    }
    return url.toString();
  };

  const buildCoverUrl = (
    songId: number,
    sourceKey?: string | null,
  ): string | null => {
    if (songId <= 0) {
      return null;
    }
    const apiBase = resolveSourceApiBase(sourceKey);
    try {
      return new URL(`${apiBase}/music/id/${songId}/cover`).toString();
    } catch {
      return new URL(
        `${apiBase}/music/id/${songId}/cover`,
        window.location.origin,
      ).toString();
    }
  };

  const shouldUseRawPlaybackPath = (song: MusicInfo): boolean => {
    return getPlaybackBackend().usesRawPlaybackPath(song);
  };

  const buildSongPlaybackUrl = (song: MusicInfo, seekTime?: number): string => {
    if (shouldUseRawPlaybackPath(song) && seekTime === undefined) {
      return song.path;
    }

    return song.stream_url ?? buildAudioUrl(song.id, song.source_key, seekTime);
  };

  const deriveSourceKeyFromUrl = (url: string): string | null => {
    try {
      const parsed =
        /^[a-zA-Z][a-zA-Z\d+\-.]*:\/\//.test(url) || url.startsWith("//")
          ? new URL(url)
          : new URL(url, window.location.origin);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
        return null;
      }
      const apiIndex = parsed.pathname.indexOf("/api/");
      if (apiIndex < 0) {
        return null;
      }
      return `${parsed.origin}${parsed.pathname.slice(0, apiIndex + 4)}`;
    } catch {
      return null;
    }
  };

  const getSongSourceKey = (song: MusicInfo): string | null => {
    if (typeof song.source_key === "string" && song.source_key) {
      return song.source_key;
    }
    return deriveSourceKeyFromUrl(buildSongPlaybackUrl(song));
  };

  const getSongIdentity = (song: MusicInfo): string => {
    const playbackUrl = buildSongPlaybackUrl(song);
    return playbackUrl || `${song.id}:${song.path}`;
  };

  const songsMatch = (left: MusicInfo, right: MusicInfo): boolean => {
    return getSongIdentity(left) === getSongIdentity(right);
  };

  const toQueueSong = (song: MusicInfo) => ({
    id: song.id,
    name: song.name,
    path: song.path,
    url: buildSongPlaybackUrl(song),
    lufs: song.lufs,
    coverUrl: song.cover_url ?? buildCoverUrl(song.id, getSongSourceKey(song)),
  });

  const toMusicInfo = (song: {
    id: number;
    name: string;
    path: string;
    lufs: number | null;
    url?: string | null;
    coverUrl?: string | null;
    sourceKey?: string | null;
  }): MusicInfo => ({
    id: song.id,
    name: song.name,
    path: song.path,
    lufs: song.lufs,
    stream_url: song.url ?? null,
    cover_url:
      song.coverUrl ??
      buildCoverUrl(
        song.id,
        song.sourceKey ?? deriveSourceKeyFromUrl(song.url ?? ""),
      ),
    source_key: song.sourceKey ?? deriveSourceKeyFromUrl(song.url ?? ""),
  });

  const toStoredPlaybackQueueSong = (
    song: MusicInfo,
  ): StoredPlaybackQueueSong => ({
    id: song.id,
    name: song.name,
    path: song.path,
    url: buildSongPlaybackUrl(song),
    lufs: song.lufs,
    coverUrl: song.cover_url ?? buildCoverUrl(song.id, getSongSourceKey(song)),
    sourceKey: getSongSourceKey(song),
  });

  const persistPlaybackSession = (
    queue: MusicInfo[],
    currentSongInfo: MusicInfo | null,
  ) => {
    if (queue.length === 0) {
      removeStoredPlaybackSession();
      return;
    }

    setStoredPlaybackSession({
      currentSongId: currentSongInfo?.id ?? null,
      currentSongUrl: currentSongInfo ? getSongIdentity(currentSongInfo) : null,
      queue: queue.map(toStoredPlaybackQueueSong),
      timestamp: Date.now(),
    });
  };

  const resolveQueueState = (
    queue: MusicInfo[],
    preferredCurrentSongUrl: string | null,
    preferredCurrentSongId: number | null,
    preferredIndex: number | null,
  ): ResolvedQueueState => {
    let resolvedIndex = -1;

    if (preferredCurrentSongUrl) {
      resolvedIndex = queue.findIndex(
        (song) => getSongIdentity(song) === preferredCurrentSongUrl,
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

    return {
      queue,
      currentIndex: resolvedIndex,
      currentSong: resolvedIndex >= 0 ? (queue[resolvedIndex] ?? null) : null,
      currentSongId: resolvedIndex >= 0 ? queue[resolvedIndex].id : null,
      currentSongUrl:
        resolvedIndex >= 0 ? getSongIdentity(queue[resolvedIndex]) : null,
    };
  };

  const recoverAndroidQueueState = (
    session: PlaybackSession,
  ): ResolvedQueueState | null => {
    const stored = getStoredPlaybackSession();
    if (!stored || stored.queue.length === 0) {
      return null;
    }

    const fallbackIndex = session.queue.currentIndex ?? 0;
    const fallbackSong =
      session.queue.songs[fallbackIndex] ?? session.queue.songs[0];
    const fallbackUrl = fallbackSong?.url;
    if (!fallbackUrl) {
      return null;
    }

    const matchedIndexes = stored.queue
      .map((song, index) => (song.url === fallbackUrl ? index : -1))
      .filter((index) => index >= 0);

    if (matchedIndexes.length !== 1) {
      return null;
    }

    const queue = stored.queue.map(toMusicInfo);
    const matchedIndex = matchedIndexes[0];
    return resolveQueueState(
      queue,
      getSongIdentity(queue[matchedIndex]),
      queue[matchedIndex].id,
      matchedIndex,
    );
  };

  const restoreWebPlaybackSession = () => {
    const stored = getStoredPlaybackSession();
    if (!stored || stored.queue.length === 0) {
      return;
    }

    const queue = stored.queue.map(toMusicInfo);
    const restored = resolveQueueState(
      queue,
      stored.currentSongUrl ?? null,
      stored.currentSongId,
      null,
    );
    activeQueue.value = restored.queue;
    currentIndex.value = restored.currentIndex;
    currentSong.value = restored.currentSong;
  };

  const getBaseQueue = (queueOverride?: MusicInfo[]): MusicInfo[] => {
    if (queueOverride && queueOverride.length > 0) {
      return queueOverride.slice();
    }
    const sourceSongs = songs();
    if (sourceSongs.length > 0) {
      return sourceSongs.slice();
    }
    return activeQueue.value.slice();
  };

  const shuffleSongs = (queue: MusicInfo[]): MusicInfo[] => {
    const shuffled = queue.slice();
    for (let i = shuffled.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      const temp = shuffled[i];
      shuffled[i] = shuffled[j];
      shuffled[j] = temp;
    }
    return shuffled;
  };

  const buildQueueForMode = (
    selectedSong: MusicInfo,
    selectedIndex?: number,
    queueOverride?: MusicInfo[],
  ): { queue: MusicInfo[]; index: number } => {
    const baseQueue = getBaseQueue(queueOverride);
    if (baseQueue.length === 0) {
      return { queue: [selectedSong], index: 0 };
    }

    const normalizedQueue = baseQueue.map((song) => {
      if (!songsMatch(song, selectedSong)) {
        return song;
      }
      return selectedSong;
    });

    const resolvedIndex =
      selectedIndex ??
      normalizedQueue.findIndex((song) => songsMatch(song, selectedSong));
    const clampedIndex = resolvedIndex >= 0 ? resolvedIndex : 0;

    if (playMode.value !== "shuffle") {
      return { queue: normalizedQueue, index: clampedIndex };
    }

    const current = normalizedQueue[clampedIndex] ?? selectedSong;
    const remaining = normalizedQueue.filter(
      (_song, index) => index !== clampedIndex,
    );
    return {
      queue: [current, ...shuffleSongs(remaining)],
      index: 0,
    };
  };

  const getQueueForPlayModeToggle = (): MusicInfo[] => {
    const currentSongIdentity = currentSong.value
      ? getSongIdentity(currentSong.value)
      : null;
    const sourceQueue = songs();

    if (
      currentSongIdentity !== null &&
      sourceQueue.some((song) => getSongIdentity(song) === currentSongIdentity)
    ) {
      return sourceQueue.slice();
    }

    if (activeQueue.value.length > 0) {
      return activeQueue.value.slice();
    }

    return sourceQueue.slice();
  };

  const prepareSongForPlayback = async (
    song: MusicInfo,
  ): Promise<MusicInfo> => {
    if (!prepareSong) {
      return song;
    }
    return await prepareSong(song);
  };

  const randomSongIndexNoRepeat = (): number => {
    const allSongs = activeQueue.value.length > 0 ? activeQueue.value : songs();
    if (allSongs.length === 0) return 0;

    const notPlayed = allSongs.length - playedSongIndexes.value.size;
    if (notPlayed === 0) {
      playedSongIndexes.value = new Set();
      return Math.floor(Math.random() * allSongs.length);
    }

    let count = Math.ceil(Math.random() * notPlayed);
    for (let i = 0; i < allSongs.length; i++) {
      if (!playedSongIndexes.value.has(i)) {
        count--;
        if (count === 0) return i;
      }
    }

    return 0;
  };

  const maybeEmitSongStart = (
    queue: MusicInfo[],
    song: MusicInfo | null,
    index: number,
  ) => {
    if (!song) {
      lastStartedSongIdentity = null;
      return;
    }
    const songIdentity = getSongIdentity(song);
    if (lastStartedSongIdentity === songIdentity) {
      return;
    }
    lastStartedSongIdentity = songIdentity;
    if (!onSongStart) {
      return;
    }

    const nextIndex =
      playMode.value === "loop"
        ? index
        : index >= 0 && index < queue.length - 1
          ? index + 1
          : queue.length > 0
            ? 0
            : -1;
    const nextSong =
      nextIndex >= 0 && nextIndex < queue.length ? queue[nextIndex] : null;
    onSongStart(song, nextSong);
  };

  const notifyPlaybackQueueStart = async (
    queue: MusicInfo[],
    index: number,
  ) => {
    if (!onPlaybackQueueStart || queue.length === 0) {
      return;
    }

    await onPlaybackQueueStart(queue.slice(), index, playMode.value);
  };

  const applyAndroidSession = (
    session: PlaybackSession,
    source = "unknown",
  ) => {
    const sessionQueue = session.queue.songs.map((song) => toMusicInfo(song));
    const shouldRecoverFromStorage =
      sessionQueue.length === 1 && sessionQueue[0]?.id <= 0;
    const resolved = shouldRecoverFromStorage
      ? (recoverAndroidQueueState(session) ??
        resolveQueueState(
          sessionQueue,
          null,
          session.currentSongId,
          session.queue.currentIndex,
        ))
      : resolveQueueState(
          sessionQueue,
          null,
          session.currentSongId,
          session.queue.currentIndex,
        );

    activeQueue.value = resolved.queue;
    currentIndex.value = resolved.currentIndex;
    currentSong.value = resolved.currentSong;
    isPlaying.value = session.runtime.isPlaying;
    duration.value = session.runtime.durationMs / 1000;
    playMode.value = session.playMode as PlayMode;
    const correctedTimeSeconds = session.runtime.positionMs / 1000;

    if (pendingSeekTargetMs !== null) {
      const driftMs = Math.abs(
        session.runtime.positionMs - pendingSeekTargetMs,
      );
      if (driftMs <= SEEK_CONFIRMATION_THRESHOLD_SECONDS * 1000) {
        pendingSeekTargetMs = null;
        currentTime.value = correctedTimeSeconds;
      } else {
        currentTime.value = pendingSeekTargetMs / 1000;
      }
    } else {
      currentTime.value = correctedTimeSeconds;
    }

    persistPlaybackSession(activeQueue.value, resolved.currentSong);
    console.log("[useAudioPlayer] applyAndroidSession", {
      source,
      currentIndex: currentIndex.value,
      currentSongId: currentSong.value?.id ?? null,
      currentSongName: currentSong.value?.name ?? null,
      queueSize: activeQueue.value.length,
      isPlaying: isPlaying.value,
      positionMs: session.runtime.positionMs,
      durationMs: session.runtime.durationMs,
      playMode: playMode.value,
    });
    maybeEmitSongStart(
      activeQueue.value,
      currentSong.value,
      currentIndex.value,
    );
  };

  const fetchAndroidSession = async (
    source = "manual",
  ): Promise<PlaybackSession> => {
    const plugin = await loadPluginApi();
    const session = await plugin.getPlaybackSession();
    applyAndroidSession(session, source);
    return session;
  };

  const startAndroidPolling = () => {
    if (pollingTimer) return;
    pollingTimer = setInterval(() => {
      void fetchAndroidSession("poll").catch((error) => {
        console.warn(
          "[useAudioPlayer] Failed to poll Android playback session:",
          error,
        );
      });
    }, POLL_INTERVAL_MS);
  };

  const stopAndroidPolling = () => {
    if (!pollingTimer) return;
    clearInterval(pollingTimer);
    pollingTimer = null;
  };

  const stopSourcePolling = () => {
    if (!sourcePollingTimer) return;
    clearInterval(sourcePollingTimer);
    sourcePollingTimer = null;
  };

  const resetPlaybackState = () => {
    currentSong.value = null;
    currentIndex.value = -1;
    activeQueue.value = [];
    isPlaying.value = false;
    currentTime.value = 0;
    duration.value = 0;
    persistPlaybackSession([], null);
  };

  let webBackend: PlaybackBackend;
  let androidBackend: PlaybackBackend;

  const getPlaybackBackend = (): PlaybackBackend => {
    return isAndroidPlayer.value ? androidBackend : webBackend;
  };

  const clearPlaybackState = async () => {
    resetPlaybackState();
    await getPlaybackBackend().clearPlaybackState();
  };

  const refreshAndroidSession = async (source = "manual") => {
    await androidBackend.refreshSession(source);
  };

  const syncAndroidQueueState = async () => {
    await androidBackend.syncQueueState();
  };

  const restartAndroidPlayback = async (
    queue: MusicInfo[],
    index: number,
    source: string,
    seekTime?: number,
  ) => {
    if (queue.length === 0) return;

    const plugin = await loadPluginApi();
    const resolvedIndex = Math.min(Math.max(index, 0), queue.length - 1);
    const preparedTargetSong = await prepareSongForPlayback(
      queue[resolvedIndex],
    );
    const preparedQueue = queue.map((song) => {
      if (!songsMatch(song, preparedTargetSong)) {
        return song;
      }
      return preparedTargetSong;
    });
    const targetSong = preparedQueue[resolvedIndex];
    const payload: PlayingQueue = {
      songs: preparedQueue.map((song) => toQueueSong(song)),
      currentIndex: resolvedIndex,
    };

    console.log("[useAudioPlayer] restartAndroidPlayback", {
      source,
      resolvedIndex,
      targetSongId: targetSong.id,
      targetSongName: targetSong.name,
      queueSize: preparedQueue.length,
      playMode: playMode.value,
    });

    await plugin.stop();
    await plugin.setPlayingQueue(payload, playMode.value as AndroidPlayMode);

    activeQueue.value = preparedQueue;
    currentIndex.value = resolvedIndex;
    currentSong.value = targetSong;
    persistPlaybackSession(activeQueue.value, currentSong.value);
    await notifyPlaybackQueueStart(activeQueue.value, currentIndex.value);

    if (seekTime !== undefined) {
      pendingSeekTargetMs = Math.max(0, Math.floor(seekTime * 1000));
      currentTime.value = pendingSeekTargetMs / 1000;
      await plugin.seekAndPlay(Math.max(0, Math.floor(seekTime * 1000)));
    } else {
      pendingSeekTargetMs = null;
      await plugin.play({
        url: buildSongPlaybackUrl(targetSong),
        title: targetSong.name,
        coverUrl:
          targetSong.cover_url ??
          buildCoverUrl(targetSong.id, getSongSourceKey(targetSong)) ??
          undefined,
      });
    }

    await fetchAndroidSession(source);

    // The plugin was just asked to play. The polled session can briefly lag the
    // native player (it may report isPlaying=false immediately after play()),
    // so mirror the explicit play command right away; the next poll reconciles
    // against the real session. Without this the UI can read "paused" for a
    // moment after a tap (or indefinitely in the lagging-session unit test).
    isPlaying.value = true;
    console.log("[useAudioPlayer] restartAndroidPlayback set isPlaying=true", {
      source,
      targetSongId: targetSong.id,
    });
  };

  const pruneQueueBySourceKeys = async (
    unreachableSourceKeys: Set<string>,
  ): Promise<boolean> => {
    if (unreachableSourceKeys.size === 0 || activeQueue.value.length === 0) {
      return false;
    }

    const previousQueue = activeQueue.value.slice();
    const previousCurrentSong = currentSong.value;
    const previousCurrentIdentity = previousCurrentSong
      ? getSongIdentity(previousCurrentSong)
      : null;
    const previousIndex = currentIndex.value;
    const shouldResumePlayback = isPlaying.value;
    const filteredQueue = previousQueue.filter((song) => {
      const sourceKey = getSongSourceKey(song);
      return !sourceKey || !unreachableSourceKeys.has(sourceKey);
    });

    if (filteredQueue.length === previousQueue.length) {
      return false;
    }

    const wasCurrentRemoved =
      previousCurrentSong !== null &&
      (() => {
        const currentSourceKey = getSongSourceKey(previousCurrentSong);
        return (
          typeof currentSourceKey === "string" &&
          unreachableSourceKeys.has(currentSourceKey)
        );
      })();

    if (filteredQueue.length === 0) {
      await clearPlaybackState();
      return true;
    }

    activeQueue.value = filteredQueue;

    if (!wasCurrentRemoved && previousCurrentIdentity) {
      currentIndex.value = filteredQueue.findIndex(
        (song) => getSongIdentity(song) === previousCurrentIdentity,
      );
      currentSong.value =
        currentIndex.value >= 0
          ? (filteredQueue[currentIndex.value] ?? null)
          : null;
      persistPlaybackSession(activeQueue.value, currentSong.value);
      await getPlaybackBackend().syncQueueState();
      return true;
    }

    const fallbackIndex =
      previousIndex >= 0
        ? Math.min(previousIndex, filteredQueue.length - 1)
        : 0;
    const nextSongCandidate = filteredQueue[fallbackIndex] ?? filteredQueue[0];
    currentIndex.value = filteredQueue.findIndex((song) =>
      songsMatch(song, nextSongCandidate),
    );
    currentSong.value = nextSongCandidate;
    persistPlaybackSession(activeQueue.value, currentSong.value);

    await getPlaybackBackend().handleQueuePruned(
      filteredQueue,
      currentIndex.value >= 0 ? currentIndex.value : 0,
      shouldResumePlayback,
      "pruneQueueBySourceKeys",
    );
    return true;
  };

  const checkSourceReachability = async (
    sourceKey: string,
  ): Promise<boolean> => {
    try {
      const response = await fetch(`${sourceKey}/discovery/self`, {
        cache: "no-store",
      });
      return response.ok;
    } catch {
      return false;
    }
  };

  const reconcileQueueSources = async (): Promise<boolean> => {
    if (sourceCheckInFlight || activeQueue.value.length === 0) {
      return false;
    }

    const sourceKeys = Array.from(
      new Set(
        activeQueue.value
          .map((song) => getSongSourceKey(song))
          .filter((sourceKey): sourceKey is string => Boolean(sourceKey)),
      ),
    );

    if (sourceKeys.length === 0) {
      return false;
    }

    sourceCheckInFlight = true;
    try {
      const reachability = await Promise.all(
        sourceKeys.map(async (sourceKey) => ({
          sourceKey,
          reachable: await checkSourceReachability(sourceKey),
        })),
      );

      const unreachable = new Set(
        reachability
          .filter((item) => !item.reachable)
          .map((item) => item.sourceKey),
      );

      return await pruneQueueBySourceKeys(unreachable);
    } finally {
      sourceCheckInFlight = false;
    }
  };

  const startSourcePolling = () => {
    if (sourcePollingTimer) return;
    sourcePollingTimer = setInterval(() => {
      void reconcileQueueSources().catch((error) => {
        console.warn(
          "[useAudioPlayer] Failed to reconcile source reachability:",
          error,
        );
      });
    }, SOURCE_POLL_INTERVAL_MS);
  };

  const syncAndroidQueue = async (
    selectedSong: MusicInfo,
    selectedIndex?: number,
    queueOverride?: MusicInfo[],
  ) => {
    const plugin = await loadPluginApi();
    const { queue, index } = buildQueueForMode(
      selectedSong,
      selectedIndex,
      queueOverride,
    );
    console.log("[useAudioPlayer] syncAndroidQueue", {
      selectedSongId: selectedSong.id,
      selectedSongName: selectedSong.name,
      selectedIndex: selectedIndex ?? null,
      resolvedIndex: index,
      queueSize: queue.length,
      playMode: playMode.value,
    });
    const payload: PlayingQueue = {
      songs: queue.map((song) => toQueueSong(song)),
      currentIndex: index,
    };
    await plugin.setPlayingQueue(payload, playMode.value as AndroidPlayMode);
    activeQueue.value = queue;
    currentIndex.value = index;
    currentSong.value = queue[index] ?? selectedSong;
    persistPlaybackSession(activeQueue.value, currentSong.value);
  };

  const syncAndroidPlayMode = async () => {
    const plugin = await loadPluginApi();
    if (playMode.value === "shuffle" && currentSong.value) {
      await syncAndroidQueue(currentSong.value, currentIndex.value);
      return;
    }
    await plugin.setPlayMode(playMode.value as AndroidPlayMode);
    await fetchAndroidSession();
  };

  const playSongAtIndex = async (
    song: MusicInfo,
    index: number,
    queueOverride?: MusicInfo[],
  ) => {
    playedSongIndexes.value.add(index);
    await playSong(song, undefined, queueOverride, index);
  };

  const handlePlaybackFailure = async (): Promise<void> => {
    const failedSong = currentSong.value;
    console.log("[web-playback] handlePlaybackFailure", {
      failedSongId: failedSong ? failedSong.id : null,
      queueSize: activeQueue.value.length,
      isPlaying: isPlaying.value,
    });
    if (!failedSong) {
      return;
    }

    const sourceKey = getSongSourceKey(failedSong);
    if (sourceKey) {
      const reachable = await checkSourceReachability(sourceKey);
      if (!reachable) {
        await pruneQueueBySourceKeys(new Set([sourceKey]));
        return;
      }
    }

    if (activeQueue.value.length <= 1) {
      isPlaying.value = false;
      return;
    }

    const failedIdentity = getSongIdentity(failedSong);
    const filteredQueue = activeQueue.value.filter(
      (song) => getSongIdentity(song) !== failedIdentity,
    );
    activeQueue.value = filteredQueue;

    if (filteredQueue.length === 0) {
      await clearPlaybackState();
      return;
    }

    const nextIndex = Math.min(currentIndex.value, filteredQueue.length - 1);
    const nextSongCandidate = filteredQueue[nextIndex] ?? filteredQueue[0];
    currentIndex.value = filteredQueue.findIndex((song) =>
      songsMatch(song, nextSongCandidate),
    );
    currentSong.value = nextSongCandidate;
    persistPlaybackSession(activeQueue.value, currentSong.value);
    await playSongAtIndex(
      nextSongCandidate,
      currentIndex.value >= 0 ? currentIndex.value : 0,
      filteredQueue,
    );
  };

  const playSong = async (
    song: MusicInfo,
    seekTime?: number,
    queueOverride?: MusicInfo[],
    selectedIndex?: number,
  ) => {
    await getPlaybackBackend().playSong(
      song,
      seekTime,
      queueOverride,
      selectedIndex,
    );
  };

  const play = async () => {
    await getPlaybackBackend().play();
  };

  const pause = async () => {
    await getPlaybackBackend().pause();
  };

  const togglePlayMode = async () => {
    if (playMode.value === "sequential") {
      playMode.value = "shuffle";
    } else if (playMode.value === "shuffle") {
      playMode.value = "loop";
    } else {
      playMode.value = "sequential";
    }

    if (currentSong.value) {
      const queueForMode = getQueueForPlayModeToggle();
      const selectedIndex = queueForMode.findIndex((song) =>
        currentSong.value ? songsMatch(song, currentSong.value) : false,
      );
      const { queue, index } = buildQueueForMode(
        currentSong.value,
        selectedIndex >= 0 ? selectedIndex : undefined,
        queueForMode,
      );

      activeQueue.value = queue;
      currentIndex.value = index;
      currentSong.value = queue[index] ?? currentSong.value;
      playedSongIndexes.value =
        currentIndex.value >= 0 ? new Set([currentIndex.value]) : new Set();
      persistPlaybackSession(activeQueue.value, currentSong.value);
    }

    await getPlaybackBackend().handlePlayModeChange();
  };

  const previousSong = async () => {
    await getPlaybackBackend().previousSong();
  };

  const playPreviousSongInWebQueue = async () => {
    const allSongs = activeQueue.value.length > 0 ? activeQueue.value : songs();
    if (!currentSong.value || allSongs.length === 0) return;

    let newIndex: number;

    if (playMode.value === "loop") {
      newIndex = currentIndex.value;
    } else if (playMode.value === "shuffle") {
      newIndex = randomSongIndexNoRepeat();
    } else {
      newIndex =
        currentIndex.value === 0 ? allSongs.length - 1 : currentIndex.value - 1;
    }

    await playSongAtIndex(allSongs[newIndex], newIndex);
  };

  const nextSong = async () => {
    await getPlaybackBackend().nextSong();
  };

  const playNextSongInWebQueue = async () => {
    const allSongs = activeQueue.value.length > 0 ? activeQueue.value : songs();
    if (!currentSong.value || allSongs.length === 0) return;

    let newIndex: number;

    if (playMode.value === "loop") {
      newIndex = currentIndex.value;
    } else if (playMode.value === "shuffle") {
      newIndex = randomSongIndexNoRepeat();
    } else {
      newIndex =
        currentIndex.value === allSongs.length - 1 ? 0 : currentIndex.value + 1;
    }

    await playSongAtIndex(allSongs[newIndex], newIndex);
  };

  const seekToTime = async (time: number) => {
    await getPlaybackBackend().seekToTime(time);
  };

  const applyWebVolume = (volume: number) => {
    const normalizedVolume = Math.min(1, Math.max(0, volume));
    if (audioElement.value) {
      audioElement.value.volume = normalizedVolume;
    }
  };

  const syncNormalizationConfig = async (
    mode: NormalizationMode,
    manualVolume: number,
    fixedLufs: number,
    lufsPrecacheCount: number,
    currentVolume: number,
  ) => {
    await getPlaybackBackend().syncNormalizationConfig(
      mode,
      manualVolume,
      fixedLufs,
      lufsPrecacheCount,
      currentVolume,
    );
  };

  const setTimedPause = async (delayMs: number) => {
    await getPlaybackBackend().setTimedPause(delayMs);
  };

  const resetPlaylist = () => {
    playedSongIndexes.value = new Set();
    currentIndex.value = -1;
  };

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  };

  // Resolves once the element has buffered enough to play, with a timeout
  // fallback. Used to retry play() after an AbortError: on auto-advance there
  // is no user gesture to resume an interrupted element, so we wait for the
  // source to be ready and start playback explicitly. See
  // docs/web-playback-isplaying.md.
  const waitForAudioReady = (
    audio: HTMLAudioElement,
    timeoutMs: number,
  ): Promise<void> =>
    new Promise((resolve) => {
      if (audio.readyState >= 2) {
        resolve();
        return;
      }
      let settled = false;
      const finish = () => {
        if (settled) return;
        settled = true;
        audio.removeEventListener("canplay", finish);
        resolve();
      };
      audio.addEventListener("canplay", finish);
      setTimeout(finish, timeoutMs);
    });

  // A single HTMLAudioElement is reused across every song. Safari/WebKit only
  // allow play() on an element that a user gesture has "unlocked"; a brand-new
  // element created on auto-advance (no gesture) is blocked with
  // NotAllowedError, so the next song would sit paused. Reusing the element the
  // user unlocked by tapping play lets us swap `src` and keep going. Listeners
  // are attached exactly once and read reactive state, so they stay correct no
  // matter which song is loaded. See docs/web-playback-isplaying.md.
  const ensureAudioElement = (): HTMLAudioElement => {
    if (audioElement.value) {
      return audioElement.value;
    }
    const audio = new Audio();
    audio.addEventListener("timeupdate", () => {
      currentTime.value = audio.currentTime || 0;
    });
    audio.addEventListener("loadedmetadata", () => {
      duration.value = audio.duration || 0;
      if (pendingPlaySeekTime === undefined) {
        return;
      }
      const clampedTime = Math.max(
        0,
        Math.min(pendingPlaySeekTime, duration.value || pendingPlaySeekTime),
      );
      audio.currentTime = clampedTime;
      currentTime.value = clampedTime;
      pendingPlaySeekTime = undefined;
    });
    audio.addEventListener("seeked", () => {
      currentTime.value = audio.currentTime || 0;
    });
    audio.addEventListener("seeking", () => {
      currentTime.value = audio.currentTime || 0;
    });
    audio.addEventListener("ended", () => {
      if (playMode.value === "loop") {
        if (currentSong.value) {
          void playSong(currentSong.value);
        }
      } else {
        void nextSong();
      }
      onSongEnd?.();
    });
    audio.addEventListener("error", () => {
      const failedSong = currentSong.value;
      const err = audio.error;
      const nameByCode: Record<number, string> = {
        1: "MEDIA_ERR_ABORTED",
        2: "MEDIA_ERR_NETWORK",
        3: "MEDIA_ERR_DECODE",
        4: "MEDIA_ERR_SRC_NOT_SUPPORTED",
      };
      console.warn("[web-playback] ERROR event -> handlePlaybackFailure", {
        songId: failedSong ? failedSong.id : null,
        url: audio.src,
        error: err
          ? {
              code: err.code,
              message: err.message,
              name: nameByCode[err.code] || "UNKNOWN",
            }
          : null,
        readyState: audio.readyState,
        networkState: audio.networkState,
      });
      void handlePlaybackFailure();
    });
    // isPlaying mirrors the element's real state. Scoped to the current
    // element (always this one, but the guard is cheap insurance) so a stale
    // element can never flip the UI. toRaw() unwraps Vue's reactive proxy so
    // the identity check holds for real HTMLAudioElement values and test
    // doubles alike.
    audio.addEventListener("playing", () => {
      if (toRaw(audioElement.value) !== toRaw(audio)) return;
      if (!isPlaying.value) {
        console.log("[web-playback] media:playing -> isPlaying=true", {
          songId: currentSong.value ? currentSong.value.id : null,
        });
        isPlaying.value = true;
      }
    });
    audio.addEventListener("pause", () => {
      if (toRaw(audioElement.value) !== toRaw(audio)) return;
      if (isPlaying.value) {
        console.log("[web-playback] media:pause -> isPlaying=false", {
          songId: currentSong.value ? currentSong.value.id : null,
        });
        isPlaying.value = false;
      }
    });
    audioElement.value = audio;
    return audio;
  };

  webBackend = {
    kind: "web",
    init: async () => {
      ensureAudioElement();
      restoreWebPlaybackSession();
      startSourcePolling();
      await reconcileQueueSources();
    },
    cleanup: () => {
      if (audioElement.value) {
        audioElement.value.pause();
        audioElement.value = null;
      }
    },
    usesRawPlaybackPath: () => false,
    playSong: async (song, seekTime, queueOverride) => {
      const preparedSong = await prepareSongForPlayback(song);
      // Hold the watcher silent for the entire song switch. The song that just
      // ended fires a natural "pause" that flips isPlaying to false; without
      // this guard the isPlaying watcher would pause the element mid-switch.
      isPlayingInternal = true;
      try {
        pendingSeekTargetMs = null;
        const sourceQueue = getBaseQueue(queueOverride);
        activeQueue.value = sourceQueue.map((sourceSong) => {
          if (!songsMatch(sourceSong, preparedSong)) {
            return sourceSong;
          }
          return preparedSong;
        });

        // Reuse the single unlocked element. Safari/WebKit block play() on a
        // brand-new element without a user gesture (NotAllowedError), so
        // creating one per song left auto-advance paused. Swapping `src` on the
        // element the user already unlocked keeps playback going. See
        // docs/web-playback-isplaying.md.
        const audio = ensureAudioElement();
        if (!audio.paused) {
          audio.pause();
        }

        const sourceUrl = buildSongPlaybackUrl(preparedSong, seekTime);
        console.log("[web-playback] playSong", {
          songId: preparedSong.id,
          songName: preparedSong.name,
          sourceUrl,
          seekTime: seekTime === undefined ? null : seekTime,
          queueSize: activeQueue.value.length,
          sourceKey: getSongSourceKey(preparedSong),
          usesRawPlaybackPath: shouldUseRawPlaybackPath(preparedSong),
          origin: typeof window !== "undefined" ? window.location.origin : null,
        });
        pendingPlaySeekTime = seekTime;
        audio.src = sourceUrl;
        audio.preload = "auto";

        currentSong.value = preparedSong;
        currentIndex.value = activeQueue.value.findIndex((queueSong) =>
          songsMatch(queueSong, preparedSong),
        );
        duration.value = 0;
        persistPlaybackSession(activeQueue.value, preparedSong);
        await notifyPlaybackQueueStart(activeQueue.value, currentIndex.value);

        maybeEmitSongStart(activeQueue.value, preparedSong, currentIndex.value);

        await new Promise((resolve) => setTimeout(resolve, 50));

        console.log("[web-playback] calling audio.play()", {
          songId: preparedSong.id,
          readyState: audio.readyState,
          networkState: audio.networkState,
          paused: audio.paused,
        });
        try {
          await audio.play();
          console.log("[web-playback] play() resolved -> isPlaying=true", {
            songId: preparedSong.id,
            paused: audio.paused,
            readyState: audio.readyState,
          });
          isPlaying.value = true;
        } catch (error) {
          const errorName = error instanceof Error ? error.name : String(error);
          const errorMessage =
            error instanceof Error ? error.message : String(error);
          if (errorName === "AbortError") {
            // play() was interrupted before playback began — typically the
            // source was still loading. Wait for it to be ready, then start
            // playback explicitly. (NotAllowedError is a separate, hard
            // autoplay block that reusing the unlocked element avoids.)
            console.warn(
              "[web-playback] play() interrupted (AbortError), retrying once ready",
              {
                songId: preparedSong.id,
                paused: audio.paused,
                readyState: audio.readyState,
                url: sourceUrl,
              },
            );
            try {
              await waitForAudioReady(audio, 2000);
              await audio.play();
              console.log(
                "[web-playback] play() retry resolved -> isPlaying=true",
                { songId: preparedSong.id },
              );
              isPlaying.value = true;
            } catch (retryError) {
              console.warn(
                "[web-playback] play() retry did not start, reconciling",
                { songId: preparedSong.id },
              );
              isPlaying.value = !audio.paused;
            }
          } else {
            console.error("[web-playback] play() REJECTED -> isPlaying=false", {
              songId: preparedSong.id,
              errorName,
              errorMessage,
              url: sourceUrl,
            });
            isPlaying.value = false;
            throw new PlaybackStartError("Autoplay was blocked by the browser");
          }
        }
      } finally {
        isPlayingInternal = false;
      }
    },
    play: async () => {
      console.log("[web-playback] play() called", {
        hasAudio: Boolean(audioElement.value),
        hasCurrentSong: Boolean(currentSong.value),
        hasSrc: audioElement.value ? Boolean(audioElement.value.src) : null,
        currentSongId: currentSong.value ? currentSong.value.id : null,
      });
      if (!audioElement.value) return;

      const allSongs = songs();
      if (!currentSong.value && allSongs.length > 0) {
        await playSongAtIndex(allSongs[0], 0);
        return;
      }

      if (currentSong.value) {
        if (!audioElement.value.src) {
          const queue =
            activeQueue.value.length > 0 ? activeQueue.value : allSongs;
          const restoredIndex =
            currentIndex.value >= 0
              ? currentIndex.value
              : queue.findIndex((song) =>
                  currentSong.value
                    ? songsMatch(song, currentSong.value)
                    : false,
                );
          if (restoredIndex >= 0 && queue[restoredIndex]) {
            await playSongAtIndex(queue[restoredIndex], restoredIndex, queue);
            return;
          }
          await playSong(currentSong.value);
          return;
        }
        await audioElement.value.play();
        isPlaying.value = true;
      }
    },
    pause: async () => {
      console.log("[web-playback] pause() called", {
        hasAudio: Boolean(audioElement.value),
        currentSongId: currentSong.value ? currentSong.value.id : null,
      });
      if (!audioElement.value) return;

      audioElement.value.pause();
      isPlaying.value = false;
    },
    previousSong: async () => {
      await playPreviousSongInWebQueue();
    },
    nextSong: async () => {
      await playNextSongInWebQueue();
    },
    seekToTime: async (time) => {
      if (!audioElement.value || duration.value === 0) return;

      const clampedTime = Math.max(0, Math.min(time, duration.value));
      audioElement.value.currentTime = clampedTime;
      currentTime.value = clampedTime;
    },
    syncQueueState: async () => {
      persistPlaybackSession(activeQueue.value, currentSong.value);
    },
    handlePlayModeChange: async () => {},
    handleQueuePruned: async (queue, index, shouldResumePlayback) => {
      if (!shouldResumePlayback) {
        if (audioElement.value) {
          audioElement.value.pause();
          audioElement.value.src = "";
        }
        isPlaying.value = false;
        return;
      }

      const nextSongCandidate = queue[index] ?? queue[0];
      if (!nextSongCandidate) {
        return;
      }

      await playSongAtIndex(nextSongCandidate, index, queue);
    },
    refreshSession: async () => {},
    syncNormalizationConfig: async (
      _mode,
      _manualVolume,
      _fixedLufs,
      currentVolume,
    ) => {
      applyWebVolume(currentVolume);
    },
    setTimedPause: async () => {},
    clearPlaybackState: async () => {
      if (audioElement.value) {
        audioElement.value.pause();
        audioElement.value.src = "";
      }
    },
  };

  androidBackend = {
    kind: "android",
    init: async () => {
      startAndroidPolling();
      await fetchAndroidSession("initAudio");
      startSourcePolling();
      await reconcileQueueSources();
    },
    cleanup: () => {},
    usesRawPlaybackPath: (song) => {
      const sourceApiBase = resolveSourceApiBase(song.source_key);
      return isLocalhostApiBase(sourceApiBase) && song.path.length > 0;
    },
    playSong: async (song, seekTime, queueOverride, selectedIndex) => {
      const { queue, index } = buildQueueForMode(
        song,
        selectedIndex ??
          (currentIndex.value >= 0 ? currentIndex.value : undefined),
        queueOverride,
      );
      await restartAndroidPlayback(queue, index, "playSong", seekTime);
    },
    play: async () => {
      const plugin = await loadPluginApi();
      if (currentSong.value) {
        if (currentTime.value > 0) {
          await plugin.seekAndPlay(
            Math.max(0, Math.floor(currentTime.value * 1000)),
          );
        } else {
          await plugin.play({
            url: buildSongPlaybackUrl(currentSong.value),
            title: currentSong.value.name,
            coverUrl:
              currentSong.value.cover_url ??
              buildCoverUrl(
                currentSong.value.id,
                getSongSourceKey(currentSong.value),
              ) ??
              undefined,
          });
        }
      } else if (activeQueue.value.length > 0) {
        const index = currentIndex.value >= 0 ? currentIndex.value : 0;
        await playSongAtIndex(activeQueue.value[index], index);
        return;
      } else {
        const sourceSongs = songs();
        if (sourceSongs.length > 0) {
          await playSongAtIndex(sourceSongs[0], 0);
          return;
        }
      }
      await fetchAndroidSession();
    },
    pause: async () => {
      const plugin = await loadPluginApi();
      await plugin.pause();
      await fetchAndroidSession();
    },
    previousSong: async () => {
      const session = await fetchAndroidSession("previousSong:before");
      const queue = session.queue.songs.map((song) => toMusicInfo(song));
      if (queue.length === 0) return;

      const sessionIndex = session.queue.currentIndex ?? 0;
      const newIndex =
        playMode.value === "loop"
          ? sessionIndex
          : sessionIndex <= 0
            ? queue.length - 1
            : sessionIndex - 1;

      await restartAndroidPlayback(queue, newIndex, "previousSong");
    },
    nextSong: async () => {
      const session = await fetchAndroidSession("nextSong:before");
      const queue = session.queue.songs.map((song) => toMusicInfo(song));
      if (queue.length === 0) return;

      const sessionIndex = session.queue.currentIndex ?? 0;
      const newIndex =
        playMode.value === "loop"
          ? sessionIndex
          : sessionIndex >= queue.length - 1
            ? 0
            : sessionIndex + 1;

      await restartAndroidPlayback(queue, newIndex, "nextSong");
    },
    seekToTime: async (time) => {
      if (duration.value === 0) return;
      const plugin = await loadPluginApi();
      const targetPositionMs = Math.max(0, Math.floor(time * 1000));
      pendingSeekTargetMs = targetPositionMs;
      currentTime.value = targetPositionMs / 1000;
      console.log(
        "[useAudioPlayer] seekToTime(android): invoking plugin seek path",
        {
          targetTimeSeconds: time,
          targetPositionMs,
          currentIndex: currentIndex.value,
          currentSongId: currentSong.value?.id ?? null,
          isPlaying: isPlaying.value,
        },
      );
      if (isPlaying.value) {
        await plugin.seek(targetPositionMs);
      } else {
        await plugin.seekAndPlay(targetPositionMs);
      }
      await fetchAndroidSession("seekToTime");
    },
    syncQueueState: async () => {
      if (activeQueue.value.length === 0) {
        return;
      }

      const plugin = await loadPluginApi();
      const payload: PlayingQueue = {
        songs: activeQueue.value.map((song) => toQueueSong(song)),
        currentIndex: currentIndex.value >= 0 ? currentIndex.value : null,
      };

      await plugin.setPlayingQueue(payload, playMode.value as AndroidPlayMode);
      persistPlaybackSession(activeQueue.value, currentSong.value);
    },
    handlePlayModeChange: async () => {
      await androidBackend.syncQueueState();
      await syncAndroidPlayMode();
    },
    handleQueuePruned: async (queue, index, shouldResumePlayback, source) => {
      if (!shouldResumePlayback) {
        await androidBackend.syncQueueState();
        return;
      }

      await restartAndroidPlayback(queue, index, source);
    },
    refreshSession: async (source = "manual") => {
      await fetchAndroidSession(source);
    },
    syncNormalizationConfig: async (
      mode,
      manualVolume,
      fixedLufs,
      lufsPrecacheCount,
    ) => {
      const plugin = await loadPluginApi();
      await plugin.setNormalizationConfig({
        mode,
        manualVolume: Math.min(1, Math.max(0, manualVolume)),
        fixedLufs,
        lufsPrecacheCount,
      });
    },
    setTimedPause: async (delayMs) => {
      const plugin = await loadPluginApi();
      await plugin.pauseAfter(Math.max(0, Math.floor(delayMs)));
    },
    clearPlaybackState: async () => {
      const plugin = await loadPluginApi();
      await plugin.stop();
      await plugin.setPlayingQueue(
        { songs: [], currentIndex: null },
        playMode.value as AndroidPlayMode,
      );
    },
  };

  const initAudio = async () => {
    const runtimeCapabilities = await getRuntimeCapabilities();
    const playbackBackend = runtimeCapabilities.usesAndroidPlaybackBackend
      ? androidBackend
      : webBackend;
    isAndroidPlayer.value = playbackBackend.kind === "android";
    await playbackBackend.init();
  };

  watch(isPlaying, (playing) => {
    console.log("[web-playback] isPlaying changed", {
      playing,
      backend: getPlaybackBackend().kind,
      isPlayingInternal,
      hasAudio: Boolean(audioElement.value),
      currentSongId: currentSong.value ? currentSong.value.id : null,
    });
    if (getPlaybackBackend().kind !== "web") return;
    if (!audioElement.value) return;
    if (isPlayingInternal) return;

    if (playing) {
      void audioElement.value.play();
    } else {
      audioElement.value.pause();
    }
  });

  const cleanup = () => {
    stopAndroidPolling();
    stopSourcePolling();
    getPlaybackBackend().cleanup();
  };

  if (getCurrentScope()) {
    onScopeDispose(cleanup);
  }

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
    syncNormalizationConfig,
    reconcileQueueSources,
  };
}
