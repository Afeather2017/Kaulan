import { ref, watch, getCurrentScope, onScopeDispose } from "vue";
import { getApiBase } from "@/utils/api";
import { checkIsAndroid } from "@/utils/platform";
import {
  getStoredPlaybackSession,
  removeStoredPlaybackSession,
  setStoredPlaybackSession,
  type StoredPlaybackQueueSong,
} from "@/utils/storage";
import type {
  PlaybackSession,
  PlayMode as AndroidPlayMode,
  PlayingQueue,
  NormalizationMode,
} from "music-notification-api";

// Related documentation:
// - `docs/lyric-sync-timing.md`

export interface MusicInfo {
  id: number;
  name: string;
  lufs: number | null;
  path: string;
  stream_url?: string | null;
  cover_url?: string | null;
  source?: "youtube" | "netease" | "bilibili";
  is_temporary?: boolean;
}

export type PlayMode = "sequential" | "shuffle" | "loop";

interface UseAudioPlayerOptions {
  songs: () => MusicInfo[];
  onSongEnd?: () => void;
  onSongStart?: (currentSong: MusicInfo, nextSong: MusicInfo | null) => void;
  prepareSong?: (song: MusicInfo) => Promise<MusicInfo>;
}

type MusicNotificationApi = typeof import("music-notification-api");

interface ResolvedQueueState {
  queue: MusicInfo[];
  currentIndex: number;
  currentSong: MusicInfo | null;
  currentSongId: number | null;
}

export function useAudioPlayer(options: UseAudioPlayerOptions) {
  const { songs, onSongEnd, onSongStart, prepareSong } = options;

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
  const apiBase = getApiBase();

  const POLL_INTERVAL_MS = 1000;
  const SEEK_CONFIRMATION_THRESHOLD_SECONDS = 0.1;

  let isPlayingInternal = false;
  let pollingTimer: ReturnType<typeof setInterval> | null = null;
  let pluginApiPromise: Promise<MusicNotificationApi> | null = null;
  let lastStartedSongId: number | null = null;
  let pendingSeekTargetMs: number | null = null;

  const loadPluginApi = async (): Promise<MusicNotificationApi> => {
    if (pluginApiPromise === null) {
      pluginApiPromise = import("music-notification-api");
    }
    return pluginApiPromise;
  };

  const buildAudioUrl = (songId: number, seekTime?: number): string => {
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

  const buildCoverUrl = (songId: number): string | null => {
    if (songId <= 0) {
      return null;
    }
    try {
      return new URL(`${apiBase}/music/id/${songId}/cover`).toString();
    } catch {
      return new URL(
        `${apiBase}/music/id/${songId}/cover`,
        window.location.origin,
      ).toString();
    }
  };

  const toQueueSong = (song: MusicInfo) => ({
    id: song.id,
    name: song.name,
    path: song.path,
    url: song.stream_url ?? buildAudioUrl(song.id),
    lufs: song.lufs,
    coverUrl: song.cover_url ?? buildCoverUrl(song.id),
  });

  const toMusicInfo = (song: {
    id: number;
    name: string;
    path: string;
    lufs: number | null;
    url?: string | null;
    coverUrl?: string | null;
  }): MusicInfo => ({
    id: song.id,
    name: song.name,
    path: song.path,
    lufs: song.lufs,
    stream_url: song.url ?? null,
    cover_url: song.coverUrl ?? buildCoverUrl(song.id),
  });

  const toStoredPlaybackQueueSong = (
    song: MusicInfo,
  ): StoredPlaybackQueueSong => ({
    id: song.id,
    name: song.name,
    path: song.path,
    url: song.stream_url ?? buildAudioUrl(song.id),
    lufs: song.lufs,
    coverUrl: song.cover_url ?? buildCoverUrl(song.id),
  });

  const persistPlaybackSession = (
    queue: MusicInfo[],
    currentSongId: number | null,
  ) => {
    if (queue.length === 0) {
      removeStoredPlaybackSession();
      return;
    }

    setStoredPlaybackSession({
      currentSongId,
      queue: queue.map(toStoredPlaybackQueueSong),
      timestamp: Date.now(),
    });
  };

  const resolveQueueState = (
    queue: MusicInfo[],
    preferredCurrentSongId: number | null,
    preferredIndex: number | null,
  ): ResolvedQueueState => {
    let resolvedIndex = -1;

    if (preferredCurrentSongId !== null) {
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
    return resolveQueueState(queue, queue[matchedIndex].id, matchedIndex);
  };

  const restoreWebPlaybackSession = () => {
    const stored = getStoredPlaybackSession();
    if (!stored || stored.queue.length === 0) {
      return;
    }

    const queue = stored.queue.map(toMusicInfo);
    const restored = resolveQueueState(queue, stored.currentSongId, null);
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
      if (song.id !== selectedSong.id) {
        return song;
      }
      return selectedSong;
    });

    const resolvedIndex =
      selectedIndex ??
      normalizedQueue.findIndex((song) => song.id === selectedSong.id);
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
    const currentSongId = currentSong.value?.id ?? null;
    const sourceQueue = songs();

    if (
      currentSongId !== null &&
      sourceQueue.some((song) => song.id === currentSongId)
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
      lastStartedSongId = null;
      return;
    }
    if (lastStartedSongId === song.id) {
      return;
    }
    lastStartedSongId = song.id;
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
          session.currentSongId,
          session.queue.currentIndex,
        ))
      : resolveQueueState(
          sessionQueue,
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

    persistPlaybackSession(activeQueue.value, resolved.currentSongId);
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

  const refreshAndroidSession = async (source = "manual") => {
    if (!isAndroidPlayer.value) return;
    const plugin = await loadPluginApi();
    const session = await plugin.getPlaybackSession();
    applyAndroidSession(session, source);
  };

  const getAndroidSessionSnapshot = async (
    source = "manual",
  ): Promise<PlaybackSession | null> => {
    if (!isAndroidPlayer.value) return null;
    const plugin = await loadPluginApi();
    const session = await plugin.getPlaybackSession();
    applyAndroidSession(session, source);
    return session;
  };

  const startAndroidPolling = () => {
    if (pollingTimer) return;
    pollingTimer = setInterval(() => {
      void refreshAndroidSession("poll").catch((error) => {
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

  const syncAndroidQueue = async (
    selectedSong: MusicInfo,
    selectedIndex?: number,
  ) => {
    const plugin = await loadPluginApi();
    const { queue, index } = buildQueueForMode(selectedSong, selectedIndex);
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
    persistPlaybackSession(activeQueue.value, currentSong.value?.id ?? null);
  };

  const syncAndroidQueueState = async () => {
    if (!isAndroidPlayer.value || activeQueue.value.length === 0) {
      return;
    }

    const plugin = await loadPluginApi();
    const payload: PlayingQueue = {
      songs: activeQueue.value.map((song) => toQueueSong(song)),
      currentIndex: currentIndex.value >= 0 ? currentIndex.value : null,
    };

    await plugin.setPlayingQueue(payload, playMode.value as AndroidPlayMode);
    persistPlaybackSession(activeQueue.value, currentSong.value?.id ?? null);
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
      if (song.id !== preparedTargetSong.id) {
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
    persistPlaybackSession(activeQueue.value, currentSong.value?.id ?? null);

    if (seekTime !== undefined) {
      pendingSeekTargetMs = Math.max(0, Math.floor(seekTime * 1000));
      currentTime.value = pendingSeekTargetMs / 1000;
      await plugin.seekAndPlay(Math.max(0, Math.floor(seekTime * 1000)));
    } else {
      pendingSeekTargetMs = null;
      await plugin.play({
        url: targetSong.stream_url ?? buildAudioUrl(targetSong.id),
        title: targetSong.name,
        coverUrl:
          targetSong.cover_url ?? buildCoverUrl(targetSong.id) ?? undefined,
      });
    }

    await refreshAndroidSession(source);
  };

  const syncAndroidPlayMode = async () => {
    if (!isAndroidPlayer.value) return;
    const plugin = await loadPluginApi();
    if (playMode.value === "shuffle" && currentSong.value) {
      await syncAndroidQueue(currentSong.value, currentIndex.value);
      return;
    }
    await plugin.setPlayMode(playMode.value as AndroidPlayMode);
    await refreshAndroidSession();
  };

  const playSongAtIndex = async (
    song: MusicInfo,
    index: number,
    queueOverride?: MusicInfo[],
  ) => {
    currentIndex.value = index;
    playedSongIndexes.value.add(index);
    await playSong(song, undefined, queueOverride);
  };

  const playSong = async (
    song: MusicInfo,
    seekTime?: number,
    queueOverride?: MusicInfo[],
  ) => {
    if (isAndroidPlayer.value) {
      const { queue, index } = buildQueueForMode(
        song,
        currentIndex.value >= 0 ? currentIndex.value : undefined,
        queueOverride,
      );
      await restartAndroidPlayback(queue, index, "playSong", seekTime);
      return;
    }

    const preparedSong = await prepareSongForPlayback(song);
    pendingSeekTargetMs = null;
    const sourceQueue = getBaseQueue(queueOverride);
    activeQueue.value = sourceQueue.map((sourceSong) => {
      if (sourceSong.id !== preparedSong.id) {
        return sourceSong;
      }
      return preparedSong;
    });

    if (audioElement.value && !audioElement.value.paused) {
      audioElement.value.pause();
    }

    const newAudio = new Audio();
    const sourceUrl =
      preparedSong.stream_url ?? buildAudioUrl(preparedSong.id, seekTime);
    newAudio.src = sourceUrl;
    newAudio.preload = "auto";

    newAudio.addEventListener("loadedmetadata", () => {
      duration.value = newAudio.duration || 0;
      if (seekTime === undefined) {
        return;
      }

      const clampedTime = Math.max(
        0,
        Math.min(seekTime, duration.value || seekTime),
      );
      newAudio.currentTime = clampedTime;
      currentTime.value = clampedTime;
    });

    newAudio.addEventListener("timeupdate", () => {
      currentTime.value = newAudio.currentTime || 0;
    });

    newAudio.addEventListener("seeked", () => {
      currentTime.value = newAudio.currentTime || 0;
    });

    newAudio.addEventListener("seeking", () => {
      currentTime.value = newAudio.currentTime || 0;
    });

    newAudio.addEventListener("ended", () => {
      if (playMode.value === "loop") {
        if (currentSong.value) {
          void playSong(currentSong.value);
        }
      } else {
        void nextSong();
      }
      onSongEnd?.();
    });

    audioElement.value = newAudio;
    currentSong.value = preparedSong;
    currentIndex.value = activeQueue.value.findIndex(
      (queueSong) => queueSong.id === preparedSong.id,
    );
    duration.value = 0;
    persistPlaybackSession(activeQueue.value, preparedSong.id);
    isPlayingInternal = true;

    maybeEmitSongStart(activeQueue.value, preparedSong, currentIndex.value);

    await new Promise((resolve) => setTimeout(resolve, 50));

    try {
      await newAudio.play();
      isPlaying.value = true;
    } catch (error) {
      console.error("Failed to play audio:", error);
      isPlaying.value = false;
    } finally {
      isPlayingInternal = false;
    }
  };

  const play = async () => {
    if (isAndroidPlayer.value) {
      const plugin = await loadPluginApi();
      if (currentSong.value) {
        if (currentTime.value > 0) {
          await plugin.seekAndPlay(
            Math.max(0, Math.floor(currentTime.value * 1000)),
          );
        } else {
          await plugin.play({
            url:
              currentSong.value.stream_url ??
              buildAudioUrl(currentSong.value.id),
            title: currentSong.value.name,
            coverUrl:
              currentSong.value.cover_url ??
              buildCoverUrl(currentSong.value.id) ??
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
      await refreshAndroidSession();
      return;
    }

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
            : queue.findIndex((song) => song.id === currentSong.value?.id);
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
  };

  const pause = async () => {
    if (isAndroidPlayer.value) {
      const plugin = await loadPluginApi();
      await plugin.pause();
      await refreshAndroidSession();
      return;
    }

    if (!audioElement.value) return;

    audioElement.value.pause();
    isPlaying.value = false;
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
      const selectedIndex = queueForMode.findIndex(
        (song) => song.id === currentSong.value?.id,
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
      persistPlaybackSession(activeQueue.value, currentSong.value?.id ?? null);
    }

    if (isAndroidPlayer.value) {
      await syncAndroidPlayMode();
    }
  };

  const previousSong = async () => {
    if (isAndroidPlayer.value) {
      const session = await getAndroidSessionSnapshot("previousSong:before");
      if (!session) return;

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
      return;
    }

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
    if (isAndroidPlayer.value) {
      const session = await getAndroidSessionSnapshot("nextSong:before");
      if (!session) return;

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
      return;
    }

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
    if (isAndroidPlayer.value) {
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
      await refreshAndroidSession("seekToTime");
      return;
    }

    if (!audioElement.value || duration.value === 0) return;

    const clampedTime = Math.max(0, Math.min(time, duration.value));
    audioElement.value.currentTime = clampedTime;
    currentTime.value = clampedTime;
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
    currentVolume: number,
  ) => {
    if (isAndroidPlayer.value) {
      const plugin = await loadPluginApi();
      await plugin.setNormalizationConfig({
        mode,
        manualVolume: Math.min(1, Math.max(0, manualVolume)),
        fixedLufs,
      });
      return;
    }

    applyWebVolume(currentVolume);
  };

  const setTimedPause = async (delayMs: number) => {
    if (!isAndroidPlayer.value) {
      return;
    }
    const plugin = await loadPluginApi();
    await plugin.pauseAfter(Math.max(0, Math.floor(delayMs)));
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

  const initAudio = async () => {
    isAndroidPlayer.value = await checkIsAndroid();
    if (isAndroidPlayer.value) {
      startAndroidPolling();
      await refreshAndroidSession("initAudio");
      return;
    }

    audioElement.value = new Audio();
    audioElement.value.addEventListener("timeupdate", () => {
      currentTime.value = audioElement.value?.currentTime || 0;
    });
    audioElement.value.addEventListener("ended", () => {
      if (playMode.value === "loop") {
        if (currentSong.value) {
          void playSong(currentSong.value);
        }
      } else {
        void nextSong();
      }
      onSongEnd?.();
    });

    restoreWebPlaybackSession();
  };

  watch(isPlaying, (playing) => {
    if (isAndroidPlayer.value) return;
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
    if (audioElement.value) {
      audioElement.value.pause();
      audioElement.value = null;
    }
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
  };
}
