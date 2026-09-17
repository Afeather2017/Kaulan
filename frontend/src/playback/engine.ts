import { computed, getCurrentScope, onScopeDispose, ref } from "vue";
import type { LibrarySourceGroup } from "@/types/library";
import type { MusicInfo } from "@/types/music";
import { getRuntimeCapabilities } from "@/utils/platform";
import type { NormalizationMode } from "music-notification-api";
import {
  buildQueueForMode,
  persistPlaybackSession,
  randomSongIndexNoRepeat,
  songsMatch,
} from "@/playback/shared";
import type {
  BackendEvent,
  NormalizationConfigPayload,
  PlayMode,
  PlaybackBackend,
  PlaybackError,
  PlaybackSnapshot,
  PlaybackStatus,
  SongEndCause,
} from "@/playback/types";

export class PlaybackStartError extends Error {
  readonly code: "autoplay_blocked";

  constructor(message: string) {
    super(message);
    this.name = "PlaybackStartError";
    this.code = "autoplay_blocked";
  }
}

export interface PlaybackEngineOptions {
  /** Live source list for the active playback context (playlist or search). */
  songs: () => MusicInfo[];
  onSongEnd?: () => void;
  onSongStart?: (currentSong: MusicInfo, nextSong: MusicInfo | null) => void;
  onPlaybackQueueStart?: (
    queue: MusicInfo[],
    currentIndex: number,
    playMode: PlayMode,
  ) => Promise<void> | void;
  prepareSong?: (song: MusicInfo) => Promise<MusicInfo>;
  sourceGroups?: () => LibrarySourceGroup[];
}

const TICK_INTERVAL_MS = 250;
/** How long a seek target pins the displayed position before backend truth wins. */
const SEEK_PIN_TIMEOUT_MS = 1500;
/** Snapshot positions within this window of the pinned seek target ack it. */
const SEEK_ACK_TOLERANCE_SEC = 0.35;

/**
 * The single owner of frontend playback state. UI commands come in, backend
 * events go out to `applySnapshot` — the only code path allowed to write
 * transport state (status/position/duration/song). Queue order and play mode
 * are the engine's own authority and are pushed to the backend.
 */
export function createPlaybackEngine(
  options: PlaybackEngineOptions,
  createBackends: () => { web: PlaybackBackend; android: PlaybackBackend },
) {
  const { songs, onSongEnd, onSongStart, onPlaybackQueueStart, prepareSong } =
    options;

  // ---- reactive state (the only mirrors of backend truth) ----
  const status = ref<PlaybackStatus>("idle");
  const currentSong = ref<MusicInfo | null>(null);
  const currentIndex = ref(-1);
  const activeQueue = ref<MusicInfo[]>([]);
  const playMode = ref<PlayMode>("sequential");
  const currentTime = ref(0);
  const duration = ref(0);
  const playedSongIndexes = ref<Set<number>>(new Set());
  const playbackError = ref<PlaybackError | null>(null);
  const isPlaying = computed(() => status.value === "playing");

  // ---- non-reactive sync bookkeeping ----
  let backend: PlaybackBackend | null = null;
  let lastSnapshot: PlaybackSnapshot | null = null;
  let lastStartedSongIdentity: string | null = null;
  let pendingSeekPin: { targetSec: number; expiresAtMs: number } | null = null;
  let ticker: ReturnType<typeof setInterval> | null = null;
  let unlisten: (() => void) | null = null;

  const { web, android } = createBackends();

  // ---- the single reconciliation path ----
  const applySnapshot = (snapshot: PlaybackSnapshot, _reason: string) => {
    lastSnapshot = snapshot;
    status.value = snapshot.status;
    playbackError.value = snapshot.error ?? null;
    duration.value = snapshot.durationSec;

    if (snapshot.queue && snapshot.queue.length > 0) {
      const identical =
        snapshot.queue.length === activeQueue.value.length &&
        snapshot.queue.every((song, i) =>
          songsMatch(song, activeQueue.value[i]),
        );
      if (!identical) {
        activeQueue.value = snapshot.queue;
      }
    }

    const newSong = snapshot.song;
    if (newSong) {
      const newIdentity = newSong.source
        ? `online:${newSong.source}:${newSong.id}:${newSong.name}`
        : `${newSong.device_id ?? "local"}:${newSong.id}`;

      // Always adopt the backend's resolved metadata; the start callback
      // fires per identity (deduped across event bursts).
      currentSong.value = newSong;
      let index = snapshot.currentIndex;
      if (
        index < 0 ||
        index >= activeQueue.value.length ||
        !songsMatch(activeQueue.value[index], newSong)
      ) {
        index = activeQueue.value.findIndex((song) =>
          songsMatch(song, newSong),
        );
      }
      currentIndex.value = index;

      if (lastStartedSongIdentity !== newIdentity) {
        if (index >= 0) {
          playedSongIndexes.value.add(index);
        }
        maybeEmitSongStart(activeQueue.value, newSong, index);
      }
    } else if (currentSong.value && snapshot.status === "idle") {
      currentSong.value = null;
      currentIndex.value = -1;
      lastStartedSongIdentity = null;
    }

    // Position reconciliation with pending-seek pinning: the displayed
    // position holds the seek target until a snapshot confirms it (or the
    // pin expires). This replaces the old Android-only drift guard and gives
    // web the same scrub behavior.
    if (pendingSeekPin) {
      const drift = Math.abs(snapshot.positionSec - pendingSeekPin.targetSec);
      const expired = Date.now() > pendingSeekPin.expiresAtMs;
      if (drift <= SEEK_ACK_TOLERANCE_SEC || expired) {
        pendingSeekPin = null;
        currentTime.value = snapshot.positionSec;
      }
      return;
    }
    currentTime.value = snapshot.positionSec;
  };

  const maybeEmitSongStart = (
    queue: MusicInfo[],
    song: MusicInfo,
    index: number,
  ) => {
    const identityKey = song.source
      ? `online:${song.source}:${song.id}:${song.name}`
      : `${song.device_id ?? "local"}:${song.id}`;
    // Identity-deduped, matching the old engine: loop replays of the same
    // song don't re-fire the start callback.
    if (lastStartedSongIdentity === identityKey) {
      return;
    }
    lastStartedSongIdentity = identityKey;
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
      nextIndex >= 0 && index >= 0 && nextIndex < queue.length
        ? queue[nextIndex]
        : null;
    onSongStart(song, nextSong);
  };

  const handleBackendEvent = (event: BackendEvent) => {
    if (event.type === "snapshot") {
      applySnapshot(event.snapshot, event.reason);
      return;
    }
    // songEnded: backend-level completion/failure of the loaded song.
    // Web reports it from the element `ended`/`error` events; Android only
    // for native errors (its completion auto-advances natively, which the
    // engine sees as a snapshot with a new index).
    onSongEnd?.();
    void advanceFromEnd(event.endedBy);
  };

  // ---- queue authority ----
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

  const persist = () => {
    persistPlaybackSession(activeQueue.value, currentSong.value);
  };

  const prepareSongForPlayback = async (
    song: MusicInfo,
  ): Promise<MusicInfo> => {
    if (!prepareSong) {
      return song;
    }
    return await prepareSong(song);
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

  const computeNextIndex = (): number => {
    const all = activeQueue.value.length > 0 ? activeQueue.value : songs();
    if (all.length === 0 || !currentSong.value) return -1;
    if (playMode.value === "loop") return currentIndex.value;
    if (playMode.value === "shuffle") {
      return randomSongIndexNoRepeat(all.length, playedSongIndexes.value);
    }
    return currentIndex.value === all.length - 1 ? 0 : currentIndex.value + 1;
  };

  const computePreviousIndex = (): number => {
    const all = activeQueue.value.length > 0 ? activeQueue.value : songs();
    if (all.length === 0 || !currentSong.value) return -1;
    if (playMode.value === "loop") return currentIndex.value;
    if (playMode.value === "shuffle") {
      return randomSongIndexNoRepeat(all.length, playedSongIndexes.value);
    }
    return currentIndex.value === 0 ? all.length - 1 : currentIndex.value - 1;
  };

  const advanceFromEnd = async (cause: SongEndCause) => {
    if (cause === "error") {
      if (!currentSong.value) return;
      if (activeQueue.value.length <= 1) {
        status.value = "paused";
        return;
      }
      const nextIndex =
        (Math.max(currentIndex.value, 0) + 1) % activeQueue.value.length;
      await skipToIndex(nextIndex);
      return;
    }

    if (playMode.value === "loop") {
      if (currentSong.value) {
        await skipToIndex(currentIndex.value, { startAtSec: 0 });
      }
      return;
    }

    const nextIndex = computeNextIndex();
    if (nextIndex >= 0) {
      await skipToIndex(nextIndex);
    }
  };

  // ---- public commands ----
  const playSong = async (
    song: MusicInfo,
    seekTime?: number,
    queueOverride?: MusicInfo[],
    selectedIndex?: number,
  ) => {
    const target = backend ?? web;
    const prepared = await prepareSongForPlayback(song);
    const { queue, index } = buildQueueForMode(
      getBaseQueue(queueOverride),
      prepared,
      selectedIndex,
      playMode.value,
    );

    activeQueue.value = queue;
    currentIndex.value = index;
    currentSong.value = queue[index] ?? prepared;
    if (index >= 0) {
      playedSongIndexes.value.add(index);
    }
    persist();
    await notifyPlaybackQueueStart(queue, index);

    // Restart semantics for the currently loaded song, preserved from the old
    // engines: re-playing the active song starts it over unless a seek target
    // is given (restored sessions reload from the top; lyric-click passes time).
    const restartCurrent =
      !seekTime &&
      status.value !== "idle" &&
      songsMatch(currentSong.value ?? prepared, prepared);
    const startAtSec = seekTime ?? (restartCurrent ? 0 : undefined);
    await target.load(queue, index, { startAtSec, playWhenReady: true });
  };

  const playSongAtIndex = async (
    song: MusicInfo,
    index: number,
    queueOverride?: MusicInfo[],
  ) => {
    await playSong(song, undefined, queueOverride, index);
  };

  const skipToIndex = async (
    index: number,
    loadOptions?: { startAtSec?: number },
  ) => {
    const target = backend ?? web;
    const queue = activeQueue.value;
    if (index < 0 || index >= queue.length) {
      return;
    }
    playedSongIndexes.value.add(index);
    await target.skipTo(index, { playWhenReady: true, ...loadOptions });
  };

  const play = async () => {
    const target = backend ?? web;
    const song = currentSong.value;
    if (!song) {
      const sourceSongs = songs();
      if (sourceSongs.length > 0) {
        await playSongAtIndex(sourceSongs[0], 0);
      }
      return;
    }
    if (status.value === "idle") {
      // Restored session with no media loaded yet: full load from the top.
      await playSong(song);
      return;
    }
    await target.play();
  };

  const pause = async () => {
    await (backend ?? web).pause();
  };

  const nextSong = async () => {
    const nextIndex = computeNextIndex();
    if (nextIndex >= 0) {
      await skipToIndex(nextIndex);
    }
  };

  const previousSong = async () => {
    const previousIndex = computePreviousIndex();
    if (previousIndex >= 0) {
      await skipToIndex(previousIndex);
    }
  };

  /**
   * Seek the current song. Product rule: seeking while paused resumes
   * (playWhenReady defaults to true in the backend contract). When metadata
   * hasn't loaded yet (duration 0), fall back to a full reload from the
   * target time — the same behavior the old web engine exposed to lyric clicks.
   */
  const seekToTime = async (time: number) => {
    const target = backend ?? web;
    if (!currentSong.value) {
      return;
    }
    if (duration.value <= 0) {
      await playSong(currentSong.value, time);
      return;
    }
    const clamped = Math.max(0, Math.min(time, duration.value));
    pendingSeekPin = {
      targetSec: clamped,
      expiresAtMs: Date.now() + SEEK_PIN_TIMEOUT_MS,
    };
    currentTime.value = clamped;
    await target.seek(clamped, { playWhenReady: true });
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
      const currentSongIdentity = currentSong.value;
      const sourceQueue = songs();
      const queueForMode = sourceQueue.some((song) =>
        songsMatch(song, currentSongIdentity),
      )
        ? sourceQueue.slice()
        : activeQueue.value.length > 0
          ? activeQueue.value.slice()
          : sourceQueue.slice();
      const selectedIndex = queueForMode.findIndex((song) =>
        songsMatch(song, currentSongIdentity),
      );
      const { queue, index } = buildQueueForMode(
        queueForMode,
        currentSong.value,
        selectedIndex >= 0 ? selectedIndex : undefined,
        playMode.value,
      );

      activeQueue.value = queue;
      currentIndex.value = index;
      currentSong.value = queue[index] ?? currentSong.value;
      playedSongIndexes.value = index >= 0 ? new Set([index]) : new Set();
      persist();
      const target = backend ?? web;
      await target.syncQueue(activeQueue.value, index);
      await target.setPlayMode(playMode.value);
    }
  };

  const setTimedPause = async (delayMs: number) => {
    await (backend ?? web).pauseAfter(delayMs);
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

  const setVolume = async (volume: number) => {
    await (backend ?? web).setVolume(volume);
  };

  const syncNormalizationConfig = async (
    mode: NormalizationMode,
    manualVolume: number,
    fixedLufs: number,
    lufsPrecacheCount: number,
    currentVolume: number,
  ) => {
    const payload: NormalizationConfigPayload = {
      mode,
      manualVolume,
      fixedLufs,
      lufsPrecacheCount,
      currentVolume,
    };
    await (backend ?? web).syncNormalizationConfig(payload);
  };

  const replaceQueue = async (nextQueue: MusicInfo[]) => {
    activeQueue.value = nextQueue;
    await (backend ?? web).syncQueue(nextQueue, currentIndex.value);
    persist();
  };

  // ---- interpolation clock: smooth position between snapshots ----
  const startTicker = () => {
    if (ticker) return;
    ticker = setInterval(() => {
      if (pendingSeekPin) return;
      const snapshot = lastSnapshot;
      if (!snapshot || snapshot.status !== "playing") {
        if (snapshot && snapshot.status !== "playing") {
          currentTime.value = snapshot.positionSec;
        }
        return;
      }
      const elapsedSec = (Date.now() - snapshot.positionAtMs) / 1000;
      const interpolated = snapshot.positionSec + Math.max(0, elapsedSec);
      currentTime.value =
        snapshot.durationSec > 0
          ? Math.min(snapshot.durationSec, interpolated)
          : interpolated;
    }, TICK_INTERVAL_MS);
  };

  const stopTicker = () => {
    if (!ticker) return;
    clearInterval(ticker);
    ticker = null;
  };

  // ---- init / dispose ----
  const initAudio = async () => {
    const runtimeCapabilities = await getRuntimeCapabilities();
    backend = runtimeCapabilities.usesAndroidPlaybackBackend ? android : web;

    unlisten = backend.on(handleBackendEvent);
    const snapshot = await backend.init();
    applySnapshot(snapshot, "restore");
    startTicker();

    // Android events can pause while the webview is backgrounded; resync on
    // return so the UI doesn't rely on the next tick.
    if (backend.kind === "android" && typeof document !== "undefined") {
      document.addEventListener("visibilitychange", handleVisibilityChange);
    }
  };

  const handleVisibilityChange = () => {
    if (document.visibilityState !== "visible" || !backend) return;
    void backend
      .getSession()
      .then((snapshot) => applySnapshot(snapshot, "restore"))
      .catch((error) => {
        console.warn("[playback] visibility resync failed", error);
      });
  };

  const refreshAndroidSession = async () => {
    if ((backend ?? web).kind !== "android") return;
    const snapshot = await (backend as typeof android).getSession();
    applySnapshot(snapshot, "restore");
  };

  const syncAndroidQueueState = async () => {
    if ((backend ?? web).kind !== "android") return;
    await (backend as typeof android).syncQueue(
      activeQueue.value,
      currentIndex.value,
    );
    persist();
  };

  const cleanup = () => {
    stopTicker();
    unlisten?.();
    unlisten = null;
    backend?.dispose();
    if (typeof document !== "undefined") {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    }
  };

  if (getCurrentScope()) {
    onScopeDispose(cleanup);
  }

  // Restored sessions keep no playMode in storage; the backend init snapshot
  // must not override the engine's mode (engine is the play-mode authority),
  // so snapshots are deliberately not applied to playMode anywhere.

  return {
    // state
    status,
    currentSong,
    currentIndex,
    activeQueue,
    playMode,
    currentTime,
    duration,
    isPlaying,
    playedSongIndexes,
    playbackError,
    isAndroidPlayer: computed(() => (backend ?? web).kind === "android"),
    backendKind: computed(() => (backend ?? web).kind),
    // commands
    initAudio,
    play,
    pause,
    playSong,
    playSongAtIndex,
    togglePlayMode,
    previousSong,
    nextSong,
    seekToTime,
    setTimedPause,
    setVolume,
    syncNormalizationConfig,
    replaceQueue,
    resetPlaylist,
    formatTime,
    refreshAndroidSession,
    syncAndroidQueueState,
    // testing seams
    __applySnapshotForTests: applySnapshot,
  };
}

export type PlaybackEngine = ReturnType<typeof createPlaybackEngine>;
