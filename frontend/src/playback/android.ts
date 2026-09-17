import type { LibrarySourceGroup } from "@/types/library";
import type { MusicInfo } from "@/types/music";
import { isLocalhostApiBase } from "@/utils/platform";
import { resolveSourceApiBase } from "@/utils/api";
import { loadStoredQueue, resolveQueueState } from "@/playback/shared";
import type {
  BackendEvent,
  NormalizationConfigPayload,
  PlaybackBackend,
  PlaybackSnapshot,
  PlayMode,
  SnapshotReason,
} from "@/playback/types";
import type {
  NativePlaybackStatus,
  PlaybackSession,
  PlaybackSnapshotEvent,
  PlayMode as NativePlayMode,
  PlayingQueue,
  QueueSong,
} from "music-notification-api";

export interface AndroidBackendOptions {
  sourceGroups: () => LibrarySourceGroup[];
}

type PluginApi = typeof import("music-notification-api");

let pluginApiPromise: Promise<PluginApi> | null = null;

const loadPluginApi = (): Promise<PluginApi> => {
  if (pluginApiPromise === null) {
    pluginApiPromise = import("music-notification-api");
  }
  return pluginApiPromise;
};

/**
 * Android backend: the native MusicPlayerService is the transport authority
 * and pushes snapshots over the plugin event stream (transitions + 250 ms
 * ticks). No polling. The backend keeps a MusicInfo mirror of the native
 * queue so snapshot events resolve instantly; an unknown song id triggers a
 * one-shot full session resync.
 */
export function createAndroidBackend(
  options: AndroidBackendOptions,
): PlaybackBackend {
  const listeners = new Set<(event: BackendEvent) => void>();
  let unlistenEvents: (() => void) | null = null;
  let sessionQueue: MusicInfo[] = [];
  let sessionIndex = -1;
  let sessionPlayMode: PlayMode = "sequential";
  let lastPushedKey: string | null = null;
  let lastEmittedSnapshot: PlaybackSnapshot | null = null;
  let disposed = false;

  const getSourceGroups = () => options.sourceGroups();

  const emit = (event: BackendEvent) => {
    if (event.type === "snapshot") {
      lastEmittedSnapshot = event.snapshot;
    }
    listeners.forEach((listener) => listener(event));
  };

  // Native "kaulan" tracks resolve playback through the device-id map
  // (/api/discovery/resolutions/{id}), which requires a non-blank id. Songs
  // restored from storage (collections, persisted queue) store "" for the
  // local device, so backfill the owning source group's id at queue-build time.
  const resolveQueueDeviceId = (song: MusicInfo): string | null => {
    if (song.device_id) {
      return song.device_id;
    }
    const apiBase = resolveSourceApiBase(song.source_key);
    // `|| null` (not `?? null`) so a still-loading group's blank device_id
    // becomes null instead of an unresolvable "" in the native queue.
    const groupDeviceId = getSourceGroups().find(
      (group) => group.apiBase === apiBase,
    )?.device_id;
    return groupDeviceId || null;
  };

  const usesRawPlaybackPath = (song: MusicInfo): boolean => {
    const sourceApiBase = resolveSourceApiBase(song.source_key);
    // Only a content URI or an absolute path is directly openable by the
    // native MediaPlayer. Songs restored from storage carry just a basename
    // (see songRestore.buildRestoredSong), which must fall back to the
    // "kaulan" HTTP stream instead of failing with ENOENT.
    const isRawLocator =
      /^content:\/\//i.test(song.path) || song.path.startsWith("/");
    return isLocalhostApiBase(sourceApiBase) && isRawLocator;
  };

  const toQueueSong = (song: MusicInfo): QueueSong => {
    const localRaw = usesRawPlaybackPath(song);
    const temporary = !!song.source || (!!song.is_temporary && song.id <= 0);
    const sourceKind = temporary
      ? "temporary"
      : localRaw
        ? "local_raw"
        : "kaulan";
    const tempSongUrl =
      sourceKind === "temporary"
        ? (song.stream_url ??
          `${resolveSourceApiBase(song.source_key)}/music/id/${song.id}`)
        : null;
    return {
      id: song.id,
      name: song.name,
      deviceId: sourceKind === "kaulan" ? resolveQueueDeviceId(song) : null,
      sourceKind,
      localUri: sourceKind === "local_raw" ? song.path : null,
      tempSongUrl,
      lufs: song.lufs,
      coverUrl: sourceKind === "temporary" ? (song.cover_url ?? null) : null,
    };
  };

  // Convert the native source-specific queue shape back to frontend metadata.
  const androidSongToMusicInfo = (song: QueueSong): MusicInfo => {
    const deviceId = song.deviceId ?? "";
    const sourceKey =
      song.sourceKind === "kaulan"
        ? (getSourceGroups().find((group) => group.device_id === deviceId)
            ?.apiBase ?? null)
        : null;
    const kaulanUrl =
      sourceKey === null ? null : `${sourceKey}/music/id/${song.id}`;
    const path =
      song.sourceKind === "local_raw"
        ? (song.localUri ?? "")
        : song.sourceKind === "temporary"
          ? (song.tempSongUrl ?? "")
          : (kaulanUrl ?? "");
    return {
      id: song.id,
      name: song.name,
      path,
      lufs: song.lufs,
      device_id: deviceId,
      stream_url:
        song.sourceKind === "local_raw"
          ? (song.localUri ?? null)
          : song.sourceKind === "temporary"
            ? (song.tempSongUrl ?? null)
            : kaulanUrl,
      cover_url:
        song.coverUrl ??
        (sourceKey === null ? null : `${sourceKey}/music/id/${song.id}/cover`),
      source_key: sourceKey,
      is_temporary: song.sourceKind === "temporary",
    };
  };

  // Content-only identity: index changes ride playTrackAtIndex and must not
  // trigger a queue re-push.
  const queuePushKey = (queue: MusicInfo[]): string =>
    JSON.stringify({
      q: queue.map((song) => [
        song.id,
        song.device_id ?? null,
        song.stream_url ?? song.path,
      ]),
      m: sessionPlayMode,
    });

  const snapshotFromSession = (
    session: PlaybackSession,
    positionAtMs: number,
    queueOverride?: MusicInfo[],
  ): PlaybackSnapshot => {
    const songs =
      queueOverride ?? session.queue.songs.map(androidSongToMusicInfo);
    const resolved = resolveQueueState(
      songs,
      null,
      session.currentSongId,
      session.queue.currentIndex,
    );
    const runtime = session.runtime;
    const status: NativePlaybackStatus =
      runtime.status ??
      (runtime.isPlaying
        ? "playing"
        : resolved.currentSong
          ? "paused"
          : "idle");
    return {
      status,
      song: resolved.currentSong,
      positionSec: runtime.positionMs / 1000,
      positionAtMs,
      durationSec: runtime.durationMs / 1000,
      currentIndex: resolved.currentIndex,
      playMode: (session.playMode as PlayMode) ?? "sequential",
      queue: songs,
    };
  };

  // A native session holding exactly one invalid song (id <= 0) is a
  // degenerate placeholder; rebuild state from the localStorage copy instead.
  const resyncSession = async (reason: SnapshotReason): Promise<void> => {
    const plugin = await loadPluginApi();
    const session = await plugin.getPlaybackSession();
    if (disposed) return;

    const converted = session.queue.songs.map(androidSongToMusicInfo);
    const shouldRecoverFromStorage =
      converted.length === 1 && converted[0]?.id <= 0;
    let queueOverride: MusicInfo[] | undefined;
    if (shouldRecoverFromStorage) {
      const stored = loadStoredQueue(getSourceGroups());
      if (stored) {
        const recovered = resolveQueueState(
          stored.queue,
          stored.currentDeviceId,
          stored.currentSongId ?? session.currentSongId,
          session.queue.currentIndex,
        );
        queueOverride = recovered.queue;
      }
    }

    const snapshot = snapshotFromSession(session, Date.now(), queueOverride);
    sessionQueue = snapshot.queue ?? [];
    sessionIndex = snapshot.currentIndex;
    sessionPlayMode = snapshot.playMode;
    emit({ type: "snapshot", snapshot, reason });
  };

  const handleSnapshotEvent = async (event: PlaybackSnapshotEvent) => {
    if (disposed) return;

    // Resolve the song from the mirror; a song id we don't know means the
    // native queue changed outside a push (restore, older session) — do one
    // full resync instead of guessing.
    const mirrored = sessionQueue[event.index] ?? null;
    if (
      event.index >= 0 &&
      (!mirrored || (event.songId !== null && mirrored.id !== event.songId))
    ) {
      try {
        await resyncSession("restore");
      } catch (error) {
        console.warn(
          "[playback/android] resync after unknown song failed",
          error,
        );
      }
      return;
    }

    const mode = (event.playMode as PlayMode) ?? sessionPlayMode;
    sessionIndex = event.index;
    sessionPlayMode = mode;
    const snapshot: PlaybackSnapshot = {
      status: event.status,
      song: mirrored,
      positionSec: event.positionMs / 1000,
      positionAtMs: event.positionAtMs,
      durationSec: event.durationMs / 1000,
      currentIndex: event.index,
      playMode: mode,
    };
    emit({
      type: "snapshot",
      snapshot,
      reason:
        event.reason === "tick"
          ? "tick"
          : event.reason === "error"
            ? "error"
            : "transition",
    });
  };

  const ensureNativeQueueSynced = async (
    plugin: PluginApi,
    queue: MusicInfo[],
  ): Promise<void> => {
    const key = queuePushKey(queue);
    if (key === lastPushedKey) {
      return;
    }
    const payload: PlayingQueue = {
      songs: queue.map(toQueueSong),
      currentIndex: sessionIndex >= 0 ? sessionIndex : null,
    };
    await plugin.setPlayingQueue(payload, sessionPlayMode as NativePlayMode);
    lastPushedKey = key;
  };

  const backend: PlaybackBackend = {
    kind: "android",

    init: async () => {
      const plugin = await loadPluginApi();
      if (!unlistenEvents) {
        unlistenEvents = await plugin.onPlaybackEvent((event) => {
          void handleSnapshotEvent(event).catch((error) => {
            console.warn("[playback/android] snapshot handling failed", error);
          });
        });
      }
      await resyncSession("init");
      return (
        lastEmittedSnapshot ?? {
          status: "idle" as NativePlaybackStatus,
          song: null,
          positionSec: 0,
          positionAtMs: Date.now(),
          durationSec: 0,
          currentIndex: -1,
          playMode: sessionPlayMode,
        }
      );
    },

    getSession: async () => {
      await resyncSession("restore");
      if (!lastEmittedSnapshot) {
        throw new Error("Android session resync produced no snapshot");
      }
      return lastEmittedSnapshot;
    },

    load: async (queue, index, loadOptions) => {
      const playWhenReady = loadOptions?.playWhenReady ?? true;
      const plugin = await loadPluginApi();
      sessionQueue = queue;
      sessionIndex = index;
      await ensureNativeQueueSynced(plugin, queue);
      await plugin.playTrackAtIndex(
        index,
        playWhenReady,
        loadOptions?.startAtSec !== undefined
          ? Math.max(0, Math.floor(loadOptions.startAtSec * 1000))
          : undefined,
      );
    },

    play: async () => {
      const plugin = await loadPluginApi();
      await plugin.resume();
    },

    pause: async () => {
      const plugin = await loadPluginApi();
      await plugin.pause();
    },

    seek: async (positionSec, seekOptions) => {
      const playWhenReady = seekOptions?.playWhenReady ?? true;
      const plugin = await loadPluginApi();
      await plugin.seek(Math.max(0, Math.floor(positionSec * 1000)), {
        autoPlay: playWhenReady,
      });
    },

    skipTo: async (index, loadOptions) => {
      const playWhenReady = loadOptions?.playWhenReady ?? true;
      const plugin = await loadPluginApi();
      sessionIndex = index;
      await ensureNativeQueueSynced(plugin, sessionQueue);
      await plugin.playTrackAtIndex(
        index,
        playWhenReady,
        loadOptions?.startAtSec !== undefined
          ? Math.max(0, Math.floor(loadOptions.startAtSec * 1000))
          : undefined,
      );
    },

    setPlayMode: async (mode) => {
      const plugin = await loadPluginApi();
      sessionPlayMode = mode;
      await plugin.setPlayMode(mode as NativePlayMode);
    },

    syncQueue: async (queue, currentIndexValue) => {
      const plugin = await loadPluginApi();
      sessionQueue = queue;
      sessionIndex = currentIndexValue;
      const payload: PlayingQueue = {
        songs: queue.map(toQueueSong),
        currentIndex: currentIndexValue >= 0 ? currentIndexValue : null,
      };
      await plugin.setPlayingQueue(payload, sessionPlayMode as NativePlayMode);
      lastPushedKey = queuePushKey(queue);
    },

    setVolume: async (volume) => {
      const plugin = await loadPluginApi();
      await plugin.setVolume({ volume: Math.min(1, Math.max(0, volume)) });
    },

    syncNormalizationConfig: async (config: NormalizationConfigPayload) => {
      const plugin = await loadPluginApi();
      await plugin.setNormalizationConfig({
        mode: config.mode,
        manualVolume: Math.min(1, Math.max(0, config.manualVolume)),
        fixedLufs: config.fixedLufs,
        lufsPrecacheCount: config.lufsPrecacheCount,
      });
    },

    pauseAfter: async (delayMs) => {
      const plugin = await loadPluginApi();
      await plugin.pauseAfter(Math.max(0, Math.floor(delayMs)));
    },

    clearPlaybackState: async () => {
      const plugin = await loadPluginApi();
      await plugin.stop();
      await plugin.setPlayingQueue(
        { songs: [], currentIndex: null },
        sessionPlayMode as NativePlayMode,
      );
      sessionQueue = [];
      sessionIndex = -1;
      lastPushedKey = null;
    },

    on: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },

    dispose: () => {
      disposed = true;
      listeners.clear();
      unlistenEvents?.();
      unlistenEvents = null;
    },
  };

  return backend;
}
